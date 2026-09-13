//! Doji reversal.
//!
//! A doji — a bar that closes where it opened after travelling — is read as
//! indecision; at the end of a directional move it is read as exhaustion.
//! The rule: after a move of at least `trendAtr` ATRs over `trendBars` bars,
//! a bar whose body is at most `bodyMaxPct` of its range and whose range is
//! at least `minRangeAtr` ATRs signals a fade of the move. Stop beyond the
//! doji's far extreme plus `bufferPips`; target `riskReward` × risk. Fill is
//! the engine's: the next bar's open.
//!
//! Only the first doji of a move counts: a second doji while the prior one's
//! bar range is still unbroken is the same signal repeated, not a new one.
//! Stateless: everything is recomputed from the closed bars at or before `i`.
//!
//! Spec: `docs/hypotheses/2026-09-13-doji.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct DojiReversal;

impl Strategy for DojiReversal {
    fn id(&self) -> &'static str {
        "doji-reversal"
    }
    fn name(&self) -> &'static str {
        "Doji reversal"
    }
    fn description(&self) -> &'static str {
        "Fade a directional move when a doji (tiny body, real range) prints at its end; stop beyond the doji, \
         target a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("bodyMaxPct", 0.10),
            ("minRangeAtr", 1.0),
            ("trendBars", 10.0),
            ("trendAtr", 1.5),
            ("bufferPips", 3.0),
            ("riskReward", 1.5),
            ("atrPeriod", 14.0),
            ("pipSize", 0.1),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("bodyMaxPct".to_string(), vec![0.05, 0.10, 0.20]),
            ("riskReward".to_string(), vec![1.0, 1.5, 2.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod").max(p.period("trendBars")) + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let atr = ctx.s(0);
        if !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }
        let bar = ctx.bar;
        let Some(move_side) = move_before(ctx, p, atr) else { return Intent::None };
        if !is_doji(bar, p, atr) {
            return Intent::None;
        }
        // First doji of this move: the previous bar must not also have been
        // a qualifying doji after the same kind of move.
        if let Some(prev) = ctx.prev()
            && is_doji(prev, p, atr)
        {
            return Intent::None;
        }
        let pip = p.get("pipSize");
        let buffer = p.get("bufferPips") * pip;
        let (side, stop) = match move_side {
            Side::Long => (Side::Short, bar.high + buffer),
            Side::Short => (Side::Long, bar.low - buffer),
        };
        let risk = (bar.close - stop).abs();
        if risk <= 0.0 {
            return Intent::None;
        }
        let target = if side.is_long() { bar.close + risk * p.get("riskReward") } else { bar.close - risk * p.get("riskReward") };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "doji after a {} move of {:.1} ATR",
                if move_side.is_long() { "rising" } else { "falling" },
                move_size(ctx, p).abs() / atr
            ),
        }
    }
}

fn is_doji(bar: &fd_core::types::Bar, p: &Params, atr: f64) -> bool {
    let range = bar.high - bar.low;
    range >= p.get("minRangeAtr") * atr && (bar.close - bar.open).abs() <= p.get("bodyMaxPct") * range
}

/// Close-to-close change over `trendBars` bars ending at the bar before this
/// one — the move the doji is supposed to end.
fn move_size(ctx: &BarContext, p: &Params) -> f64 {
    let n = p.period("trendBars");
    let Some(start) = ctx.i.checked_sub(n) else { return 0.0 };
    let Some(before) = ctx.i.checked_sub(1) else { return 0.0 };
    ctx.bars[before].close - ctx.bars[start].close
}

fn move_before(ctx: &BarContext, p: &Params, atr: f64) -> Option<Side> {
    let size = move_size(ctx, p);
    if size.abs() < p.get("trendAtr") * atr {
        return None;
    }
    Some(if size > 0.0 { Side::Long } else { Side::Short })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    fn bars_rising_then_doji() -> Vec<Bar> {
        // Twelve bars rising $2 each (a $22 move), then a doji with a $6
        // range closing where it opened.
        let mut bars: Vec<Bar> = (0..12)
            .map(|k| {
                let c = 4400.0 + 2.0 * k as f64;
                Bar { time: k as i64 * 300_000, open: c - 2.0, high: c + 0.5, low: c - 2.5, close: c, volume: None }
            })
            .collect();
        bars.push(Bar { time: 12 * 300_000, open: 4423.0, high: 4427.0, low: 4421.0, close: 4423.2, volume: None });
        bars
    }

    fn intent_at(bars: &[Bar], i: usize, atr: f64, params: &Params) -> Intent {
        let series_atr: Vec<f64> = vec![atr; bars.len()];
        let series: [&[f64]; 1] = [&series_atr];
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &series, options: None, position: None, params };
        DojiReversal.on_bar(&ctx)
    }

    #[test]
    fn a_doji_after_a_rise_sells_with_the_stop_over_its_high() {
        let bars = bars_rising_then_doji();
        let params = DojiReversal.default_params();
        let intent = intent_at(&bars, 12, 3.0, &params); // ATR 3: range 6 ≥ 1 ATR, move 20 ≥ 1.5 ATR
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Short);
        assert!((stop.unwrap() - 4427.3).abs() < 1e-9);
        let risk = 4427.3 - 4423.2;
        assert!((target.unwrap() - (4423.2 - 1.5 * risk)).abs() < 1e-9);
    }

    #[test]
    fn a_body_too_large_or_a_range_too_small_or_no_move_is_not_a_signal() {
        let mut bars = bars_rising_then_doji();
        let params = DojiReversal.default_params();
        bars[12].close = 4425.0; // body 2 of range 6 = 33%
        assert_eq!(intent_at(&bars, 12, 3.0, &params), Intent::None);
        bars[12].close = 4423.2;
        assert_eq!(intent_at(&bars, 12, 10.0, &params), Intent::None, "range 6 < 1 ATR of 10");
        // Flatten the move: no trend, no signal.
        for b in &mut bars[..12] {
            b.close = 4400.0;
        }
        assert_eq!(intent_at(&bars, 12, 3.0, &params), Intent::None);
    }

    #[test]
    fn the_second_doji_in_a_row_is_not_a_new_signal() {
        let mut bars = bars_rising_then_doji();
        bars.push(Bar { time: 13 * 300_000, open: 4423.0, high: 4427.0, low: 4421.0, close: 4423.1, volume: None });
        let params = DojiReversal.default_params();
        assert!(matches!(intent_at(&bars, 12, 3.0, &params), Intent::Enter { .. }));
        assert_eq!(intent_at(&bars, 13, 3.0, &params), Intent::None);
    }

    #[test]
    fn the_signal_does_not_change_when_later_bars_change() {
        let mut bars = bars_rising_then_doji();
        bars.push(Bar::flat(13 * 300_000, 4300.0));
        let params = DojiReversal.default_params();
        let before = intent_at(&bars, 12, 3.0, &params);
        bars[13] = Bar::flat(13 * 300_000, 4500.0);
        assert_eq!(intent_at(&bars, 12, 3.0, &params), before);
    }
}
