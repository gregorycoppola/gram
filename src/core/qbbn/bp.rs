use std::collections::HashMap;

use super::factor_graph::{FactorType, QBBNGraph};

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

fn print_graph(graph: &QBBNGraph) {
    println!("  === Graph Structure ===");
    println!("  Variables ({}):", graph.variables.len());
    for (id, var) in &graph.variables {
        let ev_str = if var.is_evidence {
            format!(" [evidence={}]", var.evidence_value.unwrap_or(false))
        } else {
            String::new()
        };
        let formula_str = var.formula.as_ref().map(|f| format!(" = {}", f)).unwrap_or_default();
        println!("    {}: {:?}{}{}", id, var.node_type, formula_str, ev_str);
    }
    println!("  Factors ({}):", graph.factors.len());
    for (id, f) in &graph.factors {
        println!("    {}: {:?}  inputs={:?} -> output={}", id, f.factor_type, f.input_ids, f.output_id);
    }
    println!("  Formula map:");
    for (formula, id) in &graph.formula_to_id {
        println!("    '{}' -> {}", formula, id);
    }
    println!();
}

fn print_beliefs(pi: &HashMap<String, [f64; 2]>, lam: &HashMap<String, [f64; 2]>, graph: &QBBNGraph) {
    println!("  --- Beliefs ---");
    for (id, var) in &graph.variables {
        let p0 = pi[id][0] * lam[id][0];
        let p1 = pi[id][1] * lam[id][1];
        let total = p0 + p1;
        let prob = if total > 0.0 { p1 / total } else { 0.5 };
        let formula_str = var.formula.as_ref().map(|f| format!(" ({})", f)).unwrap_or_default();
        println!("    {}{}: π=[{:.4}, {:.4}] λ=[{:.4}, {:.4}] belief={:.4}",
            id, formula_str, pi[id][0], pi[id][1], lam[id][0], lam[id][1], prob);
    }
    println!();
}

