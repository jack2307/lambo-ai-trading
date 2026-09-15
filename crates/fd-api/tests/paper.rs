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
use fd_api::paper::{DEFAULT_DETAIL_BARS, DetailQuery, MAX_LIVE_AGE_MS, bar, detail, intent, start, status, stop, tick};
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
