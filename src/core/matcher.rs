mod types;
mod tree;
mod engine;

pub use types::*;
pub use tree::build_syntax_tree_for_result;

use std::collections::HashMap;

use crate::core::fixture::{InputSentence, Span};
use crate::core::grammar::Rule;
use crate::core::lexicon::Lexicon;
use crate::core::value::VarGen;

use types::{DebugTrace, SpanKey, SpanResult, Match};
use tree::span_result_to_match;
use engine::{try_pattern_match, try_assembly, try_assemble_top_level};

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

    let mut indexed: Vec<(usize, &Span)> = sentence.spans.iter().enumerate().collect();
    indexed.sort_by_key(|(_, span)| (span.end - span.start, span.start));

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

    let mut cache: HashMap<SpanKey, SpanResult> = HashMap::new();

    for (_orig_idx, span) in &indexed {
        let key = SpanKey { start: span.start, end: span.end, label: span.label.clone() };
        let span_tokens = &sentence.tokens[span.start..span.end];

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

    if top_level_keys.len() == 1 {
        let key = &top_level_keys[0];
        let result = cache.get(key).unwrap();
        let span = sentence.spans.iter().find(|s| {
            SpanKey { start: s.start, end: s.end, label: s.label.clone() } == *key
        }).unwrap();
        let m = span_result_to_match(result, span, &sentence.tokens);
        Ok(vec![m])
    } else {
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