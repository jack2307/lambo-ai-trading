//! A breakout whose **stop width** comes from a forecast of the next bar's
//! range, so that stop size is the only thing that differs between arms.
//!
//! # Why this exists rather than a parameter on `donchian-breakout`
//!
//! `agent/stop-width` read the code before spending a cell and found that
//! `session_hold` declares [`Exits::Strategy`], and `check_exit`
//! (`fd-backtest/src/engine.rs:772-777`) opens with
//! `if position.self_managed { return None }`. So for every self-managed
//! method the "stop" is **never enforced** — it is only the DENOMINATOR of
//! `R`, and widening it changed zero signals, zero trades and zero dollars
//! (the trade set was identical across all ten widths; `E` and `Lbar` scaled
//! as `1/f`). A stop-width axis on a self-managed method is a units axis.
//!
//! This method is [`Exits::Engine`] and has **no exit rule of its own**: it
//! returns [`Intent::None`] whenever a position is open, so the stop, the
//! target (`reward_risk` x the risk, derived by `open_position`) and
//! `max_hold_ms` are the engine's. `donchian-breakout` could not be used as
//! the base because it exits at the channel midline, which would make the
//! stop one of two things deciding the outcome instead of the only one.
//!
//! # Where the stop width comes from, and the one rule that makes the
//! comparison a comparison
//!
//! The stop distance is
//!
//! ```text
//! d(i) = stopAtr x ATR(atrPeriod)(i) x m(i)
//! ```
//!
//! where `m(i)` is read from the **companion** series `cmp.close` — one
//! number per bar, matched on an **exact** timestamp by
//! `fd_indicators::companion::aligned_change`, `NaN` when the companion does
//! not carry that bar ("missing, not zero and not the previous value"). The
//! companion is installed once by `search --companion=<SYMBOL>`; with none
//! installed every `cmp` series is `NaN` and this method takes **no trades at
//! all**, which is the honest answer and not a trade at a fabricated 1.0.
//!
//! `stopSource` picks what `m(i)` is:
//!
//! * `0` — **`m(i) = 1.0`**, a flat ATR stop. The control.
//! * `1` — **`m(i) = cmp.close(i)`**, the forecast.
//!
//! **And `stopSource = 0` still refuses the bar when `cmp.close(i)` is
//! `NaN`.** It does not read the value; it reads whether the value exists.
//! That is deliberate: 4.9209% of this feed's 15m bars do not have all
//! fifteen 1m bars (18,638 of 378,749, measured by `agent/intrabar-shape`),
//! so a control that traded those bars would be a different sample, and the
//! difference between the two arms would be partly a difference of samples.
//! Both arms trade **exactly the same bars**.
//!
//! # Causality
//!
//! Nothing after bar `i` is read. The channel is the previous `period` bars
//! (`fd_indicators::donchian` windows `bars[i - period..i]`, the current bar
//! excluded), the ATR is warm as of bar `i`, and the companion bar stamped
//! `t(i)` closes at `t(i) + 15m` — the same instant bar `i` closes, which is
//! when the engine takes the decision to the next bar's open. Whether the
//! number in that companion bar was itself built causally is a property of
//! the file, measured in `py/research/range_sizing_precheck.py` (five cut
//! points, bit-equality, plus two deliberately look-ahead probes that must
//! fail that test).

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct RangeStop;

/// Series slots, in the order [`Strategy::series`] declares them.
const S_UPPER: usize = 0;
const S_LOWER: usize = 1;
const S_ATR: usize = 2;
const S_CMP: usize = 3;

