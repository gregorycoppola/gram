// gram/src/core/matcher/engine.rs
use std::collections::BTreeMap;

use crate::core::construct::{apply_constructor, Arg};
use crate::core::fixture::Span;
use crate::core::grammar::{is_punctuation, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
use crate::core::value::{SemValue, VarGen};

use super::tree::{build_syntax_tree_for_result, offset_constituents};
use super::types::*;

// --- Pattern matching (cache-aware) ---

const MAX_DERIVATIONS_PER_SPAN: usize = 10_000;

#[derive(Debug, Clone)]
struct PatternMatch {
    bindings: BTreeMap<String, (String, String)>,
    log: Vec<Consumption>,
    constituents: Vec<Constituent>,
    child_derivation_keys: Vec<String>,
}

pub fn try_pattern_matches(
    tokens: &[String],
    global_offset: usize,
    span: &Span,
    direct_children: &[SpanKey],
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    cache: &SpanCache,
    mut trace: Option<&mut DebugTrace>,
) -> Result<Vec<SpanResult>, String> {
    let filter = sub_filter_for_label(&span.label);
    let base_var_gen = var_gen.clone();
    let mut maximum_position = base_var_gen.position();
    let mut results = Vec::new();

    if let Some(t) = &mut trace {
        t.enter(&format!(
            "pattern match [{}] (filter: {}, direct children: {})",
            span.label,
            filter_label(&filter),
            direct_children.len()
        ));
    }

    for rule in rules {
        match &filter {
            KindFilter::Any => {}
            KindFilter::Only(kind) if rule.kind != *kind => continue,
            KindFilter::Only(_) => {}
        }

        if let Some(t) = &mut trace {
            t.push(&format!(
                "try: {} (kind: {}): {}",
                rule.name, rule.kind, rule.pattern_str
            ));
        }

        let matches = match_pattern(
            &rule.pattern,
            0,
            tokens,
            0,
            BTreeMap::new(),
            Vec::new(),
            lexicon,
            0,
            Vec::new(),
            Vec::new(),
            direct_children,
            cache,
            global_offset,
        );

        if matches.is_empty() {
            if let Some(t) = &mut trace {
                t.push("  ✗ pattern didn't match");
            }
            continue;
        }

        for pattern_match in matches {
            let PatternMatch {
                bindings,
                log,
                constituents,
                child_derivation_keys,
            } = pattern_match;

            let local_annotations = annotate_tokens(tokens, &log);
            let global_constituents = offset_constituents(&constituents, global_offset);
            let mut branch_var_gen = base_var_gen.clone();

            let (sem_value, output) = match resolve_and_construct(
                &rule.sem,
                &bindings,
                &constituents,
                &mut branch_var_gen,
            ) {
                Ok(result) => result,
                Err(error) => {
                    if let Some(t) = &mut trace {
                        t.push(&format!("  ✗ sem construct failed: {}", error));
                    }
                    continue;
                }
            };

            maximum_position = maximum_position.max(branch_var_gen.position());

            let derivation_key = format!(
                "{}|{}|{:?}|{:?}",
                rule.name, rule.pattern_str, bindings, child_derivation_keys
            );

            if let Some(t) = &mut trace {
                t.push(&format!("  ✓ → {}", output));
            }

            results.push(SpanResult {
                sem_value,
                output,
                rule_name: rule.name.clone(),
                pattern: rule.pattern_str.clone(),
                kind: rule.kind.clone(),
                bindings,
                token_annotations: local_annotations,
                constituents: global_constituents,
                derivation_key,
            });

            if results.len() > MAX_DERIVATIONS_PER_SPAN {
                return Err(format!(
                    "span [{}] tokens[{}..{}] exceeded the derivation limit of {}",
                    span.label, span.start, span.end, MAX_DERIVATIONS_PER_SPAN
                ));
            }
        }
    }

    results.sort_by(|left, right| {
        left.derivation_key
            .cmp(&right.derivation_key)
            .then_with(|| left.output.cmp(&right.output))
    });
    results.dedup_by(|left, right| left.derivation_key == right.derivation_key);

    var_gen.advance_to(maximum_position);

    if let Some(t) = &mut trace {
        if results.is_empty() {
            t.push("NO PATTERN MATCH");
        } else {
            t.push(&format!("{} derivation(s)", results.len()));
        }
        t.leave();
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn match_pattern(
    pattern: &[Slot],
    pattern_index: usize,
    tokens: &[String],
    token_index: usize,
    bindings: BTreeMap<String, (String, String)>,
    log: Vec<Consumption>,
    lexicon: &Lexicon,
    sub_count: usize,
    constituents: Vec<Constituent>,
    child_derivation_keys: Vec<String>,
    direct_children: &[SpanKey],
    cache: &SpanCache,
    global_offset: usize,
) -> Vec<PatternMatch> {
    let next_is_literal =
        pattern_index < pattern.len() && matches!(pattern[pattern_index], Slot::Literal(_));

    let mut token_index = token_index;
    let mut log = log;

    if !next_is_literal {
        while token_index < tokens.len() && is_punctuation(&clean_token(&tokens[token_index])) {
            log.push(Consumption::Skipped {
                position: token_index,
            });
            token_index += 1;
        }
    }

    if pattern_index >= pattern.len() {
        while token_index < tokens.len() && is_punctuation(&clean_token(&tokens[token_index])) {
            log.push(Consumption::Skipped {
                position: token_index,
            });
            token_index += 1;
        }

        if token_index == tokens.len() {
            return vec![PatternMatch {
                bindings,
                log,
                constituents,
                child_derivation_keys,
            }];
        }

        return Vec::new();
    }

    if token_index >= tokens.len() {
        return Vec::new();
    }

    let slot = &pattern[pattern_index];
    let token = clean_token(&tokens[token_index]);

    match slot {
        Slot::Literal(literal) => {
            if &token == literal {
                let mut next_log = log;
                next_log.push(Consumption::Literal {
                    position: token_index,
                });

                match_pattern(
                    pattern,
                    pattern_index + 1,
                    tokens,
                    token_index + 1,
                    bindings,
                    next_log,
                    lexicon,
                    sub_count,
                    constituents,
                    child_derivation_keys,
                    direct_children,
                    cache,
                    global_offset,
                )
            } else if is_punctuation(&token) {
                let mut next_log = log;
                next_log.push(Consumption::Skipped {
                    position: token_index,
                });

                match_pattern(
                    pattern,
                    pattern_index,
                    tokens,
                    token_index + 1,
                    bindings,
                    next_log,
                    lexicon,
                    sub_count,
                    constituents,
                    child_derivation_keys,
                    direct_children,
                    cache,
                    global_offset,
                )
            } else {
                Vec::new()
            }
        }
        Slot::Keyword(keyword) => {
            if matches_keyword(&token, keyword) {
                let mut next_log = log;
                next_log.push(Consumption::Keyword {
                    class: keyword.clone(),
                    position: token_index,
                });

                match_pattern(
                    pattern,
                    pattern_index + 1,
                    tokens,
                    token_index + 1,
                    bindings,
                    next_log,
                    lexicon,
                    sub_count,
                    constituents,
                    child_derivation_keys,
                    direct_children,
                    cache,
                    global_offset,
                )
            } else if is_punctuation(&token) {
                let mut next_log = log;
                next_log.push(Consumption::Skipped {
                    position: token_index,
                });

                match_pattern(
                    pattern,
                    pattern_index,
                    tokens,
                    token_index + 1,
                    bindings,
                    next_log,
                    lexicon,
                    sub_count,
                    constituents,
                    child_derivation_keys,
                    direct_children,
                    cache,
                    global_offset,
                )
            } else {
                Vec::new()
            }
        }
        Slot::Var {
            name,
            type_constraint,
        } => {
            if let Some((canonical, _category, consumed)) = lexicon.lookup_at(tokens, token_index) {
                let actual_type = lexicon.get_type(&canonical).unwrap_or_default();
                let type_ok = type_constraint
                    .as_ref()
                    .is_none_or(|expected| actual_type == *expected);

                if type_ok {
                    let mut next_bindings = bindings.clone();
                    let mut next_log = log.clone();

                    next_bindings.insert(name.clone(), (canonical.clone(), actual_type.clone()));
                    next_log.push(Consumption::Var {
                        variable: name.clone(),
                        canonical,
                        typ: actual_type,
                        start: token_index,
                        end: token_index + consumed,
                    });

                    let results = match_pattern(
                        pattern,
                        pattern_index + 1,
                        tokens,
                        token_index + consumed,
                        next_bindings,
                        next_log,
                        lexicon,
                        sub_count,
                        constituents.clone(),
                        child_derivation_keys.clone(),
                        direct_children,
                        cache,
                        global_offset,
                    );

                    if !results.is_empty() {
                        return results;
                    }
                }
            }

            if is_punctuation(&token) {
                let mut next_log = log;
                next_log.push(Consumption::Skipped {
                    position: token_index,
                });

                return match_pattern(
                    pattern,
                    pattern_index,
                    tokens,
                    token_index + 1,
                    bindings,
                    next_log,
                    lexicon,
                    sub_count,
                    constituents,
                    child_derivation_keys,
                    direct_children,
                    cache,
                    global_offset,
                );
            }

            Vec::new()
        }
        Slot::Sub {
            label, delimiter, ..
        } => {
            let Some(child) = direct_children
                .iter()
                .find(|child| child.start == global_offset + token_index && child.label == *label)
            else {
                return Vec::new();
            };

            let local_end = child.end.saturating_sub(global_offset);
            if local_end > tokens.len() || local_end <= token_index {
                return Vec::new();
            }

            if let Some(delimiter_keyword) = delimiter {
                let delimiter_position = (token_index..tokens.len())
                    .find(|index| matches_keyword(&clean_token(&tokens[*index]), delimiter_keyword))
                    .unwrap_or(tokens.len());

                if child.end != global_offset + delimiter_position {
                    return Vec::new();
                }
            }

            let Some(cached_results) = cache.get(child) else {
                return Vec::new();
            };

            let mut results = Vec::new();

            for cached in cached_results {
                let mut next_bindings = bindings.clone();
                let mut next_log = log.clone();
                let mut next_constituents = constituents.clone();
                let mut next_child_keys = child_derivation_keys.clone();

                next_log.push(Consumption::SubClause {
                    start: token_index,
                    end: local_end,
                });

                let sub_key = if sub_count == 0 {
                    "SUB".to_string()
                } else {
                    format!("SUB{}", sub_count)
                };

                next_bindings.insert(sub_key, (cached.output.clone(), label.clone()));

                let child_tokens = &tokens[token_index..local_end];
                let syntax_tree = Some(build_syntax_tree_for_result(
                    cached,
                    child_tokens,
                    child.start,
                ));

                next_constituents.push(Constituent {
                    label: label.clone(),
                    semantics: cached.output.clone(),
                    free_vars: Vec::new(),
                    span: Some((token_index, local_end)),
                    syntax: None,
                    syntax_tree,
                    sem_value: Some(cached.sem_value.clone()),
                    children: Vec::new(),
                    rule_name: cached.rule_name.clone(),
                    pattern: cached.pattern.clone(),
                });
                next_child_keys.push(cached.derivation_key.clone());

                results.extend(match_pattern(
                    pattern,
                    pattern_index + 1,
                    tokens,
                    local_end,
                    next_bindings,
                    next_log,
                    lexicon,
                    sub_count + 1,
                    next_constituents,
                    next_child_keys,
                    direct_children,
                    cache,
                    global_offset,
                ));
            }

            results
        }
    }
}

fn resolve_and_construct(
    sem_spec: &crate::core::sem_dsl::SemSpec,
    bindings: &BTreeMap<String, (String, String)>,
    constituents: &[Constituent],
    var_gen: &mut VarGen,
) -> Result<(SemValue, String), String> {
    let mut sub_values: BTreeMap<String, SemValue> = BTreeMap::new();
    for (i, constituent) in constituents.iter().enumerate() {
        let key = if i == 0 {
            "SUB".into()
        } else {
            format!("SUB{}", i)
        };
        if let Some(ref sv) = constituent.sem_value {
            sub_values.insert(key, sv.clone());
        }
    }

    let mut args: Vec<Arg> = Vec::new();
    for sem_arg in &sem_spec.args {
        match sem_arg {
            SemArg::Slot(name) => {
                let key = name.strip_prefix('$').unwrap_or(name);
                if let Some(sv) = sub_values.get(key) {
                    args.push(Arg::Sub(sv.clone()));
                } else if let Some((canonical, typ)) = bindings.get(&format!("${}", key)) {
                    args.push(Arg::Lexical(canonical.clone(), typ.clone()));
                } else if let Some((canonical, typ)) = bindings.get(key) {
                    args.push(Arg::Lexical(canonical.clone(), typ.clone()));
                } else {
                    return Err(format!("sem: cannot resolve slot ${}", name));
                }
            }
            SemArg::Literal(s) => {
                args.push(Arg::Literal(s.clone()));
            }
        }
    }

    let sv = apply_constructor(&sem_spec.constructor, &args, var_gen)?;
    let out = format!("{}", sv);
    Ok((sv, out))
}

// --- Annotation helpers ---

pub fn annotate_tokens(tokens: &[String], log: &[Consumption]) -> Vec<TokenAnnotation> {
    let mut out: Vec<TokenAnnotation> = (0..tokens.len())
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched,
            canonical: None,
            typ: None,
            variable: None,
            keyword_class: None,
        })
        .collect();
    for c in log {
        match c {
            Consumption::Var {
                variable,
                canonical,
                typ,
                start,
                end,
            } => {
                let kind = if typ == "e" {
                    TokenKind::Entity
                } else if typ.starts_with('{') {
                    TokenKind::Predicate
                } else {
                    TokenKind::Entity
                };
                for i in *start..*end {
                    if i < out.len() {
                        out[i] = TokenAnnotation {
                            kind,
                            canonical: Some(canonical.clone()),
                            typ: Some(typ.clone()),
                            variable: Some(variable.clone()),
                            keyword_class: None,
                        };
                    }
                }
            }
            Consumption::Keyword { class, position } => {
                if *position < out.len() {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Keyword,
                        canonical: None,
                        typ: None,
                        variable: None,
                        keyword_class: Some(class.clone()),
                    };
                }
            }
            Consumption::Literal { position } => {
                if *position < out.len() {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Literal,
                        canonical: None,
                        typ: None,
                        variable: None,
                        keyword_class: None,
                    };
                }
            }
            Consumption::SubClause { start, end } => {
                for i in *start..*end {
                    if i < out.len()
                        && matches!(out[i].kind, TokenKind::Unmatched | TokenKind::Ignored)
                    {
                        out[i] = TokenAnnotation {
                            kind: TokenKind::SubClause,
                            canonical: None,
                            typ: None,
                            variable: None,
                            keyword_class: None,
                        };
                    }
                }
            }
            Consumption::Skipped { position } => {
                if *position < out.len() && matches!(out[*position].kind, TokenKind::Unmatched) {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Ignored,
                        canonical: None,
                        typ: None,
                        variable: None,
                        keyword_class: None,
                    };
                }
            }
        }
    }
    out
}

fn sub_filter_for_label(label: &str) -> KindFilter {
    match label {
        "dp" => KindFilter::Only("dp".to_string()),
        "n" => KindFilter::Only("n".to_string()),
        "s\\agent" => KindFilter::Only("s\\agent".to_string()),
        "s\\patient" => KindFilter::Only("s\\patient".to_string()),
        "s\\theme" => KindFilter::Only("s\\theme".to_string()),
        "s" => KindFilter::Only("s".to_string()),
        _ => KindFilter::Any,
    }
}
