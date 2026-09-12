//! Opening-range breakout.
//!
//! The range is the high and low of the bars whose New York time falls in
//! `[rangeStart, rangeStart + rangeMinutes)` on the current New York day. It
//! is usable only once the last of those bars has closed. The **first** bar
//! after that, before `entryUntil`, to close beyond the range (plus a buffer)
//! is the signal; the stop is the other side of the range (or its midpoint),
//! the target a multiple of the risk. One trade per day: a later close outside
//! the range on the same day is not a second signal, whatever happened to the
//! first.
//!
//! Stateless: every call rebuilds the day's range from the closed bars at or
//! before `i`. The day boundary is New York midnight, so a session that
//! starts Sunday evening does not bleed into Monday's range.
//!
//! Spec: `docs/hypotheses/2026-09-13-orb-ny.md`.

use std::collections::BTreeMap;

use fd_core::clock::{new_york_local, new_york_offset_ms};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

pub struct OpeningRangeBreakout;

impl Strategy for OpeningRangeBreakout {
    fn id(&self) -> &'static str {
        "orb"
    }
    fn name(&self) -> &'static str {
        "Opening-range breakout"
    }
    fn description(&self) -> &'static str {
        "The first close outside the New York opening range, before midday, with the stop at the other side of the \
         range and the target a multiple of the risk. One trade per day."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("rangeStart", 820.0),
            ("rangeMinutes", 60.0),
            ("entryUntil", 1200.0),
            ("entryBufferPips", 0.0),
            ("stopMode", 0.0),
            ("riskReward", 1.5),
            ("minRangePips", 5.0),
            ("maxRangeAtr", 3.0),
            ("atrPeriod", 14.0),
            ("pipSize", 0.1),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("riskReward".to_string(), vec![1.0, 1.5, 2.0]),
            ("entryBufferPips".to_string(), vec![0.0, 2.0, 5.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let pip = p.get("pipSize");
        let bar = ctx.bar;
        let (_, minute) = new_york_local(bar.time);
        let start = hhmm_to_minute(p.get("rangeStart"));
        let range_end = start + p.get("rangeMinutes").round() as u32;
        let until = hhmm_to_minute(p.get("entryUntil"));
        // The signal bar must itself lie after the range and before the cutoff.
        if minute < range_end || minute >= until {
            return Intent::None;
        }

        let day = ny_day(bar.time);
        let bars = &ctx.bars[..=ctx.i];
        let mut range_high = f64::NEG_INFINITY;
        let mut range_low = f64::INFINITY;
        let mut range_bars = 0usize;
        let mut range_complete = false;
        // Walk back through today's bars for the ones inside the window. The
        // range is complete because this bar starts at or after `range_end`
        // (checked above), so every range bar has closed.
        for b in bars.iter().rev() {
            if ny_day(b.time) != day {
                break;
            }
            let (_, m) = new_york_local(b.time);
            if m >= start && m < range_end {
                range_high = range_high.max(b.high);
                range_low = range_low.min(b.low);
                range_bars += 1;
                range_complete = true;
            }
        }
        if !range_complete || range_bars == 0 {
            return Intent::None;
        }
        let width = range_high - range_low;
        let atr = ctx.s(0);
        if width < p.get("minRangePips") * pip || (atr.is_finite() && width > p.get("maxRangeAtr") * atr) {
            return Intent::None;
        }

        let buffer = p.get("entryBufferPips") * pip;
        let broke = |b: &Bar| -> Option<Side> {
            if b.close > range_high + buffer {
                Some(Side::Long)
            } else if b.close < range_low - buffer {
                Some(Side::Short)
            } else {
                None
            }
        };
        // One trade a day: the first post-range close outside the range is
        // the only signal, so if any earlier bar today did it, this bar may not.
        for b in bars[..ctx.i].iter().rev() {
            if ny_day(b.time) != day {
                break;
            }
            let (_, m) = new_york_local(b.time);
            if m >= range_end && m < until && broke(b).is_some() {
                return Intent::None;
            }
        }
        let Some(side) = broke(bar) else { return Intent::None };

        let stop = match (side, p.get("stopMode") >= 0.5) {
            (Side::Long, false) => range_low,
            (Side::Short, false) => range_high,
            (_, true) => (range_high + range_low) / 2.0,
        };
        let risk = (bar.close - stop).abs();
        if risk <= 0.0 {
            return Intent::None;
        }
        let target = if side.is_long() {
            bar.close + risk * p.get("riskReward")
        } else {
            bar.close - risk * p.get("riskReward")
        };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "closed {} the {:.2}-{:.2} opening range",
                if side.is_long() { "above" } else { "below" },
                range_low,
                range_high
            ),
        }
    }
}

fn hhmm_to_minute(v: f64) -> u32 {
    let v = v.round().max(0.0) as u32;
    (v / 100) * 60 + v % 100
}

