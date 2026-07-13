use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::qbbn::inference::{run_inference_fixture_debug, InferenceFixture};

#[derive(Args)]
pub struct InferArgs {
    /// Path to an inference fixture JSON file.
    #[arg(long)]
    pub fixture: PathBuf,

    /// Print full CPT tables and all intermediate calculations.
    #[arg(long)]
    pub debug: bool,
}

pub fn run_infer(args: InferArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.fixture)
        .with_context(|| format!("reading inference fixture {}", args.fixture.display()))?;

    let fixture: InferenceFixture = serde_json::from_str(&raw)
        .with_context(|| format!("parsing inference JSON {}", args.fixture.display()))?;

    let result = run_inference_fixture_debug(&fixture, args.debug)
        .map_err(|error| anyhow::anyhow!("inference failed: {}", error))?;

    println!("📊 {}\n", result.title);

    println!(
        "  Graph: {} propositions, {} groups, {} AND, {} OR, {} NEG, {} evidence",
        result.stats.0,
        result.stats.1,
        result.stats.2,
        result.stats.3,
        result.stats.4,
        result.stats.5
    );

    println!(
        "  Topology: {} — {}",
        result.topology.kind(),
        if result.topology.bp_should_be_exact() {
            "BP must equal exact"
        } else {
            "loopy BP is approximate"
        }
    );

    println!("  BP converged in {} iterations", result.iterations);

    match (
        result.exact_partition_function,
        result.exact_error.as_deref(),
    ) {
        (Some(partition), _) => {
            println!("  Exact enumeration succeeded: Z={:.8}", partition);
        }
        (_, Some(error)) => {
            println!("  Exact enumeration unavailable: {}", error);
        }
        _ => {}
    }

    println!();

    for query in &result.query_results {
        let status = if query.ok { "✅" } else { "❌" };

        let exact_text = query
            .exact_prob
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "—".to_string());

        let delta_text = query
            .bp_exact_delta
            .map(|value| format!("{value:+.6}"))
            .unwrap_or_else(|| "—".to_string());

        let expected_text = match (query.expected, query.expected_ok) {
            (Some(expected), Some(true)) => {
                format!("expected={expected:.6} ✓")
            }
            (Some(expected), Some(false)) => {
                format!("expected={expected:.6} ✗")
            }
            (Some(expected), None) => {
                format!("expected={expected:.6} ?")
            }
            (None, _) => "no hand expectation".to_string(),
        };

        let bp_contract = if result.topology.bp_should_be_exact() {
            match query.bp_matches_exact {
                Some(true) => "BP=exact ✓",
                Some(false) => "BP=exact ✗",
                None => "BP=exact ?",
            }
        } else {
            "loopy diagnostic"
        };

        println!(
            "  {} P({}) exact={}  {}  |  BP={:.6}  Δ={}  {}",
            status, query.formula, exact_text, expected_text, query.prob, delta_text, bp_contract
        );
    }

    let all_ok = result.query_results.iter().all(|query| query.ok);

    if all_ok {
        println!("\n  🎯 All topology-appropriate checks passed");
    } else {
        println!("\n  ⚠️  Some topology-appropriate checks failed");
    }

    Ok(())
}
