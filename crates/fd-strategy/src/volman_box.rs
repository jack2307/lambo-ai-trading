//! Bob Volman's three-box measured move, mechanised.
//!
//! The `boxBars` bars before the breakout bar are the consolidation ("box 1").
//! Its height must be small against the ATR — a tall box is a swing, not a
//! rest. The breakout bar closes beyond the box on the side the EMA favours,
//! and not by more than `maxBreakAtr` ATRs: a bar that has already travelled
//! a box is not an entry, it is the move. The stop is the far side of box 1;
//! the target is `boxes` box-heights beyond the near edge (box 2's far edge
//! is one height out, box 3's is two).
//!
//! **No future.** The box is `bars[i - boxBars .. i]`, the breakout bar is
//! `bars[i]`, and the ATR that judges the box is the previous bar's. Nothing
//! at an index above `i` is read.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

fn key(id: &str, values: &[f64]) -> String {
    let joined = values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join("_");
    format!("{id}_{joined}")
}

/// An engine-exit entry with an explicit stop and target; none when the risk
/// is not a positive finite number.
fn enter(side: Side, close: f64, stop: f64, target: f64, reason: String) -> Intent {
    let risk = if side.is_long() { close - stop } else { stop - close };
    if risk.is_nan() || risk <= 0.0 || !target.is_finite() {
        return Intent::None;
    }
    Intent::Enter { side, stop: Some(stop), target: Some(target), reason }
}

pub struct VolmanBox;

