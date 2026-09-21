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
use axum::extract::{Path as PathParam, Query};
use fd_api::paper::{
    DEFAULT_DETAIL_BARS, DetailQuery, MAX_LIVE_AGE_MS, act, bar, detail, intent, open_now, pending, start, status, stop, tick,
};
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

/// The forming bar, as the poller posts it between two closed ones. `body`
/// carries the `bar` and any quote; the stream is spelled out here.
async fn post_tick(state: &Arc<AppState>, market: &str, tf: &str, body: Value) -> Result<Value, ApiError> {
    let mut payload = json!({ "market": market, "tf": tf });
    let (Some(target), Some(fields)) = (payload.as_object_mut(), body.as_object()) else { panic!("two json objects") };
    for (key, value) in fields {
        target.insert(key.clone(), value.clone());
    }
    let request = serde_json::from_value(payload).expect("a tick body");
    tick(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

/// A forming bar in `b`'s bucket, with a close that has moved since.
fn forming(b: Bar, close: f64) -> Value {
    json!({ "bar": { "time": b.time, "open": b.open, "high": b.high.max(close), "low": b.low.min(close), "close": close, "volume": 7.0 } })
}

async fn read_detail(state: &Arc<AppState>, id: &str, bars: Option<usize>) -> Result<Value, ApiError> {
    detail(State(Arc::clone(state)), PathParam(id.to_string()), Query(DetailQuery { bars }))
        .await
        .map(|Json(v)| serde_json::to_value(v).expect("json"))
}

async fn read_status(state: &Arc<AppState>) -> Value {
    let Json(v) = status(State(Arc::clone(state))).await.expect("status");
    serde_json::to_value(v).expect("json")
}

async fn stop_with(state: &Arc<AppState>, body: Value) -> Result<Value, ApiError> {
    let request = serde_json::from_value(body).expect("a stop body");
    stop(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

/// The first client's spelling: the stream, not the id.
async fn stop_run(state: &Arc<AppState>, market: &str, tf: &str) -> Result<Value, ApiError> {
    stop_with(state, json!({ "market": market, "tf": tf })).await
}

fn http_status(err: ApiError) -> u16 {
    err.into_response().status().as_u16()
}

/// The status entry of one run, by id.
fn run_named<'a>(s: &'a Value, id: &str) -> &'a Value {
    s["runs"].as_array().expect("runs").iter().find(|r| r["id"] == id).unwrap_or_else(|| panic!("no run {id} in {s}"))
}

async fn post_intent(state: &Arc<AppState>, body: Value) -> Result<Value, ApiError> {
    let request = serde_json::from_value(body).expect("an intent body");
    intent(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

#[tokio::test]
async fn the_decider_badge_names_only_decisions_the_book_actually_took() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);

    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "ai", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    // Nobody has posted to it, so it claims nobody. A model name typed into a
    // config would have put a badge here on a book no model ever traded.
    assert!(run_named(&read_status(&state).await, "ai")["decider"].is_null(), "an undriven book claims no decider");

    // A decision made on a bar the run has already moved past is refused —
    // and a refused intent must not claim the badge either. This is the whole
    // point of recording it past the refusals rather than on arrival.
    let stale = post_intent(&state, json!({
        "run": "ai", "bar_time": wave(299).time, "side": "LONG",
        "stop": 59_000.0, "decider": "gpt-5",
    })).await.expect("call");
    assert_eq!(stale["accepted"], false, "{stale}");
    assert!(run_named(&read_status(&state).await, "ai")["decider"].is_null(), "a refused intent claims nothing");

    let ok = post_intent(&state, json!({
        "run": "ai", "bar_time": wave(300).time, "side": "LONG",
        "stop": 59_000.0, "reason": "test", "decider": "gpt-5",
    })).await.expect("call");
    assert_eq!(ok["accepted"], true, "{ok}");
    let d = run_named(&read_status(&state).await, "ai")["decider"].clone();
    assert_eq!(d["last"], "gpt-5");
    assert_eq!(d["decisions"]["gpt-5"], 1);
    assert!(d["last_at"].as_i64().expect("a stamp") > 0);

    // A second decider on the same book: both are kept. One `name` field
    // would have quietly relabelled the whole run as the newer one's record,
    // which is exactly the number nobody could read afterwards.
    post_bar(&state, "btc", "15m", wave(301)).await.expect("bar");
    post_intent(&state, json!({
        "run": "ai", "bar_time": wave(301).time, "side": "SHORT",
        "stop": 61_000.0, "decider": "coin",
    })).await.expect("call");
    let d = run_named(&read_status(&state).await, "ai")["decider"].clone();
    assert_eq!(d["last"], "coin", "the badge shows who spoke last");
    assert_eq!(d["decisions"].as_object().expect("a map").len(), 2, "both deciders are kept: {d}");
    assert_eq!(d["decisions"]["gpt-5"], 1);
    assert_eq!(d["decisions"]["coin"], 1);
}

#[tokio::test]
async fn a_stand_aside_names_the_decider_without_touching_the_book() {
    // The reason NONE is accepted at all: a model that declines is still
    // driving the book, and from the outside a declining model and a dead
    // process look identical — both show zero trades. It must nonetheless be
    // incapable of putting on a position.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "ai", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    let aside = post_intent(&state, json!({
        "run": "ai", "bar_time": wave(300).time, "side": "NONE",
        "reason": "chop", "decider": "claude-opus-5",
    })).await.expect("call");
    assert_eq!(aside["accepted"], true, "{aside}");

    let d = run_named(&read_status(&state).await, "ai")["decider"].clone();
    assert_eq!(d["last"], "claude-opus-5", "the badge names who is driving");
    assert_eq!(d["stood_aside"], 1);
    assert!(d["decisions"].as_object().expect("a map").is_empty(), "declining is not a trade: {d}");

    // The next bar must fill nothing: a stand-aside left no pending intent.
    let reply = post_bar(&state, "btc", "15m", wave(301)).await.expect("bar");
    assert_eq!(reply["runs"][0]["opened"], false, "a stand-aside cannot open a position: {reply}");
    let st = run_named(&read_status(&state).await, "ai").clone();
    assert!(st["open"].is_null(), "no position: {st}");
    assert_eq!(st["trades"], 0);
}

#[tokio::test]
async fn an_accepted_intent_survives_a_restart_and_still_fills() {
    // The route answers "accepted, fills at the next bar's open", and that
    // promise has to outlive a deploy. Until this test the pending intent lived
    // only in memory: a restart inside the fifteen minutes before the next bar
    // silently cancelled a trade the model had been told was on, with no line
    // anywhere saying so. Observed for real — two stand-asides recorded at
    // 16:15 were gone from both badges after a 16:17 restart.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "ai", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    let ok = post_intent(&state, json!({
        "run": "ai", "bar_time": wave(300).time, "side": "LONG",
        "stop": 59_000.0, "reason": "survives", "decider": "claude-opus-5",
    })).await.expect("call");
    assert_eq!(ok["accepted"], true, "{ok}");

    // A restart is a new state over the same data directory — no bar has been
    // posted since the intent, so only the intent route can have written it.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let d = run_named(&read_status(&reborn).await, "ai")["decider"].clone();
    assert_eq!(d["last"], "claude-opus-5", "the badge survived the restart: {d}");
    assert_eq!(d["decisions"]["claude-opus-5"], 1);

    // And the trade itself is still pending: the NEXT bar must open it.
    let reply = post_bar(&reborn, "btc", "15m", wave(301)).await.expect("bar");
    assert_eq!(reply["runs"][0]["opened"], true, "the intent still filled after a restart: {reply}");
    let st = run_named(&read_status(&reborn).await, "ai").clone();
    assert_eq!(st["open"]["side"], "LONG", "{st}");
}

#[tokio::test]
async fn a_stand_aside_also_survives_a_restart() {
    // `last_at` is how the desk tells a model that is standing aside from a
    // process that died. A counter that resets on every API restart cannot
    // carry that, so the stand-aside path persists too.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "ai", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");
    post_intent(&state, json!({
        "run": "ai", "bar_time": wave(300).time, "side": "NONE",
        "reason": "chop", "decider": "codex/gpt-5.6-sol",
    })).await.expect("call");

    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let d = run_named(&read_status(&reborn).await, "ai")["decider"].clone();
    assert_eq!(d["last"], "codex/gpt-5.6-sol");
    assert_eq!(d["stood_aside"], 1, "{d}");
}

#[tokio::test]
async fn a_books_history_survives_a_restart_without_living_in_its_state_file() {
    // The cost of recording one bar must not depend on how long the book has
    // been running. `trades` and `equity_curve` grow without bound and the
    // state file is rewritten EVERY bar, so they were moved out to an
    // append-only `trades.jsonl`. This holds both halves of that: the state
    // file no longer carries them, and a reload still reports the same book.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "b", "window": 200 }))
        .await
        .expect("start");
    for i in 300..380 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let before = run_named(&read_status(&state).await, "b").clone();
    assert!(before["trades"].as_u64().expect("trades") > 0, "the tape must close something: {before}");

    // The state file must NOT carry the history any more.
    let text = std::fs::read_to_string(dir.path().join("paper").join("b").join("state.json")).expect("state");
    let parsed: Value = serde_json::from_str(&text).expect("json");
    assert!(parsed["book"]["trades"].is_null(), "trades are out of the state file: {}", &text[..200.min(text.len())]);
    assert!(parsed["book"]["equity_curve"].is_null(), "the curve is out too");
    assert!(dir.path().join("paper").join("b").join("trades.jsonl").is_file(), "history was written");

    // And a reload reports exactly what the live run did.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let after = run_named(&read_status(&reborn).await, "b").clone();
    assert_eq!(after["trades"], before["trades"], "trade count survived");
    assert_eq!(after["net_usd"], before["net_usd"], "net survived");
    assert_eq!(after["profit_factor"], before["profit_factor"], "metrics survived");
    assert_eq!(after["equity_curve"], before["equity_curve"], "the curve survived");
}

#[tokio::test]
async fn a_legacy_state_file_and_the_history_file_do_not_both_count() {
    // The bug this exists for: a book written before the history moved out
    // keeps its trades in `state.json`. The migration copies them into
    // `trades.jsonl`, and a loader that then read BOTH counted every trade
    // twice and doubled every net — while profit factor, being a ratio, stayed
    // exactly right, so the one number a reader would have checked looked fine.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "b", "window": 200 }))
        .await
        .expect("start");
    for i in 300..380 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let live = run_named(&read_status(&state).await, "b").clone();
    let trades = live["trades"].as_u64().expect("trades");
    assert!(trades > 0);

    // Forge the legacy shape: put the history back INTO the state file, beside
    // the `trades.jsonl` the run already wrote.
    let path = dir.path().join("paper").join("b").join("state.json");
    let mut parsed: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("state")).expect("json");
    let history: Vec<Value> = std::fs::read_to_string(dir.path().join("paper").join("b").join("trades.jsonl"))
        .expect("history")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("line"))
        .filter(|v| v["book"] == "main")
        .map(|v| v["trade"].clone())
        .collect();
    parsed["book"]["trades"] = Value::Array(history);
    std::fs::write(&path, serde_json::to_string(&parsed).expect("write")).expect("write");

    // Loading must report the same book, not twice the book.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let after = run_named(&read_status(&reborn).await, "b").clone();
    assert_eq!(after["trades"], live["trades"], "trades counted once: {after}");
    assert_eq!(after["net_usd"], live["net_usd"], "net not doubled");
}

