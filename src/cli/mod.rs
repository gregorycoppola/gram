pub mod api;
pub mod check;
pub mod infer;
pub mod inspect;
pub mod parse;
pub mod pretty;
pub mod serve;
pub mod tree;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    Parse(parse::ParseArgs),
    ParseOne(parse::ParseOneArgs),
    Inspect(inspect::InspectArgs),
    Tree(tree::TreeArgs),
    Api(api::ApiArgs),
    Serve(serve::ServeArgs),
    Check(check::CheckArgs),
    Infer(infer::InferArgs),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Parse(args) => parse::run_parse(args),
        Command::ParseOne(args) => parse::run_parse_one(args),
        Command::Inspect(args) => inspect::run_inspect(args),
        Command::Tree(args) => tree::run_tree(args),
        Command::Api(args) => api::run_api(args),
        Command::Serve(args) => serve::run_serve(args),
        Command::Check(args) => check::run_check(args),
        Command::Infer(args) => infer::run_infer(args),
    }
}
