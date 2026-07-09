use std::collections::{BTreeMap, HashMap, HashSet};

use crate::core::construct::{Arg, apply_constructor};
use crate::core::fixture::{InputSentence, Span};
use crate::core::grammar::{is_punctuation, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::sem_dsl::SemArg;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics: Option<String>,
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

// --- Internal types ---

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

// --- Span cache types ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpanKey {
    start: usize,
    end: usize,
    label: String,
}

struct SpanResult {
    sem_value: SemValue,
    output: String,
    rule_name: String,
    kind: String,
    bindings: BTreeMap<String, (String, String)>,
    token_annotations: Vec<TokenAnnotation>,
    constituents: Vec<Constituent>,
}

// --- Helper functions ---

fn kind_to_syntax_label(kind: &str) -> String {
    match kind {
        "dp" => "DP".to_string(),
        "s" => "S".to_string(),
        "s\\agent" => "S\\agent".to_string(),
        "s\\patient" => "S\\patient".to_string(),
        "s\\theme" => "S\\theme".to_string(),
        _ => kind.to_uppercase(),
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
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Entity => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Predicate => {
                let label = ann.canonical.as_deref().unwrap_or(&cleaned);
                SyntaxNode { label: label.to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::Literal => {
                SyntaxNode { label: "LIT".to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            TokenKind::SubClause => {
                SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
            _ => {
                SyntaxNode { label: "UNK".to_string(), children: vec![], terminal: Some(cleaned), semantics: None }
            }
        };
        children.push(node);
    }
    SyntaxNode { label: top_label.to_string(), children, terminal: None, semantics: None }
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

fn parse_sub_index(key: &str) -> Option<usize> {
    if key == "SUB" {
        Some(0)
    } else if let Some(rest) = key.strip_prefix("SUB") {
        rest.parse::<usize>().ok()
    } else {
        None
    }
}

fn offset_constituents(constituents: &[Constituent], offset: usize) -> Vec<Constituent> {
    constituents.iter().map(|c| {
        let mut c = c.clone();
        c.span = c.span.map(|(s, e)| (s + offset, e + offset));
        c.children = offset_constituents(&c.children, offset);
        c
    }).collect()
}

/// Build a SyntaxNode tree from a Constituent's children when syntax_tree is absent.
/// Recursively walks nested children to produce proper SVG subtrees.
fn build_syntax_tree_from_constituent(
    c: &Constituent,
    all_tokens: &[String],
    global_offset: usize,
) -> SyntaxNode {
    let (s, e) = match c.span {
        Some((s, e)) => (s, e),
        None => {
            return SyntaxNode {
                label: kind_to_syntax_label(&c.label),
                children: vec![],
                terminal: None,
                semantics: Some(c.semantics.clone()),
            };
        }
    };
    let local_s = s.saturating_sub(global_offset);
    let local_e = e.saturating_sub(global_offset);
    if local_s >= all_tokens.len() {
        return SyntaxNode {
            label: kind_to_syntax_label(&c.label),
            children: vec![],
            terminal: None,
            semantics: Some(c.semantics.clone()),
        };
    }
    let local_e = local_e.min(all_tokens.len());

    let mut children = Vec::new();
    let mut ci = 0;
    let mut skip_until = local_s;
    for i in local_s..local_e {
        if i < skip_until {
            continue;
        }
        let cleaned = clean_token(&all_tokens[i]);
        if is_punctuation(&cleaned) {
            continue;
        }
        if ci < c.children.len() {
            let child = &c.children[ci];
            if let Some((child_s, _)) = child.span {
                if child_s == i + global_offset {
                    children.push(build_syntax_tree_from_constituent(child, all_tokens, global_offset));
                    if let Some((_, child_e)) = child.span {
                        skip_until = child_e.saturating_sub(global_offset);
                    }
                    ci += 1;
                    continue;
                }
            }
        }
        children.push(SyntaxNode {
            label: cleaned.clone(),
            children: vec![],
            terminal: Some(cleaned),
            semantics: None,
        });
    }
    SyntaxNode {
        label: kind_to_syntax_label(&c.label),
        children,
        terminal: None,
        semantics: Some(c.semantics.clone()),
    }
}

/// Build a SyntaxNode tree from a SpanResult.  `tokens` must be the exact slice
/// the SpanResult was parsed on (local to the span).  `offset` is the global
/// index of `tokens[0]` so that constituent spans (which are global) can be
/// mapped back into the local slice.
fn build_syntax_tree_for_result(
    result: &SpanResult,
    tokens: &[String],
    offset: usize,
) -> SyntaxNode {
    let mut annotations: Vec<TokenAnnotation> = tokens.iter()
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for (i, ann) in result.token_annotations.iter().enumerate() {
        if i < annotations.len() {
            annotations[i] = ann.clone();
        }
    }

    let constituent_trees: Vec<(usize, usize, SyntaxNode)> = result.constituents.iter()
        .filter_map(|c| {
            let (s, e) = c.span?;
            let local_s = s.saturating_sub(offset);
            let local_e = e.saturating_sub(offset);
            if local_s >= tokens.len() {
                return None;
            }
            let local_e = local_e.min(tokens.len());

            let tree = c.syntax_tree.clone().unwrap_or_else(|| {
                if c.children.is_empty() {
                    SyntaxNode {
                        label: kind_to_syntax_label(&c.label),
                        children: tokens[local_s..local_e].iter().filter_map(|t| {
                            let cleaned = clean_token(t);
                            if is_punctuation(&cleaned) {
                                None
                            } else {
                                Some(SyntaxNode { label: cleaned.clone(), children: vec![], terminal: Some(cleaned), semantics: None })
                            }
                        }).collect(),
                        terminal: None,
                        semantics: None,
                    }
                } else {
                    build_syntax_tree_from_constituent(c, tokens, offset)
                }
            });
            Some((local_s, local_e, tree))
        })
        .collect();

    let mut tree = build_syntax_tree(tokens, &annotations, &constituent_trees, &kind_to_syntax_label(&result.kind));
    tree.semantics = Some(result.output.clone());
    tree
}

// --- Public API ---

pub fn parse_hinted_sentence(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
) -> Result<Vec<Match>, String> {
    parse_bottom_up(sentence, lexicon, rules, None)
}

pub fn parse_hinted_sentence_traced(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    trace: &mut DebugTrace,
) -> Result<Vec<Match>, String> {
    parse_bottom_up(sentence, lexicon, rules, Some(trace))
}

// --- Bottom-up core ---

fn parse_bottom_up(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    mut trace: Option<&mut DebugTrace>,
) -> Result<Vec<Match>, String> {
    if sentence.spans.is_empty() {
        return Err("no hint spans provided".into());
    }

    let mut var_gen = VarGen::new();

    // 1. Build span set and sort: smallest first, tie-break by leftmost start
    let mut indexed: Vec<(usize, &Span)> = sentence.spans.iter().enumerate().collect();
    indexed.sort_by_key(|(_, span)| (span.end - span.start, span.start));

    // 2. Determine parentage: a span's parent is the smallest span that contains it
    let mut parent_map: HashMap<SpanKey, Option<SpanKey>> = HashMap::new();
    for (i, span) in sentence.spans.iter().enumerate() {
        let key = SpanKey { start: span.start, end: span.end, label: span.label.clone() };
        let mut best: Option<SpanKey> = None;
        for (j, other) in sentence.spans.iter().enumerate() {
            if i == j { continue; }
            if span.start >= other.start && span.end <= other.end {
                let other_key = SpanKey { start: other.start, end: other.end, label: other.label.clone() };
                match &best {
                    None => best = Some(other_key),
                    Some(b) if other.start >= b.start && other.end <= b.end => {
                        best = Some(other_key);
                    }
                    _ => {}
                }
            }
        }
        parent_map.insert(key, best);
    }

    let top_level_keys: Vec<SpanKey> = sentence.spans.iter()
        .filter_map(|s| {
            let key = SpanKey { start: s.start, end: s.end, label: s.label.clone() };
            if parent_map.get(&key) == Some(&None) { Some(key) } else { None }
        })
        .collect();

    if let Some(t) = &mut trace {
        t.push(&format!("tokens: {:?}", sentence.tokens));
        t.push(&format!("{} spans (sorted bottom-up):", indexed.len()));
        for (_i, span) in &indexed {
            let parent_str = match parent_map.get(&SpanKey { start: span.start, end: span.end, label: span.label.clone() }) {
                None => "NONE".to_string(),
                Some(None) => "TOP".to_string(),
                Some(Some(p)) => format!("[{}] {}..{}", p.label, p.start, p.end),
            };
            t.push(&format!("  [{}] {}..{} size={} parent={}",
                span.label, span.start, span.end, span.end - span.start, parent_str));
        }
        t.push("");
    }

    // 3. Parse each span bottom-up
    let mut cache: HashMap<SpanKey, SpanResult> = HashMap::new();

    for (_orig_idx, span) in &indexed {
        let key = SpanKey { start: span.start, end: span.end, label: span.label.clone() };
        let span_tokens = &sentence.tokens[span.start..span.end];

        // Find direct children: spans strictly inside this one, whose parent is this one
        let direct_children: Vec<SpanKey> = sentence.spans.iter()
            .filter_map(|s| {
                let child_key = SpanKey { start: s.start, end: s.end, label: s.label.clone() };
                if child_key == key { return None; }
                if s.start < span.start || s.end > span.end { return None; }
                match parent_map.get(&child_key) {
                    Some(Some(parent)) if *parent == key => Some(child_key),
                    _ => None,
                }
            })
            .collect();

        if let Some(t) = &mut trace {
            t.enter(&format!("parse [{}] {}..{} (size={}, children={})",
                span.label, span.start, span.end, span.end - span.start, direct_children.len()));
            t.push(&format!("tokens: {:?}", span_tokens));
        }

        let result = match_span(
            span_tokens,
            span.start,
            span,
            &direct_children,
            lexicon,
            rules,
            &mut var_gen,
            &cache,
            trace.as_deref_mut(),
        )?;

        if let Some(t) = &mut trace {
            t.push(&format!("→ {} [{}]", result.output, result.rule_name));
            t.leave();
        }

        cache.insert(key, result);
    }

    // 4. Build output from top-level spans
    if top_level_keys.len() == 1 {
        let key = &top_level_keys[0];
        let result = cache.get(key).unwrap();
        let span = sentence.spans.iter().find(|s| {
            SpanKey { start: s.start, end: s.end, label: s.label.clone() } == *key
        }).unwrap();
        let m = span_result_to_match(result, span, &sentence.tokens);
        Ok(vec![m])
    } else {
        // Multiple top-level spans: try to assemble into a sentence
        let mut sorted_top: Vec<&SpanKey> = top_level_keys.iter().collect();
        sorted_top.sort_by_key(|k| k.start);

        if let Some(t) = &mut trace {
            t.enter(&format!("assemble {} top-level spans into sentence", sorted_top.len()));
        }

        let assembled = try_assemble_top_level(
            &sorted_top,
            &cache,
            &sentence.tokens,
            lexicon,
            rules,
            &mut var_gen,
            trace.as_deref_mut(),
        );

        if let Some(m) = assembled {
            if let Some(t) = &mut trace {
                t.push(&format!("→ {} [{}]", m.output, m.rule_name));
                t.leave();
            }
            Ok(vec![m])
        } else {
            if let Some(t) = &mut trace {
                t.push("FAILED: no s-rule can assemble top-level spans");
                t.leave();
            }
            Err(format!(
                "cannot assemble {} top-level spans into a sentence: {}",
                sorted_top.len(),
                sorted_top.iter().map(|k| format!("[{}] {}..{}", k.label, k.start, k.end)).collect::<Vec<_>>().join(", ")
            ))
        }
    }
}

fn match_span(
    span_tokens: &[String],
    global_start: usize,
    span: &Span,
    direct_children: &[SpanKey],
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    cache: &HashMap<SpanKey, SpanResult>,
    mut trace: Option<&mut DebugTrace>,
) -> Result<SpanResult, String> {
    if direct_children.is_empty() {
        try_pattern_match(span_tokens, global_start, span, lexicon, rules, var_gen, cache, trace.as_deref_mut())
            .ok_or_else(|| format!(
                "span [{}] tokens[{}..{}] ({:?}): no matching rule",
                span.label, span.start, span.end, span_tokens
            ))
    } else {
        if let Some(result) = try_pattern_match(span_tokens, global_start, span, lexicon, rules, var_gen, cache, trace.as_deref_mut()) {
            return Ok(result);
        }
        if let Some(t) = &mut trace {
            t.push("pattern match failed, trying assembly");
        }
        try_assembly(span_tokens, global_start, span, direct_children, lexicon, rules, var_gen, cache, trace.as_deref_mut())
            .ok_or_else(|| format!(
                "span [{}] tokens[{}..{}] ({:?}): no rule matched (tried pattern + assembly)",
                span.label, span.start, span.end, span_tokens
            ))
    }
}

// --- Pattern matching (cache-aware) ---

fn try_pattern_match(
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
                        span: Some((global_offset + sub_start, global_offset + end)),
                        syntax: None,
                        syntax_tree,
                        sem_value: Some(cached.sem_value.clone()),
                        children: cached.constituents.clone(),
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
                            span: Some((global_offset + sub_start, global_offset + end)),
                            syntax: None,
                            syntax_tree,
                            sem_value: Some(cached.sem_value.clone()),
                            children: cached.constituents.clone(),
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

fn try_assembly(
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

fn try_assemble_top_level(
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

// --- Match building ---

fn span_result_to_match(result: &SpanResult, span: &Span, all_tokens: &[String]) -> Match {
    let mut full_annotations: Vec<TokenAnnotation> = all_tokens.iter()
        .map(|_| TokenAnnotation {
            kind: TokenKind::Unmatched, canonical: None, typ: None, variable: None, keyword_class: None,
        })
        .collect();
    for (i, ann) in result.token_annotations.iter().enumerate() {
        let global_i = span.start + i;
        if global_i < full_annotations.len() {
            full_annotations[global_i] = ann.clone();
        }
    }

    let constituent_spans: Vec<(usize, usize, String)> = result.constituents.iter()
        .filter_map(|c| c.span.map(|(s, e)| (s, e, format!("[{}]", c.label))))
        .collect();
    let syntax = build_syntax(all_tokens, &constituent_spans, &kind_to_syntax_label(&result.kind));

    let constituent_trees: Vec<(usize, usize, SyntaxNode)> = result.constituents.iter()
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
    let mut syntax_tree = build_syntax_tree(all_tokens, &full_annotations, &constituent_trees, &kind_to_syntax_label(&result.kind));
    syntax_tree.semantics = Some(result.output.clone());

    Match {
        rule_name: result.rule_name.clone(), kind: result.kind.clone(), output: result.output.clone(),
        bindings: result.bindings.clone(), token_annotations: full_annotations, constituents: result.constituents.clone(),
        syntax: Some(syntax), syntax_tree: Some(syntax_tree),
        semantics_check: None, sem_value: Some(result.sem_value.clone()),
    }
}

// --- Annotation helpers ---

fn annotate_tokens(tokens: &[String], log: &[Consumption]) -> Vec<TokenAnnotation> {
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