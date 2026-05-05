use std::collections::BTreeMap;

use crate::core::grammar::{is_ignored, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::template::apply_template;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Match {
    pub rule_name: String,
    pub kind: String,
    pub output: String,
    /// Variable name -> (canonical, type)
    pub bindings: BTreeMap<String, (String, String)>,
    /// Per-token annotation aligned 1:1 with the input sentence's tokens.
    pub token_annotations: Vec<TokenAnnotation>,
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
}

pub fn parse_sentence(tokens: &[String], lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    let mut results = Vec::new();
    for rule in rules {
        let bindings = BTreeMap::new();
        let log: Vec<Consumption> = Vec::new();
        if let Some((b, log)) = match_pattern(&rule.pattern, 0, tokens, 0, bindings, log, lexicon) {
            let output = apply_template(&rule.template, &b);
            let annotations = annotate_tokens(tokens, &log);
            results.push(Match {
                rule_name: rule.name.clone(),
                kind: rule.kind.clone(),
                output,
                bindings: b,
                token_annotations: annotations,
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
) -> Option<(BTreeMap<String, (String, String)>, Vec<Consumption>)> {
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
            return Some((bindings, log));
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
            match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon)
        }

        Slot::Literal(lit) => {
            if &token == lit {
                let mut log = log;
                log.push(Consumption::Literal { position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon)
            } else {
                None
            }
        }

        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                let mut log = log;
                log.push(Consumption::Keyword { class: kw.clone(), position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon)
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
                    if let Some(result) = match_pattern(pattern, pi + 1, tokens, ti + consumed, new_bindings, new_log, lexicon) {
                        return Some(result);
                    }
                }
            }
            if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon);
            }
            None
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
                    TokenKind::Entity // fallback for any future entity-type strings
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
            Consumption::Ignore { position } | Consumption::Skipped { position } => {
                if *position < out.len() {
                    // Don't overwrite a more-specific annotation if one was set
                    // (e.g. a Var that consumed a multi-token span including
                    // what would otherwise be a Skipped position — shouldn't
                    // happen with current logic but defensive).
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