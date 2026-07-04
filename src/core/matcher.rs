use std::collections::BTreeMap;

use crate::core::grammar::{is_ignored, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::template::apply_template;

/// A parsed sub-phrase: a labeled box with semantic output and free variables.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Constituent {
    pub label: String,
    pub semantics: String,
    pub free_vars: Vec<(String, String)>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Match {
    pub rule_name: String,
    pub kind: String,
    pub output: String,
    /// Variable name -> (canonical, type)
    pub bindings: BTreeMap<String, (String, String)>,
    /// Per-token annotation aligned 1:1 with the input sentence's tokens.
    pub token_annotations: Vec<TokenAnnotation>,
    /// Constituents produced by sub-clause slots, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constituents: Vec<Constituent>,
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

/// One pattern slot's consumption — where it landed in the token stream and
/// what it bound to. Recorded by the matcher and consumed by the annotator.
#[derive(Debug, Clone)]
enum Consumption {
    Var {
        variable: String,
        canonical: String,
        typ: String,
        start: usize,
        end: usize, // exclusive
    },
    Keyword {
        class: String,
        position: usize,
    },
    Literal {
        position: usize,
    },
    Ignore {
        position: usize,
    },
    /// A function-word skip the matcher took (an IGNORED token consumed
    /// without advancing the pattern). Recorded so we can annotate it.
    Skipped {
        position: usize,
    },
    /// A sub-clause consumed by a Sub slot.
    SubClause {
        start: usize,
        end: usize, // exclusive
    },
}

pub fn parse_sentence(tokens: &[String], lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    parse_sentence_with_vars(tokens, lexicon, rules, &[])
}

/// Parse a sentence, optionally with variables available for pronoun resolution.
/// Used recursively for sub-clauses.
pub fn parse_sentence_with_vars(
    tokens: &[String],
    lexicon: &Lexicon,
    rules: &[Rule],
    available_vars: &[(String, String)],
) -> Vec<Match> {
    let effective_lexicon = if available_vars.is_empty() {
        return parse_sentence_inner(tokens, lexicon, rules);
    } else {
        lexicon.with_pronoun_bindings(available_vars)
    };
    parse_sentence_inner(tokens, &effective_lexicon, rules)
}

fn parse_sentence_inner(tokens: &[String], lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    let mut results = Vec::new();
    for rule in rules {
        let bindings = BTreeMap::new();
        let log: Vec<Consumption> = Vec::new();
        let constituents = Vec::new();
        if let Some((b, log, constituents)) = match_pattern(
            &rule.pattern, 0, tokens, 0, bindings, log, lexicon, rules, 0, constituents,
        ) {
            let output = apply_template(&rule.template, &b);
            let annotations = annotate_tokens(tokens, &log);
            results.push(Match {
                rule_name: rule.name.clone(),
                kind: rule.kind.clone(),
                output,
                bindings: b,
                token_annotations: annotations,
                constituents,
            });
        }
    }
    results
}

fn match_pattern(
    pattern: &[Slot],
    pi: usize,
    tokens: &[String],
    ti: usize,
    bindings: BTreeMap<String, (String, String)>,
    log: Vec<Consumption>,
    lexicon: &Lexicon,
    rules: &[Rule],
    sub_count: usize,
    mut constituents: Vec<Constituent>,
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
        // Pattern exhausted — accept if remaining tokens are all ignored.
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
            match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents)
        }

        Slot::Literal(lit) => {
            if &token == lit {
                let mut log = log;
                log.push(Consumption::Literal { position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents)
            } else {
                None
            }
        }

        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                let mut log = log;
                log.push(Consumption::Keyword { class: kw.clone(), position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents)
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
                        variable: name.clone(),
                        canonical,
                        typ: actual_type,
                        start: ti,
                        end: ti + consumed,
                    });
                    if let Some(result) = match_pattern(
                        pattern, pi + 1, tokens, ti + consumed,
                        new_bindings, new_log, lexicon, rules, sub_count, constituents.clone(),
                    ) {
                        return Some(result);
                    }
                }
            }
            if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents);
            }
            None
        }

        Slot::Sub { label, available_vars, delimiter } => {
            // Skip any leading ignored tokens.
            let sub_start = ti;
            let mut sub_ti = ti;
            while sub_ti < tokens.len() && is_ignored(&clean_token(&tokens[sub_ti])) {
                log.push(Consumption::Skipped { position: sub_ti });
                sub_ti += 1;
            }

            // Find where the sub-span ends.
            let sub_end = if let Some(delim_kw) = delimiter {
                // Scan for the delimiter keyword in the remaining tokens.
                let mut end = tokens.len();
                for i in sub_ti..tokens.len() {
                    if matches_keyword(&clean_token(&tokens[i]), delim_kw) {
                        end = i;
                        break;
                    }
                }
                end
            } else {
                tokens.len()
            };

            let sub_tokens = &tokens[sub_ti..sub_end];

            if sub_tokens.is_empty() {
                return None;
            }

            // Recursively parse the sub-span with available vars for pronoun resolution.
            let sub_matches = parse_sentence_with_vars(sub_tokens, lexicon, rules, available_vars);

            // Take the best (first) match.
            if let Some(best) = sub_matches.first() {
                let mut new_bindings = bindings.clone();
                let mut new_log = log.clone();

                // Record the sub-clause consumption.
                new_log.push(Consumption::SubClause {
                    start: sub_start,
                    end: sub_end,
                });

                // Binding key: SUB for first, SUB2 for second, etc.
                let sub_key = if sub_count == 0 {
                    "SUB".to_string()
                } else {
                    format!("SUB{}", sub_count + 1)
                };
                new_bindings.insert(sub_key, (best.output.clone(), label.clone()));

                // Build the constituent.
                constituents.push(Constituent {
                    label: label.clone(),
                    semantics: best.output.clone(),
                    free_vars: available_vars.clone(),
                });

                // Continue matching from sub_end (the delimiter token, if any,
                // is left for the next pattern slot to consume).
                match_pattern(
                    pattern, pi + 1, tokens, sub_end,
                    new_bindings, new_log, lexicon, rules, sub_count + 1, constituents,
                )
            } else {
                None
            }
        }
    }
}

/// Turn a successful match's consumption log into per-token annotations.
fn annotate_tokens(tokens: &[String], log: &[Consumption]) -> Vec<TokenAnnotation> {
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
                    if i < out.len() {
                        if matches!(out[i].kind, TokenKind::Unmatched | TokenKind::Ignored) {
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
            }
            Consumption::Ignore { position } | Consumption::Skipped { position } => {
                if *position < out.len() {
                    if matches!(out[*position].kind, TokenKind::Unmatched) {
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
    }

    out
}