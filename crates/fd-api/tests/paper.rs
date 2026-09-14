//! The paper endpoints on a synthetic store.
//!
//! Handlers are called directly with the JSON bodies the wire would carry,
//! so a test reads like a session: start, post bars, read the status, stop.
//! The store is a temporary directory with one Parquet file the way
//! `bar_source.rs` builds it.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use fd_api::paper::{bar, start, status, stop};
use fd_api::{ApiError, AppState};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::write_bars;
use serde_json::{Value, json};

const BAR: i64 = 900_000;
/// Midnight UTC, 2026-09-07 (a Monday).
const START: i64 = 1_788_000_000_000 / 86_400_000 * 86_400_000;

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

/// A sine wave with a wobble: enough crosses for `ema-cross` to trade
/// within a few dozen bars, deterministic so every run sees the same tape.
fn wave(i: usize) -> Bar {
    let t = i as f64;
    let close = 60_000.0 + 400.0 * (t / 40.0 * std::f64::consts::TAU).sin() + 25.0 * (t * 0.7).cos();
    let open = 60_000.0 + 400.0 * ((t - 1.0) / 40.0 * std::f64::consts::TAU).sin() + 25.0 * ((t - 1.0) * 0.7).cos();
    Bar { time: START + i as i64 * BAR, open, high: open.max(close) + 30.0, low: open.min(close) - 30.0, close, volume: Some(1.0) }
}

/// A store with `stored` bars of the wave for `btc`, and the state over it.
fn state_over(dir: &Path, stored: usize) -> Arc<AppState> {
    let bars: Vec<Bar> = (0..stored).map(wave).collect();
    write_bars(&dir.join("bars").join("BTCUSDT-15m.parquet"), &bars).expect("write");
    Arc::new(AppState::new(config(), dir.to_path_buf()))
}

async fn start_run(state: &Arc<AppState>, body: Value) -> Result<Value, ApiError> {
    let request = serde_json::from_value(body).expect("a start body");
    start(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

async fn post_bar(state: &Arc<AppState>, market: &str, tf: &str, b: Bar) -> Result<Value, ApiError> {
    let body = json!({ "market": market, "tf": tf, "bar": { "time": b.time, "open": b.open, "high": b.high, "low": b.low, "close": b.close, "volume": b.volume } });
    let request = serde_json::from_value(body).expect("a bar body");
    bar(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

async fn read_status(state: &Arc<AppState>) -> Value {
    let Json(v) = status(State(Arc::clone(state))).await.expect("status");
    serde_json::to_value(v).expect("json")
}

async fn stop_run(state: &Arc<AppState>, market: &str, tf: &str) -> Result<Value, ApiError> {
    let request = serde_json::from_value(json!({ "market": market, "tf": tf })).expect("a stop body");
    stop(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

fn http_status(err: ApiError) -> u16 {
    err.into_response().status().as_u16()
}

#[tokio::test]
async fn a_run_takes_bars_in_order_and_refuses_the_rest() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);

    let started = start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 })).await.expect("start");
    assert_eq!(started["id"], "btc-15m");
    assert_eq!(started["bars"], 200, "warm-up is the store's last `window` bars");
    assert_eq!(started["warmup_bars"], 200);
    assert_eq!(started["guards"], true, "guards are on unless asked off");
    assert_eq!(started["params"]["slow"], 55.0, "the full parameters after the overrides");
    assert!(dir.path().join("paper").join("btc-15m").join("state.json").is_file());

    // A second start for the same market and timeframe is a conflict.
    let again = start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross" })).await;
    assert_eq!(again.map(|_| ()).map_err(http_status), Err(409));

    // Fifty bars continuing the wave; each one accepted.
    let mut opened = 0;
    let mut closed = 0;
    for i in 300..350 {
        let reply = post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
        assert_eq!(reply["accepted"], true, "{reply}");
        opened += usize::from(reply["opened"] == true);
        closed += reply["closed"].as_array().map_or(0, Vec::len);
    }
    assert!(opened > 0, "the wave must produce a cross the run fills");

    // The last bar again is seen, not an error and not a step.
    let seen = post_bar(&state, "btc", "15m", wave(349)).await.expect("seen");
    assert_eq!(seen, json!({ "accepted": false, "reason": "seen", "bars": 200, "closed": [], "opened": false }));
    // An older bar is refused, a malformed one too, and an unknown run is 404.
    assert_eq!(post_bar(&state, "btc", "15m", wave(340)).await.map(|_| ()).map_err(http_status), Err(400));
    let mut bad = wave(350);
    bad.low = bad.high + 1.0;
    assert_eq!(post_bar(&state, "btc", "15m", bad).await.map(|_| ()).map_err(http_status), Err(400));
    assert_eq!(post_bar(&state, "btc", "1h", wave(350)).await.map(|_| ()).map_err(http_status), Err(404));

    let s = read_status(&state).await;
    let run = &s["runs"][0];
    assert_eq!(s["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(run["bars"], 200, "the window holds its size");
    assert_eq!(run["bars_seen"], 50);
    assert_eq!(run["last_bar_time"], wave(349).time);
    assert_eq!(run["trades"], closed);
    assert_eq!(run["gaps"], 0);
    assert_eq!(run["news"]["events_loaded"], 0);
    assert!(run["news"]["next_blackout"].is_null());
    assert_eq!(run["last_fills"].as_array().map(Vec::len), Some(closed.min(10)));
    let open_now = !run["open"].is_null();
    assert_eq!(open_now, opened > closed, "open iff more fills than closes");

    // A hole in the feed is recorded, not filled.
    let reply = post_bar(&state, "btc", "15m", wave(353)).await.expect("gap bar");
    assert_eq!(reply["gap"], 3, "350, 351 and 352 are missing");
    assert_eq!(read_status(&state).await["runs"][0]["gaps"], 1);

    // Stop: whatever is open closes at the last close as STOPPED.
    let stopped = stop_run(&state, "btc", "15m").await.expect("stop");
    assert_eq!(stopped["stopped"], true);
    let open_at_stop = !read_status(&state).await["runs"].as_array().is_some_and(|r| r.is_empty());
    assert!(!open_at_stop, "the run is gone after the stop");
    if !stopped["closed"].is_null() {
        assert_eq!(stopped["closed"]["exitReason"], "STOPPED");
        assert_eq!(stopped["closed"]["exitTime"], wave(353).time);
    }
    let run_dir = dir.path().join("paper").join("btc-15m");
    assert!(!run_dir.join("state.json").exists(), "a stopped run must not be reloaded");
    assert!(run_dir.join("final.json").is_file());
    let fills = std::fs::read_to_string(run_dir.join("fills.jsonl")).expect("fills");
    let kinds: Vec<String> = fills
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("a json line")["kind"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(kinds.first().map(String::as_str), Some("started"));
    assert_eq!(kinds.last().map(String::as_str), Some("stopped"));
    assert!(kinds.contains(&"gap".to_string()));
    assert_eq!(kinds.iter().filter(|k| *k == "trade").count(), stopped["trades"].as_u64().unwrap_or(0) as usize);

    // Stopping again is 404.
    assert_eq!(stop_run(&state, "btc", "15m").await.map(|_| ()).map_err(http_status), Err(404));
}

#[tokio::test]
async fn stop_closes_a_self_managed_hold_at_the_last_close() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 20);
    // Buy-and-hold: long on the first bar after warm-up, never exits.
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "buy-and-hold", "window": 50, "guards": false }))
        .await
        .expect("start");
    // Bar 20 decides, bar 21 fills at its open, bar 22 is held through.
    for i in 20..23 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let s = read_status(&state).await;
    let open = &s["runs"][0]["open"];
    assert_eq!(open["side"], "LONG");
    assert_eq!(open["entry_time"], wave(21).time);
    assert!(open["stop"].is_null(), "a self-managed hold carries no stop");
    assert!(open["unrealised_usd_at_last_close"].is_number());

    let stopped = stop_run(&state, "btc", "15m").await.expect("stop");
    assert_eq!(stopped["closed"]["exitReason"], "STOPPED");
    assert_eq!(stopped["closed"]["exitTime"], wave(22).time);
    assert_eq!(stopped["trades"], 1);
    // Exit at the last close less half the spread: the engine's costs.
    let spread = config().market("btc").expect("btc").trading.spread;
    let exit = stopped["closed"]["exitPrice"].as_f64().expect("exit");
    assert!((exit - (wave(22).close - spread / 2.0)).abs() < 0.01, "{exit}");
}