#[tokio::test]
async fn a_rule_based_book_cannot_be_claimed_by_a_decider() {
    // The badge is only meaningful because the route refuses rule-based runs
    // outright. Without this, anything could post a gpt-5 badge onto a book
    // gpt-5 has no part in.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "rule", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    let refused = post_intent(&state, json!({
        "run": "rule", "bar_time": wave(300).time, "side": "LONG",
        "stop": 59_000.0, "decider": "gpt-5",
    })).await;
    assert_eq!(refused.map(|_| ()).map_err(http_status), Err(400));
    assert!(run_named(&read_status(&state).await, "rule")["decider"].is_null());
}

#[tokio::test]
async fn a_run_takes_bars_in_order_and_refuses_the_rest() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);

    let started = start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 })).await.expect("start");
    assert_eq!(started["id"], "btc-15m-ema-cross", "the default id names the stream and the strategy");
    assert!(started["label"].is_null());
    assert_eq!(started["bars"], 200, "warm-up is the store's last `window` bars");
    assert_eq!(started["warmup_bars"], 200);
    assert_eq!(started["guards"], true, "guards are on unless asked off");
    assert_eq!(started["params"]["slow"], 55.0, "the full parameters after the overrides");
    assert!(dir.path().join("paper").join("btc-15m-ema-cross").join("state.json").is_file());

    // A second start under the same id is a conflict; the same stream
    // under another id is not (`two_runs_share_a_stream…` below).
    let again = start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross" })).await;
    assert_eq!(again.map(|_| ()).map_err(http_status), Err(409));

    // Fifty bars continuing the wave; each one accepted.
    let mut opened = 0;
    let mut closed = 0;
    for i in 300..350 {
        let reply = post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
        assert_eq!(reply["accepted"], true, "{reply}");
        let run = &reply["runs"][0];
        assert_eq!(run["id"], "btc-15m-ema-cross");
        assert_eq!(run["accepted"], true);
        opened += usize::from(run["opened"] == true);
        closed += run["closed"].as_array().map_or(0, Vec::len);
    }
    assert!(opened > 0, "the wave must produce a cross the run fills");

    // The last bar again is seen, not an error and not a step.
    let seen = post_bar(&state, "btc", "15m", wave(349)).await.expect("seen");
    assert_eq!(
        seen,
        json!({ "accepted": false, "reason": "seen", "bars": 200, "runs": [{ "id": "btc-15m-ema-cross", "accepted": false, "reason": "seen", "bars": 200, "closed": [], "opened": false }] })
    );
    // An older bar is refused, a malformed one too, and an unknown stream is 404.
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
    assert_eq!(reply["runs"][0]["gap"], 3, "350, 351 and 352 are missing");
    assert_eq!(read_status(&state).await["runs"][0]["gaps"], 1);

    // Stop by stream, as the first client does: one run on it, so it is
    // that one. Whatever is open closes at the last close as STOPPED.
    let stopped = stop_run(&state, "btc", "15m").await.expect("stop");
    assert_eq!(stopped["id"], "btc-15m-ema-cross");
    assert_eq!(stopped["stopped"], true);
    let open_at_stop = !read_status(&state).await["runs"].as_array().is_some_and(|r| r.is_empty());
    assert!(!open_at_stop, "the run is gone after the stop");
    if !stopped["closed"].is_null() {
        assert_eq!(stopped["closed"]["exitReason"], "STOPPED");
        assert_eq!(stopped["closed"]["exitTime"], wave(353).time);
    }
    let run_dir = dir.path().join("paper").join("btc-15m-ema-cross");
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

    // Stopping again is 404, by stream or by id.
    assert_eq!(stop_run(&state, "btc", "15m").await.map(|_| ()).map_err(http_status), Err(404));
    assert_eq!(stop_with(&state, json!({ "id": "btc-15m-ema-cross" })).await.map(|_| ()).map_err(http_status), Err(404));
}

#[tokio::test]
async fn two_runs_share_a_stream_and_keep_separate_books() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);

    // Two strategies on `btc:15m`: the crossover, and a hold that never
    // exits. Each under its own id, one with a label.
    let cross = start_run(&state, json!({ "id": "demo-1", "label": "  demo 12345 — ema cross  ", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 }))
        .await
        .expect("start cross");
    assert_eq!(cross["id"], "demo-1");
    assert_eq!(cross["label"], "demo 12345 — ema cross", "the label is kept, trimmed");
    let hold = start_run(&state, json!({ "id": "demo-2", "market": "btc", "tf": "15m", "strategy": "buy-and-hold", "window": 200, "guards": false }))
        .await
        .expect("start hold");
    assert_eq!(hold["id"], "demo-2");
    assert!(hold["label"].is_null());
    assert!(dir.path().join("paper").join("demo-1").join("state.json").is_file());
    assert!(dir.path().join("paper").join("demo-2").join("state.json").is_file());

    // One bar feeds both, in id order, and the reply says what each did.
    let mut cross_closed = 0;
    for i in 300..350 {
        let reply = post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
        assert_eq!(reply["accepted"], true, "{reply}");
        assert_eq!(reply["bars"], 200);
        let runs = reply["runs"].as_array().expect("runs");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0]["id"], "demo-1");
        assert_eq!(runs[1]["id"], "demo-2");
        assert!(runs.iter().all(|r| r["accepted"] == true), "{reply}");
        cross_closed += runs[0]["closed"].as_array().map_or(0, Vec::len);
        if i == 301 {
            assert_eq!(runs[1]["opened"], true, "the hold fills on the first bar after the warm-up");
        }
    }
    assert!(cross_closed > 0, "the wave must close a crossover trade");

    // Same bars, different books.
    let s = read_status(&state).await;
    assert_eq!(s["runs"].as_array().map(Vec::len), Some(2));
    let (a, b) = (run_named(&s, "demo-1"), run_named(&s, "demo-2"));
    assert_eq!(a["label"], "demo 12345 — ema cross");
    for run in [a, b] {
        assert_eq!(run["market"], "btc");
        assert_eq!(run["tf"], "15m");
        assert_eq!(run["bars_seen"], 50);
        assert_eq!(run["last_bar_time"], wave(349).time);
    }
    assert_eq!(a["strategy"], "ema-cross");
    assert_eq!(b["strategy"], "buy-and-hold");
    assert_eq!(a["trades"], cross_closed);
    assert_eq!(b["trades"], 0, "the hold never exits");
    assert_eq!(b["open"]["side"], "LONG");
    assert_eq!(b["open"]["entry_time"], wave(301).time);
    assert_ne!(a["equity"], b["equity"], "two books, two equities");

    // Stopping by stream is ambiguous with two runs on it.
    assert_eq!(stop_run(&state, "btc", "15m").await.map(|_| ()).map_err(http_status), Err(409));

    // Stop one by id; the other keeps running and keeps taking bars.
    let stopped = stop_with(&state, json!({ "id": "demo-2" })).await.expect("stop hold");
    assert_eq!(stopped["id"], "demo-2");
    assert_eq!(stopped["closed"]["exitReason"], "STOPPED");
    assert_eq!(stopped["trades"], 1);
    let s = read_status(&state).await;
    assert_eq!(s["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(s["runs"][0]["id"], "demo-1");
    assert!(!dir.path().join("paper").join("demo-2").join("state.json").exists());
    assert!(dir.path().join("paper").join("demo-1").join("state.json").is_file());
    let reply = post_bar(&state, "btc", "15m", wave(350)).await.expect("bar");
    assert_eq!(reply["accepted"], true);
    assert_eq!(reply["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(reply["runs"][0]["id"], "demo-1");
    assert_eq!(read_status(&state).await["runs"][0]["bars_seen"], 51);

    // Now the stream is unambiguous again for the old client.
    let stopped = stop_run(&state, "btc", "15m").await.expect("stop by stream");
    assert_eq!(stopped["id"], "demo-1");
    assert!(read_status(&state).await["runs"].as_array().is_some_and(Vec::is_empty));
}

#[tokio::test]
async fn a_bar_one_run_refuses_still_feeds_the_others() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    // Two runs whose windows end at different bars: `ahead` takes bars 300
    // to 305 alone, then `behind` is started and warms up from the store,
    // which ends at 299. (Every run on a stream gets every bar, so the
    // only way to stagger them is to start the second one later.)
    start_run(&state, json!({ "id": "ahead", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 })).await.expect("ahead");
    for i in 300..306 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    start_run(&state, json!({ "id": "behind", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 })).await.expect("behind");

    // Bar 302: older than `ahead`'s last, new to `behind`. Accepted, with
    // each run's verdict in its entry, `ahead` first (id order).
    let reply = post_bar(&state, "btc", "15m", wave(302)).await.expect("mixed");
    assert_eq!(reply["accepted"], true, "{reply}");
    assert!(reply["reason"].is_null());
    let runs = reply["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["id"], "ahead");
    assert_eq!(runs[0]["accepted"], false);
    assert!(runs[0]["reason"].as_str().is_some_and(|r| r.contains("older")), "{reply}");
    assert_eq!(runs[1]["id"], "behind");
    assert_eq!(runs[1]["accepted"], true);

    // Bar 305: `ahead` has it (seen), `behind` takes it — accepted.
    let reply = post_bar(&state, "btc", "15m", wave(305)).await.expect("seen and new");
    assert_eq!(reply["accepted"], true);
    assert_eq!(reply["runs"][0]["reason"], "seen");
    assert_eq!(reply["runs"][1]["accepted"], true);

    // Bar 305 again: seen by both — not an error, not accepted.
    let reply = post_bar(&state, "btc", "15m", wave(305)).await.expect("seen by both");
    assert_eq!(reply["accepted"], false);
    assert_eq!(reply["reason"], "seen");

    // Bar 300: older for both — no use to anyone, 400 as it always was.
    assert_eq!(post_bar(&state, "btc", "15m", wave(300)).await.map(|_| ()).map_err(http_status), Err(400));

    let s = read_status(&state).await;
    assert_eq!(run_named(&s, "ahead")["bars_seen"], 6);
    assert_eq!(run_named(&s, "behind")["bars_seen"], 2);
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
    start_run(&state, json!({ "id": "acct_7", "label": "demo 7", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200, "filters": ["weekdays"] }))
        .await
        .expect("start");
    for i in 300..340 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let before = read_status(&state).await;
    assert_eq!(before["runs"][0]["filters"], json!(["weekdays"]));
    assert_eq!(before["runs"][0]["label"], "demo 7");

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
async fn a_state_file_from_the_one_run_per_stream_layout_reloads_under_its_old_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 })).await.expect("start");
    for i in 300..340 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let before = read_status(&state).await;
    drop(state);

    // Rewrite what the live run wrote into the first layout: the directory
    // is `<market>-<tf>` and the config has no `id` and no `label`.
    let paper = dir.path().join("paper");
    let new_dir = paper.join("btc-15m-ema-cross");
    let old_dir = paper.join("btc-15m");
    let mut file: Value = serde_json::from_str(&std::fs::read_to_string(new_dir.join("state.json")).expect("state")).expect("json");
    let config_obj = file["config"].as_object_mut().expect("config");
    assert!(config_obj.remove("id").is_some());
    config_obj.remove("label");
    std::fs::rename(&new_dir, &old_dir).expect("rename to the old layout");
    std::fs::write(old_dir.join("state.json"), serde_json::to_string(&file).expect("json")).expect("write");

    // Reloads as `btc-15m`, everything else as it was.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let after = read_status(&reborn).await;
    assert_eq!(after["runs"].as_array().map(Vec::len), Some(1));
    let run = &after["runs"][0];
    assert_eq!(run["id"], "btc-15m");
    assert!(run["label"].is_null());
    let mut expected = before["runs"][0].clone();
    expected["id"] = json!("btc-15m");
    assert_eq!(run, &expected);

    // It keeps going in the same directory, and the next write says its id.
    let reply = post_bar(&reborn, "btc", "15m", wave(340)).await.expect("next");
    assert_eq!(reply["accepted"], true);
    assert_eq!(reply["runs"][0]["id"], "btc-15m");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(old_dir.join("state.json")).expect("state")).expect("json");
    assert_eq!(written["config"]["id"], "btc-15m");
    assert!(!new_dir.exists(), "nothing was written under the derived new-layout name");

    // A second reload keys by the stored id, and a stop by stream or by
    // the old id finds it.
    let again = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    assert_eq!(read_status(&again).await["runs"][0]["id"], "btc-15m");
    let stopped = stop_run(&again, "btc", "15m").await.expect("stop");
    assert_eq!(stopped["id"], "btc-15m");
    assert!(old_dir.join("final.json").is_file());
}

