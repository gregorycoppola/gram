use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::db::store::{
    self, CreateEntity, CreatePredicate, CreateRule, CreateSentence,
};
use crate::core::grammar::compile_rules;
use crate::core::lexicon::Lexicon;
use crate::core::matcher::parse_sentence;
use crate::core::tokenize::{split_sentences, tokenize};
use super::types::{ParseResult, ParseStatus};
use super::AppState;

type Res = Result<Json<Value>, (StatusCode, String)>;
fn e500(e: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

// ── Predicates ────────────────────────────────────────────────────────────────

pub async fn list_predicates(State(s): State<AppState>) -> Res {
    let rows = store::list_predicates(&s.db).await.map_err(e500)?;
    Ok(Json(json!({ "predicates": rows })))
}

pub async fn create_predicate(State(s): State<AppState>, Json(input): Json<CreatePredicate>) -> Res {
    let row = store::create_predicate(&s.db, input).await.map_err(e500)?;
    Ok(Json(json!({ "predicate": row })))
}

pub async fn delete_predicate(State(s): State<AppState>, Path(id): Path<i64>) -> Res {
    let ok = store::delete_predicate(&s.db, id).await.map_err(e500)?;
    if ok { Ok(Json(json!({ "deleted": id }))) }
    else { Err((StatusCode::NOT_FOUND, format!("no predicate {id}"))) }
}

// ── Entities ──────────────────────────────────────────────────────────────────

pub async fn list_entities(State(s): State<AppState>) -> Res {
    let rows = store::list_entities(&s.db).await.map_err(e500)?;
    Ok(Json(json!({ "entities": rows })))
}

pub async fn create_entity(State(s): State<AppState>, Json(input): Json<CreateEntity>) -> Res {
    let row = store::create_entity(&s.db, input).await.map_err(e500)?;
    Ok(Json(json!({ "entity": row })))
}

pub async fn delete_entity(State(s): State<AppState>, Path(id): Path<i64>) -> Res {
    let ok = store::delete_entity(&s.db, id).await.map_err(e500)?;
    if ok { Ok(Json(json!({ "deleted": id }))) }
    else { Err((StatusCode::NOT_FOUND, format!("no entity {id}"))) }
}

// ── Rules ─────────────────────────────────────────────────────────────────────

pub async fn list_rules(State(s): State<AppState>) -> Res {
    let rows = store::list_rules(&s.db).await.map_err(e500)?;
    Ok(Json(json!({ "rules": rows })))
}

pub async fn create_rule(State(s): State<AppState>, Json(input): Json<CreateRule>) -> Res {
    let fixture_rule = crate::core::fixture::FixtureRule {
        name: input.name.clone(),
        pattern: input.pattern.clone(),
        template: input.template.clone(),
        kind: input.kind.clone(),
    };
    if let Err(e) = crate::core::grammar::compile_rules(&[fixture_rule]) {
        return Err((StatusCode::BAD_REQUEST, format!("invalid pattern: {e}")));
    }
    let row = store::create_rule(&s.db, input).await.map_err(e500)?;
    Ok(Json(json!({ "rule": row })))
}

pub async fn delete_rule(State(s): State<AppState>, Path(id): Path<i64>) -> Res {
    let ok = store::delete_rule(&s.db, id).await.map_err(e500)?;
    if ok { Ok(Json(json!({ "deleted": id }))) }
    else { Err((StatusCode::NOT_FOUND, format!("no rule {id}"))) }
}

// ── Sentences ─────────────────────────────────────────────────────────────────

pub async fn list_sentences(State(s): State<AppState>) -> Res {
    let rows = store::list_sentences(&s.db).await.map_err(e500)?;
    Ok(Json(json!({ "sentences": rows })))
}

pub async fn create_sentence(State(s): State<AppState>, Json(input): Json<CreateSentence>) -> Res {
    let row = store::create_sentence(&s.db, input).await.map_err(e500)?;
    Ok(Json(json!({ "sentence": row })))
}

pub async fn delete_sentence(State(s): State<AppState>, Path(id): Path<i64>) -> Res {
    let ok = store::delete_sentence(&s.db, id).await.map_err(e500)?;
    if ok { Ok(Json(json!({ "deleted": id }))) }
    else { Err((StatusCode::NOT_FOUND, format!("no sentence {id}"))) }
}

// ── Parse all ─────────────────────────────────────────────────────────────────

pub async fn parse_all(State(s): State<AppState>) -> Result<Json<Vec<ParseResult>>, (StatusCode, String)> {
    let fixture = store::build_fixture(&s.db).await.map_err(e500)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).map_err(e500)?;

    let mut results: Vec<ParseResult> = Vec::new();
    for raw in &fixture.sentences {
        for sent in split_sentences(raw) {
            let tokens = tokenize(&sent);
            let matches = parse_sentence(&tokens, &lexicon, &rules);
            let status = ParseStatus::from_matches(&matches);
            results.push(ParseResult { sentence: sent, tokens, matches, status });
        }
    }
    Ok(Json(results))
}

// ── Parse one ─────────────────────────────────────────────────────────────────

pub async fn parse_one(State(s): State<AppState>, Json(body): Json<Value>) -> Result<Json<Vec<ParseResult>>, (StatusCode, String)> {
    let text = body.get("sentence")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing sentence".into()))?
        .to_string();

    let fixture = store::build_fixture(&s.db).await.map_err(e500)?;
    let lexicon = Lexicon::from_fixture(&fixture);
    let rules = compile_rules(&fixture.grammar).map_err(e500)?;

    let mut results: Vec<ParseResult> = Vec::new();
    for sent in split_sentences(&text) {
        let tokens = tokenize(&sent);
        let matches = parse_sentence(&tokens, &lexicon, &rules);
        let status = ParseStatus::from_matches(&matches);
        results.push(ParseResult { sentence: sent, tokens, matches, status });
    }
    Ok(Json(results))
}