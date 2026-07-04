mod error;
mod routes;
pub mod db_routes;
mod types;

use anyhow::Result;
use axum::{routing::{delete, get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::db::client::Db;

#[derive(Clone)]
pub struct AppState {
    pub fixtures_dir: Arc<PathBuf>,
    pub db: Db,
}

pub async fn run_server(port: u16, fixtures_dir: PathBuf, gloss_dir: PathBuf) -> Result<()> {
    let db = Db::connect().await?;
    let state = AppState {
        fixtures_dir: Arc::new(fixtures_dir),
        db,
    };
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    // Static frontend serving.
    //
    // gloss is a SolidJS app built by Vite to gloss/dist/. gram serves that
    // directory directly from disk so what Claude sees by reading the source
    // and what you see in the browser are the same files, served by the same
    // process that owns the parser. No embed, no separate static host.
    //
    // API routes are matched first; anything else falls through to ServeDir.
    // If the requested file doesn't exist (e.g. a client-side route), we
    // fall back to index.html.
    if !gloss_dir.exists() {
        eprintln!(
            "warn: gloss_dir {} does not exist — frontend will not be served.",
            gloss_dir.display()
        );
        eprintln!("      build it with:  (cd ../gloss && npm run build)");
    }
    let index_path = gloss_dir.join("index.html");
    let static_service = ServeDir::new(gloss_dir).fallback(ServeFile::new(index_path));

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/fixtures", get(routes::list_fixtures))
        .route("/fixtures/:name", get(routes::get_fixture))
        .route("/fixtures/:name/parse", get(routes::parse_fixture))
        .route("/predicates", get(db_routes::list_predicates).post(db_routes::create_predicate))
        .route("/predicates/:id", delete(db_routes::delete_predicate))
        .route("/entities", get(db_routes::list_entities).post(db_routes::create_entity))
        .route("/entities/:id", delete(db_routes::delete_entity))
        .route("/rules", get(db_routes::list_rules).post(db_routes::create_rule))
        .route("/rules/:id", delete(db_routes::delete_rule))
        .route("/sentences", get(db_routes::list_sentences).post(db_routes::create_sentence))
        .route("/sentences/:id", delete(db_routes::delete_sentence))
        .route("/parse", get(db_routes::parse_all).post(routes::parse_one))
        .route("/parse/one", post(db_routes::parse_one))
        .fallback_service(static_service)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("🌐 gram serve listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}