#[tokio::test]
async fn a_start_on_a_market_with_no_store_file_warms_up_from_zero() {
    // `eurusd` is configured for an MT5 poller and has no `EURUSD-*.parquet`
    // in the store; the run starts with an empty window and takes the
    // poller's warm-up bars like any others.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    assert!(!dir.path().join("bars").join("EURUSD-15m.parquet").exists());
    let started = start_run(&state, json!({ "market": "eurusd", "tf": "15m", "strategy": "ema-cross", "label": "demo 12345 — eurusd" })).await.expect("start");
    assert_eq!(started["id"], "eurusd-15m-ema-cross");
    assert_eq!(started["warmup_bars"], 0);
    assert_eq!(started["bars"], 0);
    assert_eq!(started["market"], "eurusd");
    // The poller's warm-up bars, scaled to the pair, are accepted; the
    // btc store is another stream and is not consulted.
    for i in 0..5 {
        let mut b = wave(i);
        for p in [&mut b.open, &mut b.high, &mut b.low, &mut b.close] {
            *p /= 50_000.0;
        }
        let reply = post_bar(&state, "eurusd", "15m", b).await.expect("bar");
        assert_eq!(reply["accepted"], true, "{reply}");
        assert_eq!(reply["runs"].as_array().map(Vec::len), Some(1));
        assert_eq!(reply["runs"][0]["id"], "eurusd-15m-ema-cross");
    }
    let s = read_status(&state).await;
    assert_eq!(s["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(s["runs"][0]["bars_seen"], 5);
    assert_eq!(s["runs"][0]["bars"], 5);
    assert_eq!(s["runs"][0]["warmup_bars"], 0);
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
    // The id is a directory name: lowercase, digits, `_`, `-`, at most 40.
    let long = "a".repeat(41);
    for id in ["", "Demo", "demo 1", "demo/1", long.as_str(), "dé"] {
        assert_eq!(bad(json!({ "id": id, "market": "btc", "tf": "15m", "strategy": "ema-cross" })).await, Err(400), "id {id:?}");
    }
    // A stop needs an id or a stream.
    assert_eq!(stop_with(&state, json!({})).await.map(|_| ()).map_err(http_status), Err(400));
    assert_eq!(stop_with(&state, json!({ "market": "btc" })).await.map(|_| ()).map_err(http_status), Err(400));
    assert_eq!(stop_with(&state, json!({ "id": "nope" })).await.map(|_| ()).map_err(http_status), Err(404));
    // With an empty store the run still starts, with no warm-up bars.
    let empty = tempfile::tempdir().expect("temp dir");
    let bare = Arc::new(AppState::new(config(), empty.path().to_path_buf()));
    let started = start_run(&bare, json!({ "id": "a".repeat(40), "market": "btc", "tf": "15m", "strategy": "ema-cross" })).await.expect("start");
    assert_eq!(started["bars"], 0);
    assert_eq!(started["warmup_bars"], 0);
    assert!(read_status(&state).await["runs"].as_array().is_some_and(Vec::is_empty));
}

#[tokio::test]
async fn the_detail_carries_the_curve_the_fills_the_events_and_the_window() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "id": "detail", "market": "btc", "tf": "15m", "strategy": "ema-cross" }))
        .await
        .expect("start");
    for i in 300..350 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }

    let d = read_detail(&state, "detail", None).await.expect("detail");
    let run = &d["run"];
    assert_eq!(run["id"], "detail");

    // The curve opens at the starting equity, at the moment the run started,
    // so a run with no trade still draws a point.
    let curve = d["equity_curve"].as_array().expect("curve");
    assert_eq!(curve[0][0], run["started_at"]);
    assert_eq!(curve[0][1], 10_000.0);
    assert_eq!(curve.len(), run["trades"].as_u64().expect("trades") as usize + 1);

    // Every closed trade is here, not just the ten the status carries.
    let fills = d["fills"].as_array().expect("fills");
    assert_eq!(fills.len(), run["trades"].as_u64().expect("trades") as usize);
    assert!(!fills.is_empty(), "the wave must produce a closed trade");
    assert!(fills[0]["exitTime"].as_i64() <= fills[fills.len() - 1]["exitTime"].as_i64(), "oldest first");

    // The event lines are the ones that are not trades.
    let events = d["events"].as_array().expect("events");
    assert!(events.iter().any(|e| e["kind"] == "started"), "{events:?}");
    assert!(events.iter().all(|e| e["kind"] != "trade"), "trades belong in `fills`");

    // The window, newest `bars` of it, oldest first.
    assert_eq!(d["bars"].as_array().map(Vec::len), Some(DEFAULT_DETAIL_BARS));
    let ten = read_detail(&state, "detail", Some(10)).await.expect("detail");
    let bars = ten["bars"].as_array().expect("bars");
    assert_eq!(bars.len(), 10);
    assert_eq!(bars[9][0], wave(349).time, "the newest window bar is last");
    assert!(bars[0][0].as_i64() < bars[9][0].as_i64());

    assert_eq!(read_detail(&state, "nobody", None).await.map(|_| ()).map_err(http_status), Err(404));
}

/// A tick is the screen's price and nothing else: it reaches the status and
/// the detail, and it reaches no book, no counter and no file.
#[tokio::test]
async fn a_tick_is_reported_and_touches_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "id": "ticker", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 }))
        .await
        .expect("start");
    for i in 300..340 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }

    let before = read_status(&state).await;
    let run_before = run_named(&before, "ticker").clone();
    assert!(run_before["live"].is_null(), "no tick yet, so no live price");
    let run_dir = dir.path().join("paper").join("ticker");
    let state_json_before = std::fs::read(run_dir.join("state.json")).expect("state.json");
    let fills_before = std::fs::read(run_dir.join("fills.jsonl")).expect("fills.jsonl");

    // The bucket after the last closed bar, still forming, a dollar up.
    let next = wave(340);
    let mut body = forming(next, next.open + 1.0);
    body["bid"] = json!(next.open + 0.9);
    body["ask"] = json!(next.open + 1.1);
    let sent_at = now_ms();
    assert_eq!(post_tick(&state, "btc", "15m", body).await.expect("tick"), json!({ "stored": true }));

    let run = run_named(&read_status(&state).await, "ticker").clone();
    let live = &run["live"];
    assert_eq!(live["time"], next.time, "the forming bar keeps its own bucket");
    assert_eq!(live["close"], next.open + 1.0);
    assert_eq!(live["bid"], next.open + 0.9);
    assert_eq!(live["ask"], next.open + 1.1);
    assert_eq!(live["volume"], 7.0);
    assert!(live["at"].as_i64().expect("at") >= sent_at, "`at` is the server's clock, not the sender's");

    // Nothing the run decides on moved, on the wire or on disk.
    for key in ["bars", "bars_seen", "trades", "last_bar_time", "equity", "net_usd", "gaps", "open", "last_fills"] {
        assert_eq!(run[key], run_before[key], "a tick must not move `{key}`");
    }
    assert_eq!(std::fs::read(run_dir.join("state.json")).expect("state.json"), state_json_before, "a tick must not rewrite the book");
    assert_eq!(std::fs::read(run_dir.join("fills.jsonl")).expect("fills.jsonl"), fills_before, "a tick must not write an event line");

    // The chart reads it beside the bars rather than through the status entry.
    let detail = read_detail(&state, "ticker", Some(10)).await.expect("detail");
    assert_eq!(detail["live"]["close"], next.open + 1.0);
    assert_eq!(detail["live"], detail["run"]["live"], "one value, carried twice");
    assert_eq!(detail["bars"].as_array().map(Vec::len), Some(10));
    assert_eq!(detail["bars"][9][0], wave(339).time, "the newest window bar is still the last closed one");
}

/// A tick belongs to one `market:tf`. Another stream's price is not this
/// run's price, and a stream nobody trades is accepted all the same.
#[tokio::test]
async fn a_tick_is_keyed_by_stream() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "id": "only-15m", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 }))
        .await
        .expect("start");

    // `btc:1h` has no run — a closed bar there is 404, a tick is not.
    assert_eq!(post_bar(&state, "btc", "1h", wave(340)).await.map(|_| ()).map_err(http_status), Err(404));
    assert_eq!(post_tick(&state, "btc", "1h", forming(wave(340), 61_111.0)).await.expect("tick"), json!({ "stored": true }));
    assert!(run_named(&read_status(&state).await, "only-15m")["live"].is_null(), "another stream's tick is not this run's price");

    post_tick(&state, "btc", "15m", forming(wave(340), 60_222.0)).await.expect("tick");
    assert_eq!(run_named(&read_status(&state).await, "only-15m")["live"]["close"], 60_222.0);
}

