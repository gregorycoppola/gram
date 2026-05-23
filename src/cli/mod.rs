pub mod db;
pub mod inspect;
pub mod parse;
pub mod serve;

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
    /// Run the HTTP server (fixtures + DB) on localhost.
    Serve(serve::ServeArgs),
    /// Manage sentences in the DB.
    #[command(subcommand)]
    Sentence(db::SentenceCmd),
    /// Manage predicates (lexicon) in the DB.
    #[command(subcommand)]
    Predicate(db::PredicateCmd),
    /// Manage entities (lexicon) in the DB.
    #[command(subcommand)]
    Entity(db::EntityCmd),
    /// Manage grammar rules in the DB.
    #[command(subcommand)]
    Rule(db::RuleCmd),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Parse(args) => parse::run_parse(args),
        Command::ParseOne(args) => parse::run_parse_one(args),
        Command::Inspect(args) => inspect::run_inspect(args),
        Command::Serve(args) => serve::run_serve(args),
        Command::Sentence(c) => db::run_db(db::DbCommand::Sentence(c)),
        Command::Predicate(c) => db::run_db(db::DbCommand::Predicate(c)),
        Command::Entity(c) => db::run_db(db::DbCommand::Entity(c)),
        Command::Rule(c) => db::run_db(db::DbCommand::Rule(c)),
    }
}