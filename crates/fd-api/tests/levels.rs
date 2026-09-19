//! `/api/paper/levels`, read the way a caller reads it.
//!
//! Two things are pinned here and they are different things. One is the
//! ABSENT case: a market whose bars have never been exported must answer a
//! sentence, not an empty list, because an empty list is a measurement ("this
//! window has no unfilled gaps") and a missing file is not a measurement at
//! all. The other is the SHAPE, checked field for field against the list in
//! `docs/hypotheses/2026-09-18-smc-context.md` lines 62-66 — the registration
//! was written before the route existed, so the contract is the older
//! document and this test reads it rather than the code.
//!
//! The shape is asserted on the SERVED JSON and not on the Rust types. On
//! 2026-09-18 a contract for `/api/chart/bars` was agreed, verified by
//! serving the response, and was still wrong: `exported_at_ms` sat one level
//! deeper than the contract said and the client read `undefined` forever. The
//! names were checked; the NESTING was never stated out loud. So these
//! assertions index by path.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use fd_api::levels::{LevelsQuery, levels};
use fd_api::{ApiError, AppState};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::write_bars;
use serde_json::Value;

const M15: i64 = 900_000;
const DAY: i64 = 86_400_000;
/// Midnight UTC on a Monday, so the weekend hole below falls where a reader
/// would expect it and no assertion turns into an argument about boundaries.
const START: i64 = 1_788_000_000_000 / DAY * DAY;
/// Bars in one of the fixture's trading days: ten hours of quarter-hours.
const BARS_PER_DAY: usize = 40;
/// Trading days in the fixture: two broker weeks, which is the smallest
/// fixture in which "the last complete week" exists at all.
const DAYS: usize = 10;

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

/// Ten trading days of 15m bars with the two holes the route's rules read:
/// fourteen hours between one day and the next, and a weekend after every
/// fifth.
///
/// Prices are a seeded random walk and NOT a sine, which is the second
/// fixture this test had. A sine's highs tie: with `high = max(open, close) +
/// pad`, the bar at the peak and the bar after it carry the SAME high, and
/// `fd_indicators::swing` is strict on both sides on purpose — a flat top is
/// not a swing. So the sine version produced ten days of bars and not one
/// swing high, and the liquidity list came back holding only the period
/// extremes. Written down because the fixture looked obviously fine and the
/// failure looked like a bug in the route.
fn fixture() -> Vec<Bar> {
    // A 64-bit LCG, so the series is the same on every machine and no two
    // adjacent highs are ever equal by construction.
    let mut seed: u64 = 0x5eed_1234_5678_9abc;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 40) as f64 / 16_777_216.0
    };

    let mut bars: Vec<Bar> = Vec::new();
    let mut prev = 4300.0;
    for d in 0..DAYS {
        let day_index = (d / 5) * 7 + (d % 5);
        for i in 0..BARS_PER_DAY {
            let close = prev + (next() - 0.5) * 4.0;
            let time = START + day_index as i64 * DAY + i as i64 * M15;
            bars.push(Bar {
                time,
                open: prev,
                high: prev.max(close) + next() * 2.0,
                low: prev.min(close) - next() * 2.0,
                close,
                volume: None,
            });
            prev = close;
        }
    }

    // The last eight bars are written out by hand, so the gap, the block and
    // the displacement in the assertions below are visible on the page rather
    // than emergent from a sine. Reading down the closes: two down candles, a
    // 26-dollar impulse, then a drift that never comes back.
    //
    //   n-8  4300 / 4302 / 4298 / 4299   down
    //   n-7  4299 / 4301 / 4297 / 4298   down  <- the order block
    //   n-6  4298 / 4303 / 4297 / 4302
    //   n-5  4302 / 4330 / 4302 / 4328   the displacement, and the gap's middle
    //   n-4  4328 / 4335 / 4320 / 4332   low 4320 > 4303: a bullish gap, 4303..4320
    //   n-3  4332 / 4338 / 4328 / 4336
    //   n-2  4336 / 4340 / 4330 / 4338
    //   n-1  4338 / 4342 / 4333 / 4340   nothing returns below 4320, so it stays unfilled
    let tail = [
        (4300.0, 4302.0, 4298.0, 4299.0),
        (4299.0, 4301.0, 4297.0, 4298.0),
        (4298.0, 4303.0, 4297.0, 4302.0),
        (4302.0, 4330.0, 4302.0, 4328.0),
        (4328.0, 4335.0, 4320.0, 4332.0),
        (4332.0, 4338.0, 4328.0, 4336.0),
        (4336.0, 4340.0, 4330.0, 4338.0),
        (4338.0, 4342.0, 4333.0, 4340.0),
    ];
    let n = bars.len();
    for (k, (open, high, low, close)) in tail.into_iter().enumerate() {
        let b = &mut bars[n - tail.len() + k];
        b.open = open;
        b.high = high;
        b.low = low;
        b.close = close;
    }
    bars
}

