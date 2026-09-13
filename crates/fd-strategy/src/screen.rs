//! Five indicator mechanisms for the recent-year screen.
//!
//! The owner's criterion for this program is the last twelve months at
//! Vantage (`docs/hypotheses/2026-09-13-recent-year-screen.md`); these are
//! the textbook mechanisms the registry did not yet have, each one a
//! different reading of the same bars:
//!
//! - **Keltner break** — a close outside the Keltner channel goes with it.
//! - **MACD cross** — the MACD line crossing its signal, histogram agreeing.
//! - **RSI(2) pullback** — Connors' rule: in a trend (close vs SMA200), an
//!   extreme two-bar RSI is bought or sold back toward the trend.
//! - **Squeeze break** — Bollinger width at a lookback low, then a close
//!   outside the bands.
//! - **Stochastic reversal** — %K crossing %D from an extreme.
//!
//! All are `Exits::Engine`: a stop of `stopAtr` ATRs (or a structural level),
//! a target of `riskReward` × the risk. Nothing after bar `i` is read; every
//! signal compares the bar's own series values with the previous bar's.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

fn key(id: &str, values: &[f64]) -> String {
    let joined = values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join("_");
    format!("{id}_{joined}")
}

fn grid(entries: &[(&str, &[f64])]) -> BTreeMap<String, Vec<f64>> {
    entries.iter().map(|(k, v)| ((*k).to_string(), v.to_vec())).collect()
}

/// An engine-exit entry: stop at `stop`, target `rr` × the risk from `close`.
fn enter(side: Side, close: f64, stop: f64, rr: f64, reason: String) -> Intent {
    let risk = (close - stop).abs();
    if risk.is_nan() || risk <= 0.0 {
        return Intent::None;
    }
    let target = if side.is_long() { close + risk * rr } else { close - risk * rr };
    Intent::Enter { side, stop: Some(stop), target: Some(target), reason }
}

fn atr_stop(side: Side, close: f64, atr: f64, mult: f64) -> f64 {
    if side.is_long() { close - atr * mult } else { close + atr * mult }
}

/* ---------------- Keltner break ---------------- */

pub struct KeltnerBreak;

impl Strategy for KeltnerBreak {
    fn id(&self) -> &'static str {
        "keltner-break"
    }
    fn name(&self) -> &'static str {
        "Keltner break"
    }
    fn description(&self) -> &'static str {
        "Go with the first close outside the Keltner channel; stop at the channel's middle, target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("period", 20.0), ("kAtr", 10.0), ("mult", 1.5), ("riskReward", 1.5), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("mult", &[1.5, 2.0, 2.5]), ("riskReward", &[1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("keltner").with("period", p.get("period")).with("atrPeriod", p.get("kAtr")).with("mult", p.get("mult")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("keltner", &[p.get("period"), p.get("kAtr"), p.get("mult")]);
        vec![format!("{base}.upper"), format!("{base}.lower"), format!("{base}.middle"), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period").max(p.period("kAtr")) + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let Some(prev) = ctx.prev() else { return Intent::None };
        let (upper, lower, middle) = (ctx.s(0), ctx.s(1), ctx.s(2));
        let (upper_prev, lower_prev) = (ctx.s_back(0, 1), ctx.s_back(1, 1));
        if ![upper, lower, middle, upper_prev, lower_prev].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }
        let close = ctx.bar.close;
        let rr = ctx.params.get("riskReward");
        if close > upper && prev.close <= upper_prev {
            return enter(Side::Long, close, middle, rr, "close above the Keltner channel".into());
        }
        if close < lower && prev.close >= lower_prev {
            return enter(Side::Short, close, middle, rr, "close below the Keltner channel".into());
        }
        Intent::None
    }
}

/* ---------------- MACD cross ---------------- */

pub struct MacdCross;

