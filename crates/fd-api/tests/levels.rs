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

/// The same walk at another timeframe, without the hand-written tail.
///
/// `bars_per_day` is a fact about the timeframe and is passed in rather than
/// divided out of `bar_ms`, because the broker's day is not a round number of
/// bars everywhere: 24 hours with a one-hour hole in it is 23 bars of 1h and
/// 6 bars of 4h, and dividing would say 24 and 6.
fn series(bar_ms: i64, bars_per_day: usize, days: usize) -> Vec<Bar> {
    let mut seed: u64 = 0xfeed_4321_8765_cbaf;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 40) as f64 / 16_777_216.0
    };
    let mut bars: Vec<Bar> = Vec::new();
    let mut prev = 4300.0;
    for d in 0..days {
        // Five trading days then a weekend, so `htf::weeks_of` splits this
        // series where it splits a real one.
        let day_index = (d / 5) * 7 + (d % 5);
        for i in 0..bars_per_day {
            let close = prev + (next() - 0.5) * 8.0;
            bars.push(Bar {
                time: START + day_index as i64 * DAY + i as i64 * bar_ms,
                open: prev,
                high: prev.max(close) + next() * 3.0,
                low: prev.min(close) - next() * 3.0,
                close,
                volume: None,
            });
            prev = close;
        }
    }

    // The same tail `fixture` writes by hand — two down candles, an impulse
    // of about 26, then a drift that never comes back — but as OFFSETS from
    // where the walk happens to be, because this walk is thirty days long on
    // the 1d call and would be nowhere near 4300 by the end. Planted for the
    // same reason it is planted there: a gap and a displacement that a
    // reader can see on the page, so the served sample of a non-default
    // timeframe shows an order block rather than an empty list.
    const TAIL: [(f64, f64, f64, f64); 8] = [
        (0.0, 2.0, -2.0, -1.0),
        (-1.0, 1.0, -3.0, -2.0),
        (-2.0, 3.0, -3.0, 2.0),
        (2.0, 30.0, 2.0, 28.0),
        (28.0, 35.0, 20.0, 32.0),
        (32.0, 38.0, 28.0, 36.0),
        (36.0, 40.0, 30.0, 38.0),
        (38.0, 42.0, 33.0, 40.0),
    ];
    let n = bars.len();
    if n > TAIL.len() {
        let base = bars[n - TAIL.len() - 1].close;
        for (k, (open, high, low, close)) in TAIL.into_iter().enumerate() {
            let b = &mut bars[n - TAIL.len() + k];
            b.open = base + open;
            b.high = base + high;
            b.low = base + low;
            b.close = base + close;
        }
    }
    bars
}

fn state_with_bars(bars: Option<&[Bar]>) -> Arc<AppState> {
    match bars {
        Some(bars) => state_with_files(&[("15m", bars)]),
        None => state_with_files(&[]),
    }
}

