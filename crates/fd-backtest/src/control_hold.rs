//! A control for drift claims: hold a random side for a fixed time.
//!
//! [`crate::control::RandomEntry`] answers "what does random *entry* with a
//! stop and a target produce"; a drift claim — "long from 09:30 to 16:00
//! pays" — has no stop and no target, only a window, so its noise floor is
//! random *holds* of the same length: a coin-flip side, entered where the
//! filters allow (the same window open the method uses), closed after
//! `holdMinutes`. Same costs, same sizing, same fills. What the method has to
//! beat is the distribution of these across seeds.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

/// SplitMix64, as in `control.rs`.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn draw(seed: u64, bar: u64, stream: u64) -> f64 {
    let value = mix(seed ^ mix(bar.wrapping_mul(0x1000_0000_0000_01B3) ^ stream));
    (value >> 11) as f64 / (1u64 << 53) as f64
}

pub struct RandomHold;

impl Strategy for RandomHold {
    fn id(&self) -> &'static str {
        "null-hold"
    }
    fn name(&self) -> &'static str {
        "Random hold (control)"
    }
    fn description(&self) -> &'static str {
        "Holds a coin-flip side for a fixed number of minutes from wherever the filters let it enter. \
         The noise floor for a drift claim."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("holdMinutes", 390.0), ("entryRate", 1.0), ("atrPeriod", 14.0), ("seed", 1.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let p = ctx.params;
        if let Some(open) = ctx.position {
            let held = ctx.bar.time - open.entry_time;
            return if held >= (p.get("holdMinutes") * 60_000.0) as i64 {
                Intent::Exit { reason: "hold elapsed".into() }
            } else {
                Intent::None
            };
        }
        let seed = p.get("seed") as u64;
        let bar = ctx.bar.time as u64;
        if draw(seed, bar, 0) >= p.get("entryRate") {
            return Intent::None;
        }
        Intent::Enter {
            side: if draw(seed, bar, 1) < 0.5 { Side::Long } else { Side::Short },
            stop: None,
            target: None,
            reason: "coin flip, fixed hold".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;
    use fd_strategy::registry::OpenPosition;

    #[test]
    fn a_hold_ends_when_its_minutes_have_passed_and_not_before() {
        let bars: Vec<Bar> = (0..3).map(|i| Bar::flat(i * 60_000 * 195, 100.0)).collect();
        let ind = fd_indicators::IndicatorSet::new();
        let params = RandomHold.default_params(); // 390 minutes
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        let at = |i: usize| BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: Some(open), params: &params };
        assert_eq!(RandomHold.on_bar(&at(1)), Intent::None, "195 minutes in: still held");
        assert!(matches!(RandomHold.on_bar(&at(2)), Intent::Exit { .. }), "390 minutes: closed");
    }

    #[test]
    fn different_seeds_are_different_worlds_and_the_same_seed_replays() {
        let bars: Vec<Bar> = (0..40).map(|i| Bar::flat(i * 60_000, 100.0)).collect();
        let ind = fd_indicators::IndicatorSet::new();
        let sides = |seed: f64| -> Vec<bool> {
            let mut p = RandomHold.default_params();
            p.set("seed", seed);
            (0..40)
                .map(|i| {
                    let ctx = BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: None, params: &p };
                    matches!(RandomHold.on_bar(&ctx), Intent::Enter { side: Side::Long, .. })
                })
                .collect()
        };
        assert_eq!(sides(1.0), sides(1.0));
        assert_ne!(sides(1.0), sides(2.0));
    }
}
