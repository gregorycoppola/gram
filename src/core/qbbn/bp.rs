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
        let neg_str = if var.negated { " [negated]" } else { "" };
        let formula_str = var.formula.as_ref().map(|f| format!(" = {}", f)).unwrap_or_default();
        println!("    {}: {:?}{}{}{}", id, var.node_type, formula_str, ev_str, neg_str);
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

/// Compute P(p=1 | assignment) using log-linear model.
fn cpt_prob_true(score_pos: f64, score_neg: f64) -> f64 {
    let psi_1 = score_pos.exp();
    let psi_0 = score_neg.exp();
    if psi_1.is_infinite() && psi_0.is_infinite() {
        0.5
    } else if psi_1.is_infinite() {
        1.0
    } else if psi_0.is_infinite() {
        0.0
    } else {
        psi_1 / (psi_1 + psi_0)
    }
}

/// Run loopy belief propagation with CPT enumeration for OR factors.
pub fn belief_propagation(
    graph: &mut QBBNGraph,
    iterations: usize,
    damping: f64,
    tolerance: f64,
    debug: bool,
) -> BPTrace {
    if debug {
        println!("=== Belief Propagation Debug (CPT enumeration) ===");
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

        if debug {
            println!("-- Forward (π) --");
        }

        // AND factors: propositions → groups
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::And) {
            let prob_all_true: f64 = factor.input_ids.iter().map(|p_id| pi[p_id][1]).product();
            let g_id = &factor.output_id;
            if !graph.variables[g_id].is_evidence {
                if debug {
                    println!("  AND {}: inputs={:?} -> {}  π(g=1) = product = {:.4}",
                        factor.id, factor.input_ids, g_id, prob_all_true);
                }
                pi.insert(g_id.clone(), [1.0 - prob_all_true, prob_all_true]);
            }
        }

        // OR factors: groups → propositions (CPT enumeration)
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            if graph.variables[p_id].is_evidence {
                continue;
            }
            let n = factor.input_ids.len();
            let mut prob_true = 0.0;

            for assignment in 0..(1 << n) {
                let mut prob_assignment = 1.0;
                let mut score_pos = 0.0;
                let mut score_neg = 0.0;

                for i in 0..n {
                    let g_id = &factor.input_ids[i];
                    let g = &graph.variables[g_id];
                    let is_active = (assignment >> i) & 1 == 1;

                    if is_active {
                        prob_assignment *= pi[g_id][1];
                        if let Some(rule_id) = &g.rule_id {
                            if let Some(rule) = graph.rules.get(rule_id) {
                                if g.negated {
                                    score_neg += rule.weight;
                                } else {
                                    score_pos += rule.weight;
                                }
                            }
                        }
                    } else {
                        prob_assignment *= pi[g_id][0];
                    }
                }

                let prob_true_given_assignment = cpt_prob_true(score_pos, score_neg);
                prob_true += prob_true_given_assignment * prob_assignment;
            }

            if debug {
                println!("  OR {}: inputs={:?} -> {}  π(p=1)={:.4} (CPT over {} assignments)",
                    factor.id, factor.input_ids, p_id, prob_true, 1 << n);
            }
            pi.insert(p_id.clone(), [1.0 - prob_true, prob_true]);
        }

        if debug {
            println!("-- Backward (λ) --");
        }

        // OR factors backward: propositions → groups (CPT enumeration)
        for factor in graph.factors.values().filter(|f| f.factor_type == FactorType::Or) {
            let p_id = &factor.output_id;
            let lam_p = lam[p_id];
            let n = factor.input_ids.len();

            for k in 0..n {
                let g_id = &factor.input_ids[k];
                if graph.variables[g_id].is_evidence {
                    continue;
                }

                let mut prob_true_given_g0 = 0.0;
                let mut prob_true_given_g1 = 0.0;
                let mut prob_g0 = 0.0;
                let mut prob_g1 = 0.0;

                for assignment in 0..(1 << n) {
                    let is_gk_active = (assignment >> k) & 1 == 1;

                    let mut prob_assignment = 1.0;
                    let mut score_pos = 0.0;
                    let mut score_neg = 0.0;

                    for i in 0..n {
                        if i == k { continue; }
                        let g_id_i = &factor.input_ids[i];
                        let g_i = &graph.variables[g_id_i];
                        let is_active = (assignment >> i) & 1 == 1;

                        if is_active {
                            prob_assignment *= pi[g_id_i][1];
                            if let Some(rule_id) = &g_i.rule_id {
                                if let Some(rule) = graph.rules.get(rule_id) {
                                    if g_i.negated {
                                        score_neg += rule.weight;
                                    } else {
                                        score_pos += rule.weight;
                                    }
                                }
                            }
                        } else {
                            prob_assignment *= pi[g_id_i][0];
                        }
                    }

                    let g = &graph.variables[g_id];
                    let w = if let Some(rule_id) = &g.rule_id {
                        graph.rules.get(rule_id).map(|r| r.weight).unwrap_or(0.0)
                    } else {
                        0.0
                    };

                    let prob_true_g0 = cpt_prob_true(score_pos, score_neg);
                    let prob_true_g1 = cpt_prob_true(
                        score_pos + if g.negated { 0.0 } else { w },
                        score_neg + if g.negated { w } else { 0.0 },
                    );

                    if is_gk_active {
                        prob_g1 += prob_assignment;
                        prob_true_given_g1 += prob_true_g1 * prob_assignment;
                    } else {
                        prob_g0 += prob_assignment;
                        prob_true_given_g0 += prob_true_g0 * prob_assignment;
                    }
                }

                if prob_g0 > 0.0 {
                    prob_true_given_g0 /= prob_g0;
                }
                if prob_g1 > 0.0 {
                    prob_true_given_g1 /= prob_g1;
                }

                let lam_g_0 = lam_p[0] * (1.0 - prob_true_given_g0) + lam_p[1] * prob_true_given_g0;
                let lam_g_1 = lam_p[0] * (1.0 - prob_true_given_g1) + lam_p[1] * prob_true_given_g1;

                if debug {
                    println!("  OR {} backward: {} -> {}  λ({}) = [{:.4}, {:.4}]  (p1|0={:.4}, p1|1={:.4})",
                        factor.id, g_id, p_id, g_id, lam_g_0, lam_g_1, prob_true_given_g0, prob_true_given_g1);
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