/// A data root holding one exported file per timeframe named.
///
/// More than one is possible because the accepted set of `?tf=` is read off
/// this directory rather than listed in the route: a test that asks for a
/// timeframe the store does not hold is only testing the refusal if there is
/// something else on disk that a resample could have been built from.
fn state_with_files(files: &[(&str, &[Bar])]) -> Arc<AppState> {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.keep();
    for (tf, bars) in files {
        write_bars(&root.join("bars").join(format!("XAUUSD-{tf}.parquet")), bars).expect("write");
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

    // BOTH reasons, named in the sentence and not only in a doc comment the
    // caller cannot see. Each one alone is refutable — the anchor argument
    // does not apply to H1, whose buckets really do line up with the epoch —
    // so a refusal carrying one of them invites "but this timeframe is
    // fine", which is true and is not permission.
    assert!(why.contains("21:00Z"), "the anchor, by its value: {why}");
    assert!(why.contains("epoch"), "and what it is not anchored to: {why}");
    assert!(why.contains("PARTIAL"), "and the trailing partial bucket: {why}");
    assert!(why.contains("15m"), "naming the file it declined to rebucket, which is on disk: {why}");
    // It still says which timeframe is missing and how to export it: the
    // sentence GREW, and the half a caller already reads must not have moved.
    assert!(why.contains("no 4h bars for xauusd"), "{why}");
    assert!(why.contains("mt5_export.py"), "{why}");

    // With two finer series on disk the sentence names the one a resample
    // would actually have read: `AppState::source_for` takes the COARSEST
    // that still fits, so an operator holding 5m and 15m must not be told
    // the route declined the 5m file.
    let five = series(300_000, 120, DAYS);
    let both = state_with_files(&[("5m", &five), ("15m", &fixture())]);
    let why = read(&both, "xauusd", Some("4h"), None).await.expect("200")["unavailable"]
        .as_str()
        .expect("a sentence")
        .to_string();
    assert!(why.contains("The 15m bars on disk"), "{why}");
    assert!(!why.contains("The 5m bars on disk"), "{why}");

    // A timeframe that is not a timeframe is a bad request, not an empty
    // answer: the caller has to change the call.
    let err = read(&state, "xauusd", Some("7s"), None).await.expect_err("400");
    assert_eq!(http_status(err), 400);

    // An unknown market is a 404 from the config, as everywhere else.
    let err = read(&state, "nosuchmarket", None, None).await.expect_err("404");
    assert_eq!(http_status(err), 404);
}

/* ------------------------------------------- what `?tf=` added, 2026-09-19 */

/// Every key path in a response, arrays folded onto their first element.
///
/// Paths and not values: the committed sample was served off the live store
/// and this test serves a fixture, so no number can agree. What must agree is
/// the SHAPE, which is the thing a client breaks on — the `exported_at_ms`
/// miss this whole directory exists for was a nesting error, not a wrong
/// number.
fn key_paths(v: &Value, at: &str, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                let path = format!("{at}.{k}");
                out.insert(path.clone());
                key_paths(val, &path, out);
            }
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                key_paths(first, &format!("{at}[]"), out);
            }
        }
        _ => {}
    }
}

fn paths_of(v: &Value) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    key_paths(v, "", &mut out);
    out
}

fn sample(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("docs").join("api-samples").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("the committed sample is json")
}

#[tokio::test]
async fn a_caller_that_passes_no_tf_gets_the_response_the_committed_sample_pins() {
    // `?tf=` must be invisible to everyone who does not pass it.
    // `py/live/smc_context.py` calls this route with no `tf` and its selftest
    // reads `docs/api-samples/paper-levels.json` as its fixture, so a field
    // added, renamed or moved on the default path changes a REGISTERED
    // campaign's prompt with nobody editing the book (`9a5dbfb`, again).
    let state = state_with_bars(Some(&fixture()));
    let served = read(&state, "xauusd", None, None).await.expect("levels");
    assert_eq!(paths_of(&served), paths_of(&sample("paper-levels.json")), "the default response changed shape");

    // And the absent case, which is a different set of paths on purpose:
    // every block null rather than missing.
    let nothing = state_with_bars(None);
    let served = read(&nothing, "xauusd", None, None).await.expect("levels");
    assert_eq!(paths_of(&served), paths_of(&sample("paper-levels-unavailable.json")));
    // Its sentence is the whole content of that response, so it is pinned by
    // value and not only by shape. It carries no resample clause: with no
    // file on disk at all there is nothing a resample could have read.
    assert_eq!(served["unavailable"], sample("paper-levels-unavailable.json")["unavailable"]);
}