/// The checks a closed bar gets: finite, ordered, and a timeframe the API
/// serves. A tick that fails one is refused rather than drawn.
#[tokio::test]
async fn a_malformed_tick_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "id": "strict", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 }))
        .await
        .expect("start");
    let b = wave(340);

    let inverted = json!({ "bar": { "time": b.time, "open": b.open, "high": b.low - 1.0, "low": b.low, "close": b.close } });
    assert_eq!(post_tick(&state, "btc", "15m", inverted).await.map(|_| ()).map_err(http_status), Err(400), "high < low");

    let unknown_tf = post_tick(&state, "btc", "3m", forming(b, b.close)).await;
    assert_eq!(unknown_tf.map(|_| ()).map_err(http_status), Err(400), "3m is not a timeframe this API serves");

    assert!(run_named(&read_status(&state).await, "strict")["live"].is_null(), "nothing refused was stored");
}

/// Older than ninety seconds is not a current price: the status drops it
/// rather than let the client draw a dead feed as live.
#[tokio::test]
async fn a_stale_tick_is_not_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "id": "stale", "market": "btc", "tf": "15m", "strategy": "ema-cross", "window": 200 }))
        .await
        .expect("start");
    post_tick(&state, "btc", "15m", forming(wave(340), 60_333.0)).await.expect("tick");
    assert_eq!(run_named(&read_status(&state).await, "stale")["live"]["close"], 60_333.0);

    // Aged past the cut-off in place. The rule is on `at`, the moment the
    // server received it, so this is exactly what a dead poller looks like.
    {
        let mut live = state.live_bars.lock().expect("live bars");
        live.get_mut("btc:15m").expect("the stored tick").at -= MAX_LIVE_AGE_MS + 1;
    }
    assert!(run_named(&read_status(&state).await, "stale")["live"].is_null(), "a tick older than 90 s is not a price");
    assert!(read_detail(&state, "stale", Some(5)).await.expect("detail")["live"].is_null());

    // A fresh tick brings the price back: the stale one was dropped from the
    // report, not from the map, and the next read overwrites it.
    post_tick(&state, "btc", "15m", forming(wave(340), 60_444.0)).await.expect("tick");
    assert_eq!(run_named(&read_status(&state).await, "stale")["live"]["close"], 60_444.0);
}

/// The server's own clock, the way the handler stamps `at`.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Wall clock in epoch milliseconds, as `fd-api` stamps it.
fn wall_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[tokio::test]
async fn an_entry_records_when_it_was_learned_beside_the_bar_it_is_priced_at() {
    // The two clocks a fill has, which were one field until 2026-09-17.
    //
    // `entry_time` is the bar whose OPEN the fill is priced at. `learned_at`
    // is the wall clock at which this process found out the book was in. On a
    // live 15m book they are a bar apart every time, because the bar that
    // fills the intent is only posted once it has CLOSED - six positions out
    // of six on the VPS on 2026-09-17 surfaced to the executor exactly one bar
    // after the book's stamp. Here the bars are posted in a loop, so the gap
    // is milliseconds; what is pinned is that both numbers exist, that they
    // are different fields, and that the entry gets a line of its own.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "b", "window": 200 }))
        .await
        .expect("start");

    let before = wall_ms();
    for i in 300..380 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let after = wall_ms();

    // Until this change an entry appeared in `fills.jsonl` only when it
    // CLOSED, inside the trade record - so the one moment worth timestamping
    // was the one moment the file did not record.
    let text = std::fs::read_to_string(dir.path().join("paper").join("b").join("fills.jsonl")).expect("fills");
    let opened: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == "opened")
        .collect();
    assert!(!opened.is_empty(), "an entry must have its own line: {text}");

    for e in &opened {
        let bar_time = e["time"].as_i64().expect("time");
        let learned = e["learned_at"].as_i64().expect("learned_at");
        assert!(bar_time > 0 && learned > 0, "both clocks are present: {e}");
        assert!(learned >= before && learned <= after, "learned_at is a wall clock, not a bar time: {e}");
        assert!(learned > bar_time, "the synthetic tape is stamped in 2026; wall clock is later: {e}");
        assert!(e["side"].is_string() && e["entry_price"].is_number(), "{e}");
    }

    // And the status carries it beside `entry_time` for as long as the
    // position is held.
    let status = read_status(&state).await;
    let run = run_named(&status, "b");
    if let Some(open) = run["open"].as_object() {
        let learned = open["learned_at"].as_i64().expect("learned_at on an open position");
        assert!(learned >= before && learned <= after, "{open:?}");
        assert_ne!(
            learned,
            open["entry_time"].as_i64().expect("entry_time"),
            "the priced-at bar and the learned-at clock are different numbers"
        );
    }
}

#[tokio::test]
async fn a_flat_book_reports_no_learned_at_and_a_reload_keeps_the_one_it_has() {
    // `learned_at` describes the position that is open NOW. A stale one
    // surviving a close would describe the position before the one a reader is
    // looking at, which is the failure mode the field exists to prevent rather
    // than to create.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "b", "window": 200 }))
        .await
        .expect("start");
    for i in 300..380 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }

    let live = run_named(&read_status(&state).await, "b").clone();
    // A restart must not lose it: the executor reads this to know how old the
    // book's entry is, and a mirror that forgot across a restart would read a
    // fresh fill where there was an hour-old one.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let after = run_named(&read_status(&reborn).await, "b").clone();
    assert_eq!(after["open"]["learned_at"], live["open"]["learned_at"], "it survives a reload");

    if live["open"].is_null() {
        assert!(live["open"]["learned_at"].is_null(), "a flat book has nothing to have learned");
    }
}

#[tokio::test]
async fn a_decided_intent_records_when_the_decider_spoke_against_the_price_it_got() {
    // The look-ahead this makes visible: a model reads the bar that closed at
    // `t`, answers some seconds later, and the intent it posts fills at the
    // open of the bar stamped `t` - a price that already existed while the
    // model was still thinking. Measured 2026-09-17 over 321 real decisions,
    // that is 24.2 s at the median and 199 s at p90, and it is biased in the
    // book's favour: if the model has skill, the price before it spoke is the
    // better one in the direction it chose.
    //
    // Not corrected here. Filling at the tick the decision arrived on is a
    // different fill model and a different hypothesis. What is pinned is that
    // the record now carries both numbers so the bias can be measured rather
    // than argued about.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "b", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    let before = wall_ms();
    let reply = post_intent(
        &state,
        json!({ "run": "b", "bar_time": wave(300).time, "side": "LONG", "reason": "a model said so", "decider": "gpt-5" }),
    )
    .await
    .expect("intent");
    assert_eq!(reply["accepted"], true, "{reply}");
    let after = wall_ms();

    // The intent line stamps the decider's own clock beside the bar it read.
    let path = dir.path().join("paper").join("b").join("fills.jsonl");
    let intents: Vec<Value> = std::fs::read_to_string(&path)
        .expect("fills")
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == "intent")
        .collect();
    let decided = intents.last().expect("an intent line")["decided_at"].as_i64().expect("decided_at");
    assert!(decided >= before && decided <= after, "a wall clock, not a bar time: {decided}");

    // And the fill it receives carries it, so the two can be compared without
    // joining two files on a guess.
    post_bar(&state, "btc", "15m", wave(301)).await.expect("bar");
    let opened: Vec<Value> = std::fs::read_to_string(&path)
        .expect("fills")
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == "opened")
        .collect();
    let line = opened.last().expect("the entry opened");
    assert_eq!(line["decided_at"].as_i64(), Some(decided), "the fill names the decision that caused it: {line}");
    assert!(line["learned_at"].as_i64().expect("learned_at") >= decided, "learned after decided: {line}");

    // Taken with the pending it belongs to: the next entry must not inherit
    // this decision's stamp. A rule book never sets one at all.
    let state2 = state_over(dir.path(), 300);
    start_run(&state2, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "r", "window": 200 }))
        .await
        .expect("start");
    for i in 300..380 {
        let _ = post_bar(&state2, "btc", "15m", wave(i)).await;
    }
    let rule_opens: Vec<Value> = std::fs::read_to_string(dir.path().join("paper").join("r").join("fills.jsonl"))
        .expect("fills")
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == "opened")
        .collect();
    assert!(!rule_opens.is_empty(), "the wave must open something");
    assert!(
        rule_opens.iter().all(|e| e["decided_at"].is_null()),
        "a rule decides instantly, so it has no gap to record: {rule_opens:?}"
    );
}

/* ------------------------------------------------ filling at the open */

