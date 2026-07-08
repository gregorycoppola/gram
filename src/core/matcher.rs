
use std::collections::{BTreeMap, HashSet};

use crate::core::construct::{Arg, apply_constructor};
use crate::core::fixture::{InputSentence, Span};
use crate::core::grammar::{is_ignored, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
use crate::core::semantics::parse_with_types;
use crate::core::template::apply_template;
use crate::core::value::{SemValue, VarGen};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Constituent {
    pub label: String,
    pub semantics: String,
    pub free_vars: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_tree: Option<SyntaxNode>,
    #[serde(skip)]
    pub sem_value: Option<SemValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Constituent>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Match {
    pub rule_name: String,
    pub kind: String,
    pub output: String,
    pub bindings: BTreeMap<String, (String, String)>,
    pub token_annotations: Vec<TokenAnnotation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constituents: Vec<Constituent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_tree: Option<SyntaxNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics_check: Option<String>,
    #[serde(skip)]
    pub sem_value: Option<SemValue>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyntaxNode {
    pub label: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SyntaxNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
}

impl std::fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        render_tree(f, self, 0)
    }
}

fn render_tree(f: &mut std::fmt::Formatter, node: &SyntaxNode, depth: usize) -> std::fmt::Result {
    let indent = "  ".repeat(depth);
    if let Some(ref term) = node.terminal {
        writeln!(f, "{}{} -> \"{}\"", indent, node.label, term)?;
    } else {
        writeln!(f, "{}{}", indent, node.label)?;
        for child in &node.children {
            render_tree(f, child, depth + 1)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenAnnotation {
    pub kind: TokenKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub typ: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_class: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Entity,
    Predicate,
    Keyword,
    Literal,
    Ignored,
    Unmatched,
    SubClause,
}

#[derive(Debug, Clone)]
enum KindFilter {
    Any,
    Only(String),
    Exclude(Vec<String>),
}

#[derive(Debug, Clone)]
enum Consumption {
    Var { variable: String, canonical: String, typ: String, start: usize, end: usize },
    Keyword { class: String, position: usize },
    Literal { position: usize },
    Ignore { position: usize },
    Skipped { position: usize },
    SubClause { start: usize, end: usize },
}

fn kind_to_syntax_label(kind: &str) -> &'static str {
    match kind {
        "dp" => "DP",
        "s_gapped" => "S",
        _ => "S",
    }
}

fn build_syntax(tokens: &[String], constituents: &[(usize, usize, String)], top_label: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut ci = 0;
    let mut skip_until = 0;
    for i in 0..tokens.len() {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&tokens[i]);
        if is_ignored(&cleaned) {
            continue;
        }
        if ci < constituents.len() && i == constituents[ci].0 {
            parts.push(constituents[ci].2.clone());
            skip_until = constituents[ci].1;
            ci += 1;
            continue;
        }
        parts.push(cleaned);
    }
    if parts.is_empty() {
        format!("[{}]", top_label)
    } else {
        format!("[{} {}]", top_label, parts.join(" "))
    }
}

fn build_syntax_tree(
    tokens: &[String],
    annotations: &[TokenAnnotation],
    constituents: &[(usize, usize, SyntaxNode)],
    top_label: &str,
) -> SyntaxNode {
    let mut children = Vec::new();
    let mut ci = 0;
    let mut skip_until = 0;
    for i in 0..tokens.len() {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&tokens[i]);
        if is_ignored(&cleaned) {
            continue;
        }
        if ci < constituents.len() && i == constituents[ci].0 {
            children.push(constituents[ci].2.clone());
            skip_until = constituents[ci].1;
            ci += 1;
            continue;
        }
        let ann = &annotations[i];
        let node = match ann.kind {
            TokenKind::Keyword => {
                let label = ann.keyword_class.as_deref().unwrap_or("KW");
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned) }
            }
            TokenKind::Entity => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned) }
            }
            TokenKind::Predicate => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned) }
            }
            TokenKind::Literal => {
                SyntaxNode { label: "LIT".to_string(), children: vec![], terminal: Some(cleaned) }
            }
            _ => {
                SyntaxNode { label: "UNK".to_string(), children: vec![], terminal: Some(cleaned) }
            }
        };
        children.push(node);
    }
    SyntaxNode { label: top_label.to_string(), children, terminal: None }
}

