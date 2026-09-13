//! Market intraday momentum: the first half-hour predicts the last.
//!
//! Gao, Han, Li and Zhou (2018, "Market intraday momentum", Journal of
//! Financial Economics) report that the return over the first half-hour of
//! the trading day predicts the return over the last half-hour, with the
//! same sign. The rule here is session-hold with a signed predictor: on the
//! first bar of the New York hold window the sign of the same day's
//! `firstFrom`–`firstTo` return picks the side, the position is held to the
//! window's end, and the clock is the only exit.
//!
//! **Sizing.** As session-hold: a sizing stop of `riskDailyRanges` mean New
//! York-day ranges (over `rangeDays` days), which the engine uses for lots
//! and R and does not enforce. `minMove` (in the same daily ranges) drops
//! predictor moves too small to carry a sign.
//!
//! **No future.** The predictor is read from bars at indices ≤ `i` on the
//! signal bar's own New York day; a day whose predictor window has no bars
//! (holiday, hole) gives no trade.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

pub struct IntradayMomentum;

impl Strategy for IntradayMomentum {
    fn id(&self) -> &'static str {
        "intraday-momentum"
    }
    fn name(&self) -> &'static str {
        "Intraday momentum"
    }
    fn description(&self) -> &'static str {
        "Long (short) through a late New York window when the day's first half-hour return was positive (negative), after Gao, Han, Li and Zhou (2018), \"Market intraday momentum\"."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("firstFrom", 830.0),
            ("firstTo", 900.0),
            ("from", 1515.0),
            ("to", 1630.0),
            ("minMove", 0.0),
            ("riskDailyRanges", 1.0),
            ("rangeDays", 20.0),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // The windows are the paper's; a sweep over them would be a search.
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let p = ctx.params;
        let from = hhmm(p.get("from"));
        let to = hhmm(p.get("to"));
        let inside = |m: u32| if from <= to { m >= from && m < to } else { m >= from || m < to };
        let (today, minute) = ny_day_minute(ctx.bar.time);
        match ctx.position {
            Some(_) if !inside(minute) => Intent::Exit { reason: "window closed".into() },
            Some(_) => Intent::None,
            None if inside(minute) => {
                // Enter only at the window's first bar of this day: a
                // position opened late in the window is a different, shorter
                // hold, and yesterday's window does not continue into today's.
                let previous_inside = ctx.prev().is_some_and(|b| {
                    let (d, m) = ny_day_minute(b.time);
                    (from > to || d == today) && inside(m)
                });
                if previous_inside {
                    return Intent::None;
                }
                let first_from = hhmm(p.get("firstFrom"));
                let first_to = hhmm(p.get("firstTo"));
                let Some((start, end)) = predictor(&ctx.bars[..=ctx.i], today, first_from, first_to) else {
                    return Intent::None;
                };
                let r = end.close / start.open - 1.0;
                let side = if r > 0.0 {
                    Side::Long
                } else if r < 0.0 {
                    Side::Short
                } else {
                    return Intent::None;
                };
                let days = p.period("rangeDays");
                let min_move = p.get("minMove");
                if min_move > 0.0 {
                    // One mean daily range, read through the sizing stop so
                    // the threshold and the stop share one definition of a day.
                    let Some(one_range_below) = crate::tsmom::sizing_stop(&ctx.bars[..=ctx.i], ctx.bar, Side::Long, 1.0, days) else {
                        return Intent::None;
                    };
                    let range = ctx.bar.close - one_range_below;
                    if (end.close - start.open).abs() < min_move * range {
                        return Intent::None;
                    }
                }
                let Some(stop) = crate::tsmom::sizing_stop(&ctx.bars[..=ctx.i], ctx.bar, side, p.get("riskDailyRanges"), days) else {
                    return Intent::None;
                };
                Intent::Enter {
                    side,
                    stop: Some(stop),
                    target: None,
                    reason: format!(
                        "first {}-{} {:+.2}%; hold {}-{} New York",
                        clock(p.get("firstFrom")),
                        clock(p.get("firstTo")),
                        r * 100.0,
                        clock(p.get("from")),
                        clock(p.get("to"))
                    ),
                }
            }
            None => Intent::None,
        }
    }
}

