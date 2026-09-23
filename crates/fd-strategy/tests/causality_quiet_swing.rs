//! `quiet-swing` reads no bar after the one it is deciding on.
//!
//! The property, stated the way `fd-indicators` states it for every indicator:
//! computing over a **truncated** series must equal the prefix of computing
//! over the full series. For a strategy the observable is the intent, so the
//! sequence of intents produced while walking `bars[..=i]` must equal, bar for
//! bar, the sequence produced while walking the whole series — and the two
//! walks must agree even when the tail of the full series is wildly different
//! from anything the truncated walk could have guessed.
//!
//! This matters here specifically because the method resamples: it builds
//! 17:00-New-York sessions out of 15-minute bars, and resampling is exactly
//! where a higher timeframe leaks. A session bar must not be visible before
//! its session has closed. The third case below is the one that catches it:
//! the bars of the session in progress are replaced with an enormous range, so
//! a method that let the forming session into its range sum or its close would
//! decide differently on the same bar.

use fd_core::clock::days_from_civil;
use fd_core::types::Bar;
use fd_indicators::IndicatorSet;
use fd_strategy::quiet_swing::{QuietTapeSwing, session_index};
use fd_strategy::registry::{BarContext, Intent, OpenPosition, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const BAR_MS: i64 = 900_000;

/// A deterministic but uneven 15-minute series: `sessions` sessions of `per`
/// bars each, starting at 17:00 New York, with a per-session step and range
/// driven by a small integer hash so that the quiet condition switches on and
/// off and both sides get taken.
fn series(sessions: usize, per: usize) -> Vec<Bar> {
    // 21:00Z on a March-18 that is already on New York daylight time (DST
    // began on the 10th) is exactly 17:00 in New York, so session `d` starts
    // at bar `d * per`. All 90 sessions stay inside EDT.
    let start = days_from_civil(2024, 3, 18) * DAY_MS + 21 * HOUR_MS;
    let mut bars = Vec::with_capacity(sessions * per);
    let mut price = 2000.0;
    for d in 0..sessions {
        // Deterministic pseudo-noise; no rand dependency in this crate.
        let h = ((d as u64).wrapping_mul(2_654_435_761) >> 7) % 1000;
        let step = (h as f64 - 500.0) / 50.0; // -10 .. +10 dollars
        let range = 2.0 + (h % 37) as f64; // 2 .. 38 dollars
        for b in 0..per {
            let t = start + (d as i64) * DAY_MS + (b as i64) * BAR_MS;
            let frac = (b + 1) as f64 / per as f64;
            let close = price + step * frac;
            bars.push(Bar {
                time: t,
                open: price + step * (b as f64 / per as f64),
                high: close.max(price) + range / 2.0,
                low: close.min(price) - range / 2.0,
                close,
                volume: Some(1.0),
            });
        }
        price += step;
    }
    bars
}

fn params() -> Params {
    let mut p = QuietTapeSwing.default_params();
    p.set("lookbackSessions", 5.0);
    p.set("windowSessions", 20.0);
    p.set("rangeDays", 10.0);
    p.set("holdSessions", 3.0);
    p
}

/// Every intent the method issues over `bars`, with a position tracked the way
/// the engine tracks one: an `Enter` opens at the next bar's open, an `Exit`
/// closes at the next bar's open.
fn walk(bars: &[Bar], p: &Params) -> Vec<(i64, Intent)> {
    let ind = IndicatorSet::new();
    let mut out = Vec::new();
    let mut position: Option<OpenPosition> = None;
    let mut pending: Option<Intent> = None;
    for (i, bar) in bars.iter().enumerate() {
        match pending.take() {
            Some(Intent::Enter { side, stop, target, .. }) if position.is_none() => {
                position = Some(OpenPosition { side, entry_price: bar.open, entry_time: bar.time, stop, target });
            }
            Some(Intent::Exit { .. }) => position = None,
            _ => {}
        }
        let ctx = BarContext { bar, i, bars, ind: &ind, series: &[], options: None, position, params: p };
        let intent = QuietTapeSwing.on_bar(&ctx);
        if intent != Intent::None {
            out.push((bar.time, intent.clone()));
            pending = Some(intent);
        }
    }
    out
}

#[test]
fn a_truncated_walk_is_a_prefix_of_the_full_walk() {
    let p = params();
    let full = series(90, 8);
    let whole = walk(&full, &p);
    assert!(whole.len() >= 10, "the fixture must actually trade: {} intents", whole.len());
    assert!(
        whole.iter().any(|(_, i)| matches!(i, Intent::Enter { side: Side::Long, .. }))
            && whole.iter().any(|(_, i)| matches!(i, Intent::Enter { side: Side::Short, .. })),
        "the fixture must exercise both sides"
    );

    // Truncate at every session boundary from the 30th session on.
    for cut_session in 30..90 {
        let cut = cut_session * 8;
        let truncated = walk(&full[..cut], &p);
        let expected: Vec<(i64, Intent)> = whole.iter().filter(|(t, _)| *t < full[cut].time).cloned().collect();
        assert_eq!(
            truncated, expected,
            "truncating at session {cut_session} changed the decisions before it"
        );
    }
}

#[test]
fn the_session_in_progress_is_invisible_however_extreme_it_is() {
    let p = params();
    let base = series(90, 8);
    // The decision bar: the first bar of session 60.
    let i = 60 * 8;
    assert!(
        session_index(base[i].time) != session_index(base[i - 1].time),
        "bar {i} must open a session"
    );

    let ind = IndicatorSet::new();
    let decide = |bars: &[Bar]| {
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position: None, params: &p };
        QuietTapeSwing.on_bar(&ctx)
    };
    let before = decide(&base);

    // Case 1: rewrite every LATER bar into a violent series. Nothing after `i`
    // may be read, so the decision must not move.
    let mut future = base.clone();
    for b in future.iter_mut().skip(i + 1) {
        b.high += 500.0;
        b.low -= 500.0;
        b.close += 250.0;
    }
    assert_eq!(decide(&future), before, "a bar after the decision bar was read");

    // Case 2: rewrite the REST OF THE DECISION BAR'S OWN SESSION. Those bars
    // are after `i` too, and they belong to the session that has not closed.
    let mut same_session = base.clone();
    for b in same_session.iter_mut().skip(i + 1).take(7) {
        b.high += 900.0;
        b.low -= 900.0;
    }
    assert_eq!(decide(&same_session), before, "the forming session leaked into the range sum");

    // Case 3: the decision bar itself. Its own high and low are part of the
    // forming session and must not enter the range sum either; only its close
    // may be used, and only to place the sizing stop. So a change to its high
    // and low alone must leave the decision identical.
    let mut own_bar = base.clone();
    own_bar[i].high += 900.0;
    own_bar[i].low -= 900.0;
    assert_eq!(decide(&own_bar), before, "the decision bar's own range entered the statistic");

    // And truncating the series to end AT the decision bar must give the same
    // answer as having the whole future available.
    assert_eq!(decide(&base[..=i]), before, "the answer depended on there being more data");
}
