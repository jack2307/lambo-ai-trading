//! Long-horizon signals, expressed only INSIDE a session and flat across every
//! 17:00 New York rollover.
//!
//! Registration: `docs/decisions/2026-10-07-rollover-flat.md`.
//!
//! # Why this module exists
//!
//! `agent/pure-drift` measured that financing on this desk's instrument is
//! charged **per 17:00 New York crossing, not per hour held**: `swap$/spread$`
//! spans 0.014x to 11,139x across eight rows that are all gold exposure on one
//! instrument, and a book flat 16:00-18:00 New York holds 65.7% of the calendar
//! clock while paying 3-5% of a continuous book's financing. The consequence,
//! with `expectancy_net`:
//!
//! ```text
//! ts-l60   expectancy +0.370R  ->  expectancy_net -0.271R
//! qs-h15   expectancy +0.130R  ->  expectancy_net -0.179R
//! px-1s    expectancy +0.012R  ->  expectancy_net +0.011R
//! ```
//!
//! Carry is avoidable; the mechanisms, held continuously, are not. So the
//! question these two strategies exist to answer is whether anything of the
//! long-horizon *signal* survives when the *expression* pays a round trip every
//! session instead of financing every night.
//!
//! # What is the same as the parents, and what is not
//!
//! Same: the signal, read at the same New York minute from the same completed
//! history; the sizing unit (`riskDailyRanges` x the mean New York-day range of
//! the last `rangeDays` days, via [`crate::tsmom::sizing_stop`], sizing only and
//! never enforced); `Exits::Strategy` with an **empty grid**, so
//! `hypotheses::control_for` builds the `RandomHold` drift null the parents were
//! measured against.
//!
//! Not the same: the position. It opens at the first bar at or after `from` New
//! York and closes at the first bar outside `[from, to)`. There is no mid-window
//! flip — a flip inside the window would be a second mechanism.
//!
//! # The one invariant this module must not get wrong
//!
//! If the window contained 17:00 New York the whole point would be lost in
//! silence, so it is checked rather than trusted: [`crosses_rollover`] refuses
//! the window and [`window_minutes`]'s callers return [`Intent::None`]. The
//! default 18:00 -> 16:00 is the window `agent/pure-drift` already measured at
//! 0.116x-0.188x of a round trip in financing, all of it holidays where the feed
//! prints no bar in the flat two hours.
//!
//! # Causality
//!
//! Both strategies read `bars[..=i]` only, through
//! [`crate::quiet_swing::completed_sessions`] (which discards every bar of the
//! session in progress, and the oldest session it reaches) or a backward scan.
//! `tests/causality_rollover_flat.rs` asserts the intent sequence over a
//! truncated series is a prefix of the sequence over the full one.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::quiet_swing::{SessionBar, completed_sessions};
use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;
/// 17:00 New York as a minute of the local day — the rollover boundary
/// `fd_core::clock::swap_nights` charges on.
const ROLLOVER_MINUTE: u32 = 17 * 60;

fn ny_day_minute(utc_ms: i64) -> (i64, u32) {
    let local = utc_ms + new_york_offset_ms(utc_ms);
    (local.div_euclid(DAY_MS), (local.rem_euclid(DAY_MS) / 60_000) as u32)
}

