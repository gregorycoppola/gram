use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::qbbn::inference::{
    run_inference_fixture_debug, InferenceFixture,
};

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
        .with_context(|| {
            format!(
                "reading inference fixture {}",
                args.fixture.display()
            )
        })?;

    let fixture: InferenceFixture =
        serde_json::from_str(&raw).with_context(|| {
            format!(
                "parsing inference JSON {}",
                args.fixture.display()
            )
        })?;

    let result =
        run_inference_fixture_debug(&fixture, args.debug)
            .map_err(|error| {
                anyhow::anyhow!("inference failed: {}", error)
            })?;

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
    println!("  BP converged in {} iterations", result.iterations);

    match (
        result.exact_partition_function,
        result.exact_error.as_deref(),
    ) {
        (Some(partition), _) => {
            println!(
                "  Exact enumeration succeeded: Z={:.8}",
                partition
            );
        }
        (_, Some(error)) => {
            println!("  Exact enumeration unavailable: {}", error);
        }
        _ => {}
    }

    println!();

    for query in &result.query_results {
        let status = if query.ok { "✅" } else { "❌" };

        let exact_text = match query.exact_prob {
            Some(exact) => format!("{:.6}", exact),
            None => "—".to_string(),
        };

        let delta_text = match query.bp_exact_delta {
            Some(delta) => format!("{:+.6}", delta),
            None => "—".to_string(),
        };

        let expected_text = match query.expected {
            Some(expected) => format!(
                "  expected={:.6} ± {:.6}",
                expected, query.tolerance
            ),
            None => String::new(),
        };

        println!(
            "  {} P({})  BP={:.6}  exact={}  Δ={}{}",
            status,
            query.formula,
            query.prob,
            exact_text,
            delta_text,
            expected_text
        );
    }

    let all_ok =
        result.query_results.iter().all(|query| query.ok);

    if all_ok {
        println!("\n  🎯 All BP expectations passed");
    } else {
        println!("\n  ⚠️  Some BP expectations failed");
    }

    Ok(())
}