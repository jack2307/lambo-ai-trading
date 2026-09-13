//! Pullback in a trend.
//!
//! Two horizons: the trend is the slow average's side and slope; the entry
//! is a pullback *against* it on the fast average. Long when price has been
//! above the fast EMA for at least `runBars` bars and then closes below it
//! while still above the slow EMA and the slow EMA is rising; stop under
//! the lowest low of the last `swingBars` bars minus a buffer; target a
//! multiple of the risk. Symmetric for shorts. The direction comes from the
//! slow horizon, the timing from the fast one — the opposite composition to
//! a breakout, which takes direction from the short-term move itself.
//!
//! Spec: `docs/hypotheses/2026-09-13-trend-pullback.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct TrendPullback;

impl Strategy for TrendPullback {
    fn id(&self) -> &'static str {
        "trend-pullback"
    }
    fn name(&self) -> &'static str {
        "Trend pullback"
    }
    fn description(&self) -> &'static str {
        "With the slow EMA rising and price above it, buy the first close back under the fast EMA after a run \
         above it; stop under the recent swing low; target a multiple of the risk. Symmetric for shorts."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("fast", 20.0),
            ("slow", 200.0),
            ("slopeBars", 20.0),
            ("runBars", 3.0),
            ("swingBars", 5.0),
            ("bufferPips", 3.0),
            ("riskReward", 1.5),
            ("atrPeriod", 14.0),
            ("pipSize", 0.1),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([("fast".to_string(), vec![10.0, 20.0, 34.0]), ("riskReward".to_string(), vec![1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("ema").with("period", p.get("fast")),
            IndicatorSpec::new("ema").with("period", p.get("slow")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("ema_{}", p.get("fast")), format!("ema_{}", p.get("slow")), format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("slow") + p.period("slopeBars") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        const FAST: usize = 0;
        const SLOW: usize = 1;
        let p = ctx.params;
        let (fast, slow) = (ctx.s(FAST), ctx.s(SLOW));
        let slow_before = ctx.s_back(SLOW, p.period("slopeBars"));
        if ![fast, slow, slow_before].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }
        let bar = ctx.bar;
        let run = p.period("runBars");
        let swing = p.period("swingBars");
        if ctx.i < run.max(swing) + 1 {
            return Intent::None;
        }
        let pip = p.get("pipSize");
        let buffer = p.get("bufferPips") * pip;

        // Uptrend: slow rising, price above it; the last `run` bars closed
        // above the fast EMA and this one closed below it.
        let uptrend = slow > slow_before && bar.close > slow;
        let ran_above = (1..=run).all(|k| ctx.bars[ctx.i - k].close > ctx.s_back(FAST, k));
        if uptrend && ran_above && bar.close < fast {
            let low = ctx.bars[ctx.i + 1 - swing..=ctx.i].iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
            let stop = low - buffer;
            let risk = bar.close - stop;
            if risk > 0.0 {
                return Intent::Enter {
                    side: Side::Long,
                    stop: Some(stop),
                    target: Some(bar.close + risk * p.get("riskReward")),
                    reason: format!("pullback under EMA{} in a rising EMA{}", p.period("fast"), p.period("slow")),
                };
            }
        }
        let downtrend = slow < slow_before && bar.close < slow;
        let ran_below = (1..=run).all(|k| ctx.bars[ctx.i - k].close < ctx.s_back(FAST, k));
        if downtrend && ran_below && bar.close > fast {
            let high = ctx.bars[ctx.i + 1 - swing..=ctx.i].iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
            let stop = high + buffer;
            let risk = stop - bar.close;
            if risk > 0.0 {
                return Intent::Enter {
                    side: Side::Short,
                    stop: Some(stop),
                    target: Some(bar.close - risk * p.get("riskReward")),
                    reason: format!("pullback over EMA{} in a falling EMA{}", p.period("fast"), p.period("slow")),
                };
            }
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    /// Bars 0..7 above the fast EMA (4405 vs 4400), bar 8 closes below it;
    /// slow EMA 4380 rising from 4370.
    fn scenario() -> (Vec<Bar>, Vec<f64>, Vec<f64>) {
        let bars: Vec<Bar> = (0..9)
            .map(|k| {
                let c = if k == 8 { 4398.0 } else { 4405.0 };
                Bar { time: k * 60_000, open: c, high: c + 2.0, low: c - 2.0, close: c, volume: None }
            })
            .collect();
        let fast = vec![4400.0; 9];
        let mut slow = vec![4380.0; 9];
        slow[0] = 4370.0;
        (bars, fast, slow)
    }

    fn intent(bars: &[Bar], fast: &[f64], slow: &[f64], i: usize, params: &Params) -> Intent {
        let atr = vec![3.0; bars.len()];
        let series: [&[f64]; 3] = [fast, slow, &atr];
        let ind = fd_indicators::IndicatorSet::new();
        let ctx =
            BarContext { bar: &bars[i], i, bars, ind: &ind, series: &series, options: None, position: None, params };
        TrendPullback.on_bar(&ctx)
    }

    #[test]
    fn the_first_close_under_the_fast_ema_in_an_uptrend_buys_with_the_stop_under_the_swing_low() {
        let (bars, fast, slow) = scenario();
        let mut p = TrendPullback.default_params();
        p.set("slopeBars", 8.0);
        let it = intent(&bars, &fast, &slow, 8, &p);
        let Intent::Enter { side, stop, target, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Long);
        // Lowest low of the last five bars is 4396 (bar 8); minus 3 pips.
        assert!((stop.unwrap() - 4395.7).abs() < 1e-9, "stop {stop:?}");
        let risk = 4398.0 - 4395.7;
        assert!((target.unwrap() - (4398.0 + 1.5 * risk)).abs() < 1e-9);
        assert_eq!(intent(&bars, &fast, &slow, 7, &p), Intent::None, "still above the fast EMA");
    }

    #[test]
    fn no_trend_means_no_pullback_trade() {
        let (bars, fast, mut slow) = scenario();
        slow[0] = 4390.0; // slow falling: 4390 -> 4380
        let mut p = TrendPullback.default_params();
        p.set("slopeBars", 8.0);
        assert_eq!(intent(&bars, &fast, &slow, 8, &p), Intent::None);
    }
}
