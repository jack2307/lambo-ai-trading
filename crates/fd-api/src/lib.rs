//! HTTP API over the store and the engines.
//!
//! One binary serves the JSON the browser reads and, in a release build, the
//! built SPA beside it. That is the whole deployment: no process manager, no
//! reverse proxy, nothing to keep in sync.

pub mod dto;
pub mod error;
pub mod live;
pub mod research;
pub mod routes;
pub mod state;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

pub use error::ApiError;
pub use state::AppState;

/// Build the router.
///
/// `ui` is the directory holding a built SPA. When it is absent the API still
/// serves — which is what a `vite dev` session wants, since Vite serves the
/// client itself and proxies only `/api` here.
pub fn router(state: Arc<AppState>, ui: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/api/chart/catalog", get(routes::catalog))
        .route("/api/chart/bars", get(routes::bars))
        .route("/api/chart/levels", get(routes::levels))
        .route("/api/chart/leaderboard", get(routes::leaderboard))
        .route("/api/chart/indicators", post(routes::indicators))
        .route("/api/chart/backtest", post(routes::backtest))
        .route("/api/live", get(live::stream))
        .route("/api/live/status", get(live::status))
        .route("/api/research", get(research::research))
        .with_state(state);

    match ui.filter(|dir| dir.is_dir()) {
        Some(dir) => {
            let index = dir.join("index.html");
            // Unknown paths fall back to index.html so the client's own routing
            // survives a reload — but only after the API routes have had their
            // chance, or a typo'd endpoint would answer with a page.
            // index.html must never be served from a browser cache: it names
            // hashed bundles, and a stale copy after a rebuild points at files
            // that no longer exist (a blank page, 2026-09-14). The bundles
            // themselves are content-addressed and may cache forever.
            let index = tower_http::set_header::SetResponseHeader::overriding(
                ServeFile::new(index),
                axum::http::header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-cache"),
            );
            api.fallback_service(ServeDir::new(dir).fallback(index))
        }
        // Permissive CORS only in the no-SPA case, which is the dev setup where
        // the client is served from another port.
        None => api.layer(CorsLayer::permissive()),
    }
}
