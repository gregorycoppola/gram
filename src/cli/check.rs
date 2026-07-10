use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::core::proof::checker::check_proof;
use crate::core::proof::ProofFile;

#[derive(Args)]
pub struct CheckArgs {
    /// Path to a proof JSON file.
    #[arg(long)]
    pub proof: PathBuf,
}

pub fn run_check(args: CheckArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.proof)
        .with_context(|| format!("reading proof file {}", args.proof.display()))?;
    let file: ProofFile = serde_json::from_str(&raw)
        .with_context(|| format!("parsing proof JSON {}", args.proof.display()))?;

    let result = check_proof(&file)
        .map_err(|e| anyhow::anyhow!("proof check failed: {}", e))?;

    println!("📋 {}\n", result.title);
    for (step, status) in &result.steps {
        match status {
            crate::core::proof::StepResult::Ok => {
                println!("  ✅ {:<14} {}", step.justification, step.formula);
                if !step.from.is_empty() {
                    let from_str: String = step.from.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
                    println!("     (from steps: {})", from_str);
                }
            }
            crate::core::proof::StepResult::Err(e) => {
                println!("  ❌ {:<14} {}", step.justification, step.formula);
                println!("     {}", e);
            }
        }
    }

    println!();
    if result.conclusion_reached {
        println!("  🎯 Conclusion verified: {}", result.conclusion);
    } else {
        println!("  ⚠️  Conclusion NOT reached: {}", result.conclusion);
    }

    Ok(())
}