use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub lexicon: FixtureLexicon,
    pub grammar: Vec<FixtureRule>,
    #[serde(default)]
    pub sentences: Vec<SentenceInput>,
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
    #[serde(default)]
    pub template: String,
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
}

/// Sentences can be plain strings (old format) or structured hints (new format).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SentenceInput {
    Plain(String),
    Hinted(InputSentence),
}

impl Fixture {
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let fixture: Fixture = serde_json::from_str(&text)?;
        Ok(fixture)
    }
}
