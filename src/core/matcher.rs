use std::collections::{BTreeMap, HashSet};

use crate::core::construct::{Arg, apply_constructor};
use crate::core::fixture::{InputSentence, Span};
use crate::core::grammar::{is_punctuation, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
use crate::core::semantics::parse_with_types;
use crate::core::template::apply_template;
use crate::core::value::{SemValue, VarGen};

// --- Debug trace ---

pub struct DebugTrace {
    lines: Vec<String>,
    indent: usize,
}

impl DebugTrace {
    pub fn new() -> Self {
        Self { lines: Vec::new(), indent: 0 }
    }

    pub fn push(&mut self, msg: &str) {
        self.lines.push(format!("{}{}", "  ".repeat(self.indent), msg));
    }

    pub fn enter(&mut self, msg: &str) {
        self.push(msg);
        self.indent += 1;
    }

    pub fn leave(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }

    pub fn emit(&self) {
        for line in &self.lines {
            eprintln!("{}", line);
        }
    }
}

fn filter_label(f: &KindFilter) -> String {
    match f {
        KindFilter::Any => "any".to_string(),
        KindFilter::Only(k) => format!("only({})", k),
    }
}

// --- Public types ---

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
        if is_punctuation(&cleaned) {
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
        if is_punctuation(&cleaned) {
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

// --- Public API ---

pub fn parse_hinted_sentence(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
) -> Vec<Match> {
    parse_hinted_inner(sentence, lexicon, rules, None)
}

pub fn parse_hinted_sentence_traced(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    trace: &mut DebugTrace,
) -> Vec<Match> {
    parse_hinted_inner(sentence, lexicon, rules, Some(trace))
}

// --- Span tree helpers ---

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

// --- Bottom-Up Hinted Parsing ---

fn parse_hinted_inner(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    mut trace: Option<&mut DebugTrace>,
) -> Vec<Match> {
    let n = sentence.spans.len();
    if n == 0 {
        return Vec::new();
    }

    let parent_map = build_parent_map(&sentence.spans);
    let top_level: Vec<usize> = (0..n).filter(|&i| parent_map[i].is_none()).collect();

    // Sort spans by size (end - start) to parse smallest phrases first.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| sentence.spans[i].end - sentence.spans[i].start);

    let mut parsed: Vec<Option<SemValue>> = vec![None; n];
    let mut parsed_matches: Vec<Option<Match>> = vec![None; n];

    let mut var_gen = VarGen::new();

    for &i in &order {
        let span = &sentence.spans[i];
        let mut children: Vec<usize> = (0..n).filter(|&j| parent_map[j] == Some(i)).collect();
        children.sort_by_key(|&j| sentence.spans[j].start);

        if let Some(t) = &mut trace {
            t.enter(&format!("parsing span [{}] {}..{}", span.label, span.start, span.end));
            t.push(&format!("tokens: {:?}", &sentence.tokens[span.start..span.end]));
            t.push(&format!("children: {:?}", children.iter().map(|&c| &sentence.spans[c]).collect::<Vec<_>>()));
        }

        let matches = match_span(i, &children, sentence, lexicon, rules, &parsed, &mut var_gen);

        if let Some(t) = &mut trace {
            if matches.is_empty() {
                t.push("FAIL: no match");
            } else {
                t.push(&format!("→ {}", matches[0].output));
            }
            t.leave();
        }

        if let Some(best) = matches.first() {
            parsed[i] = best.sem_value.clone();
            parsed_matches[i] = Some(best);
        }
    }

    top_level.iter().filter_map(|&i| parsed_matches[i].clone()).collect()
}

fn match_span(
    span_idx: usize,
    children: &[usize],
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    parsed: &[Option<SemValue>],
    var_gen: &mut VarGen,
) -> Vec<Match> {
    let span = &sentence.spans[span_idx];
    let tokens = &sentence.tokens;
    let kind_filter = sub_filter_for_label(&span.label);

    let mut results = Vec::new();

    for rule in rules {
        match &kind_filter {
            KindFilter::Any => {}
            KindFilter::Only(k) if rule.kind != *k => continue,
            KindFilter::Only(_) => {}
        }

        let bindings = BTreeMap::new();
        let log = Vec::new();
        let constituents = Vec::new();

        if let Some((b, log, consts)) = match_pattern(
            &rule.pattern, 0, tokens, span.start, span.end,
            0, children, &sentence.spans, parsed, bindings, log, constituents, lexicon, var_gen
        ) {
            let annotations = annotate_tokens(tokens, &log);
            let rel_annotations = &annotations[span.start..span.end];
            
            let rel_consts: Vec<(usize, usize, String)> = consts.iter()
                .filter_map(|c| c.span.map(|(s, e)| (s - span.start, e - span.start, c.syntax.clone().unwrap_or_default())))
                .collect();
            let syntax = build_syntax(&tokens[span.start..span.end], &rel_consts, kind_to_syntax_label(&rule.kind));

            let rel_const_trees: Vec<(usize, usize, SyntaxNode)> = consts.iter()
                .filter_map(|c| c.span.zip(c.syntax_tree.clone()).map(|(s, t)| (s.0 - span.start, s.1 - span.start, t)))
                .collect();
            let syntax_tree = build_syntax_tree(&tokens[span.start..span.end], rel_annotations, &rel_const_trees, kind_to_syntax_label(&rule.kind));

            let (output, sem_value, semantics_check) = if let Some(ref sem_spec) = rule.sem {
                match resolve_and_construct(sem_spec, &b, &consts, var_gen) {
                    Ok((sv, out)) => (out, Some(sv), None),
                    Err(_e) => continue,
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
                constituents: consts,
                syntax: Some(syntax),
                syntax_tree: Some(syntax_tree),
                semantics_check,
                sem_value,
            });
        }
    }

    results
}

fn match_pattern(
    pattern: &[Slot], 
    pi: usize, 
    tokens: &[String], 
    pos: usize, 
    end: usize, 
    child_idx: usize, 
    children: &[usize], 
    spans: &[Span], 
    parsed: &[Option<SemValue>], 
    bindings: BTreeMap<String, (String, String)>, 
    mut log: Vec<Consumption>,
    constituents: Vec<Constituent>, 
    lexicon: &Lexicon, 
    var_gen: &mut VarGen,
) -> Option<(BTreeMap<String, (String, String)>, Vec<Consumption>, Vec<Constituent>)> {
    
    // Skip punctuation
    let mut pos = pos;
    while pos < end && is_punctuation(&clean_token(&tokens[pos])) {
        log.push(Consumption::Skipped { position: pos });
        pos += 1;
    }

    if pi >= pattern.len() {
        if pos >= end {
            return Some((bindings, log, constituents));
        }
        return None;
    }

    let at_child_start = child_idx < children.len() && spans[children[child_idx]].start == pos;
    
    let slot = &pattern[pi];
    match slot {
        Slot::Sub { label, .. } => {
            if !at_child_start {
                return None;
            }
            let child_span_idx = children[child_idx];
            let child_span = &spans[child_span_idx];
            let child_sem = parsed[child_span_idx].clone()?;
            
            let sub_key = format!("SUB{}", constituents.len() + 1);
            let mut new_bindings = bindings.clone();
            new_bindings.insert(sub_key, (format!("{}", child_sem), child_span.label.clone()));
            
            // Keep legacy $SUB alias for the first sub-clause
            if constituents.is_empty() {
                new_bindings.insert("SUB".to_string(), (format!("{}", child_sem), child_span.label.clone()));
            }

            log.push(Consumption::SubClause { start: child_span.start, end: child_span.end });
            
            let mut new_consts = constituents.clone();
            new_consts.push(Constituent {
                label: child_span.label.clone(),
                semantics: format!("{}", child_sem),
                free_vars: Vec::new(),
                span: Some((child_span.start, child_span.end)),
                syntax: None,
                syntax_tree: None,
                sem_value: Some(child_sem.clone()),
                children: Vec::new(),
            });

            match_pattern(
                pattern, pi + 1, tokens, child_span.end, end, 
                child_idx + 1, children, spans, parsed, new_bindings, log, new_consts, lexicon, var_gen
            )
        }
        Slot::Ignore => {
            if at_child_start || pos >= end {
                return None;
            }
            log.push(Consumption::Ignore { position: pos });
            match_pattern(pattern, pi + 1, tokens, pos + 1, end, child_idx, children, spans, parsed, bindings, log, constituents, lexicon, var_gen)
        }
        Slot::Literal(lit) => {
            if at_child_start || pos >= end {
                return None;
            }
            let token = clean_token(&tokens[pos]);
            if &token == lit {
                log.push(Consumption::Literal { position: pos });
                match_pattern(pattern, pi + 1, tokens, pos + 1, end, child_idx, children, spans, parsed, bindings, log, constituents, lexicon, var_gen)
            } else {
                None
            }
        }
        Slot::Keyword(kw) => {
            if at_child_start || pos >= end {
                return None;
            }
            let token = clean_token(&tokens[pos]);
            if matches_keyword(&token, kw) {
                log.push(Consumption::Keyword { class: kw.clone(), position: pos });
                match_pattern(pattern, pi + 1, tokens, pos + 1, end, child_idx, children, spans, parsed, bindings, log, constituents, lexicon, var_gen)
            } else {
                None
            }
        }
        Slot::Var { name, type_constraint } => {
            if at_child_start || pos >= end {
                return None;
            }
            
            let next_child_start = if child_idx < children.len() { spans[children[child_idx]].start } else { end };
            
            if let Some((canonical, _cat, consumed)) = lexicon.lookup_at(tokens, pos) {
                if pos + consumed > next_child_start {
                    return None; // Overlaps next child
                }
                let actual_type = lexicon.get_type(&canonical).unwrap_or_default();
                let type_ok = match type_constraint {
                    Some(t) => &actual_type == t,
                    None => true,
                };
                if type_ok {
                    let mut new_bindings = bindings.clone();
                    new_bindings.insert(name.clone(), (canonical.clone(), actual_type.clone()));
                    let mut new_log = log.clone();
                    new_log.push(Consumption::Var {
                        variable: name.clone(), canonical: canonical.clone(), typ: actual_type.clone(), 
                        start: pos, end: pos + consumed,
                    });
                    return match_pattern(
                        pattern, pi + 1, tokens, pos + consumed, end, child_idx, children, spans, parsed, 
                        new_bindings, new_log, constituents, lexicon, var_gen
                    );
                }
            }
            None
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

fn resolve_and_construct(
    sem_spec: &crate::core::sem_dsl::SemSpec,
    bindings: &BTreeMap<String, (String, String)>,
    constituents: &[Constituent],
    var_gen: &mut VarGen,
) -> Result<(SemValue, String), String> {
    let mut sub_values: BTreeMap<String, SemValue> = BTreeMap::new();
    for (i, constituent) in constituents.iter().enumerate() {
        if let Some(ref sv) = constituent.sem_value {
            let key = format!("SUB{}", i + 1);
            sub_values.insert(key, sv.clone());
            if i == 0 {
                sub_values.insert("SUB".to_string(), sv.clone());
            }
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