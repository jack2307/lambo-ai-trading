//! Previous-day high and low.
//!
//! Yesterday's extremes — the last completed New York day that had bars —
//! are where resting orders sit. Two opposite readings of a touch, chosen by
//! `mode`:
//!
//! * **fade** (`mode = 0`): a bar trades through the level and *closes back
//!   inside* the day's range — the touch was absorbed. Sell at the high /
//!   buy at the low, stop beyond the bar's extreme plus a buffer, target a
//!   multiple of the risk.
//! * **break** (`mode = 1`): a bar *closes* beyond the level — the orders
//!   were taken out. Buy above the high / sell below the low, stop at the
//!   level minus a buffer.
//!
//! One trade per level per day: the first qualifying bar at each of the two
//! levels is the signal; later touches of the same level that day are not.
//! Session gating (Asia for the fade, New York morning for the break) is the
//! batch's job, through [`crate::filter::Filtered`], so the null is gated the
//! same way.
//!
//! Stateless: yesterday's range and today's earlier touches are rebuilt from
//! the closed bars on every call.
//!
//! Spec: `docs/hypotheses/2026-09-13-pdhl.md`.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

pub struct PreviousDayLevels;

impl Strategy for PreviousDayLevels {
    fn id(&self) -> &'static str {
        "pdhl"
    }
    fn name(&self) -> &'static str {
        "Previous-day high/low"
    }
    fn description(&self) -> &'static str {
        "Yesterday's high and low as levels: fade the first touch that closes back inside (mode 0), or trade the \
         first close through (mode 1). Stop beyond the extreme plus a buffer; target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("mode", 0.0),
            ("bufferPips", 3.0),
            ("riskReward", 1.5),
            ("maxRiskAtr", 3.0),
            ("atrPeriod", 14.0),
            ("pipSize", 0.1),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("riskReward".to_string(), vec![1.0, 1.5, 2.0]),
            ("bufferPips".to_string(), vec![1.0, 3.0, 6.0]),
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
        let buffer = p.get("bufferPips") * pip;
        let fade = p.get("mode") < 0.5;
        let bars = &ctx.bars[..=ctx.i];
        let bar = ctx.bar;
        let today = ny_day(bar.time);

        // Yesterday = the most recent completed New York day with bars.
        let Some((pdh, pdl)) = previous_day_range(bars, today) else { return Intent::None };

        // Which level, if any, this bar qualifies at.
        let signal = |b: &Bar| -> Option<(Side, f64, f64)> {
            // (side, level, stop)
            if fade {
                if b.high > pdh && b.close < pdh {
                    return Some((Side::Short, pdh, b.high + buffer));
                }
                if b.low < pdl && b.close > pdl {
                    return Some((Side::Long, pdl, b.low - buffer));
                }
            } else {
                if b.close > pdh {
                    return Some((Side::Long, pdh, pdh - buffer));
                }
                if b.close < pdl {
                    return Some((Side::Short, pdl, pdl + buffer));
                }
            }
            None
        };
        let Some((side, level, stop)) = signal(bar) else { return Intent::None };

        // First touch of this level today only.
        for b in bars[..ctx.i].iter().rev() {
            if ny_day(b.time) != today {
                break;
            }
            if let Some((_, earlier_level, _)) = signal(b)
                && earlier_level == level
            {
                return Intent::None;
            }
        }

        let risk = (bar.close - stop).abs();
        let atr = ctx.s(0);
        if risk <= 0.0 || (atr.is_finite() && risk > p.get("maxRiskAtr") * atr) {
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
                "{} yesterday's {} at {:.2}",
                if fade { "faded" } else { "broke" },
                if level == pdh { "high" } else { "low" },
                level
            ),
        }
    }
}

/// High and low of the last completed New York day before `today` that has
/// bars, scanning back through the closed bars.
fn previous_day_range(bars: &[Bar], today: i64) -> Option<(f64, f64)> {
    let mut day = None;
    let (mut high, mut low) = (f64::NEG_INFINITY, f64::INFINITY);
    for b in bars.iter().rev() {
        let d = ny_day(b.time);
        if d >= today {
            continue;
        }
        match day {
            None => day = Some(d),
            Some(current) if d != current => break,
            _ => {}
        }
        high = high.max(b.high);
        low = low.min(b.low);
    }
    day.map(|_| (high, low))
}

