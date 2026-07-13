use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use std::path::PathBuf;

use crate::core::fixture::Fixture;
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::{evaluate_gold, parse_hinted_sentence, semantic_count};
use crate::core::proof::checker::check_proof as proof_check;
use crate::core::proof::ProofFile;
use crate::core::proof::StepResult;

use super::error::{AppError, AppResult};
use super::types::{
    CheckProofRequest, CheckProofResponse, CheckProofStep, CoverageArticleSummary,
    CoverageExampleSummary, FixtureSummary, ParseRequest, ParseResult, ParseStatus,
};
use super::AppState;

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn list_proofs(State(state): State<AppState>) -> AppResult<Json<Vec<String>>> {
    let dir = state.proofs_dir.join("regression");
    let mut names = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| AppError(anyhow::anyhow!("reading proofs dir: {}", e)))?
        {
            if let Ok(e) = entry {
                if let Some(name) = e.path().file_stem().and_then(|s| s.to_str()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(Json(names))
}

pub async fn get_proof(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Value>> {
    let path = state
        .proofs_dir
        .join("regression")
        .join(format!("{}.json", name));
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AppError(anyhow::anyhow!("reading proof {}: {}", path.display(), e)))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| AppError(anyhow::anyhow!("parsing proof JSON: {}", e)))?;
    Ok(Json(value))
}

pub async fn check_proof(
    Json(req): Json<CheckProofRequest>,
) -> AppResult<Json<CheckProofResponse>> {
    let file = ProofFile {
        title: req.title,
        premises: req.premises,
        conclusion: req.conclusion,
        proof: req.proof,
    };

    let result = proof_check(&file).map_err(|e| AppError(anyhow::anyhow!(e)))?;

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
        all_steps_valid: result.all_steps_valid,
        conclusion_derived: result.conclusion_derived,
        final_step_is_conclusion: result.final_step_is_conclusion,
        proof_valid: result.proof_valid,
        steps,
    }))
}

pub async fn list_fixtures(State(state): State<AppState>) -> AppResult<Json<Vec<FixtureSummary>>> {
    let dir = state.fixtures_dir.as_ref();
    let mut summaries = Vec::new();
    walk_dir(dir, "", &mut summaries, 0);
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(summaries))
}

fn walk_dir(
    dir: &std::path::Path,
    prefix: &str,
    summaries: &mut Vec<FixtureSummary>,
    depth: usize,
) {
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
            let dir_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
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
            let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
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

pub async fn list_coverage(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<CoverageArticleSummary>>> {
    let news_dir = state.data_dir.join("news");

    if !news_dir.is_dir() {
        return Err(AppError(anyhow::anyhow!(
            "coverage news directory not found: {}",
            news_dir.display()
        )));
    }

    let mut article_dirs = std::fs::read_dir(&news_dir)
        .map_err(|error| {
            AppError(anyhow::anyhow!(
                "reading coverage directory {}: {}",
                news_dir.display(),
                error
            ))
        })?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();

    article_dirs.sort_by_key(|entry| entry.file_name());

    let mut articles = Vec::new();

    for article_entry in article_dirs {
        let article_name = match article_entry.file_name().to_str() {
            Some(name) if !name.starts_with('.') => name.to_string(),
            _ => continue,
        };

        let parses_dir = article_entry.path().join("parses");

        if !parses_dir.is_dir() {
            continue;
        }

        let mut fixture_paths = std::fs::read_dir(&parses_dir)
            .map_err(|error| {
                AppError(anyhow::anyhow!(
                    "reading article parses {}: {}",
                    parses_dir.display(),
                    error
                ))
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|value| value.to_str()) == Some("json")
            })
            .collect::<Vec<_>>();

        fixture_paths.sort();

        let mut examples = Vec::new();

        for fixture_path in fixture_paths {
            let example_name = match fixture_path
                .file_stem()
                .and_then(|value| value.to_str())
            {
                Some(name) => name.to_string(),
                None => continue,
            };

            let fixture_name = format!(
                "{}/parses/{}",
                article_name,
                example_name
            );

            let fixture = Fixture::from_path(&fixture_path).map_err(|error| {
                AppError(anyhow::anyhow!(
                    "loading coverage fixture {}: {}",
                    fixture_path.display(),
                    error
                ))
            })?;

            let sentence = fixture
                .sentences
                .first()
                .map(|sentence| sentence.tokens.join(" "))
                .unwrap_or_default();

            let results = parse_fixture_at_path(&fixture_path).map_err(AppError)?;
            let result = results.into_iter().next();

            let (
                status,
                parse_count,
                semantic_count,
                gold,
                gold_correct,
                gold_match_count,
                error,
            ) = match result {
                Some(result) => {
                    let status = parse_status_label(&result.status).to_string();
                    let error = match &result.status {
                        ParseStatus::Error(message) => Some(message.clone()),
                        _ => None,
                    };

                    let gold = result
                        .gold_evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.gold.clone());

                    let gold_correct = result
                        .gold_evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.correct);

                    let gold_match_count = result
                        .gold_evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.gold_match_count);

                    (
                        status,
                        result.matches.len(),
                        result.semantic_count,
                        gold,
                        gold_correct,
                        gold_match_count,
                        error,
                    )
                }
                None => (
                    "empty".to_string(),
                    0,
                    0,
                    None,
                    None,
                    None,
                    Some("fixture has no sentences".to_string()),
                ),
            };

            examples.push(CoverageExampleSummary {
                name: example_name,
                fixture: fixture_name,
                sentence,
                status,
                parse_count,
                semantic_count,
                gold,
                gold_correct,
                gold_match_count,
                error,
            });
        }

        articles.push(CoverageArticleSummary {
            title: article_name.replace('_', " "),
            name: article_name,
            examples,
        });
    }

    Ok(Json(articles))
}

