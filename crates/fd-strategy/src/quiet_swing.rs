//! Multi-day continuation taken only while the tape is quiet.
//!
//! Designed for `docs/hypotheses/2026-09-23-designed-methods.md`, angle 3
//! (lower frequency). Design note:
//! `docs/research/designs/2026-09-23-designed-3-quiet-swing.md`.
//!
//! # The mechanism, stated before anything was fitted
//!
//! Two things were true about this market before any parameter here existed.
//!
//! 1. **Realised volatility clusters.** Absolute returns are positively
//!    autocorrelated at the daily horizon in every liquid market (Engle,
//!    1982). So "the tape has been quiet for two weeks" is a statement about
//!    the *next* two weeks as well as the last two, and it can be measured
//!    from completed sessions alone.
//! 2. **Signal extraction.** If an observed `K`-session move is a drift term
//!    plus noise, the posterior mean of the drift given that move scales as
//!    `var(drift) / (var(drift) + var(noise))`. The same observed move implies
//!    *more* drift when the noise variance is small. So whatever short-horizon
//!    continuation exists should be detectable when realised range is low and
//!    should wash out when it is high.
//!
//! That is a prediction about *where* continuation lives, not a claim that it
//! exists. On the design window it is what the bars say: over 3,958 Dukascopy
//! gold sessions (2010-06 → 2025-09-22), the two-sided `K=10`-session
//! momentum earns +0.1649 of a 20-session ATR over the following five
//! sessions when the trailing 10-session true-range sum sits in the bottom
//! third of its own trailing year (n=1,323, t=+3.48), and **−0.0027** when it
//! sits in the top third (n=1,294, t=−0.07). The quiet-third figure survives
//! splitting the fifteen years into three five-year blocks: +0.136, +0.177,
//! +0.137, all with a long share of 50–61%, so it is not gold's drift.
//!
//! # Why this shape and not an intraday one
//!
//! `cost/R = spread/stop`. The sizing unit here is 1.5 average New York-day
//! ranges — tens of dollars of gold — against a configured spread of 0.28,
//! so the spread costs about 1% of R. An intraday method stopped two dollars
//! away pays 14% of R for the same signal. The break-even edge falls by more
//! than a factor of ten, and that is the whole point of the angle.
//!
//! # Exits, and why the strategy owns them
//!
//! `[trading] max_hold_ms` is 14,400,000 — four hours — and
//! `trading_rules_for` reads it from the shared table with no per-market
//! override, so **every engine-managed method on this desk is force-closed
//! four hours after entry whatever its signal horizon**. A five-session hold
//! is unrepresentable under `Exits::Engine`. So exits are the strategy's:
//! it closes on the decision bar of the `holdSessions`-th session after
//! entry, or earlier when the sign of the `K`-session return flips. Loss
//! control is then the guards' `max_open_loss_r` (2.0 R of the sizing unit),
//! the weekend flat and the news windows — which is what those guards are
//! for, and which the design note measures the cost of.
//!
//! **Sizing.** `riskDailyRanges` × the average New York-day range of the last
//! `rangeDays` days, the same function `tsmom` uses and the same two parameter
//! names, *deliberately*: `hypotheses::control_for` copies exactly those two
//! names onto the `RandomHold` drift null, so the null is sized like the
//! method and pays the same spread as a share of its own R. A null sized on a
//! 15-minute ATR would pay ten times the cost per R this method does, and the
//! percentile would then be measuring the sizing rule rather than the signal.
//!
//! # Causality
//!
//! Every number on the decision bar comes from sessions that had already
//! closed when that bar opened: [`completed_sessions`] scans backwards over
//! `bars[..=i]` and discards every bar of the session bar `i` itself belongs
//! to. `tests/causality.rs` asserts the intent sequence over a truncated
//! series is a prefix of the sequence over the full one.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

