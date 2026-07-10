use std::collections::{BTreeMap, HashMap};

use crate::core::construct::{Arg, apply_constructor};
use crate::core::fixture::Span;
use crate::core::grammar::{is_punctuation, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
use crate::core::template::apply_template;
use crate::core::value::{SemValue, VarGen};

use super::types::*;
use super::tree::{build_syntax_tree_for_result, offset_constituents};

// --- Pattern matching (cache-aware) ---

pub fn try_pattern_match(
    tokens: &[String],
    global_offset: usize,
    span: &Span,
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    cache: &HashMap<SpanKey, SpanResult>,
    mut trace: Option<&mut DebugTrace>,
) -> Option<SpanResult> {
    let filter = sub_filter_for_label(&span.label);

    if let Some(t) = &mut trace {
        t.enter(&format!("pattern match [{}] (filter: {})", span.label, filter_label(&filter)));
    }

    for rule in rules {
        match &filter {
            KindFilter::Any => {}
            KindFilter::Only(k) if rule.kind != *k => continue,
            KindFilter::Only(_) => {}
        }
        if let Some(t) = &mut trace {
            t.push(&format!("try: {} (kind: {}): {}", rule.name, rule.kind, rule.pattern_str));
        }
        let bindings = BTreeMap::new();
        let log: Vec<Consumption> = Vec::new();
        let constituents = Vec::new();
        if let Some((b, log, constituents)) = match_pattern(
            &rule.pattern, 0, tokens, 0, bindings, log, lexicon, rules, 0,
            constituents, &filter, var_gen, cache, global_offset,
        ) {
            let local_annotations = annotate_tokens(tokens, &log);
            let global_constituents = offset_constituents(&constituents, global_offset);

            let (output, sem_value) = if let Some(ref sem_spec) = rule.sem {
                match resolve_and_construct(sem_spec, &b, &constituents, var_gen) {
                    Ok((sv, out)) => (out, Some(sv)),
                    Err(e) => {
                        if let Some(t) = &mut trace {
                            t.push(&format!("  ✗ sem construct failed: {}", e));
                        }
                        continue;
                    }
                }
            } else {
                let out = apply_template(&rule.template, &b);
                (out, None)
            };

            if let Some(t) = &mut trace {
                t.push(&format!("  ✓ → {}", output));
            }

            return Some(SpanResult {
                sem_value: sem_value.unwrap_or_else(|| SemValue::Prop(crate::core::logic::Expr::Entity("_no_sem".into()))),
                output,
                rule_name: rule.name.clone(),
                pattern: rule.pattern_str.clone(),
                kind: rule.kind.clone(),
                bindings: b,
                token_annotations: local_annotations,
                constituents: global_constituents,
            });
        } else {
            if let Some(t) = &mut trace {
                t.push("  ✗ pattern didn't match");
            }
        }
    }

    if let Some(t) = &mut trace {
        t.push("NO PATTERN MATCH");
        t.leave();
    }

    None
}

fn match_pattern(
    pattern: &[Slot], pi: usize, tokens: &[String], ti: usize,
    bindings: BTreeMap<String, (String, String)>, log: Vec<Consumption>,
    lexicon: &Lexicon, rules: &[Rule], sub_count: usize,
    constituents: Vec<Constituent>, kind_filter: &KindFilter,
    var_gen: &mut VarGen,
    cache: &HashMap<SpanKey, SpanResult>,
    global_offset: usize,
) -> Option<(BTreeMap<String, (String, String)>, Vec<Consumption>, Vec<Constituent>)> {
    let next_is_literal = pi < pattern.len()
        && matches!(pattern[pi], Slot::Literal(_));

    let mut ti = ti;
    let mut log = log;
    if !next_is_literal {
        while ti < tokens.len() && is_punctuation(&clean_token(&tokens[ti])) {
            log.push(Consumption::Skipped { position: ti });
            ti += 1;
        }
    }

    if pi >= pattern.len() {
        let mut ti = ti;
        let mut log = log;
        while ti < tokens.len() && is_punctuation(&clean_token(&tokens[ti])) {
            log.push(Consumption::Skipped { position: ti });
            ti += 1;
        }
        if ti >= tokens.len() {
            return Some((bindings, log, constituents));
        }
        return None;
    }

    if ti >= tokens.len() {
        return None;
    }

    let slot = &pattern[pi];
    let token = clean_token(&tokens[ti]);

    match slot {
        Slot::Literal(lit) => {
            if &token == lit {
                let mut log = log;
                log.push(Consumption::Literal { position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset)
            } else if is_punctuation(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset)
            } else {
                None
            }
        }
        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                let mut log = log;
                log.push(Consumption::Keyword { class: kw.clone(), position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset)
            } else if is_punctuation(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset)
            } else {
                None
            }
        }
        Slot::Var { name, type_constraint } => {
            if let Some((canonical, _cat, consumed)) = lexicon.lookup_at(tokens, ti) {
                let actual_type = lexicon.get_type(&canonical).unwrap_or_default();
                let type_ok = match type_constraint {
                    Some(t) => &actual_type == t,
                    None => true,
                };
                if type_ok {
                    let mut new_bindings = bindings.clone();
                    let mut new_log = log.clone();
                    new_bindings.insert(name.clone(), (canonical.clone(), actual_type.clone()));
                    new_log.push(Consumption::Var {
                        variable: name.clone(), canonical, typ: actual_type, start: ti, end: ti + consumed,
                    });
                    if let Some(result) = match_pattern(
                        pattern, pi + 1, tokens, ti + consumed,
                        new_bindings, new_log, lexicon, rules, sub_count, constituents.clone(), kind_filter,
                        var_gen, cache, global_offset,
                    ) {
                        return Some(result);
                    }
                }
            }
            if is_punctuation(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset);
            }
            None
        }
        Slot::Sub { label, delimiter, .. } => {
            let sub_start = ti;
            let mut sub_ti = ti;
            while sub_ti < tokens.len() && is_punctuation(&clean_token(&tokens[sub_ti])) {
                log.push(Consumption::Skipped { position: sub_ti });
                sub_ti += 1;
            }

            if let Some(delim_kw) = delimiter {
                let mut end = tokens.len();
                for i in sub_ti..tokens.len() {
                    if matches_keyword(&clean_token(&tokens[i]), delim_kw) {
                        end = i;
                        break;
                    }
                }
                let sub_tokens = &tokens[sub_ti..end];
                if sub_tokens.is_empty() {
                    return None;
                }
                let cache_key = SpanKey {
                    start: global_offset + sub_start,
                    end: global_offset + end,
                    label: label.clone(),
                };
                if let Some(cached) = cache.get(&cache_key) {
                    let mut new_bindings = bindings.clone();
                    let mut new_log = log.clone();
                    new_log.push(Consumption::SubClause { start: sub_start, end });
                    let sub_key = if sub_count == 0 { "SUB".into() } else { format!("SUB{}", sub_count) };
                    new_bindings.insert(sub_key, (cached.output.clone(), label.clone()));
                    let sub_tokens_slice = &tokens[sub_start..end];
                    let syntax_tree = Some(build_syntax_tree_for_result(cached, sub_tokens_slice, global_offset + sub_start));
                    let constituent = Constituent {
                        label: label.clone(),
                        semantics: cached.output.clone(),
                        free_vars: Vec::new(),
                        span: Some((sub_start, end)),
                        syntax: None,
                        syntax_tree,
                        sem_value: Some(cached.sem_value.clone()),
                        children: vec![],
                        rule_name: cached.rule_name.clone(),
                        pattern: cached.pattern.clone(),
                    };
                    let mut trial_constituents = constituents;
                    trial_constituents.push(constituent);
                    return match_pattern(
                        pattern, pi + 1, tokens, end,
                        new_bindings, new_log, lexicon, rules, sub_count + 1,
                        trial_constituents, kind_filter, var_gen, cache, global_offset,
                    );
                } else {
                    None
                }
            } else {
                let max_end = tokens.len();
                for end in (sub_ti + 1)..=max_end {
                    let cache_key = SpanKey {
                        start: global_offset + sub_start,
                        end: global_offset + end,
                        label: label.clone(),
                    };
                    if let Some(cached) = cache.get(&cache_key) {
                        let mut trial_constituents = constituents.clone();
                        let mut new_bindings = bindings.clone();
                        let mut new_log = log.clone();
                        new_log.push(Consumption::SubClause { start: sub_start, end });
                        let sub_key = if sub_count == 0 { "SUB".into() } else { format!("SUB{}", sub_count) };
                        new_bindings.insert(sub_key, (cached.output.clone(), label.clone()));
                        let sub_tokens_slice = &tokens[sub_start..end];
                        let syntax_tree = Some(build_syntax_tree_for_result(cached, sub_tokens_slice, global_offset + sub_start));
                        trial_constituents.push(Constituent {
                            label: label.clone(),
                            semantics: cached.output.clone(),
                            free_vars: Vec::new(),
                            span: Some((sub_start, end)),
                            syntax: None,
                            syntax_tree,
                            sem_value: Some(cached.sem_value.clone()),
                            children: vec![],
                            rule_name: cached.rule_name.clone(),
                            pattern: cached.pattern.clone(),
                        });
                        if let Some(result) = match_pattern(
                            pattern, pi + 1, tokens, end,
                            new_bindings, new_log, lexicon, rules, sub_count + 1,
                            trial_constituents, kind_filter, var_gen, cache, global_offset,
                        ) {
                            return Some(result);
                        }
                    }
                }
                None
            }
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
        let key = if i == 0 { "SUB".into() } else { format!("SUB{}", i) };
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
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for c in log {
        match c {
            Consumption::Var { variable, canonical, typ, start, end } => {
                let kind = if typ == "e" { TokenKind::Entity }
                    else if typ.starts_with('{') { TokenKind::Predicate }
                    else { TokenKind::Entity };
                for i in *start..*end {
                    if i < out.len() {
                        out[i] = TokenAnnotation {
                            kind, canonical: Some(canonical.clone()), typ: Some(typ.clone()),
                            variable: Some(variable.clone()), keyword_class: None,
                        };
                    }
                }
            }
            Consumption::Keyword { class, position } => {
                if *position < out.len() {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Keyword, canonical: None, typ: None, variable: None,
                        keyword_class: Some(class.clone()),
                    };
                }
            }
            Consumption::Literal { position } => {
                if *position < out.len() {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Literal, canonical: None, typ: None, variable: None, keyword_class: None,
                    };
                }
            }
            Consumption::SubClause { start, end } => {
                for i in *start..*end {
                    if i < out.len() && matches!(out[i].kind, TokenKind::Unmatched | TokenKind::Ignored) {
                        out[i] = TokenAnnotation {
                            kind: TokenKind::SubClause, canonical: None, typ: None, variable: None, keyword_class: None,
                        };
                    }
                }
            }
            Consumption::Skipped { position } => {
                if *position < out.len() && matches!(out[*position].kind, TokenKind::Unmatched) {
                    out[*position] = TokenAnnotation {
                        kind: TokenKind::Ignored, canonical: None, typ: None, variable: None, keyword_class: None,
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
        "s\\agent" => KindFilter::Only("s\\agent".to_string()),
        "s\\patient" => KindFilter::Only("s\\patient".to_string()),
        "s\\theme" => KindFilter::Only("s\\theme".to_string()),
        "s" => KindFilter::Only("s".to_string()),
        _ => KindFilter::Any,
    }
}