const CONSTITUENT_KINDS: &[&str] = &["dp", "s_gapped"];

fn sub_filter_for_label(label: &str) -> KindFilter {
    match label {
        "dp" => KindFilter::Only("dp".to_string()),
        "s_gapped" => KindFilter::Only("s_gapped".to_string()),
        "s" => KindFilter::Exclude(CONSTITUENT_KINDS.iter().map(|s| s.to_string()).collect()),
        _ => KindFilter::Any,
    }
}

pub fn parse_sentence(tokens: &[String], lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    let mut var_gen = VarGen::new();
    parse_sentence_inner(tokens, lexicon, rules, &KindFilter::Any, &mut var_gen)
}

pub fn parse_sentence_with_vars(
    tokens: &[String], lexicon: &Lexicon, rules: &[Rule], available_vars: &[(String, String)],
) -> Vec<Match> {
    let mut var_gen = VarGen::new();
    let effective_lexicon = if available_vars.is_empty() {
        return parse_sentence_inner(tokens, lexicon, rules, &KindFilter::Any, &mut var_gen);
    } else {
        lexicon.with_pronoun_bindings(available_vars)
    };
    parse_sentence_inner(tokens, &effective_lexicon, rules, &KindFilter::Any, &mut var_gen)
}

// --- Span tree helpers for hinted parsing ---

fn build_parent_map(spans: &[Span]) -> Vec<Option<usize>> {
    let n = spans.len();
    let mut parents: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let mut best: Option<usize> = None;
        for j in 0..n {
            if i == j {
                continue;
            }
            if spans[i].start >= spans[j].start && spans[i].end <= spans[j].end {
                match best {
                    None => best = Some(j),
                    Some(b) if spans[j].start >= spans[b].start && spans[j].end <= spans[b].end => {
                        best = Some(j);
                    }
                    _ => {}
                }
            }
        }
        parents[i] = best;
    }
    parents
}

fn make_sub_sentence(
    tokens: &[String],
    spans: &[Span],
    parent_idx: usize,
    parent_map: &[Option<usize>],
) -> InputSentence {
    let parent = &spans[parent_idx];
    let sub_tokens: Vec<String> = tokens[parent.start..parent.end].to_vec();
    let mut sub_spans: Vec<Span> = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        if parent_map[i] == Some(parent_idx) {
            sub_spans.push(Span {
                label: span.label.clone(),
                start: span.start - parent.start,
                end: span.end - parent.start,
            });
        }
    }
    InputSentence { tokens: sub_tokens, spans: sub_spans }
}

