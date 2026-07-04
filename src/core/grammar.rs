use anyhow::{anyhow, Result};

use crate::core::fixture::FixtureRule;

#[derive(Debug, Clone)]
pub enum Slot {
    /// Match any token, ignore it.
    Ignore,
    /// Match a specific literal word.
    Literal(String),
    /// Match a keyword class (COP, ALL, IF, etc.).
    Keyword(String),
    /// Match a token that resolves to a typed predicate or entity.
    /// type_constraint is e.g. "e" or "{theme:e,reference:e}" (always canonicalized) or None for "any".
    Var { name: String, type_constraint: Option<String> },
    /// Match a sub-span as a constituent. Consumes remaining tokens unless
    /// a delimiter keyword is specified, in which case it consumes up to
    /// (but not including) the delimiter.
    /// available_vars are variable bindings provided by the outer rule's template
    /// context (e.g. a quantifier variable that the sub-clause's pronouns resolve to).
    Sub {
        label: String,
        available_vars: Vec<(String, String)>,
        /// If Some, consume only up to this keyword class (e.g. "THEN", "AND").
        delimiter: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub pattern: Vec<Slot>,
    pub template: String,
    pub kind: String,
}

pub fn compile_rules(rules: &[FixtureRule]) -> Result<Vec<Rule>> {
    rules.iter().map(|r| compile_rule(r)).collect()
}

fn compile_rule(r: &FixtureRule) -> Result<Rule> {
    let pattern: Vec<Slot> = r.pattern.split_whitespace()
        .map(parse_slot)
        .collect::<Result<Vec<_>>>()?;
    Ok(Rule {
        name: r.name.clone(),
        pattern,
        template: r.template.clone(),
        kind: r.kind.clone(),
    })
}

fn parse_slot(spec: &str) -> Result<Slot> {
    if spec == "_" {
        return Ok(Slot::Ignore);
    }
    if let Some(rest) = spec.strip_prefix("LIT:") {
        return Ok(Slot::Literal(rest.to_lowercase()));
    }
    if let Some(rest) = spec.strip_prefix("SUB:") {
        return parse_sub_slot(rest);
    }
    if let Some(rest) = spec.strip_prefix('$') {
        if let Some((name, typ)) = rest.split_once(':') {
            return Ok(Slot::Var {
                name: format!("${}", name),
                type_constraint: Some(canonicalize_type(typ)),
            });
        }
        return Ok(Slot::Var {
            name: format!("${}", rest),
            type_constraint: None,
        });
    }
    // Otherwise it's a keyword (COP, ALL, IF, THEN, AND, NOT, A, SOMEONE, WH, THERE, ...).
    if !spec.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        return Err(anyhow!("invalid slot: {}", spec));
    }
    Ok(Slot::Keyword(spec.to_string()))
}

/// Parse the part after "SUB:" — e.g. "s", "s[$x:e]", "s:THEN", "s:THEN[$x:e]".
fn parse_sub_slot(rest: &str) -> Result<Slot> {
    // Split off optional delimiter after label: "s:THEN" or "s"
    let (label_and_vars, delimiter) = if let Some(colon_pos) = rest.find(':') {
        // Could be "s:THEN" or "s[$x:e]:THEN" — the colon inside brackets is not a delimiter separator.
        // Check if there's a bracket before this colon.
        let bracket_pos = rest.find('[');
        if let Some(bp) = bracket_pos {
            if bp < colon_pos {
                // Colon is inside or after brackets — not a delimiter separator.
                // Look for next colon after the closing bracket.
                if let Some(close_bracket) = rest.find(']') {
                    if close_bracket + 1 < rest.len() && rest.as_bytes()[close_bracket + 1] == b':' {
                        let lbl = &rest[..close_bracket + 1];
                        let delim = &rest[close_bracket + 2..];
                        return parse_sub_slot_parts(lbl, Some(delim.to_string()));
                    }
                }
                // No colon after bracket — no delimiter.
                parse_sub_slot_parts(rest, None)?
            } else {
                // Colon before bracket — "s:THEN[$x:e]"
                let lbl = &rest[..colon_pos];
                let after_colon = &rest[colon_pos + 1..];
                // after_colon might be "THEN[$x:e]" — split delimiter from vars
                if let Some(bracket_start) = after_colon.find('[') {
                    let delim = &after_colon[..bracket_start];
                    let vars_part = &after_colon[bracket_start..];
                    let full = format!("{}{}", lbl, vars_part);
                    parse_sub_slot_parts(&full, Some(delim.to_string()))?
                } else {
                    // No bracket — "s:THEN" with no vars
                    parse_sub_slot_parts(lbl, Some(after_colon.to_string()))?
                }
            }
        } else {
            // No bracket at all — "s:THEN"
            let lbl = &rest[..colon_pos];
            let delim = &rest[colon_pos + 1..];
            parse_sub_slot_parts(lbl, Some(delim.to_string()))?
        }
    } else {
        parse_sub_slot_parts(rest, None)?
    };

    Ok(label_and_vars)
}

