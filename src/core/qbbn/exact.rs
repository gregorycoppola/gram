//! Exact inference for small QBBN graphs.
//!
//! This module is intentionally simple and exponential. Its purpose is to
//! define and inspect the probability distribution represented by a QBBN,
//! providing an oracle against which approximate inference can be tested.
//!
//! Semantics used here:
//!
//! - Every proposition with no producing OR factor has a Bernoulli root prior.
//!   The default prior is 0.5.
//! - An AND factor is deterministic:
//!     g = literal_1 AND ... AND literal_n
//! - An OR factor uses the same log-linear CPT as `bp::cpt_prob_true`.
//! - `evidence_prob = q` is represented as a unary likelihood:
//!     likelihood(X = true)  = q
//!     likelihood(X = false) = 1 - q
//! - The product of all factors is globally normalized.
//!
//! For a source proposition with a 0.5 root prior, evidence probability `q`
//! therefore produces posterior `q`. Hard evidence is represented by 0.0 or
//! 1.0.
//!
//! This interpretation also remains well-defined for graphs containing directed
//! cycles: the local conditional tables are treated as factors, their product is
//! enumerated, and the resulting finite distribution is normalized.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use super::bp::cpt_prob_true;
use super::factor_graph::{FactorType, NodeType, QBBNGraph};

#[derive(Debug, Clone)]
pub struct ExactConfig {
    /// Refuse to enumerate graphs larger than this.
    pub max_variables: usize,

    /// Prior probability for propositions with no producing OR factor.
    pub root_prior: f64,

    /// Retain every normalized assignment for inspection.
    pub retain_assignments: bool,
}

impl Default for ExactConfig {
    fn default() -> Self {
        Self {
            max_variables: 20,
            root_prior: 0.5,
            retain_assignments: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExactAssignment {
    /// Boolean value for every variable ID.
    pub values: HashMap<String, bool>,

    /// Product of all local factors before global normalization.
    pub unnormalized_weight: f64,

    /// Normalized probability of this complete assignment.
    pub posterior: f64,
}

#[derive(Debug, Clone)]
pub struct ExactResult {
    /// Deterministic variable order used during enumeration.
    pub variable_ids: Vec<String>,

    /// Sum of all unnormalized assignment weights.
    pub partition_function: f64,

    /// Exact posterior P(variable = true), keyed by variable ID.
    pub marginals: HashMap<String, f64>,

    /// Complete assignments, when `retain_assignments` is enabled.
    pub assignments: Vec<ExactAssignment>,
}

impl ExactResult {
    pub fn prob_var(&self, variable_id: &str) -> Result<f64, ExactError> {
        self.marginals
            .get(variable_id)
            .copied()
            .ok_or_else(|| ExactError::UnknownVariable(variable_id.to_string()))
    }

    pub fn prob_formula(
        &self,
        graph: &QBBNGraph,
        formula: &str,
    ) -> Result<f64, ExactError> {
        let (positive_formula, negate) = match formula.strip_prefix("not ") {
            Some(rest) => (rest, true),
            None => (formula, false),
        };

        let variable_id = graph
            .formula_to_id
            .get(positive_formula)
            .ok_or_else(|| ExactError::UnknownVariable(formula.to_string()))?;

        let prob = self.prob_var(variable_id)?;
        Ok(if negate { 1.0 - prob } else { prob })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExactError {
    TooManyVariables {
        count: usize,
        max: usize,
    },
    InvalidProbability {
        label: String,
        value: f64,
    },
    UnknownVariable(String),
    MissingProducer(String),
    MultipleProducers {
        variable_id: String,
        first_factor: String,
        second_factor: String,
    },
    WrongProducerType {
        variable_id: String,
        expected: String,
        actual: String,
    },
    MalformedFactor {
        factor_id: String,
        detail: String,
    },
    MissingRule {
        group_id: String,
        rule_id: String,
    },
    ZeroPartition,
}

impl Display for ExactError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExactError::TooManyVariables { count, max } => {
                write!(
                    f,
                    "exact inference requires enumerating 2^{count} assignments; \
                     configured maximum is {max} variables"
                )
            }
            ExactError::InvalidProbability { label, value } => {
                write!(f, "invalid probability for {label}: {value}")
            }
            ExactError::UnknownVariable(id) => {
                write!(f, "unknown variable: {id}")
            }
            ExactError::MissingProducer(id) => {
                write!(f, "group variable {id} has no producing AND factor")
            }
            ExactError::MultipleProducers {
                variable_id,
                first_factor,
                second_factor,
            } => {
                write!(
                    f,
                    "variable {variable_id} has multiple producing factors: \
                     {first_factor} and {second_factor}"
                )
            }
            ExactError::WrongProducerType {
                variable_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "variable {variable_id} must be produced by {expected}, \
                     but is produced by {actual}"
                )
            }
            ExactError::MalformedFactor { factor_id, detail } => {
                write!(f, "malformed factor {factor_id}: {detail}")
            }
            ExactError::MissingRule { group_id, rule_id } => {
                write!(
                    f,
                    "group {group_id} refers to missing rule {rule_id}"
                )
            }
            ExactError::ZeroPartition => {
                write!(
                    f,
                    "all complete assignments have zero weight; \
                     the evidence or factor graph is inconsistent"
                )
            }
        }
    }
}

impl Error for ExactError {}

fn factor_type_name(factor_type: &FactorType) -> &'static str {
    match factor_type {
        FactorType::And => "AND",
        FactorType::Or => "OR",
    }
}

