use std::collections::{HashMap, HashSet};

use super::factor_graph::{Factor, FactorType, NodeType, QBBNGraph};

type Message = [f64; 2];
type MessageKey = (String, String);

#[derive(Debug, Clone)]
pub struct BPTrace {
    pub iterations: Vec<HashMap<String, f64>>,
}

impl BPTrace {
    pub fn new() -> Self {
        Self {
            iterations: Vec::new(),
        }
    }
}

fn normalize(message: Message) -> Message {
    let total = message[0] + message[1];

    if total.is_finite() && total > 0.0 {
        [message[0] / total, message[1] / total]
    } else {
        [0.5, 0.5]
    }
}

fn damp(old: Message, new: Message, damping: f64) -> Message {
    let damping = damping.clamp(0.0, 1.0);

    normalize([
        damping * old[0] + (1.0 - damping) * new[0],
        damping * old[1] + (1.0 - damping) * new[1],
    ])
}

fn message_change(left: Message, right: Message) -> f64 {
    (left[0] - right[0])
        .abs()
        .max((left[1] - right[1]).abs())
}

fn factor_variable_ids(factor: &Factor) -> Vec<String> {
    let mut ids = factor.input_ids.clone();
    ids.push(factor.output_id.clone());
    ids
}

fn produced_propositions(graph: &QBBNGraph) -> HashSet<String> {
    graph
        .factors
        .values()
        .filter(|factor| factor.factor_type == FactorType::Or)
        .map(|factor| factor.output_id.clone())
        .collect()
}

/// Unary potential attached to a variable.
///
/// Source propositions receive the default Bernoulli(0.5) root prior.
/// Produced propositions and group variables receive no independent prior.
/// Evidence is represented as a unary likelihood [1-q, q].
fn unary_potential(
    graph: &QBBNGraph,
    variable_id: &str,
    produced: &HashSet<String>,
) -> Message {
    let variable = &graph.variables[variable_id];

    let mut potential = if variable.node_type == NodeType::Proposition
        && !produced.contains(variable_id)
    {
        [0.5, 0.5]
    } else {
        [1.0, 1.0]
    };

    if variable.is_evidence {
        let probability = variable
            .evidence_prob
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        potential[0] *= 1.0 - probability;
        potential[1] *= probability;
    }

    potential
}

pub fn cpt_prob_true(score_pos: f64, score_neg: f64) -> f64 {
    let max_score = score_pos.max(score_neg);

    if max_score.is_infinite() {
        if score_pos == score_neg {
            return 0.5;
        }

        return if score_pos > score_neg { 1.0 } else { 0.0 };
    }

    // Stable two-class softmax:
    //
    // exp(score_pos) /
    // [exp(score_pos) + exp(score_neg)]
    let exp_pos = (score_pos - max_score).exp();
    let exp_neg = (score_neg - max_score).exp();

    exp_pos / (exp_pos + exp_neg)
}

fn factor_potential(
    graph: &QBBNGraph,
    factor: &Factor,
    variable_ids: &[String],
    assignment: &[bool],
) -> f64 {
    let output_value = assignment[variable_ids.len() - 1];

    match factor.factor_type {
        FactorType::And => {
            let mut conjunction = true;

            for index in 0..factor.input_ids.len() {
                let raw_value = assignment[index];
                let literal_value = if factor.input_negated[index] {
                    !raw_value
                } else {
                    raw_value
                };

                conjunction &= literal_value;
            }

            if output_value == conjunction {
                1.0
            } else {
                0.0
            }
        }
        FactorType::Or => {
            let mut score_pos = 0.0;
            let mut score_neg = 0.0;

            for (index, group_id) in
                factor.input_ids.iter().enumerate()
            {
                if !assignment[index] {
                    continue;
                }

                let group = &graph.variables[group_id];

                let weight = group
                    .rule_id
                    .as_ref()
                    .and_then(|rule_id| graph.rules.get(rule_id))
                    .map(|rule| rule.weight)
                    .unwrap_or(0.0);

                if group.negated {
                    score_neg += weight;
                } else {
                    score_pos += weight;
                }
            }

            let prob_true = cpt_prob_true(score_pos, score_neg);

            if output_value {
                prob_true
            } else {
                1.0 - prob_true
            }
        }
    }
}