pub fn parse_hinted_sentence(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
) -> Vec<Match> {
    let mut var_gen = VarGen::new();

    if sentence.spans.is_empty() {
        return Vec::new();
    }

    let parent_map = build_parent_map(&sentence.spans);

    let top_level: Vec<usize> = (0..sentence.spans.len())
        .filter(|&i| parent_map[i].is_none())
        .collect();

    let mut child_results: Vec<(&str, SemValue, String, Option<String>, Option<SyntaxNode>, (usize, usize), Vec<Constituent>)> = Vec::new();
    for &idx in &top_level {
        let span = &sentence.spans[idx];
        let sub_tokens = &sentence.tokens[span.start..span.end];

        if span.label == "s" {
            let sub_sentence = make_sub_sentence(&sentence.tokens, &sentence.spans, idx, &parent_map);
            let matches = parse_hinted_sentence(&sub_sentence, lexicon, rules);
            if let Some(best) = matches.first() {
                let sv = best.sem_value.clone().unwrap_or_else(|| {
                    SemValue::Prop(crate::core::logic::Expr::Entity("_error".into()))
                });
                child_results.push((
                    &span.label,
                    sv,
                    best.output.clone(),
                    best.syntax.clone(),
                    best.syntax_tree.clone(),
                    (span.start, span.end),
                    best.constituents.clone(),
                ));
            } else {
                return Vec::new();
            }
        } else {
            let filter = sub_filter_for_label(&span.label);
            let matches = parse_sentence_inner(sub_tokens, lexicon, rules, &filter, &mut var_gen);
            if let Some(best) = matches.first() {
                let sv = best.sem_value.clone().unwrap_or_else(|| {
                    match crate::core::semantics::parse(&best.output) {
                        Ok(expr) => SemValue::Prop(expr),
                        Err(_) => SemValue::Prop(crate::core::logic::Expr::Entity("_error".into())),
                    }
                });
                child_results.push((
                    &span.label,
                    sv,
                    best.output.clone(),
                    best.syntax.clone(),
                    best.syntax_tree.clone(),
                    (span.start, span.end),
                    Vec::new(),
                ));
            } else {
                return Vec::new();
            }
        }
    }

    let mut covered: HashSet<usize> = HashSet::new();
    for &idx in &top_level {
        let span = &sentence.spans[idx];
        for i in span.start..span.end {
            covered.insert(i);
        }
    }
    let mut gap_entries: Vec<(String, String, usize)> = Vec::new();
    for (i, token) in sentence.tokens.iter().enumerate() {
        if !covered.contains(&i) {
            if let Some((canonical, _cat, _consumed)) = lexicon.lookup_at(&[token.clone()], 0) {
                let typ = lexicon.get_type(&canonical).unwrap_or_default();
                gap_entries.push((canonical, typ, i));
            }
        }
    }

    let mut results = Vec::new();
    for rule in rules {
        if rule.kind != "s" {
            continue;
        }
        let sem_spec = match &rule.sem {
            Some(s) => s,
            None => continue,
        };

        let mut consumed_children: HashSet<usize> = HashSet::new();
        let mut gap_idx = 0;
        let mut args: Vec<Arg> = Vec::new();
        let mut ok = true;

        for sem_arg in &sem_spec.args {
            match sem_arg {
                SemArg::Slot(name) => {
                    let key = name.strip_prefix('$').unwrap_or(name);
                    if key == "SUB" || key.starts_with("SUB") {
                        if let Some(idx) = parse_sub_index(key) {
                            if idx < child_results.len() {
                                consumed_children.insert(idx);
                                args.push(Arg::Sub(child_results[idx].1.clone()));
                                continue;
                            }
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

        if !ok {
            continue;
        }

        if consumed_children.len() != child_results.len() {
            continue;
        }
        if gap_idx != gap_entries.len() {
            continue;
        }

        match apply_constructor(&sem_spec.constructor, &args, &mut var_gen) {
            Ok(sv) => {
                let output = format!("{}", sv);

                let constituents: Vec<Constituent> = child_results.iter().map(|(label, sv, sem, syn, syn_tree, span, children)| {
                    Constituent {
                        label: label.to_string(),
                        semantics: sem.clone(),
                        free_vars: Vec::new(),
                        span: Some(*span),
                        syntax: syn.clone(),
                        syntax_tree: syn_tree.clone(),
                        sem_value: Some(sv.clone()),
                        children: children.clone(),
                    }
                }).collect();

                let annotations = build_annotations_from_hints(
                    &sentence.tokens,
                    &sentence.spans,
                    &gap_entries,
                    &top_level,
                );

                let constituent_spans: Vec<(usize, usize, String)> = constituents.iter()
                    .filter_map(|c| c.span.map(|(s, e)| (s, e, c.syntax.clone().unwrap_or_default())))
                    .collect();
                let syntax = build_syntax(&sentence.tokens, &constituent_spans, "S");

                let constituent_trees: Vec<(usize, usize, SyntaxNode)> = constituents.iter()
                    .filter_map(|c| c.span.zip(c.syntax_tree.clone()).map(|(s, t)| (s.0, s.1, t)))
                    .collect();
                let syntax_tree = build_syntax_tree(&sentence.tokens, &annotations, &constituent_trees, "S");

                results.push(Match {
                    rule_name: rule.name.clone(),
                    kind: rule.kind.clone(),
                    output,
                    bindings: BTreeMap::new(),
                    token_annotations: annotations,
                    constituents,
                    syntax: Some(syntax),
                    syntax_tree: Some(syntax_tree),
                    semantics_check: None,
                    sem_value: Some(sv),
                });
            }
            Err(_) => {
                continue;
            }
        }
    }

    results
}

fn parse_sub_index(key: &str) -> Option<usize> {
    if key == "SUB" {
        Some(0)
    } else if let Some(rest) = key.strip_prefix("SUB") {
        rest.parse::<usize>().ok()
    } else {
        None
    }
}

fn build_annotations_from_hints(
    tokens: &[String],
    spans: &[Span],
    gap_entries: &[(String, String, usize)],
    top_level: &[usize],
) -> Vec<TokenAnnotation> {
    let mut out: Vec<TokenAnnotation> = tokens.iter()
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched,
            canonical: None,
            typ: None,
            variable: None,
            keyword_class: None,
        })
        .collect();

    for &idx in top_level {
        let span = &spans[idx];
        for i in span.start..span.end {
            if i < out.len() {
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

    for (canonical, typ, pos) in gap_entries {
        if *pos < out.len() {
            let kind = if typ == "e" {
                TokenKind::Entity
            } else if typ.starts_with('{') {
                TokenKind::Predicate
            } else {
                TokenKind::Entity
            };
            out[*pos] = TokenAnnotation {
                kind,
                canonical: Some(canonical.clone()),
                typ: Some(typ.clone()),
                variable: None,
                keyword_class: None,
            };
        }
    }

    out
}

fn parse_sentence_inner(
    tokens: &[String], lexicon: &Lexicon, rules: &[Rule], kind_filter: &KindFilter,
    var_gen: &mut VarGen,
) -> Vec<Match> {
    let mut results = Vec::new();
    for rule in rules {
        match kind_filter {
            KindFilter::Any => {}
            KindFilter::Only(k) if rule.kind != *k => continue,
            KindFilter::Exclude(ks) if ks.contains(&rule.kind) => continue,
            _ => {}
        }
        let bindings = BTreeMap::new();
        let log: Vec<Consumption> = Vec::new();
        let constituents = Vec::new();
        if let Some((b, log, constituents)) = match_pattern(
            &rule.pattern, 0, tokens, 0, bindings, log, lexicon, rules, 0, constituents, kind_filter,
            var_gen,
        ) {
            let annotations = annotate_tokens(tokens, &log);

            let constituent_spans: Vec<(usize, usize, String)> = constituents.iter()
                .filter_map(|c| c.span.map(|(s, e)| (s, e, c.syntax.clone().unwrap_or_default())))
                .collect();
            let syntax = build_syntax(tokens, &constituent_spans, kind_to_syntax_label(&rule.kind));

            let constituent_trees: Vec<(usize, usize, SyntaxNode)> = constituents.iter()
                .filter_map(|c| c.span.zip(c.syntax_tree.clone()).map(|(s, t)| (s.0, s.1, t)))
                .collect();
            let syntax_tree = build_syntax_tree(tokens, &annotations, &constituent_trees, kind_to_syntax_label(&rule.kind));

            let (output, sem_value, semantics_check) = if let Some(ref sem_spec) = rule.sem {
                match resolve_and_construct(sem_spec, &b, &constituents, var_gen) {
                    Ok((sv, out)) => (out, Some(sv), None),
                    Err(_) => {
                        continue;
                    }
                }
            } else {
                let out = apply_template(&rule.template, &b);
                let check = match parse_with_types(&out, lexicon) {
                    Ok(_) => None,
                    Err(e) => Some(e),
                };
                (out, None, check)
            };

            results.push(Match {
                rule_name: rule.name.clone(),
                kind: rule.kind.clone(),
                output,
                bindings: b,
                token_annotations: annotations,
                constituents,
                syntax: Some(syntax),
                syntax_tree: Some(syntax_tree),
                semantics_check,
                sem_value,
            });
        }
    }
    results
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
            "SUB".to_string()
        } else {
            format!("SUB{}", i + 1)
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

fn match_pattern(
    pattern: &[Slot], pi: usize, tokens: &[String], ti: usize,
    bindings: BTreeMap<String, (String, String)>, log: Vec<Consumption>,
    lexicon: &Lexicon, rules: &[Rule], sub_count: usize,
    mut constituents: Vec<Constituent>, kind_filter: &KindFilter,
    var_gen: &mut VarGen,
) -> Option<(BTreeMap<String, (String, String)>, Vec<Consumption>, Vec<Constituent>)> {
    let next_is_literal_or_ignore = pi < pattern.len()
        && matches!(pattern[pi], Slot::Literal(_) | Slot::Ignore);

    let mut ti = ti;
    let mut log = log;
    if !next_is_literal_or_ignore {
        while ti < tokens.len() && is_ignored(&clean_token(&tokens[ti])) {
            log.push(Consumption::Skipped { position: ti });
            ti += 1;
        }
    }

    if pi >= pattern.len() {
        let mut ti = ti;
        let mut log = log;
        while ti < tokens.len() && is_ignored(&clean_token(&tokens[ti])) {
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
            match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen)
        }
        Slot::Literal(lit) => {
            if &token == lit {
                let mut log = log;
                log.push(Consumption::Literal { position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen)
            } else {
                None
            }
        }
        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                let mut log = log;
                log.push(Consumption::Keyword { class: kw.clone(), position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen)
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
                        var_gen,
                    ) {
                        return Some(result);
                    }
                }
            }
            if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter, var_gen);
            }
            None
        }
        Slot::Sub { label, available_vars, delimiter } => {
            let sub_start = ti;
            let mut sub_ti = ti;
            while sub_ti < tokens.len() && is_ignored(&clean_token(&tokens[sub_ti])) {
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
                let sub_filter = sub_filter_for_label(label);
                let effective_lexicon = if available_vars.is_empty() {
                    None
                } else {
                    Some(lexicon.with_pronoun_bindings(available_vars))
                };
                let sub_lexicon = effective_lexicon.as_ref().unwrap_or(lexicon);
                let sub_matches = parse_sentence_inner(sub_tokens, sub_lexicon, rules, &sub_filter, var_gen);
                if let Some(best) = sub_matches.first() {
                    let mut new_bindings = bindings.clone();
                    let mut new_log = log.clone();
                    new_log.push(Consumption::SubClause { start: sub_start, end });
                    let sub_key = if sub_count == 0 {
                        "SUB".to_string()
                    } else {
                        format!("SUB{}", sub_count + 1)
                    };
                    new_bindings.insert(sub_key, (best.output.clone(), label.clone()));
                    constituents.push(Constituent {
                        label: label.clone(),
                        semantics: best.output.clone(),
                        free_vars: available_vars.clone(),
                        span: Some((sub_start, end)),
                        syntax: best.syntax.clone(),
                        syntax_tree: best.syntax_tree.clone(),
                        sem_value: best.sem_value.clone(),
                        children: Vec::new(),
                    });
                    return match_pattern(
                        pattern, pi + 1, tokens, end,
                        new_bindings, new_log, lexicon, rules, sub_count + 1, constituents, kind_filter,
                        var_gen,
                    );
                } else {
                    None
                }
            } else {
                let sub_filter = sub_filter_for_label(label);
                let effective_lexicon = if available_vars.is_empty() {
                    None
                } else {
                    Some(lexicon.with_pronoun_bindings(available_vars))
                };
                let sub_lexicon = effective_lexicon.as_ref().unwrap_or(lexicon);
                let max_end = tokens.len();
                for end in (sub_ti + 1)..=max_end {
                    let sub_tokens = &tokens[sub_ti..end];
                    let sub_matches = parse_sentence_inner(sub_tokens, sub_lexicon, rules, &sub_filter, var_gen);
                    if let Some(best) = sub_matches.first() {
                        let mut trial_constituents = constituents.clone();
                        let mut new_bindings = bindings.clone();
                        let mut new_log = log.clone();
                        new_log.push(Consumption::SubClause { start: sub_start, end });
                        let sub_key = if sub_count == 0 {
                            "SUB".to_string()
                        } else {
                            format!("SUB{}", sub_count + 1)
                        };
                        new_bindings.insert(sub_key, (best.output.clone(), label.clone()));
                        trial_constituents.push(Constituent {
                            label: label.clone(),
                            semantics: best.output.clone(),
                            free_vars: available_vars.clone(),
                            span: Some((sub_start, end)),
                            syntax: best.syntax.clone(),
                            syntax_tree: best.syntax_tree.clone(),
                            sem_value: best.sem_value.clone(),
                            children: Vec::new(),
                        });
                        if let Some(result) = match_pattern(
                            pattern, pi + 1, tokens, end,
                            new_bindings, new_log, lexicon, rules, sub_count + 1, trial_constituents, kind_filter,
                            var_gen,
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

fn annotate_tokens(tokens: &[String], log: &[Consumption]) -> Vec<TokenAnnotation> {
    let mut out: Vec<TokenAnnotation> = (0..tokens.len())
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for c in log {
        match c {
            Consumption::Var { variable, canonical, typ, start, end } => {
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
                    if i < out.len() && matches!(out[i].kind, TokenKind::Unmatched | TokenKind::Ignored) {
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
            Consumption::Ignore { position } | Consumption::Skipped { position } => {
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