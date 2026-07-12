use anyhow::Result;
use crate::core::qbbn::inference::InferenceResult;

pub async fn run_fixtures(client: &reqwest::Client, base: &str) -> Result<()> {
    let url = format!("{}/inference/fixtures", base);
    let names: Vec<String> = super::get_json(client, &url).await?;
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
    let fixture_url = format!("{}/fixtures/qbbn%2F{}", base, fixture);
    let fixture_json: serde_json::Value = super::get_json(client, &fixture_url).await?;

    let url = format!("{}/inference/run", base);
    let result: InferenceResult = super::post_json(client, &url, fixture_json).await?;

    if pretty {
        print_inference_pretty(&result);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    Ok(())
}

fn print_inference_pretty(r: &InferenceResult) {
    println!("\n════════════════════════════════════════════════════════");
    println!("  {}", r.title);
    println!("════════════════════════════════════════════════════════\n");

    let (n_props, n_groups, n_and, n_or, _n_neg, n_evidence) = r.stats;
    println!(
        "  graph: {} propositions, {} groups, {} AND, {} OR, {} evidence",
        n_props, n_groups, n_and, n_or, n_evidence
    );
    println!("  converged in {} iterations\n", r.iterations);

    println!("  ── queries ──");
    for qr in &r.query_results {
        let status = if qr.ok { "✓" } else { "✗" };
        let expected_str = match qr.expected {
            Some(e) => format!(" (expected {:.4} ± {:.4})", e, qr.tolerance),
            None => String::new(),
        };
        println!("  {} P({}) = {:.4}{}", status, qr.formula, qr.prob, expected_str);
    }
    let all_ok = r.query_results.iter().all(|q| q.ok);
    if all_ok {
        println!("\n  ✅ all queries passed");
    } else {
        println!("\n  ⚠️  some queries failed");
    }

    if let Some(ref graph) = r.graph {
        println!("\n  ── formula map ──");
        for (formula, id) in &graph.formula_map {
            println!("  {} → {}", id, formula);
        }

        let or_targets: std::collections::HashSet<&str> = graph
            .edges
            .iter()
            .filter(|e| e.edge_type == "or")
            .map(|e| e.target_id.as_str())
            .collect();

        let and_by_target: std::collections::HashMap<&str, &str> = graph
            .edges
            .iter()
            .filter(|e| e.edge_type == "and")
            .map(|e| (e.target_id.as_str(), e.id.as_str()))
            .collect();

        let or_by_target: std::collections::HashMap<&str, &str> = graph
            .edges
            .iter()
            .filter(|e| e.edge_type == "or")
            .map(|e| (e.target_id.as_str(), e.id.as_str()))
            .collect();

        let source_props: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.node_type == "proposition" && !or_targets.contains(n.id.as_str()))
            .collect();
        let computed_props: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.node_type == "proposition" && or_targets.contains(n.id.as_str()))
            .collect();
        let groups: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.node_type == "group")
            .collect();

        println!("\n  ── source propositions (no OR parent) ──");
        for node in &source_props {
            let kind = if node.is_evidence { "📡" } else { "  " };
            let neg = if node.negated { "¬" } else { "" };
            let formula = node.formula.as_deref().unwrap_or("—");
            let prob = if node.is_evidence {
                format!("evidence={:.2}", node.evidence_prob.unwrap_or(0.5))
            } else {
                format!("belief={:.4}", node.belief)
            };
            let and_out = and_by_target.get(node.id.as_str())
                .map(|id| format!(" → {}", id))
                .unwrap_or_default();
            println!(
                "  {} {}{}  {:<30}  {}{}",
                kind, neg, node.id, formula, prob, and_out
            );
        }

        println!("\n  ── groups (AND output, OR input) ──");
        for node in &groups {
            let neg = if node.negated { "¬" } else { "" };
            let and_in = and_by_target.get(node.id.as_str()).unwrap_or(&"?");
            let or_out = or_by_target.get(node.conclusion_id.as_deref().unwrap_or(""))
                .map(|id| format!(" → {}", id))
                .unwrap_or_default();
            let rule = node.rule_id.as_deref().unwrap_or("?");
            println!(
                "  {}{}  and={:<6}  rule={:<6}  conclusion={}{}",
                neg,
                node.id,
                and_in,
                rule,
                node.conclusion_id.as_deref().unwrap_or("—"),
                or_out
            );
        }

        println!("\n  ── computed propositions (OR output, AND input) ──");
        for node in &computed_props {
            let neg = if node.negated { "¬" } else { "" };
            let formula = node.formula.as_deref().unwrap_or("—");
            let or_in = or_by_target.get(node.id.as_str()).unwrap_or(&"?");
            let and_out = and_by_target.get(node.id.as_str())
                .map(|id| format!(" → {}", id))
                .unwrap_or_default();
            println!(
                "  {}{}  {:<30}  or={:<6}  belief={:.4}{}",
                neg, node.id, formula, or_in, node.belief, and_out
            );
        }

        println!("\n  ── flow chains ──");
        for src in &source_props {
            let src_id = src.id.as_str();
            for edge in &graph.edges {
                if edge.edge_type != "and" {
                    continue;
                }
                let input_idx = edge.source_ids.iter().position(|s| s == src_id);
                if input_idx.is_none() {
                    continue;
                }
                let group_id = &edge.target_id;
                let group = groups.iter().find(|g| g.id == *group_id);
                if group.is_none() {
                    continue;
                }
                let g = group.unwrap();
                let is_negated = edge.input_negated[input_idx.unwrap()];
                let neg = if is_negated { "¬" } else { "" };
                let input_prob = if is_negated {
                    1.0 - src.belief
                } else {
                    src.belief
                };
                let conclusion = g.conclusion_id.as_deref().unwrap_or("?");
                let or_id = or_by_target.get(conclusion).unwrap_or(&"?");
                let rule_weight = g.rule_id.as_ref()
                    .and_then(|rid| graph.rules.iter().find(|r| r.id == *rid))
                    .map(|r| r.weight)
                    .unwrap_or(0.0);
                println!(
                    "  {}{}({:.2}) → {} → {}{}(w={:.1}) → {} → {}",
                    neg,
                    src_id,
                    input_prob,
                    edge.id,
                    if g.negated { "¬" } else { "" },
                    group_id,
                    rule_weight,
                    or_id,
                    conclusion
                );
            }
        }

        println!("\n  ── factors ──");
        for edge in &graph.edges {
            let neg_str = if edge.input_negated.iter().any(|&n| n) {
                let negs: Vec<String> = edge
                    .input_negated
                    .iter()
                    .enumerate()
                    .filter(|(_, &n)| n)
                    .map(|(i, _)| edge.source_ids[i].clone())
                    .collect();
                format!(" (negated: {})", negs.join(", "))
            } else {
                String::new()
            };
            println!(
                "  {:>3} {:<6}  {} → {}{}",
                edge.edge_type.to_uppercase(),
                edge.id,
                edge.source_ids.join(", "),
                edge.target_id,
                neg_str
            );
        }

        println!("\n  ── rules ──");
        for rule in &graph.rules {
            println!(
                "  {:<6}  w={:<5.1}  {} → {}",
                rule.id,
                rule.weight,
                rule.premise_patterns.join(", "),
                rule.conclusion_pattern
            );
        }

        println!("\n  ── CPT tables ──");
        for cpt in &graph.cpt_tables {
            println!(
                "  {:>3} {}: {} → {}",
                cpt.factor_type.to_uppercase(),
                cpt.factor_id,
                cpt.inputs.join(" ∧ "),
                cpt.output
            );
            if cpt.truncated {
                println!("    (truncated — {} inputs)", cpt.inputs.len());
            } else if cpt.factor_type == "and" {
                println!("    deterministic AND");
            } else if !cpt.rows.is_empty() {
                let header = cpt.inputs.join(" | ");
                println!("    {:<20} | P(out=1)", header);
                println!("    {}", "─".repeat(header.len() + 18));
                for row in &cpt.rows {
                    let vals: Vec<String> = row
                        .assignment
                        .iter()
                        .map(|&b| if b { "T".to_string() } else { "F".to_string() })
                        .collect();
                    println!("    {:<20} | {:.4}", vals.join(" | "), row.prob_true);
                }
            }
            println!();
        }
    }

    println!();
}