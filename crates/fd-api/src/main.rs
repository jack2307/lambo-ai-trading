//! Serve the API, and the built client beside it when there is one.

use std::path::PathBuf;
use std::sync::Arc;

use fd_api::{AppState, router};
use fd_core::config::Config;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = arg("port", "8138").parse()?;
    let data = PathBuf::from(arg("data", "data"));
    let ui = PathBuf::from(arg("ui", "ui/dist"));
    let docs = PathBuf::from(arg("docs", "docs"));

    let config = Config::load(arg("config", "config"))?;
    let state = Arc::new(AppState::new(config, data.clone()).with_docs(docs));

    let serving_ui = ui.is_dir();
    let app = router(Arc::clone(&state), Some(ui.clone()));

    // Bound to loopback on purpose. This process reads a research store and
    // runs backtests on request; nothing about it is ready to face a network.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("fd-api on http://127.0.0.1:{port}  data={}", data.display());
    println!(
        "{}",
        if serving_ui {
            format!("serving the client from {}", ui.display())
        } else {
            format!("no client build at {} — API only (vite dev proxies here)", ui.display())
        }
    );
    axum::serve(listener, app).await?;
    Ok(())
}
