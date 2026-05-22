use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::client::Db;
use crate::core::fixture::{Fixture, FixtureEntity, FixtureLexicon, FixturePredicate, FixtureRule};

// ── Row types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PredicateRow {
    pub id: i64,
    pub name: String,
    pub roles_json: String,
    pub forms_json: String,
    pub gloss: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EntityRow {
    pub id: i64,
    pub name: String,
    pub typ: String,
    pub forms_json: String,
    pub gloss: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RuleRow {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub template: String,
    pub kind: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SentenceRow {
    pub id: i64,
    pub text: String,
    pub notes: String,
    pub created_at: String,
}

// ── Input types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePredicate {
    pub name: String,
    #[serde(default)]
    pub roles: BTreeMap<String, String>,
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub gloss: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntity {
    pub name: String,
    #[serde(default = "default_e")]
    pub typ: String,
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub gloss: String,
}
fn default_e() -> String { "e".into() }

#[derive(Debug, Deserialize)]
pub struct CreateRule {
    pub name: String,
    pub pattern: String,
    pub template: String,
    #[serde(default = "default_fact")]
    pub kind: String,
}
fn default_fact() -> String { "fact".into() }

#[derive(Debug, Deserialize)]
pub struct CreateSentence {
    pub text: String,
    #[serde(default)]
    pub notes: String,
}

// ── Predicates ────────────────────────────────────────────────────────────────

pub async fn list_predicates(db: &Db) -> Result<Vec<PredicateRow>> {
    Ok(sqlx::query_as("SELECT id, name, roles_json, forms_json, gloss, created_at FROM predicates ORDER BY name ASC")
        .fetch_all(&db.pool).await?)
}

pub async fn create_predicate(db: &Db, input: CreatePredicate) -> Result<PredicateRow> {
    let roles_json = serde_json::to_string(&input.roles)?;
    let forms_json = serde_json::to_string(&input.forms)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO predicates (name, roles_json, forms_json, gloss) VALUES (?, ?, ?, ?) RETURNING id",
    ).bind(&input.name).bind(&roles_json).bind(&forms_json).bind(&input.gloss)
    .fetch_one(&db.pool).await?;
    get_predicate(db, id).await?.ok_or_else(|| anyhow::anyhow!("predicate vanished"))
}

pub async fn get_predicate(db: &Db, id: i64) -> Result<Option<PredicateRow>> {
    Ok(sqlx::query_as("SELECT id, name, roles_json, forms_json, gloss, created_at FROM predicates WHERE id = ?")
        .bind(id).fetch_optional(&db.pool).await?)
}

pub async fn delete_predicate(db: &Db, id: i64) -> Result<bool> {
    let r = sqlx::query("DELETE FROM predicates WHERE id = ?").bind(id).execute(&db.pool).await?;
    Ok(r.rows_affected() > 0)
}

// ── Entities ──────────────────────────────────────────────────────────────────

pub async fn list_entities(db: &Db) -> Result<Vec<EntityRow>> {
    Ok(sqlx::query_as("SELECT id, name, typ, forms_json, gloss, created_at FROM entities ORDER BY name ASC")
        .fetch_all(&db.pool).await?)
}

pub async fn create_entity(db: &Db, input: CreateEntity) -> Result<EntityRow> {
    let forms_json = serde_json::to_string(&input.forms)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (name, typ, forms_json, gloss) VALUES (?, ?, ?, ?) RETURNING id",
    ).bind(&input.name).bind(&input.typ).bind(&forms_json).bind(&input.gloss)
    .fetch_one(&db.pool).await?;
    get_entity(db, id).await?.ok_or_else(|| anyhow::anyhow!("entity vanished"))
}

pub async fn get_entity(db: &Db, id: i64) -> Result<Option<EntityRow>> {
    Ok(sqlx::query_as("SELECT id, name, typ, forms_json, gloss, created_at FROM entities WHERE id = ?")
        .bind(id).fetch_optional(&db.pool).await?)
}

