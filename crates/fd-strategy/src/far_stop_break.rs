//! A dull channel break whose invalidation level is structural, not a fixed
//! distance — the instrument that measures the cost term.
//!
//! The signal is the oldest one there is and is deliberately ordinary: a close
//! beyond the extreme of the previous `period` bars. What varies is **where the
//! trade is wrong**, and only that:
//!
//! * `stopMode = 0` — the **opposite edge of the same channel**. The level at
//!   which a break is simply undone and the range is intact again. It is far
//!   from the entry by construction, because it is the whole width of the
//!   range that was broken.
//! * `stopMode = 1` — `stopAtr` x ATR, which is what every method in this
//!   registry does and therefore the control.
//!
//! Both arms are this one `on_bar`, so a difference between them is the stop
//! rule and nothing else. That is the point: `cost/R = spread/stop`, so the
//! bar a method has to clear is set by how far its invalidation sits from its
//! entry, and the two arms differ by a measured factor of three to seven on
//! exactly that ratio while sharing a signal.
//!
//! `minStopPoints` states the same condition in cost units rather than price
//! units: refuse the entry unless the invalidation is at least that many
//! points away. Set to K x the market's configured spread it reads "only trade
//! when the spread is at most 1/K of the risk". Zero is off.
//!
//! `stopMode` is deliberately **not** on the grid. A sweep that could choose
//! between two stop rules would be selecting the mechanism, and the contrast
//! this file exists to measure would become a search over it.
//!
//! Design note: `docs/research/designs/2026-09-23-designed-1-far-stop-break.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct FarStopBreak;

/// Slots of [`Strategy::series`], in the order `on_bar` indexes them.
const UPPER: usize = 0;
const LOWER: usize = 1;
const ATR: usize = 2;

impl Strategy for FarStopBreak {
    fn id(&self) -> &'static str {
        "far-stop-break"
    }
    fn name(&self) -> &'static str {
        "Channel break, structural invalidation"
    }
    fn description(&self) -> &'static str {
        "Buy a close above the prior N-bar high, sell a close below the prior N-bar low, and place the stop at \
         the OPPOSITE edge of that same channel (stopMode 0) or at stopAtr x ATR (stopMode 1). The signal is \
         ordinary on purpose; the stop distance is the variable under test."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("period", 20.0),
            ("atrPeriod", 14.0),
            // 0 = the opposite edge of the channel; 1 = stopAtr x ATR.
            ("stopMode", 0.0),
            ("stopAtr", 1.2),
            ("riskReward", 1.0),
            // Points of invalidation distance below which the entry is refused.
            // Zero is off. K x spread expresses the cost condition directly.
            ("minStopPoints", 0.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("period".to_string(), vec![20.0, 40.0, 80.0]),
            ("riskReward".to_string(), vec![1.0, 1.8]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("donchian").with("period", p.get("period")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let base = format!("donchian_{}", whole(p.get("period")));
        vec![format!("{base}.upper"), format!("{base}.lower"), format!("atr_{}.atr", whole(p.get("atrPeriod")))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period").max(p.period("atrPeriod")) + 5
    }
    fn exits(&self) -> Exits {
        // The engine's stop, target and maximum hold. The whole claim is about
        // where the engine's stop sits, so the engine has to be the one
        // holding it.
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let (upper, lower, atr) = (ctx.s(UPPER), ctx.s(LOWER), ctx.s(ATR));
        if ![upper, lower, atr].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }
        let close = ctx.bar.close;
        let side = if close > upper {
            Side::Long
        } else if close < lower {
            Side::Short
        } else {
            return Intent::None;
        };

        // The one branch in this file. `structural` is the far edge of the
        // channel that was just broken; the ATR arm is the registry's habit.
        let structural = p.get("stopMode") < 0.5;
        let stop = if structural {
            if side.is_long() { lower } else { upper }
        } else if side.is_long() {
            close - atr * p.get("stopAtr")
        } else {
            close + atr * p.get("stopAtr")
        };

        let risk = if side.is_long() { close - stop } else { stop - close };
        // NaN and a zero-width channel both refuse the trade rather than size
        // on a guess; the comparison is negated so NaN falls through it.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(risk > 0.0) {
            return Intent::None;
        }
        let floor = p.get("minStopPoints");
        if floor.is_finite() && risk < floor {
            return Intent::None;
        }
        let reward = risk * p.get("riskReward");
        let target = if side.is_long() { close + reward } else { close - reward };

        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: if structural {
                format!("broke the {}-bar channel; wrong at its far edge, {risk:.2} points away", p.period("period"))
            } else {
                format!("broke the {}-bar channel; wrong {risk:.2} points away ({}x ATR)", p.period("period"), p.get("stopAtr"))
            },
        }
    }
}

