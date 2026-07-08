
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use std::path::PathBuf;

use crate::core::fixture::{Fixture, SentenceInput};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::{split_sentences, tokenize};

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
        let plain_matches: Vec<_> = match input {
            SentenceInput::Plain(s) => {
                split_sentences(s).into_iter().map(|sent| {
                    let tokens = tokenize(&sent);
                    let matches = parse_sentence(&tokens, &lexicon, &rules);
                    (sent, tokens, matches)
                }).collect()
            }
            SentenceInput::Hinted(s) => {
                vec![(s.tokens.join(" "), s.tokens.clone(), Vec::new())]
            }
        };
        for (sent, tokens, matches) in plain_matches {
            let status = ParseStatus::from_matches(&matches);
            results.push(ParseResult { sentence: sent, tokens, matches, status });
        }
    }
    Ok(Json(results))
}

pub async fn parse_one(
    State(state): State<AppState>,
    Json(req): Json<ParseRequest>,
) -> AppResult<Json<Vec<ParseResult>>> {
    let path = fixture_path(state.fixtures_dir.as_ref(), &req.fixture);
    let fixture = Fixture::from_path(&path)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();
    for sent in split_sentences(&req.sentence) {
        let tokens = tokenize(&sent);
        let matches = parse_sentence(&tokens, &lexicon, &rules);
        let status = ParseStatus::from_matches(&matches);
        results.push(ParseResult { sentence: sent, tokens, matches, status });
    }
    Ok(Json(results))
}

fn fixture_path(dir: &PathBuf, name: &str) -> PathBuf {
    let mut p = dir.clone();
    p.push(format!("{}.json", name));
    p
}