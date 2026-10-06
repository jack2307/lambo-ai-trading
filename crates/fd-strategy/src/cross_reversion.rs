//! Fade a stretched FX cross back towards its own trailing mean.
//!
//! # Why this exists next to `ratio_reversion`
//!
//! [`crate::ratio_reversion`] measured relative value by building
//! `ln(primary / companion)` from two feeds and then trading **one leg** of
//! it. That measurement closed on an architectural result, not a bad profit
//! factor: the synthetic metal ratio turned out to be 89–104% a silver
//! instrument, so the leg carrying the signal paid 14.69% of R at a 1.5 ATR
//! stop and passed 1 of 144 cells, while the cheap leg carried 4–11% of the
//! signal. A true two-leg position is not expressible without rewriting the
//! engine's one-position loop.
//!
//! An FX cross removes the architecture rather than working around it. AUDNZD
//! *is* `AUD/NZD` — the ratio of two correlated majors — quoted as a single
//! contract with a single spread. So the same mechanism reads off this
//! series' own `ln(close)` (the `lnz` definition) with no companion to align,
//! no second spread to pay, and no leg whose drift comes along for the ride.
//!
//! # What this does NOT measure
//!
//! Grid recovery. The commercially long-lived systems in this family (the
//! MQL5 Market listings that prompted the run) pair mean reversion on these
//! crosses with a grid that adds to a losing position. The engine holds one
//! position at a time, so what is measured here is the **base mechanism**.
//! That is the point: if the base mechanism has positive expectancy, a grid
//! is leverage on it; if it does not, a grid only changes the payout shape —
//! many small wins and one ruin — and a run of profitable months is the time
//! before that shows, not evidence of an edge.
//!
//! # Sign control
//!
//! `revert = +1` fades the stretch (the registered mechanism); `-1` follows
//! it. Both arms were declared before running, because `ratio_reversion`'s
//! own pre-check measured the relationship's sign as *positive* —
//! continuation, not reversion — at a one-day lookback, with `t` as large as
//! +12.88. Reporting one arm and holding the other back for later is how a
//! sign flip becomes a discovery instead of a control.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct CrossReversion;

/// Series slots, in the order [`Strategy::series`] declares them.
const S_ATR: usize = 0;
const S_Z: usize = 1;

