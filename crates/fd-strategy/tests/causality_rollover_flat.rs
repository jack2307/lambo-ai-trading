//! `tsmom-flat` and `quiet-swing-flat` read no bar after the one they decide
//! on — and the positions they open cross no 17:00 New York rollover.
//!
//! The first property, stated the way `fd-indicators` states it for every
//! indicator: computing over a **truncated** series must equal the prefix of
//! computing over the full series. For a strategy the observable is the intent,
//! so the sequence of intents produced while walking `bars[..=i]` must equal,
//! bar for bar, the sequence produced while walking the whole series.
//!
//! It matters here for the same reason it matters for the parent
//! (`causality_quiet_swing.rs`): `quiet-swing-flat` resamples 15-minute bars
//! into 17:00-New-York sessions, and resampling is exactly where a higher
//! timeframe leaks. It matters additionally because this variant reaches
//! `holdSessions` further back than the parent does.
//!
//! The second property is the whole mechanism. A flat-across-the-rollover
//! variant that quietly carried a position over 17:00 New York would be
//! measuring its parent while wearing a different name, and
//! `fd_core::clock::swap_nights` is the same function the engine charges
//! financing with — so the assertion is made against the charger, not against
//! a restatement of it.

use fd_core::clock::{days_from_civil, swap_nights};
use fd_core::types::Bar;
use fd_indicators::IndicatorSet;
use fd_strategy::registry::{BarContext, Intent, OpenPosition, Params, Side, Strategy};
use fd_strategy::rollover_flat::{QuietSwingFlat, TsmomFlat};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const BAR_MS: i64 = 900_000;
/// A whole 24-hour session of 15-minute bars. Anything shorter has no 16:00
/// New York bar to exit on, and the fixture would be testing a feed gap.
const PER: usize = 96;