fn ny_day(utc_ms: i64) -> i64 {
    (utc_ms + new_york_offset_ms(utc_ms)).div_euclid(DAY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::clock::days_from_civil;
    use fd_indicators::compute_indicators;

    const MIN: i64 = 60_000;

    /// Two New York summer days of five-minute bars, 09:00–16:00, flat at
    /// 4400 with yesterday's high 4420 (at 11:00) and low 4380 (at 14:00).
    fn two_days() -> (Vec<Bar>, usize) {
        let mut bars = Vec::new();
        for (dom, is_today) in [(14u32, false), (15u32, true)] {
            let base = days_from_civil(2026, 9, dom) * DAY_MS + 13 * 3_600_000; // 09:00 NY
            for k in 0..84 {
                let mut b = Bar { time: base + k * 5 * MIN, open: 4400.0, high: 4401.0, low: 4399.0, close: 4400.0, volume: None };
                if !is_today && k == 24 {
                    b.high = 4420.0; // 11:00
                }
                if !is_today && k == 60 {
                    b.low = 4380.0; // 14:00
                }
                bars.push(b);
            }
        }
        (bars, 84)
    }

    fn intent_at(bars: &[Bar], params: &Params, i: usize) -> Intent {
        let s = PreviousDayLevels;
        let ind = compute_indicators(bars, &s.indicators(params)).unwrap();
        let keys = s.series(params);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &series, options: None, position: None, params };
        s.on_bar(&ctx)
    }

    fn params(mode: f64) -> Params {
        let mut p = PreviousDayLevels.default_params();
        p.set("mode", mode);
        p.set("maxRiskAtr", 100.0); // synthetic ATR is tiny; the gate has its own test
        p
    }

    #[test]
    fn the_fade_sells_the_first_touch_of_yesterday_s_high_that_closes_back_inside() {
        let (mut bars, today) = two_days();
        // 10:30 today: trades through 4420, closes at 4415.
        let i = today + 18;
        bars[i].high = 4423.0;
        bars[i].close = 4415.0;
        let intent = intent_at(&bars, &params(0.0), i);
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Short);
        assert!((stop.unwrap() - 4423.3).abs() < 1e-9, "stop above the touch bar's high plus 3 pips");
        let risk = 4423.3 - 4415.0;
        assert!((target.unwrap() - (4415.0 - 1.5 * risk)).abs() < 1e-9);

        // A second touch of the same level later today is not a signal;
        // a first touch of the LOW still is.
        bars[i + 6].high = 4422.0;
        bars[i + 6].close = 4416.0;
        assert_eq!(intent_at(&bars, &params(0.0), i + 6), Intent::None);
        bars[i + 8].low = 4378.0;
        bars[i + 8].close = 4383.0;
        assert!(matches!(intent_at(&bars, &params(0.0), i + 8), Intent::Enter { side: Side::Long, .. }));
    }

    #[test]
    fn a_close_through_the_level_is_a_break_not_a_fade() {
        let (mut bars, today) = two_days();
        let i = today + 18;
        bars[i].high = 4425.0;
        bars[i].close = 4424.0;
        assert_eq!(intent_at(&bars, &params(0.0), i), Intent::None, "fade mode needs a close back inside");
        let intent = intent_at(&bars, &params(1.0), i);
        let Intent::Enter { side, stop, .. } = intent else { panic!("break mode should enter, got {intent:?}") };
        assert_eq!(side, Side::Long);
        assert!((stop.unwrap() - (4420.0 - 0.3)).abs() < 1e-9, "stop just under the broken level");
    }

    #[test]
    fn yesterday_is_the_last_day_with_bars_and_today_s_bars_never_count() {
        let (bars, today) = two_days();
        // Today's own high so far must not be "yesterday's": push a big high
        // early today, then check the level used at a later bar is still 4420.
        let mut b = bars.clone();
        // A bar that closes above the level is not a fade touch, so it does
        // not use up today's one signal at that level.
        b[today + 2].high = 4450.0;
        b[today + 2].close = 4450.0;
        b[today + 18].high = 4423.0;
        b[today + 18].close = 4415.0;
        assert!(matches!(intent_at(&b, &params(0.0), today + 18), Intent::Enter { side: Side::Short, .. }));
        // Without any earlier day there is nothing to touch.
        assert_eq!(intent_at(&bars[..today], &params(0.0), 40), Intent::None);
    }

    #[test]
    fn the_signal_on_bar_i_does_not_change_when_later_bars_change() {
        let (mut bars, today) = two_days();
        let i = today + 18;
        bars[i].high = 4423.0;
        bars[i].close = 4415.0;
        let before = intent_at(&bars, &params(0.0), i);
        let mut altered = bars.clone();
        for b in &mut altered[i + 1..] {
            b.high = 4470.0;
            b.close = 4460.0;
        }
        assert_eq!(intent_at(&altered, &params(0.0), i), before);
    }
}