impl Strategy for MacdCross {
    fn id(&self) -> &'static str {
        "macd-cross"
    }
    fn name(&self) -> &'static str {
        "MACD cross"
    }
    fn description(&self) -> &'static str {
        "Buy the MACD line crossing above its signal, sell it crossing below; ATR stop, target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("fast", 12.0), ("slow", 26.0), ("signal", 9.0), ("stopAtr", 1.5), ("riskReward", 1.5), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("fast", &[8.0, 12.0, 16.0]), ("riskReward", &[1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("macd").with("fast", p.get("fast")).with("slow", p.get("slow")).with("signal", p.get("signal")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("macd", &[p.get("fast"), p.get("slow"), p.get("signal")]);
        vec![format!("{base}.macd"), format!("{base}.signal"), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("slow") + p.period("signal") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let (m, sg, atr) = (ctx.s(0), ctx.s(1), ctx.s(2));
        let (m0, sg0) = (ctx.s_back(0, 1), ctx.s_back(1, 1));
        if ![m, sg, atr, m0, sg0].iter().all(|v| v.is_finite()) || atr <= 0.0 {
            return Intent::None;
        }
        let p = ctx.params;
        let close = ctx.bar.close;
        if m > sg && m0 <= sg0 {
            return enter(Side::Long, close, atr_stop(Side::Long, close, atr, p.get("stopAtr")), p.get("riskReward"), "MACD crossed above its signal".into());
        }
        if m < sg && m0 >= sg0 {
            return enter(Side::Short, close, atr_stop(Side::Short, close, atr, p.get("stopAtr")), p.get("riskReward"), "MACD crossed below its signal".into());
        }
        Intent::None
    }
}

/* ---------------- RSI(2) pullback ---------------- */

pub struct Rsi2Pullback;

impl Strategy for Rsi2Pullback {
    fn id(&self) -> &'static str {
        "rsi2-pullback"
    }
    fn name(&self) -> &'static str {
        "RSI(2) pullback"
    }
    fn description(&self) -> &'static str {
        "Above the SMA200, buy a two-bar RSI under the level; below it, sell one over 100 minus the level. ATR stop, target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("level", 10.0), ("trend", 200.0), ("stopAtr", 2.0), ("riskReward", 1.0), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("level", &[5.0, 10.0, 15.0]), ("riskReward", &[0.75, 1.0, 1.5])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("rsi").with("period", 2.0),
            IndicatorSpec::new("sma").with("period", p.get("trend")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![key("rsi", &[2.0]), key("sma", &[p.get("trend")]), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("trend") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let (rsi, trend, atr) = (ctx.s(0), ctx.s(1), ctx.s(2));
        if ![rsi, trend, atr].iter().all(|v| v.is_finite()) || atr <= 0.0 {
            return Intent::None;
        }
        let p = ctx.params;
        let level = p.get("level");
        let close = ctx.bar.close;
        if close > trend && rsi < level {
            return enter(Side::Long, close, atr_stop(Side::Long, close, atr, p.get("stopAtr")), p.get("riskReward"), format!("RSI(2) {rsi:.0} in an uptrend"));
        }
        if close < trend && rsi > 100.0 - level {
            return enter(Side::Short, close, atr_stop(Side::Short, close, atr, p.get("stopAtr")), p.get("riskReward"), format!("RSI(2) {rsi:.0} in a downtrend"));
        }
        Intent::None
    }
}

/* ---------------- Squeeze break ---------------- */

pub struct SqueezeBreak;

