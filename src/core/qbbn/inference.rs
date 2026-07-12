use serde::Deserialize;

use crate::core::logic::Expr;
use crate::core::semantics;
use super::kb::KnowledgeBase;
use super::factor_graph::QBBNGraph;
use super::bp::belief_propagation;

#[derive(Debug, Deserialize)]
pub struct InferenceFixture {
    pub title: String,
    #[serde(default)]
    pub entities: Vec<FixtureEntity>,
    #[serde(default)]
    pub rules: Vec<FixtureRule>,
    #[serde(default)]
    pub evidence: Vec<FixtureEvidence>,
    #[serde(default)]
    pub queries: Vec<FixtureQuery>,
}

#[derive(Debug, Deserialize)]
pub struct FixtureEntity {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: String,
}

#[derive(Debug, Deserialize)]
pub struct FixtureRule {
    pub formula: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
}

fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct FixtureEvidence {
    pub formula: String,
    pub value: bool,
}

#[derive(Debug, Deserialize)]
pub struct FixtureQuery {
    pub formula: String,
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    #[serde(default)]
    pub expected_prob: Option<f64>,
}

fn default_tolerance() -> f64 {
    0.01
}

fn extract_horn_clause(expr: &Expr) -> Result<(Vec<Expr>, Expr, Vec<(String, String)>), String> {
    match expr {
        Expr::ForAll { var, var_type, body } => {
            let (premises, conclusion, mut vars) = extract_impl(body)?;
            vars.insert(0, (var.clone(), var_type.clone()));
            Ok((premises, conclusion, vars))
        }
        Expr::Implies { ante, cons } => {
            let premises = flatten_and(ante);
            Ok((premises, *cons.clone(), Vec::new()))
        }
        _ => {
            Ok((Vec::new(), expr.clone(), Vec::new()))
        }
    }
}

fn extract_impl(expr: &Expr) -> Result<(Vec<Expr>, Expr, Vec<(String, String)>), String> {
    match expr {
        Expr::Implies { ante, cons } => {
            let premises = flatten_and(ante);
            Ok((premises, *cons.clone(), Vec::new()))
        }
        other => Err(format!("expected implication inside ForAll, got: {}", other)),
    }
}

fn flatten_and(expr: &Expr) -> Vec<Expr> {
    match expr {
        Expr::And(l, r) => {
            let mut left = flatten_and(l);
            left.extend(flatten_and(r));
            left
        }
        other => vec![other.clone()],
    }
}

#[derive(Debug)]
pub struct QueryResult {
    pub formula: String,
    pub prob: f64,
    pub expected: Option<f64>,
    pub tolerance: f64,
    pub ok: bool,
}

#[derive(Debug)]
pub struct InferenceResult {
    pub title: String,
    pub query_results: Vec<QueryResult>,
    pub stats: (usize, usize, usize, usize, usize, usize),
    pub iterations: usize,
}

pub fn run_inference_fixture(fixture: &InferenceFixture) -> Result<InferenceResult, String> {
    run_inference_fixture_debug(fixture, false)
}

pub fn run_inference_fixture_debug(fixture: &InferenceFixture, debug: bool) -> Result<InferenceResult, String> {
    let mut kb = KnowledgeBase::new();

    for ent in &fixture.entities {
        kb.add_entity(ent.name.clone(), ent.typ.clone());
    }

    for rule in &fixture.rules {
        let expr = semantics::parse(&rule.formula)
            .map_err(|e| format!("parse error in '{}': {}", rule.formula, e))?;
        let (premises, conclusion, variables) = extract_horn_clause(&expr)?;
        kb.add_rule(premises, conclusion, variables, rule.weight);
    }

    let mut graph = QBBNGraph::from_kb(&kb);

    for ev in &fixture.evidence {
        let parsed = semantics::parse(&ev.formula)
            .map_err(|e| format!("parse error in evidence '{}': {}", ev.formula, e))?;
        let canonical = parsed.to_string();
        if !graph.set_evidence(&canonical, ev.value) {
            return Err(format!("evidence formula '{}' (canonical: '{}') not found in graph", ev.formula, canonical));
        }
    }

    let trace = belief_propagation(&mut graph, 50, 0.5, 1e-6, debug);

    let mut query_results = Vec::new();
    for q in &fixture.queries {
        let parsed = semantics::parse(&q.formula)
            .map_err(|e| format!("parse error in query '{}': {}", q.formula, e))?;
        let canonical = parsed.to_string();
        let prob = graph.prob(&canonical);
        let ok = if let Some(expected) = q.expected_prob {
            (prob - expected).abs() <= q.tolerance
        } else {
            true
        };
        query_results.push(QueryResult {
            formula: q.formula.clone(),
            prob,
            expected: q.expected_prob,
            tolerance: q.tolerance,
            ok,
        });
    }

    Ok(InferenceResult {
        title: fixture.title.clone(),
        query_results,
        stats: graph.stats(),
        iterations: trace.iterations.len(),
    })
}