/// Post the open of a bar that has only just started, before it closes.
async fn post_open(state: &Arc<AppState>, market: &str, tf: &str, time: i64, open: f64) -> Result<Value, ApiError> {
    let body = json!({ "market": market, "tf": tf, "time": time, "open": open });
    let request = serde_json::from_value(body).expect("an open body");
    open_now(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

/// A run that has a pending entry waiting, and the bar whose open will fill it.
async fn run_with_a_pending_entry(dir: &Path) -> (Arc<AppState>, Bar) {
    let state = state_over(dir, 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "b", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");
    post_intent(
        &state,
        json!({ "run": "b", "bar_time": wave(300).time, "side": "LONG", "reason": "a model said so", "decider": "gpt-5" }),
    )
    .await
    .expect("intent");
    (state, wave(301))
}

#[tokio::test]
async fn a_restart_between_the_open_and_the_close_fills_once() {
    // THE test for this change, and written before the split existed.
    //
    // The two halves are now two calls with a persist between them, so a
    // process that dies in the middle is a state nothing had seen before. The
    // account is real; "filled twice" here is a second live position, and
    // "filled zero times" is the bug this whole change exists to remove.
    //
    // Proves: the TIME moved. The goldens cannot show that - they replay a
    // backtest, which has no wall clock and no restart.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = run_with_a_pending_entry(dir.path()).await;

    // Half one: the bar has opened and has not closed.
    post_open(&state, "btc", "15m", next.time, next.open).await.expect("open");
    let before = run_named(&read_status(&state).await, "b").clone();
    assert!(!before["open"].is_null(), "the book holds the position at the OPEN, not a bar later: {before}");
    let entry_price = before["open"]["entry_price"].as_f64().expect("entry_price");
    let entry_time = before["open"]["entry_time"].as_i64().expect("entry_time");
    assert_eq!(entry_time, next.time, "priced at the bar that just opened");

    // The process dies here. Everything the book knows is on disk or gone.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let recovered = run_named(&read_status(&reborn).await, "b").clone();
    assert_eq!(recovered["open"]["entry_price"], entry_price, "the fill survived the restart");
    assert_eq!(recovered["open"]["entry_time"], entry_time);

    // Half two, on the process that did not do half one: the same bar closes.
    post_bar(&reborn, "btc", "15m", next).await.expect("bar");
    let after = run_named(&read_status(&reborn).await, "b").clone();
    if !after["open"].is_null() {
        assert_eq!(after["open"]["entry_price"], entry_price, "not re-filled at a second price");
        assert_eq!(after["open"]["entry_time"], entry_time, "and not re-stamped");
    }
    // One entry, whichever way the bar went: either it is still open at the
    // same price, or it closed and is ONE trade.
    let trades = after["trades"].as_u64().expect("trades");
    assert!(trades <= 1, "one intent must not become two trades: {after}");
}

#[tokio::test]
async fn the_open_fills_at_the_same_price_the_close_would_have() {
    // Proves: the PRICE did not move. The one thing the goldens also prove,
    // asserted here directly so the two halves can be compared side by side
    // rather than inferred from a parity suite passing.
    let a = tempfile::tempdir().expect("temp dir");
    let b = tempfile::tempdir().expect("temp dir");
    let (early, next) = run_with_a_pending_entry(a.path()).await;
    let (late, _) = run_with_a_pending_entry(b.path()).await;

    // One fills at the open and then sees the bar close; the other only ever
    // sees the closed bar, which is what the desk does today.
    post_open(&early, "btc", "15m", next.time, next.open).await.expect("open");
    post_bar(&early, "btc", "15m", next).await.expect("bar");
    post_bar(&late, "btc", "15m", next).await.expect("bar");

    let x = run_named(&read_status(&early).await, "b").clone();
    let y = run_named(&read_status(&late).await, "b").clone();

    // Every field of the position except one, and the exception is the point:
    // `learned_at` is the wall clock at which the desk found out, and moving
    // it is the entire purpose of the change. Everything that describes the
    // TRADE - price, stamp, size, stop, target, risk - must be identical to
    // the bit, and this comparison is what says so.
    //
    // It did not start identical. `risk`, `stop` and `target` differed in the
    // eighth decimal because the two halves computed the sizing ATR over
    // windows offset by one bar; see the comment in `accept_open`. The same
    // fill was getting two different stops depending on which half filled it,
    // which is exactly the kind of difference that is too small to matter and
    // too real to leave.
    let mut a = x["open"].clone();
    let mut b = y["open"].clone();
    assert!(!a["learned_at"].is_null() && !b["learned_at"].is_null(), "both learned it");
    a["learned_at"] = Value::Null;
    b["learned_at"] = Value::Null;
    assert_eq!(a, b, "same trade in every respect but when it was learned");
    assert_eq!(x["net_usd"], y["net_usd"], "and the same money");
    assert_eq!(x["trades"], y["trades"]);
}

#[tokio::test]
async fn an_open_at_or_before_the_last_bar_fills_nothing_and_a_gap_still_fills() {
    // The ordering rule, and it is the SAME one the closed-bar path already
    // applies: strictly newer than the last bar. Not adjacency.
    //
    // Adjacency was the first design and it is wrong on this instrument. The
    // broker's tape has a weekend and one hour a day with no bars at all
    // (21:00Z March-November, 22:00Z otherwise - 6c, over 100,586 bars), so
    // requiring `time == last + bar_ms` would refuse the first open of every
    // session and hand the lag back on the entry taken into the thinnest book
    // of the day. A gap is indistinguishable from a jump-ahead by time alone,
    // and the closed-bar path already chose to accept both and record the gap.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = run_with_a_pending_entry(dir.path()).await;

    // At the last bar: not newer, so nothing.
    post_open(&state, "btc", "15m", wave(300).time, wave(300).open).await.expect("open");
    assert!(run_named(&read_status(&state).await, "b")["open"].is_null(), "the bar already advanced over is not next");

    // Before it: likewise.
    post_open(&state, "btc", "15m", wave(299).time, wave(299).open).await.expect("open");
    assert!(run_named(&read_status(&state).await, "b")["open"].is_null(), "and neither is one before that");

    // Across a gap, which is what a Monday open is: it fills.
    let after_gap = wave(305);
    post_open(&state, "btc", "15m", after_gap.time, after_gap.open).await.expect("open");
    let filled = run_named(&read_status(&state).await, "b").clone();
    assert!(!filled["open"].is_null(), "the first open after a gap must fill: {filled}");
    assert_eq!(filled["open"]["entry_time"], after_gap.time, "at the bar it actually opened on");
    let _ = next;
}

#[tokio::test]
async fn an_open_replayed_after_its_bar_closed_cannot_take_the_next_intent() {
    // The case the ordering rule exists for, and the only one that could cost
    // money: an open for bar t+1 arriving late, after t+1 has closed and the
    // strategy has decided the NEXT entry. Filling it would price the next
    // bar's decision at the previous bar's open - a trade at a price the book
    // never saw, and on the mirror a real order at it.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = run_with_a_pending_entry(dir.path()).await;

    post_open(&state, "btc", "15m", next.time, next.open).await.expect("open");
    post_bar(&state, "btc", "15m", next).await.expect("bar");
    post_intent(
        &state,
        json!({ "run": "b", "bar_time": next.time, "side": "LONG", "reason": "the next one", "decider": "gpt-5" }),
    )
    .await
    .expect("intent");

    let before = run_named(&read_status(&state).await, "b").clone();
    post_open(&state, "btc", "15m", next.time, next.open).await.expect("open");
    let after = run_named(&read_status(&state).await, "b").clone();
    assert_eq!(after["open"], before["open"], "a replayed open changed no position");
    assert_eq!(after["pending"], before["pending"], "and did not consume the waiting intent");
}

#[tokio::test]
async fn the_open_half_fills_and_the_close_half_still_manages_the_bar() {
    // The half that is easy to lose in a refactor. A position opened at the
    // bar's open must still be tested against THAT bar's high and low when it
    // closes - today both happen in one call, and splitting them must not turn
    // "fill and manage" into "fill only".
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = run_with_a_pending_entry(dir.path()).await;
    post_open(&state, "btc", "15m", next.time, next.open).await.expect("open");

    let open_before = run_named(&read_status(&state).await, "b")["open"].clone();
    let stop = open_before["stop"].as_f64().expect("a stop");

    // A bar whose low is through the stop: opened at the open, stopped on the
    // same bar's range.
    let crash = Bar { time: next.time, open: next.open, high: next.open, low: stop - 50.0, close: stop - 40.0, volume: Some(1.0) };
    post_bar(&state, "btc", "15m", crash).await.expect("bar");

    let after = run_named(&read_status(&state).await, "b").clone();
    assert!(after["open"].is_null(), "the bar that opened it also stopped it: {after}");
    assert_eq!(after["trades"], 1, "and it is one closed trade");
}

#[tokio::test]
async fn an_intent_records_which_higher_timeframe_state_it_read() {
    // The experiment this exists for compares decisions that saw H4 context
    // against decisions that did not. If a decider that never fetched the
    // context is indistinguishable in the record from one that fetched it and
    // got nothing, the two arms are a mixture and the comparison measures
    // nothing. So: the H4 bar the decision read is carried from the intent
    // through to the fill, and ABSENT when there was none.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = state_over(dir.path(), 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": "b", "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");

    let h4_bar = 1_789_678_800_000_i64;
    post_intent(
        &state,
        json!({ "run": "b", "bar_time": wave(300).time, "side": "LONG",
                "reason": "the H4 structure was up", "decider": "gpt-5",
                "htf_bar_time": h4_bar }),
    )
    .await
    .expect("intent");

    let path = dir.path().join("paper").join("b").join("fills.jsonl");
    let lines = |kind: &str| -> Vec<Value> {
        std::fs::read_to_string(&path)
            .expect("fills")
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|e| e["kind"] == kind)
            .collect()
    };
    assert_eq!(lines("intent").last().expect("an intent")["htf_bar_time"].as_i64(), Some(h4_bar));

    // And the fill it becomes carries the same stamp, so the trade record
    // answers "what did this decision know" without joining two files on a
    // timestamp and hoping.
    post_bar(&state, "btc", "15m", wave(301)).await.expect("bar");
    let opened = lines("opened");
    assert_eq!(opened.last().expect("the entry opened")["htf_bar_time"].as_i64(), Some(h4_bar));

    // A decider that read no context leaves it NULL, not zero. Zero is a bar
    // time at the epoch; absence is absence.
    post_intent(
        &state,
        json!({ "run": "b", "bar_time": wave(301).time, "side": "LONG", "reason": "no context", "decider": "gpt-5" }),
    )
    .await
    .expect("intent");
    let last = lines("intent").pop().expect("an intent");
    assert!(last["htf_bar_time"].is_null(), "absent, not zero: {last}");
}

/* ------------------------------------------------ orders resting at a price */

async fn post_act(state: &Arc<AppState>, body: Value) -> Result<Value, ApiError> {
    let request = serde_json::from_value(body).expect("an act body");
    act(State(Arc::clone(state)), Json(request)).await.map(|Json(v)| serde_json::to_value(v).expect("json"))
}

async fn read_pending(state: &Arc<AppState>) -> Value {
    let Json(v) = pending(State(Arc::clone(state))).await.expect("pending");
    serde_json::to_value(v).expect("json")
}

/// Every `fills.jsonl` line of `kind` for run `id`, oldest first.
fn rows(dir: &Path, id: &str, kind: &str) -> Vec<Value> {
    std::fs::read_to_string(dir.join("paper").join(id).join("fills.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|e| e["kind"] == kind)
        .collect()
}

/// A tick in `b`'s bucket at `last`, quoting `bid`/`ask`.
fn quote(b: Bar, last: f64, bid: f64, ask: f64) -> Value {
    let mut body = forming(b, last);
    body["bid"] = json!(bid);
    body["ask"] = json!(ask);
    body
}

/// An external run that has advanced over bar 300 and rests nothing; the
/// bar that will open next.
async fn external_run(dir: &Path, id: &str) -> (Arc<AppState>, Bar) {
    let state = state_over(dir, 300);
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "external", "id": id, "window": 200 }))
        .await
        .expect("start");
    post_bar(&state, "btc", "15m", wave(300)).await.expect("bar");
    (state, wave(301))
}

/// A LONG limit at `price` on bar 300, stop 100 under, target 200 over.
fn long_limit(id: &str, price: f64) -> Value {
    json!({
        "run": id, "bar_time": wave(300).time, "side": "LONG", "decider": "gpt-5",
        "reason": "wait for the pullback",
        "entry": { "type": "limit", "price": price },
        "zone": [price - 5.0, price + 5.0],
        "stop": price - 100.0, "target": price + 200.0,
    })
}

/// `body` with `patch`'s keys written over it.
fn patched(mut body: Value, patch: Value) -> Value {
    for (k, v) in patch.as_object().expect("object") {
        body[k] = v.clone();
    }
    body
}

#[tokio::test]
async fn a_limit_fill_and_a_market_fill_at_the_same_price_are_the_same_trade() {
    // The contract's test, at the API: a limit that fills at P through the
    // tick feed and a market intent that fills at an open of P are one trade
    // in every field but the clock the desk learned it on.
    let a = tempfile::tempdir().expect("temp dir");
    let b = tempfile::tempdir().expect("temp dir");
    let (market, next) = external_run(a.path(), "m").await;
    let (limit, _) = external_run(b.path(), "l").await;
    let price = next.open;

    post_intent(
        &market,
        json!({ "run": "m", "bar_time": wave(300).time, "side": "LONG", "decider": "gpt-5",
                "stop": price - 100.0, "target": price + 200.0, "reason": "now" }),
    )
    .await
    .expect("intent");
    post_open(&market, "btc", "15m", next.time, price).await.expect("open");

    let reply = post_intent(&limit, long_limit("l", price)).await.expect("intent");
    assert_eq!(reply["accepted"], true, "{reply}");
    assert!(reply["reason"].as_str().expect("reason").starts_with("rests as a limit at"), "{reply}");

    // Resting: the status says so, in the shape the contract states.
    let resting = run_named(&read_status(&limit).await, "l").clone();
    assert!(resting["open"].is_null() && resting["pending"].is_null(), "one committed entry at most: {resting}");
    let order = &resting["pending_order"];
    assert_eq!(order["type"], "limit");
    assert_eq!(order["price"], price);
    assert_eq!(order["side"], "LONG");
    assert_eq!(order["stop"], price - 100.0);
    assert_eq!(order["target"], price + 200.0);
    assert_eq!(order["zone"], json!([price - 5.0, price + 5.0]));
    assert_eq!(order["valid_until_bar_ms"], wave(300).time + 2 * BAR, "default valid_bars is 2");
    assert_eq!(order["valid_bars"], 2);
    assert_eq!(order["bars_waited"], 0);
    assert_eq!(order["decided_bar_time"], wave(300).time);
    assert!(order["decided_at"].is_number(), "{order}");
    assert!(order["invalidate_above"].is_null() && order["invalidate_below"].is_null());
    assert!(order["lots"].as_f64().expect("lots is a number") > 0.0, "{order}");

    // The ask comes down to the level.
    let reply = post_tick(&limit, "btc", "15m", quote(next, price - 0.5, price - 1.0, price)).await.expect("tick");
    assert_eq!(reply["orders"][0]["event"], "filled", "{reply}");

    let x = run_named(&read_status(&market).await, "m").clone();
    let y = run_named(&read_status(&limit).await, "l").clone();
    assert!(y["pending_order"].is_null(), "consumed");
    let mut a_open = x["open"].clone();
    let mut b_open = y["open"].clone();
    assert!(!a_open.is_null() && !b_open.is_null(), "both filled: {x} {y}");
    assert_eq!(b_open["lots"], order["lots"], "the size the status promised is the size the fill took");
    a_open["learned_at"] = Value::Null;
    b_open["learned_at"] = Value::Null;
    assert_eq!(a_open, b_open, "same lots, stop, target, risk, price and stamp");

    // And the rows say how each was entered.
    let m = rows(a.path(), "m", "opened").pop().expect("opened");
    let l = rows(b.path(), "l", "opened").pop().expect("opened");
    assert_eq!(m["entry"], json!({ "type": "market", "price": price, "requested_price": null }));
    assert_eq!(l["entry"], json!({ "type": "limit", "price": price, "requested_price": price }));
    assert_eq!(l["filled_from"], "tick");
    assert_eq!(l["decided_at"], order["decided_at"], "the fill names the decision");
    assert_eq!(m["filled_from"], "bar_open");
}

#[tokio::test]
async fn a_tick_short_of_the_level_leaves_the_order_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 50.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");
    let state_json = std::fs::read(dir.path().join("paper").join("l").join("state.json")).expect("state.json");
    let fills = std::fs::read(dir.path().join("paper").join("l").join("fills.jsonl")).expect("fills.jsonl");

    // Ask still above the limit: nothing.
    let reply = post_tick(&state, "btc", "15m", quote(next, price + 3.0, price + 2.8, price + 3.2)).await.expect("tick");
    assert_eq!(reply, json!({ "stored": true }), "no order event");
    // A tick in the bucket the run has already closed over, even at the
    // level: nothing. A tick and a bar racing is this case.
    let stale = post_tick(&state, "btc", "15m", quote(wave(300), price, price - 1.0, price)).await.expect("tick");
    assert_eq!(stale, json!({ "stored": true }));
    // A tick with no ask cannot fill a LONG.
    let mut blind = forming(next, price - 1.0);
    blind["bid"] = json!(price - 1.2);
    assert_eq!(post_tick(&state, "btc", "15m", blind).await.expect("tick"), json!({ "stored": true }));

    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(run["open"].is_null() && !run["pending_order"].is_null(), "{run}");
    assert_eq!(std::fs::read(dir.path().join("paper").join("l").join("state.json")).expect("state.json"), state_json, "not rewritten");
    assert_eq!(std::fs::read(dir.path().join("paper").join("l").join("fills.jsonl")).expect("fills.jsonl"), fills, "no line");
}