pub async fn delete_entity(db: &Db, id: i64) -> Result<bool> {
    let r = sqlx::query("DELETE FROM entities WHERE id = ?").bind(id).execute(&db.pool).await?;
    Ok(r.rows_affected() > 0)
}

// ── Rules ─────────────────────────────────────────────────────────────────────

pub async fn list_rules(db: &Db) -> Result<Vec<RuleRow>> {
    Ok(sqlx::query_as("SELECT id, name, pattern, template, kind, created_at FROM rules ORDER BY id ASC")
        .fetch_all(&db.pool).await?)
}

pub async fn create_rule(db: &Db, input: CreateRule) -> Result<RuleRow> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO rules (name, pattern, template, kind) VALUES (?, ?, ?, ?) RETURNING id",
    ).bind(&input.name).bind(&input.pattern).bind(&input.template).bind(&input.kind)
    .fetch_one(&db.pool).await?;
    get_rule(db, id).await?.ok_or_else(|| anyhow::anyhow!("rule vanished"))
}

pub async fn get_rule(db: &Db, id: i64) -> Result<Option<RuleRow>> {
    Ok(sqlx::query_as("SELECT id, name, pattern, template, kind, created_at FROM rules WHERE id = ?")
        .bind(id).fetch_optional(&db.pool).await?)
}

pub async fn delete_rule(db: &Db, id: i64) -> Result<bool> {
    let r = sqlx::query("DELETE FROM rules WHERE id = ?").bind(id).execute(&db.pool).await?;
    Ok(r.rows_affected() > 0)
}

// ── Sentences ─────────────────────────────────────────────────────────────────

pub async fn list_sentences(db: &Db) -> Result<Vec<SentenceRow>> {
    Ok(sqlx::query_as("SELECT id, text, notes, created_at FROM sentences ORDER BY id ASC")
        .fetch_all(&db.pool).await?)
}

pub async fn create_sentence(db: &Db, input: CreateSentence) -> Result<SentenceRow> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sentences (text, notes) VALUES (?, ?) RETURNING id",
    ).bind(&input.text).bind(&input.notes)
    .fetch_one(&db.pool).await?;
    get_sentence(db, id).await?.ok_or_else(|| anyhow::anyhow!("sentence vanished"))
}

pub async fn get_sentence(db: &Db, id: i64) -> Result<Option<SentenceRow>> {
    Ok(sqlx::query_as("SELECT id, text, notes, created_at FROM sentences WHERE id = ?")
        .bind(id).fetch_optional(&db.pool).await?)
}

pub async fn delete_sentence(db: &Db, id: i64) -> Result<bool> {
    let r = sqlx::query("DELETE FROM sentences WHERE id = ?").bind(id).execute(&db.pool).await?;
    Ok(r.rows_affected() > 0)
}

// ── Build Fixture from DB ─────────────────────────────────────────────────────

pub async fn build_fixture(db: &Db) -> Result<Fixture> {
    let pred_rows = list_predicates(db).await?;
    let ent_rows = list_entities(db).await?;
    let rule_rows = list_rules(db).await?;
    let sent_rows = list_sentences(db).await?;

    let predicates = pred_rows.into_iter().map(|r| {
        let roles: BTreeMap<String, String> = serde_json::from_str(&r.roles_json).unwrap_or_default();
        let forms: Vec<String> = serde_json::from_str(&r.forms_json).unwrap_or_default();
        FixturePredicate { name: r.name, roles, forms, gloss: r.gloss }
    }).collect();

    let entities = ent_rows.into_iter().map(|r| {
        let forms: Vec<String> = serde_json::from_str(&r.forms_json).unwrap_or_default();
        FixtureEntity { name: r.name, typ: r.typ, forms, gloss: r.gloss }
    }).collect();

    let grammar = rule_rows.into_iter().map(|r| {
        FixtureRule { name: r.name, pattern: r.pattern, template: r.template, kind: r.kind }
    }).collect();

    let sentences = sent_rows.into_iter().map(|r| r.text).collect();

    Ok(Fixture {
        lexicon: FixtureLexicon { predicates, entities },
        grammar,
        sentences,
    })
}