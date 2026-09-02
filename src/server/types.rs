use serde::{Deserialize, Serialize};

use crate::core::matcher::{GoldEvaluation, Match};
use crate::core::proof::ProofStep;

pub const HTTP_API_VERSION: &str = "0.1";
pub const SYNTAX_FIXTURE_VERSION: u32 = 1;
pub const PROOF_FIXTURE_VERSION: u32 = 1;
pub const INFERENCE_FIXTURE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub gram_version: String,
    pub http_api_version: String,
    pub syntax_fixture_version: u32,
    pub proof_fixture_version: u32,
    pub inference_fixture_version: u32,
}

impl Default for VersionResponse {
    fn default() -> Self {
        Self {
            gram_version: env!("CARGO_PKG_VERSION").to_string(),
            http_api_version: HTTP_API_VERSION.to_string(),
            syntax_fixture_version: SYNTAX_FIXTURE_VERSION,
            proof_fixture_version: PROOF_FIXTURE_VERSION,
            inference_fixture_version: INFERENCE_FIXTURE_VERSION,
        }
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn reports_current_compatibility_versions() {
        let version = VersionResponse::default();

        assert_eq!(version.gram_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(version.http_api_version, "0.1");
        assert_eq!(version.syntax_fixture_version, 1);
        assert_eq!(version.proof_fixture_version, 1);
        assert_eq!(version.inference_fixture_version, 1);
    }
}

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

#[derive(Debug, Serialize)]
pub struct CoverageExampleSummary {
    /// Example name within the article, such as "00".
    pub name: String,

    /// Path used by the coverage fixture endpoints.
    pub fixture: String,

    pub sentence: String,
    pub status: String,
    pub parse_count: usize,
    pub semantic_count: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold_correct: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold_match_count: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CoverageArticleSummary {
    /// Filesystem-safe article identifier.
    pub name: String,

    /// Human-readable label derived from the directory name.
    pub title: String,

    pub examples: Vec<CoverageExampleSummary>,
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