impl Strategy for SqueezeBreak {
    fn id(&self) -> &'static str {
        "squeeze-break"
    }
    fn name(&self) -> &'static str {
        "Squeeze break"
    }
    fn description(&self) -> &'static str {
        "When the Bollinger bands are at their narrowest of the lookback, go with the first close outside them; stop at the middle band."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("period", 20.0), ("mult", 2.0), ("lookback", 60.0), ("riskReward", 1.5), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("lookback", &[40.0, 60.0, 100.0]), ("riskReward", &[1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("bbands").with("period", p.get("period")).with("mult", p.get("mult")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("bbands", &[p.get("period"), p.get("mult")]);
        vec![format!("{base}.upper"), format!("{base}.lower"), format!("{base}.middle"), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period") + p.period("lookback") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let (upper, lower, middle) = (ctx.s(0), ctx.s(1), ctx.s(2));
        if ![upper, lower, middle].iter().all(|v| v.is_finite()) || middle <= 0.0 {
            return Intent::None;
        }
        let p = ctx.params;
        let lookback = p.period("lookback");
        if ctx.i < lookback + 1 {
            return Intent::None;
        }
        // The squeeze is judged on the previous bar, so the breaking bar's
        // own width (which widens on the break) does not undo it.
        let width_at = |n: usize| (ctx.s_back(0, n) - ctx.s_back(1, n)) / ctx.s_back(2, n);
        let width_prev = width_at(1);
        let mut narrowest = f64::INFINITY;
        for n in 1..=lookback {
            let w = width_at(n);
            if w.is_finite() {
                narrowest = narrowest.min(w);
            }
        }
        if !width_prev.is_finite() || width_prev > narrowest * 1.05 {
            return Intent::None;
        }
        let close = ctx.bar.close;
        let rr = p.get("riskReward");
        if close > upper {
            return enter(Side::Long, close, middle, rr, "break out of a squeeze, upward".into());
        }
        if close < lower {
            return enter(Side::Short, close, middle, rr, "break out of a squeeze, downward".into());
        }
        Intent::None
    }
}

/* ---------------- Stochastic reversal ---------------- */

pub struct StochReversal;

impl Strategy for StochReversal {
    fn id(&self) -> &'static str {
        "stoch-reversal"
    }
    fn name(&self) -> &'static str {
        "Stochastic reversal"
    }
    fn description(&self) -> &'static str {
        "Buy %K crossing above %D from under the level, sell it crossing below from over 100 minus the level; ATR stop, target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("period", 14.0), ("smoothK", 3.0), ("smoothD", 3.0), ("level", 20.0), ("stopAtr", 1.5), ("riskReward", 1.5), ("atrPeriod", 14.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("level", &[15.0, 20.0, 25.0]), ("riskReward", &[1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("stoch").with("period", p.get("period")).with("smoothK", p.get("smoothK")).with("smoothD", p.get("smoothD")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("stoch", &[p.get("period"), p.get("smoothK"), p.get("smoothD")]);
        vec![format!("{base}.k"), format!("{base}.d"), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period") + p.period("smoothK") + p.period("smoothD") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let (k, d, atr) = (ctx.s(0), ctx.s(1), ctx.s(2));
        let (k0, d0) = (ctx.s_back(0, 1), ctx.s_back(1, 1));
        if ![k, d, atr, k0, d0].iter().all(|v| v.is_finite()) || atr <= 0.0 {
            return Intent::None;
        }
        let p = ctx.params;
        let level = p.get("level");
        let close = ctx.bar.close;
        if k > d && k0 <= d0 && k0 < level {
            return enter(Side::Long, close, atr_stop(Side::Long, close, atr, p.get("stopAtr")), p.get("riskReward"), format!("%K up through %D from {k0:.0}"));
        }
        if k < d && k0 >= d0 && k0 > 100.0 - level {
            return enter(Side::Short, close, atr_stop(Side::Short, close, atr, p.get("stopAtr")), p.get("riskReward"), format!("%K down through %D from {k0:.0}"));
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    fn bars(closes: &[f64]) -> Vec<Bar> {
        closes.iter().enumerate().map(|(i, c)| Bar { time: i as i64 * 60_000, open: *c, high: c + 1.0, low: c - 1.0, close: *c, volume: None }).collect()
    }

    fn run(strategy: &dyn Strategy, bars: &[Bar], series: &[&[f64]], i: usize) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let params = strategy.default_params();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series, options: None, position: None, params: &params };
        strategy.on_bar(&ctx)
    }

    #[test]
    fn keltner_break_goes_with_the_first_close_outside_and_stops_at_the_middle() {
        let b = bars(&[100.0, 100.0, 103.0]);
        let upper = [102.0, 102.0, 102.0];
        let lower = [98.0, 98.0, 98.0];
        let middle = [100.0, 100.0, 100.0];
        let atr = [1.0, 1.0, 1.0];
        let it = run(&KeltnerBreak, &b, &[&upper, &lower, &middle, &atr], 2);
        let Intent::Enter { side, stop, target, .. } = it else { panic!("{it:?}") };
        assert_eq!(side, Side::Long);
        assert_eq!(stop, Some(100.0));
        assert!((target.unwrap() - 107.5).abs() < 1e-9, "1.5R above 103");
        assert_eq!(run(&KeltnerBreak, &b, &[&upper, &lower, &middle, &atr], 1), Intent::None, "inside the channel");
    }

    #[test]
    fn macd_cross_needs_the_cross_on_this_bar() {
        let b = bars(&[100.0, 100.0, 100.0]);
        let macd = [-1.0, -0.2, 0.3];
        let signal = [0.0, 0.0, 0.0];
        let atr = [2.0, 2.0, 2.0];
        let it = run(&MacdCross, &b, &[&macd, &signal, &atr], 2);
        let Intent::Enter { side, stop, .. } = it else { panic!("{it:?}") };
        assert_eq!(side, Side::Long);
        assert_eq!(stop, Some(97.0), "1.5 ATR of 2 under 100");
        assert_eq!(run(&MacdCross, &b, &[&macd, &signal, &atr], 1), Intent::None, "still below");
    }

    #[test]
    fn rsi2_pullback_buys_only_with_the_trend() {
        let b = bars(&[100.0, 100.0]);
        let rsi = [5.0, 5.0];
        let atr = [1.0, 1.0];
        let up = [90.0, 90.0];
        let down = [110.0, 110.0];
        assert!(matches!(run(&Rsi2Pullback, &b, &[&rsi, &up, &atr], 1), Intent::Enter { side: Side::Long, .. }));
        assert_eq!(run(&Rsi2Pullback, &b, &[&rsi, &down, &atr], 1), Intent::None, "oversold below the trend is not bought");
    }

    #[test]
    fn squeeze_break_needs_a_narrow_band_before_the_break() {
        // Bands narrow for the lookback, then a close above the upper band.
        let n = 70;
        let mut closes = vec![100.0; n];
        closes[n - 1] = 103.0;
        let b = bars(&closes);
        let upper = vec![101.0; n];
        let lower = vec![99.0; n];
        let middle = vec![100.0; n];
        let atr = vec![1.0; n];
        let it = run(&SqueezeBreak, &b, &[&upper, &lower, &middle, &atr], n - 1);
        assert!(matches!(it, Intent::Enter { side: Side::Long, stop: Some(s), .. } if (s - 100.0).abs() < 1e-9), "{it:?}");
        // The same break after a wide history: the previous bar was not a squeeze.
        let mut wide = vec![110.0; n];
        wide[n - 2] = 101.0;
        wide[n - 1] = 101.0;
        let mut wide_lower = vec![90.0; n];
        wide_lower[n - 2] = 99.0;
        wide_lower[n - 1] = 99.0;
        assert!(matches!(run(&SqueezeBreak, &b, &[&wide, &wide_lower, &middle, &atr], n - 1), Intent::Enter { .. }), "narrowest of the lookback is the previous bar itself");
        let mut never = vec![101.0; n];
        never[n - 5] = 100.2;
        let mut never_lower = vec![99.0; n];
        never_lower[n - 5] = 99.8;
        assert_eq!(run(&SqueezeBreak, &b, &[&never, &never_lower, &middle, &atr], n - 1), Intent::None, "a narrower bar in the lookback means no squeeze now");
    }

    #[test]
    fn stochastic_reversal_needs_the_cross_from_the_extreme() {
        let b = bars(&[100.0, 100.0, 100.0]);
        let k = [10.0, 12.0, 22.0];
        let d = [15.0, 15.0, 15.0];
        let atr = [1.0, 1.0, 1.0];
        assert!(matches!(run(&StochReversal, &b, &[&k, &d, &atr], 2), Intent::Enter { side: Side::Long, .. }));
        let high = [60.0, 62.0, 72.0];
        let high_d = [65.0, 65.0, 65.0];
        assert_eq!(run(&StochReversal, &b, &[&high, &high_d, &atr], 2), Intent::None, "a cross from the middle is not a reversal");
    }
}
