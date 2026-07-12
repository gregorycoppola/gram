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
        .map_err(|e| anyhow::anyhow!("inference failed: {}", e))?;

    println!("📊 {}\n", result.title);
    println!("  Graph: {} propositions, {} groups, {} AND, {} OR, {} NEG, {} evidence",
        result.stats.0, result.stats.1, result.stats.2, result.stats.3, result.stats.4, result.stats.5);
    println!("  Converged in {} iterations\n", result.iterations);

    for qr in &result.query_results {
        match qr.expected {
            Some(expected) => {
                if qr.ok {
                    println!("  ✅ P({}) = {:.4}  (expected {:.4} ± {:.4})",
                        qr.formula, qr.prob, expected, qr.tolerance);
                } else {
                    println!("  ❌ P({}) = {:.4}  (expected {:.4} ± {:.4})",
                        qr.formula, qr.prob, expected, qr.tolerance);
                }
            }
            None => {
                println!("  ℹ️  P({}) = {:.4}", qr.formula, qr.prob);
            }
        }
    }

    let all_ok = result.query_results.iter().all(|qr| qr.ok);
    if all_ok {
        println!("\n  🎯 All queries passed");
    } else {
        println!("\n  ⚠️  Some queries failed");
    }

    Ok(())
}