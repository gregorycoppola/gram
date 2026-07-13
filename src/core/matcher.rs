mod engine;
mod tree;
mod types;

pub use tree::build_syntax_tree_for_result;
pub use types::{Constituent, DebugTrace, Match, SyntaxNode, TokenAnnotation, TokenKind};

use std::collections::{HashMap, HashSet};

use crate::core::fixture::{InputSentence, Span};
use crate::core::grammar::Rule;
use crate::core::lexicon::Lexicon;
use crate::core::value::VarGen;

use engine::try_pattern_match;
use tree::span_result_to_match;
use types::{SpanKey, SpanResult};

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

// --- Span-tree validation ---

fn span_key(span: &Span) -> SpanKey {
    SpanKey {
        start: span.start,
        end: span.end,
        label: span.label.clone(),
    }
}

fn strictly_contains(outer: &Span, inner: &Span) -> bool {
    outer.start <= inner.start
        && inner.end <= outer.end
        && (outer.start < inner.start || inner.end < outer.end)
}

fn spans_cross(left: &Span, right: &Span) -> bool {
    (left.start < right.start && right.start < left.end && left.end < right.end)
        || (right.start < left.start && left.start < right.end && right.end < left.end)
}

fn validate_span_tree(
    sentence: &InputSentence,
) -> Result<HashMap<SpanKey, Option<SpanKey>>, String> {
    if sentence.tokens.is_empty() {
        return Err("hinted sentence has no tokens".into());
    }

    if sentence.spans.is_empty() {
        return Err("no hint spans provided".into());
    }

    let token_count = sentence.tokens.len();
    let mut intervals = HashSet::new();

    for span in &sentence.spans {
        if span.start >= span.end {
            return Err(format!(
                "invalid span [{}] {}..{}: start must be before end",
                span.label, span.start, span.end
            ));
        }

        if span.end > token_count {
            return Err(format!(
                "invalid span [{}] {}..{}: sentence has {} tokens",
                span.label, span.start, span.end, token_count
            ));
        }

        if !intervals.insert((span.start, span.end)) {
            return Err(format!(
                "duplicate span interval {}..{}",
                span.start, span.end
            ));
        }
    }

    for (index, left) in sentence.spans.iter().enumerate() {
        for right in sentence.spans.iter().skip(index + 1) {
            if spans_cross(left, right) {
                return Err(format!(
                    "crossing spans: [{}] {}..{} and [{}] {}..{}",
                    left.label, left.start, left.end, right.label, right.start, right.end
                ));
            }
        }
    }

    let roots = sentence
        .spans
        .iter()
        .filter(|span| span.start == 0 && span.end == token_count)
        .collect::<Vec<_>>();

    if roots.len() != 1 {
        return Err(format!(
            "expected exactly one full-sentence root span 0..{}, found {}",
            token_count,
            roots.len()
        ));
    }

    let root_key = span_key(roots[0]);
    let mut parent_map = HashMap::new();

    for span in &sentence.spans {
        let key = span_key(span);

        if key == root_key {
            parent_map.insert(key, None);
            continue;
        }

        let parent = sentence
            .spans
            .iter()
            .filter(|candidate| strictly_contains(candidate, span))
            .min_by_key(|candidate| candidate.end - candidate.start)
            .map(span_key)
            .ok_or_else(|| {
                format!(
                    "span [{}] {}..{} is not reachable from the root",
                    span.label, span.start, span.end
                )
            })?;

        parent_map.insert(key, Some(parent));
    }

    for span in &sentence.spans {
        let start_key = span_key(span);
        let mut current = start_key.clone();
        let mut visited = HashSet::new();

        loop {
            if current == root_key {
                break;
            }

            if !visited.insert(current.clone()) {
                return Err(format!(
                    "cycle detected while validating span [{}] {}..{}",
                    span.label, span.start, span.end
                ));
            }

            current = parent_map
                .get(&current)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "span [{}] {}..{} is not reachable from the root",
                        span.label, span.start, span.end
                    )
                })?;
        }
    }

    Ok(parent_map)
}

// --- Bottom-up core ---