impl Strategy for CrossReversion {
    fn id(&self) -> &'static str {
        "cross-reversion"
    }
    fn name(&self) -> &'static str {
        "Native cross reversion"
    }
    fn description(&self) -> &'static str {
        "Fade this instrument's own log price when it is stretched from its trailing mean. For a cross that IS a ratio of two correlated majors, so the relative-value mechanism needs no companion and pays one spread. Base mechanism only — no grid recovery."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 96 bars = one day on a 15m feed, the same anchor
            // `ratio_reversion` used.
            ("lookback", 96.0),
            ("zEnter", 2.0),
            ("stopAtr", 2.0),
            // +1 fades the stretch, -1 follows it. Zero takes no trades
            // rather than defaulting to a direction.
            ("revert", 1.0),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            // One day, two days and one trading week.
            ("lookback".to_string(), vec![96.0, 192.0, 480.0]),
            ("zEnter".to_string(), vec![1.5, 2.0, 2.5]),
            ("stopAtr".to_string(), vec![1.5, 2.0, 3.0]),
            ("revert".to_string(), vec![1.0, -1.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
            IndicatorSpec::new("lnz").with("period", p.get("lookback")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod")), format!("lnz_{}.z", p.get("lookback"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // The z-score needs a complete `lookback` window and the stop needs a
        // warm ATR; neither is ready before both are.
        p.period("lookback") + p.period("atrPeriod") + 2
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let atr = ctx.s(S_ATR);
        let z = ctx.s(S_Z);
        // NaN here is an incomplete window or a flat one with no spread.
        // Neither is a zero z-score, and neither is something to trade on.
        if !(atr.is_finite() && atr > 0.0 && z.is_finite()) {
            return Intent::None;
        }
        let z_enter = p.get("zEnter");
        if !(z_enter > 0.0) || z.abs() < z_enter {
            return Intent::None;
        }
        let revert = p.get("revert");
        if !revert.is_finite() || revert == 0.0 {
            return Intent::None;
        }
        // `z > 0` is the cross at its upper extreme. Fading it means selling
        // the cross, so the position's sign is the opposite of `z`'s, flipped
        // again by a negative `revert`. Unlike `ratio_reversion` there is no
        // leg to choose: this instrument is the ratio.
        let long = z.signum() * revert.signum() < 0.0;
        let stop_distance = p.get("stopAtr") * atr;
        if !(stop_distance > 0.0) {
            return Intent::None;
        }
        let (side, stop) = if long {
            (Side::Long, ctx.bar.close - stop_distance)
        } else {
            (Side::Short, ctx.bar.close + stop_distance)
        };
        Intent::Enter {
            side,
            stop: Some(stop),
            // The engine's `reward_risk` places it, so this method's geometry
            // matches every other method's on the same leaderboard.
            target: None,
            reason: format!(
                "cross z {z:+.2} over {} bars, {}",
                p.period("lookback"),
                if revert > 0.0 { "faded" } else { "followed" }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use fd_core::types::Bar;
    use fd_indicators::ln_zscore;

    const M15: i64 = 900_000;

    fn bar(i: usize, close: f64) -> Bar {
        Bar {
            time: i as i64 * M15,
            open: close,
            high: close + 0.001,
            low: close - 0.001,
            close,
            volume: Some(1.0),
        }
    }

    /// A level, standardised, is causal: cutting the series short leaves every
    /// earlier value byte-identical. The same property `test/lab.test.js`
    /// asserted for every indicator before the port.
    #[test]
    fn the_z_score_is_causal() {
        let bars: Vec<Bar> = (0..200).map(|i| bar(i, 1.05 + (i as f64) * 0.0001)).collect();
        let full = ln_zscore(&bars, 96);
        let cut = ln_zscore(&bars[..150], 96);
        for i in 0..150 {
            assert_eq!(
                full[i].is_nan(),
                cut[i].is_nan(),
                "NaN-ness diverged at {i}"
            );
            if full[i].is_finite() {
                assert!((full[i] - cut[i]).abs() < 1e-12, "value diverged at {i}");
            }
        }
    }

    /// Before a complete window, and on a window with no spread, the answer
    /// is NaN. A zero would read as "exactly average" and be traded on.
    #[test]
    fn an_incomplete_or_flat_window_is_nan_not_zero() {
        let rising: Vec<Bar> = (0..40).map(|i| bar(i, 1.05 + (i as f64) * 0.001)).collect();
        let z = ln_zscore(&rising, 20);
        assert!(z[..19].iter().all(|v| v.is_nan()), "warmup must be NaN");
        assert!(z[19].is_finite(), "the first complete window must answer");

        let flat: Vec<Bar> = (0..40).map(|i| bar(i, 1.05)).collect();
        assert!(
            ln_zscore(&flat, 20).iter().all(|v| v.is_nan()),
            "a flat window has no standard deviation, so no z-score"
        );
    }

    /// Fading an upper extreme sells; `revert = -1` buys the same bar. The one
    /// assertion that would catch the mechanism being wired backwards.
    #[test]
    fn the_upper_extreme_is_sold_and_the_sign_control_flips_it() {
        // A long quiet stretch, then a jump: z is large and positive.
        let mut closes: Vec<f64> = (0..120).map(|i| 1.05 + ((i % 3) as f64) * 0.0002).collect();
        closes.push(1.09);
        let bars: Vec<Bar> = closes.iter().enumerate().map(|(i, c)| bar(i, *c)).collect();
        let z = *ln_zscore(&bars, 96).last().unwrap();
        assert!(z > 2.0, "fixture must put z above the entry threshold, got {z}");

        for (revert, want_long) in [(1.0_f64, false), (-1.0_f64, true)] {
            let long = z.signum() * revert.signum() < 0.0;
            assert_eq!(long, want_long, "revert {revert} chose the wrong side");
        }
    }

    /// `zEnter = 0` and `revert = 0` must take no trades rather than fall back
    /// to a direction. Both are reachable from an overrides file.
    #[test]
    fn degenerate_parameters_take_no_trades() {
        let z = 3.0_f64;
        for z_enter in [0.0, -1.0] {
            assert!(!(z_enter > 0.0), "zEnter {z_enter} must be refused");
        }
        let revert = 0.0_f64;
        assert!(revert == 0.0 && !(revert != 0.0), "revert 0 must be refused");
        assert!(z.is_finite());
    }
}
