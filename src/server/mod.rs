pub mod types;

mod error;
mod routes;

use anyhow::Result;
use axum::{routing::{get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use types::{ParseResult, ParseStatus, FixtureSummary, ParseRequest, CheckProofRequest, CheckProofResponse};

#[derive(Clone)]
pub struct AppState {
    pub fixtures_dir: Arc<PathBuf>,
}

pub async fn run_server(port: u16, fixtures_dir: PathBuf) -> Result<()> {
    let state = AppState {
        fixtures_dir: Arc::new(fixtures_dir),
    };
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/proof/check", post(routes::check_proof))
        .route("/fixtures", get(routes::list_fixtures))
        .route("/fixtures/*path", get(routes::handle_fixture))
        .route("/parse/one", post(routes::parse_one))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("🌐 gram serve listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}