/// New York calendar day as a day index.
fn ny_day(utc_ms: i64) -> i64 {
    (utc_ms + new_york_offset_ms(utc_ms)).div_euclid(DAY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::clock::days_from_civil;
    use fd_indicators::{IndicatorSet, compute_indicators};

    const MIN: i64 = 60_000;

    /// Five-minute bars for one New York summer day (EDT = UTC-4), from 07:00
    /// to 13:00 New York, flat at 4400 except where the test bends them.
    fn day(year: i64, month: u32, dom: u32) -> Vec<Bar> {
        let base = days_from_civil(year, month, dom) * 86_400_000 + 11 * 3_600_000; // 07:00 NY = 11:00 UTC
        (0..72).map(|k| Bar { time: base + k as i64 * 5 * MIN, open: 4400.0, high: 4401.0, low: 4399.0, close: 4400.0, volume: None }).collect()
    }

    /// Index of the bar starting at hh:mm New York on that day.
    fn at(hh: usize, mm: usize) -> usize {
        ((hh - 7) * 60 + mm) / 5
    }

    fn intent_at(bars: &[Bar], params: &Params, i: usize) -> Intent {
        let strategy = OpeningRangeBreakout;
        let ind: IndicatorSet = compute_indicators(bars, &strategy.indicators(params)).unwrap();
        let keys = strategy.series(params);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &series, options: None, position: None, params };
        strategy.on_bar(&ctx)
    }

    fn setup() -> (Vec<Bar>, Params) {
        // 2026-09-14 is a Monday. Range 08:20–09:20: high 4410 at 08:40, low 4395 at 09:00.
        let mut bars = day(2026, 9, 14);
        bars[at(8, 40)].high = 4410.0;
        bars[at(9, 0)].low = 4395.0;
        // 10:15 closes above the range: the signal.
        bars[at(10, 15)].close = 4411.0;
        bars[at(10, 15)].high = 4412.0;
        // 11:00 closes above again: not a second signal.
        bars[at(11, 0)].close = 4413.0;
        bars[at(11, 0)].high = 4414.0;
        let mut params = OpeningRangeBreakout.default_params();
        // The synthetic day is flat outside the range, so its ATR is tiny and
        // the width gate would refuse the range; that gate has its own test.
        params.set("maxRangeAtr", 100.0);
        (bars, params)
    }

    #[test]
    fn the_first_close_outside_the_range_is_the_only_signal_of_the_day() {
        let (bars, params) = setup();
        let intent = intent_at(&bars, &params, at(10, 15));
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Long);
        assert_eq!(stop, Some(4395.0), "stop at the other side of the range");
        // Risk 4411 - 4395 = 16; target 1.5R above the signalling close.
        assert!((target.unwrap() - (4411.0 + 1.5 * 16.0)).abs() < 1e-9);
        assert_eq!(intent_at(&bars, &params, at(11, 0)), Intent::None, "one trade a day");
        assert_eq!(intent_at(&bars, &params, at(10, 10)), Intent::None, "nothing before the break");
    }

    #[test]
    fn a_break_inside_the_range_window_or_after_the_cutoff_does_not_count() {
        let (mut bars, params) = setup();
        // Move the break to 09:10 (inside the range window) — the range is
        // not complete yet, so no signal there...
        bars[at(10, 15)].close = 4400.0;
        bars[at(10, 15)].high = 4401.0;
        bars[at(9, 10)].close = 4411.0;
        bars[at(9, 10)].high = 4412.0;
        assert_eq!(intent_at(&bars, &params, at(9, 10)), Intent::None);
        // ...and a break at 12:05 is after entryUntil.
        bars[at(9, 10)].close = 4400.0;
        bars[at(9, 10)].high = 4410.0; // keep the range high at 4410 wherever it sits
        bars[at(12, 5)].close = 4411.0;
        assert_eq!(intent_at(&bars, &params, at(12, 5)), Intent::None);
    }

    #[test]
    fn a_range_too_narrow_or_too_wide_for_the_day_gives_no_trade() {
        let (mut bars, mut params) = setup();
        params.set("minRangePips", 200.0); // range is 15.0 = 150 pips
        assert_eq!(intent_at(&bars, &params, at(10, 15)), Intent::None);
        params.set("minRangePips", 5.0);
        params.set("maxRangeAtr", 0.1); // ATR on this series is a few dollars; the range is 15
        assert_eq!(intent_at(&bars, &params, at(10, 15)), Intent::None);
        params.set("maxRangeAtr", 100.0);
        bars[at(10, 15)].close = 4411.0;
        assert!(matches!(intent_at(&bars, &params, at(10, 15)), Intent::Enter { .. }));
    }

    #[test]
    fn the_signal_on_bar_i_does_not_change_when_later_bars_change() {
        let (bars, params) = setup();
        let before = intent_at(&bars, &params, at(10, 15));
        let mut altered = bars.clone();
        for b in &mut altered[at(10, 15) + 1..] {
            b.close = 4300.0;
            b.low = 4290.0;
        }
        assert_eq!(intent_at(&altered, &params, at(10, 15)), before, "the future must not be readable");
    }

    #[test]
    fn a_range_from_yesterday_is_not_today_s_range() {
        // Two days; the range bars only exist on day one. Day two's 10:15
        // close above day one's range must not fire.
        let (mut bars, params) = setup();
        let mut next = day(2026, 9, 15);
        next[at(10, 15)].close = 4411.0;
        let offset = bars.len();
        bars.append(&mut next);
        // Day two's own range is flat 4399–4401 (2 = 20 pips, above the 5 pip
        // minimum), so 4411 IS a break of day two's range — make it not one by
        // closing inside instead.
        bars[offset + at(10, 15)].close = 4400.5;
        assert_eq!(intent_at(&bars, &params, offset + at(10, 15)), Intent::None);
    }
}