fn validate_probability(label: impl Into<String>, value: f64) -> Result<(), ExactError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ExactError::InvalidProbability {
            label: label.into(),
            value,
        })
    }
}

/// Map every produced variable to its unique producing factor.
fn build_producer_map(
    graph: &QBBNGraph,
) -> Result<HashMap<String, String>, ExactError> {
    let mut producers = HashMap::new();

    for factor in graph.factors.values() {
        if factor.input_ids.len() != factor.input_negated.len() {
            return Err(ExactError::MalformedFactor {
                factor_id: factor.id.clone(),
                detail: format!(
                    "{} inputs but {} negation flags",
                    factor.input_ids.len(),
                    factor.input_negated.len()
                ),
            });
        }

        for input_id in &factor.input_ids {
            if !graph.variables.contains_key(input_id) {
                return Err(ExactError::MalformedFactor {
                    factor_id: factor.id.clone(),
                    detail: format!("unknown input variable {input_id}"),
                });
            }
        }

        if !graph.variables.contains_key(&factor.output_id) {
            return Err(ExactError::MalformedFactor {
                factor_id: factor.id.clone(),
                detail: format!("unknown output variable {}", factor.output_id),
            });
        }

        if let Some(first_factor) =
            producers.insert(factor.output_id.clone(), factor.id.clone())
        {
            return Err(ExactError::MultipleProducers {
                variable_id: factor.output_id.clone(),
                first_factor,
                second_factor: factor.id.clone(),
            });
        }
    }

    for variable in graph.variables.values() {
        match variable.node_type {
            NodeType::Group => {
                let factor_id = producers
                    .get(&variable.id)
                    .ok_or_else(|| ExactError::MissingProducer(variable.id.clone()))?;

                let factor = &graph.factors[factor_id];
                if factor.factor_type != FactorType::And {
                    return Err(ExactError::WrongProducerType {
                        variable_id: variable.id.clone(),
                        expected: "AND".to_string(),
                        actual: factor_type_name(&factor.factor_type).to_string(),
                    });
                }
            }
            NodeType::Proposition => {
                if let Some(factor_id) = producers.get(&variable.id) {
                    let factor = &graph.factors[factor_id];
                    if factor.factor_type != FactorType::Or {
                        return Err(ExactError::WrongProducerType {
                            variable_id: variable.id.clone(),
                            expected: "OR".to_string(),
                            actual: factor_type_name(&factor.factor_type).to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(producers)
}

fn assignment_value(
    variable_id: &str,
    values: &[bool],
    indices: &HashMap<String, usize>,
) -> Result<bool, ExactError> {
    let index = indices
        .get(variable_id)
        .copied()
        .ok_or_else(|| ExactError::UnknownVariable(variable_id.to_string()))?;

    Ok(values[index])
}

fn assignment_weight(
    graph: &QBBNGraph,
    config: &ExactConfig,
    producers: &HashMap<String, String>,
    indices: &HashMap<String, usize>,
    values: &[bool],
) -> Result<f64, ExactError> {
    let mut weight = 1.0;

    // Priors for source propositions.
    for variable in graph.variables.values() {
        if variable.node_type == NodeType::Proposition
            && !producers.contains_key(&variable.id)
        {
            let value = assignment_value(&variable.id, values, indices)?;
            weight *= if value {
                config.root_prior
            } else {
                1.0 - config.root_prior
            };
        }
    }

    // Conditional factors.
    for factor in graph.factors.values() {
        let output_value =
            assignment_value(&factor.output_id, values, indices)?;

        match factor.factor_type {
            FactorType::And => {
                let mut conjunction_value = true;

                for (index, input_id) in factor.input_ids.iter().enumerate() {
                    let raw_value =
                        assignment_value(input_id, values, indices)?;
                    let literal_value = if factor.input_negated[index] {
                        !raw_value
                    } else {
                        raw_value
                    };
                    conjunction_value &= literal_value;
                }

                if output_value != conjunction_value {
                    return Ok(0.0);
                }
            }
            FactorType::Or => {
                let mut score_pos = 0.0;
                let mut score_neg = 0.0;

                for group_id in &factor.input_ids {
                    let active =
                        assignment_value(group_id, values, indices)?;

                    if !active {
                        continue;
                    }

                    let group = graph
                        .variables
                        .get(group_id)
                        .ok_or_else(|| {
                            ExactError::UnknownVariable(group_id.clone())
                        })?;

                    let rule_id = group.rule_id.as_ref().ok_or_else(|| {
                        ExactError::MalformedFactor {
                            factor_id: factor.id.clone(),
                            detail: format!(
                                "input group {group_id} has no rule ID"
                            ),
                        }
                    })?;

                    let rule = graph.rules.get(rule_id).ok_or_else(|| {
                        ExactError::MissingRule {
                            group_id: group_id.clone(),
                            rule_id: rule_id.clone(),
                        }
                    })?;

                    if group.negated {
                        score_neg += rule.weight;
                    } else {
                        score_pos += rule.weight;
                    }
                }

                let prob_true = cpt_prob_true(score_pos, score_neg);
                weight *= if output_value {
                    prob_true
                } else {
                    1.0 - prob_true
                };
            }
        }

        if weight == 0.0 {
            return Ok(0.0);
        }
    }

    // Unary likelihood factors for evidence.
    for variable in graph.variables.values().filter(|v| v.is_evidence) {
        let probability = variable.evidence_prob.unwrap_or(0.5);
        validate_probability(
            format!("evidence on {}", variable.id),
            probability,
        )?;

        let value = assignment_value(&variable.id, values, indices)?;
        weight *= if value {
            probability
        } else {
            1.0 - probability
        };
    }

    Ok(weight)
}

/// Enumerate all complete assignments, normalize them, and return exact
/// posterior marginals.
///
/// This is exponential and intended only for tests, debugging, and small
/// explanatory examples.
pub fn exact_inference(
    graph: &QBBNGraph,
    config: ExactConfig,
) -> Result<ExactResult, ExactError> {
    validate_probability("root prior", config.root_prior)?;

    let mut variable_ids: Vec<String> =
        graph.variables.keys().cloned().collect();
    variable_ids.sort();

    let variable_count = variable_ids.len();
    if variable_count > config.max_variables {
        return Err(ExactError::TooManyVariables {
            count: variable_count,
            max: config.max_variables,
        });
    }

    let total_assignments = 1usize
        .checked_shl(variable_count as u32)
        .ok_or(ExactError::TooManyVariables {
            count: variable_count,
            max: usize::BITS as usize - 1,
        })?;

    let indices: HashMap<String, usize> = variable_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index))
        .collect();

    let producers = build_producer_map(graph)?;

    let mut weighted_assignments = Vec::with_capacity(total_assignments);
    let mut marginal_numerators = vec![0.0; variable_count];
    let mut partition_function = 0.0;

    for mask in 0..total_assignments {
        let values: Vec<bool> = (0..variable_count)
            .map(|index| ((mask >> index) & 1) == 1)
            .collect();

        let weight = assignment_weight(
            graph,
            &config,
            &producers,
            &indices,
            &values,
        )?;

        partition_function += weight;

        for (index, value) in values.iter().enumerate() {
            if *value {
                marginal_numerators[index] += weight;
            }
        }

        if config.retain_assignments {
            weighted_assignments.push((values, weight));
        }
    }

    if !partition_function.is_finite() || partition_function <= 0.0 {
        return Err(ExactError::ZeroPartition);
    }

    let marginals = variable_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                id.clone(),
                marginal_numerators[index] / partition_function,
            )
        })
        .collect();

    let assignments = weighted_assignments
        .into_iter()
        .map(|(values, weight)| {
            let value_map = variable_ids
                .iter()
                .enumerate()
                .map(|(index, id)| (id.clone(), values[index]))
                .collect();

            ExactAssignment {
                values: value_map,
                unnormalized_weight: weight,
                posterior: weight / partition_function,
            }
        })
        .collect();

    Ok(ExactResult {
        variable_ids,
        partition_function,
        marginals,
        assignments,
    })
}