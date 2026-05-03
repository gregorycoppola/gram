pub mod inspect;
pub mod parse;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Parse all sentences in a fixture file.
    Parse(parse::ParseArgs),
    /// Parse a single sentence using the lexicon/grammar from a fixture.
    ParseOne(parse::ParseOneArgs),
    /// Dump fixture internals — lexicon form index, tokenization, compiled rules.
    Inspect(inspect::InspectArgs),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Parse(args) => parse::run_parse(args),
        Command::ParseOne(args) => parse::run_parse_one(args),
        Command::Inspect(args) => inspect::run_inspect(args),
    }
}