fn state_with_bars(bars: Option<&[Bar]>) -> Arc<AppState> {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.keep();
    if let Some(bars) = bars {
        write_bars(&root.join("bars").join("XAUUSD-15m.parquet"), bars).expect("write");
    }
    Arc::new(AppState::new(config(), root))
}

async fn read(state: &Arc<AppState>, market: &str, tf: Option<&str>, days: Option<usize>) -> Result<Value, ApiError> {
    levels(
        State(Arc::clone(state)),
        Query(LevelsQuery { market: market.to_string(), tf: tf.map(str::to_string), days }),
    )
    .await
    .map(|Json(v)| serde_json::to_value(v).expect("json"))
}

fn http_status(err: ApiError) -> u16 {
    err.into_response().status().as_u16()
}

/// The five fields the registration says EVERY level carries, checked on one
/// level object. Lines 62-66: "a price or a band, the bar it formed on, its
/// age, its state, and the rule that produced it".
fn assert_carries_the_five(level: &Value, what: &str) {
    assert!(level.get("price").is_some(), "{what}: no price field: {level}");
    assert!(level.get("band_low").is_some(), "{what}: no band_low field: {level}");
    assert!(level.get("band_high").is_some(), "{what}: no band_high field: {level}");
    let priced = !level["price"].is_null();
    let banded = !level["band_low"].is_null() && !level["band_high"].is_null();
    assert!(priced || banded, "{what}: a level must be a price OR a band: {level}");
    assert!(level["formed_at_bar_ms"].as_i64().is_some(), "{what}: no forming bar: {level}");
    assert!(level["age_bars"].as_u64().is_some(), "{what}: no age in bars: {level}");
    assert!(level["state"].as_str().is_some(), "{what}: no state: {level}");
    let rule = level["rule"].as_str().unwrap_or_default();
    assert!(!rule.is_empty(), "{what}: a level with no rule is a level nobody can argue with: {level}");
}

#[tokio::test]
async fn a_market_with_no_stored_bars_says_so_rather_than_answering_an_empty_list() {
    let state = state_with_bars(None);
    let v = read(&state, "xauusd", None, None).await.expect("200 for a known market");

    let why = v["unavailable"].as_str().expect("a sentence: {v}");
    assert!(why.contains("15m"), "the sentence must name the timeframe: {why}");
    assert!(why.contains("XAUUSD-15m.parquet"), "and the file: {why}");
    assert!(why.contains("mt5_export.py"), "and how to fix it: {why}");
    assert!(why.contains("M15"), "with the MT5 name of the timeframe, not ours: {why}");

    // NULL, not []. An empty list says "this window has no unfilled gaps",
    // which is a measurement; a missing file has measured nothing. A card
    // that cannot tell them apart renders "no gaps" over a market it has
    // never looked at.
    for key in ["profile", "fair_value_gaps", "order_blocks", "liquidity", "extremes", "source", "window"] {
        assert!(v[key].is_null(), "{key} must be null and not empty when there are no bars: {v}");
    }
    assert!(v["atr14"].is_null());
    assert!(v["last_close"].is_null());
    assert!(v["computed_at_bar_ms"].is_null());
    // The things that are known without a bar are still answered.
    assert_eq!(v["market"], "xauusd");
    assert_eq!(v["timeframe"], "15m");
    assert_eq!(v["bar_ms"], 900_000);
}

