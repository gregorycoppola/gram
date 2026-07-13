use gram::core::qbbn::{
    belief_propagation, exact_inference, ExactConfig, ExactResult, NodeType,
    QBBNGraph,
};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "got {actual:.12}, expected {expected:.12} ± {tolerance}"
    );
}

fn exact(graph: &QBBNGraph) -> ExactResult {
    exact_inference(graph, ExactConfig::default())
        .expect("exact inference should succeed")
}

fn add_rule(
    graph: &mut QBBNGraph,
    premises: &[&str],
    conclusion: &str,
    weight: f64,
) {
    let premise_formulas: Vec<String> =
        premises.iter().map(|p| (*p).to_string()).collect();

    let rule_id = graph.add_rule(
        premise_formulas.clone(),
        conclusion.to_string(),
        Vec::new(),
        weight,
    );

    graph.add_grounded_rule(
        premise_formulas,
        conclusion.to_string(),
        rule_id,
    );
}

fn single_rule_graph(
    premise: &str,
    conclusion: &str,
    weight: f64,
) -> QBBNGraph {
    let mut graph = QBBNGraph::new();
    add_rule(&mut graph, &[premise], conclusion, weight);
    graph.build_or_factors();
    graph
}

#[test]
fn soft_evidence_on_uniform_root_produces_requested_posterior() {
    let mut graph = QBBNGraph::new();
    graph.add_proposition("a");
    graph.set_evidence("a", 0.7);

    let result = exact(&graph);

    assert_close(
        result.prob_formula(&graph, "a").unwrap(),
        0.7,
        1e-12,
    );
}

#[test]
fn exact_forward_probability_matches_hand_calculation() {
    // With w = ln(3):
    //
    // P(B=1 | A=0) = 0.5
    // P(B=1 | A=1) = 0.75
    //
    // Given P(A=1)=0.7:
    //
    // P(B=1) = 0.3(0.5) + 0.7(0.75) = 0.675.
    let mut graph = single_rule_graph("a", "b", 3.0_f64.ln());
    graph.set_evidence("a", 0.7);

    let result = exact(&graph);

    assert_close(
        result.prob_formula(&graph, "a").unwrap(),
        0.7,
        1e-12,
    );
    assert_close(
        result.prob_formula(&graph, "b").unwrap(),
        0.675,
        1e-12,
    );
}

#[test]
fn exact_backward_conditioning_matches_bayes_rule() {
    // Prior P(A=1) = 0.5.
    //
    // P(B=1 | A=1) = 0.75
    // P(B=1 | A=0) = 0.50
    //
    // P(A=1 | B=1)
    //   = 0.5(0.75) / [0.5(0.75) + 0.5(0.50)]
    //   = 0.6.
    let mut graph = single_rule_graph("a", "b", 3.0_f64.ln());
    graph.set_evidence("b", 1.0);

    let result = exact(&graph);

    assert_close(
        result.prob_formula(&graph, "a").unwrap(),
        0.6,
        1e-12,
    );
}

#[test]
fn exact_combines_evidence_from_two_children() {
    // P(B=1 | A=1) = P(C=1 | A=1) = 0.75
    // P(B=1 | A=0) = P(C=1 | A=0) = 0.50
    //
    // P(A=1 | B=1,C=1)
    //   = 0.75² / (0.75² + 0.50²)
    //   = 9/13.
    let mut graph = QBBNGraph::new();
    add_rule(&mut graph, &["a"], "b", 3.0_f64.ln());
    add_rule(&mut graph, &["a"], "c", 3.0_f64.ln());
    graph.build_or_factors();

    graph.set_evidence("b", 1.0);
    graph.set_evidence("c", 1.0);

    let result = exact(&graph);

    assert_close(
        result.prob_formula(&graph, "a").unwrap(),
        9.0 / 13.0,
        1e-12,
    );
}

#[test]
fn exact_handles_negated_premise_in_backward_direction() {
    // not A -> B
    //
    // P(B=1 | A=0) = 0.75
    // P(B=1 | A=1) = 0.50
    //
    // P(A=1 | B=1)
    //   = 0.5(0.50) / [0.5(0.50) + 0.5(0.75)]
    //   = 0.4.
    let mut graph =
        single_rule_graph("not a", "b", 3.0_f64.ln());
    graph.set_evidence("b", 1.0);

    let result = exact(&graph);

    assert_close(
        result.prob_formula(&graph, "a").unwrap(),
        0.4,
        1e-12,
    );
    assert_close(
        result.prob_formula(&graph, "not a").unwrap(),
        0.6,
        1e-12,
    );
}

#[test]
fn and_group_is_deterministic_in_every_nonzero_assignment() {
    let mut graph = QBBNGraph::new();
    add_rule(&mut graph, &["a", "c"], "b", 3.0_f64.ln());
    graph.build_or_factors();

    let result = exact(&graph);

    let a_id = graph.formula_to_id["a"].clone();
    let c_id = graph.formula_to_id["c"].clone();
    let group_id = graph
        .variables
        .values()
        .find(|variable| variable.node_type == NodeType::Group)
        .expect("graph should contain a group")
        .id
        .clone();

    for assignment in result
        .assignments
        .iter()
        .filter(|assignment| assignment.posterior > 0.0)
    {
        let a = assignment.values[&a_id];
        let c = assignment.values[&c_id];
        let group = assignment.values[&group_id];

        assert_eq!(
            group,
            a && c,
            "nonzero assignment violates deterministic AND: {:?}",
            assignment.values
        );
    }
}

#[test]
fn exact_assignment_probabilities_normalize_to_one() {
    let mut graph = QBBNGraph::new();
    add_rule(&mut graph, &["a"], "b", 3.0_f64.ln());
    add_rule(&mut graph, &["b"], "c", 2.0_f64.ln());
    graph.build_or_factors();
    graph.set_evidence("c", 0.8);

    let result = exact(&graph);
    let total: f64 =
        result.assignments.iter().map(|a| a.posterior).sum();

    assert_close(total, 1.0, 1e-12);

    for probability in result.marginals.values() {
        assert!((0.0..=1.0).contains(probability));
    }
}

#[test]
fn current_bp_matches_exact_for_simple_forward_tree() {
    let mut graph = single_rule_graph("a", "b", 3.0_f64.ln());
    graph.set_evidence("a", 0.7);

    let exact_result = exact(&graph);
    let exact_b = exact_result.prob_formula(&graph, "b").unwrap();

    let mut bp_graph = graph.clone();
    belief_propagation(
        &mut bp_graph,
        100,
        0.5,
        1e-10,
        false,
    );

    assert_close(bp_graph.prob("b"), exact_b, 1e-6);
}

#[test]
#[ignore = "documents the backward-message bug in the current BP implementation"]
fn bp_should_match_exact_for_downstream_evidence_on_tree() {
    let mut graph = single_rule_graph("a", "b", 3.0_f64.ln());
    graph.set_evidence("b", 1.0);

    let exact_result = exact(&graph);
    let exact_a = exact_result.prob_formula(&graph, "a").unwrap();

    let mut bp_graph = graph.clone();
    belief_propagation(
        &mut bp_graph,
        100,
        0.5,
        1e-10,
        false,
    );

    assert_close(bp_graph.prob("a"), exact_a, 1e-6);
}