#[tokio::test]
async fn a_stored_timeframe_answers_in_its_own_bars_and_its_own_days() {
    // 23 bars to a 1h trading day: 24 hours with the broker's one-hour hole
    // in it. Ten trading days of them is 230 bars against 400 of 15m — the
    // same span of clock, a different count of bars, which is the whole
    // point of the window being a span.
    const H1: i64 = 3_600_000;
    const BARS_PER_DAY_1H: usize = 23;
    let fifteen = fixture();
    let hour = series(H1, BARS_PER_DAY_1H, DAYS);
    let state = state_with_files(&[("15m", &fifteen), ("1h", &hour)]);

    let v = read(&state, "xauusd", Some("1h"), None).await.expect("levels");
    assert!(v["unavailable"].is_null(), "1h is on disk: {v}");
    assert_eq!(v["timeframe"], "1h");
    assert_eq!(v["bar_ms"], H1);
    assert_eq!(v["source"]["file"], "bars/XAUUSD-1h.parquet", "not the 15m file, and not a resample of it");
    assert_eq!(v["source"]["timeframe"], "1h");

    // TEN TRADING DAYS, not ten bars and not four hundred.
    assert_eq!(v["window"]["days"], DAYS as u64);
    assert_eq!(v["window"]["bars"], (DAYS * BARS_PER_DAY_1H) as u64);
    assert_eq!(v["extremes"]["day"]["bars"], BARS_PER_DAY_1H as u64, "one 1h trading day");
    assert_eq!(v["extremes"]["week"]["bars"], (5 * BARS_PER_DAY_1H) as u64, "one broker week of them");

    // Ages count THIS series' bars. Inside the session run there is no hole,
    // so the age in bars and the distance in clock are the same fact stated
    // twice and must agree through `bar_ms` — the arithmetic
    // `smc_context_selftest` records getting wrong in the other direction
    // ("a bar count divided out of a millisecond delta is out by half").
    let end = v["computed_at_bar_ms"].as_i64().expect("a newest bar");
    let high = &v["extremes"]["session"]["high"];
    let age = high["age_bars"].as_i64().expect("age_bars");
    assert!(age < BARS_PER_DAY_1H as i64, "a bar of the session in progress: {high}");
    assert_eq!(age * H1, end - high["formed_at_bar_ms"].as_i64().expect("formed"), "ages are in 1h bars: {high}");

    // The swing ids carry the timeframe they were measured on, which is what
    // lets `/api/paper/htf` be joined to this response rather than guessed at.
    let ids: Vec<&str> = v["liquidity"]
        .as_array()
        .expect("a list")
        .iter()
        .flat_map(|p| p["swing_ids"].as_array().unwrap().iter().filter_map(|i| i.as_str()))
        .collect();
    assert!(!ids.is_empty(), "ten days of 1h swings produce pools");
    assert!(ids.iter().all(|id| id.starts_with("1h-")), "1h swings, not 15m ones: {ids:?}");

    // The default is still the default with another file beside it.
    let default = read(&state, "xauusd", None, None).await.expect("levels");
    assert_eq!(default["timeframe"], "15m");
    assert_eq!(default["window"]["bars"], (DAYS * BARS_PER_DAY) as u64);
    assert_eq!(default["window"]["days"], DAYS as u64, "the same ten days, four hundred bars of them");
}

#[tokio::test]
async fn ten_trading_days_of_daily_bars_is_ten_bars_and_the_response_says_which() {
    // The reader this test protects is the one who cannot tell "10 days of
    // 1d" from "10 bars of 1d". They are the same slice, and the response
    // still distinguishes them: `days` is measured in the stamps, `bars` is
    // counted, and both are published.
    let daily = series(DAY, 1, 30);
    let state = state_with_files(&[("1d", &daily)]);
    let v = read(&state, "xauusd", Some("1d"), None).await.expect("levels");
    assert_eq!(v["timeframe"], "1d");
    assert_eq!(v["bar_ms"], DAY);
    assert_eq!(v["window"]["days"], 10, "ten trading days, as on every other timeframe");
    assert_eq!(v["window"]["bars"], 10, "which on 1d is ten bars");
    assert_eq!(v["extremes"]["day"]["bars"], 1, "one bar IS one trading day here");

    // And the price of the clock decision, pinned rather than discovered: ten
    // 1d bars are fewer than ATR(14) needs, so the denominator is null and
    // every ATR-derived block is empty. NULL and [] mean different things
    // here and both are correct — there is no ATR to measure against, and the
    // lists that do not need one were measured and found nothing. If a 1d
    // export ever lands on this desk (none exists on 2026-09-19), the thing
    // to revisit is ANALYSIS_DAYS, not the window being a span of clock.
    assert!(v["atr14"].is_null(), "ATR(14) over ten bars is not a number: {v}");
    assert!(v["profile"].is_null(), "and the bucket height derives from it");
    assert_eq!(v["order_blocks"].as_array().expect("a list").len(), 0);
}