/// Trading-session index, counted from 17:00 New York.
///
/// The same boundary `fd_core::clock::swap_nights` charges a rollover on, and
/// the one the CFD's own day turns over at — not midnight UTC, which cuts the
/// New York afternoon in half and moves against the wall clock twice a year.
#[must_use]
pub fn session_index(utc_ms: i64) -> i64 {
    (utc_ms + new_york_offset_ms(utc_ms) - 17 * HOUR_MS).div_euclid(DAY_MS)
}

/// One completed trading session, reduced to what this method reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionBar {
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Bars the feed printed in this session; used only to recognise a stub.
    pub bars: usize,
}

/// The last `want` completed sessions before the session bar `i` belongs to,
/// newest first, or `None` when the history does not reach that far.
///
/// **Completed** is the whole point: every bar whose session index is the
/// current one is skipped, so the session in progress contributes nothing.
/// Reads `bars[..=i]` only.
///
/// A session the feed printed far fewer bars for than its neighbours is a
/// holiday stub, not a session: its true range is small for a reason that has
/// nothing to do with a quiet market, and letting it into the range sum makes
/// the week after every holiday look quiet. So `want + want/5 + 2` sessions
/// are collected and those with fewer than half the bar count of the fullest
/// are dropped — the same rule, for the same reason, as
/// `tsmom::average_day_range` (`docs/decisions/2026-09-13-close-reopen-drift.md`).
#[must_use]
pub fn completed_sessions(bars: &[Bar], i: usize, want: usize) -> Option<Vec<SessionBar>> {
    if want == 0 || i >= bars.len() {
        return None;
    }
    let collect = want + want / 5 + 2;
    let now = session_index(bars[i].time);
    let mut found: Vec<SessionBar> = Vec::with_capacity(collect);
    let mut current: Option<i64> = None;
    let mut acc = SessionBar { high: f64::NEG_INFINITY, low: f64::INFINITY, close: f64::NAN, bars: 0 };
    for b in bars[..=i].iter().rev() {
        let s = session_index(b.time);
        if s >= now {
            continue;
        }
        match current {
            Some(c) if c != s => {
                // `acc` was terminated by a bar of an earlier session, so the
                // session it describes is whole.
                found.push(acc);
                if found.len() == collect {
                    break;
                }
                current = Some(s);
                // Scanning backwards: this bar is the session's LAST bar, so
                // its close is the session's close.
                acc = SessionBar { high: b.high, low: b.low, close: b.close, bars: 1 };
            }
            Some(_) => {
                acc.high = acc.high.max(b.high);
                acc.low = acc.low.min(b.low);
                acc.bars += 1;
            }
            None => {
                current = Some(s);
                acc = SessionBar { high: b.high, low: b.low, close: b.close, bars: 1 };
            }
        }
    }
    // `acc` is deliberately NOT pushed here. The oldest session the scan
    // reached may be cut off by the start of the data, and a truncated
    // session's range is not its range; the scan cannot tell the two apart, so
    // it always discards the oldest. The cost is one session of extra history —
    // `want` sessions need `want + 1` in the feed — and the alternative is a
    // range sum that is quietly too small at the left edge of every window.
    let fullest = found.iter().map(|s| s.bars).max().unwrap_or(0);
    let kept: Vec<SessionBar> = found.into_iter().filter(|s| s.bars * 2 >= fullest).take(want).collect();
    (kept.len() == want).then_some(kept)
}

/// True range of `sessions[j]`, which needs `sessions[j + 1]`'s close.
fn true_range(sessions: &[SessionBar], j: usize) -> Option<f64> {
    let (s, prev) = (sessions.get(j)?, sessions.get(j + 1)?);
    let r = s.high.max(prev.close) - s.low.min(prev.close);
    r.is_finite().then_some(r)
}

pub struct QuietTapeSwing;