fn hhmm(v: f64) -> Option<u32> {
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    let v = v.round() as u32;
    let (h, m) = (v / 100, v % 100);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Is `minute` of the New York day inside the half-open window `[from, to)`?
/// A window with `from > to` wraps midnight, which 18:00 -> 16:00 does.
#[must_use]
pub fn in_window(minute: u32, from: u32, to: u32) -> bool {
    if from <= to { minute >= from && minute < to } else { minute >= from || minute < to }
}

/// Does a position held over `[from, to)` cross 17:00 New York?
///
/// The whole mechanism is "flat across the rollover", so a window that contains
/// the rollover is not a variant of it — it is the thing it was built to avoid,
/// and it would fail in silence. Both strategies refuse to trade such a window.
#[must_use]
pub fn crosses_rollover(from: u32, to: u32) -> bool {
    from == to || in_window(ROLLOVER_MINUTE, from, to)
}

/// `(from, to)` in minutes of the New York day, or `None` when the window is
/// unreadable or crosses the rollover.
fn window_minutes(p: &Params) -> Option<(u32, u32)> {
    let (from, to) = (hhmm(p.get("from"))?, hhmm(p.get("to"))?);
    (!crosses_rollover(from, to)).then_some((from, to))
}

/// The position half of both strategies: open at the window's first bar on the
/// side `signal` returns, close at the first bar outside the window, never
/// cross 17:00 New York.
///
/// `signal` is only called on a bar that would open a position, so a strategy
/// pays for its history scan once a session rather than once a bar.
fn drive(ctx: &BarContext, signal: impl FnOnce() -> Option<Side>) -> Intent {
    let p = ctx.params;
    let Some((from, to)) = window_minutes(p) else { return Intent::None };
    let (_, minute) = ny_day_minute(ctx.bar.time);
    let inside = in_window(minute, from, to);
    if ctx.position.is_some() {
        return if inside { Intent::None } else { Intent::Exit { reason: "window closed".into() } };
    }
    if !inside {
        return Intent::None;
    }
    // Only at the window's first bar: a position opened late in the window is a
    // different, shorter hold, and re-entering after a guard flattened the book
    // mid-window would be a different mechanism again.
    let previous_inside = ctx.prev().is_some_and(|b| in_window(ny_day_minute(b.time).1, from, to));
    if previous_inside {
        return Intent::None;
    }
    let Some(side) = signal() else { return Intent::None };
    let bars = &ctx.bars[..=ctx.i];
    let Some(stop) =
        crate::tsmom::sizing_stop(bars, ctx.bar, side, p.get("riskDailyRanges"), p.period("rangeDays"))
    else {
        return Intent::None;
    };
    Intent::Enter {
        side,
        stop: Some(stop),
        target: None,
        reason: format!(
            "{} {:04}-{:04} New York, flat across the rollover",
            if side.is_long() { "long" } else { "short" },
            p.get("from") as u32,
            p.get("to") as u32
        ),
    }
}

/* ------------------------------------------------------------------ tsmom */

/// Sign of the trailing `lookback`-New-York-day return at `bar`, read exactly
/// as [`crate::tsmom`] reads it: the close of the last bar on or before the
/// same minute `lookback` New York days earlier. `None` until that bar exists.
#[must_use]
pub fn trailing_day_return(bars: &[Bar], bar: &Bar, lookback: i64) -> Option<f64> {
    if lookback <= 0 {
        return None;
    }
    let (day, _) = ny_day_minute(bar.time);
    let target_day = day - lookback;
    let past = bars.iter().rev().find(|b| ny_day_minute(b.time).0 <= target_day)?;
    let ret = bar.close / past.close - 1.0;
    ret.is_finite().then_some(ret)
}

pub struct TsmomFlat;

impl Strategy for TsmomFlat {
    fn id(&self) -> &'static str {
        "tsmom-flat"
    }
    fn name(&self) -> &'static str {
        "Time-series momentum, flat across the rollover"
    }
    fn description(&self) -> &'static str {
        "Hold the side of the trailing N-New-York-day return, but only inside the from-to New York \
         window, so the book is flat across every 17:00 New York rollover and pays a round trip \
         each session instead of financing every night."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("lookbackDays", 60.0),
            ("from", 1800.0),
            ("to", 1600.0),
            ("atrPeriod", 14.0),
            // tsmom's own default, so the flat variant is measured in the same
            // R unit as the baseline it is a variant of.
            ("riskDailyRanges", 2.0),
            ("rangeDays", 20.0),
        ])
    }
    /// **Empty on purpose**, like `quiet-swing`'s and `session-hold`'s: it is
    /// what makes `hypotheses::control_for` build the `RandomHold` drift null
    /// rather than the stop-and-target one, and a drift null is the only
    /// control a self-managed hold can be measured against.
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        // Declared so `sizing_atr_key` resolves to a real series; every entry
        // carries an explicit sizing stop, so this ATR never sets risk.
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // Deliberately `tsmom`'s own formula, not a tighter one: the parent is
        // the row this variant is compared against, and a variant that starts
        // trading months earlier than its baseline is a different sample as
        // well as a different mechanism.
        (p.period("lookbackDays") + 5) * 288
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let lookback = ctx.params.period("lookbackDays") as i64;
        drive(ctx, || {
            let ret = trailing_day_return(&ctx.bars[..=ctx.i], ctx.bar, lookback)?;
            if ret > 0.0 {
                Some(Side::Long)
            } else if ret < 0.0 {
                Some(Side::Short)
            } else {
                None
            }
        })
    }
}