#[tokio::test]
async fn the_json_carries_the_registrations_list_field_for_field() {
    let bars = fixture();
    let state = state_with_bars(Some(&bars));
    let v = read(&state, "xauusd", Some("15m"), None).await.expect("levels");
    assert!(v["unavailable"].is_null(), "there are bars: {v}");

    // Provenance, the same three facts `/api/paper/htf` carries.
    assert_eq!(v["source"]["file"], "bars/XAUUSD-15m.parquet");
    assert_eq!(v["source"]["bars"], (DAYS * BARS_PER_DAY) as u64);
    assert_eq!(v["source"]["timeframe"], "15m");
    assert_eq!(v["computed_at_bar_ms"], bars.last().unwrap().time);
    assert_eq!(v["last_close"], 4340.0);

    // The denominator of every `*_atr` field on the response.
    let atr = v["atr14"].as_f64().expect("atr14");
    assert!(atr > 0.0 && atr < 26.0, "the planted displacement must clear one ATR: {atr}");

    // 1. POC, VAH and VAL from an activity profile over price bars.
    let profile = &v["profile"];
    assert_eq!(profile["measure"], "TIME_AT_PRICE", "not volume: the feed's volume is unreliable");
    assert_eq!(profile["value_area_pct"], 0.7);
    assert_eq!(profile["buckets_per_atr"], 4.0);
    let bucket = profile["bucket_size_price"].as_f64().expect("bucket_size_price");
    assert!((bucket - atr / 4.0).abs() < 1e-9, "the bucket must follow ATR: {bucket} vs {atr}");
    for (key, kind) in [("poc", "POC"), ("vah", "VAH"), ("val", "VAL")] {
        assert_eq!(profile[key]["kind"], kind, "{key}: {profile}");
        assert_eq!(profile[key]["state"], "CURRENT");
        assert_carries_the_five(&profile[key], key);
    }
    let held = profile["activity_in_value_area_bar_buckets"].as_f64().expect("in area")
        / profile["activity_total_bar_buckets"].as_f64().expect("total");
    assert!(held >= 0.7, "the value area must enclose 70% of the activity, got {held}");

    // 2. Unfilled fair value gaps, with the fraction filled so far.
    let gaps = v["fair_value_gaps"].as_array().expect("a list");
    let planted = gaps
        .iter()
        .find(|g| g["band_low"] == 4303.0 && g["band_high"] == 4320.0)
        .unwrap_or_else(|| panic!("the planted 4303..4320 gap: {gaps:#?}"));
    assert_eq!(planted["kind"], "FAIR_VALUE_GAP");
    assert_eq!(planted["direction"], "BULLISH");
    assert_eq!(planted["state"], "UNFILLED");
    assert_eq!(planted["filled_fraction"], 0.0);
    assert!(planted["price"].is_null(), "a gap is a band and has no single price");
    assert!(planted["confirmed_at_bar_ms"].as_i64().is_some());
    for g in gaps {
        assert_carries_the_five(g, "fair value gap");
        let filled = g["filled_fraction"].as_f64().expect("filled_fraction");
        assert!((0.0..1.0).contains(&filled), "a filled gap must not be reported: {g}");
    }

    // 3. Order blocks, with untested / tested / broken.
    let blocks = v["order_blocks"].as_array().expect("a list");
    let planted = blocks
        .iter()
        .find(|b| b["band_low"] == 4297.0 && b["band_high"] == 4301.0)
        .unwrap_or_else(|| panic!("the down candle before the 26-dollar impulse: {blocks:#?}"));
    assert_eq!(planted["kind"], "ORDER_BLOCK");
    assert_eq!(planted["direction"], "BULLISH", "a bullish block is the DOWN candle an up move left");
    assert_eq!(planted["state"], "UNTESTED", "nothing traded back into 4297..4301");
    let body = planted["displacement_body_atr"].as_f64().expect("displacement_body_atr");
    assert!(body > 1.0, "the threshold is one ATR and this is the number it was compared against: {body}");
    for b in blocks {
        assert_carries_the_five(b, "order block");
        assert!(
            ["UNTESTED", "TESTED", "BROKEN"].contains(&b["state"].as_str().unwrap_or_default()),
            "an order block's state is one of three: {b}"
        );
    }

    // 4. Buy-side and sell-side liquidity with their swept state, and the
    //    prior day's and prior week's extremes among them.
    let liquidity = v["liquidity"].as_array().expect("a list");
    assert!(!liquidity.is_empty(), "ten days of swings produce pools");
    for p in liquidity {
        assert_carries_the_five(p, "liquidity");
        let side = p["side"].as_str().unwrap_or_default();
        assert!(["BUY_SIDE", "SELL_SIDE"].contains(&side), "{p}");
        let swept = p["swept"].as_bool().expect("swept");
        // The flag and the bar are one fact: neither is ever set alone, or a
        // reader has to decide which of the two to believe.
        assert_eq!(swept, !p["swept_at_bar_ms"].is_null(), "{p}");
        assert_eq!(p["state"], if swept { "SWEPT" } else { "RESTING" }, "{p}");
        assert!(p["swing_ids"].is_array(), "empty, never absent: {p}");
    }
    let kinds: Vec<&str> = liquidity.iter().filter_map(|p| p["kind"].as_str()).collect();
    for wanted in ["PRIOR_DAY_HIGH", "PRIOR_DAY_LOW", "PRIOR_WEEK_HIGH", "PRIOR_WEEK_LOW"] {
        assert!(kinds.contains(&wanted), "{wanted} is in the registration's liquidity list: {kinds:?}");
    }
    let pools = liquidity.iter().filter(|p| p["kind"] == "EQUAL_HIGHS" || p["kind"] == "EQUAL_LOWS");
    let ids: Vec<&str> =
        pools.flat_map(|p| p["swing_ids"].as_array().unwrap().iter().filter_map(|i| i.as_str())).collect();
    assert!(!ids.is_empty(), "swing pools must name their swings so /api/paper/htf can be joined");
    assert!(
        ids.iter().all(|id| id.starts_with("15m-hi-") || id.starts_with("15m-lo-")),
        "the shared id scheme is <timeframe>-<side>-<bar_ms>: {ids:?}"
    );

    // 5. Session, day and week extremes.
    let ex = &v["extremes"];
    for (period, high_kind, low_kind, state) in [
        ("session", "SESSION_HIGH", "SESSION_LOW", "FORMING"),
        ("day", "DAY_HIGH", "DAY_LOW", "COMPLETE"),
        ("week", "WEEK_HIGH", "WEEK_LOW", "COMPLETE"),
    ] {
        assert_eq!(ex[period]["high"]["kind"], high_kind, "{period}: {ex}");
        assert_eq!(ex[period]["low"]["kind"], low_kind, "{period}: {ex}");
        assert_eq!(ex[period]["high"]["state"], state, "{period}: {ex}");
        assert_carries_the_five(&ex[period]["high"], period);
        assert_carries_the_five(&ex[period]["low"], period);
        assert!(ex[period]["start_bar_ms"].as_i64().is_some());
        assert!(ex[period]["end_bar_ms"].as_i64().is_some());
    }
    // The week split is the weekend HOLE, so the last complete week is the
    // fixture's first five days and not a calendar Monday-to-Sunday.
    assert_eq!(ex["week"]["bars"], (5 * BARS_PER_DAY) as u64);
    assert_eq!(ex["day"]["bars"], BARS_PER_DAY as u64);
    assert_eq!(ex["session"]["bars"], BARS_PER_DAY as u64);

    // NOTHING that ranks, scores or votes. The registration forbids it in
    // advance (lines 68-72) and the cheapest way to keep that true through a
    // future edit is to fail here when a field with one of these names
    // appears.
    let text = serde_json::to_string(&v).expect("json");
    for banned in ["\"score\"", "\"composite\"", "\"confluence\"", "\"bias\"", "\"rank\"", "\"strength\""] {
        assert!(!text.contains(banned), "the route carries no verdict, and {banned} is one");
    }
}