impl Strategy for QuietTapeSwing {
    fn id(&self) -> &'static str {
        "quiet-swing"
    }
    fn name(&self) -> &'static str {
        "Quiet-tape multi-day continuation"
    }
    fn description(&self) -> &'static str {
        "At the 17:00 New York session boundary, hold the side of the trailing K-session return for \
         holdSessions sessions — but only when the trailing K-session true-range sum sits in the low \
         part of its own trailing-window distribution. Stops are structural (daily ranges), exits are \
         the strategy's, and the guards carry the weekend and the calendar."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("lookbackSessions", 10.0),
            ("windowSessions", 60.0),
            ("quietPct", 0.50),
            ("holdSessions", 5.0),
            ("riskDailyRanges", 1.5),
            ("rangeDays", 20.0),
            ("atrPeriod", 14.0),
        ])
    }
    /// **Empty on purpose.** An empty grid is what makes
    /// `hypotheses::control_for` build the `RandomHold` drift null rather than
    /// the stop-and-target `RandomEntry` one, and a drift null is the only
    /// control a multi-day self-managed hold can be measured against. It also
    /// means the parameters can only arrive frozen, which is what the
    /// registration asks for.
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        // Declared so `sizing_atr_key` resolves to a real series; the entry
        // always carries an explicit sizing stop, so this ATR never sets risk.
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // Tight on purpose. The real gate is `completed_sessions` returning
        // `None` until the sessions exist, which is exact on any bar width;
        // a generous bar-count warmup would throw away weeks of a one-year
        // test window for nothing. `tsmom` takes the other choice and its
        // three cells took zero trades on a three-month window because of it.
        p.period("lookbackSessions") + p.period("windowSessions") + p.period("rangeDays") + 4
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let p = ctx.params;
        let bars = &ctx.bars[..=ctx.i];
        let bar = ctx.bar;
        let now = session_index(bar.time);

        // One decision per session, on its first bar. `prev` is the bar before
        // this one in the same series, so this is a pure function of the past.
        if !ctx.prev().is_none_or(|b| session_index(b.time) != now) {
            return Intent::None;
        }

        let k = p.period("lookbackSessions");
        let w = p.period("windowSessions");
        let hold = p.period("holdSessions");
        let quiet_pct = p.get("quietPct");
        if k == 0 || w == 0 || hold == 0 || !quiet_pct.is_finite() {
            return Intent::None;
        }

        let Some(sessions) = completed_sessions(bars, ctx.i, k + w + 1) else {
            return Intent::None;
        };
        // Sign of the trailing K-session return, from completed closes only.
        let (last, past) = (sessions[0].close, sessions[k].close);
        if !(last.is_finite() && past.is_finite() && past != 0.0) {
            return Intent::None;
        }
        let ret = last / past - 1.0;
        let side = if ret > 0.0 {
            Side::Long
        } else if ret < 0.0 {
            Side::Short
        } else {
            return Intent::None;
        };

        // An open position is managed first: the hold clock and the flip are
        // the only exits this method issues, and neither needs the tape to be
        // quiet — the quiet condition decides when to ENTER, not when to hold.
        if let Some(open) = ctx.position {
            let held = now - session_index(open.entry_time);
            if held >= hold as i64 {
                return Intent::Exit { reason: format!("held {held} sessions of {hold}") };
            }
            if open.side != side {
                return Intent::Exit { reason: format!("{k}-session return flipped to {:+.2}%", ret * 100.0) };
            }
            return Intent::None;
        }

        // Where the trailing K-session true-range sum sits inside the last `w`
        // values of the same statistic, each of them computed from sessions
        // strictly older than the one before it. Nothing here reads a session
        // that had not closed.
        let mut sums: Vec<f64> = Vec::with_capacity(w + 1);
        for m in 0..=w {
            let mut total = 0.0;
            for j in m..m + k {
                let Some(tr) = true_range(&sessions, j) else { return Intent::None };
                total += tr;
            }
            sums.push(total);
        }
        let current = sums[0];
        if !(current.is_finite() && current > 0.0) {
            return Intent::None;
        }
        let below = sums[1..].iter().filter(|v| v.is_finite() && **v < current).count();
        let pct = below as f64 / w as f64;
        if pct >= quiet_pct {
            return Intent::None;
        }

        // Sizing stop only — the engine does not enforce it for a
        // self-managed position, but it sizes lots and defines R from it, and
        // the drift null is sized from the same two parameter names.
        let Some(stop) =
            crate::tsmom::sizing_stop(bars, bar, side, p.get("riskDailyRanges"), p.period("rangeDays"))
        else {
            return Intent::None;
        };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: None,
            reason: format!(
                "{k}-session return {:+.2}%, range sum at the {:.0}th percentile of the last {w} sessions",
                ret * 100.0,
                pct * 100.0
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_core::clock::days_from_civil;

    /// 15-minute bars, `per` of them per session, starting at 17:00 New York
    /// (21:00 UTC in summer) on 2026-06-01, with each session's close moving
    /// by `step[session]` dollars and a fixed high-low range of `range`.
    fn series(steps: &[f64], range: f64, per: usize) -> Vec<Bar> {
        let mut bars = Vec::new();
        let mut price = 1000.0;
        let start = days_from_civil(2026, 6, 1) * DAY_MS + 21 * HOUR_MS;
        for (d, step) in steps.iter().enumerate() {
            for b in 0..per {
                let t = start + (d as i64) * DAY_MS + (b as i64) * 900_000;
                // The whole move lands on the session's last bar, so each
                // session's close is `price + step` and its range is `range`.
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

    fn params() -> Params {
        let mut p = QuietTapeSwing.default_params();
        p.set("lookbackSessions", 3.0);
        p.set("windowSessions", 6.0);
        p.set("holdSessions", 2.0);
        p.set("rangeDays", 5.0);
        p
    }

    fn intent(bars: &[Bar], i: usize, position: Option<OpenPosition>, p: &Params) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position, params: p };
        QuietTapeSwing.on_bar(&ctx)
    }

    #[test]
    fn the_session_index_turns_over_at_17_00_new_york() {
        // 2026-06-18 21:00Z is 17:00 EDT.
        let t = days_from_civil(2026, 6, 18) * DAY_MS + 21 * HOUR_MS;
        assert_eq!(session_index(t) - session_index(t - 1), 1, "the boundary is exactly 17:00 NY");
        // In winter the same wall clock is 22:00Z.
        let w = days_from_civil(2026, 12, 18) * DAY_MS + 22 * HOUR_MS;
        assert_eq!(session_index(w) - session_index(w - 1), 1, "and it moves with New York, not with UTC");
    }

    #[test]
    fn only_completed_sessions_are_read_and_a_truncated_one_is_refused() {
        let bars = series(&[1.0; 12], 2.0, 4);
        // Bar 4 opens session 1. One session has closed, but the scan always
        // discards the oldest it reaches, so one is not yet available.
        assert_eq!(completed_sessions(&bars, 4, 1), None);
        // Bar 8 opens session 2: sessions 0 and 1 have closed, 0 is discarded.
        assert_eq!(completed_sessions(&bars, 8, 1).map(|s| s.len()), Some(1));
        assert_eq!(completed_sessions(&bars, 8, 2), None, "two are not available yet");
        // Mid-session the answer does not change: the session in progress is
        // not a session.
        assert_eq!(completed_sessions(&bars, 11, 2), None);
        // Newest first, and the close is the session's LAST bar's close.
        let got = completed_sessions(&bars, 12, 2).expect("sessions 1 and 2 have closed");
        assert_eq!(got[0].close, bars[11].close);
        assert_eq!(got[1].close, bars[7].close);
        assert!(got[0].close > got[1].close, "session 2 closed above session 1");
    }

    #[test]
    fn a_holiday_stub_is_not_a_session() {
        // Nine full sessions of 8 bars, then one of 2 bars, then nine more.
        let mut bars = series(&[1.0; 10], 2.0, 8);
        let stub_start = bars.last().unwrap().time + DAY_MS;
        for b in 0..2 {
            bars.push(Bar::flat(stub_start + b * 900_000, 1100.0));
        }
        let tail = series(&[1.0; 9], 2.0, 8);
        let shift = stub_start + DAY_MS - tail[0].time;
        bars.extend(tail.into_iter().map(|mut b| {
            b.time += shift;
            b
        }));
        let last = bars.len() - 1;
        let got = completed_sessions(&bars, last, 12).expect("enough history");
        assert!(got.iter().all(|s| s.bars * 2 >= 8), "the two-bar stub was dropped: {:?}", got.iter().map(|s| s.bars).collect::<Vec<_>>());
    }

    #[test]
    fn it_decides_once_a_session_on_the_first_bar() {
        let bars = series(&[1.0; 30], 2.0, 4);
        let p = params();
        // Session 11 starts at bar 44; every other bar of it is silent.
        let first = intent(&bars, 44, None, &p);
        assert!(matches!(first, Intent::Enter { .. }), "{first:?}");
        for b in 45..48 {
            assert_eq!(intent(&bars, b, None, &p), Intent::None, "bar {b} is not a decision bar");
        }
    }

    #[test]
    fn it_takes_the_side_of_the_trailing_return_and_refuses_a_loud_tape() {
        let p = params();

        // Rising, and every session the same small range: the trailing
        // 3-session range sum equals every one of the previous six, so
        // nothing is strictly below it — the 0th percentile, and quiet.
        let calm = series(&[1.0; 30], 2.0, 4);
        let Intent::Enter { side, stop, .. } = intent(&calm, 48, None, &p) else {
            panic!("expected an entry on a calm rising tape")
        };
        assert_eq!(side, Side::Long);
        assert!(stop.unwrap() < calm[48].close, "a long's sizing stop sits below the price");

        // Falling instead: same ranges, opposite sign.
        let falling = series(&[-1.0; 30], 2.0, 4);
        let Intent::Enter { side, stop, .. } = intent(&falling, 48, None, &p) else {
            panic!("expected an entry on a calm falling tape")
        };
        assert_eq!(side, Side::Short);
        assert!(stop.unwrap() > falling[48].close, "a short's sizing stop sits above the price");

        // Now make the two most recent sessions the widest in the window: the
        // trailing sum is above every earlier one, the 100th percentile, loud.
        let mut loud = calm.clone();
        for b in 40..48 {
            loud[b].high += 40.0;
            loud[b].low -= 40.0;
        }
        assert_eq!(intent(&loud, 48, None, &p), Intent::None, "a loud tape is not entered");
    }

    #[test]
    fn it_holds_for_the_session_count_and_flips_on_a_sign_change() {
        let bars = series(&[1.0; 30], 2.0, 4);
        let p = params(); // holdSessions = 2
        let entry_time = bars[48].time;
        let long = OpenPosition { side: Side::Long, entry_price: 1000.0, entry_time, stop: None, target: None };

        assert_eq!(intent(&bars, 52, Some(long), &p), Intent::None, "one session held of two");
        let out = intent(&bars, 56, Some(long), &p);
        assert!(matches!(out, Intent::Exit { .. }), "two sessions held: out. {out:?}");

        let short = OpenPosition { side: Side::Short, ..long };
        let flip = intent(&bars, 52, Some(short), &p);
        assert!(matches!(flip, Intent::Exit { .. }), "wrong side of a rising tape: out. {flip:?}");
    }

    #[test]
    fn nothing_is_decided_before_the_sessions_exist() {
        let bars = series(&[1.0; 30], 2.0, 4);
        // Needs lookback 3 + window 6 + 1 = 10 available sessions, which needs
        // 11 closed ones because the scan discards the oldest it reaches.
        let p = params();
        for session in 0..11 {
            let i = session * 4;
            assert_eq!(intent(&bars, i, None, &p), Intent::None, "session {session} has too little history");
        }
        assert!(matches!(intent(&bars, 44, None, &p), Intent::Enter { .. }), "session 11 has enough");
    }
}