impl Strategy for VolmanBox {
    fn id(&self) -> &'static str {
        "volman-box"
    }
    fn name(&self) -> &'static str {
        "Volman box"
    }
    fn description(&self) -> &'static str {
        "A close out of a tight box on the EMA's side; stop at the box's far edge, target a fixed number of box heights beyond the near one."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("boxBars", 20.0),
            ("maxBoxAtr", 1.5),
            ("boxes", 2.0),
            ("emaPeriod", 18.0),
            ("atrPeriod", 14.0),
            ("maxBreakAtr", 1.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        let mut grid = BTreeMap::new();
        grid.insert("boxBars".to_string(), vec![12.0, 20.0, 30.0]);
        grid
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("ema").with("period", p.get("emaPeriod")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![key("ema", &[p.get("emaPeriod")]), key("atr", &[p.get("atrPeriod")])]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("boxBars") + p.period("emaPeriod").max(p.period("atrPeriod")) + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let box_bars = p.period("boxBars");
        if box_bars == 0 {
            return Intent::None;
        }
        let Some(start) = ctx.i.checked_sub(box_bars) else { return Intent::None };
        let ema = ctx.s(0);
        let atr_now = ctx.s(1);
        let atr_prev = ctx.s_back(1, 1);
        if ![ema, atr_now, atr_prev].iter().all(|v| v.is_finite()) || atr_now <= 0.0 || atr_prev <= 0.0 {
            return Intent::None;
        }

        // The box: the bars before this one, this one excluded.
        let (mut hi, mut lo) = (f64::NEG_INFINITY, f64::INFINITY);
        for b in &ctx.bars[start..ctx.i] {
            hi = hi.max(b.high);
            lo = lo.min(b.low);
        }
        let h = hi - lo;
        if !h.is_finite() || h <= 0.0 || h > p.get("maxBoxAtr") * atr_prev {
            return Intent::None;
        }

        let close = ctx.bar.close;
        let boxes = p.get("boxes");
        let max_break = p.get("maxBreakAtr") * atr_now;
        let ema_period = p.period("emaPeriod");
        let ratio = h / atr_prev;

        if close > hi && close > ema && close - hi <= max_break {
            let reason = format!("box {box_bars} bars h={h:.2} ({ratio:.1} ATR) broke up above EMA{ema_period}; target +{boxes} boxes");
            return enter(Side::Long, close, lo, hi + boxes * h, reason);
        }
        if close < lo && close < ema && lo - close <= max_break {
            let reason = format!("box {box_bars} bars h={h:.2} ({ratio:.1} ATR) broke down below EMA{ema_period}; target -{boxes} boxes");
            return enter(Side::Short, close, hi, lo - boxes * h, reason);
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use fd_core::types::Bar;

    const N: usize = 30;
    const BOX_HI: f64 = 101.0;
    const BOX_LO: f64 = 99.0;

    /// `N` bars: a flat box 99–101 throughout, the last bar closing at `close`
    /// with its own range widened to cover it. The box for bar `N - 1` is the
    /// twenty bars before it.
    fn bars(close: f64) -> Vec<Bar> {
        (0..N)
            .map(|i| {
                let t = i as i64 * 60_000;
                if i == N - 1 {
                    Bar { time: t, open: 100.0, high: BOX_HI.max(close) + 0.5, low: BOX_LO.min(close) - 0.5, close, volume: None }
                } else {
                    Bar { time: t, open: 100.0, high: BOX_HI, low: BOX_LO, close: 100.0, volume: None }
                }
            })
            .collect()
    }

    fn run(bars: &[Bar], ema: f64, atr: f64, i: usize) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let params = VolmanBox.default_params();
        let ema_s = vec![ema; bars.len()];
        let atr_s = vec![atr; bars.len()];
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &[&ema_s, &atr_s], options: None, position: None, params: &params };
        VolmanBox.on_bar(&ctx)
    }

    #[test]
    fn a_close_out_of_a_tight_box_on_the_ema_side_enters_with_box_stop_and_measured_target() {
        // Box height 2, ATR 2 → 1.0 ATR box (≤ 1.5). Close 102 is 1 above the
        // edge (≤ 1 ATR) and above an EMA at 100.
        let up = bars(102.0);
        let it = run(&up, 100.0, 2.0, N - 1);
        let Intent::Enter { side, stop, target, reason } = it else { panic!("{it:?}") };
        assert_eq!(side, Side::Long);
        assert_eq!(stop, Some(BOX_LO), "the far side of box 1");
        assert!((target.unwrap() - 105.0).abs() < 1e-9, "box high + 2 × 2: {target:?}");
        assert_eq!(reason, "box 20 bars h=2.00 (1.0 ATR) broke up above EMA18; target +2 boxes");
        assert_eq!(run(&up, 100.0, 2.0, N - 2), Intent::None, "inside the box nothing happens");

        let down = bars(98.0);
        let it = run(&down, 100.0, 2.0, N - 1);
        let Intent::Enter { side, stop, target, reason } = it else { panic!("{it:?}") };
        assert_eq!(side, Side::Short);
        assert_eq!(stop, Some(BOX_HI));
        assert!((target.unwrap() - 95.0).abs() < 1e-9, "box low − 2 × 2: {target:?}");
        assert_eq!(reason, "box 20 bars h=2.00 (1.0 ATR) broke down below EMA18; target -2 boxes");
    }

    #[test]
    fn a_box_taller_than_max_box_atr_is_not_a_consolidation() {
        // Same box, ATR 1: height 2 > 1.5 × 1.
        assert_eq!(run(&bars(102.0), 100.0, 1.0, N - 1), Intent::None);
        assert_eq!(run(&bars(98.0), 100.0, 1.0, N - 1), Intent::None);
    }

    #[test]
    fn a_breakout_that_has_already_travelled_more_than_max_break_atr_is_not_an_entry() {
        // Close 104 is 3 above the edge; 1 × ATR 2 allows 2.
        assert_eq!(run(&bars(104.0), 100.0, 2.0, N - 1), Intent::None);
        assert_eq!(run(&bars(96.0), 100.0, 2.0, N - 1), Intent::None);
        // Exactly at the limit is still allowed.
        assert!(matches!(run(&bars(103.0), 100.0, 2.0, N - 1), Intent::Enter { side: Side::Long, .. }));
    }

    #[test]
    fn a_close_beyond_the_box_on_the_wrong_side_of_the_ema_is_no_trade() {
        assert_eq!(run(&bars(102.0), 103.0, 2.0, N - 1), Intent::None, "up-break under the EMA");
        assert_eq!(run(&bars(98.0), 97.0, 2.0, N - 1), Intent::None, "down-break over the EMA");
    }

    #[test]
    fn the_signal_on_bar_i_does_not_change_when_later_bars_change() {
        let mut long = bars(102.0);
        let i = N - 1;
        // Add bars after i so there is a future to alter.
        for k in 0..5 {
            long.push(Bar::flat((N + k) as i64 * 60_000, 100.0));
        }
        let before = run(&long, 100.0, 2.0, i);
        assert!(matches!(before, Intent::Enter { side: Side::Long, .. }), "{before:?}");
        let mut altered = long.clone();
        for b in &mut altered[i + 1..] {
            b.open = 50.0;
            b.high = 200.0;
            b.low = 10.0;
            b.close = 45.0;
        }
        assert_eq!(run(&altered, 100.0, 2.0, i), before, "the future must not be readable");
        assert_eq!(run(&long[..=i], 100.0, 2.0, i), before, "the slice cut at i is the same world");
    }

    #[test]
    fn registered_and_engine_exits() {
        let registry = Registry::with_builtins();
        let s = registry.get("volman-box").unwrap();
        assert_eq!(s.exits(), Exits::Engine);
        assert_eq!(s.grid().get("boxBars"), Some(&vec![12.0, 20.0, 30.0]));
        assert_eq!(s.grid().len(), 1, "everything else is pinned");
        assert_eq!(s.warmup(&s.default_params()), 20 + 18 + 5);
        assert_eq!(s.series(&s.default_params()), vec!["ema_18".to_string(), "atr_14".to_string()]);
    }

    #[test]
    fn too_few_bars_for_a_box_gives_nothing() {
        let b = bars(102.0);
        assert_eq!(run(&b, 100.0, 2.0, 10), Intent::None);
    }
}