/// Compute one factor-to-variable sum-product message.
///
/// m(f -> x)(x) =
///     sum over assignments to N(f) \ {x}
///         psi_f(N(f)) * product m(y -> f)(y)
fn compute_factor_message(
    graph: &QBBNGraph,
    factor: &Factor,
    target_id: &str,
    variable_to_factor: &HashMap<MessageKey, Message>,
) -> Message {
    let variable_ids = factor_variable_ids(factor);

    let target_index = variable_ids
        .iter()
        .position(|id| id == target_id)
        .expect("target variable must belong to factor");

    let other_count = variable_ids.len() - 1;

    let assignment_count = 1usize
        .checked_shl(other_count as u32)
        .expect("factor is too large for exact message enumeration");

    let mut result = [0.0, 0.0];

    for target_state in 0..=1 {
        for mask in 0..assignment_count {
            let mut assignment = vec![false; variable_ids.len()];
            assignment[target_index] = target_state == 1;

            let mut other_index = 0;

            for variable_index in 0..variable_ids.len() {
                if variable_index == target_index {
                    continue;
                }

                assignment[variable_index] =
                    ((mask >> other_index) & 1) == 1;
                other_index += 1;
            }

            let potential = factor_potential(
                graph,
                factor,
                &variable_ids,
                &assignment,
            );

            if potential == 0.0 {
                continue;
            }

            let mut incoming_product = 1.0;

            for (variable_index, variable_id) in
                variable_ids.iter().enumerate()
            {
                if variable_index == target_index {
                    continue;
                }

                let key =
                    (factor.id.clone(), variable_id.clone());

                let incoming = variable_to_factor
                    .get(&key)
                    .copied()
                    .unwrap_or([0.5, 0.5]);

                let state = usize::from(assignment[variable_index]);
                incoming_product *= incoming[state];
            }

            result[target_state] += potential * incoming_product;
        }
    }

    normalize(result)
}

/// Compute one variable-to-factor cavity message.
///
/// m(x -> f)(x) =
///     unary_x(x) * product over h in N(x) \ {f} m(h -> x)(x)
fn compute_variable_message(
    graph: &QBBNGraph,
    variable_id: &str,
    target_factor_id: &str,
    factor_to_variable: &HashMap<MessageKey, Message>,
    produced: &HashSet<String>,
) -> Message {
    let mut result =
        unary_potential(graph, variable_id, produced);

    for factor_id in &graph.var_to_factors[variable_id] {
        if factor_id == target_factor_id {
            continue;
        }

        let key =
            (factor_id.clone(), variable_id.to_string());

        let incoming = factor_to_variable
            .get(&key)
            .copied()
            .unwrap_or([1.0, 1.0]);

        result[0] *= incoming[0];
        result[1] *= incoming[1];
    }

    normalize(result)
}

fn compute_belief(
    graph: &QBBNGraph,
    variable_id: &str,
    factor_to_variable: &HashMap<MessageKey, Message>,
    produced: &HashSet<String>,
) -> Message {
    let mut belief =
        unary_potential(graph, variable_id, produced);

    for factor_id in &graph.var_to_factors[variable_id] {
        let key =
            (factor_id.clone(), variable_id.to_string());

        let incoming = factor_to_variable
            .get(&key)
            .copied()
            .unwrap_or([1.0, 1.0]);

        belief[0] *= incoming[0];
        belief[1] *= incoming[1];
    }

    normalize(belief)
}

fn print_graph(graph: &QBBNGraph) {
    println!("  === Graph Structure ===");

    let mut variable_ids: Vec<_> =
        graph.variables.keys().cloned().collect();
    variable_ids.sort();

    println!("  Variables ({}):", variable_ids.len());

    for variable_id in variable_ids {
        let variable = &graph.variables[&variable_id];

        let evidence = if variable.is_evidence {
            format!(
                " [evidence={:.4}]",
                variable.evidence_prob.unwrap_or(0.5)
            )
        } else {
            String::new()
        };

        let negated = if variable.negated {
            " [negated]"
        } else {
            ""
        };

        let formula = variable
            .formula
            .as_ref()
            .map(|formula| format!(" = {}", formula))
            .unwrap_or_default();

        println!(
            "    {}: {:?}{}{}{}",
            variable_id,
            variable.node_type,
            formula,
            evidence,
            negated
        );
    }

    let mut factor_ids: Vec<_> =
        graph.factors.keys().cloned().collect();
    factor_ids.sort();

    println!("  Factors ({}):", factor_ids.len());

    for factor_id in factor_ids {
        let factor = &graph.factors[&factor_id];

        println!(
            "    {}: {:?} inputs={:?} negated={:?} -> {}",
            factor_id,
            factor.factor_type,
            factor.input_ids,
            factor.input_negated,
            factor.output_id
        );
    }

    println!();
}

fn print_beliefs(
    graph: &QBBNGraph,
    factor_to_variable: &HashMap<MessageKey, Message>,
    produced: &HashSet<String>,
) {
    let mut variable_ids: Vec<_> =
        graph.variables.keys().cloned().collect();
    variable_ids.sort();

    println!("  --- Beliefs ---");

    for variable_id in variable_ids {
        let belief = compute_belief(
            graph,
            &variable_id,
            factor_to_variable,
            produced,
        );

        let formula = graph.variables[&variable_id]
            .formula
            .as_ref()
            .map(|formula| format!(" ({})", formula))
            .unwrap_or_default();

        println!(
            "    {}{}: [{:.6}, {:.6}]",
            variable_id, formula, belief[0], belief[1]
        );
    }

    println!();
}

