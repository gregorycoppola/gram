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