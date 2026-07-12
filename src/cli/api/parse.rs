use anyhow::Result;
use crate::cli::pretty::print_pretty;
use crate::server::ParseResult;

pub async fn run_fixtures(client: &reqwest::Client, base: &str) -> Result<()> {
    let url = format!("{}/fixtures", base);
    let body: serde_json::Value = super::get_json(client, &url).await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

pub async fn run_fixture(client: &reqwest::Client, base: &str, name: &str) -> Result<()> {
    let url = format!("{}/fixtures/{}", base, name);
    let body: serde_json::Value = super::get_json(client, &url).await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

pub async fn run_parse(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    pretty: bool,
) -> Result<()> {
    let url = format!("{}/fixtures/{}/parse", base, name);
    let results: Vec<ParseResult> = super::get_json(client, &url).await?;
    if pretty {
        print_pretty(&results);
    } else {
        println!("{}", serde_json::to_string_pretty(&results)?);
    }
    Ok(())
}

pub async fn run_parse_one(
    client: &reqwest::Client,
    base: &str,
    fixture: &str,
    sentence: &str,
    pretty: bool,
) -> Result<()> {
    let url = format!("{}/parse/one", base);
    let body = serde_json::json!({ "fixture": fixture, "sentence": sentence });
    let results: Vec<ParseResult> = super::post_json(client, &url, body).await?;
    if pretty {
        print_pretty(&results);
    } else {
        println!("{}", serde_json::to_string_pretty(&results)?);
    }
    Ok(())
}