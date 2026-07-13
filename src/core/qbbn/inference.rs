use serde::{Deserialize, Serialize};

use crate::core::logic::Expr;
use crate::core::semantics;

use super::bp::belief_propagation;
use super::exact::{exact_inference, ExactConfig};
use super::factor_graph::QBBNGraph;
use super::kb::KnowledgeBase;
use super::topology::{analyze_topology, GraphTopology};

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

fn extract_horn_clause(
    expr: &Expr,
) -> Result<(Vec<Expr>, Expr, Vec<(String, String)>), String> {
    match expr {
        Expr::ForAll {
            var,
            var_type,
            body,
        } => {
            let (premises, conclusion, mut vars) = extract_impl(body)?;
            vars.insert(0, (var.clone(), var_type.clone()));
            Ok((premises, conclusion, vars))
        }
        Expr::Implies { ante, cons } => {
            let premises = flatten_and(ante);
            Ok((premises, *cons.clone(), Vec::new()))
        }
        _ => Ok((Vec::new(), expr.clone(), Vec::new())),
    }
}

fn extract_impl(
    expr: &Expr,
) -> Result<(Vec<Expr>, Expr, Vec<(String, String)>), String> {
    match expr {
        Expr::Implies { ante, cons } => {
            let premises = flatten_and(ante);
            Ok((premises, *cons.clone(), Vec::new()))
        }
        other => Err(format!(
            "expected implication inside ForAll, got: {}",
            other
        )),
    }
}

fn flatten_and(expr: &Expr) -> Vec<Expr> {
    match expr {
        Expr::And(left, right) => {
            let mut flattened = flatten_and(left);
            flattened.extend(flatten_and(right));
            flattened
        }
        other => vec![other.clone()],
    }
}