/// The predictor window's bounds on `day`: the first bar at or after
/// `first_from` (its open starts the return) and the last bar before
/// `first_to` (its close ends it), both on `day` and in `bars`, which ends at
/// the signal bar. `None` when either is missing or the start comes after
/// the end — a hole where the window should be. On a thirty-minute feed the
/// two are the same bar.
fn predictor(bars: &[Bar], day: i64, first_from: u32, first_to: u32) -> Option<(&Bar, &Bar)> {
    let mut start = None;
    let mut end = None;
    for (idx, b) in bars.iter().enumerate().rev() {
        let (d, m) = ny_day_minute(b.time);
        if d != day {
            break;
        }
        if m >= first_from {
            start = Some(idx); // scanning backwards: the last one seen is the earliest
        }
        if m < first_to && end.is_none() {
            end = Some(idx);
        }
    }
    let (s, e) = (start?, end?);
    if s <= e { Some((&bars[s], &bars[e])) } else { None }
}

fn ny_day_minute(utc_ms: i64) -> (i64, u32) {
    let local = utc_ms + new_york_offset_ms(utc_ms);
    (local.div_euclid(DAY_MS), (local.rem_euclid(DAY_MS) / 60_000) as u32)
}

fn hhmm(v: f64) -> u32 {
    let v = v.round().max(0.0) as u32;
    (v / 100) * 60 + v % 100
}

