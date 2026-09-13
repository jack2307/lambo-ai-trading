//! Weekend gap fade.
//!
//! Gold closes Friday 17:00 New York and reopens Sunday 18:00 on whatever
//! the weekend's news left; the first prints are thin, and the gap they
//! open tends to be revisited once the week's liquidity arrives. The rule:
//! on the first bar after a break in the bars of more than `minGapHours`,
//! if the open sits more than `minGapAtr` ATRs from the last close before
//! the break, enter toward that close, target it (or `fillPct` of the way),
//! stop `stopGapMult` gaps beyond the open. One trade per gap.
//!
//! The engine's maximum hold applies (it is an `Exits::Engine` method), so
//! the fill has the configured hours to happen; a gap that needs the whole
//! week is a gap that did not fill.
//!
//! Spec: `docs/hypotheses/2026-09-13-gap-fade.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct GapFade;

impl Strategy for GapFade {
    fn id(&self) -> &'static str {
        "gap-fade"
    }
    fn name(&self) -> &'static str {
        "Weekend gap fade"
    }
    fn description(&self) -> &'static str {
        "On the first bar after a break in trading, fade a gap larger than N ATRs back toward the last close."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("minGapHours", 24.0),
            ("minGapAtr", 1.0),
            ("fillPct", 100.0),
            ("stopGapMult", 1.0),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("minGapAtr".to_string(), vec![0.5, 1.0, 2.0]),
            ("stopGapMult".to_string(), vec![0.5, 1.0, 2.0]),
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
        let Some(prev) = ctx.prev() else {
            return Intent::None;
        };
        let bar = ctx.bar;
        // The bar after the break, and only that bar.
        if bar.time - prev.time < (p.get("minGapHours") * 3_600_000.0) as i64 {
            return Intent::None;
        }
        // ATR at the last bar before the break: the current bar's ATR
        // already contains the gap itself.
        let atr = ctx.s_back(0, 1);
        if !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }
        let gap = bar.open - prev.close;
        if gap.abs() < p.get("minGapAtr") * atr {
            return Intent::None;
        }
        let side = if gap > 0.0 { Side::Short } else { Side::Long };
        let fill = p.get("fillPct") / 100.0;
        // The signal bar is the gap bar; the fill is the next bar's open,
        // so the target and stop are placed off the gap geometry, not the
        // close: target toward the pre-break close, stop beyond the open.
        let target = bar.open - gap * fill;
        let stop = bar.open + gap * p.get("stopGapMult");
        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "gap of {:.2} ({:.1} ATR) over {:.0}h",
                gap,
                gap.abs() / atr,
                (bar.time - prev.time) as f64 / 3.6e6
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    fn intent(bars: &[Bar], i: usize, atr: f64) -> Intent {
        let a: Vec<f64> = vec![atr; bars.len()];
        let series: [&[f64]; 1] = [&a];
        let ind = fd_indicators::IndicatorSet::new();
        let params = GapFade.default_params();
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
        GapFade.on_bar(&ctx)
    }

    #[test]
    fn a_gap_up_over_a_weekend_is_sold_toward_the_friday_close() {
        const H: i64 = 3_600_000;
        let bars = vec![
            Bar::flat(0, 4400.0),
            Bar::flat(15 * 60_000, 4400.0), // Friday's last bar closes 4400
            Bar { time: 15 * 60_000 + 49 * H, open: 4412.0, high: 4414.0, low: 4410.0, close: 4411.0, volume: None },
            Bar::flat(15 * 60_000 + 49 * H + 15 * 60_000, 4411.0),
        ];
        let it = intent(&bars, 2, 3.0); // gap 12 = 4 ATR
        let Intent::Enter { side, stop, target, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Short);
        assert_eq!(target, Some(4400.0), "the whole gap");
        assert_eq!(stop, Some(4424.0), "one gap beyond the open");
        assert_eq!(intent(&bars, 3, 3.0), Intent::None, "only the first bar after the break");
        assert_eq!(intent(&bars, 1, 3.0), Intent::None, "no break, no gap");
    }

    #[test]
    fn a_small_gap_is_ignored() {
        const H: i64 = 3_600_000;
        let bars = vec![
            Bar::flat(0, 4400.0),
            Bar { time: 49 * H, open: 4401.0, high: 4402.0, low: 4400.0, close: 4401.0, volume: None },
        ];
        assert_eq!(intent(&bars, 1, 3.0), Intent::None);
    }
}