fn convert_bound_entities(
    expr: &Expr,
    vars: &[(String, String)],
) -> Expr {
    match expr {
        Expr::Entity(name) => {
            if let Some((_, typ)) =
                vars.iter().find(|(var, _)| var == name)
            {
                Expr::Var {
                    name: name.clone(),
                    typ: typ.clone(),
                }
            } else {
                Expr::Entity(name.clone())
            }
        }
        Expr::Pred { name, roles } => Expr::Pred {
            name: name.clone(),
            roles: roles
                .iter()
                .map(|(role, arg)| {
                    (
                        role.clone(),
                        convert_bound_entities(arg, vars),
                    )
                })
                .collect(),
        },
        Expr::Not(inner) => {
            Expr::Not(Box::new(convert_bound_entities(inner, vars)))
        }
        Expr::And(left, right) => Expr::And(
            Box::new(convert_bound_entities(left, vars)),
            Box::new(convert_bound_entities(right, vars)),
        ),
        Expr::Implies { ante, cons } => Expr::Implies {
            ante: Box::new(convert_bound_entities(ante, vars)),
            cons: Box::new(convert_bound_entities(cons, vars)),
        },
        Expr::ForAll {
            var,
            var_type,
            body,
        } => Expr::ForAll {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::The {
            var,
            var_type,
            body,
        } => Expr::The {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::This {
            var,
            var_type,
            body,
        } => Expr::This {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::That {
            var,
            var_type,
            body,
        } => Expr::That {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::Exists {
            var,
            var_type,
            body,
            count,
        } => Expr::Exists {
            var: var.clone(),
            var_type: var_type.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
            count: count.clone(),
        },
        Expr::ExistsMany {
            var,
            var_type,
            count,
            body,
        } => Expr::ExistsMany {
            var: var.clone(),
            var_type: var_type.clone(),
            count: count.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
        Expr::Var { name, typ } => Expr::Var {
            name: name.clone(),
            typ: typ.clone(),
        },
        Expr::Question { label, body } => Expr::Question {
            label: label.clone(),
            body: Box::new(convert_bound_entities(body, vars)),
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub formula: String,

    /// Approximate result produced by belief propagation.
    pub prob: f64,

    /// Exact marginal under the declared QBBN semantics.
    #[serde(default)]
    pub exact_prob: Option<f64>,

    /// Signed BP minus exact difference.
    #[serde(default)]
    pub bp_exact_delta: Option<f64>,

    pub expected: Option<f64>,
    pub tolerance: f64,

    /// Whether exact inference matches the handwritten expected value.
    ///
    /// This tests the declared model semantics, independently of BP.
    #[serde(default)]
    pub expected_ok: Option<bool>,

    /// Whether BP matches exact inference within the BP tolerance.
    #[serde(default)]
    pub bp_matches_exact: Option<bool>,

    /// Topology-aware overall status.
    ///
    /// Acyclic graph:
    /// - exact must match any handwritten expectation;
    /// - BP must match exact.
    ///
    /// Loopy graph:
    /// - exact must match any handwritten expectation;
    /// - BP versus exact is diagnostic only.
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceResult {
    pub title: String,
    pub query_results: Vec<QueryResult>,
    pub stats: (usize, usize, usize, usize, usize, usize),
    pub iterations: usize,

    pub topology: GraphTopology,

    #[serde(default)]
    pub exact_partition_function: Option<f64>,

    #[serde(default)]
    pub exact_error: Option<String>,

    pub graph:
        Option<crate::core::qbbn::factor_graph::GraphSnapshot>,
}

pub fn run_inference_fixture(
    fixture: &InferenceFixture,
) -> Result<InferenceResult, String> {
    run_inference_fixture_debug(fixture, false)
}

pub fn run_inference_fixture_debug(
    fixture: &InferenceFixture,
    debug: bool,
) -> Result<InferenceResult, String> {
    let mut kb = KnowledgeBase::new();

    for entity in &fixture.entities {
        kb.add_entity(entity.name.clone(), entity.typ.clone());
    }

    for rule in &fixture.rules {
        let expr = semantics::parse(&rule.formula)
            .map_err(|error| {
                format!(
                    "parse error in '{}': {}",
                    rule.formula, error
                )
            })?;

        if debug {
            println!("=== Parsed rule ===");
            println!("  AST: {:?}", expr);
            println!("  to_string: {}", expr);
        }

        let (premises, conclusion, variables) =
            extract_horn_clause(&expr)?;

        let premises: Vec<Expr> = premises
            .iter()
            .map(|premise| {
                convert_bound_entities(premise, &variables)
            })
            .collect();

        let conclusion =
            convert_bound_entities(&conclusion, &variables);

        if debug {
            println!("=== Extracted Horn clause (after convert) ===");

            for (index, premise) in premises.iter().enumerate() {
                println!(
                    "  premise[{}]: {}  AST: {:?}",
                    index, premise, premise
                );
            }

            println!(
                "  conclusion: {}  AST: {:?}",
                conclusion, conclusion
            );
            println!("  variables: {:?}", variables);
        }

        kb.add_rule(
            premises,
            conclusion,
            variables,
            rule.weight,
        );
    }

    if debug {
        println!("=== KB state ===");
        println!("  entities: {:?}", kb.entities);
        println!("  types: {:?}", kb.types);

        let grounded = kb.ground_all();
        println!("  grounded clauses ({}):", grounded.len());

        for (index, clause) in grounded.iter().enumerate() {
            println!("    clause {}:", index);

            for (premise_index, premise) in
                clause.premises.iter().enumerate()
            {
                println!(
                    "      premise[{}]: {}  AST: {:?}",
                    premise_index, premise, premise
                );
            }

            println!(
                "      conclusion: {}  AST: {:?}",
                clause.conclusion, clause.conclusion
            );
            println!("      variables: {:?}", clause.variables);
            println!("      is_fact: {}", clause.is_fact());
        }
    }

    let mut graph = QBBNGraph::from_kb(&kb);

    for evidence in &fixture.evidence {
        let parsed = semantics::parse(&evidence.formula)
            .map_err(|error| {
                format!(
                    "parse error in evidence '{}': {}",
                    evidence.formula, error
                )
            })?;

        let canonical = parsed.to_string();

        if debug {
            println!("=== Evidence ===");
            println!("  raw: {}", evidence.formula);
            println!("  parsed: {}  AST: {:?}", parsed, parsed);
            println!("  canonical: {}", canonical);
            println!("  likelihood: {:.6}", evidence.prob);
        }

        if !graph.set_evidence(&canonical, evidence.prob) {
            return Err(format!(
                "evidence formula '{}' (canonical: '{}') not found in graph",
                evidence.formula, canonical
            ));
        }
    }

    let topology = analyze_topology(&graph);

    if debug {
        println!("=== Factor-graph topology ===");
        println!("  kind: {}", topology.kind());
        println!("  acyclic: {}", topology.acyclic);
        println!(
            "  components: {}  variables: {}  factors: {}  edges: {}",
            topology.connected_components,
            topology.variable_nodes,
            topology.factor_nodes,
            topology.edges
        );
    }

    let exact_attempt = exact_inference(
        &graph,
        ExactConfig {
            retain_assignments: false,
            ..ExactConfig::default()
        },
    );

    let (
        exact_result,
        exact_partition_function,
        exact_error,
    ) = match exact_attempt {
        Ok(result) => {
            let partition = Some(result.partition_function);
            (Some(result), partition, None)
        }
        Err(error) => {
            if debug {
                println!("=== Exact inference unavailable ===");
                println!("  {}", error);
            }

            (None, None, Some(error.to_string()))
        }
    };

    let trace =
        belief_propagation(&mut graph, 50, 0.5, 1e-6, debug);

    let mut query_results = Vec::new();

    for query in &fixture.queries {
        let parsed = semantics::parse(&query.formula)
            .map_err(|error| {
                format!(
                    "parse error in query '{}': {}",
                    query.formula, error
                )
            })?;

        let canonical = parsed.to_string();
        let bp_prob = graph.prob(&canonical);

        let exact_prob = exact_result
            .as_ref()
            .and_then(|result| {
                result.prob_formula(&graph, &canonical).ok()
            });

        let bp_exact_delta =
            exact_prob.map(|exact| bp_prob - exact);

        let expected_ok = query.expected_prob.map(|expected| {
            exact_prob
                .map(|exact| {
                    (exact - expected).abs()
                        <= query.tolerance + 1e-12
                })
                .unwrap_or(false)
        });

        let bp_matches_exact = exact_prob.map(|exact| {
            (bp_prob - exact).abs() <= 1e-6
        });

        let semantic_ok = expected_ok.unwrap_or(true);

        let algorithm_ok = if topology.bp_should_be_exact() {
            bp_matches_exact.unwrap_or(false)
        } else {
            true
        };

        let ok = semantic_ok && algorithm_ok;

        if debug {
            println!("=== Query ===");
            println!("  raw: {}", query.formula);
            println!("  parsed: {}  AST: {:?}", parsed, parsed);
            println!("  canonical: {}", canonical);
            println!("  BP: {:.12}", bp_prob);

            if let Some(exact) = exact_prob {
                println!("  exact: {:.12}", exact);
                println!("  BP - exact: {:+.12}", bp_prob - exact);
            }

            println!("  expected_ok: {:?}", expected_ok);
            println!("  bp_matches_exact: {:?}", bp_matches_exact);
            println!("  topology-aware ok: {}", ok);
        }

        query_results.push(QueryResult {
            formula: query.formula.clone(),
            prob: bp_prob,
            exact_prob,
            bp_exact_delta,
            expected: query.expected_prob,
            tolerance: query.tolerance,
            expected_ok,
            bp_matches_exact,
            ok,
        });
    }

    let graph_snapshot = graph.snapshot();

    Ok(InferenceResult {
        title: fixture.title.clone(),
        query_results,
        stats: graph.stats(),
        iterations: trace.iterations.len(),
        topology,
        exact_partition_function,
        exact_error,
        graph: Some(graph_snapshot),
    })
}