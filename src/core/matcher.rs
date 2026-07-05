use std::collections::BTreeMap;

use crate::core::grammar::{is_ignored, matches_keyword, Rule, Slot};
use crate::core::lexicon::{clean_token, Lexicon};
use crate::core::semantics::parse_with_types;
use crate::core::template::apply_template;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Constituent {
    pub label: String,
    pub semantics: String,
    pub free_vars: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
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
    /// Result of running `semantics::parse_with_types` on `output`.
    /// `None` means the check passed (no error). `Some(String)` is the
    /// error message — same string the CLI prints after `⚠️ semantics:`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics_check: Option<String>,
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
    Var {
        variable: String,
        canonical: String,
        typ: String,
        start: usize,
        end: usize,
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
    Skipped {
        position: usize,
    },
    SubClause {
        start: usize,
        end: usize,
    },
}

fn kind_to_syntax_label(kind: &str) -> &'static str {
    match kind {
        "dp" => "DP",
        "s_gapped" => "S",
        _ => "S",
    }
}

fn build_syntax(
    tokens: &[String],
    constituents: &[(usize, usize, String)],
    top_label: &str,
) -> String {
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

/// Kinds that are not top-level sentences — they are sub-constituents
/// and should be excluded by SUB:s.
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
    parse_sentence_with_vars(tokens, lexicon, rules, &[])
}

pub fn parse_sentence_with_vars(
    tokens: &[String],
    lexicon: &Lexicon,
    rules: &[Rule],
    available_vars: &[(String, String)],
) -> Vec<Match> {
    let effective_lexicon = if available_vars.is_empty() {
        return parse_sentence_inner(tokens, lexicon, rules, &KindFilter::Any);
    } else {
        lexicon.with_pronoun_bindings(available_vars)
    };
    parse_sentence_inner(tokens, &effective_lexicon, rules, &KindFilter::Any)
}

fn parse_sentence_inner(
    tokens: &[String],
    lexicon: &Lexicon,
    rules: &[Rule],
    kind_filter: &KindFilter,
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
        ) {
            let output = apply_template(&rule.template, &b);
            let annotations = annotate_tokens(tokens, &log);

            let constituent_spans: Vec<(usize, usize, String)> = constituents.iter()
                .filter_map(|c| {
                    c.span.map(|(s, e)| (s, e, c.syntax.clone().unwrap_or_default()))
                })
                .collect();
            let syntax = build_syntax(tokens, &constituent_spans, kind_to_syntax_label(&rule.kind));

            // Run the semantics type-checker on the output. The CLI does this
            // in emit_pretty; doing it here means the API response carries the
            // same diagnostic, so gloss can render it without a second call.
            // None = passed, Some(msg) = error message.
            let semantics_check = match parse_with_types(&output, lexicon) {
                Ok(_) => None,
                Err(e) => Some(e),
            };

            results.push(Match {
                rule_name: rule.name.clone(),
                kind: rule.kind.clone(),
                output,
                bindings: b,
                token_annotations: annotations,
                constituents,
                syntax: Some(syntax),
                semantics_check,
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
    kind_filter: &KindFilter,
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
            match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter)
        }

        Slot::Literal(lit) => {
            if &token == lit {
                let mut log = log;
                log.push(Consumption::Literal { position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter)
            } else {
                None
            }
        }

        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                let mut log = log;
                log.push(Consumption::Keyword { class: kw.clone(), position: ti });
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter)
            } else if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter)
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
                        new_bindings, new_log, lexicon, rules, sub_count, constituents.clone(), kind_filter,
                    ) {
                        return Some(result);
                    }
                }
            }
            if is_ignored(&token) {
                let mut log = log;
                log.push(Consumption::Skipped { position: ti });
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, log, lexicon, rules, sub_count, constituents, kind_filter);
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

            let sub_end = if let Some(delim_kw) = delimiter {
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

            let sub_filter = sub_filter_for_label(label);

            let effective_lexicon = if available_vars.is_empty() {
                None
            } else {
                Some(lexicon.with_pronoun_bindings(available_vars))
            };
            let sub_lexicon = effective_lexicon.as_ref().unwrap_or(lexicon);
            let sub_matches = parse_sentence_inner(sub_tokens, sub_lexicon, rules, &sub_filter);

            if let Some(best) = sub_matches.first() {
                let mut new_bindings = bindings.clone();
                let mut new_log = log.clone();

                new_log.push(Consumption::SubClause {
                    start: sub_start,
                    end: sub_end,
                });

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
                    span: Some((sub_start, sub_end)),
                    syntax: best.syntax.clone(),
                });

                match_pattern(
                    pattern, pi + 1, tokens, sub_end,
                    new_bindings, new_log, lexicon, rules, sub_count + 1, constituents, kind_filter,
                )
            } else {
                None
            }
        }
    }
}

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