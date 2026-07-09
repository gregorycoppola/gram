
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

use super::error::AppResult;
use super::types::{FixtureSummary, ParseRequest, ParseResult, ParseStatus};
use super::AppState;

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn list_fixtures(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FixtureSummary>>> {
    let dir = state.fixtures_dir.as_ref();
    let mut summaries = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("reading fixtures dir {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
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

    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(summaries))
}

pub async fn get_fixture(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Value>> {
    let path = fixture_path(state.fixtures_dir.as_ref(), &name);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading {}: {}", path.display(), e))?;
    let value: Value = serde_json::from_str(&raw)?;
    Ok(Json(value))
}

pub async fn parse_fixture(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Vec<ParseResult>>> {
    let path = fixture_path(state.fixtures_dir.as_ref(), &name);
    let fixture = Fixture::from_path(&path)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();
    for input in &fixture.sentences {
        match input {
            SentenceInput::Plain(_) => {
                continue;
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
    Ok(Json(results))
}

pub async fn parse_one(
    _state: State<AppState>,
    _req: Json<ParseRequest>,
) -> AppResult<Json<Vec<ParseResult>>> {
    use super::error::AppError;
    Err(AppError::from(anyhow::anyhow!("parse-one requires hinted sentences (tokens + spans); POST a JSON body with tokens and spans fields")))
}

fn fixture_path(dir: &PathBuf, name: &str) -> PathBuf {
    let mut p = dir.clone();
    p.push(format!("{}.json", name));
    p
}