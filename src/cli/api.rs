pub mod inference;
pub mod parse;
pub mod proof;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ApiCommand {
    /// GET /health
    Health,
    /// GET /version — API and fixture compatibility versions
    Version,
    /// GET /fixtures — list all fixtures
    Fixtures,
    /// GET /fixtures/:name — raw fixture JSON
    Fixture { name: String },
    /// GET /fixtures/:name/parse — parse all sentences
    Parse {
        name: String,
        /// Pretty-print the response.
        #[arg(long)]
        pretty: bool,
    },
    /// POST /parse/one — parse a single sentence
    ParseOne {
        #[arg(long)]
        fixture: String,
        #[arg(long)]
        sentence: String,
        /// Pretty-print the response.
        #[arg(long)]
        pretty: bool,
    },
    /// GET /proofs — list all proof names
    Proofs,
    /// GET /proofs/:name — show a proof file
    Proof { name: String },
    /// POST /proof/check — check a proof
    CheckProof {
        /// Path to a proof JSON file.
        #[arg(long)]
        proof: PathBuf,
    },
    /// GET /inference/fixtures — list QBBN inference fixtures
    InferenceFixtures,
    /// POST /inference/run — run inference on a fixture
    InferenceRun {
        /// Fixture name (in fixtures/qbbn/).
        #[arg(long)]
        fixture: String,
        /// Pretty-print the graph and results.
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(Args)]
pub struct ApiArgs {
    #[command(subcommand)]
    pub command: ApiCommand,
    /// Server base URL.
    #[arg(long, default_value = "http://127.0.0.1:9101")]
    pub url: String,
}

pub fn run_api(args: ApiArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_api_async(args))
}

async fn run_api_async(args: ApiArgs) -> Result<()> {
    let base = args.url.trim_end_matches('/');
    let client = reqwest::Client::new();

    match args.command {
        ApiCommand::Health => {
            let url = format!("{}/health", base);
            let resp = client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("GET {}", url))?;
            let body = resp
                .json::<serde_json::Value>()
                .await
                .context("parsing health response")?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        ApiCommand::Version => {
            let url = format!("{}/version", base);
            let body: crate::server::VersionResponse = get_json(&client, &url).await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
            Ok(())
        }
        ApiCommand::Fixtures => parse::run_fixtures(&client, base).await,
        ApiCommand::Fixture { name } => parse::run_fixture(&client, base, &name).await,
        ApiCommand::Parse { name, pretty } => parse::run_parse(&client, base, &name, pretty).await,
        ApiCommand::ParseOne {
            fixture,
            sentence,
            pretty,
        } => parse::run_parse_one(&client, base, &fixture, &sentence, pretty).await,
        ApiCommand::Proofs => proof::run_list(&client, base).await,
        ApiCommand::Proof { name } => proof::run_show(&client, base, &name).await,
        ApiCommand::CheckProof { proof } => proof::run_check(&client, base, &proof).await,
        ApiCommand::InferenceFixtures => inference::run_fixtures(&client, base).await,
        ApiCommand::InferenceRun { fixture, pretty } => {
            inference::run_run(&client, base, &fixture, pretty).await
        }
    }
}

pub async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .with_context(|| format!("reading body from {}", url))?;
    if !status.is_success() {
        anyhow::bail!("{} -> {}: {}", url, status, body_text);
    }
    serde_json::from_str(&body_text).with_context(|| format!("parsing JSON from {}", url))
}

pub async fn post_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    body: serde_json::Value,
) -> Result<T> {
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {}", url))?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .with_context(|| format!("reading body from {}", url))?;
    if !status.is_success() {
        anyhow::bail!("{} -> {}: {}", url, status, body_text);
    }
    serde_json::from_str(&body_text).with_context(|| format!("parsing JSON from {}", url))
}