#[tokio::test]
async fn the_state_file_survives_a_restart() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200, "filters": ["weekdays"] }))
        .await
        .expect("start");
    for i in 300..340 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let before = read_status(&state).await;
    assert_eq!(before["runs"][0]["filters"], json!(["weekdays"]));

    // A restart is a new state over the same data directory.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let after = read_status(&reborn).await;
    assert_eq!(after, before, "the reloaded run reports what the live one did");

    // And it keeps going: the next bar is accepted, the previous is seen.
    assert_eq!(post_bar(&reborn, "btc", "15m", wave(339)).await.expect("seen")["reason"], "seen");
    assert_eq!(post_bar(&reborn, "btc", "15m", wave(340)).await.expect("next")["accepted"], true);
    assert_eq!(read_status(&reborn).await["runs"][0]["bars_seen"], 41);
}

#[tokio::test]
async fn a_start_is_checked_like_a_backtest_request() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 100);
    let bad = |body: Value| async {
        start_run(&state, body).await.map(|_| ()).map_err(http_status)
    };
    assert_eq!(bad(json!({ "market": "btc", "tf": "15m", "strategy": "nothing" })).await, Err(400));
    assert_eq!(bad(json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "params": { "nope": 1 } })).await, Err(400));
    assert_eq!(bad(json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "filters": ["hours:2500-0100"] })).await, Err(400));
    assert_eq!(bad(json!({ "market": "btc", "tf": "3m", "strategy": "ema-cross" })).await, Err(400));
    assert_eq!(bad(json!({ "market": "nowhere", "tf": "15m", "strategy": "ema-cross" })).await, Err(400));
    assert_eq!(bad(json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 10 })).await, Err(400), "too short to ever trade");
    assert_eq!(bad(json!({ "market": "btc", "tf": "15m", "strategy": "flow-momentum" })).await, Err(400), "needs an options frame");
    // With an empty store the run still starts, with no warm-up bars.
    let empty = tempfile::tempdir().expect("temp dir");
    let bare = Arc::new(AppState::new(config(), empty.path().to_path_buf()));
    let started = start_run(&bare, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross" })).await.expect("start");
    assert_eq!(started["bars"], 0);
    assert!(read_status(&state).await["runs"].as_array().is_some_and(Vec::is_empty));
}
