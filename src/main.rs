use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "gram",
    about = "Typed slot grammar parser for natural language"
)]
struct Cli {
    #[command(subcommand)]
    command: gram::cli::Command,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    gram::cli::run(cli.command)
}
