//! **Relative value between two metals**: the XAU/XAG ratio stretched away
//! from its own trailing mean, entered on ONE leg.
//!
//! Registered in `docs/decisions/2026-10-06-ratio-reversion.md` before this
//! file existed. Read that note first: its pre-check already weakened the
//! premise, and the numbers there are the prior this file was written against.
//!
//! # Why a ratio was worth measuring at all
//!
//! Every one-directional rule on a single instrument is exposed to that
//! instrument's drift, and the desk has now paid for that twice: the only cell
//! this engine ever printed `SURVIVES` for read the 98th percentile against a
//! coin-flip null and then `null p95 = 1.721` against a drift-matched one,
//! above its own `PF 1.704`. It was borrowing gold's July–September 2025
//! rally. A **ratio** has no instrument drift to borrow: gold and silver
//! rising together moves neither leg of `XAU/XAG`.
//!
//! # What this file can and cannot express — state it before reading a number
//!
//! The engine trades ONE instrument. `BarContext` carries one `bars` slice and
//! `Intent::Enter` carries one `side`, so a two-legged pairs trade (long gold,
//! short silver) is not expressible here without rewriting the engine. **This
//! is not a pairs trade.**
//!
//! What it is: the *signal* is computed from the ratio, and the *position* is
//! one leg of it. So the drift immunity above applies to the signal and **not
//! to the trade** — a one-leg position is still fully exposed to that leg's
//! drift, and the advantage is therefore only partly obtained. Nobody should
//! read this as a market-neutral method.
//!
//! The mechanism is read from either end by swapping which instrument is
//! primary, because [`fd_indicators::companion::ratio_zscore`] works in logs
//! and the z-score of the reciprocal ratio is the same number negated:
//!
//! * `--market=xauduka --companion=XAGDUKA` trades the **gold** leg;
//! * `--market=xagduka --companion=XAUDUKA` trades the **silver** leg.
//!
//! These are one rule read from two sides, not two rules. Which leg is the
//! right one to trade is a measurement, and the registration's pre-check says
//! it is not the cheap one: over both windows gold accounts for **4–11%** of
//! the variance of the ratio's changes and silver for **89–104%**, while
//! silver costs about **4.5× gold at the same stop size**. The cheap leg
//! carries almost none of the signal. Both are measured for that reason.
//!
//! # The rule
//!
//! On bar `i`, with `lookback` bars of complete two-series history behind it:
//!
//! * `z` = the z-score of `ln(primary.close / companion.close)` over the last
//!   `lookback` bars;
//! * if `|z| < zEnter`, nothing happens;
//! * otherwise enter **against** the stretch when `revert > 0` — `z` high
//!   means the ratio is at its upper extreme, reversion expects it to fall,
//!   and the one-leg expression of a falling `primary/companion` is SHORT the
//!   primary — and **with** it when `revert < 0`.
//!
//! Stop is `stopAtr` of the primary's own ATR from the signal bar's close. The
//! engine fills at the next bar's open, derives the target from
//! `[trading] reward_risk`, and owns the maximum hold.
//!
//! # `revert` is a declared parameter, not a rescue
//!
//! The continuation arm (`revert = -1`) is in the grid because the
//! registration's pre-check measured the sign of the relationship to **flip
//! with the lookback and with the window**: at `lookback = 96` the forward
//! move of the ratio is positively related to `z` in *both* windows (t up to
//! +12.88, i.e. continuation), while at `lookback = 480` window B reverts and
//! window A does not. Measuring only the reversion arm and then switching to
//! continuation after seeing the result would be fitting; declaring both
//! before running and counting both against the multiple-testing ledger is
//! not. Both arms are reported.
//!
//! # No companion means no trades
//!
//! With nothing installed every `ratz` output is NaN and this strategy takes
//! no trades at all. A run that produced nothing because no companion was
//! loaded is not a run that found no signals, and the binary's `companion:`
//! receipt line is what tells the two apart.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct RatioReversion;

/// Series slots, in the order [`Strategy::series`] declares them.
const S_ATR: usize = 0;
const S_Z: usize = 1;

