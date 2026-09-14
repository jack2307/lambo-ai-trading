//! A control for drift claims: hold a random side for a matched time.
//!
//! [`crate::control::RandomEntry`] answers "what does random *entry* with a
//! stop and a target produce"; a drift claim — "long from 09:30 to 16:00
//! pays" — has no stop and no target, only a window, so its noise floor is
//! random *holds* of the same length: a coin-flip side, entered where the
//! filters allow (the same window open the method uses), closed after
//! `holdMinutes`. Same costs, same sizing, same fills. What the method has to
//! beat is the distribution of these across seeds.
//!
//! Sizing follows the method: when `riskDailyRanges` is set (copied from a
//! method that sizes on daily ranges, see `fd_strategy::tsmom`), the hold
//! carries the same sizing stop; otherwise the engine's ATR fallback, as for
//! a session hold.
//!
//! The hold is a distribution, not a mean. Until 2026-09-14 the null held
//! the method's *mean* realised hold, fixed for every trade; the adversary on
//! `2026-09-14-tsmom-eurusd.md` called that "mean-matched, tail-unmatched":
//! a fixed 23-day random hold cannot produce the 356-day, +23R trade that
//! carried the row, so the null's profit factor is bounded where the
//! method's is not and the method's percentile is inflated by the exit rule,
//! not by its sign. With `holdLogSd > 0` each position's hold is drawn per
//! trade from a log-normal, `minutes = holdMinutes × exp(holdLogSd × z)` with
//! `z` standard normal, so `holdMinutes` is the log-median (the geometric
//! mean of the realised holds) and `holdLogSd` the standard deviation of
//! their logs — the two numbers `hypotheses::hold_distribution` measures on
//! the method's own trades. The null then owns the same long tail of holds
//! the method does. The draw is a pure function of the position's entry time
//! and `seed` (Box–Muller on two hashed uniforms, see [`hold_for`]),
//! recomputed on every bar, so the exit decision needs no state and the
//! same seed replays. `holdLogSd = 0` is the fixed hold, exactly as before.

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

