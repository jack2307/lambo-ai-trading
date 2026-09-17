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

    let config_dir = PathBuf::from(arg("config", "config"));
    let config = Config::load(&config_dir)?;
    let state = Arc::new(
        AppState::new(config, data.clone()).with_docs(docs).with_config_dir(config_dir),
    );

    // The scheduled-news calendar every `news:` filter reads, installed once
    // for the process. Missing or unreadable is not fatal: the filters are
    // then no-ops, and the line below says which case holds.
    let news_path = data.join("news").join("events.parquet");
    if news_path.is_file() {
        match fd_store::read_news(&news_path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            Ok(_) => {}
            Err(e) => println!("news: could not load {}: {e}", news_path.display()),
        }
    }
    println!("{}", fd_strategy::news::summary("data/news/events.parquet"));
    {
        let runs = state.paper.lock().expect("paper runs");
        if runs.is_empty() {
            println!("paper: no runs on disk under {}", data.join("paper").display());
        } else {
            let ids: Vec<&str> = runs.keys().map(String::as_str).collect();
            println!("paper: reloaded {} run(s): {}", runs.len(), ids.join(", "));
        }
    }

    let serving_ui = ui.is_dir();
    let app = router(Arc::clone(&state), Some(ui.clone()));

    // Bound to loopback on purpose. This process reads a research store and
    // runs backtests on request; nothing about it is ready to face a network.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    // Said at startup as well as served, because the log is what survives a
    // process that has already gone away - and "which build was running when
    // that happened" is the question nobody can answer afterwards.
    println!("fd-api {}", fd_api::version::summary());
    println!("fd-api on http://127.0.0.1:{port}  data={}", data.display());
    // The advisor credentials panel, and whether it is switched on at all.
    //
    // Printing the setup code while the group is OFF would be the worst of both
    // readings: it says a control exists that every route refuses. So the line
    // reports the state first, and the code only when it can be used.
    if std::env::var(fd_api::advisor::PANEL_ENV)
        .map(|v| !v.trim().is_empty() && v.trim() != "0" && v.trim() != "false")
        .unwrap_or(false)
    {
        println!(
            "advisor credentials panel: ON. Setup code {} (needed once, to store an API key)",
            fd_api::advisor::setup_code()
        );
    } else {
        println!(
            "advisor credentials panel: off. Set {}=1 and restart to enable it — off by \
             default because these routes are unauthenticated on loopback and one of them \
             deletes a credential a live campaign may depend on.",
            fd_api::advisor::PANEL_ENV
        );
    }
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