/* ------------------------------------------------------------- quiet swing */

/// True range of `sessions[j]`, which needs `sessions[j + 1]`'s close. Same
/// definition as `quiet_swing`'s, which is private to that module.
fn true_range(sessions: &[SessionBar], j: usize) -> Option<f64> {
    let (s, prev) = (sessions.get(j)?, sessions.get(j + 1)?);
    let r = s.high.max(prev.close) - s.low.min(prev.close);
    r.is_finite().then_some(r)
}

/// `quiet-swing`'s signal state for the session that `sessions[0]` is the last
/// completed session of, reconstructed **statelessly** from completed sessions
/// alone.
///
/// ON with side `s` iff there is an `m` in `0..hold` such that
///
/// * the trailing `k`-session true-range sum, *as it stood `m` sessions ago*,
///   sat strictly below the `quiet_pct` quantile of the previous `w` values of
///   that same statistic, and
/// * the sign of the trailing `k`-session return has been `s` at **every**
///   session from `m` ago until now.
///
/// This is the declared approximation of registration section 4, not a port:
/// the parent restarts its hold clock only on a fresh entry after an exit,
/// while this restarts it whenever the quiet condition fires again inside a
/// run, so a run here can be longer than the parent would have held.
#[must_use]
pub fn quiet_swing_state(sessions: &[SessionBar], k: usize, w: usize, hold: usize, quiet_pct: f64) -> Option<Side> {
    if k == 0 || w == 0 || hold == 0 || !quiet_pct.is_finite() {
        return None;
    }
    // Sign of the trailing k-session return as it stood `m` sessions ago.
    let ret_sign = |m: usize| -> Option<Side> {
        let (last, past) = (sessions.get(m)?.close, sessions.get(m + k)?.close);
        if !(last.is_finite() && past.is_finite() && past != 0.0) {
            return None;
        }
        let ret = last / past - 1.0;
        if ret > 0.0 {
            Some(Side::Long)
        } else if ret < 0.0 {
            Some(Side::Short)
        } else {
            None
        }
    };
    let side = ret_sign(0)?;
    // How far back the sign has been unbroken, capped at the hold clock.
    let mut run = 1usize;
    while run < hold && ret_sign(run) == Some(side) {
        run += 1;
    }
    // Prefix sums of the session true ranges, so each of the `run` quiet tests
    // is O(1) instead of O(k * w).
    let need_tr = run + w + k;
    let mut prefix: Vec<f64> = Vec::with_capacity(need_tr + 1);
    prefix.push(0.0);
    for j in 0..need_tr {
        let tr = true_range(sessions, j)?;
        prefix.push(prefix[j] + tr);
    }
    // Trailing k-session true-range sum ending `j` sessions ago.
    let sum = |j: usize| prefix[j + k] - prefix[j];
    for m in 0..run {
        let current = sum(m);
        if !(current.is_finite() && current > 0.0) {
            return None;
        }
        let below = (m + 1..=m + w).filter(|j| sum(*j) < current).count();
        if (below as f64 / w as f64) < quiet_pct {
            return Some(side);
        }
    }
    None
}

