//! HTTP API over the store and the engines.
//!
//! One binary serves the JSON the browser reads and, in a release build, the
//! built SPA beside it. That is the whole deployment: no process manager, no
//! reverse proxy, nothing to keep in sync.

pub mod advisor;
pub mod dto;
pub mod error;
pub mod live;
pub mod htf;
pub mod paper;
pub mod research;
pub mod routes;
pub mod state;
pub mod version;

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
        // What this binary was built FROM, so a deploy can prove that what is
        // answering is what was just shipped. The readiness probe used to
        // accept any 200 from /api/paper/status, which a binary from any
        // earlier build answers perfectly. See
        // docs/decisions/2026-09-17-deploy-staleness.md.
        .route("/api/version", get(version::version))
        .route("/api/chart/catalog", get(routes::catalog))
        .route("/api/chart/bars", get(routes::bars))
        .route("/api/chart/levels", get(routes::levels))
        .route("/api/chart/leaderboard", get(routes::leaderboard))
        .route("/api/chart/indicators", post(routes::indicators))
        .route("/api/chart/backtest", post(routes::backtest))
        .route("/api/live", get(live::stream))
        .route("/api/live/status", get(live::status))
        .route("/api/research", get(research::research))
        // The advisor's credentials. `store` is the only control on this desk
        // that can create spending power, so it alone costs the setup code
        // printed on this process's console; `clear` takes capability away and
        // costs nothing.
        .route("/api/advisor/credentials", get(advisor::credentials).post(advisor::store))
        .route("/api/advisor/credentials/test", post(advisor::test))
        .route("/api/advisor/credentials/clear", post(advisor::clear))
        .route("/api/paper/start", post(paper::start))
        .route("/api/paper/bar", post(paper::bar))
        .route("/api/paper/tick", post(paper::tick))
        // The open of a bar that has just started. Separate from `tick` on
        // purpose: this one can open a position, `tick` never touches a run.
        .route("/api/paper/open", post(paper::open_now))
        // Higher-timeframe facts, computed once for the model and the desk.
        .route("/api/paper/htf", get(htf::htf))
        .route("/api/paper/stop", post(paper::stop))
        .route("/api/paper/status", get(paper::status))
        .route("/api/paper/accounts", get(paper::accounts))
        .route("/api/paper/pause", post(paper::pause))
        .route("/api/paper/guards", get(paper::guards_get).post(paper::guards_set))
        .route("/api/paper/run/{id}", get(paper::detail))
        // What the models said about one book: the decider's own decisions and
        // the advisor panel's consultations, kept apart because they are two
        // different powers over a trade.
        .route("/api/paper/reasoning/{id}", get(paper::reasoning))
        // What ONE account did with one book, which is a different log from
        // what the book did - see the handler.
        .route("/api/paper/broker-events/{id}", get(paper::broker_events))
        // The advisor lives outside this process and speaks only through
        // these two: it reads what is about to happen and may ask for less.
        .route("/api/paper/pending", get(paper::pending))
        .route("/api/paper/advice", post(paper::advice))
        // An entry proposed from outside, for a run whose strategy is
        // `external`. Fills at the next bar like every rule; cannot arrive
        // late and still trade; cannot reach a broker.
        .route("/api/paper/intent", post(paper::intent))
        // The forming candle, pushed. Removes the last of the three polls
        // between a tick in the terminal and the chart on the screen.
        .route("/api/paper/stream", get(paper::stream))
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
            // ServeDir answers `/` with index.html itself, so the header goes on
            // the whole static service, applied only to HTML responses.
            let html_no_cache = |res: &axum::http::Response<_>| {
                let html = res
                    .headers()
                    .get(axum::http::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with("text/html"));
                html.then(|| axum::http::HeaderValue::from_static("no-cache"))
            };
            let statics = tower_http::set_header::SetResponseHeader::overriding(
                ServeDir::new(dir).fallback(ServeFile::new(index)),
                axum::http::header::CACHE_CONTROL,
                html_no_cache,
            );
            api.fallback_service(statics)
        }
        // Permissive CORS only in the no-SPA case, which is the dev setup where
        // the client is served from another port.
        None => api.layer(CorsLayer::permissive()),
    }
}