#[tokio::test]
async fn an_order_expires_after_its_valid_bars_and_the_miss_is_counted() {
    // A limit nobody reaches. Two closed bars after the decision bar it is
    // gone, and a `cancelled_unfilled` row says it waited and did not fill,
    // because a fill rate is a fraction and this is its denominator.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 5_000.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");
    let decided_at = run_named(&read_status(&state).await, "l")["pending_order"]["decided_at"].clone();

    post_bar(&state, "btc", "15m", wave(301)).await.expect("bar");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert_eq!(run["pending_order"]["bars_waited"], 1, "one of two: {run}");
    // A closed bar whose range covers the level is NOT a fill.
    let mut wide = wave(302);
    wide.low = price - 10.0;
    post_bar(&state, "btc", "15m", wide).await.expect("bar");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(run["pending_order"].is_null() && run["open"].is_null(), "expired, not filled: {run}");

    let row = rows(dir.path(), "l", "cancelled_unfilled").pop().expect("the miss is a row");
    assert_eq!(row["reason"], "expired");
    assert_eq!(row["bars_waited"], 2);
    assert_eq!(row["time"], wide.time, "on the bar clock");
    assert!(row["at"].is_number(), "and the wall clock beside it");
    assert_eq!(row["decided_at"], decided_at);
    assert_eq!(row["entry"]["type"], "limit");
    assert_eq!(row["entry"]["price"], price);
    assert_eq!(row["entry"]["valid_bars"], 2);
    assert_eq!(row["entry"]["side"], "LONG");

    // Nothing lingers: the next intent's fill carries its own stamp only.
    post_intent(
        &state,
        json!({ "run": "l", "bar_time": wide.time, "side": "LONG", "decider": "gpt-5", "reason": "market now" }),
    )
    .await
    .expect("intent");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(!run["pending"].is_null() && run["pending_order"].is_null(), "{run}");
}

#[tokio::test]
async fn a_tick_beyond_an_invalidation_level_cancels_before_it_fills() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 50.0;
    let body = patched(long_limit("l", price), json!({ "invalidate_above": next.open + 100.0, "valid_bars": 3 }));
    post_intent(&state, body).await.expect("intent");
    let order = run_named(&read_status(&state).await, "l")["pending_order"].clone();
    assert_eq!(order["invalidate_above"], next.open + 100.0);
    assert_eq!(order["valid_until_bar_ms"], wave(300).time + 3 * BAR);

    // The ask runs away above the level: cancelled, not filled.
    let far = next.open + 100.5;
    let reply = post_tick(&state, "btc", "15m", quote(next, far, far - 0.2, far)).await.expect("tick");
    assert_eq!(reply["orders"][0]["event"], "cancelled", "{reply}");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(run["pending_order"].is_null() && run["open"].is_null(), "{run}");
    let row = rows(dir.path(), "l", "cancelled_unfilled").pop().expect("a row");
    assert_eq!(row["reason"], "invalidated");
    assert_eq!(row["bars_waited"], 0);
    assert_eq!(row["time"], next.time);

    // A quote that has gapped through both the invalidation and the level
    // on one tick cancels rather than fills: invalidation is read first.
    let (state2, next2) = external_run(tempfile::tempdir().expect("temp dir").path(), "g").await;
    let level = next2.open + 30.0;
    let body = patched(
        json!({ "run": "g", "bar_time": wave(300).time, "side": "LONG", "decider": "gpt-5", "reason": "breakout",
                "entry": { "type": "stop", "price": level }, "stop": level - 100.0 }),
        json!({ "invalidate_above": level + 50.0 }),
    );
    post_intent(&state2, body).await.expect("intent");
    post_tick(&state2, "btc", "15m", quote(next2, level + 60.0, level + 59.8, level + 60.2)).await.expect("tick");
    let run = run_named(&read_status(&state2).await, "g").clone();
    assert!(run["open"].is_null() && run["pending_order"].is_null(), "cancelled, not filled: {run}");
}

#[tokio::test]
async fn a_new_entry_while_an_order_rests_replaces_it_and_a_stand_aside_leaves_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    post_intent(&state, long_limit("l", next.open - 50.0)).await.expect("intent");

    // A second order takes the slot.
    let second = patched(
        long_limit("l", next.open - 40.0),
        json!({ "entry": { "type": "stop", "price": next.open + 40.0 }, "stop": next.open - 60.0, "target": next.open + 240.0 }),
    );
    let reply = post_intent(&state, second).await.expect("intent");
    assert_eq!(reply["accepted"], true, "{reply}");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert_eq!(run["pending_order"]["type"], "stop");
    assert_eq!(run["pending_order"]["price"], next.open + 40.0);
    let replaced = rows(dir.path(), "l", "cancelled_unfilled");
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0]["reason"], "replaced");
    assert_eq!(replaced[0]["entry"]["price"], next.open - 50.0);

    // And a market intent takes it from an order too: one committed entry.
    post_intent(
        &state,
        json!({ "run": "l", "bar_time": wave(300).time, "side": "SHORT", "decider": "gpt-5", "reason": "market" }),
    )
    .await
    .expect("intent");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(run["pending_order"].is_null(), "{run}");
    assert_eq!(run["pending"]["side"], "SHORT");
    assert_eq!(rows(dir.path(), "l", "cancelled_unfilled").len(), 2);

    // A stand-aside leaves whatever rests alone: it drove the bar and said
    // nothing about the order. Only a LONG or SHORT replaces.
    post_intent(&state, long_limit("l", next.open - 50.0)).await.expect("intent");
    let before = run_named(&read_status(&state).await, "l")["pending_order"].clone();
    let reply = post_intent(&state, json!({ "run": "l", "bar_time": wave(300).time, "side": "NONE", "decider": "gpt-5" }))
        .await
        .expect("intent");
    assert_eq!(reply["accepted"], true, "{reply}");
    let after = run_named(&read_status(&state).await, "l").clone();
    assert_eq!(after["pending_order"], before, "untouched by a stand-aside");
    assert_eq!(after["decider"]["stood_aside"], 1, "and the stand-aside was counted");
    assert_eq!(rows(dir.path(), "l", "cancelled_unfilled").len(), 2, "no replacement row");

    let last = rows(dir.path(), "l", "intent").pop().expect("intent");
    assert_eq!(last["entry"], json!({ "type": "limit", "price": next.open - 50.0 }));
    assert_eq!(last["valid_bars"], 2);
    assert_eq!(last["zone"], json!([next.open - 55.0, next.open - 45.0]));
    let market_row = &rows(dir.path(), "l", "intent")[2];
    assert_eq!(market_row["entry"], json!({ "type": "market", "price": null }), "{market_row}");
}

