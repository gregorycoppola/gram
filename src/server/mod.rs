pub mod types;

mod error;
mod routes;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use types::{
    CheckProofRequest, CheckProofResponse, CoverageArticleSummary,
    CoverageExampleSummary, FixtureSummary, ParseRequest, ParseResult, ParseStatus,
};

#[derive(Clone)]
pub struct AppState {
    pub fixtures_dir: Arc<PathBuf>,
    pub proofs_dir: Arc<PathBuf>,
    pub data_dir: Arc<PathBuf>,
}

pub async fn run_server(
    port: u16,
    fixtures_dir: PathBuf,
    proofs_dir: PathBuf,
    data_dir: PathBuf,
) -> Result<()> {
    let state = AppState {
        fixtures_dir: Arc::new(fixtures_dir),
        proofs_dir: Arc::new(proofs_dir),
        data_dir: Arc::new(data_dir),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/proofs", get(routes::list_proofs))
        .route("/proofs/:name", get(routes::get_proof))
        .route("/proof/check", post(routes::check_proof))
        .route("/fixtures", get(routes::list_fixtures))
        .route("/fixtures/*path", get(routes::handle_fixture))
        .route("/coverage", get(routes::list_coverage))
        .route("/coverage/*path", get(routes::handle_coverage))
        .route("/parse/one", post(routes::parse_one))
        .route("/inference/fixtures", get(routes::list_inference_fixtures))
        .route("/inference/run", post(routes::run_inference))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("🌐 gram serve listening on http://{}", addr);
    println!("📚 coverage data: {}", listener_local_data_label());

    axum::serve(listener, app).await?;

    Ok(())
}

fn listener_local_data_label() -> &'static str {
    "configured by --data-dir"
}