pub struct QuietSwingFlat;

impl Strategy for QuietSwingFlat {
    fn id(&self) -> &'static str {
        "quiet-swing-flat"
    }
    fn name(&self) -> &'static str {
        "Quiet-tape continuation, flat across the rollover"
    }
    fn description(&self) -> &'static str {
        "Hold the side of the trailing K-session return while the trailing K-session true-range sum \
         sits in the low part of its own trailing window — but only inside the from-to New York \
         window, so the book is flat across every 17:00 New York rollover."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("lookbackSessions", 10.0),
            ("windowSessions", 60.0),
            ("quietPct", 0.50),
            ("holdSessions", 5.0),
            ("from", 1800.0),
            ("to", 1600.0),
            // quiet-swing's own default, for the same reason tsmom-flat keeps
            // 2.0: same R unit as the baseline it is a variant of.
            ("riskDailyRanges", 1.5),
            ("rangeDays", 20.0),
            ("atrPeriod", 14.0),
        ])
    }
    /// Empty, for the reason given on [`TsmomFlat::grid`].
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // `quiet-swing`'s formula plus the hold clock this variant has to look
        // back over. Tight on purpose, exactly as the parent argues: the real
        // gate is `completed_sessions` returning `None` until the sessions
        // exist, which is exact on any bar width.
        p.period("lookbackSessions")
            + p.period("windowSessions")
            + p.period("holdSessions")
            + p.period("rangeDays")
            + 4
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let p = ctx.params;
        let (k, w, hold) = (p.period("lookbackSessions"), p.period("windowSessions"), p.period("holdSessions"));
        let quiet_pct = p.get("quietPct");
        drive(ctx, || {
            // Enough for the oldest quiet test the hold clock can reach: the
            // test `hold - 1` sessions ago needs `w` earlier sums, each over
            // `k` true ranges, and a true range needs one session older still.
            let want = hold + w + k + 1;
            let sessions = completed_sessions(&ctx.bars[..=ctx.i], ctx.i, want)?;
            quiet_swing_state(&sessions, k, w, hold, quiet_pct)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use crate::quiet_swing::session_index;
    use fd_core::clock::days_from_civil;

    const HOUR_MS: i64 = 3_600_000;

    #[test]
    fn the_default_window_does_not_cross_the_rollover_and_a_window_that_does_is_refused() {
        // 18:00 -> 16:00, the window agent/pure-drift measured at 0.116x-0.188x
        // of one round trip in financing.
        assert!(!crosses_rollover(18 * 60, 16 * 60));
        // Anything containing 17:00 New York is the thing this module avoids.
        assert!(crosses_rollover(16 * 60, 18 * 60));
        assert!(crosses_rollover(0, 24 * 60 - 1), "a near-whole day contains it");
        assert!(crosses_rollover(17 * 60, 17 * 60 + 15), "the boundary bar itself");
        assert!(crosses_rollover(9 * 60, 9 * 60), "an empty window is not a window");
        // And the strategies refuse it rather than trading it quietly.
        let mut p = TsmomFlat.default_params();
        p.set("from", 1600.0);
        p.set("to", 1800.0);
        assert_eq!(window_minutes(&p), None);
        p.set("from", 2500.0);
        p.set("to", 1600.0);
        assert_eq!(window_minutes(&p), None, "an unreadable hhmm is refused too");
    }

    /// 15-minute bars from 17:00 New York (21:00 UTC in summer) on 2026-06-01,
    /// `per` of them a session, each session's whole move on its last bar.
    fn series(steps: &[f64], range: f64, per: usize) -> Vec<Bar> {
        let mut bars = Vec::new();
        let mut price = 1000.0;
        let start = days_from_civil(2026, 6, 1) * DAY_MS + 21 * HOUR_MS;
        for (d, step) in steps.iter().enumerate() {
            for b in 0..per {
                let t = start + (d as i64) * DAY_MS + (b as i64) * 900_000;
                let close = if b + 1 == per { price + step } else { price };
                bars.push(Bar {
                    time: t,
                    open: price,
                    high: close.max(price) + range / 2.0,
                    low: close.min(price) - range / 2.0,
                    close,
                    volume: None,
                });
            }
            price += step;
        }
        bars
    }

    /// One bar an hour, 24 a day, from 2026-06-01 00:00 New York, rising `step`
    /// dollars an hour with a $2 bar range.
    fn hourly(hours: i64, step: f64) -> Vec<Bar> {
        (0..hours)
            .map(|h| {
                // 2026-06 is EDT: New York = UTC - 4.
                let t = days_from_civil(2026, 6, 1) * DAY_MS + (h + 4) * HOUR_MS;
                let c = 1000.0 + step * h as f64;
                Bar { time: t, open: c, high: c + 1.0, low: c - 1.0, close: c, volume: None }
            })
            .collect()
    }

    fn intent(strategy: &dyn Strategy, bars: &[Bar], i: usize, position: Option<OpenPosition>, p: &Params) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position, params: p };
        strategy.on_bar(&ctx)
    }

    #[test]
    fn a_position_opens_at_18_00_new_york_closes_at_16_00_and_never_spans_17_00() {
        let bars = hourly(24 * 90, 0.05);
        let mut p = TsmomFlat.default_params();
        p.set("lookbackDays", 20.0);
        let open = |i: usize| OpenPosition {
            side: Side::Long,
            entry_price: bars[i].close,
            entry_time: bars[i].time,
            stop: None,
            target: None,
        };
        // Day 60 of the series; hour 18 of it is the window's first bar.
        let base = 60 * 24;
        let entry = intent(&TsmomFlat, &bars, base + 18, None, &p);
        let Intent::Enter { side: Side::Long, stop, target: None, .. } = entry else { panic!("{entry:?}") };
        assert!(stop.unwrap() < bars[base + 18].close, "a long's sizing stop sits below the price");
        assert_eq!(ny_day_minute(bars[base + 18].time).1, 18 * 60, "the entry bar is 18:00 New York");

        // 19:00 is inside but not the first bar.
        assert_eq!(intent(&TsmomFlat, &bars, base + 19, None, &p), Intent::None);
        // Held right through midnight and the New York morning.
        for h in [19, 23, 24 + 2, 24 + 9, 24 + 15] {
            assert_eq!(intent(&TsmomFlat, &bars, base + h, Some(open(base + 18)), &p), Intent::None, "hour {h}");
        }
        // Out at 16:00 the next day — one hour before the rollover.
        let out = intent(&TsmomFlat, &bars, base + 24 + 16, Some(open(base + 18)), &p);
        assert!(matches!(out, Intent::Exit { .. }), "{out:?}");
        let (_, exit_minute) = ny_day_minute(bars[base + 24 + 16].time);
        assert_eq!(exit_minute, 16 * 60);
        // THE invariant: the realised hold crosses no 17:00 New York boundary.
        assert_eq!(
            fd_core::clock::swap_nights(bars[base + 18].time, bars[base + 24 + 16].time),
            0,
            "a flat-across-the-rollover position must be charged zero nights"
        );
        // And it is flat over the rollover hour itself.
        assert_eq!(intent(&TsmomFlat, &bars, base + 24 + 17, None, &p), Intent::None, "17:00 is outside the window");
    }

    #[test]
    fn tsmom_flat_takes_the_sign_of_the_trailing_return_and_waits_for_the_history() {
        let mut p = TsmomFlat.default_params();
        p.set("lookbackDays", 20.0);
        let rising = hourly(24 * 90, 0.05);
        let falling = hourly(24 * 90, -0.05);
        let base = 60 * 24 + 18;
        assert!(matches!(intent(&TsmomFlat, &rising, base, None, &p), Intent::Enter { side: Side::Long, .. }));
        assert!(matches!(intent(&TsmomFlat, &falling, base, None, &p), Intent::Enter { side: Side::Short, .. }));
        // No 20-day history on day 5: the scan finds no bar that old.
        let early = 5 * 24 + 18;
        assert_eq!(intent(&TsmomFlat, &rising, early, None, &p), Intent::None);
        assert_eq!(trailing_day_return(&rising[..=early], &rising[early], 20), None);
        // A flat tape has no side.
        let flat = hourly(24 * 90, 0.0);
        assert_eq!(intent(&TsmomFlat, &flat, base, None, &p), Intent::None);
    }

    #[test]
    fn the_signal_is_read_at_the_same_new_york_minute_the_parent_rebalances_at() {
        // tsmom's default rebalanceHHMM is 1800 and this variant's `from` is
        // 1800, so the two read the trailing return on the same bar. If either
        // default moves, this test says so.
        assert_eq!(crate::tsmom::TimeSeriesMomentum.default_params().get("rebalanceHHMM"), 1800.0);
        assert_eq!(TsmomFlat.default_params().get("from"), 1800.0);
        let bars = hourly(24 * 90, 0.05);
        let i = 60 * 24 + 18;
        let mine = trailing_day_return(&bars[..=i], &bars[i], 20).unwrap();
        // The parent computes bar.close / past.close - 1 over the same scan.
        let (day, _) = ny_day_minute(bars[i].time);
        let past = bars[..=i].iter().rev().find(|b| ny_day_minute(b.time).0 <= day - 20).unwrap();
        assert!((mine - (bars[i].close / past.close - 1.0)).abs() < 1e-12);
    }

    fn qs_params() -> Params {
        let mut p = QuietSwingFlat.default_params();
        p.set("lookbackSessions", 3.0);
        p.set("windowSessions", 6.0);
        p.set("holdSessions", 2.0);
        p.set("rangeDays", 5.0);
        p
    }

    #[test]
    fn quiet_swing_flat_enters_a_calm_trending_tape_and_refuses_a_loud_one() {
        let p = qs_params();
        // Sessions of eight 15-minute bars starting at 17:00 New York, so bar
        // 8*s + 4 is 18:00 of session s: inside the window and its first bar.
        // hold 2 + window 6 + lookback 3 + 1 = 12 sessions must be available,
        // which needs 13 closed ones because the scan discards the oldest.
        let calm = series(&[1.0; 30], 2.0, 8);
        let i = 13 * 8 + 4;
        assert_eq!(ny_day_minute(calm[i].time).1, 18 * 60, "the decision bar is 18:00 New York");
        let got = intent(&QuietSwingFlat, &calm, i, None, &p);
        assert!(matches!(got, Intent::Enter { side: Side::Long, .. }), "{got:?}");

        let falling = series(&[-1.0; 30], 2.0, 8);
        let got = intent(&QuietSwingFlat, &falling, i, None, &p);
        assert!(matches!(got, Intent::Enter { side: Side::Short, .. }), "{got:?}");

        // Widen the four most recent completed sessions — every session the
        // hold clock (2) and the lookback (3) can reach — so both quiet tests
        // sit at the 100th percentile of their own windows: loud, no entry.
        let mut loud = calm.clone();
        for b in 9 * 8..=i {
            loud[b].high += 60.0;
            loud[b].low -= 60.0;
        }
        assert_eq!(intent(&QuietSwingFlat, &loud, i, None, &p), Intent::None, "a loud tape is not entered");
    }

    #[test]
    fn the_hold_clock_and_the_sign_run_are_what_keep_the_state_on() {
        // Sums are all equal on a constant-range tape, so NOTHING is strictly
        // below the current sum: the 0th percentile, quiet at every session.
        let sessions: Vec<SessionBar> =
            (0..40).rev().map(|s| SessionBar { high: 1000.0 + s as f64, low: 998.0 + s as f64, close: 999.0 + s as f64, bars: 8 }).collect();
        // Newest first and rising, so the trailing return is positive.
        assert_eq!(quiet_swing_state(&sessions, 3, 6, 2, 0.5), Some(Side::Long));
        // quietPct = 0.0 can never be cleared: a percentile is never below 0.
        assert_eq!(quiet_swing_state(&sessions, 3, 6, 2, 0.0), None);
        // A hold clock of zero is not a mechanism.
        assert_eq!(quiet_swing_state(&sessions, 3, 6, 0, 0.5), None);
        // Too little history is `None`, never a side.
        assert_eq!(quiet_swing_state(&sessions[..5], 3, 6, 2, 0.5), None);

        // Sign run: break the sign one session back and a hold of 2 still
        // holds (the current session's own quiet test carries it), but the
        // state is the CURRENT sign, never the older one.
        let mut broken = sessions.clone();
        broken[0].close = broken[3].close - 5.0; // trailing 3-session return now negative
        assert_eq!(quiet_swing_state(&broken, 3, 6, 2, 0.5), Some(Side::Short));
    }

    #[test]
    fn nothing_is_decided_before_the_sessions_exist() {
        let bars = series(&[1.0; 30], 2.0, 8);
        let p = qs_params();
        // hold 2 + window 6 + lookback 3 + 1 = 12 available sessions, which
        // needs 13 closed ones: the scan always discards the oldest it reaches.
        for session in 0..13 {
            let i = session * 8 + 4;
            assert_eq!(intent(&QuietSwingFlat, &bars, i, None, &p), Intent::None, "session {session} is too early");
        }
        assert!(matches!(intent(&QuietSwingFlat, &bars, 13 * 8 + 4, None, &p), Intent::Enter { .. }));
    }

    #[test]
    fn both_strategies_declare_an_empty_grid_and_own_their_exits() {
        // An empty grid is what makes `hypotheses::control_for` build the
        // RandomHold drift null instead of the stop-and-target one. If either
        // of these gains a grid axis, every percentile in the record moves.
        for s in [&TsmomFlat as &dyn Strategy, &QuietSwingFlat] {
            assert!(s.grid().is_empty(), "{} must have an empty grid", s.id());
            assert_eq!(s.exits(), Exits::Strategy, "{} must own its exits", s.id());
            let p = s.default_params();
            assert!(p.contains("from") && p.contains("to"), "{} must carry the window", s.id());
            assert!(p.contains("riskDailyRanges") && p.contains("rangeDays"), "{} must size on daily ranges", s.id());
            assert!(!crosses_rollover(hhmm(p.get("from")).unwrap(), hhmm(p.get("to")).unwrap()));
        }
    }

    /// The session boundary the whole module is built around, asserted here so
    /// a change to `new_york_offset_ms` cannot move it in silence.
    #[test]
    fn the_rollover_minute_is_the_boundary_session_index_turns_over_on() {
        for (y, m, d) in [(2026, 6, 18), (2026, 12, 18)] {
            let midnight = days_from_civil(y, m, d) * DAY_MS;
            // Find the UTC instant whose New York minute is 17:00 on that day.
            let t = (0..96)
                .map(|q| midnight + q * 900_000)
                .find(|t| ny_day_minute(*t) == (days_from_civil(y, m, d), ROLLOVER_MINUTE))
                .expect("17:00 New York exists on that day");
            assert_eq!(session_index(t) - session_index(t - 1), 1, "{y}-{m}-{d}: the boundary is 17:00 New York");
        }
    }
}
