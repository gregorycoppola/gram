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
}

pub fn parse_sentence(tokens: &[String], lexicon: &Lexicon, rules: &[Rule]) -> Vec<Match> {
    let mut results = Vec::new();
    for rule in rules {
        let bindings = BTreeMap::new();
        if let Some(b) = match_pattern(&rule.pattern, 0, tokens, 0, bindings, lexicon) {
            let output = apply_template(&rule.template, &b);
            results.push(Match {
                rule_name: rule.name.clone(),
                kind: rule.kind.clone(),
                output,
                bindings: b,
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
    lexicon: &Lexicon,
) -> Option<BTreeMap<String, (String, String)>> {
    // Skip ignored tokens unless the next slot is a literal or ignore (which consumes one token verbatim).
    let next_is_literal_or_ignore = pi < pattern.len()
        && matches!(pattern[pi], Slot::Literal(_) | Slot::Ignore);

    let mut ti = ti;
    if !next_is_literal_or_ignore {
        while ti < tokens.len() && is_ignored(&clean_token(&tokens[ti])) {
            ti += 1;
        }
    }

    if pi >= pattern.len() {
        // Pattern exhausted — accept if remaining tokens are all ignored.
        let mut ti = ti;
        while ti < tokens.len() && is_ignored(&clean_token(&tokens[ti])) {
            ti += 1;
        }
        if ti >= tokens.len() {
            return Some(bindings);
        }
        return None;
    }

    if ti >= tokens.len() {
        return None;
    }

    let slot = &pattern[pi];
    let token = clean_token(&tokens[ti]);

    match slot {
        Slot::Ignore => match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, lexicon),

        Slot::Literal(lit) => {
            if &token == lit {
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, lexicon)
            } else if is_ignored(&token) {
                match_pattern(pattern, pi, tokens, ti + 1, bindings, lexicon)
            } else {
                None
            }
        }

        Slot::Keyword(kw) => {
            if matches_keyword(&token, kw) {
                match_pattern(pattern, pi + 1, tokens, ti + 1, bindings, lexicon)
            } else if is_ignored(&token) {
                match_pattern(pattern, pi, tokens, ti + 1, bindings, lexicon)
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
                    new_bindings.insert(name.clone(), (canonical, actual_type));
                    if let Some(result) = match_pattern(pattern, pi + 1, tokens, ti + consumed, new_bindings, lexicon) {
                        return Some(result);
                    }
                }
            }
            if is_ignored(&token) {
                return match_pattern(pattern, pi, tokens, ti + 1, bindings, lexicon);
            }
            None
        }
    }
}