impl Strategy for RangeStop {
    fn id(&self) -> &'static str {
        "range-stop"
    }
    fn name(&self) -> &'static str {
        "Breakout, stop width from a range forecast"
    }
    fn description(&self) -> &'static str {
        "Prior-channel breakout with no exit rule of its own; the stop is stopAtr x ATR x the companion's number. Needs `search --companion=<SYMBOL>` and takes no trades without one."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // The registry's own Donchian default, not a fitted level.
            ("period", 20.0),
            ("atrPeriod", 14.0),
            ("stopAtr", 1.0),
            // 0 = flat ATR stop (control), 1 = the companion's forecast.
            ("stopSource", 0.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // Empty on purpose: every cell this method is run in is declared by
        // label in docs/decisions/2026-10-11-range-sizing.md. A grid here
        // would let a sweep spend cells that registration did not count.
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("donchian").with("period", p.get("period")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
            // `period` is the companion's own lookback for `.change`, which
            // this method does not read; 1 is the cheapest legal value.
            IndicatorSpec::new("cmp").with("period", 1.0).with("atrPeriod", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let d = format!("donchian_{}", p.get("period"));
        let cmp = format!("cmp_{}_{}", 1.0, p.get("atrPeriod"));
        vec![
            format!("{d}.upper"),
            format!("{d}.lower"),
            format!("atr_{}.atr", p.get("atrPeriod")),
            format!("{cmp}.close"),
        ]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period").max(p.period("atrPeriod")) + 5
    }
    fn exits(&self) -> Exits {
        // THE declaration this whole axis rests on. With `Exits::Strategy`
        // the stop below would never be enforced and this would be a units
        // sweep; see the module docs.
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        // No exit rule of its own: while a position is open the engine owns
        // it entirely.
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let (upper, lower, atr, cmp) = (ctx.s(S_UPPER), ctx.s(S_LOWER), ctx.s(S_ATR), ctx.s(S_CMP));

        // `cmp` is required by BOTH sources. The control does not read its
        // value and does read its existence, so the two arms trade the same
        // bars; see the module docs.
        if ![upper, lower, atr, cmp].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }
        if !(atr > 0.0) {
            return Intent::None;
        }

        let m = if p.get("stopSource") >= 0.5 { cmp } else { 1.0 };
        if !(m > 0.0) {
            return Intent::None;
        }
        let d = p.get("stopAtr") * atr * m;
        if !(d > 0.0) {
            return Intent::None;
        }

        let close = ctx.bar.close;
        // `target: None` is not "no target": `open_position` derives it as
        // `reward_risk` x the risk, which is the engine's L2 shape. Writing a
        // price here would make it L3 and a different question.
        if close > upper {
            return Intent::Enter {
                side: Side::Long,
                stop: Some(close - d),
                target: None,
                reason: format!("close above the prior {}-bar high", p.period("period")),
            };
        }
        if close < lower {
            return Intent::Enter {
                side: Side::Short,
                stop: Some(close + d),
                target: None,
                reason: format!("close below the prior {}-bar low", p.period("period")),
            };
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_core::types::Bar;

    fn params(source: f64, stop_atr: f64) -> Params {
        Params::new(&[("period", 2.0), ("atrPeriod", 14.0), ("stopAtr", stop_atr), ("stopSource", source)])
    }

    fn bar(close: f64) -> Bar {
        Bar { time: 0, open: close, high: close, low: close, close, volume: None }
    }

    /// Series in the declared order: `upper`, `lower`, `atr`, `cmp.close`.
    fn intent(bars: &[Bar], series: &[&[f64]], p: &Params, position: Option<OpenPosition>) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let i = bars.len() - 1;
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series, options: None, position, params: p };
        RangeStop.on_bar(&ctx)
    }

    #[test]
    fn the_engine_owns_the_exit_and_the_registry_says_so_in_the_type() {
        // If this ever reads `Exits::Strategy`, every number this axis
        // produced is a number about a denominator and not about a stop.
        assert_eq!(RangeStop.exits(), Exits::Engine);
    }

    #[test]
    fn a_missing_companion_bar_refuses_the_trade_in_both_sources() {
        // The property that makes the control and the method one comparison:
        // neither trades a bar the companion does not carry, so neither
        // trades on a fabricated 1.0 either.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [1.0, 1.0, 1.0]);
        let missing = [f64::NAN, f64::NAN, f64::NAN];
        for source in [0.0, 1.0] {
            let p = params(source, 1.0);
            let series: [&[f64]; 4] = [&upper, &lower, &atr, &missing];
            assert!(
                matches!(intent(&bars, &series, &p, None), Intent::None),
                "stopSource {source} traded a bar the companion does not carry"
            );
        }
    }

    #[test]
    fn the_companion_number_scales_the_stop_and_only_the_stop() {
        // Same bars, same channel, same ATR: the entry side is identical and
        // the stop distance is multiplied by the companion's number.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [2.0, 2.0, 2.0]);
        let m = [f64::NAN, f64::NAN, 1.5];
        let series: [&[f64]; 4] = [&upper, &lower, &atr, &m];

        let flat = params(0.0, 1.0);
        let fore = params(1.0, 1.0);
        match (intent(&bars, &series, &flat, None), intent(&bars, &series, &fore, None)) {
            (
                Intent::Enter { side: sa, stop: Some(sta), target: ta, .. },
                Intent::Enter { side: sb, stop: Some(stb), target: tb, .. },
            ) => {
                assert!(sa.is_long() && sb.is_long());
                assert!((sta - 108.0).abs() < 1e-9, "flat stop is 1.0 x ATR 2.0 below 110: {sta}");
                assert!((stb - 107.0).abs() < 1e-9, "forecast stop is 1.5 x that: {stb}");
                // `None` both times: `open_position` derives the target as
                // `reward_risk` x the risk, so it scales with the stop (L2).
                // A price here would make this a different question (L3).
                assert!(ta.is_none() && tb.is_none(), "a price target here would be a different question");
            }
            other => panic!("expected two long entries, got {other:?}"),
        }
    }

    #[test]
    fn it_proposes_nothing_while_a_position_is_open() {
        // No exit rule of its own, so nothing but the stop, the target and
        // the clock can end a trade.
        let bars = [bar(100.0), bar(100.0), bar(80.0)];
        let (upper, lower) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0]);
        let (atr, m) = ([1.0, 1.0, 1.0], [1.0, 1.0, 1.0]);
        let series: [&[f64]; 4] = [&upper, &lower, &atr, &m];
        let p = params(1.0, 1.0);
        let held = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        assert!(matches!(intent(&bars, &series, &p, Some(held)), Intent::None));
        // ... and the same bars with no position open DO produce a short.
        assert!(matches!(intent(&bars, &series, &p, None), Intent::Enter { .. }));
    }
}
