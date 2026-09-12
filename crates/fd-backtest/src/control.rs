//! A strategy that knows nothing, run through the whole pipeline.
//!
//! Every number this project reports comes out of a machine that searches: nine
//! methods, a parameter grid each, parameters chosen on a training window. A
//! search finds the best of what it was given, and the best of a pile of noise
//! is not zero. So the question "is a profit factor of 1.2 good?" cannot be
//! answered by staring at 1.2 — it needs to be compared against what this same
//! machine produces when it is fed no signal at all.
//!
//! This is that control. It takes a position at random, and is deliberately
//! identical to a real strategy in every other respect:
//!
//! * same ATR-derived stop and target,
//! * same next-bar-open fill model,
//! * same position sizing and the same costs,
//! * **and the same parameter selection**, because selection is part of what
//!   might be manufacturing a result. Comparing an optimised strategy against
//!   an unoptimised coin flip would flatter the strategy for free.
//!
//! Randomness is derived from the bar rather than held as state: `on_bar` takes
//! `&self` and runs across rayon threads, and a run has to be reproducible from
//! its seed alone.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Intent, Params, Side, Strategy};

/// One round of SplitMix64.
///
/// Chosen because it is a few lines, has no state, and passes the statistical
/// tests that matter here. Anything weaker risks the control failing for a
/// reason that has nothing to do with the market.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform draw in `[0, 1)` for one bar of one stream.
fn draw(seed: u64, bar: u64, stream: u64) -> f64 {
    let value = mix(seed ^ mix(bar.wrapping_mul(0x1000_0000_0000_01B3) ^ stream));
    // 53 bits is the whole mantissa: enough for a probability, and exact.
    (value >> 11) as f64 / (1u64 << 53) as f64
}

pub struct RandomEntry;

impl Strategy for RandomEntry {
    fn id(&self) -> &'static str {
        "null-random"
    }

    fn name(&self) -> &'static str {
        "Random entry (control)"
    }

    fn description(&self) -> &'static str {
        "Takes a position at random with the same stops, sizing and costs as every other method. \
         Not a trading method: the distribution of its results is what any real method has to beat."
    }

    fn default_params(&self) -> Params {
        Params::new(&[("entryRate", 0.02), ("atrPeriod", 14.0), ("stopAtr", 1.5), ("seed", 1.0)])
    }

    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // Deliberately the same shape as a real strategy's grid — nine cells,
        // two axes — so that the selection step has the same amount of room to
        // find something flattering.
        BTreeMap::from([
            ("entryRate".to_string(), vec![0.01, 0.02, 0.04]),
            ("stopAtr".to_string(), vec![1.0, 1.5, 2.0]),
        ])
    }

    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }

    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 5
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", format_period(p.get("atrPeriod")))]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let atr = ctx.s(ATR);
        if !atr.is_finite() || ctx.position.is_some() {
            return Intent::None;
        }
        // The seed is a parameter so a whole run is reproducible from the
        // command line, and so that different seeds are different worlds rather
        // than different offsets into one.
        let seed = p.get("seed") as u64;
        let bar = ctx.bar.time as u64;
        if draw(seed, bar, 0) >= p.get("entryRate") {
            return Intent::None;
        }
        let side = if draw(seed, bar, 1) < 0.5 { Side::Long } else { Side::Short };
        let stop_atr = p.get("stopAtr");
        Intent::Enter {
            side,
            stop: Some(if side.is_long() {
                ctx.bar.close - atr * stop_atr
            } else {
                ctx.bar.close + atr * stop_atr
            }),
            target: None,
            reason: "coin flip".into(),
        }
    }
}

/// The indicator crate formats a whole-number parameter without a decimal
/// point; the key has to match exactly or the series lookup silently misses.
fn format_period(value: f64) -> String {
    if value.fract() == 0.0 { format!("{}", value as i64) } else { format!("{value}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_reproduces_its_own_sequence() {
        let first: Vec<f64> = (0..8).map(|i| draw(7, i * 900_000, 0)).collect();
        let again: Vec<f64> = (0..8).map(|i| draw(7, i * 900_000, 0)).collect();
        assert_eq!(first, again, "a run must be reproducible from its seed");
    }

    #[test]
    fn different_seeds_are_different_worlds() {
        let a: Vec<f64> = (0..16).map(|i| draw(1, i * 900_000, 0)).collect();
        let b: Vec<f64> = (0..16).map(|i| draw(2, i * 900_000, 0)).collect();
        assert_ne!(a, b);
    }

    #[test]
    fn the_two_streams_are_independent() {
        // Entry and direction must not be the same coin, or every entry would
        // be a long.
        let entries: Vec<f64> = (0..32).map(|i| draw(3, i * 900_000, 0)).collect();
        let sides: Vec<f64> = (0..32).map(|i| draw(3, i * 900_000, 1)).collect();
        assert_ne!(entries, sides);
    }

    #[test]
    fn draws_stay_inside_the_unit_interval() {
        for i in 0..10_000u64 {
            let value = draw(11, i * 60_000, 0);
            assert!((0.0..1.0).contains(&value), "draw out of range: {value}");
        }
    }

    #[test]
    fn the_entry_rate_is_roughly_honoured() {
        let rate = 0.02;
        let hits = (0..100_000u64).filter(|i| draw(5, i * 60_000, 0) < rate).count();
        let expected = 100_000.0 * rate;
        // Five sigma on a binomial with these numbers is about 220.
        assert!(
            (hits as f64 - expected).abs() < 300.0,
            "entry rate drifted: {hits} hits, expected about {expected}"
        );
    }
}
