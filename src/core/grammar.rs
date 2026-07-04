use anyhow::{anyhow, Result};

use crate::core::fixture::FixtureRule;

#[derive(Debug, Clone)]
pub enum Slot {
    Ignore,
    Literal(String),
    Keyword(String),
    Var { name: String, type_constraint: Option<String> },
    Sub {
        label: String,
        available_vars: Vec<(String, String)>,
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
    if !spec.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        return Err(anyhow!("invalid slot: {}", spec));
    }
    Ok(Slot::Keyword(spec.to_string()))
}

fn parse_sub_slot(rest: &str) -> Result<Slot> {
    let mut delimiter: Option<String> = None;
    let mut label_vars = rest;

    let mut depth = 0usize;
    let bytes = rest.as_bytes();
    let mut last_keyword_colon = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => depth -= 1,
            b':' if depth == 0 => {
                let after = &rest[i + 1..];
                if !after.is_empty() && after.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
                    last_keyword_colon = Some(i);
                }
            }
            _ => {}
        }
    }

    if let Some(colon_pos) = last_keyword_colon {
        delimiter = Some(rest[colon_pos + 1..].to_string());
        label_vars = &rest[..colon_pos];
    }

    parse_sub_slot_parts(label_vars, delimiter)
}

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

pub fn keywords() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("COP",     &["is", "are", "was", "were", "am", "be"]),
        ("ALL",     &["all", "every"]),
        ("ANY",     &["any"]),
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
        ("WHO",     &["who"]),
    ]
}

pub fn ignored_tokens() -> &'static [&'static str] {
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

pub fn pronouns() -> &'static [&'static str] {
    &["they", "he", "she", "them", "everyone", "everybody", "anyone", "anybody"]
}

pub fn is_pronoun(token: &str) -> bool {
    pronouns().iter().any(|p| p.eq_ignore_ascii_case(token))
}