/// Run loopy belief propagation on a QBBN graph.
pub fn belief_propagation(
    graph: &mut QBBNGraph,
    iterations: usize,
    damping: f64,
    tolerance: f64,
    debug: bool,
) -> BPTrace {
    if debug {
        println!("=== Belief Propagation Debug ===");
        print_graph(graph);
    }

    let mut trace = BPTrace::new();

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

    let initial: HashMap<String, f64> = graph
        .variables
        .keys()
        .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
        .collect();
    trace.iterations.push(initial);

    if debug {
        println!("--- Initial beliefs ---");
        print_beliefs(&pi, &lam, graph);
    }

    for iter in 0..iterations {
        if debug {
            println!("=== Iteration {} ===", iter + 1);
        }

        let old_beliefs: HashMap<String, f64> = graph
            .variables
            .keys()
            .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
            .collect();

        // === FORWARD PASS (π) ===

        if debug {
            println!("-- Forward (π) --");
        }

        // AND factors: propositions → groups
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::And) {
            let prob_all_true: f64 = factor.input_ids.iter().map(|p_id| pi[p_id][1]).product();
            let g_id = &factor.output_id;
            if !graph.variables[g_id].is_evidence {
                if debug {
                    println!("  AND {}: inputs={:?} -> {}  π(g=1) = product of inputs = {:.4}",
                        factor.id, factor.input_ids, g_id, prob_all_true);
                }
                pi.insert(g_id.clone(), [1.0 - prob_all_true, prob_all_true]);
            }
        }

        // OR factors: groups → propositions (weighted maximum entropy)
        // P(p=1) = 1 - Π_i (1 - w_i * π(g_i=1))
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            if graph.variables[p_id].is_evidence {
                continue;
            }
            let mut prob_all_false: f64 = 1.0;
            for g_id in &factor.input_ids {
                let pi_g = pi[g_id][1];
                if let Some(rule_id) = graph.variables[g_id].rule_id.clone() {
                    if let Some(rule) = graph.rules.get(&rule_id) {
                        let w = rule.weight.min(1.0).max(0.0);
                        prob_all_false *= 1.0 - w * pi_g;
                    }
                }
            }
            let prob_true = 1.0 - prob_all_false;
            if debug {
                println!("  OR {}: inputs={:?} -> {}  π(p=1) = 1 - product(1-w*π(g)) = 1 - {:.4} = {:.4}",
                    factor.id, factor.input_ids, p_id, prob_all_false, prob_true);
            }
            pi.insert(p_id.clone(), [1.0 - prob_true, prob_true]);
        }

        // NEG factors: enforce pos + neg = 1
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Neg) {
            let pos_id = &factor.input_ids[0];
            let neg_id = &factor.output_id;
            if graph.variables[neg_id].is_evidence && !graph.variables[pos_id].is_evidence {
                if debug {
                    println!("  NEG {}: {} -> {}  π({}) = [π({})[1], π({})[0]] = [{:.4}, {:.4}]",
                        factor.id, pos_id, neg_id, pos_id, neg_id, neg_id, pi[neg_id][1], pi[neg_id][0]);
                }
                pi.insert(pos_id.clone(), [pi[neg_id][1], pi[neg_id][0]]);
            } else if graph.variables[pos_id].is_evidence && !graph.variables[neg_id].is_evidence {
                if debug {
                    println!("  NEG {}: {} -> {}  π({}) = [π({})[1], π({})[0]] = [{:.4}, {:.4}]",
                        factor.id, pos_id, neg_id, neg_id, pos_id, pos_id, pi[pos_id][1], pi[pos_id][0]);
                }
                pi.insert(neg_id.clone(), [pi[pos_id][1], pi[pos_id][0]]);
            } else if !graph.variables[pos_id].is_evidence && !graph.variables[neg_id].is_evidence {
                let pos_prob = pi[pos_id][1];
                let neg_prob = pi[neg_id][1];
                let pos_new = (pos_prob + (1.0 - neg_prob)) / 2.0;
                let neg_new = (neg_prob + (1.0 - pos_prob)) / 2.0;
                if debug {
                    println!("  NEG {}: {} <-> {}  project onto pos+neg=1: pos={:.4}-> {:.4}, neg={:.4}-> {:.4}",
                        factor.id, pos_id, neg_id, pos_prob, pos_new, neg_prob, neg_new);
                }
                pi.insert(pos_id.clone(), [1.0 - pos_new, pos_new]);
                pi.insert(neg_id.clone(), [neg_new, 1.0 - neg_new]);
            }
        }

        // === BACKWARD PASS (λ) ===

        if debug {
            println!("-- Backward (λ) --");
        }

        // OR factors backward: propositions → groups
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            let lam_p = lam[p_id];
            for g_id in &factor.input_ids {
                if graph.variables[g_id].is_evidence {
                    continue;
                }
                let mut prob_all_false_without: f64 = 1.0;
                let mut w_i: f64 = 0.0;
                for (_j, other_g_id) in factor.input_ids.iter().enumerate() {
                    if other_g_id == g_id {
                        if let Some(rule_id) = graph.variables[other_g_id].rule_id.clone() {
                            if let Some(rule) = graph.rules.get(&rule_id) {
                                w_i = rule.weight.min(1.0).max(0.0);
                            }
                        }
                    } else {
                        let pi_gj = pi[other_g_id][1];
                        if let Some(rule_id) = graph.variables[other_g_id].rule_id.clone() {
                            if let Some(rule) = graph.rules.get(&rule_id) {
                                let w_j = rule.weight.min(1.0).max(0.0);
                                prob_all_false_without *= 1.0 - w_j * pi_gj;
                            }
                        }
                    }
                }
                let p_given_g0 = 1.0 - prob_all_false_without;
                let p_given_g1 = 1.0 - (1.0 - w_i) * prob_all_false_without;
                let lam_g_0 = lam_p[0] * (1.0 - p_given_g0) + lam_p[1] * p_given_g0;
                let lam_g_1 = lam_p[0] * (1.0 - p_given_g1) + lam_p[1] * p_given_g1;
                if debug {
                    println!("  OR {} backward: {} -> {}  λ({}) = [{:.4}, {:.4}]  (p_g0={:.4}, p_g1={:.4})",
                        factor.id, g_id, p_id, g_id, lam_g_0, lam_g_1, p_given_g0, p_given_g1);
                }
                lam.insert(g_id.clone(), [lam_g_0, lam_g_1]);
            }
        }

        // AND factors backward: groups → propositions
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
                if debug {
                    println!("  AND {} backward: {} -> {}  λ({}) = [{:.4}, {:.4}]  (other_prob_true={:.4})",
                        factor.id, p_id, g_id, p_id, lam_p_0, lam_p_1, other_prob_true);
                }
                lam.insert(p_id.clone(), [lam_p_0, lam_p_1]);
            }
        }

        // NEG factors backward: λ swap
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Neg) {
            let pos_id = &factor.input_ids[0];
            let neg_id = &factor.output_id;
            if !graph.variables[pos_id].is_evidence {
                let lam_neg = lam[neg_id];
                if debug {
                    println!("  NEG {} backward: {} -> {}  λ({}) = [{:.4}, {:.4}]",
                        factor.id, neg_id, pos_id, pos_id, lam_neg[1], lam_neg[0]);
                }
                lam.insert(pos_id.clone(), [lam_neg[1], lam_neg[0]]);
            }
            if !graph.variables[neg_id].is_evidence {
                let lam_pos = lam[pos_id];
                if debug {
                    println!("  NEG {} backward: {} -> {}  λ({}) = [{:.4}, {:.4}]",
                        factor.id, pos_id, neg_id, neg_id, lam_pos[1], lam_pos[0]);
                }
                lam.insert(neg_id.clone(), [lam_pos[1], lam_pos[0]]);
            }
        }

        // === UPDATE BELIEFS ===
        if debug {
            println!("-- Update beliefs --");
        }

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
            if debug {
                let formula_str = var.formula.as_ref().map(|f| format!(" ({})", f)).unwrap_or_default();
                println!("  {}{}: old={:.4} -> new={:.4} (raw={:.4})",
                    id, formula_str, old_prob, new_prob, prob);
            }
        }

        let new_beliefs: HashMap<String, f64> = graph
            .variables
            .keys()
            .map(|id| (id.clone(), compute_belief(id, &pi, &lam)))
            .collect();
        trace.iterations.push(new_beliefs.clone());

        if debug {
            println!("-- End of iteration {} --", iter + 1);
            print_beliefs(&pi, &lam, graph);
        }

        let max_diff: f64 = graph
            .variables
            .keys()
            .map(|id| (new_beliefs[id] - old_beliefs[id]).abs())
            .fold(0.0, f64::max);

        if debug {
            println!("  max_diff = {:.6} (tolerance = {:.6})\n", max_diff, tolerance);
        }

        if max_diff < tolerance {
            if debug {
                println!("=== Converged after {} iterations ===\n", iter + 1);
            }
            break;
        }
    }

    trace
}
