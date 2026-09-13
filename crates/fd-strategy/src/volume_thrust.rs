//! Volume thrust continuation.
//!
//! A bar on far more volume than its neighbours that closes near its
//! extreme is a bar where one side got filled in size and did not get
//! pushed back — the footprint of a participant who will be back. The rule:
//! volume at least `volMult` × the mean of the last `volBars`, close in the
//! top (bottom) `closePct` of the range, and range at least `minRangeAtr`
//! ATRs; enter with the bar, stop beyond its other extreme plus a buffer,
//! target a multiple of the risk. First thrust of a run only.
//!
//! Needs real volume: Binance BTC has it; Dukascopy gold does not, and the
//! batch file says so.
//!
//! Spec: `docs/hypotheses/2026-09-13-volume-thrust.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct VolumeThrust;

impl Strategy for VolumeThrust {
    fn id(&self) -> &'static str {
        "volume-thrust"
    }
    fn name(&self) -> &'static str {
        "Volume thrust"
    }
    fn description(&self) -> &'static str {
        "Go with a bar on unusual volume that closes at its extreme; stop beyond its other end, target a multiple \
         of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("volMult", 2.5),
            ("volBars", 20.0),
            ("closePct", 0.2),
            ("minRangeAtr", 1.0),
            ("bufferPips", 3.0),
            ("riskReward", 1.5),
            ("atrPeriod", 14.0),
            ("pipSize", 1.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([("volMult".to_string(), vec![2.0, 2.5, 3.5]), ("riskReward".to_string(), vec![1.0, 1.5, 2.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("volBars").max(p.period("atrPeriod")) + 5
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
        let bar = ctx.bar;
        let n = p.period("volBars");
        if !atr.is_finite() || atr <= 0.0 || ctx.i < n + 1 {
            return Intent::None;
        }
        let Some(volume) = bar.volume else {
            return Intent::None;
        };
        let mean = ctx.bars[ctx.i - n..ctx.i].iter().filter_map(|b| b.volume).sum::<f64>() / n as f64;
        if mean <= 0.0 || volume < p.get("volMult") * mean {
            return Intent::None;
        }
        let range = bar.high - bar.low;
        if range < p.get("minRangeAtr") * atr {
            return Intent::None;
        }
        let pct = p.get("closePct");
        let side = if bar.close >= bar.high - range * pct {
            Side::Long
        } else if bar.close <= bar.low + range * pct {
            Side::Short
        } else {
            return Intent::None;
        };
        // First thrust of a run: the previous bar was not itself one.
        if let Some(prev) = ctx.prev()
            && prev.volume.is_some_and(|v| v >= p.get("volMult") * mean)
        {
            return Intent::None;
        }
        let buffer = p.get("bufferPips") * p.get("pipSize");
        let stop = if side.is_long() { bar.low - buffer } else { bar.high + buffer };
        let risk = (bar.close - stop).abs();
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
                "volume {:.1}x, close at the {}",
                volume / mean,
                if side.is_long() { "high" } else { "low" }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    fn bars() -> Vec<Bar> {
        let mut v: Vec<Bar> = (0..22)
            .map(|k| Bar { time: k * 60_000, open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: Some(10.0) })
            .collect();
        // Bar 21: 5x volume, closes at the high of a 4-point range.
        v[21] = Bar { time: 21 * 60_000, open: 100.0, high: 104.0, low: 100.0, close: 103.5, volume: Some(50.0) };
        v
    }

    fn intent(bars: &[Bar], i: usize) -> Intent {
        let atr = vec![2.0; bars.len()];
        let series: [&[f64]; 1] = [&atr];
        let ind = fd_indicators::IndicatorSet::new();
        let params = VolumeThrust.default_params();
        let ctx = BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &ind,
            series: &series,
            options: None,
            position: None,
            params: &params,
        };
        VolumeThrust.on_bar(&ctx)
    }

    #[test]
    fn a_high_volume_bar_closing_at_its_high_is_bought_with_the_stop_under_its_low() {
        let b = bars();
        let it = intent(&b, 21);
        let Intent::Enter { side, stop, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Long);
        assert!((stop.unwrap() - 97.0).abs() < 1e-9, "low 100 minus 3 pips of 1.0");
    }

    #[test]
    fn ordinary_volume_or_a_mid_range_close_is_nothing() {
        let mut b = bars();
        b[21].volume = Some(12.0);
        assert_eq!(intent(&b, 21), Intent::None);
        b[21].volume = Some(50.0);
        b[21].close = 102.0;
        assert_eq!(intent(&b, 21), Intent::None);
    }
}