/// Parse label[vars] and combine with an optional delimiter.
fn parse_sub_slot_parts(rest: &str, delimiter: Option<String>) -> Result<Slot> {
    let (label, vars_str) = if let Some(bracket_start) = rest.find('[') {
        if !rest.ends_with(']') {
            return Err(anyhow!("unterminated bracket in SUB slot: {}", rest));
        }
        let label = &rest[..bracket_start];
        let vars_str = &rest[bracket_start + 1..rest.len() - 1];
        (label, vars_str)
    } else {
        (rest, "")
    };

    let mut available_vars = Vec::new();
    if !vars_str.is_empty() {
        for var_spec in vars_str.split(',') {
            let var_spec = var_spec.trim();
            let var_spec = var_spec.strip_prefix('$')
                .ok_or_else(|| anyhow!("SUB var must start with $: {}", var_spec))?;
            let (name, typ) = var_spec.split_once(':')
                .ok_or_else(|| anyhow!("SUB var must have :type: {}", var_spec))?;
            available_vars.push((format!("${}", name), typ.to_string()));
        }
    }

    Ok(Slot::Sub {
        label: label.to_string(),
        available_vars,
        delimiter,
    })
}

/// Canonicalize a type string so role-typed signatures compare equal regardless
/// of role order. "e" stays "e". "{theme:e,reference:e}" and
/// "{reference:e,theme:e}" both become "{reference:e,theme:e}" (alphabetical).
pub fn canonicalize_type(typ: &str) -> String {
    let trimmed = typ.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return trimmed.to_string();
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.is_empty() {
        return "{}".to_string();
    }
    let mut parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    parts.sort();
    format!("{{{}}}", parts.join(","))
}

/// Map of keyword class -> set of surface forms.
pub fn keywords() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("COP",     &["is", "are", "was", "were", "am", "be"]),
        ("ALL",     &["all", "every"]),
        ("IF",      &["if", "when", "whenever"]),
        ("THEN",    &["then"]),
        ("AND",     &["and", "&"]),
        ("NOT",     &["not", "never", "no", "n't"]),
        ("A",       &["a", "an"]),
        ("SOMEONE", &["someone", "somebody", "anyone"]),
        ("WH",      &["who", "what", "where", "when", "why", "how"]),
        ("THERE",   &["there"]),
        ("THE",     &["the"]),
        ("THIS",    &["this"]),
        ("THAT",    &["that"]),
    ]
}

/// Tokens that are matched-then-skipped.
pub fn ignored_tokens() -> &'static [&'static str] {
    // Only punctuation. Function words (articles, pronouns, prepositions)
    // must be explicitly matched with _ in patterns so that preposition
    // presence/absence is structurally significant.
    &[".", ",", "!", "?"]
}

pub fn matches_keyword(token: &str, keyword: &str) -> bool {
    for (kw, forms) in keywords() {
        if *kw == keyword {
            return forms.iter().any(|f| f.eq_ignore_ascii_case(token));
        }
    }
    false
}

pub fn is_ignored(token: &str) -> bool {
    ignored_tokens().iter().any(|i| i.eq_ignore_ascii_case(token))
}

/// Pronouns that can resolve to an available variable in a sub-clause.
pub fn pronouns() -> &'static [&'static str] {
    &["they", "he", "she", "them", "everyone", "everybody", "anyone", "anybody"]
}

pub fn is_pronoun(token: &str) -> bool {
    pronouns().iter().any(|p| p.eq_ignore_ascii_case(token))
}