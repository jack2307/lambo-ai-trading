//! Time-series momentum, rebalanced daily.
//!
//! The one anomaly with evidence across every asset class (Moskowitz, Ooi &
//! Pedersen, 2012): the sign of an asset's own past 1–12 month return
//! predicts the sign of its next month's. Here: at the first bar of each
//! New York day, hold the side of the trailing `lookbackDays` return; flip
//! when the sign flips, otherwise stay. Multi-day holds, no stop of its own
//! (`Exits::Strategy`), so it needs a swap-free account or the swap in the
//! cost model — this project's is swap-free.
//!
//! The trailing return is read from the bars themselves: the close
//! `lookbackDays` New York days ago, found by scanning back, so no daily
//! series is needed and nothing after `i` is read.
//!
//! Spec: `docs/hypotheses/2026-09-13-tsmom.md`.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

pub struct TimeSeriesMomentum;

impl Strategy for TimeSeriesMomentum {
    fn id(&self) -> &'static str {
        "tsmom"
    }
    fn name(&self) -> &'static str {
        "Time-series momentum"
    }
    fn description(&self) -> &'static str {
        "Hold the side of the trailing N-day return, rebalanced at the first bar of each New York day."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("lookbackDays", 60.0), ("rebalanceHHMM", 1800.0), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([("lookbackDays".to_string(), vec![20.0, 60.0, 120.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // Enough five-minute bars for the longest lookback plus slack; on
        // coarser bars this is generous, on finer ones the scan itself
        // returns None until the history exists.
        (p.period("lookbackDays") + 5) * 288
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let p = ctx.params;
        let bars = &ctx.bars[..=ctx.i];
        let bar = ctx.bar;
        let (day, minute) = ny_day_minute(bar.time);
        let rebalance = hhmm(p.get("rebalanceHHMM"));
        // Rebalance on the first bar at or after the rebalance minute of each day.
        let is_first = minute >= rebalance
            && ctx.prev().is_none_or(|b| {
                let (pd, pm) = ny_day_minute(b.time);
                pd != day || pm < rebalance
            });
        if !is_first {
            return Intent::None;
        }
        let lookback = p.period("lookbackDays") as i64;
        // The close of the last bar on or before the same minute `lookback`
        // days earlier: scan back through the bars.
        let target_day = day - lookback;
        let Some(past) = bars.iter().rev().find(|b| ny_day_minute(b.time).0 <= target_day) else {
            return Intent::None;
        };
        let ret = bar.close / past.close - 1.0;
        if ret == 0.0 {
            return Intent::None;
        }
        let want = if ret > 0.0 { Side::Long } else { Side::Short };
        match ctx.position {
            Some(open) if open.side == want => Intent::None,
            Some(_) => Intent::Exit { reason: format!("{lookback}-day return flipped") },
            None => Intent::Enter {
                side: want,
                stop: None,
                target: None,
                reason: format!("{lookback}-day return {:+.1}%", ret * 100.0),
            },
        }
    }
}

fn hhmm(v: f64) -> u32 {
    let v = v.round().max(0.0) as u32;
    (v / 100) * 60 + v % 100
}

fn ny_day_minute(utc_ms: i64) -> (i64, u32) {
    let local = utc_ms + new_york_offset_ms(utc_ms);
    (local.div_euclid(DAY_MS), (local.rem_euclid(DAY_MS) / 60_000) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_core::clock::days_from_civil;
    use fd_core::types::Bar;

    /// Daily bars at 18:00 New York (22:00 UTC in summer), rising 1% a day.
    fn rising(days: i64) -> Vec<Bar> {
        (0..days)
            .map(|d| {
                let t = (days_from_civil(2026, 6, 1) + d) * DAY_MS + 22 * 3_600_000;
                Bar::flat(t, 100.0 * 1.01f64.powi(d as i32))
            })
            .collect()
    }

    fn intent(bars: &[Bar], i: usize, position: Option<OpenPosition>) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let mut params = TimeSeriesMomentum.default_params();
        params.set("lookbackDays", 20.0);
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[], options: None, position, params: &params };
        TimeSeriesMomentum.on_bar(&ctx)
    }

    #[test]
    fn holds_the_side_of_the_trailing_return_and_flips_when_it_flips() {
        let bars = rising(40);
        assert_eq!(intent(&bars, 10, None), Intent::None, "no 20-day history yet");
        assert!(matches!(intent(&bars, 30, None), Intent::Enter { side: Side::Long, .. }));
        let long = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        assert_eq!(intent(&bars, 31, Some(long)), Intent::None, "same side: stay");
        let short = OpenPosition { side: Side::Short, ..long };
        assert!(matches!(intent(&bars, 31, Some(short)), Intent::Exit { .. }), "wrong side: flip");
    }

    #[test]
    fn only_the_first_bar_of_the_day_rebalances() {
        let mut bars = rising(40);
        // Add a second bar on day 30, ten minutes later.
        let extra = Bar::flat(bars[30].time + 10 * 60_000, bars[30].close);
        bars.insert(31, extra);
        assert!(matches!(intent(&bars, 30, None), Intent::Enter { .. }));
        assert_eq!(intent(&bars, 31, None), Intent::None);
    }
}
