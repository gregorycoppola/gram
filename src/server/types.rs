use serde::{Deserialize, Serialize};

use crate::core::matcher::Match;

/// One parsed sentence — the unit gloss renders.
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseResult {
    pub sentence: String,
    pub tokens: Vec<String>,
    pub matches: Vec<Match>,
    pub status: ParseStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseStatus {
    Parsed,
    Ambiguous,
    Failed,
    Error(String),
}

impl ParseStatus {
    pub fn from_matches(matches: &[Match]) -> Self {
        match matches.len() {
            0 => ParseStatus::Failed,
            1 => ParseStatus::Parsed,
            _ => ParseStatus::Ambiguous,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ParseRequest {
    /// Fixture name (without .json extension) whose lexicon/grammar to use.
    pub fixture: String,
    /// Sentence to parse.
    pub sentence: String,
}

#[derive(Debug, Serialize)]
pub struct FixtureSummary {
    pub name: String,
    pub predicates: usize,
    pub entities: usize,
    pub rules: usize,
    pub sentences: usize,
}