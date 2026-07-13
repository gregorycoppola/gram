use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::server::run_server;

#[derive(Args)]
pub struct ServeArgs {
    /// Port to bind on.
    #[arg(long, default_value_t = 9101)]
    pub port: u16,

    /// Regression fixtures directory (relative or absolute).
    #[arg(long, default_value = "fixtures")]
    pub fixtures_dir: PathBuf,

    /// Proofs directory (relative or absolute).
    #[arg(long, default_value = "proofs")]
    pub proofs_dir: PathBuf,

    /// Corpus repository containing news/<article>/parses/*.json.
    ///
    /// The default assumes gram and gram-data are sibling repositories.
    #[arg(long, default_value = "../gram-data")]
    pub data_dir: PathBuf,
}

pub fn run_serve(args: ServeArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(run_server(
        args.port,
        args.fixtures_dir,
        args.proofs_dir,
        args.data_dir,
    ))
}