/// A deterministic but uneven 15-minute series: `sessions` complete sessions of
/// 96 bars, starting at 17:00 New York, with a per-session step and range driven
/// by a small integer hash so that the quiet condition switches on and off, the
/// trailing return changes sign, and both sides get taken.
fn series(sessions: usize) -> Vec<Bar> {
    // 21:00Z on a March-18 already on New York daylight time (DST began on the
    // 10th) is exactly 17:00 in New York, so session `d` starts at bar
    // `d * PER`. The fixture stays inside EDT.
    let start = days_from_civil(2024, 3, 18) * DAY_MS + 21 * HOUR_MS;
    let mut bars = Vec::with_capacity(sessions * PER);
    let mut price = 2000.0;
    for d in 0..sessions {
        // Deterministic pseudo-noise; no rand dependency in this crate.
        let h = ((d as u64).wrapping_mul(2_654_435_761) >> 7) % 1000;
        let step = (h as f64 - 500.0) / 50.0; // -10 .. +10 dollars
        let range = 2.0 + (h % 37) as f64; // 2 .. 38 dollars
        for b in 0..PER {
            let t = start + (d as i64) * DAY_MS + (b as i64) * BAR_MS;
            let frac = (b + 1) as f64 / PER as f64;
            let close = price + step * frac;
            bars.push(Bar {
                time: t,
                open: price + step * (b as f64 / PER as f64),
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

fn qs_params() -> Params {
    let mut p = QuietSwingFlat.default_params();
    p.set("lookbackSessions", 5.0);
    p.set("windowSessions", 20.0);
    p.set("rangeDays", 10.0);
    p.set("holdSessions", 3.0);
    p
}

fn ts_params() -> Params {
    let mut p = TsmomFlat.default_params();
    p.set("lookbackDays", 10.0);
    p.set("rangeDays", 10.0);
    p
}

/// Every intent the strategy issues over `bars`, with a position tracked the
/// way the engine tracks one: an `Enter` opens at the next bar's open, an
/// `Exit` closes at the next bar's open. Also returns the realised
/// entry/exit instants, which is what the rollover assertion reads.
fn walk(strategy: &dyn Strategy, bars: &[Bar], p: &Params) -> (Vec<(i64, Intent)>, Vec<(i64, i64)>) {
    let ind = IndicatorSet::new();
    let mut out = Vec::new();
    let mut holds = Vec::new();
    let mut position: Option<OpenPosition> = None;
    let mut pending: Option<Intent> = None;
    for (i, bar) in bars.iter().enumerate() {
        match pending.take() {
            Some(Intent::Enter { side, stop, target, .. }) if position.is_none() => {
                position = Some(OpenPosition { side, entry_price: bar.open, entry_time: bar.time, stop, target });
            }
            Some(Intent::Exit { .. }) => {
                if let Some(open) = position.take() {
                    holds.push((open.entry_time, bar.time));
                }
            }
            _ => {}
        }
        let ctx = BarContext { bar, i, bars, ind: &ind, series: &[], options: None, position, params: p };
        let intent = strategy.on_bar(&ctx);
        if intent != Intent::None {
            out.push((bar.time, intent.clone()));
            pending = Some(intent);
        }
    }
    (out, holds)
}

fn both() -> Vec<(&'static dyn Strategy, Params)> {
    vec![(&TsmomFlat as &dyn Strategy, ts_params()), (&QuietSwingFlat as &dyn Strategy, qs_params())]
}

#[test]
fn a_truncated_walk_is_a_prefix_of_the_full_walk() {
    let full = series(80);
    for (strategy, p) in both() {
        let (whole, _) = walk(strategy, &full, &p);
        assert!(whole.len() >= 20, "{} must actually trade: {} intents", strategy.id(), whole.len());
        assert!(
            whole.iter().any(|(_, i)| matches!(i, Intent::Enter { side: Side::Long, .. }))
                && whole.iter().any(|(_, i)| matches!(i, Intent::Enter { side: Side::Short, .. })),
            "{} must exercise both sides",
            strategy.id()
        );
        // Truncate at every session boundary from the 40th session on.
        for cut_session in 40..80 {
            let cut = cut_session * PER;
            let (truncated, _) = walk(strategy, &full[..cut], &p);
            let expected: Vec<(i64, Intent)> =
                whole.iter().filter(|(t, _)| *t < full[cut].time).cloned().collect();
            assert_eq!(
                truncated,
                expected,
                "{}: truncating at session {cut_session} changed the decisions before it",
                strategy.id()
            );
        }
    }
}

#[test]
fn no_position_either_strategy_opens_is_ever_charged_a_rollover() {
    let full = series(80);
    for (strategy, p) in both() {
        let (_, holds) = walk(strategy, &full, &p);
        assert!(holds.len() >= 20, "{} must close positions to be worth checking: {}", strategy.id(), holds.len());
        for (entry, exit) in &holds {
            assert_eq!(
                swap_nights(*entry, *exit),
                0,
                "{}: a position from {entry} to {exit} crossed 17:00 New York — the one thing this \
                 mechanism must never do, measured with the same function the engine charges with",
                strategy.id()
            );
        }
        // And every hold is a real one, not a one-bar artefact: the default
        // 18:00 -> 16:00 window is 22 hours.
        let span = |(a, b): &(i64, i64)| (b - a) as f64 / HOUR_MS as f64;
        let longest = holds.iter().map(span).fold(0.0f64, f64::max);
        assert!((longest - 22.0).abs() < 0.3, "{}: longest hold {longest} h, expected ~22", strategy.id());
    }
}

#[test]
fn the_session_in_progress_is_invisible_however_extreme_it_is() {
    let base = series(80);
    let ind = IndicatorSet::new();
    for (strategy, p) in both() {
        let at = |bars: &[Bar], i: usize| {
            let ctx =
                BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position: None, params: &p };
            strategy.on_bar(&ctx)
        };
        // The decision bar: 18:00 New York, four bars into the first session
        // from the 50th on where this strategy actually issues an entry.
        // Hard-coding a session would silently test `None == None`.
        let i = (50..79)
            .map(|s| s * PER + 4)
            .find(|i| matches!(at(&base, *i), Intent::Enter { .. }))
            .unwrap_or_else(|| panic!("{} never entered in the fixture", strategy.id()));
        let decide = |bars: &[Bar]| at(bars, i);
        let before = decide(&base);
        assert!(matches!(before, Intent::Enter { .. }), "{}: got {before:?}", strategy.id());

        // Case 1: rewrite every LATER bar into a violent series.
        let mut future = base.clone();
        for b in future.iter_mut().skip(i + 1) {
            b.high += 500.0;
            b.low -= 500.0;
            b.close += 250.0;
        }
        assert_eq!(decide(&future), before, "{}: a bar after the decision bar was read", strategy.id());

        // Case 2: rewrite the REST OF THE DECISION BAR'S OWN SESSION. Those
        // bars are after `i` and belong to a session that has not closed.
        let mut same_session = base.clone();
        for b in same_session.iter_mut().skip(i + 1).take(PER - 5) {
            b.high += 900.0;
            b.low -= 900.0;
        }
        assert_eq!(decide(&same_session), before, "{}: the forming session leaked in", strategy.id());

        // Case 3: truncating the series to end AT the decision bar must give
        // the same answer as having the whole future available.
        assert_eq!(decide(&base[..=i]), before, "{}: the answer needed more data", strategy.id());
    }
}