fn clock(v: f64) -> String {
    let v = v.round().max(0.0) as u32;
    format!("{:02}:{:02}", v / 100, v % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{OpenPosition, Registry};
    use fd_core::clock::days_from_civil;

    fn day(d: i64) -> i64 {
        (days_from_civil(2026, 8, 24) + d) * DAY_MS
    }

    /// The test day is 2026-09-14, EDT: New York = UTC - 4.
    fn at(hour: i64, minute: i64) -> i64 {
        day(21) + (hour + 4) * 3_600_000 + minute * 60_000
    }

    /// Twenty-one prior days of two bars each (08:30 and 08:45 New York, a
    /// $2 range, an up predictor of +0.9%) so the daily-range sizing stop
    /// exists and a leak from yesterday's window would show as a Long.
    fn history() -> Vec<Bar> {
        (0..21)
            .flat_map(|d| {
                let t = |h: i64, m: i64| day(d) + (h + 4) * 3_600_000 + m * 60_000;
                [
                    Bar { time: t(8, 30), open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: None },
                    Bar { time: t(8, 45), open: 100.0, high: 101.0, low: 99.0, close: 100.9, volume: None },
                ]
            })
            .collect()
    }

    /// The test day: fifteen-minute flat bars at 100 from 08:00 to 17:00 New
    /// York, the 08:45 bar closing at `end` so the 08:30 open → 09:00 close
    /// return is `end / 100 - 1`.
    fn day_bars(end: f64) -> Vec<Bar> {
        let mut bars = history();
        for q in 0..=36 {
            let t = at(8, 0) + q * 15 * 60_000;
            let mut b = Bar::flat(t, 100.0);
            if t == at(8, 45) {
                b.close = end;
                b.high = b.high.max(end);
                b.low = b.low.min(end);
            }
            bars.push(b);
        }
        bars
    }

    fn index_at(bars: &[Bar], hour: i64, minute: i64) -> usize {
        bars.iter().position(|b| b.time == at(hour, minute)).expect("the test day has this bar")
    }

    fn intent_at(bars: &[Bar], params: &Params, i: usize, position: Option<OpenPosition>) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position, params };
        IntradayMomentum.on_bar(&ctx)
    }

    #[test]
    fn registered_and_self_managed() {
        let registry = Registry::with_builtins();
        let s = registry.get("intraday-momentum").unwrap();
        assert_eq!(s.exits(), Exits::Strategy);
        assert!(s.grid().is_empty());
    }

    #[test]
    fn an_up_first_half_hour_buys_the_hold_window_and_a_down_one_sells_it() {
        let params = IntradayMomentum.default_params();
        let up = day_bars(100.42);
        let i = index_at(&up, 15, 15);
        let it = intent_at(&up, &params, i, None);
        let Intent::Enter { side: Side::Long, stop: Some(stop), target: None, reason } = it else { panic!("{it:?}") };
        assert!((stop - 98.0).abs() < 1e-9, "one mean daily range of $2 below the close: {stop}");
        assert_eq!(reason, "first 08:30-09:00 +0.42%; hold 15:15-16:30 New York");
        assert_eq!(intent_at(&up, &params, i + 1, None), Intent::None, "not the first bar of the window");
        assert_eq!(intent_at(&up, &params, index_at(&up, 12, 0), None), Intent::None, "outside the window nothing happens");

        // Yesterday's predictor was up; today's own decides.
        let down = day_bars(99.58);
        let it = intent_at(&down, &params, i, None);
        let Intent::Enter { side: Side::Short, stop: Some(stop), .. } = it else { panic!("{it:?}") };
        assert!((stop - 102.0).abs() < 1e-9, "one mean daily range above the close: {stop}");

        assert_eq!(intent_at(&day_bars(100.0), &params, i, None), Intent::None, "a zero return has no sign");
    }

    #[test]
    fn a_day_with_no_bars_in_the_predictor_window_gives_no_trade() {
        let params = IntradayMomentum.default_params();
        // Drop today's 08:30 and 08:45 bars: the first bar at or after 08:30
        // is now 09:00 and the last before 09:00 is 08:15, a hole. Every
        // earlier day still has an up window, and none of it may stand in.
        let mut bars = day_bars(100.42);
        bars.retain(|b| b.time != at(8, 30) && b.time != at(8, 45));
        assert_eq!(intent_at(&bars, &params, index_at(&bars, 15, 15), None), Intent::None);
        // Nothing before 09:00 at all — the day started late.
        bars.retain(|b| b.time < day(21) || b.time >= at(9, 0));
        assert_eq!(intent_at(&bars, &params, index_at(&bars, 15, 15), None), Intent::None);
    }

    #[test]
    fn the_exit_fires_on_the_first_bar_at_or_after_the_window_end() {
        let params = IntradayMomentum.default_params();
        let bars = day_bars(100.42);
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: at(15, 30), stop: Some(98.0), target: None };
        assert_eq!(intent_at(&bars, &params, index_at(&bars, 16, 15), Some(open)), Intent::None);
        assert!(matches!(intent_at(&bars, &params, index_at(&bars, 16, 30), Some(open)), Intent::Exit { .. }));
        assert!(
            matches!(intent_at(&bars, &params, index_at(&bars, 16, 45), Some(open)), Intent::Exit { .. }),
            "still outside: the exit stands until the engine fills it"
        );
        assert_eq!(intent_at(&bars, &params, index_at(&bars, 16, 45), None), Intent::None, "outside the window and flat: nothing");
    }

    #[test]
    fn the_signal_on_bar_i_does_not_change_when_later_bars_change() {
        let params = IntradayMomentum.default_params();
        let bars = day_bars(100.42);
        let i = index_at(&bars, 15, 15);
        let before = intent_at(&bars, &params, i, None);
        assert!(matches!(before, Intent::Enter { .. }));
        let mut altered = bars.clone();
        for b in &mut altered[i + 1..] {
            b.open = 50.0;
            b.high = 60.0;
            b.low = 40.0;
            b.close = 45.0;
        }
        assert_eq!(intent_at(&altered, &params, i, None), before, "the future must not be readable");
        assert_eq!(intent_at(&bars[..=i], &params, i, None), before, "the slice cut at i is the same world");
    }

    #[test]
    fn a_predictor_move_smaller_than_min_move_daily_ranges_is_no_trade() {
        // A $0.42 move against a $2 mean daily range.
        let bars = day_bars(100.42);
        let i = index_at(&bars, 15, 15);
        let mut params = IntradayMomentum.default_params();
        params.set("minMove", 0.25); // $0.50
        assert_eq!(intent_at(&bars, &params, i, None), Intent::None);
        params.set("minMove", 0.2); // $0.40
        assert!(matches!(intent_at(&bars, &params, i, None), Intent::Enter { side: Side::Long, .. }));
    }
}
