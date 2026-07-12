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
        println!("\n  ── nodes ──");
        for node in &graph.nodes {
            let kind = if node.is_evidence { "📡" } else { "  " };
            let type_icon = match node.node_type.as_str() {
                "proposition" => "●",
                "group" => "◆",
                _ => "○",
            };
            let neg = if node.negated { "¬" } else { "" };
            let formula = node.formula.as_deref().unwrap_or("—");
            let prob = if node.is_evidence {
                format!("evidence={:.2}", node.evidence_prob.unwrap_or(0.5))
            } else {
                format!("belief={:.4}", node.belief)
            };
            println!(
                "  {} {} {}{}  {:<30}  {}",
                kind, type_icon, neg, node.id, formula, prob
            );
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
                        .map(|&b| if b { "T" } else { "F" })
                        .collect();
                    println!("    {:<20} | {:.4}", vals.join(" | "), row.prob_true);
                }
            }
            println!();
        }
    }

    println!();
}