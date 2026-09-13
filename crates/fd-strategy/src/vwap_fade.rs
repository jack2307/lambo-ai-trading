//! Reversion to the session VWAP.
//!
//! Institutions working large orders benchmark to the day's volume-weighted
//! average price, so price stretched far from it is price that those orders
//! will pull back. The rule: when a bar closes more than `stretchAtr` ATRs
//! below the session VWAP, buy with the VWAP as the target and a stop
//! `stopAtr` ATRs further out; symmetric above. Only the *first* bar that
//! crosses the threshold in a session and direction signals, so a trending
//! day is one trade, not a ladder of them.
//!
//! The VWAP comes from `fd-indicators` (`vwap`, session = UTC day). Where a
//! feed has no volume the indicator falls back to equal weights, i.e. a
//! TWAP; the batch file says which feed the claim is really being tested on.
//!
//! Spec: `docs/hypotheses/2026-09-13-vwap-fade.md`.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct VwapFade;

impl Strategy for VwapFade {
    fn id(&self) -> &'static str {
        "vwap-fade"
    }
    fn name(&self) -> &'static str {
        "VWAP fade"
    }
    fn description(&self) -> &'static str {
        "Against the first close more than N ATRs from the session VWAP, targeting the VWAP, stop N ATRs further out."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("stretchAtr", 1.5), ("stopAtr", 1.0), ("atrPeriod", 14.0), ("sessionMs", 86_400_000.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("stretchAtr".to_string(), vec![1.0, 1.5, 2.0]),
            ("stopAtr".to_string(), vec![1.0, 1.5, 2.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("vwap").with("sessionMs", p.get("sessionMs")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("vwap_{}", p.get("sessionMs")), format!("atr_{}", p.get("atrPeriod"))]
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
        const VWAP: usize = 0;
        const ATR: usize = 1;
        let p = ctx.params;
        let (vwap, atr) = (ctx.s(VWAP), ctx.s(ATR));
        if !vwap.is_finite() || !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }
        let stretch = p.get("stretchAtr") * atr;
        let close = ctx.bar.close;
        let side = if close <= vwap - stretch {
            Side::Long
        } else if close >= vwap + stretch {
            Side::Short
        } else {
            return Intent::None;
        };
        // First crossing only: the previous bar must not already have been
        // stretched the same way against the previous VWAP.
        let (prev_vwap, prev_atr) = (ctx.s_back(VWAP, 1), ctx.s_back(ATR, 1));
        if let Some(prev) = ctx.prev()
            && prev_vwap.is_finite()
            && prev_atr.is_finite()
        {
            let prev_stretch = p.get("stretchAtr") * prev_atr;
            let already = match side {
                Side::Long => prev.close <= prev_vwap - prev_stretch,
                Side::Short => prev.close >= prev_vwap + prev_stretch,
            };
            if already {
                return Intent::None;
            }
        }
        let stop_distance = p.get("stopAtr") * atr;
        let (stop, target) = match side {
            Side::Long => (close - stop_distance, vwap),
            Side::Short => (close + stop_distance, vwap),
        };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!("{:.1} ATR from the session VWAP {:.2}", (close - vwap).abs() / atr, vwap),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    fn ctx_with<'a>(
        bars: &'a [Bar],
        i: usize,
        vwap: &'a [f64],
        atr: &'a [f64],
        params: &'a Params,
        ind: &'a fd_indicators::IndicatorSet,
        series: &'a [&'a [f64]],
    ) -> BarContext<'a> {
        let _ = (vwap, atr);
        BarContext { bar: &bars[i], i, bars, ind, series, options: None, position: None, params }
    }

    #[test]
    fn the_first_stretched_close_fades_toward_the_vwap_and_the_next_does_not() {
        let bars = vec![Bar::flat(0, 100.0), Bar::flat(60_000, 96.0), Bar::flat(120_000, 95.0)];
        let vwap = [100.0, 100.0, 100.0];
        let atr = [2.0, 2.0, 2.0];
        let params = VwapFade.default_params(); // stretch 1.5 ATR = 3.0
        let ind = fd_indicators::IndicatorSet::new();
        let series: [&[f64]; 2] = [&vwap, &atr];
        let first = VwapFade.on_bar(&ctx_with(&bars, 1, &vwap, &atr, &params, &ind, &series));
        let Intent::Enter { side, stop, target, .. } = first else { panic!("expected an entry, got {first:?}") };
        assert_eq!(side, Side::Long);
        assert_eq!(target, Some(100.0), "target is the VWAP");
        assert_eq!(stop, Some(96.0 - 2.0), "stop one ATR below the close");
        let second = VwapFade.on_bar(&ctx_with(&bars, 2, &vwap, &atr, &params, &ind, &series));
        assert_eq!(second, Intent::None, "still stretched: not a new crossing");
    }

    #[test]
    fn inside_the_band_or_without_an_atr_there_is_nothing() {
        let bars = vec![Bar::flat(0, 100.0), Bar::flat(60_000, 99.0)];
        let params = VwapFade.default_params();
        let ind = fd_indicators::IndicatorSet::new();
        let vwap = [100.0, 100.0];
        let atr = [2.0, 2.0];
        let series: [&[f64]; 2] = [&vwap, &atr];
        assert_eq!(VwapFade.on_bar(&ctx_with(&bars, 1, &vwap, &atr, &params, &ind, &series)), Intent::None);
        let nan = [f64::NAN, f64::NAN];
        let series2: [&[f64]; 2] = [&vwap, &nan];
        assert_eq!(VwapFade.on_bar(&ctx_with(&bars, 1, &vwap, &nan, &params, &ind, &series2)), Intent::None);
    }
}