#[tokio::test]
async fn the_analysis_window_holds_two_weeks_whatever_the_profile_window_is_asked_for() {
    let bars = fixture();
    let state = state_with_bars(Some(&bars));

    // The default: five trading days of profile inside ten of analysis.
    let five = read(&state, "xauusd", None, None).await.expect("levels");
    assert_eq!(five["window"]["profile_days"], 5);
    assert_eq!(five["window"]["days"], DAYS as u64);
    assert_eq!(five["window"]["bars"], (DAYS * BARS_PER_DAY) as u64);
    assert_eq!(five["profile"]["window_bars"], (5 * BARS_PER_DAY) as u64);

    // A caller asking for one day gets one day of PROFILE and the same two
    // weeks of everything else — the prior week has to stay in view or the
    // prior-week levels would vanish for a reason the response could not
    // state.
    let one = read(&state, "xauusd", None, Some(1)).await.expect("levels");
    assert_eq!(one["profile"]["window_bars"], BARS_PER_DAY as u64);
    assert_eq!(one["window"]["days"], DAYS as u64);
    let kinds: Vec<&str> =
        one["liquidity"].as_array().unwrap().iter().filter_map(|p| p["kind"].as_str()).collect();
    assert!(kinds.contains(&"PRIOR_WEEK_HIGH"), "{kinds:?}");

    // A narrower profile is a narrower profile: fewer bars, so a bucket that
    // was busy all fortnight can stop being the POC. Asserted as a different
    // window rather than a different answer, because which bucket wins is a
    // measurement and not something this test gets to decide.
    assert!(
        one["profile"]["window_bars"].as_u64() < five["profile"]["window_bars"].as_u64(),
        "the dial must actually turn"
    );
}

#[tokio::test]
async fn a_timeframe_nothing_stores_is_refused_rather_than_resampled() {
    let state = state_with_bars(Some(&fixture()));

    // 4h is not stored here. The route does NOT rebucket the 15m file into
    // it: `fd_store::resample` emits the trailing partial bucket as an
    // ordinary bar, and a still-forming candle would make every level on the
    // response repaint invisibly. Same refusal `htf::stored_only` makes.
    let v = read(&state, "xauusd", Some("4h"), None).await.expect("200");
    let why = v["unavailable"].as_str().expect("a sentence");
    assert!(why.contains("XAUUSD-4h.parquet"), "{why}");
    assert!(why.contains("H4"), "the export hint names H4 and not our own spelling: {why}");
    assert!(v["profile"].is_null());

    // A timeframe that is not a timeframe is a bad request, not an empty
    // answer: the caller has to change the call.
    let err = read(&state, "xauusd", Some("7s"), None).await.expect_err("400");
    assert_eq!(http_status(err), 400);

    // An unknown market is a 404 from the config, as everywhere else.
    let err = read(&state, "nosuchmarket", None, None).await.expect_err("404");
    assert_eq!(http_status(err), 404);
}
