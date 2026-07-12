use std::collections::HashMap;

use super::factor_graph::{FactorType, NodeType, QBBNGraph};

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

/// Run loopy belief propagation on a QBBN graph.
/// π = forward messages (causes → effects)
/// λ = backward messages (effects → causes)
pub fn belief_propagation(
    graph: &mut QBBNGraph,
    iterations: usize,
    damping: f64,
    tolerance: f64,
) -> BPTrace {
    let mut trace = BPTrace::new();

    // Initialize π and λ for all variables
    let mut pi: HashMap<String, [f64; 2]> = HashMap::new();
    let mut lam: HashMap<String, [f64; 2]> = HashMap::new();

    for (id, var) in &graph.variables {
        if var.is_evidence {
            if var.evidence_value == Some(true) {
                pi.insert(id.clone(), [0.0, 1.0]);
                lam.insert(id.clone(), [0.0, 1.0]);
            } else {
                pi.insert(id.clone(), [1.0, 0.0]);
                lam.insert(id.clone(), [1.0, 0.0]);
            }
        } else {
            pi.insert(id.clone(), [0.5, 0.5]);
            lam.insert(id.clone(), [1.0, 1.0]);
        }
    }

    let compute_belief =
        |var_id: &str, pi: &HashMap<String, [f64; 2]>, lam: &HashMap<String, [f64; 2]>| {
            let p0 = pi[var_id][0] * lam[var_id][0];
            let p1 = pi[var_id][1] * lam[var_id][1];
            let total = p0 + p1;
            if total > 0.0 {
                p1 / total
            } else {
                0.5
            }
        };

    // Record initial beliefs
    let initial: HashMap<String, f64> = graph
        .variables
        .keys()
        .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
        .collect();
    trace.iterations.push(initial);

    for _ in 0..iterations {
        let old_beliefs: HashMap<String, f64> = graph
            .variables
            .keys()
            .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
            .collect();

        // === FORWARD PASS (π) ===

        // 1. AND factors: propositions → groups
        // π(g=1) = Π_i π(p_i=1)
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::And) {
            let prob_all_true: f64 = factor.input_ids.iter().map(|p_id| pi[p_id][1]).product();
            let g_id = &factor.output_id;
            if !graph.variables[g_id].is_evidence {
                pi.insert(g_id.clone(), [1.0 - prob_all_true, prob_all_true]);
            }
        }

        // 2. OR factors: groups → propositions (deterministic, maximum entropy)
        // π(p=1) = 1 - Π_i (1 - π(g_i=1))
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            if graph.variables[p_id].is_evidence {
                continue;
            }
            let prob_all_false: f64 = factor
                .input_ids
                .iter()
                .map(|g_id| 1.0 - pi[g_id][1])
                .product();
            let prob_true = 1.0 - prob_all_false;
            pi.insert(p_id.clone(), [1.0 - prob_true, prob_true]);
        }

        // 3. NEG factors: bidirectional π swap
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Neg) {
            let pos_id = &factor.input_ids[0];
            let neg_id = &factor.output_id;
            if graph.variables[neg_id].is_evidence && !graph.variables[pos_id].is_evidence {
                pi.insert(pos_id.clone(), [pi[neg_id][1], pi[neg_id][0]]);
            } else if graph.variables[pos_id].is_evidence && !graph.variables[neg_id].is_evidence {
                pi.insert(neg_id.clone(), [pi[pos_id][1], pi[pos_id][0]]);
            }
        }

        // === BACKWARD PASS (λ) ===

        // 4. OR factors backward: propositions → groups
        // λ(g_i=0) = λ(p=0) * Π_{j≠i}(1-π(g_j=1)) + λ(p=1) * (1 - Π_{j≠i}(1-π(g_j=1)))
        // λ(g_i=1) = λ(p=1)
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            let lam_p = lam[p_id];
            for g_id in &factor.input_ids {
                if graph.variables[g_id].is_evidence {
                    continue;
                }
                let other_prob_false: f64 = factor
                    .input_ids
                    .iter()
                    .filter(|id| *id != g_id)
                    .map(|j_id| 1.0 - pi[j_id][1])
                    .product();
                let lam_g_0 = lam_p[0] * other_prob_false + lam_p[1] * (1.0 - other_prob_false);
                let lam_g_1 = lam_p[1];
                lam.insert(g_id.clone(), [lam_g_0, lam_g_1]);
            }
        }

        // 5. AND factors backward: groups → propositions
        // λ(p_i=1) = (Π_{j≠i} π(p_j=1)) * λ(g=1) + (1 - Π_{j≠i} π(p_j=1)) * λ(g=0)
        // λ(p_i=0) = λ(g=0)
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::And) {
            let g_id = &factor.output_id;
            let lam_g = lam[g_id];
            for (i, p_id) in factor.input_ids.iter().enumerate() {
                if graph.variables[p_id].is_evidence {
                    continue;
                }
                let other_prob_true: f64 = factor
                    .input_ids
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, j_id)| pi[j_id][1])
                    .product();
                let lam_p_1 = other_prob_true * lam_g[1] + (1.0 - other_prob_true) * lam_g[0];
                let lam_p_0 = lam_g[0];
                lam.insert(p_id.clone(), [lam_p_0, lam_p_1]);
            }
        }

        // 6. NEG factors backward: λ swap
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Neg) {
            let pos_id = &factor.input_ids[0];
            let neg_id = &factor.output_id;
            if !graph.variables[pos_id].is_evidence {
                let lam_neg = lam[neg_id];
                lam.insert(pos_id.clone(), [lam_neg[1], lam_neg[0]]);
            }
            if !graph.variables[neg_id].is_evidence {
                let lam_pos = lam[pos_id];
                lam.insert(neg_id.clone(), [lam_pos[1], lam_pos[0]]);
            }
        }

        // === UPDATE BELIEFS ===
        for (id, var) in graph.variables.iter_mut() {
            if var.is_evidence {
                continue;
            }
            let p0 = pi[id][0] * lam[id][0];
            let p1 = pi[id][1] * lam[id][1];
            let total = p0 + p1;
            let prob = if total > 0.0 { p1 / total } else { 0.5 };
            let old_prob = old_beliefs[id];
            let new_prob = damping * old_prob + (1.0 - damping) * prob;
            var.belief = [1.0 - new_prob, new_prob];
            pi.insert(id.clone(), [1.0 - new_prob, new_prob]);
            lam.insert(id.clone(), [1.0, 1.0]);
        }

        // Record
        let new_beliefs: HashMap<String, f64> = graph
            .variables
            .keys()
            .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
            .collect();
        trace.iterations.push(new_beliefs.clone());

        // Check convergence
        let max_diff: f64 = graph
            .variables
            .keys()
            .map(|id| (new_beliefs[id] - old_beliefs[id]).abs())
            .fold(0.0, f64::max);

        if max_diff < tolerance {
            break;
        }
    }

    trace
}