#[tokio::test]
async fn an_unknown_timeframe_is_a_400_that_lists_what_this_market_has() {
    // Two files on disk, so the list is a list and its order can be checked.
    let state = state_with_files(&[("15m", &fixture()), ("1h", &series(3_600_000, 23, DAYS))]);

    for asked in ["7s", "", "4hh", "M15"] {
        let err = read(&state, "xauusd", Some(asked), None).await.expect_err("400");
        let text = err.to_string();
        assert_eq!(http_status(err), 400, "{asked}");
        assert!(text.contains("15m, 1h"), "finest first, so a reader can scan it: {text}");
        assert!(text.contains("never"), "and why the list is what it is: {text}");
    }

    // A market whose bars have never been exported says that rather than
    // listing nothing and leaving a reader to wonder what an empty list is.
    let empty = state_with_files(&[]);
    let err = read(&empty, "xauusd", Some("7s"), None).await.expect_err("400");
    assert!(err.to_string().contains("no exported bars at all"), "{err}");

    // `.bak` files are not timeframes. The live store keeps
    // `XAUUSD-15m.parquet.bak` beside the live file, and a looser suffix test
    // would advertise a timeframe called `15m.parquet`.
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.keep();
    write_bars(&root.join("bars").join("XAUUSD-15m.parquet"), &fixture()).expect("write");
    std::fs::copy(root.join("bars").join("XAUUSD-15m.parquet"), root.join("bars").join("XAUUSD-15m.parquet.bak"))
        .expect("the backup the live store keeps");
    let state = Arc::new(AppState::new(config(), root));
    let err = read(&state, "xauusd", Some("7s"), None).await.expect_err("400");
    // The whole sentence, by value: one timeframe listed once, and the text
    // a client may be displaying pinned so a rewrite of it is a diff here.
    assert_eq!(
        err.to_string(),
        "unknown timeframe \"7s\": xauusd has 15m. This route serves stored bars only and never \
         resamples, so it can answer for a timeframe on disk and no other."
    );
}

#[tokio::test]
async fn the_route_carries_no_verdict_on_any_timeframe() {
    // The guard the registration pre-commits to, re-run per timeframe:
    // `?tf=` multiplied the number of responses this route can emit, and a
    // banned field would only have to appear on one of them.
    let hour = series(3_600_000, 23, DAYS);
    let daily = series(DAY, 1, 30);
    let state = state_with_files(&[("15m", &fixture()), ("1h", &hour), ("1d", &daily)]);
    for tf in [None, Some("15m"), Some("1h"), Some("1d"), Some("4h")] {
        let v = read(&state, "xauusd", tf, None).await.expect("200");
        let text = serde_json::to_string(&v).expect("json");
        for banned in ["\"score\"", "\"composite\"", "\"confluence\"", "\"bias\"", "\"rank\"", "\"strength\""] {
            assert!(!text.contains(banned), "{tf:?} carries {banned}");
        }
    }
}

/// Refreshes the served samples in `docs/api-samples/`.
///
/// `cargo test -p fd-api --test levels -- --ignored --nocapture`, and ignored
/// by default because a test that writes into the repository on every run
/// makes `git status` a liar. The bars behind these two are the fixture
/// above and NOT an export: this desk has never exported a 1h or 4h XAUUSD
/// series (checked 2026-09-19 — 1m, 5m and 15m and nothing else), so a
/// sample of a non-default timeframe off the live store is not a thing that
/// can exist today. What they pin is what the ROUTE emits: the timeframe and
/// `bar_ms` it carries, ages counted in its own bars, ten trading days of
/// them, and the refusal's wording.
#[tokio::test]
#[ignore = "writes docs/api-samples; run with --ignored to refresh"]
async fn write_the_served_samples() {
    let state = state_with_files(&[("15m", &fixture()), ("1h", &series(3_600_000, 23, DAYS))]);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("docs").join("api-samples");
    for (name, tf) in [("paper-levels-1h.json", "1h"), ("paper-levels-4h-refused.json", "4h")] {
        let v = read(&state, "xauusd", Some(tf), None).await.expect("200");
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string_pretty(&v).expect("json")).expect("write the sample");
        println!("wrote {}", path.display());
    }
}