fn parse_bottom_up(
    sentence: &InputSentence,
    lexicon: &Lexicon,
    rules: &[Rule],
    mut trace: Option<&mut DebugTrace>,
) -> Result<Vec<Match>, String> {
    let parent_map = validate_span_tree(sentence)?;

    let mut var_gen = VarGen::new();

    let mut indexed: Vec<(usize, &Span)> = sentence.spans.iter().enumerate().collect();
    indexed.sort_by_key(|(_, span)| (span.end - span.start, span.start));

    let top_level_keys: Vec<SpanKey> = sentence
        .spans
        .iter()
        .filter_map(|s| {
            let key = SpanKey {
                start: s.start,
                end: s.end,
                label: s.label.clone(),
            };
            if parent_map.get(&key) == Some(&None) {
                Some(key)
            } else {
                None
            }
        })
        .collect();

    if let Some(t) = &mut trace {
        t.push(&format!("tokens: {:?}", sentence.tokens));
        t.push(&format!("{} spans (sorted bottom-up):", indexed.len()));
        for (_i, span) in &indexed {
            let parent_str = match parent_map.get(&SpanKey {
                start: span.start,
                end: span.end,
                label: span.label.clone(),
            }) {
                None => "NONE".to_string(),
                Some(None) => "TOP".to_string(),
                Some(Some(p)) => format!("[{}] {}..{}", p.label, p.start, p.end),
            };
            t.push(&format!(
                "  [{}] {}..{} size={} parent={}",
                span.label,
                span.start,
                span.end,
                span.end - span.start,
                parent_str
            ));
        }
        t.push("");
    }

    let mut cache: HashMap<SpanKey, SpanResult> = HashMap::new();

    for (_orig_idx, span) in &indexed {
        let key = SpanKey {
            start: span.start,
            end: span.end,
            label: span.label.clone(),
        };
        let span_tokens = &sentence.tokens[span.start..span.end];

        let direct_children: Vec<SpanKey> = sentence
            .spans
            .iter()
            .filter_map(|s| {
                let child_key = SpanKey {
                    start: s.start,
                    end: s.end,
                    label: s.label.clone(),
                };
                if child_key == key {
                    return None;
                }
                if s.start < span.start || s.end > span.end {
                    return None;
                }
                match parent_map.get(&child_key) {
                    Some(Some(parent)) if *parent == key => Some(child_key),
                    _ => None,
                }
            })
            .collect();

        if let Some(t) = &mut trace {
            t.enter(&format!(
                "parse [{}] {}..{} (size={}, children={})",
                span.label,
                span.start,
                span.end,
                span.end - span.start,
                direct_children.len()
            ));
            t.push(&format!("tokens: {:?}", span_tokens));
        }

        let result = match_span(
            span_tokens,
            span.start,
            span,
            lexicon,
            rules,
            &mut var_gen,
            &cache,
            trace.as_deref_mut(),
        )?;

        if let Some(t) = &mut trace {
            t.push(&format!(
                "→ {} [{}: {}]",
                result.output, result.rule_name, result.pattern
            ));
            t.leave();
        }

        cache.insert(key, result);
    }

    if top_level_keys.len() == 1 {
        let key = &top_level_keys[0];
        let result = cache.get(key).unwrap();
        let span = sentence
            .spans
            .iter()
            .find(|s| {
                SpanKey {
                    start: s.start,
                    end: s.end,
                    label: s.label.clone(),
                } == *key
            })
            .unwrap();
        let m = span_result_to_match(result, span, &sentence.tokens);
        Ok(vec![m])
    } else {
        return Err(format!(
            "expected exactly one top-level span, found {}: {}",
            top_level_keys.len(),
            top_level_keys
                .iter()
                .map(|k| format!("[{}] {}..{}", k.label, k.start, k.end))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn match_span(
    span_tokens: &[String],
    global_start: usize,
    span: &Span,
    lexicon: &Lexicon,
    rules: &[Rule],
    var_gen: &mut VarGen,
    cache: &HashMap<SpanKey, SpanResult>,
    trace: Option<&mut DebugTrace>,
) -> Result<SpanResult, String> {
    try_pattern_match(
        span_tokens,
        global_start,
        span,
        lexicon,
        rules,
        var_gen,
        cache,
        trace,
    )
    .ok_or_else(|| {
        format!(
            "span [{}] tokens[{}..{}] ({:?}): no matching rule",
            span.label, span.start, span.end, span_tokens
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentence(token_count: usize, spans: Vec<(&str, usize, usize)>) -> InputSentence {
        InputSentence {
            tokens: (0..token_count)
                .map(|index| format!("t{}", index))
                .collect(),
            spans: spans
                .into_iter()
                .map(|(label, start, end)| Span {
                    label: label.to_string(),
                    start,
                    end,
                })
                .collect(),
        }
    }

    #[test]
    fn valid_tree_assigns_nearest_strict_parents() {
        let input = sentence(
            5,
            vec![("n", 1, 2), ("dp", 0, 2), ("dp", 3, 5), ("s", 0, 5)],
        );

        let parents = validate_span_tree(&input).unwrap();

        assert_eq!(
            parents.get(&SpanKey {
                label: "n".into(),
                start: 1,
                end: 2,
            }),
            Some(&Some(SpanKey {
                label: "dp".into(),
                start: 0,
                end: 2,
            }))
        );

        assert_eq!(
            parents.get(&SpanKey {
                label: "s".into(),
                start: 0,
                end: 5,
            }),
            Some(&None)
        );
    }

    #[test]
    fn rejects_empty_or_reversed_spans() {
        let empty = sentence(3, vec![("dp", 1, 1), ("s", 0, 3)]);
        assert!(validate_span_tree(&empty)
            .unwrap_err()
            .contains("start must be before end"));

        let reversed = sentence(3, vec![("dp", 2, 1), ("s", 0, 3)]);
        assert!(validate_span_tree(&reversed)
            .unwrap_err()
            .contains("start must be before end"));
    }

    #[test]
    fn rejects_out_of_bounds_spans() {
        let input = sentence(3, vec![("dp", 2, 4), ("s", 0, 3)]);
        assert!(validate_span_tree(&input)
            .unwrap_err()
            .contains("sentence has 3 tokens"));
    }

    #[test]
    fn rejects_duplicate_intervals_even_with_different_labels() {
        let input = sentence(3, vec![("dp", 0, 1), ("n", 0, 1), ("s", 0, 3)]);

        assert!(validate_span_tree(&input)
            .unwrap_err()
            .contains("duplicate span interval 0..1"));
    }

    #[test]
    fn rejects_crossing_spans() {
        let input = sentence(5, vec![("left", 0, 3), ("right", 2, 5), ("s", 0, 5)]);

        assert!(validate_span_tree(&input)
            .unwrap_err()
            .contains("crossing spans"));
    }

    #[test]
    fn requires_one_full_sentence_root() {
        let input = sentence(4, vec![("dp", 0, 1), ("s", 0, 3)]);

        assert!(validate_span_tree(&input)
            .unwrap_err()
            .contains("full-sentence root span 0..4"));
    }

    #[test]
    fn rejects_empty_token_sequence() {
        let input = sentence(0, vec![]);
        assert_eq!(
            validate_span_tree(&input).unwrap_err(),
            "hinted sentence has no tokens"
        );
    }
}
