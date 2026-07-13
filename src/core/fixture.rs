use anyhow::Result;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub lexicon: FixtureLexicon,
    pub grammar: Vec<FixtureRule>,
    #[serde(default, deserialize_with = "deserialize_sentences")]
    pub sentences: Vec<InputSentence>,
}

#[derive(Debug, Deserialize)]
pub struct FixtureLexicon {
    #[serde(default)]
    pub predicates: Vec<FixturePredicate>,
    #[serde(default)]
    pub entities: Vec<FixtureEntity>,
}

#[derive(Debug, Deserialize)]
pub struct FixturePredicate {
    pub name: String,
    pub roles: BTreeMap<String, String>,
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub gloss: String,
}

#[derive(Debug, Deserialize)]
pub struct FixtureEntity {
    pub name: String,
    #[serde(rename = "type", default = "default_entity_type")]
    pub typ: String,
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub gloss: String,
}

fn default_entity_type() -> String {
    "e".to_string()
}

#[derive(Debug, Deserialize)]
pub struct FixtureRule {
    pub name: String,
    pub pattern: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub sem: Option<String>,
}

fn default_kind() -> String {
    "fact".to_string()
}

/// A labeled span over a token list, used as a parsing hint.
#[derive(Debug, Clone, Deserialize)]
pub struct Span {
    pub label: String,
    pub start: usize,
    pub end: usize,
}

/// A sentence with explicit phrase-structure hints.
#[derive(Debug, Clone, Deserialize)]
pub struct InputSentence {
    pub tokens: Vec<String>,
    pub spans: Vec<Span>,
    #[serde(default)]
    pub gold: Option<String>,
}

fn deserialize_sentences<'de, D>(deserializer: D) -> Result<Vec<InputSentence>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;

    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            if value.is_string() {
                return Err(<D::Error as serde::de::Error>::custom(format!(
                    "sentences[{index}] is a legacy plain string; use an object with tokens and spans"
                )));
            }

            serde_json::from_value(value).map_err(|error| {
                <D::Error as serde::de::Error>::custom(format!(
                    "invalid sentences[{index}]: {error}"
                ))
            })
        })
        .collect()
}

impl Fixture {
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let fixture: Fixture = serde_json::from_str(&text)?;
        Ok(fixture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_legacy_plain_sentence_string() {
        let json = r#"{
            "lexicon": {},
            "grammar": [],
            "sentences": ["Sue is happy"]
        }"#;

        let error = serde_json::from_str::<Fixture>(json)
            .unwrap_err()
            .to_string();

        assert!(error.contains("legacy plain string"), "{error}");
        assert!(error.contains("object with tokens and spans"), "{error}");
    }
}