pub async fn handle_coverage(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> AppResult<Json<Value>> {
    let root = state.data_dir.join("news");

    if let Some(name) = path.strip_suffix("/parse") {
        let fixture_path = fixture_path(&root, name)?;
        let results = parse_fixture_at_path(&fixture_path)?;
        let value = serde_json::to_value(results)
            .map_err(|error| {
                anyhow::anyhow!(
                    "serializing coverage parse results: {}",
                    error
                )
            })?;

        Ok(Json(value))
    } else {
        let fixture_path = fixture_path(&root, &path)?;
        let raw = std::fs::read_to_string(&fixture_path).map_err(|error| {
            anyhow::anyhow!(
                "reading coverage fixture {}: {}",
                fixture_path.display(),
                error
            )
        })?;

        let value: Value = serde_json::from_str(&raw)
            .map_err(|error| {
                anyhow::anyhow!(
                    "parsing coverage fixture JSON: {}",
                    error
                )
            })?;

        Ok(Json(value))
    }
}

fn parse_status_label(status: &ParseStatus) -> &'static str {
    match status {
        ParseStatus::Parsed => "parsed",
        ParseStatus::Ambiguous => "ambiguous",
        ParseStatus::Failed => "failed",
        ParseStatus::Error(_) => "error",
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
    std::fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("reading {}: {}", path.display(), e))
}

async fn do_parse_fixture(
    state: AppState,
    name: &str,
) -> Result<Vec<ParseResult>, anyhow::Error> {
    let path = fixture_path(state.fixtures_dir.as_ref(), name)?;
    parse_fixture_at_path(&path)
}

fn parse_fixture_at_path(
    path: &std::path::Path,
) -> Result<Vec<ParseResult>, anyhow::Error> {
    let fixture = Fixture::from_path(path)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar)?;

    let mut results = Vec::new();

    for sentence in &fixture.sentences {
        let (matches, status, distinct_semantics, gold_evaluation) =
            match parse_hinted_sentence(sentence, &lexicon, &rules) {
                Ok(matches) => {
                    let status = ParseStatus::from_matches(&matches);
                    let distinct_semantics = semantic_count(&matches);
                    let gold_evaluation = sentence
                        .gold
                        .as_deref()
                        .map(|gold| evaluate_gold(&matches, gold))
                        .transpose()
                        .map_err(anyhow::Error::msg)?;

                    (
                        matches,
                        status,
                        distinct_semantics,
                        gold_evaluation,
                    )
                }
                Err(error) => (
                    Vec::new(),
                    ParseStatus::Error(error),
                    0,
                    None,
                ),
            };

        results.push(ParseResult {
            sentence: sentence.tokens.join(" "),
            tokens: sentence.tokens.clone(),
            matches,
            status,
            semantic_count: distinct_semantics,
            gold_evaluation,
        });
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

pub async fn list_inference_fixtures(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<String>>> {
    let dir = state.fixtures_dir.join("qbbn");
    let mut names = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| AppError(anyhow::anyhow!("reading qbbn dir: {}", e)))?
        {
            if let Ok(e) = entry {
                if let Some(name) = e.path().file_stem().and_then(|s| s.to_str()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(Json(names))
}

pub async fn run_inference(
    Json(req): Json<crate::core::qbbn::inference::InferenceFixture>,
) -> AppResult<Json<crate::core::qbbn::inference::InferenceResult>> {
    let result = crate::core::qbbn::inference::run_inference_fixture(&req)
        .map_err(|e| AppError(anyhow::anyhow!("inference failed: {}", e)))?;
    Ok(Json(result))
}
