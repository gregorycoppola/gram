use anyhow::{Context, Result};
use clap::{Args, Subcommand};

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
    },
    /// POST /parse/one — parse a single sentence
    ParseOne {
        #[arg(long)]
        fixture: String,
        #[arg(long)]
        sentence: String,
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

    let url = match args.command {
        ApiCommand::Health => format!("{}/health", base),
        ApiCommand::Fixtures => format!("{}/fixtures", base),
        ApiCommand::Fixture { ref name } => format!("{}/fixtures/{}", base, name),
        ApiCommand::Parse { ref name } => format!("{}/fixtures/{}/parse", base, name),
        ApiCommand::ParseOne { .. } => format!("{}/parse/one", base),
    };

    let client = reqwest::Client::new();

    let resp = match args.command {
        ApiCommand::ParseOne {
            ref fixture,
            ref sentence,
        } => client
            .post(&url)
            .json(&serde_json::json!({
                "fixture": fixture,
                "sentence": sentence,
            }))
            .send()
            .await
            .with_context(|| format!("POST {}", url))?,
        _ => client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {}", url))?,
    };

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .with_context(|| format!("reading response body from {}", url))?;

    if !status.is_success() {
        anyhow::bail!("{} -> {}: {}", url, status, body);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&body).context("serializing response")?
    );
    Ok(())
}