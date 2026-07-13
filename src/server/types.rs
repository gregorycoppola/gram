use serde::{Deserialize, Serialize};

use crate::core::matcher::{GoldEvaluation, Match};
use crate::core::proof::ProofStep;

/// One parsed sentence — the unit gloss renders.
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseResult {
    pub sentence: String,
    pub tokens: Vec<String>,
    pub matches: Vec<Match>,
    pub status: ParseStatus,
    pub semantic_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold_evaluation: Option<GoldEvaluation>,
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

/// POST /proof/check request body.
#[derive(Debug, Deserialize)]
pub struct CheckProofRequest {
    pub title: String,
    pub premises: Vec<String>,
    pub conclusion: String,
    pub proof: Vec<ProofStep>,
}

/// POST /proof/check response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckProofResponse {
    pub title: String,
    pub conclusion: String,
    pub all_steps_valid: bool,
    pub conclusion_derived: bool,
    pub final_step_is_conclusion: bool,
    pub proof_valid: bool,
    pub steps: Vec<CheckProofStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckProofStep {
    pub step: usize,
    pub formula: String,
    pub justification: String,
    pub from: Vec<usize>,
    pub ok: bool,
    pub error: Option<String>,
}