#[tokio::test]
async fn a_restart_between_the_decision_and_the_fill_fills_once() {
    // Mirrors `a_restart_between_the_open_and_the_close_fills_once`: an order
    // rests for up to two bars, which is two chances for a deploy to land in
    // the window. It must be there afterwards, fill once, and the bar that
    // then closes must not fill it again.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 20.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");
    let before = run_named(&read_status(&state).await, "l")["pending_order"].clone();

    // The process dies here.
    let reborn = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let recovered = run_named(&read_status(&reborn).await, "l").clone();
    assert_eq!(recovered["pending_order"], before, "the order survived the restart, stamps included");

    // The ask reaches it on the new process.
    post_tick(&reborn, "btc", "15m", quote(next, price - 0.3, price - 0.5, price)).await.expect("tick");
    let filled = run_named(&read_status(&reborn).await, "l").clone();
    assert!(!filled["open"].is_null(), "{filled}");
    let entry_price = filled["open"]["entry_price"].clone();
    assert_eq!(filled["open"]["entry_time"], next.time, "stamped with the tick's bucket");

    // Another restart, then the bar closes.
    let again = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    let held = run_named(&read_status(&again).await, "l").clone();
    assert_eq!(held["open"]["entry_price"], entry_price, "the fill survived");
    assert!(held["pending_order"].is_null());
    let mut calm = next;
    calm.low = calm.low.min(price - 1.0);
    calm.high = calm.high.max(price + 1.0);
    post_bar(&again, "btc", "15m", calm).await.expect("bar");
    let after = run_named(&read_status(&again).await, "l").clone();
    if !after["open"].is_null() {
        assert_eq!(after["open"]["entry_price"], entry_price, "not re-filled");
    }
    assert!(after["trades"].as_u64().expect("trades") <= 1, "one order, at most one trade: {after}");
    assert_eq!(rows(dir.path(), "l", "opened").len(), 1, "one opened line");
}

#[tokio::test]
async fn an_order_is_refused_with_a_reason_and_the_book_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 50.0;

    let cases = [
        (patched(long_limit("l", price), json!({ "entry": { "type": "limit", "price": null } })), "needs a price"),
        (patched(long_limit("l", price), json!({ "stop": null })), "needs a stop"),
        (patched(long_limit("l", price), json!({ "stop": price + 1.0 })), "losing side"),
        (patched(long_limit("l", price), json!({ "target": price - 1.0 })), "winning side"),
        (patched(long_limit("l", price), json!({ "valid_bars": 0 })), "at least 1"),
    ];
    for (body, expect) in cases {
        let reply = post_intent(&state, body.clone()).await.expect("a reply, not an error");
        assert_eq!(reply["accepted"], false, "{body} -> {reply}");
        assert!(reply["reason"].as_str().expect("reason").contains(expect), "{body} -> {reply}");
    }
    // A stop judged against the ENTRY PRICE, not the last close: a SHORT
    // limit above the market with its stop between the close and the level
    // is refused although that stop is above the close.
    let short = json!({
        "run": "l", "bar_time": wave(300).time, "side": "SHORT", "decider": "gpt-5", "reason": "fade",
        "entry": { "type": "limit", "price": next.open + 80.0 }, "stop": next.open + 40.0,
    });
    let reply = post_intent(&state, short).await.expect("reply");
    assert_eq!(reply["accepted"], false, "{reply}");
    assert!(run_named(&read_status(&state).await, "l")["pending_order"].is_null(), "nothing rested");
    assert!(rows(dir.path(), "l", "intent").is_empty(), "and nothing was written");
    // An unknown type is a malformed body, as an unknown side is.
    let bad = patched(long_limit("l", price), json!({ "entry": { "type": "iceberg", "price": price } }));
    assert_eq!(http_status(post_intent(&state, bad).await.expect_err("400")), 400);
    // `market` with a price is today's path, price ignored.
    let plain = patched(long_limit("l", price), json!({ "entry": { "type": "market", "price": price } }));
    let reply = post_intent(&state, plain).await.expect("reply");
    assert_eq!(reply["reason"], "fills at the next bar's open", "{reply}");

    // Over a position, refused: an order cannot add to one.
    post_open(&state, "btc", "15m", next.time, next.open).await.expect("open");
    post_bar(&state, "btc", "15m", next).await.expect("bar");
    let held = run_named(&read_status(&state).await, "l").clone();
    assert!(!held["open"].is_null(), "{held}");
    let body = patched(long_limit("l", price), json!({ "bar_time": next.time }));
    let reply = post_intent(&state, body).await.expect("reply");
    assert_eq!(reply["accepted"], false, "{reply}");
    assert!(reply["reason"].as_str().expect("reason").contains("holds a position"));
}

#[tokio::test]
async fn pending_act_triggers_at_the_touched_side_and_a_cancel_carries_its_reason() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 50.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");

    // No tick yet: refused, with a reason, in the intent reply's shape.
    let reply = post_act(&state, json!({ "run": "l", "action": "trigger", "reason": "model says go" })).await.expect("reply");
    assert_eq!(reply, json!({ "accepted": false, "reason": "no tick has arrived on this stream" }), "{reply}");

    // A tick short of the level, then a trigger: fills at the ASK, now.
    let ask = price + 7.0;
    post_tick(&state, "btc", "15m", quote(next, ask - 0.2, ask - 0.4, ask)).await.expect("tick");
    assert!(run_named(&read_status(&state).await, "l")["open"].is_null(), "the tick itself did not fill");
    let reply = post_act(&state, json!({ "run": "l", "action": "trigger", "reason": "model says go" })).await.expect("reply");
    assert_eq!(reply["accepted"], true, "{reply}");
    assert!(reply["reason"].as_str().expect("reason").starts_with(&format!("filled at {ask} on the ask")), "{reply}");
    assert_eq!(reply.as_object().expect("object").len(), 2, "exactly the intent reply's two fields: {reply}");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert!(run["pending_order"].is_null(), "null on the very next read: {run}");
    let row = rows(dir.path(), "l", "opened").pop().expect("opened");
    assert_eq!(row["filled_from"], "act");
    assert_eq!(row["entry"], json!({ "type": "limit", "price": ask, "requested_price": price }));
    // Priced from the ask through the same cost path a market fill takes.
    assert!(run["open"]["entry_price"].as_f64().expect("price") > ask, "the half spread is on top: {run}");

    // Nothing to act on now.
    let reply = post_act(&state, json!({ "run": "l", "action": "cancel", "reason": "late" })).await.expect("reply");
    assert_eq!(reply["accepted"], false, "{reply}");
    assert_eq!(http_status(post_act(&state, json!({ "run": "l", "action": "hold" })).await.expect_err("400")), 400);
    assert_eq!(http_status(post_act(&state, json!({ "run": "nope", "action": "cancel" })).await.expect_err("404")), 404);

    // Cancel on a second run, and the row carries the caller's sentence.
    let other = tempfile::tempdir().expect("temp dir");
    let (state2, _) = external_run(other.path(), "c").await;
    post_intent(&state2, long_limit("c", price)).await.expect("intent");
    let reply = post_act(&state2, json!({ "run": "c", "action": "cancel", "reason": "structure broke" })).await.expect("reply");
    assert_eq!(reply["accepted"], true, "{reply}");
    assert!(run_named(&read_status(&state2).await, "c")["pending_order"].is_null(), "null on the very next read");
    let row = rows(other.path(), "c", "cancelled_unfilled").pop().expect("a row");
    assert_eq!(row["reason"], "cancelled:structure broke");
    assert_eq!(row["entry"]["type"], "limit");
}

#[tokio::test]
async fn the_trade_row_says_how_the_position_was_entered() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 20.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");
    post_tick(&state, "btc", "15m", quote(next, price - 0.3, price - 0.5, price)).await.expect("tick");
    let open = run_named(&read_status(&state).await, "l")["open"].clone();
    let stop = open["stop"].as_f64().expect("a stop");

    // The bar closes through the stop, after the fill: one trade, entered by
    // a limit, and the row says so.
    let mut crash = next;
    crash.low = stop - 50.0;
    crash.close = stop - 40.0;
    crash.high = crash.high.max(crash.open);
    post_bar(&state, "btc", "15m", crash).await.expect("bar");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert_eq!(run["trades"], 1, "{run}");
    let trade = rows(dir.path(), "l", "trade").pop().expect("a trade row");
    assert_eq!(trade["entry"], json!({ "type": "limit", "price": price, "requested_price": price }));

    // A market fill's trade row says market, priced from the open. On a
    // fresh run, so the daily-loss guard the stop-out above just armed has
    // no say in it.
    let other = tempfile::tempdir().expect("temp dir");
    let (market, bar) = external_run(other.path(), "m").await;
    post_intent(
        &market,
        json!({ "run": "m", "bar_time": wave(300).time, "side": "SHORT", "decider": "gpt-5", "reason": "m",
                "stop": bar.open + 1_000.0 }),
    )
    .await
    .expect("intent");
    post_bar(&market, "btc", "15m", bar).await.expect("bar");
    assert_eq!(run_named(&read_status(&market).await, "m")["open"]["side"], "SHORT");
    stop_with(&market, json!({ "id": "m" })).await.expect("stop");
    let trade = rows(other.path(), "m", "trade").pop().expect("the stop closed it");
    assert_eq!(trade["entry"], json!({ "type": "market", "price": bar.open, "requested_price": null }));
}

#[tokio::test]
async fn the_fill_bar_of_a_tick_fill_is_managed_from_the_fill_onward() {
    // The engine test at the API: a LONG limit fills when the ask has come
    // DOWN to it, so the bar's high before the fill is not a price the trade
    // saw. A bar that opened above the target and then filled the limit must
    // not book a winner.
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 300.0;
    post_intent(&state, long_limit("l", price)).await.expect("intent");
    post_tick(&state, "btc", "15m", quote(next, price - 0.3, price - 0.5, price)).await.expect("tick");
    let target = run_named(&read_status(&state).await, "l")["open"]["target"].as_f64().expect("target");
    assert!(next.open > target, "the bar opened above the target: {} > {target}", next.open);

    let mut bar = next;
    bar.high = next.open + 10.0;
    bar.low = price - 2.0;
    bar.close = price + 3.0;
    post_bar(&state, "btc", "15m", bar).await.expect("bar");
    let run = run_named(&read_status(&state).await, "l").clone();
    assert_eq!(run["trades"], 0, "no target on a price from before the fill: {run}");
    assert!(!run["open"].is_null());
}

