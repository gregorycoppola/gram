use serde::Deserialize;
use std::collections::BTreeMap;

pub mod checker;

/// A proof file declares premises, a conclusion, and a step-by-step proof.
#[derive(Debug, Deserialize, Clone)]
pub struct ProofFile {
    pub title: String,
    pub premises: Vec<String>,
    pub conclusion: String,
    pub proof: Vec<ProofStep>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProofStep {
    pub step: usize,
    pub formula: String,
    pub justification: String,
    #[serde(default)]
    pub from: Vec<usize>,
    #[serde(default)]
    pub substitution: Option<BTreeMap<String, String>>,
}

/// Result of checking a single step.
#[derive(Debug, Clone)]
pub enum StepResult {
    Ok,
    Err(String),
}

/// Overall proof result.
#[derive(Debug)]
pub struct ProofResult {
    pub title: String,
    pub conclusion: String,
    pub steps: Vec<(ProofStep, StepResult)>,
    pub conclusion_reached: bool,
}