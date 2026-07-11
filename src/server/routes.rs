use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_hinted_sentence;
use crate::core::proof::checker::check_proof;
use crate::core::proof::ProofFile;
use crate::core::proof::StepResult;
use crate::core::tokenize::tokenize;

use super::error::{AppError, AppResult};
use super::types::{
    CheckProofRequest, CheckProofResponse, CheckProofStep, FixtureSummary, ParseRequest,
    ParseResult, ParseStatus,
};
use super::AppState;

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn check_proof(Json(req): Json<CheckProofRequest>) -> AppResult<Json<CheckProofResponse>> {
    let file = ProofFile {
        title: req.title,
        premises: req.premises,
        conclusion: req.conclusion,
        proof: req.proof,
    };

    let result = check_proof(&file).map_err(AppError::from)?;

    let steps = result
        .steps
        .into_iter()
        .map(|(step, status)| {
            let (ok, error) = match status {
                StepResult::Ok => (true, None),
                StepResult::Err(e) => (false, Some(e)),
            };
            CheckProofStep {
                step: step.step,
                formula: step.formula,
                justification: step.justification,
                from: step.from,
                ok,
                error,
            }
        })
        .collect();

    Ok(Json(CheckProofResponse {
        title: result.title,
        conclusion: result.conclusion,
        conclusion_reached: result.conclusion_reached,
        steps,
    }))
}

pub async fn list_fixtures(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FixtureSummary>>> {
    let dir = state.fixtures_dir.as_ref();
    let mut summaries = Vec::new();
    walk_dir(dir, "", &mut summaries, 0);
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(summaries))
}

fn walk_dir(dir: &std::path::Path, prefix: &str, summaries: &mut Vec<FixtureSummary>, depth: usize) {
    if depth > 10 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if dir_name.starts_with('.') {
                continue;
            }
            let new_prefix = if prefix.is_empty() {
                dir_name.to_string()
            } else {
                format!("{}/{}", prefix, dir_name)
            };
            walk_dir(&path, &new_prefix, summaries, depth + 1);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let file_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if file_name.starts_with('.') {
                continue;
            }
            let name = if prefix.is_empty() {
                file_name.to_string()
            } else {
                format!("{}/{}", prefix, file_name)
            };
            match Fixture::from_path(&path) {
                Ok(fixture) => {
                    summaries.push(FixtureSummary {
                        name,
                        predicates: fixture.lexicon.predicates.len(),
                        entities: fixture.lexicon.entities.len(),
                        rules: fixture.grammar.len(),
                        sentences: fixture.sentences.len(),
                    });
                }
                Err(_) => continue,
            }
        }
    }
}

pub async fn handle_fixture(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> AppResult<Json<Value>> {
    if let Some(name) = path.strip_suffix("/parse") {
        let results = do_parse_fixture(state, name).await?;
        let value = serde_json::to_value(results)
            .map_err(|e| anyhow::anyhow!("serializing parse results: {}", e))?;
        Ok(Json(value))
    } else {
        let raw = do_get_fixture(state, &path).await?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parsing fixture JSON: {}", e))?;
        Ok(Json(value))
    }
}

async fn do_get_fixture(state: AppState, name: &str) -> Result<String, anyhow::Error> {
    let path = fixture_path(state.fixtures_dir.as_ref(), name)?;
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading {}: {}", path.display(), e))
}

async fn do_parse_fixture(state: AppState, name: &str) -> Result<Vec<ParseResult>, anyhow::Error> {
    let path = fixture_path(state.fixtures_dir.as_ref(), name)?;
    let fixture = Fixture::from_path(&path)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();
    for input in &fixture.sentences {
        match input {
            SentenceInput::Plain(text) => {
                let tokens = tokenize(text);
                results.push(ParseResult {
                    sentence: text.clone(),
                    tokens,
                    matches: Vec::new(),
                    status: ParseStatus::Failed,
                });
            }
            SentenceInput::Hinted(s) => {
                let (matches, status) = match parse_hinted_sentence(s, &lexicon, &rules) {
                    Ok(m) => {
                        let status = ParseStatus::from_matches(&m);
                        (m, status)
                    }
                    Err(e) => (Vec::new(), ParseStatus::Error(e)),
                };
                results.push(ParseResult {
                    sentence: s.tokens.join(" "),
                    tokens: s.tokens.clone(),
                    matches,
                    status,
                });
            }
        }
    }
    Ok(results)
}

pub async fn parse_one(
    _state: State<AppState>,
    _req: Json<ParseRequest>,
) -> AppResult<Json<Vec<ParseResult>>> {
    Err(AppError::from(anyhow::anyhow!("parse-one requires hinted sentences (tokens + spans); POST a JSON body with tokens and spans fields")))
}

fn fixture_path(dir: &PathBuf, name: &str) -> Result<PathBuf, anyhow::Error> {
    let mut p = dir.clone();
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            anyhow::bail!("invalid fixture name segment: {:?}", segment);
        }
        if segment.contains('\\') || segment.contains('\0') {
            anyhow::bail!("invalid fixture name segment: {:?}", segment);
        }
        p.push(segment);
    }
    p.set_extension("json");
    Ok(p)
}