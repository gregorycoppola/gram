use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::core::qbbn::factor_graph::{
    CPTTable, GraphEdge, GraphSnapshot,
};
use crate::core::qbbn::inference::InferenceResult;

pub async fn run_fixtures(
    client: &reqwest::Client,
    base: &str,
) -> Result<()> {
    let url = format!("{}/inference/fixtures", base);
    let names: Vec<String> =
        super::get_json(client, &url).await?;

    for name in names {
        println!("  {}", name);
    }

    Ok(())
}

pub async fn run_run(
    client: &reqwest::Client,
    base: &str,
    fixture: &str,
    pretty: bool,
) -> Result<()> {
    let fixture_url =
        format!("{}/fixtures/qbbn%2F{}", base, fixture);

    let fixture_json: serde_json::Value =
        super::get_json(client, &fixture_url).await?;

    let url = format!("{}/inference/run", base);

    let result: InferenceResult =
        super::post_json(client, &url, fixture_json).await?;

    if pretty {
        print_inference_pretty(&result);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&result)?
        );
    }

    Ok(())
}

fn print_inference_pretty(result: &InferenceResult) {
    println!(
        "\n════════════════════════════════════════════════════════"
    );
    println!("  {}", result.title);
    println!(
        "════════════════════════════════════════════════════════\n"
    );

    let (
        proposition_count,
        group_count,
        and_count,
        or_count,
        _negation_count,
        evidence_count,
    ) = result.stats;

    println!(
        "  graph: {} propositions, {} groups, {} AND, {} OR, {} evidence",
        proposition_count,
        group_count,
        and_count,
        or_count,
        evidence_count
    );

    let topology_policy =
        if result.topology.bp_should_be_exact() {
            "BP must equal exact"
        } else {
            "loopy BP is approximate"
        };

    println!(
        "  topology: {} — {}",
        result.topology.kind(),
        topology_policy
    );

    println!(
        "  structure: {} component(s), {} variables, {} factors, {} edges",
        result.topology.connected_components,
        result.topology.variable_nodes,
        result.topology.factor_nodes,
        result.topology.edges
    );

    println!(
        "  converged in {} iterations",
        result.iterations
    );

    if let Some(partition) =
        result.exact_partition_function
    {
        println!(
            "  exact partition function: {:.8}",
            partition
        );
    }

    if let Some(error) = &result.exact_error {
        println!("  exact inference unavailable: {}", error);
    }

    println!("\n  ── queries ──");

    for query in &result.query_results {
        let status = if query.ok { "✓" } else { "✗" };

        let expected = match query.expected {
            Some(value) => format!(
                "  expected={:.6} ± {}",
                value,
                format_tolerance(query.tolerance)
            ),
            None => String::new(),
        };

        let exact = match query.exact_prob {
            Some(value) => format!("  exact={:.6}", value),
            None => String::new(),
        };

        let delta = match query.bp_exact_delta {
            Some(value) => format!("  Δ={:+.2e}", value),
            None => String::new(),
        };

        println!(
            "  {} P({})  BP={:.6}{}{}{}",
            status,
            query.formula,
            query.prob,
            exact,
            delta,
            expected
        );
    }

    let all_ok =
        result.query_results.iter().all(|query| query.ok);

    if all_ok {
        println!("\n  ✅ all queries passed");
    } else {
        println!("\n  ⚠️  some queries failed");
    }

    if let Some(graph) = &result.graph {
        print_graph(graph);
    }

    println!();
}