/// A whole-number parameter without a decimal point, the way the indicator
/// crate builds its keys. A mismatch here misses the series silently.
fn whole(value: f64) -> String {
    if value.fract() == 0.0 { format!("{}", value as i64) } else { format!("{value}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;
    use fd_indicators::compute_indicators;

    /// A channel of 20 bars between 2400 and 2410, then a close above it.
    fn bars() -> Vec<Bar> {
        let mut v: Vec<Bar> = (0..21)
            .map(|k| Bar { time: k * 900_000, open: 2405.0, high: 2410.0, low: 2400.0, close: 2405.0, volume: None })
            .collect();
        v[20].close = 2412.0;
        v[20].high = 2413.0;
        v
    }

    fn intent(p: &Params, i: usize, v: &[Bar]) -> Intent {
        let upper = vec![2410.0; v.len()];
        let lower = vec![2400.0; v.len()];
        let atr = vec![2.0; v.len()];
        let series: [&[f64]; 3] = [&upper, &lower, &atr];
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext { bar: &v[i], i, bars: v, ind: &ind, series: &series, options: None, position: None, params: p };
        FarStopBreak.on_bar(&ctx)
    }

    #[test]
    fn the_structural_stop_is_the_far_edge_of_the_channel_that_broke() {
        let v = bars();
        let p = FarStopBreak.default_params(); // stopMode 0, riskReward 1
        let it = intent(&p, 20, &v);
        let Intent::Enter { side, stop, target, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Long);
        assert!((stop.unwrap() - 2400.0).abs() < 1e-9, "stop {stop:?}");
        // Risk is the whole 12-point channel-plus-break, not an ATR distance.
        assert!((target.unwrap() - 2424.0).abs() < 1e-9, "target {target:?}");
        assert_eq!(intent(&p, 19, &v), Intent::None, "inside the channel there is no trade");
    }

    #[test]
    fn the_atr_arm_is_the_same_signal_with_a_near_invalidation() {
        let v = bars();
        let mut p = FarStopBreak.default_params();
        p.set("stopMode", 1.0);
        let Intent::Enter { stop, .. } = intent(&p, 20, &v) else { panic!("expected an entry") };
        // 2412 - 1.2 * 2.0: 2.4 points of risk against the structural 12.
        assert!((stop.unwrap() - 2409.6).abs() < 1e-9, "stop {stop:?}");
    }

    #[test]
    fn an_invalidation_closer_than_the_floor_is_refused_rather_than_sized() {
        let v = bars();
        let mut p = FarStopBreak.default_params();
        p.set("stopMode", 1.0);
        // 2.4 points of risk against a floor of 10 x the 0.28 spread.
        p.set("minStopPoints", 2.8);
        assert_eq!(intent(&p, 20, &v), Intent::None);
        p.set("minStopPoints", 2.0);
        assert!(matches!(intent(&p, 20, &v), Intent::Enter { .. }), "above the floor it trades");
    }

    /// The property required of every method in this program: the decision on
    /// bar `i` reads no bar after `i`. Tested the way `fd-indicators` tests
    /// it — the truncated run must be a prefix of the full one, through the
    /// real indicator computation and not a hand-made series.
    #[test]
    fn the_decision_on_a_bar_reads_no_later_bar() {
        // A wavy series with a real trend and real channels in it.
        let full: Vec<Bar> = (0..400)
            .map(|k| {
                let x = k as f64;
                let mid = 2400.0 + x * 0.05 + (x / 9.0).sin() * 14.0 + (x / 31.0).cos() * 22.0;
                Bar { time: k * 900_000, open: mid, high: mid + 2.5, low: mid - 2.5, close: mid + (x / 5.0).sin(), volume: None }
            })
            .collect();
        let cut = 280;

        for mode in [0.0, 1.0] {
            let mut p = FarStopBreak.default_params();
            p.set("stopMode", mode);
            let specs = FarStopBreak.indicators(&p);
            let keys = FarStopBreak.series(&p);

            let run = |bars: &[Bar], upto: usize| -> Vec<Intent> {
                let set = compute_indicators(bars, &specs).expect("indicators");
                let series: Vec<&[f64]> = keys.iter().map(|k| &set.get(k).expect("series")[..]).collect();
                (0..upto)
                    .map(|i| {
                        let ctx = BarContext {
                            bar: &bars[i],
                            i,
                            bars,
                            ind: &set,
                            series: &series,
                            options: None,
                            position: None,
                            params: &p,
                        };
                        FarStopBreak.on_bar(&ctx)
                    })
                    .collect()
            };

            let whole_run = run(&full, cut);
            let truncated = run(&full[..cut], cut);
            assert_eq!(
                whole_run, truncated,
                "stopMode {mode}: the decisions over the first {cut} bars changed when later bars were added"
            );
            assert!(
                whole_run.iter().any(|i| matches!(i, Intent::Enter { .. })),
                "stopMode {mode}: the test series produced no entry at all, so it proves nothing"
            );
        }
    }
}