/// Sum-product belief propagation on the QBBN factor graph.
///
/// Messages are edge-specific. A variable-to-factor message excludes the
/// message received from that target factor, which prevents immediate
/// double-counting and gives exact marginals on tree-structured factor graphs.
///
/// On graphs containing undirected cycles, this is loopy BP: convergence and
/// exactness are not guaranteed, but its result can be compared directly with
/// `exact_inference` for small graphs.
pub fn belief_propagation(
    graph: &mut QBBNGraph,
    iterations: usize,
    damping: f64,
    tolerance: f64,
    debug: bool,
) -> BPTrace {
    if debug {
        println!("=== Belief Propagation Debug (edge sum-product) ===");
        print_graph(graph);
    }

    let produced = produced_propositions(graph);

    let mut factor_ids: Vec<String> =
        graph.factors.keys().cloned().collect();
    factor_ids.sort();

    let mut variable_ids: Vec<String> =
        graph.variables.keys().cloned().collect();
    variable_ids.sort();

    let mut factor_to_variable:
        HashMap<MessageKey, Message> = HashMap::new();

    let mut variable_to_factor:
        HashMap<MessageKey, Message> = HashMap::new();

    for factor_id in &factor_ids {
        let factor = &graph.factors[factor_id];

        for variable_id in factor_variable_ids(factor) {
            let key =
                (factor_id.clone(), variable_id.clone());

            factor_to_variable.insert(
                key.clone(),
                [0.5, 0.5],
            );

            variable_to_factor.insert(
                key,
                normalize(unary_potential(
                    graph,
                    &variable_id,
                    &produced,
                )),
            );
        }
    }

    let mut trace = BPTrace::new();

    let initial_beliefs: HashMap<String, f64> = variable_ids
        .iter()
        .map(|variable_id| {
            let belief = compute_belief(
                graph,
                variable_id,
                &factor_to_variable,
                &produced,
            );

            (variable_id.clone(), belief[1])
        })
        .collect();

    trace.iterations.push(initial_beliefs);

    if debug {
        println!("--- Initial beliefs ---");
        print_beliefs(
            graph,
            &factor_to_variable,
            &produced,
        );
    }

    for iteration in 0..iterations {
        let old_beliefs: HashMap<String, f64> = variable_ids
            .iter()
            .map(|variable_id| {
                (
                    variable_id.clone(),
                    graph.variables[variable_id].prob(),
                )
            })
            .collect();

        let mut next_factor_to_variable =
            factor_to_variable.clone();

        let mut max_message_change: f64 = 0.0;

        // Factor -> variable messages.
        for factor_id in &factor_ids {
            let factor = &graph.factors[factor_id];

            for variable_id in factor_variable_ids(factor) {
                let key =
                    (factor_id.clone(), variable_id.clone());

                let raw = compute_factor_message(
                    graph,
                    factor,
                    &variable_id,
                    &variable_to_factor,
                );

                let old = factor_to_variable[&key];
                let updated = damp(old, raw, damping);

                max_message_change = max_message_change.max(
                    message_change(old, updated),
                );

                next_factor_to_variable.insert(key, updated);
            }
        }

        // Variable -> factor cavity messages.
        let mut next_variable_to_factor =
            variable_to_factor.clone();

        for variable_id in &variable_ids {
            let mut neighbor_ids =
                graph.var_to_factors[variable_id].clone();
            neighbor_ids.sort();

            for factor_id in neighbor_ids {
                let key =
                    (factor_id.clone(), variable_id.clone());

                let raw = compute_variable_message(
                    graph,
                    variable_id,
                    &factor_id,
                    &next_factor_to_variable,
                    &produced,
                );

                let old = variable_to_factor[&key];
                let updated = damp(old, raw, damping);

                max_message_change = max_message_change.max(
                    message_change(old, updated),
                );

                next_variable_to_factor.insert(key, updated);
            }
        }

        factor_to_variable = next_factor_to_variable;
        variable_to_factor = next_variable_to_factor;

        let mut iteration_beliefs = HashMap::new();
        let mut max_belief_change: f64 = 0.0;

        for variable_id in &variable_ids {
            let belief = compute_belief(
                graph,
                variable_id,
                &factor_to_variable,
                &produced,
            );

            let previous = old_beliefs[variable_id];

            max_belief_change = max_belief_change.max(
                (belief[1] - previous).abs(),
            );

            graph
                .variables
                .get_mut(variable_id)
                .expect("variable must exist")
                .belief = belief;

            iteration_beliefs
                .insert(variable_id.clone(), belief[1]);
        }

        trace.iterations.push(iteration_beliefs);

        if debug {
            println!("=== Iteration {} ===", iteration + 1);
            println!(
                "  max message change = {:.12}",
                max_message_change
            );
            println!(
                "  max belief change  = {:.12}",
                max_belief_change
            );

            print_beliefs(
                graph,
                &factor_to_variable,
                &produced,
            );
        }

        if max_message_change < tolerance
            && max_belief_change < tolerance
        {
            if debug {
                println!(
                    "=== Converged after {} iterations ===\n",
                    iteration + 1
                );
            }

            break;
        }
    }

    trace
}