fn print_graph(graph: &GraphSnapshot) {
    println!("\n  ── formula map ──");

    for (formula, id) in &graph.formula_map {
        println!("  {} → {}", id, formula);
    }

    let or_targets: HashSet<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "or")
        .map(|edge| edge.target_id.as_str())
        .collect();

    let and_by_target: HashMap<&str, &str> = graph
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "and")
        .map(|edge| {
            (
                edge.target_id.as_str(),
                edge.id.as_str(),
            )
        })
        .collect();

    let or_by_target: HashMap<&str, &str> = graph
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "or")
        .map(|edge| {
            (
                edge.target_id.as_str(),
                edge.id.as_str(),
            )
        })
        .collect();

    let mut downstream_and:
        HashMap<&str, Vec<&str>> = HashMap::new();

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "and")
    {
        for source_id in &edge.source_ids {
            downstream_and
                .entry(source_id.as_str())
                .or_default()
                .push(edge.id.as_str());
        }
    }

    let source_propositions: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.node_type == "proposition"
                && !or_targets.contains(node.id.as_str())
        })
        .collect();

    let computed_propositions: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.node_type == "proposition"
                && or_targets.contains(node.id.as_str())
        })
        .collect();

    let groups: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.node_type == "group")
        .collect();

    println!(
        "\n  ── source propositions (no OR parent) ──"
    );

    for node in &source_propositions {
        let kind =
            if node.is_evidence { "📡" } else { "  " };

        let formula =
            node.formula.as_deref().unwrap_or("—");

        let probability = if node.is_evidence {
            format!(
                "evidence={:.4}",
                node.evidence_prob.unwrap_or(0.5)
            )
        } else {
            format!("belief={:.4}", node.belief)
        };

        let downstream =
            format_downstream_and(
                &downstream_and,
                node.id.as_str(),
            );

        println!(
            "  {} {}  {:<32}  {}{}",
            kind,
            node.id,
            formula,
            probability,
            downstream
        );
    }

    println!(
        "\n  ── groups (AND output, signed OR input) ──"
    );

    for node in &groups {
        let polarity =
            if node.negated { "negative" } else { "positive" };

        let and_factor = and_by_target
            .get(node.id.as_str())
            .copied()
            .unwrap_or("?");

        let conclusion =
            node.conclusion_id.as_deref().unwrap_or("—");

        let or_factor = or_by_target
            .get(conclusion)
            .map(|id| format!(" → {}", id))
            .unwrap_or_default();

        let rule =
            node.rule_id.as_deref().unwrap_or("?");

        println!(
            "  {}  polarity={:<8}  and={:<6}  rule={:<6}  conclusion={}{}",
            node.id,
            polarity,
            and_factor,
            rule,
            conclusion,
            or_factor
        );
    }

    println!(
        "\n  ── computed propositions (OR output) ──"
    );

    for node in &computed_propositions {
        let formula =
            node.formula.as_deref().unwrap_or("—");

        let or_factor = or_by_target
            .get(node.id.as_str())
            .copied()
            .unwrap_or("?");

        let downstream =
            format_downstream_and(
                &downstream_and,
                node.id.as_str(),
            );

        println!(
            "  {}  {:<32}  or={:<6}  belief={:.4}{}",
            node.id,
            formula,
            or_factor,
            node.belief,
            downstream
        );
    }

    println!("\n  ── rule flows ──");

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "and")
    {
        print_rule_flow(
            graph,
            edge,
            &or_by_target,
        );
    }

    println!("\n  ── factors ──");

    for edge in &graph.edges {
        let inputs =
            display_edge_inputs(graph, edge);

        println!(
            "  {:>3} {:<6}  {} → {}",
            edge.edge_type.to_uppercase(),
            edge.id,
            inputs.join(", "),
            edge.target_id
        );
    }

    println!("\n  ── rules ──");

    for rule in &graph.rules {
        println!(
            "  {:<6}  w={:<5.1}  {} → {}",
            rule.id,
            rule.weight,
            rule.premise_patterns.join(" ∧ "),
            rule.conclusion_pattern
        );
    }

    println!("\n  ── CPT tables ──");

    for cpt in &graph.cpt_tables {
        print_cpt(graph, cpt);
    }
}

