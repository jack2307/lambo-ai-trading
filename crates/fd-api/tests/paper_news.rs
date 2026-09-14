//! The paper status's `next_blackout` is the run's market's business only.
//!
//! Its own test binary: the calendar behind `fd_strategy::news` is a
//! process-wide `OnceLock`, and `tests/paper.rs` asserts the
//! nothing-installed state (`events_loaded: 0`). This binary installs one
//! calendar and reads the status of runs on two markets against it.
//!
//! The runs start on an empty store (no warm-up bars), which `start`
//! allows; nothing here posts a bar.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use fd_api::AppState;
use fd_api::paper::{start, status};
use fd_core::config::Config;
use fd_core::types::NewsEvent;
use serde_json::{Value, json};

const HOUR: i64 = 3_600_000;

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Ahead of the wall clock, so every event is a *next* blackout: a
/// Canadian rate decision first, then a medium USD print, then FOMC, then a
/// global holiday.
fn calendar(now: i64) -> Vec<NewsEvent> {
    vec![
        NewsEvent { time: now + HOUR, impact: 3, currency: "CAD".into(), name: "BoC Rate Statement".into() },
        NewsEvent { time: now + 2 * HOUR, impact: 2, currency: "USD".into(), name: "Existing Home Sales".into() },
        NewsEvent { time: now + 3 * HOUR, impact: 3, currency: "USD".into(), name: "FOMC Statement".into() },
        NewsEvent { time: now + 4 * HOUR, impact: 3, currency: "All".into(), name: "Bank Holiday".into() },
        NewsEvent { time: now + 5 * HOUR, impact: 3, currency: "EUR".into(), name: "ECB Press Conference".into() },
    ]
}

async fn start_run(state: &Arc<AppState>, body: Value) -> Value {
    let request = serde_json::from_value(body).expect("a start body");
    let Json(status) = start(State(Arc::clone(state)), Json(request)).await.expect("start");
    serde_json::to_value(status).expect("json")
}

async fn read_status(state: &Arc<AppState>) -> Value {
    let Json(v) = status(State(Arc::clone(state))).await.expect("status");
    serde_json::to_value(v).expect("json")
}

#[tokio::test]
async fn the_next_blackout_is_the_first_release_of_the_markets_own_currencies() {
    let now = now_ms();
    fd_strategy::news::install(calendar(now)).expect("this binary's one calendar");
    let dir = tempfile::tempdir().expect("temp dir");
    let state = Arc::new(AppState::new(config(), dir.path().to_path_buf()));

    // Gold reads USD: the CAD decision an hour out is skipped, the medium
    // USD print is under the guards' threshold, FOMC is the answer — and
    // the status says what it is.
    let gold = start_run(&state, json!({ "market": "xauusd", "tf": "15m", "strategy": "ema-cross" })).await;
    assert_eq!(gold["news"]["events_loaded"], 5);
    let next = &gold["news"]["next_blackout"];
    assert_eq!(next["currency"], "USD", "{next}");
    assert_eq!(next["impact"], 3);
    assert_eq!(next["name"], "FOMC Statement");
    assert_eq!(next["time"], now + 3 * HOUR);

    // The same through the status endpoint, and with guards off the scope
    // is still the market's (it is the rules', not the guards').
    let silver = start_run(&state, json!({ "market": "xagduka", "tf": "15m", "strategy": "ema-cross", "guards": false })).await;
    assert_eq!(silver["news"]["next_blackout"]["name"], "FOMC Statement");
    let s = read_status(&state).await;
    let runs = s["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2);
    for run in runs {
        assert_eq!(run["news"]["next_blackout"]["currency"], "USD", "{run}");
        assert_eq!(run["news"]["next_blackout"]["name"], "FOMC Statement");
    }

    // EURUSD reads USD and EUR: still FOMC first (the ECB is later), and
    // never the CAD one.
    let eur = start_run(&state, json!({ "market": "eurduka", "tf": "15m", "strategy": "ema-cross" })).await;
    assert_eq!(eur["news"]["next_blackout"]["name"], "FOMC Statement");
    assert_ne!(eur["news"]["next_blackout"]["currency"], "CAD");
}