/// The hold, in minutes, of a position entered at `entry_time`.
///
/// `log_sd <= 0` (or NaN) is the fixed hold: `hold_minutes`, untouched.
/// Otherwise a log-normal draw with `hold_minutes` as its log-median:
/// two uniforms hashed from `seed` and the entry time on streams 2 and 3
/// (the entry coin flips use 0 and 1, keyed on the signal bar), a Box–Muller
/// transform for the standard normal `z = sqrt(-2 ln u1) · cos(2π u2)`, and
/// `minutes = hold_minutes · exp(log_sd · z)`. `u1` is taken as `1 - draw`
/// so it lies in `(0, 1]` and the log is finite. Pure in its inputs: every
/// bar of a position computes the same hold, which is what makes a stateless
/// exit decision stable.
fn hold_for(seed: u64, entry_time: u64, hold_minutes: f64, log_sd: f64) -> f64 {
    if log_sd.is_nan() || log_sd <= 0.0 {
        return hold_minutes;
    }
    let u1 = 1.0 - draw(seed, entry_time, 2);
    let u2 = draw(seed, entry_time, 3);
    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    hold_minutes * (log_sd * z).exp()
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
        "Holds a coin-flip side for holdMinutes from wherever the filters let it enter — a fixed hold, or with \
         holdLogSd a log-normal draw per trade around it. The noise floor for a drift claim."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("holdMinutes", 390.0),
            ("holdLogSd", 0.0),
            ("entryRate", 1.0),
            ("atrPeriod", 14.0),
            ("seed", 1.0),
            ("riskDailyRanges", 0.0),
            ("rangeDays", 20.0),
        ])
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
        let seed = p.get("seed") as u64;
        if let Some(open) = ctx.position {
            // The engine fills an exit at the NEXT bar's open, so the signal
            // has to come one bar early for the fill to land at the hold's
            // end — exactly where a session hold's exit lands (its signal is
            // the first bar at or after `to`, filled at the next open).
            // Until 2026-09-14 this fired on the bar where the minutes had
            // elapsed, so every window null held one bar longer than the
            // method, and across the daily halt or the weekend when that
            // bar was the day's last (adversary, `2026-09-14-intraday-momentum.md`).
            let held = ctx.bar.time - open.entry_time;
            let interval = ctx.prev().map_or(0, |b| (ctx.bar.time - b.time).max(0));
            let minutes = hold_for(seed, open.entry_time as u64, p.get("holdMinutes"), p.get("holdLogSd"));
            return if held + interval >= (minutes * 60_000.0) as i64 {
                Intent::Exit { reason: "hold elapsed".into() }
            } else {
                Intent::None
            };
        }
        let bar = ctx.bar.time as u64;
        if draw(seed, bar, 0) >= p.get("entryRate") {
            return Intent::None;
        }
        let side = if draw(seed, bar, 1) < 0.5 { Side::Long } else { Side::Short };
        let ranges = p.get("riskDailyRanges");
        let stop = if ranges > 0.0 {
            let Some(stop) = fd_strategy::tsmom::sizing_stop(&ctx.bars[..=ctx.i], ctx.bar, side, ranges, p.period("rangeDays")) else {
                return Intent::None;
            };
            Some(stop)
        } else {
            None
        };
        Intent::Enter { side, stop, target: None, reason: "coin flip, fixed hold".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;
    use fd_strategy::registry::OpenPosition;

    #[test]
    fn a_hold_ends_when_its_minutes_have_passed_and_not_before() {
        // Bars every 130 minutes; a 390-minute hold entered at bar 0 must be
        // signalled on the bar at 260 so the engine's next-open fill lands
        // at 390 — the bar a session hold with `to` at 390 would fill on.
        let bars: Vec<Bar> = (0..4).map(|i| Bar::flat(i * 60_000 * 130, 100.0)).collect();
        let ind = fd_indicators::IndicatorSet::new();
        let params = RandomHold.default_params(); // 390 minutes
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        let at = |i: usize| BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: Some(open), params: &params };
        assert_eq!(RandomHold.on_bar(&at(1)), Intent::None, "130 minutes in, next bar at 260: still held");
        assert!(matches!(RandomHold.on_bar(&at(2)), Intent::Exit { .. }), "260 minutes in, next bar at 390: signal now");
    }

    #[test]
    fn the_window_null_fills_its_exit_on_the_same_bar_as_a_session_hold() {
        // Fifteen-minute bars from 15:30 (the fill of a 15:15 signal); a
        // 75-minute hold must signal on the 16:30 bar, not the 16:45 one.
        let t0 = 1_000_000_000_000i64;
        let bars: Vec<Bar> = (0..8).map(|i| Bar::flat(t0 + i * 15 * 60_000, 100.0)).collect();
        let ind = fd_indicators::IndicatorSet::new();
        let mut params = RandomHold.default_params();
        params.set("holdMinutes", 75.0);
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: t0, stop: None, target: None };
        let at = |i: usize| BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: Some(open), params: &params };
        assert_eq!(RandomHold.on_bar(&at(3)), Intent::None, "16:15: 45 minutes in, next bar at 60");
        assert!(matches!(RandomHold.on_bar(&at(4)), Intent::Exit { .. }), "16:30: 60 in, next bar at 75 — signal");
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

    /// Two hundred entry times spread over a few months, a minute apart at
    /// least so no two share a hash input.
    fn entry_times() -> impl Iterator<Item = u64> {
        (0..200u64).map(|i| 1_600_000_000_000 + i * 60_000 * 37)
    }

    #[test]
    fn a_zero_log_sd_is_the_fixed_hold_exactly() {
        for t in entry_times() {
            for seed in [1u64, 2, 99] {
                assert_eq!(hold_for(seed, t, 390.0, 0.0), 390.0, "seed {seed} at {t}: fixed hold, bit for bit");
                assert_eq!(hold_for(seed, t, 390.0, f64::NAN), 390.0, "an undeclared holdLogSd (NaN) is the fixed hold");
            }
        }
    }

    #[test]
    fn a_positive_log_sd_draws_a_log_normal_hold_pinned_by_entry_time_and_seed() {
        let logs: Vec<f64> = entry_times().map(|t| hold_for(7, t, 390.0, 1.0).ln()).collect();
        let n = logs.len() as f64;
        let mean = logs.iter().sum::<f64>() / n;
        let sd = (logs.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n).sqrt();
        let target = 390f64.ln();
        assert!((mean - target).abs() < 0.25 * target, "log-mean {mean:.3} should sit near ln(390) = {target:.3}");
        assert!((sd - 1.0).abs() < 0.25, "log-sd {sd:.3} should be near the 1.0 asked for");
        assert!(logs.iter().any(|l| *l < target) && logs.iter().any(|l| *l > target), "holds fall on both sides of the median");

        for t in entry_times() {
            assert_eq!(hold_for(7, t, 390.0, 1.0), hold_for(7, t, 390.0, 1.0), "same entry, same seed: same hold");
            assert_ne!(hold_for(7, t, 390.0, 1.0), hold_for(8, t, 390.0, 1.0), "another seed is another world");
        }
    }

    #[test]
    fn the_exit_honours_the_drawn_hold_on_every_bar() {
        let mut params = RandomHold.default_params();
        params.set("holdMinutes", 30.0);
        params.set("holdLogSd", 1.0);
        params.set("seed", 3.0);
        let entry: i64 = 1_600_000_000_000;
        let minutes = hold_for(3, entry as u64, 30.0, 1.0);
        assert_ne!(minutes, 30.0, "the draw moved the hold off its median");
        // The bar the engine's rule closes on: the first whole minute at or
        // past the drawn hold, truncated the way `on_bar` truncates.
        let closes_at = (1..).find(|k: &usize| (*k as i64) * 60_000 >= (minutes * 60_000.0) as i64).unwrap();
        let bars: Vec<Bar> = (0..=closes_at as i64).map(|i| Bar::flat(entry + i * 60_000, 100.0)).collect();
        let ind = fd_indicators::IndicatorSet::new();
        let open = OpenPosition { side: Side::Short, entry_price: 100.0, entry_time: entry, stop: None, target: None };
        let at = |i: usize| BarContext { bar: &bars[i], i, bars: &bars, ind: &ind, series: &[], options: None, position: Some(open), params: &params };
        // The signal comes one bar before the close so the next-open fill
        // lands on the closing minute.
        for i in 0..closes_at - 1 {
            assert_eq!(RandomHold.on_bar(&at(i)), Intent::None, "minute {i} of a {minutes:.2}-minute hold: still held");
        }
        assert!(matches!(RandomHold.on_bar(&at(closes_at - 1)), Intent::Exit { .. }), "minute {}: signal, fill at {closes_at}", closes_at - 1);
    }
}
