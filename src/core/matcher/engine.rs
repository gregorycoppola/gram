use std::collections::{BTreeMap, HashMap, HashSet};

use crate::core::construct::{Arg, apply_constructor};
use crate::core::fixture::Span;
use crate::core::grammar::{is_punctuation, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
use crate::core::template::apply_template;
use crate::core::value::{SemValue, VarGen};

use super::types::*;
use super::tree::{build_syntax_tree_for_result, build_syntax_tree, build_syntax_tree_from_constituent, build_syntax, offset_constituents};

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
            t.push(&format!("try: {} (kind: {})", rule.name, rule.kind));
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
    let next_is_literal_or_ignore = pi < pattern.len()
        && matches!(pattern[pi], Slot::Literal(_) | Slot::Ignore);

    let mut ti = ti;
    let mut log = log;
    if !next_is_literal_or_ignore {
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
        Slot::Ignore => {
            let mut log = log;
            log.push(Consumption::Ignore { position: ti });
            match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen, cache, global_offset)
        }
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
                    let sub_key = if sub_count == 0 { "SUB".into() } else { format!("SUB{}", sub_count + 1) };
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
                        let sub_key = if sub_count == 0 { "SUB".into() } else { format!("SUB{}", sub_count + 1) };
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
        let key = if i == 0 { "SUB".into() } else { format!("SUB{}", i + 1) };
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

// --- Assembly ---

pub fn try_assembly(
    span_tokens: &[String],
    global_start: usize,
    span: &Span,
    direct_children: &[SpanKey],
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    cache: &HashMap<SpanKey, SpanResult>,
    mut trace: Option<&mut DebugTrace>,
) -> Option<SpanResult> {
    let mut sorted_children: Vec<&SpanKey> = direct_children.iter().collect();
    sorted_children.sort_by_key(|k| k.start);

    let mut covered: HashSet<usize> = HashSet::new();
    for child_key in &sorted_children {
        for i in child_key.start..child_key.end {
            covered.insert(i - global_start);
        }
    }
    let mut gap_entries: Vec<(String, String, usize)> = Vec::new();
    for (i, _token) in span_tokens.iter().enumerate() {
        if !covered.contains(&i) {
            if let Some((canonical, _cat, _consumed)) = lexicon.lookup_at(span_tokens, i) {
                let typ = lexicon.get_type(&canonical).unwrap_or_default();
                gap_entries.push((canonical, typ, i));
            }
        }
    }

    if let Some(t) = &mut trace {
        t.enter(&format!("assembly: {} children, {} gaps", sorted_children.len(), gap_entries.len()));
        for (i, child_key) in sorted_children.iter().enumerate() {
            let cached = cache.get(*child_key).unwrap();
            t.push(&format!("  child[{}]: [{}] {}..{} → {}", i, child_key.label, child_key.start, child_key.end, cached.output));
        }
        for (canonical, typ, pos) in &gap_entries {
            t.push(&format!("  gap: {} ({}) at local {}", canonical, typ, pos));
        }
    }

    let child_results: Vec<&SpanResult> = sorted_children.iter()
        .map(|k| cache.get(k).unwrap())
        .collect();

    for rule in rules {
        if rule.kind != span.label { continue; }
        let sem_spec = match &rule.sem {
            Some(s) => s,
            None => continue,
        };

        if let Some(t) = &mut trace {
            t.push(&format!("try: {} (sem args: {:?})", rule.name, sem_spec.args));
        }

        let mut consumed_children: HashSet<usize> = HashSet::new();
        let mut gap_idx = 0;
        let mut args: Vec<Arg> = Vec::new();
        let mut ok = true;

        for sem_arg in &sem_spec.args {
            match sem_arg {
                SemArg::Slot(name) => {
                    let key = name.strip_prefix('$').unwrap_or(name);
                    if let Some(idx) = parse_sub_index(key) {
                        if idx < child_results.len() {
                            consumed_children.insert(idx);
                            args.push(Arg::Sub(child_results[idx].sem_value.clone()));
                            continue;
                        }
                    }
                    if gap_idx < gap_entries.len() {
                        let (canonical, typ, _) = &gap_entries[gap_idx];
                        args.push(Arg::Lexical(canonical.clone(), typ.clone()));
                        gap_idx += 1;
                    } else {
                        ok = false;
                        break;
                    }
                }
                SemArg::Literal(s) => {
                    args.push(Arg::Literal(s.clone()));
                }
            }
        }

        if !ok { continue; }
        if consumed_children.len() != child_results.len() { continue; }
        if gap_idx != gap_entries.len() { continue; }

        match apply_constructor(&sem_spec.constructor, &args, var_gen) {
            Ok(sv) => {
                let output = format!("{}", sv);
                if let Some(t) = &mut trace {
                    t.push(&format!("  ✓ SUCCESS → {}", output));
                }

                let constituents: Vec<Constituent> = sorted_children.iter().zip(child_results.iter()).map(|(child_key, cached)| {
                    let child_local_start = child_key.start - global_start;
                    let child_local_end = child_key.end - global_start;
                    let child_tokens = &span_tokens[child_local_start..child_local_end];
                    let syntax_tree = Some(build_syntax_tree_for_result(cached, child_tokens, child_key.start));
                    Constituent {
                        label: child_key.label.clone(),
                        semantics: cached.output.clone(),
                        free_vars: Vec::new(),
                        span: Some((child_key.start, child_key.end)),
                        syntax: None,
                        syntax_tree,
                        sem_value: Some(cached.sem_value.clone()),
                        children: cached.constituents.clone(),
                    }
                }).collect();

                let mut annotations: Vec<TokenAnnotation> = span_tokens.iter()
                    .map(|_| TokenAnnotation {
                        kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
                    })
                    .collect();
                for child_key in &sorted_children {
                    for i in child_key.start..child_key.end {
                        let local_i = i - global_start;
                        if local_i < annotations.len() {
                            annotations[local_i] = TokenAnnotation {
                                kind: TokenKind::SubClause, canonical: None, typ: None, variable: None, keyword_class: None,
                            };
                        }
                    }
                }
                for (canonical, typ, pos) in &gap_entries {
                    if *pos < annotations.len() {
                        let kind = if typ == "e" { TokenKind::Entity }
                            else if typ.starts_with('{') { TokenKind::Predicate }
                            else { TokenKind::Entity };
                        annotations[*pos] = TokenAnnotation {
                            kind, canonical: Some(canonical.clone()), typ: Some(typ.clone()),
                            variable: None, keyword_class: None,
                        };
                    }
                }

                if let Some(t) = &mut trace { t.leave(); }
                return Some(SpanResult {
                    sem_value: sv, output,
                    rule_name: rule.name.clone(), kind: rule.kind.clone(),
                    bindings: BTreeMap::new(),
                    token_annotations: annotations, constituents,
                });
            }
            Err(e) => {
                if let Some(t) = &mut trace {
                    t.push(&format!("  FAIL: constructor error: {}", e));
                }
            }
        }
    }

    if let Some(t) = &mut trace {
        t.push("NO ASSEMBLY MATCH");
        t.leave();
    }
    None
}

pub fn try_assemble_top_level(
    top_keys: &[&SpanKey],
    cache: &HashMap<SpanKey, SpanResult>,
    all_tokens: &[String],
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    mut trace: Option<&mut DebugTrace>,
) -> Option<Match> {
    let child_results: Vec<&SpanResult> = top_keys.iter()
        .map(|k| cache.get(*k).unwrap())
        .collect();

    let mut covered: HashSet<usize> = HashSet::new();
    for key in top_keys {
        for i in key.start..key.end {
            covered.insert(i);
        }
    }
    let mut gap_entries: Vec<(String, String, usize)> = Vec::new();
    for (i, _token) in all_tokens.iter().enumerate() {
        if !covered.contains(&i) {
            if let Some((canonical, _cat, _consumed)) = lexicon.lookup_at(all_tokens, i) {
                let typ = lexicon.get_type(&canonical).unwrap_or_default();
                gap_entries.push((canonical, typ, i));
            }
        }
    }

    if let Some(t) = &mut trace {
        t.push(&format!("top-level assembly: {} children, {} gaps", child_results.len(), gap_entries.len()));
        for (i, (key, cached)) in top_keys.iter().zip(child_results.iter()).enumerate() {
            t.push(&format!("  child[{}]: [{}] {}..{} → {}", i, key.label, key.start, key.end, cached.output));
        }
        for (canonical, typ, pos) in &gap_entries {
            t.push(&format!("  gap: {} ({}) at {}", canonical, typ, pos));
        }
    }

    for rule in rules {
        if rule.kind != "s" { continue; }
        let sem_spec = match &rule.sem { Some(s) => s, None => continue };

        if let Some(t) = &mut trace {
            t.push(&format!("try: {} (sem args: {:?})", rule.name, sem_spec.args));
        }

        let mut consumed_children: HashSet<usize> = HashSet::new();
        let mut gap_idx = 0;
        let mut args: Vec<Arg> = Vec::new();
        let mut ok = true;

        for sem_arg in &sem_spec.args {
            match sem_arg {
                SemArg::Slot(name) => {
                    let key = name.strip_prefix('$').unwrap_or(name);
                    if let Some(idx) = parse_sub_index(key) {
                        if idx < child_results.len() {
                            consumed_children.insert(idx);
                            args.push(Arg::Sub(child_results[idx].sem_value.clone()));
                            continue;
                        }
                    }
                    if gap_idx < gap_entries.len() {
                        let (canonical, typ, _) = &gap_entries[gap_idx];
                        args.push(Arg::Lexical(canonical.clone(), typ.clone()));
                        gap_idx += 1;
                    } else { ok = false; break; }
                }
                SemArg::Literal(s) => { args.push(Arg::Literal(s.clone())); }
            }
        }

        if !ok { continue; }
        if consumed_children.len() != child_results.len() { continue; }
        if gap_idx != gap_entries.len() { continue; }

        match apply_constructor(&sem_spec.constructor, &args, var_gen) {
            Ok(sv) => {
                let output = format!("{}", sv);
                if let Some(t) = &mut trace {
                    t.push(&format!("  ✓ → {}", output));
                }

                let constituents: Vec<Constituent> = top_keys.iter().zip(child_results.iter()).map(|(key, cached)| {
                    let child_tokens = &all_tokens[key.start..key.end];
                    let syntax_tree = Some(build_syntax_tree_for_result(cached, child_tokens, key.start));
                    Constituent {
                        label: key.label.clone(), semantics: cached.output.clone(),
                        free_vars: Vec::new(),
                        span: Some((key.start, key.end)),
                        syntax: None,
                        syntax_tree,
                        sem_value: Some(cached.sem_value.clone()),
                        children: cached.constituents.clone(),
                    }
                }).collect();

                let mut annotations: Vec<TokenAnnotation> = all_tokens.iter()
                    .map(|_| TokenAnnotation {
                        kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
                    })
                    .collect();
                for key in top_keys {
                    for i in key.start..key.end {
                        if i < annotations.len() {
                            annotations[i] = TokenAnnotation {
                                kind: TokenKind::SubClause, canonical: None, typ: None, variable: None, keyword_class: None,
                            };
                        }
                    }
                }
                for (canonical, typ, pos) in &gap_entries {
                    if *pos < annotations.len() {
                        let kind = if typ == "e" { TokenKind::Entity }
                            else if typ.starts_with('{') { TokenKind::Predicate }
                            else { TokenKind::Entity };
                        annotations[*pos] = TokenAnnotation {
                            kind, canonical: Some(canonical.clone()), typ: Some(typ.clone()),
                            variable: None, keyword_class: None,
                        };
                    }
                }

                let constituent_spans: Vec<(usize, usize, String)> = constituents.iter()
                    .filter_map(|c| c.span.map(|(s, e)| (s, e, format!("[{}]", c.label))))
                    .collect();
                let syntax = build_syntax(all_tokens, &constituent_spans, "S");

                let constituent_trees: Vec<(usize, usize, SyntaxNode)> = constituents.iter()
                    .filter_map(|c| {
                        let (s, e) = c.span?;
                        let tree = c.syntax_tree.clone().unwrap_or_else(|| {
                            if c.children.is_empty() {
                                SyntaxNode {
                                    label: kind_to_syntax_label(&c.label),
                                    children: all_tokens[s..e].iter().filter_map(|t| {
                                        let cleaned = clean_token(t);
                                        if is_punctuation(&cleaned) { None } else {
                                            Some(SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None })
                                        }
                                    }).collect(),
                                    terminal: None,
                                    semantics: None,
                                }
                            } else {
                                build_syntax_tree_from_constituent(c, all_tokens, 0)
                            }
                        });
                        Some((s, e, tree))
                    })
                    .collect();
                let mut syntax_tree = build_syntax_tree(all_tokens, &annotations, &constituent_trees, "S");
                syntax_tree.semantics = Some(output.clone());

                return Some(Match {
                    rule_name: rule.name.clone(), kind: rule.kind.clone(), output,
                    bindings: BTreeMap::new(), token_annotations: annotations, constituents,
                    syntax: Some(syntax), syntax_tree: Some(syntax_tree),
                    semantics_check: None, sem_value: Some(sv),
                });
            }
            Err(_) => continue,
        }
    }

    None
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
            Consumption::Ignore { position } | Consumption::Skipped { position } => {
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