fn print_rule_flow(
    graph: &GraphSnapshot,
    edge: &GraphEdge,
    or_by_target: &HashMap<&str, &str>,
) {
    let input_literals: Vec<String> = edge
        .source_ids
        .iter()
        .enumerate()
        .map(|(index, source_id)| {
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == *source_id);

            let belief =
                node.map(|node| node.belief).unwrap_or(0.5);

            let negated = edge
                .input_negated
                .get(index)
                .copied()
                .unwrap_or(false);

            let literal_probability =
                if negated { 1.0 - belief } else { belief };

            if negated {
                format!(
                    "¬{}({:.4})",
                    source_id,
                    literal_probability
                )
            } else {
                format!(
                    "{}({:.4})",
                    source_id,
                    literal_probability
                )
            }
        })
        .collect();

    let group = graph
        .nodes
        .iter()
        .find(|node| node.id == edge.target_id);

    let Some(group) = group else {
        println!(
            "  [{}] → {} → {}",
            input_literals.join(" ∧ "),
            edge.id,
            edge.target_id
        );
        return;
    };

    let conclusion =
        group.conclusion_id.as_deref().unwrap_or("?");

    let or_factor = or_by_target
        .get(conclusion)
        .copied()
        .unwrap_or("?");

    let weight = group
        .rule_id
        .as_ref()
        .and_then(|rule_id| {
            graph
                .rules
                .iter()
                .find(|rule| {
                    rule.id.as_str()
                        == rule_id.as_str()
                })
        })
        .map(|rule| rule.weight)
        .unwrap_or(0.0);

    let signed_group = if group.negated {
        format!("{}[negative,w={:.1}]", group.id, weight)
    } else {
        format!("{}[positive,w={:.1}]", group.id, weight)
    };

    println!(
        "  [{}] → {} → {} → {} → {}",
        input_literals.join(" ∧ "),
        edge.id,
        signed_group,
        or_factor,
        conclusion
    );
}

fn print_cpt(
    graph: &GraphSnapshot,
    cpt: &CPTTable,
) {
    let edge = graph
        .edges
        .iter()
        .find(|edge| edge.id == cpt.factor_id);

    let display_inputs: Vec<String> = cpt
        .inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let negated = edge
                .and_then(|edge| {
                    edge.input_negated.get(index)
                })
                .copied()
                .unwrap_or(false);

            if negated {
                format!("not {}", input)
            } else {
                input.clone()
            }
        })
        .collect();

    println!(
        "  {:>3} {}: {} → {}",
        cpt.factor_type.to_uppercase(),
        cpt.factor_id,
        display_inputs.join(" ∧ "),
        cpt.output
    );

    if cpt.truncated {
        println!(
            "    (truncated — {} inputs)",
            cpt.inputs.len()
        );
    } else if cpt.factor_type == "and" {
        println!(
            "    deterministic conjunction: output is true iff every displayed literal is true"
        );
    } else if !cpt.rows.is_empty() {
        let header =
            display_inputs.join(" | ");

        println!(
            "    {:<24} | P(out=1)",
            header
        );

        println!(
            "    {}",
            "─".repeat(header.len().max(24) + 13)
        );

        for row in &cpt.rows {
            let values: Vec<String> = row
                .assignment
                .iter()
                .map(|value| {
                    if *value {
                        "T".to_string()
                    } else {
                        "F".to_string()
                    }
                })
                .collect();

            println!(
                "    {:<24} | {:.4}",
                values.join(" | "),
                row.prob_true
            );
        }
    }

    println!();
}

fn display_edge_inputs(
    graph: &GraphSnapshot,
    edge: &GraphEdge,
) -> Vec<String> {
    edge.source_ids
        .iter()
        .enumerate()
        .map(|(index, source_id)| {
            let formula = graph
                .nodes
                .iter()
                .find(|node| node.id == *source_id)
                .and_then(|node| node.formula.as_deref())
                .unwrap_or(source_id.as_str());

            let negated = edge
                .input_negated
                .get(index)
                .copied()
                .unwrap_or(false);

            if negated {
                format!("not {} ({})", formula, source_id)
            } else {
                format!("{} ({})", formula, source_id)
            }
        })
        .collect()
}

fn format_downstream_and(
    downstream_and: &HashMap<&str, Vec<&str>>,
    node_id: &str,
) -> String {
    downstream_and
        .get(node_id)
        .map(|factor_ids| {
            format!(
                " → {}",
                factor_ids.join(", ")
            )
        })
        .unwrap_or_default()
}

fn format_tolerance(value: f64) -> String {
    if value != 0.0 && value.abs() < 0.0001 {
        format!("{:.1e}", value)
    } else {
        format!("{:.4}", value)
    }
}