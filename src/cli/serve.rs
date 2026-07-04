use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::server::run_server;

#[derive(Args)]
pub struct ServeArgs {
    /// Port to bind on.
    #[arg(long, default_value_t = 9101)]
    pub port: u16,
    /// Fixtures directory (relative or absolute).
    #[arg(long, default_value = "fixtures")]
    pub fixtures_dir: PathBuf,
    /// Built gloss frontend directory. gram serves index.html and assets
    /// from here for any path that isn't an API route. Defaults to
    /// ../gloss/dist (relative to the gram repo root). If the directory
    /// does not exist, the API still works; the browser will 404 on /.
    #[arg(long, default_value = "../gloss/dist")]
    pub gloss_dir: PathBuf,
}

pub fn run_serve(args: ServeArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_server(args.port, args.fixtures_dir, args.gloss_dir))
}