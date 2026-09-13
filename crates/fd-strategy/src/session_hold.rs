//! Hold through a New York time window, every day.
//!
//! A drift claim, not a signal: enter at the window's first bar, exit at its
//! last, one side only, no stop of its own (the engine's maximum hold and
//! ATR stop still apply unless it manages exits — here it does, so the
//! window is the whole rule). Used to test whether a market's return is
//! concentrated in particular hours: the same rule on the complement window
//! and buy-and-hold are the comparisons, and a random-entry null gated to the
//! same hours is the noise floor.
//!
//! **Sizing.** With `riskDailyRanges` > 0 the entry carries a sizing stop of
//! that many mean New York-day ranges (over `rangeDays` days), which the
//! engine uses for lots and R and does not enforce — the clock is the exit.
//! Without it the engine's bar-ATR fallback sizes the hold, which the risk
//! role called "a scaling constant, not a loss limit" for an eight-bar hold.
//!
//! Spec: `docs/hypotheses/2026-09-13-btc-us-hours.md`, `2026-09-13-close-reopen-drift.md`.

use std::collections::BTreeMap;

use fd_core::clock::new_york_local;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct SessionHold;

impl Strategy for SessionHold {
    fn id(&self) -> &'static str {
        "session-hold"
    }
    fn name(&self) -> &'static str {
        "Session hold"
    }
    fn description(&self) -> &'static str {
        "Long (or short) from the first bar of a New York window to its last, every day. A drift test, not a signal."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("from", 930.0), ("to", 1600.0), ("side", 1.0), ("atrPeriod", 14.0), ("riskDailyRanges", 0.0), ("rangeDays", 20.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // A drift has nothing to tune; the walk-forward sees one cell and
        // selection cannot flatter it.
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
        let (_, minute) = new_york_local(ctx.bar.time);
        let from = hhmm(p.get("from"));
        let to = hhmm(p.get("to"));
        let inside = if from <= to { minute >= from && minute < to } else { minute >= from || minute < to };
        match ctx.position {
            Some(_) if !inside => Intent::Exit { reason: "window closed".into() },
            Some(_) => Intent::None,
            None if inside => {
                // Enter only at the window's first bar: a position opened
                // late in the window is a different, shorter hold.
                let previous_inside = ctx.prev().is_some_and(|b| {
                    let (_, m) = new_york_local(b.time);
                    if from <= to { m >= from && m < to } else { m >= from || m < to }
                });
                if previous_inside {
                    return Intent::None;
                }
                let side = if p.get("side") >= 0.0 { Side::Long } else { Side::Short };
                let ranges = p.get("riskDailyRanges");
                let stop = if ranges > 0.0 {
                    let Some(stop) = crate::tsmom::sizing_stop(&ctx.bars[..=ctx.i], ctx.bar, side, ranges, p.period("rangeDays")) else {
                        return Intent::None;
                    };
                    Some(stop)
                } else {
                    None
                };
                Intent::Enter {
                    side,
                    stop,
                    target: None,
                    reason: format!("window {:04}-{:04} New York", p.get("from") as u32, p.get("to") as u32),
                }
            }
            None => Intent::None,
        }
    }
}

fn hhmm(v: f64) -> u32 {
    let v = v.round().max(0.0) as u32;
    (v / 100) * 60 + v % 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_core::clock::days_from_civil;
    use fd_core::types::Bar;

    fn at(hour: i64, minute: i64) -> i64 {
        // 2026-09-14, EDT: New York = UTC - 4.
        days_from_civil(2026, 9, 14) * 86_400_000 + (hour + 4) * 3_600_000 + minute * 60_000
    }

    #[test]
    fn a_daily_range_sizing_stop_rides_with_the_entry_and_nothing_else_changes() {
        // Twenty-one days of one bar each at 09:00 New York with a $2 range,
        // then the 09:25 and 09:30 bars of the last day.
        let day = |d: i64| days_from_civil(2026, 8, 24) * 86_400_000 + d * 86_400_000 + 13 * 3_600_000;
        let mut bars: Vec<Bar> = (0..21).map(|d| Bar { time: day(d), open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: None }).collect();
        bars.push(Bar::flat(day(21) + 25 * 60_000, 100.0));
        bars.push(Bar::flat(day(21) + 30 * 60_000, 100.0));
        let i = bars.len() - 1;
        let ind = fd_indicators::IndicatorSet::new();
        let mut params = SessionHold.default_params();
        params.set("riskDailyRanges", 1.0);
        let ctx = BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: None, params: &params };
        let it = SessionHold.on_bar(&ctx);
        let Intent::Enter { side: Side::Long, stop, target: None, .. } = it else { panic!("{it:?}") };
        assert!((stop.unwrap() - 98.0).abs() < 1e-9, "one mean daily range of $2 below the close: {stop:?}");
        // Off by default: the bare hold carries no stop, as every earlier record ran it.
        assert!(matches!(intent(at(9, 30), Some(at(9, 25)), None), Intent::Enter { stop: None, .. }));
    }

    fn intent(time: i64, prev: Option<i64>, position: Option<OpenPosition>) -> Intent {
        let bars: Vec<Bar> = prev.into_iter().chain([time]).map(|t| Bar::flat(t, 100.0)).collect();
        let i = bars.len() - 1;
        let ind = fd_indicators::IndicatorSet::new();
        let params = SessionHold.default_params();
        let ctx = BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position, params: &params };
        SessionHold.on_bar(&ctx)
    }

    #[test]
    fn enters_on_the_first_bar_of_the_window_and_exits_on_the_first_bar_after() {
        assert!(matches!(intent(at(9, 30), Some(at(9, 25)), None), Intent::Enter { side: Side::Long, .. }));
        assert_eq!(intent(at(9, 35), Some(at(9, 30)), None), Intent::None, "not the first bar");
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: at(9, 30), stop: None, target: None };
        assert_eq!(intent(at(12, 0), Some(at(11, 55)), Some(open)), Intent::None);
        assert!(matches!(intent(at(16, 0), Some(at(15, 55)), Some(open)), Intent::Exit { .. }));
        assert_eq!(intent(at(17, 0), Some(at(16, 55)), None), Intent::None, "outside the window nothing happens");
    }
}
