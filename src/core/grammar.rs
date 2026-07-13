use anyhow::{anyhow, Result};

use crate::core::fixture::FixtureRule;
use crate::core::sem_dsl::SemSpec;

#[derive(Debug, Clone)]
pub enum Slot {
    Literal(String),
    Keyword(String),
    Var {
        name: String,
        type_constraint: Option<String>,
    },
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
    pub pattern_str: String,
    pub kind: String,
    pub sem: SemSpec,
}

pub fn compile_rules(rules: &[FixtureRule]) -> Result<Vec<Rule>> {
    rules.iter().map(|r| compile_rule(r)).collect()
}

fn compile_rule(r: &FixtureRule) -> Result<Rule> {
    let pattern: Vec<Slot> = r
        .pattern
        .split_whitespace()
        .map(parse_slot)
        .collect::<Result<Vec<_>>>()?;
    let sem_source = r
        .sem
        .as_deref()
        .ok_or_else(|| anyhow!("rule {:?} is missing required sem", r.name))?;
    let sem = crate::core::sem_dsl::parse_sem(sem_source).map_err(|error| {
        anyhow!(
            "rule {:?} has invalid sem {:?}: {}",
            r.name,
            sem_source,
            error
        )
    })?;

    Ok(Rule {
        name: r.name.clone(),
        pattern,
        pattern_str: r.pattern.clone(),
        kind: r.kind.clone(),
        sem,
    })
}

fn parse_slot(spec: &str) -> Result<Slot> {
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
            let var_spec = var_spec
                .strip_prefix('$')
                .ok_or_else(|| anyhow!("SUB var must start with $: {}", var_spec))?;
            let (name, typ) = var_spec
                .split_once(':')
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
        ("COP", &["is", "are", "was", "were", "am", "be"]),
        ("ALL", &["all", "every"]),
        ("ANY", &["any"]),
        ("IF", &["if", "when", "whenever"]),
        ("THEN", &["then"]),
        ("AND", &["and", "&"]),
        ("NOT", &["not", "never", "no", "n't"]),
        ("A", &["a", "an"]),
        ("SOMEONE", &["someone", "somebody", "anyone"]),
        ("WH", &["who", "what", "where", "when", "why", "how"]),
        ("THERE", &["there"]),
        ("THE", &["the"]),
        ("THIS", &["this"]),
        ("THAT", &["that"]),
        ("WHO", &["who"]),
    ]
}

pub fn punctuation_tokens() -> &'static [&'static str] {
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

pub fn is_punctuation(token: &str) -> bool {
    punctuation_tokens()
        .iter()
        .any(|p| p.eq_ignore_ascii_case(token))
}

pub fn pronouns() -> &'static [&'static str] {
    &[
        "they",
        "he",
        "she",
        "them",
        "everyone",
        "everybody",
        "anyone",
        "anybody",
    ]
}

pub fn is_pronoun(token: &str) -> bool {
    pronouns().iter().any(|p| p.eq_ignore_ascii_case(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_rule(name: &str, sem: Option<&str>) -> FixtureRule {
        FixtureRule {
            name: name.to_string(),
            pattern: "$x:e".to_string(),
            kind: "dp".to_string(),
            sem: sem.map(str::to_string),
        }
    }

    #[test]
    fn compile_rejects_rule_without_typed_semantics() {
        let error = compile_rules(&[fixture_rule("missing_sem", None)])
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("missing_sem") && error.contains("missing required sem"),
            "{error}"
        );
    }

    #[test]
    fn compile_reports_rule_name_for_malformed_semantics() {
        let error = compile_rules(&[fixture_rule("broken_sem", Some("bare_dp($x"))])
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("broken_sem") && error.contains("invalid sem"),
            "{error}"
        );
    }

    #[test]
    fn compile_accepts_rule_with_typed_semantics() {
        let rules = compile_rules(&[fixture_rule("typed_rule", Some("bare_dp($x)"))]).unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].sem.constructor, "bare_dp");
    }
}
