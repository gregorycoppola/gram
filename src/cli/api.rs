use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::cli::pretty::print_pretty;
use crate::server::ParseResult;

#[derive(Subcommand)]
pub enum ApiCommand {
    /// GET /health
    Health,
    /// GET /fixtures — list all fixtures
    Fixtures,
    /// GET /fixtures/:name — raw fixture JSON
    Fixture {
        name: String,
    },
    /// GET /fixtures/:name/parse — parse all sentences in a fixture
    Parse {
        name: String,
        /// Pretty-print the response (bracket tree, constituents, rule trace) instead of raw JSON.
        #[arg(long)]
        pretty: bool,
    },
    /// POST /parse/one — parse a single sentence
    ParseOne {
        #[arg(long)]
        fixture: String,
        #[arg(long)]
        sentence: String,
        /// Pretty-print the response (bracket tree, constituents, rule trace) instead of raw JSON.
        #[arg(long)]
        pretty: bool,
    },
    /// POST /proof/check — check a proof file for valid inference steps
    CheckProof {
        /// Path to a proof JSON file.
        #[arg(long)]
        proof: PathBuf,
    },
}

#[derive(Args)]
pub struct ApiArgs {
    #[command(subcommand)]
    command: ApiCommand,
    /// Server base URL.
    #[arg(long, default_value = "http://127.0.0.1:9101")]
    url: String,
}

pub fn run_api(args: ApiArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_api_async(args))
}

async fn run_api_async(args: ApiArgs) -> Result<()> {
    let base = args.url.trim_end_matches('/');

    let (url, pretty, is_post, body) = match &args.command {
        ApiCommand::Health => (format!("{}/health", base), false, false, None),
        ApiCommand::Fixtures => (format!("{}/fixtures", base), false, false, None),
        ApiCommand::Fixture { name } => (format!("{}/fixtures/{}", base, name), false, false, None),
        ApiCommand::Parse { name, pretty } => (format!("{}/fixtures/{}/parse", base, name), *pretty, false, None),
        ApiCommand::ParseOne { fixture, sentence, pretty } => {
            let body = serde_json::json!({
                "fixture": fixture,
                "sentence": sentence,
            });
            (format!("{}/parse/one", base), *pretty, true, Some(body))
        }
        ApiCommand::CheckProof { proof } => {
            let raw = std::fs::read_to_string(proof)
                .with_context(|| format!("reading proof file {}", proof.display()))?;
            let body: serde_json::Value = serde_json::from_str(&raw)
                .with_context(|| format!("parsing proof JSON {}", proof.display()))?;
            (format!("{}/proof/check", base), false, true, Some(body))
        }
    };

    let client = reqwest::Client::new();

    let resp = if is_post {
        let mut req = client.post(&url);
        if let Some(b) = body {
            req = req.json(&b);
        }
        req.send().await.with_context(|| format!("POST {}", url))?
    } else {
        client.get(&url).send().await.with_context(|| format!("GET {}", url))?
    };

    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .with_context(|| format!("reading response body from {}", url))?;

    if !status.is_success() {
        let body: serde_json::Value = serde_json::from_str(&body_text).unwrap_or(serde_json::json!({}));
        anyhow::bail!("{} -> {}: {}", url, status, body);
    }

    if pretty {
        let results: Vec<ParseResult> = serde_json::from_str(&body_text)
            .with_context(|| "deserializing parse results for pretty-print")?;
        print_pretty(&results);
        return Ok(());
    }

    let body: serde_json::Value = serde_json::from_str(&body_text)
        .with_context(|| format!("parsing JSON response from {}", url))?;
    println!("{}", serde_json::to_string_pretty(&body).context("serializing response")?);
    Ok(())
}