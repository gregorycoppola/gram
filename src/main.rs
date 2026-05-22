mod cli;
mod core;
mod db;
mod server;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "gram", about = "Typed slot grammar parser for natural language")]
struct Cli {
    #[command(subcommand)]
    command: cli::Command,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli::run(cli.command)
}