//! Minute bars folded from the tick feed.
//!
//! The aggregator is driven with explicit stamps, because the tick handler
//! stamps every tick with the server's own clock and a test cannot move that.
//! The route is then read the way a client would, so what is asserted is the
//! JSON a trigger call will see and not the aggregator's private state. One
//! test goes through the handler itself, to pin the hook in `paper::tick`
//! and the rule that two pollers posting one quote make one tick.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use fd_api::m1::{Fold, M1Query, RING_MINUTES, m1};
use fd_api::paper::tick;
use fd_api::{ApiError, AppState};
use fd_core::config::Config;
use serde_json::{Value, json};

/// Midnight UTC, 2026-09-07 (a Monday), so every minute below is aligned by
/// construction and the boundary under test is the only boundary.
const START: i64 = 1_788_000_000_000 / 86_400_000 * 86_400_000;
const MINUTE: i64 = 60_000;

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

fn state() -> Arc<AppState> {
    let dir = tempfile::tempdir().expect("temp dir");
    // The store is empty on purpose: nothing here reads a stored bar, and the
    // route must answer without one.
    Arc::new(AppState::new(config(), dir.keep()))
}

/// A quote `spread` wide around `mid`, stamped `at`.
fn quote(state: &AppState, market: &str, at: i64, mid: f64, spread: f64) -> Fold {
    state.m1.fold(market, at, Some(mid - spread / 2.0), Some(mid + spread / 2.0))
}

async fn read(state: &Arc<AppState>, market: &str, n: Option<usize>) -> Result<Value, ApiError> {
    m1(State(Arc::clone(state)), Query(M1Query { market: market.to_string(), n }))
        .await
        .map(|Json(v)| serde_json::to_value(v).expect("json"))
}

fn http_status(err: ApiError) -> u16 {
    err.into_response().status().as_u16()
}

#[tokio::test]
async fn ticks_across_a_minute_boundary_close_one_bar_and_start_the_next() {
    let state = state();
    // Four quotes in the first minute: open 4300, a high of 4302, a low of
    // 4299, close 4301 — then one in the next minute, which is what closes it.
    assert_eq!(quote(&state, "xauusd", START + 1_000, 4300.0, 0.20), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + 15_000, 4302.0, 0.30), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + 40_000, 4299.0, 0.40), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + 59_999, 4301.0, 0.30), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + MINUTE + 500, 4301.5, 0.20), Fold::Folded);

    let v = read(&state, "xauusd", Some(30)).await.expect("m1");
    assert_eq!(v["tf"], "1m");
    assert_eq!(v["source"], "ticks");
    assert_eq!(v["symbol"], "XAUUSD", "the config's bar symbol says whose quote the mid is");
    assert!(v["unavailable"].is_null(), "there is data: {v}");

    let bars = v["bars"].as_array().expect("bars");
    assert_eq!(bars.len(), 1, "one complete minute: {v}");
    let bar = &bars[0];
    assert_eq!(bar["time"], START, "the minute's open, aligned, not the first tick's stamp");
    assert_eq!(bar["open"], 4300.0);
    assert_eq!(bar["high"], 4302.0);
    assert_eq!(bar["low"], 4299.0);
    assert_eq!(bar["close"], 4301.0);
    assert_eq!(bar["ticks"], 4);
    let spread_mean = bar["spread_mean"].as_f64().expect("spread_mean");
    assert!((spread_mean - 0.30).abs() < 1e-9, "mean of 0.20, 0.30, 0.40, 0.30: {spread_mean}");

    let forming = &v["forming"];
    assert_eq!(forming["time"], START + MINUTE, "the next minute is forming: {v}");
    assert_eq!(forming["open"], 4301.5);
    assert_eq!(forming["ticks"], 1);
    assert_eq!(v["first_minute_ms"], START);
    assert_eq!(v["last_tick_ms"], START + MINUTE + 500);
    assert_eq!(v["dropped"], 0);
    assert_eq!(v["duplicates"], 0);
}

/// A tick is its quote; the stamp only orders. The same quote under the same
/// stamp is one tick, the same quote under a later stamp (the second poller,
/// or a quiet second) is still one tick — and a DIFFERENT quote under the
/// same stamp is a second tick, because the receipt clock has millisecond
/// resolution and two POSTs have already been seen to land in one
/// millisecond (the handler test below, 2026-09-18).
#[tokio::test]
async fn a_duplicate_stamp_is_counted_once_and_the_quote_is_what_makes_a_tick() {
    let state = state();
    assert_eq!(quote(&state, "xauusd", START + 1_000, 4300.0, 0.20), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + 1_000, 4300.0, 0.20), Fold::Duplicate);
    assert_eq!(quote(&state, "xauusd", START + 1_300, 4300.0, 0.20), Fold::Duplicate);
    assert_eq!(quote(&state, "xauusd", START + 1_300, 4305.0, 0.20), Fold::Folded);

    let v = read(&state, "xauusd", None).await.expect("m1");
    assert_eq!(v["forming"]["ticks"], 2, "{v}");
    assert_eq!(v["forming"]["high"], 4305.0, "the new quote under a repeated stamp must reach the bar: {v}");
    assert_eq!(v["duplicates"], 2);
    assert_eq!(v["dropped"], 0, "a duplicate is not a fault");
    assert_eq!(v["last_tick_ms"], START + 1_300, "a duplicate does not advance the clock; the new quote does");
}

