mod error;
mod routes;
mod types;

use anyhow::Result;
use axum::{routing::{get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub fixtures_dir: Arc<PathBuf>,
}

pub async fn run_server(port: u16, fixtures_dir: PathBuf) -> Result<()> {
    let state = AppState {
        fixtures_dir: Arc::new(fixtures_dir),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/fixtures", get(routes::list_fixtures))
        .route("/fixtures/:name", get(routes::get_fixture))
        .route("/fixtures/:name/parse", get(routes::parse_fixture))
        .route("/parse", post(routes::parse_one))
        .with_state(state)
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("🌐 gram serve listening on http://{}", addr);
    println!("   fixtures dir: {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}