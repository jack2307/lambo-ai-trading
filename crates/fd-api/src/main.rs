//! Serve the API, and the built client beside it when there is one.
//!
//! On **two** listeners, and the difference between them is the whole of this
//! process's security model. 8138 is the desk's own bus: loopback,
//! unauthenticated, called constantly by the Python processes that keep the
//! books moving, and unchanged by any of this. The second one — off unless
//! somebody switches it on — refuses every request that does not carry a
//! session, and it is the one a Cloudflare tunnel is pointed at.
//! `crates/fd-api/src/auth.rs` explains why that had to be a second port
//! rather than a rule about the peer's address; the short version is that
//! `cloudflared` runs here and everything it forwards looks local.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fd_api::auth::{self, Tunnel};
use fd_api::{AppState, authed_router, router};
use fd_core::config::Config;

fn arg(name: &str, fallback: &str) -> String {
    opt(name).unwrap_or_else(|| fallback.to_string())
}

/// `--<name>=<value>`, absent rather than defaulted — for the switches where
/// "not given" and "given the usual value" are different decisions.
fn opt(name: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
}

/// A bare `--<name>`.
fn flag(name: &str) -> bool {
    let want = format!("--{name}");
    std::env::args().any(|a| a == want)
}

/// The repository root, from the config directory.
///
/// `config` is usually relative, and its parent is then the EMPTY path rather
/// than `None`. `advisor::root` names the same trap; this is the second place
/// to meet it.
fn repo_root(config_dir: &Path) -> PathBuf {
    match config_dir.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = PathBuf::from(arg("config", "config"));

    // Before anything else. This reads no store, binds no port and touches
    // nothing the desk is using, so it must not depend on a startup that a
    // half-configured machine cannot finish.
    if flag("set-password") {
        return match auth::set_password_command(&repo_root(&config_dir), &config_dir) {
            Ok(()) => Ok(()),
            Err(why) => {
                println!("{why}");
                std::process::exit(2);
            }
        };
    }

    let port: u16 = arg("port", "8138").parse()?;
    let data = PathBuf::from(arg("data", "data"));
    let ui = PathBuf::from(arg("ui", "ui/dist"));
    let docs = PathBuf::from(arg("docs", "docs"));

    let config = Config::load(&config_dir)?;
    let state = Arc::new(
        AppState::new(config, data.clone()).with_docs(docs).with_config_dir(config_dir.clone()),
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

    // The second listener, decided before anything is printed so the two lines
    // about it arrive together. Bound HERE rather than inside the spawned
    // task, so a port already in use is a startup error somebody sees and not
    // a message on a console nobody is reading.
    // Parsed with a hard failure rather than `.ok()`: a typo in `--auth-port`
    // that silently fell back to "nobody asked" would leave the listener shut
    // while its owner watched a tunnel fail to connect.
    let cli_auth_port: Option<u16> = match opt("auth-port") {
        None => None,
        Some(text) => Some(text.parse::<u16>().map_err(|e| format!("--auth-port={text}: {e}")).and_then(
            |n| if n == 0 { Err("--auth-port=0 is not a port".to_string()) } else { Ok(n) },
        )?),
    };
    let tunnel = auth::tunnel(&config_dir, cli_auth_port, flag("auth"));
    let authed = match &tunnel {
        Tunnel::Ready { port, auth } => {
            let bound = tokio::net::TcpListener::bind(("127.0.0.1", *port)).await?;
            Some((bound, authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(auth))))
        }
        // Refused and Off both bind nothing. There is no branch here that
        // opens a listener without a password, and there must never be one.
        Tunnel::Refused { .. } | Tunnel::Off => None,
    };
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

    // The second listener's state, said out loud in every case. A refusal in
    // particular is shouted rather than logged quietly: the operator who asked
    // for it will otherwise point a tunnel at a port nothing is listening on
    // and spend the evening on Cloudflare's side of the problem.
    match &tunnel {
        Tunnel::Ready { port: authed_port, .. } => {
            println!(
                "authenticated listener on http://127.0.0.1:{authed_port} — every request needs \
                 a session, including from loopback. Point the tunnel here, never at {port}."
            );
        }
        Tunnel::Refused { port: authed_port, why } => {
            println!("!! the authenticated listener on {authed_port} DID NOT START: {why}");
            println!(
                "!! it was asked for and there is no usable password, so nothing was bound. It \
                 is never opened unauthenticated. Port {port} is serving as usual."
            );
        }
        Tunnel::Off => {
            println!(
                "authenticated listener: off. `fd-api --set-password`, then restart with --auth \
                 (default port {}), and point the Cloudflare tunnel at that — {port} has no \
                 authentication and must not be exposed.",
                auth::DEFAULT_AUTH_PORT
            );
        }
    }

    // Spawned rather than joined: if the authed listener ever falls over, the
    // desk's own bus must keep serving. A book that stops because a web login
    // stopped is a worse outcome than a tunnel that needs a restart.
    if let Some((bound, app)) = authed {
        tokio::spawn(async move {
            if let Err(e) = axum::serve(bound, app).await {
                println!("!! the authenticated listener stopped: {e}");
            }
        });
    }
    axum::serve(listener, app).await?;
    Ok(())
}