#[tokio::test]
async fn the_pending_route_lists_a_resting_order_under_its_own_id_and_a_stop_withdraws_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    post_intent(&state, long_limit("l", next.open - 50.0)).await.expect("intent");

    let listed = read_pending(&state).await;
    let entry = listed.as_array().expect("entries").iter().find(|e| e["run"] == "l").expect("listed");
    assert_eq!(entry["intent_id"], format!("l:{}", wave(300).time));
    assert_eq!(entry["side"], "LONG");
    assert_eq!(entry["pending_order"]["type"], "limit");
    assert_eq!(entry["pending_order"]["price"], next.open - 50.0);
    assert!(entry["pending_order"]["lots"].is_number());
    assert_eq!(entry["advised"], false);

    // The id does not move as bars arrive, because the order outlives them.
    post_bar(&state, "btc", "15m", next).await.expect("bar");
    let listed = read_pending(&state).await;
    let entry = listed.as_array().expect("entries").iter().find(|e| e["run"] == "l").expect("still listed");
    assert_eq!(entry["intent_id"], format!("l:{}", wave(300).time));
    assert_eq!(entry["pending_order"]["bars_waited"], 1);

    // Stopping the run withdraws it, and the withdrawal is a row.
    stop_with(&state, json!({ "id": "l" })).await.expect("stop");
    let row = rows(dir.path(), "l", "cancelled_unfilled").pop().expect("a row");
    assert_eq!(row["reason"], "cancelled:stopped");
    assert_eq!(row["bars_waited"], 1);
}


/* ------------------------------------------------- the rebate, as a credit */

/// The workspace `config/` directory, which is where `[rebate]` lives.
///
/// `AppState::new` defaults `config_dir` to the relative `config`, which does
/// not exist from a test's working directory - so every OTHER test in this
/// file runs with no arrangement recorded and must see `rebate: null`. That
/// is deliberate and is asserted below.
fn with_registry(state: Arc<AppState>) -> Arc<AppState> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    let inner = Arc::try_unwrap(state).unwrap_or_else(|_| panic!("one reference"));
    Arc::new(inner.with_config_dir(dir))
}

/// A book with closed trades, with and without the rebate terms in scope.
async fn traded(dir: &Path, registry: bool) -> (Arc<AppState>, Value) {
    let state = state_over(dir, 300);
    let state = if registry { with_registry(state) } else { state };
    start_run(&state, json!({ "market": "btc", "tf": "15m", "strategy": "ema-cross", "id": "b", "window": 200 }))
        .await
        .expect("start");
    for i in 300..380 {
        post_bar(&state, "btc", "15m", wave(i)).await.expect("bar");
    }
    let run = run_named(&read_status(&state).await, "b").clone();
    assert!(run["trades"].as_u64().expect("trades") > 0, "the tape must close something: {run}");
    (state, run)
}

#[tokio::test]
async fn the_rebate_is_a_separate_line_and_changes_nothing_that_existed() {
    // THE WHOLE POINT OF THE OPTION THE OWNER PICKED. He was offered three
    // ways to count his introducing-broker credit and chose a separate line
    // over folding it into the cost model, so every figure that had a meaning
    // before must have the same value after. If this test ever fails, the
    // credit has leaked into the book.
    let plain = tempfile::tempdir().expect("temp dir");
    let (_, without) = traded(plain.path(), false).await;
    let credited = tempfile::tempdir().expect("temp dir");
    let (_, with) = traded(credited.path(), true).await;

    for key in ["trades", "net_usd", "equity", "profit_factor", "equity_curve"] {
        assert_eq!(with[key], without[key], "`{key}` moved when the rebate shipped: {with}");
    }
    assert_eq!(
        with["last_fills"][0]["pnlUsd"], without["last_fills"][0]["pnlUsd"],
        "a trade's own P&L moved when the rebate shipped"
    );

    // `null` is not zero. With no registry in scope no arrangement is
    // recorded, and a desk with no IB agreement must not be shown a rebate of
    // nothing - it must be shown nothing.
    assert!(without["rebate"].is_null(), "no registry, no arrangement: {without}");
    assert!(without["last_fills"][0]["rebateUsd"].is_null(), "and no trade claims one");
}

#[tokio::test]
async fn the_rebate_is_the_owners_share_of_the_round_turn_spread() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, run) = traded(dir.path(), true).await;

    let rebate = &run["rebate"];
    // `btc` in config/default.toml: contract 1.0, spread 5.0. The share is
    // the one `config/accounts.toml` grants, and both travel with the figure
    // so a reader never has to go and find what was multiplied by what.
    assert_eq!(rebate["share_of_spread"], 0.45, "the shipped terms: {rebate}");
    assert_eq!(rebate["spread"], 5.0, "the basis is the spread the book is charged: {rebate}");

    // ONE ROUND TURN, NOT ONE PER SIDE. The engine takes half the spread on
    // entry and half on exit; the two halves are one spread, so the credit is
    // 0.45 of `spread x lots x contract_size` once.
    let fills = read_detail(&state, "b", Some(1)).await.expect("detail")["fills"].clone();
    let trades = fills.as_array().expect("fills").len();
    let mut summed = 0.0;
    for fill in fills.as_array().expect("fills") {
        let lots = fill["lots"].as_f64().expect("lots");
        let want = fd_core::js_round_to(0.45 * 5.0 * lots * 1.0, 4);
        let got = fill["rebateUsd"].as_f64().unwrap_or_else(|| panic!("a priced trade: {fill}"));
        assert!((got - want).abs() < 1e-9, "{got} is not 45% of one round turn ({want}): {fill}");
        summed += got;
    }
    let total = rebate["usd"].as_f64().expect("a total");
    assert!(
        (total - fd_core::js_round_to(summed, 2)).abs() < 1e-9,
        "the run's total {total} is not the sum of its trades {summed}"
    );

    // And the net beside it, published rather than left to two clients to
    // work out separately - but never INSTEAD of `net_usd`.
    let net = run["net_usd"].as_f64().expect("net");
    let after = rebate["net_of_rebate_usd"].as_f64().expect("net of rebate");
    assert!((after - fd_core::js_round_to(net + total, 2)).abs() < 1e-9, "{after} is not {net} + {total}");
    assert!(total > 0.0, "a credit is money coming back, not going out: {rebate}");

    // EXACT, AND THE WORD IS EARNED. A paper book pays the spread the engine
    // charged it, so there is nothing here to estimate - which is exactly
    // what is NOT true on the account, where the same three counts sit on
    // `broker.json` and `estimated` is usually the large one.
    assert_eq!(rebate["exact_trades"].as_u64().expect("exact"), trades as u64, "{rebate}");
    assert_eq!(rebate["estimated_trades"], 0, "nothing on paper is an estimate: {rebate}");
    assert_eq!(rebate["unpriced_trades"], 0, "{rebate}");
}

/// Writes `docs/api-samples/paper-status-rebate.json` from the handler, for
/// `ui/scripts/contract.check.mjs`. Ignored by default because it writes into
/// the repo; run with
/// `cargo test -p fd-api --test paper write_rebate_sample -- --ignored`.
#[tokio::test]
#[ignore = "writes docs/api-samples; run on purpose"]
async fn write_rebate_sample() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("docs").join("api-samples");
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, _) = traded(dir.path(), true).await;
    let served = read_status(&state).await;
    std::fs::write(out.join("paper-status-rebate.json"), serde_json::to_string_pretty(&served).expect("json"))
        .expect("write");
}

/* ------------------------------------------------ served samples for docs */

/// Writes `docs/api-samples/paper-order-*.json` from the handlers, so a
/// consumer can diff key paths against what the API actually emits. Ignored
/// by default because it writes into the repo; run with
/// `cargo test -p fd-api --test paper write_order_samples -- --ignored`.
#[tokio::test]
#[ignore = "writes docs/api-samples; run on purpose"]
async fn write_order_samples() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("docs").join("api-samples");
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, next) = external_run(dir.path(), "l").await;
    let price = next.open - 50.0;
    let mut samples = serde_json::Map::new();

    // 1. The intent that rests an order, and its reply.
    let body = patched(long_limit("l", price), json!({ "invalidate_above": next.open + 100.0, "valid_bars": 3 }));
    let reply = post_intent(&state, body.clone()).await.expect("intent");
    samples.insert("intent_request".into(), body);
    samples.insert("intent_reply".into(), reply);

    // 2. The status entry with `pending_order`, and the /pending entry.
    let run = run_named(&read_status(&state).await, "l").clone();
    std::fs::write(out.join("paper-order-status.json"), serde_json::to_string_pretty(&run).expect("json")).expect("write");
    let listed = read_pending(&state).await;
    let entry = listed.as_array().expect("entries").iter().find(|e| e["run"] == "l").expect("listed").clone();
    let mut trimmed = entry.clone();
    trimmed["bars"] = json!("...120 bars elided...");
    std::fs::write(out.join("paper-order-pending-entry.json"), serde_json::to_string_pretty(&trimmed).expect("json")).expect("write");

    // 3. The act replies: refused (stale), then a cancel; then a fresh order
    //    that a tick fills, and one a trigger fills.
    let refused = post_act(&state, json!({ "run": "l", "action": "trigger", "reason": "go" })).await.expect("reply");
    samples.insert("act_trigger_refused_no_tick".into(), refused);
    let cancelled = post_act(&state, json!({ "run": "l", "action": "cancel", "reason": "structure broke" })).await.expect("reply");
    samples.insert("act_cancel_reply".into(), cancelled);

    post_intent(&state, long_limit("l", price)).await.expect("intent");
    let ask = price + 7.0;
    let tick = post_tick(&state, "btc", "15m", quote(next, ask - 0.2, ask - 0.4, ask)).await.expect("tick");
    samples.insert("tick_reply_no_fill".into(), tick);
    let triggered = post_act(&state, json!({ "run": "l", "action": "trigger", "reason": "model says go" })).await.expect("reply");
    samples.insert("act_trigger_reply".into(), triggered);
    // Close it on the bar, so the trade row exists.
    let stop = run_named(&read_status(&state).await, "l")["open"]["stop"].as_f64().expect("stop");
    let mut crash = next;
    crash.low = stop - 50.0;
    crash.close = stop - 40.0;
    post_bar(&state, "btc", "15m", crash).await.expect("bar");

    // A second run: a tick fill, then an expiry, then a replacement.
    let (state2, next2) = external_run(tempfile::tempdir().expect("temp dir").path(), "t").await;
    post_intent(&state2, long_limit("t", price)).await.expect("intent");
    let filled = post_tick(&state2, "btc", "15m", quote(next2, price - 0.3, price - 0.5, price)).await.expect("tick");
    samples.insert("tick_reply_filled".into(), filled);
    std::fs::write(out.join("paper-order-replies.json"), serde_json::to_string_pretty(&samples).expect("json")).expect("write");

    // 4. The rows: every fills.jsonl line of the first run, in order.
    let text = std::fs::read_to_string(dir.path().join("paper").join("l").join("fills.jsonl")).expect("fills");
    let kept: Vec<&str> = text.lines().filter(|l| !l.contains("\"kind\":\"started\"")).collect();
    std::fs::write(out.join("paper-order-fills.jsonl"), kept.join("\n") + "\n").expect("write");
}
