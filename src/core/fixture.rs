use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub lexicon: FixtureLexicon,
    pub grammar: Vec<FixtureRule>,
    #[serde(default)]
    pub sentences: Vec<String>,
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

fn default_entity_type() -> String { "e".to_string() }

#[derive(Debug, Deserialize)]
pub struct FixtureRule {
    pub name: String,
    pub pattern: String,
    pub template: String,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String { "fact".to_string() }

impl Fixture {
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let fixture: Fixture = serde_json::from_str(&text)?;
        Ok(fixture)
    }
}