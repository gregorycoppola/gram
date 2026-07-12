use serde::{Deserialize, Serialize};

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
    pub prob: f64,
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

fn convert_bound_entities(expr: &Expr, vars: &[(String, String)]) -> Expr {
    match expr {
        Expr::Entity(name) => {
            if let Some((_, typ)) = vars.iter().find(|(v, _)| v == name) {
                Expr::Var { name: name.clone(), typ: typ.clone() }
            } else {
                Expr::Entity(name.clone())
            }
        }
        Expr::Pred { name, roles } => Expr::Pred {
            name: name.clone(),
            roles: roles.iter().map(|(r, a)| (r.clone(), convert_bound_entities(a, vars))).collect(),
        },
        Expr::Not(e) => Expr::Not(Box::new(convert_bound_entities(e, vars))),
        Expr::And(l, r) => Expr::And(
            Box::new(convert_bound_entities(l, vars)),
            Box::new(convert_bound_entities(r, vars)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(convert_bound_entities(ante, vars)),
            cons: Box::new(convert_bound_entities(cons, vars)),
        },
        Expr::ForAll { var, var_type, body } => Expr::ForAll {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::The { var, var_type, body } => Expr::The {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::This { var, var_type, body } => Expr::This {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::That { var, var_type, body } => Expr::That {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::Exists { var, var_type, body, count } => Expr::Exists {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
            count: count.clone(),
        },
        Expr::ExistsMany { var, var_type, count, body } => Expr::ExistsMany {
            var: var.clone(),
            var_type: var_type.clone(),
            count: count.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::Var { name, typ } => Expr::Var { name: name.clone(), typ: typ.clone() },
        Expr::Question { label, body } => Expr::Question {
            label: label.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub formula: String,
    pub prob: f64,
    pub expected: Option<f64>,
    pub tolerance: f64,
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceResult {
    pub title: String,
    pub query_results: Vec<QueryResult>,
    pub stats: (usize, usize, usize, usize, usize, usize),
    pub iterations: usize,
    pub graph: Option<crate::core::qbbn::factor_graph::GraphSnapshot>,
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
        if debug {
            println!("=== Parsed rule ===");
            println!("  AST: {:?}", expr);
            println!("  to_string: {}", expr);
        }
        let (premises, conclusion, variables) = extract_horn_clause(&expr)?;
        let premises: Vec<Expr> = premises.iter().map(|p| convert_bound_entities(p, &variables)).collect();
        let conclusion = convert_bound_entities(&conclusion, &variables);
        if debug {
            println!("=== Extracted Horn clause (after convert) ===");
            for (i, p) in premises.iter().enumerate() {
                println!("  premise[{}]: {}  AST: {:?}", i, p, p);
            }
            println!("  conclusion: {}  AST: {:?}", conclusion, conclusion);
            println!("  variables: {:?}", variables);
        }
        kb.add_rule(premises, conclusion, variables, rule.weight);
    }

    if debug {
        println!("=== KB state ===");
        println!("  entities: {:?}", kb.entities);
        println!("  types: {:?}", kb.types);
        let grounded = kb.ground_all();
        println!("  grounded clauses ({}):", grounded.len());
        for (i, c) in grounded.iter().enumerate() {
            println!("    clause {}:", i);
            for (j, p) in c.premises.iter().enumerate() {
                println!("      premise[{}]: {}  AST: {:?}", j, p, p);
            }
            println!("      conclusion: {}  AST: {:?}", c.conclusion, c.conclusion);
            println!("      variables: {:?}", c.variables);
            println!("      is_fact: {}", c.is_fact());
        }
    }

    let mut graph = QBBNGraph::from_kb(&kb);

    for ev in &fixture.evidence {
        let parsed = semantics::parse(&ev.formula)
            .map_err(|e| format!("parse error in evidence '{}': {}", ev.formula, e))?;
        let canonical = parsed.to_string();
        if debug {
            println!("=== Evidence ===");
            println!("  raw: {}", ev.formula);
            println!("  parsed: {}  AST: {:?}", parsed, parsed);
            println!("  canonical: {}", canonical);
        }
        if !graph.set_evidence(&canonical, ev.prob) {
            return Err(format!("evidence formula '{}' (canonical: '{}') not found in graph", ev.formula, canonical));
        }
    }

    let trace = belief_propagation(&mut graph, 50, 0.5, 1e-6, debug);

    let mut query_results = Vec::new();
    for q in &fixture.queries {
        let parsed = semantics::parse(&q.formula)
            .map_err(|e| format!("parse error in query '{}': {}", q.formula, e))?;
        let canonical = parsed.to_string();
        if debug {
            println!("=== Query ===");
            println!("  raw: {}", q.formula);
            println!("  parsed: {}  AST: {:?}", parsed, parsed);
            println!("  canonical: {}", canonical);
        }
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

    let graph_snapshot = graph.snapshot();

    Ok(InferenceResult {
        title: fixture.title.clone(),
        query_results,
        stats: graph.stats(),
        iterations: trace.iterations.len(),
        graph: Some(graph_snapshot),
    })
}