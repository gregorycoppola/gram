pub mod api;
pub mod inspect;
pub mod parse;
pub mod serve;
pub mod tree;

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
    /// Pretty-print syntax trees for all sentences in a fixture.
    Tree(tree::TreeArgs),
    /// HTTP client — hit server endpoints and print JSON.
    Api(api::ApiArgs),
    /// Run the HTTP server on localhost. Serves fixtures from a directory;
    /// the gloss frontend is a separate Vite app that talks to this API.
    Serve(serve::ServeArgs),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Parse(args) => parse::run_parse(args),
        Command::ParseOne(args) => parse::run_parse_one(args),
        Command::Inspect(args) => inspect::run_inspect(args),
        Command::Tree(args) => tree::run_tree(args),
        Command::Api(args) => api::run_api(args),
        Command::Serve(args) => serve::run_serve(args),
    }
}