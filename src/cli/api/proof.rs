use anyhow::{Context, Result};
use std::path::PathBuf;

pub async fn run_list(client: &reqwest::Client, base: &str) -> Result<()> {
    let url = format!("{}/proofs", base);
    let names: Vec<String> = super::get_json(client, &url).await?;
    for name in names {
        println!("  {}", name);
    }
    Ok(())
}

pub async fn run_show(client: &reqwest::Client, base: &str, name: &str) -> Result<()> {
    let url = format!("{}/proofs/{}", base, name);
    let body: serde_json::Value = super::get_json(client, &url).await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

pub async fn run_check(client: &reqwest::Client, base: &str, proof: &PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(proof)
        .with_context(|| format!("reading proof file {}", proof.display()))?;
    let body: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing proof JSON {}", proof.display()))?;
    let url = format!("{}/proof/check", base);
    let result: crate::server::CheckProofResponse = super::post_json(client, &url, body).await?;
    print_check_result(&result);
    Ok(())
}

fn print_check_result(r: &crate::server::CheckProofResponse) {
    println!("\n════════════════════════════════════════════════════════");
    println!("  {}", r.title);
    println!("════════════════════════════════════════════════════════\n");
    if r.conclusion_reached {
        println!("  ✅ conclusion reached: {}\n", r.conclusion);
    } else {
        println!("  ❌ conclusion NOT reached: {}\n", r.conclusion);
    }
    for step in &r.steps {
        let status = if step.ok { "✓" } else { "✗" };
        println!(
            "  {} {:>2}. {}  [{}]  (from {:?})",
            status, step.step, step.formula, step.justification, step.from
        );
        if let Some(ref err) = step.error {
            println!("      💥 {}", err);
        }
    }
    println!();
}