#[tokio::test]
async fn an_older_tick_is_ignored_and_counted_in_dropped() {
    let state = state();
    assert_eq!(quote(&state, "xauusd", START + 10_000, 4300.0, 0.20), Fold::Folded);
    assert_eq!(quote(&state, "xauusd", START + 20_000, 4301.0, 0.20), Fold::Folded);
    // Older than the last accepted, with a price that would be a new low if
    // it were folded.
    assert_eq!(quote(&state, "xauusd", START + 15_000, 4290.0, 0.20), Fold::Dropped);
    // A tick with no quote at all is not a price and is refused the same way.
    assert_eq!(state.m1.fold("xauusd", START + 25_000, None, Some(4301.1)), Fold::Dropped);

    let v = read(&state, "xauusd", None).await.expect("m1");
    assert_eq!(v["forming"]["low"], 4300.0, "the older tick must not move the bar: {v}");
    assert_eq!(v["forming"]["ticks"], 2);
    assert_eq!(v["dropped"], 2);
    assert_eq!(v["last_tick_ms"], START + 20_000, "a dropped tick does not advance the clock");
}

#[tokio::test]
async fn the_route_with_no_ticks_answers_unavailable() {
    let state = state();
    let v = read(&state, "xauusd", Some(30)).await.expect("m1");
    assert_eq!(v["bars"].as_array().map(Vec::len), Some(0));
    assert!(v["forming"].is_null());
    assert!(v["first_minute_ms"].is_null());
    assert!(v["last_tick_ms"].is_null());
    let sentence = v["unavailable"].as_str().expect("a sentence, not null");
    assert!(sentence.starts_with("no ticks yet"), "{sentence}");
    assert!(sentence.contains("restart"), "it must say the ring starts empty at a restart: {sentence}");
}

#[tokio::test]
async fn more_than_the_ring_is_refused_and_the_refusal_names_the_ring() {
    let state = state();
    let err = read(&state, "xauusd", Some(RING_MINUTES + 1)).await.expect_err("721 minutes");
    let text = err.to_string();
    assert!(text.contains("720"), "{text}");
    assert_eq!(http_status(err), 400);
    // Exactly the ring is fine.
    read(&state, "xauusd", Some(RING_MINUTES)).await.expect("720 minutes");
}

#[tokio::test]
async fn the_ring_keeps_twelve_hours_and_drops_the_oldest() {
    let state = state();
    // 800 minutes of one tick each; the last opens minute 799 and leaves it
    // forming, so 799 complete of which the ring keeps the newest 720.
    for i in 0..800 {
        let at = START + i * MINUTE + 100;
        assert_eq!(quote(&state, "xauusd", at, 4300.0 + i as f64, 0.20), Fold::Folded);
    }
    let v = read(&state, "xauusd", Some(RING_MINUTES)).await.expect("m1");
    let bars = v["bars"].as_array().expect("bars");
    assert_eq!(bars.len(), RING_MINUTES);
    assert_eq!(v["first_minute_ms"], START + 79 * MINUTE, "minutes 0..79 have fallen off");
    assert_eq!(bars[0]["time"], START + 79 * MINUTE);
    assert_eq!(bars[RING_MINUTES - 1]["time"], START + 798 * MINUTE);
    assert_eq!(v["forming"]["time"], START + 799 * MINUTE);

    // `n` is the newest n, oldest first.
    let v = read(&state, "xauusd", Some(3)).await.expect("m1");
    let bars = v["bars"].as_array().expect("bars");
    assert_eq!(bars.iter().map(|b| b["time"].as_i64().expect("time")).collect::<Vec<_>>(), vec![START + 796 * MINUTE, START + 797 * MINUTE, START + 798 * MINUTE]);
}

/// Through the handler: the hook in `paper::tick` feeds the aggregator, and
/// the same quote from the 15m and the 5m poller is one tick, not two.
#[tokio::test]
async fn two_pollers_posting_one_quote_make_one_tick() {
    let state = state();
    let post = |tf: &'static str, bid: f64, ask: f64| {
        let state = Arc::clone(&state);
        async move {
            let body = json!({
                "market": "xauusd", "tf": tf,
                "bar": { "time": START, "open": 4300.0, "high": 4301.0, "low": 4299.0, "close": 4300.5, "volume": 7.0 },
                "bid": bid, "ask": ask,
            });
            let request = serde_json::from_value(body).expect("a tick body");
            let Json(_) = tick(State(state), Json(request)).await.expect("tick stored");
        }
    };
    post("15m", 4300.4, 4300.6).await;
    post("5m", 4300.4, 4300.6).await;
    post("15m", 4300.5, 4300.7).await;
    post("5m", 4300.5, 4300.7).await;

    let v = read(&state, "xauusd", None).await.expect("m1");
    let forming = &v["forming"];
    assert!(!forming.is_null(), "the hook must feed the aggregator: {v}");
    assert_eq!(forming["ticks"], 2, "two quotes, each posted by two pollers: {v}");
    assert_eq!(forming["open"], 4300.5, "the mid of the posted quote, not the bar's close");
    assert_eq!(forming["close"], 4300.6);
    assert_eq!(v["duplicates"], 2);
    assert_eq!(v["dropped"], 0);

    // Each market has its own series; the other market has heard nothing.
    let other = read(&state, "btc", None).await.expect("m1");
    assert!(other["unavailable"].is_string(), "{other}");
}