impl Strategy for RatioReversion {
    fn id(&self) -> &'static str {
        "ratio-reversion"
    }
    fn name(&self) -> &'static str {
        "Metal ratio reversion (one leg)"
    }
    fn description(&self) -> &'static str {
        "Enter one leg when the primary/companion price ratio is stretched from its own trailing mean. Reads two series; needs `search --companion=<SYMBOL>` installed, and takes no trades without one. NOT a pairs trade — the position is one leg and carries that leg's drift."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 96 bars = one day on a 15m feed.
            ("lookback", 96.0),
            ("zEnter", 2.0),
            ("stopAtr", 2.0),
            // +1 fades the stretch (the registered mechanism), -1 follows it
            // (the declared sign control). Zero takes no trades rather than
            // defaulting to a direction.
            ("revert", 1.0),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            // One day and one trading week, the two horizons the registration
            // measured the sign of the relationship at.
            ("lookback".to_string(), vec![96.0, 480.0]),
            ("zEnter".to_string(), vec![1.5, 2.0, 2.5]),
            ("stopAtr".to_string(), vec![1.5, 2.0, 3.0]),
            ("revert".to_string(), vec![1.0, -1.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
            IndicatorSpec::new("ratz").with("period", p.get("lookback")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod")), format!("ratz_{}.z", p.get("lookback"))]
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
        // NaN here is a missing companion bar somewhere in the window, no
        // companion at all, or a flat window with no spread. None of them is a
        // zero z-score, and none of them is something to trade on.
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
        // `z > 0` is the ratio at its upper extreme. Fading it expects the
        // ratio to fall, and a falling `primary / companion` is expressed on
        // this leg by selling the primary — so the position's sign is the
        // opposite of `z`'s, flipped again by a negative `revert`.
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
                "ratio z {z:+.2} over {} bars, {}",
                p.period("lookback"),
                if revert > 0.0 { "faded" } else { "followed" }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    const M15: i64 = 900_000;

    fn bars(n: usize) -> Vec<Bar> {
        (0..n)
            .map(|i| Bar {
                time: i as i64 * M15,
                open: 3300.0,
                high: 3310.0,
                low: 3290.0,
                close: 3300.0,
                volume: None,
            })
            .collect()
    }

    fn decide(bars: &[Bar], i: usize, atr: f64, z: f64, params: &Params) -> Intent {
        let a = vec![atr; bars.len()];
        let zs = vec![z; bars.len()];
        let series: [&[f64]; 2] = [&a, &zs];
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &ind,
            series: &series,
            options: None,
            position: None,
            params,
        };
        RatioReversion.on_bar(&ctx)
    }

    #[test]
    fn a_ratio_at_its_upper_extreme_is_sold_on_this_leg() {
        let p = RatioReversion.default_params();
        let b = bars(10);
        let it = decide(&b, 9, 10.0, 2.5, &p);
        let Intent::Enter { side, stop, target, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Short, "a high ratio faded is a short of the primary");
        assert_eq!(stop, Some(3300.0 + 20.0), "stopAtr 2 x ATR 10 above the signal close");
        assert_eq!(target, None, "the engine places the target");
    }

    #[test]
    fn a_ratio_at_its_lower_extreme_is_bought_on_this_leg() {
        let p = RatioReversion.default_params();
        let b = bars(10);
        let Intent::Enter { side, stop, .. } = decide(&b, 9, 10.0, -2.5, &p) else { panic!("expected an entry") };
        assert_eq!(side, Side::Long);
        assert_eq!(stop, Some(3300.0 - 20.0));
    }

    #[test]
    fn the_continuation_arm_takes_the_opposite_side_of_the_same_bar() {
        let mut p = RatioReversion.default_params();
        p.set("revert", -1.0);
        let b = bars(10);
        let Intent::Enter { side, .. } = decide(&b, 9, 10.0, 2.5, &p) else { panic!("expected an entry") };
        assert_eq!(side, Side::Long, "following a high ratio buys the primary");
    }

    #[test]
    fn a_stretch_under_the_threshold_is_left_alone() {
        let p = RatioReversion.default_params();
        let b = bars(10);
        assert_eq!(decide(&b, 9, 10.0, 1.9, &p), Intent::None);
        assert_eq!(decide(&b, 9, 10.0, -1.9, &p), Intent::None);
        // Exactly at the threshold IS a signal: the comparison refuses only
        // what is strictly inside.
        assert!(matches!(decide(&b, 9, 10.0, 2.0, &p), Intent::Enter { .. }));
    }

    #[test]
    fn no_companion_means_no_trade_rather_than_a_trade_at_a_zero_z_score() {
        let p = RatioReversion.default_params();
        let b = bars(10);
        assert_eq!(decide(&b, 9, 10.0, f64::NAN, &p), Intent::None);
        // And a zero z-score is genuinely no signal, not a missing one.
        assert_eq!(decide(&b, 9, 10.0, 0.0, &p), Intent::None);
    }

    #[test]
    fn an_unwarm_atr_or_a_degenerate_stop_takes_no_trade() {
        let mut p = RatioReversion.default_params();
        let b = bars(10);
        assert_eq!(decide(&b, 9, f64::NAN, 2.5, &p), Intent::None);
        assert_eq!(decide(&b, 9, 0.0, 2.5, &p), Intent::None);
        p.set("stopAtr", 0.0);
        assert_eq!(decide(&b, 9, 10.0, 2.5, &p), Intent::None);
    }

    #[test]
    fn a_zero_revert_takes_no_trade_rather_than_picking_a_direction() {
        let mut p = RatioReversion.default_params();
        p.set("revert", 0.0);
        let b = bars(10);
        assert_eq!(decide(&b, 9, 10.0, 2.5, &p), Intent::None);
    }

    #[test]
    fn the_declared_series_names_match_the_keys_the_specs_produce() {
        for lookback in [96.0, 480.0] {
            let mut p = RatioReversion.default_params();
            p.set("lookback", lookback);
            let specs = RatioReversion.indicators(&p);
            let b = bars(600);
            let set = fd_indicators::compute_indicators(&b, &specs).expect("specs resolve");
            for key in RatioReversion.series(&p) {
                assert!(set.contains_key(&key), "series `{key}` is not among {:?}", set.keys().collect::<Vec<_>>());
            }
        }
    }

    /// Every grid cell must name series the specs actually produce, or a sweep
    /// silently measures a strategy that never sees its own signal.
    #[test]
    fn every_grid_cell_resolves_its_series() {
        let defaults = RatioReversion.default_params();
        let combos = crate::registry::parameter_combinations(&defaults, &RatioReversion.grid());
        assert_eq!(combos.len(), 36, "the registration declared 36 rows");
        let b = bars(600);
        for p in &combos {
            let set = fd_indicators::compute_indicators(&b, &RatioReversion.indicators(p)).expect("specs resolve");
            for key in RatioReversion.series(p) {
                assert!(set.contains_key(&key), "grid cell is missing series `{key}`");
            }
            assert!(RatioReversion.warmup(p) > 0);
        }
    }
}
