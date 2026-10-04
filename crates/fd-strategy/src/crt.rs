//! Candle Range Theory: a higher-timeframe candle's range, swept and rejected.
//!
//! Three candles of one chosen timeframe, which need not be the timeframe the
//! bars arrive on:
//!
//! 1. **Candle 1** forms a range — its own high and low.
//! 2. **Candle 2**, the very next candle, trades *strictly beyond* one extreme
//!    of that range and **closes strictly back inside** it. One extreme only:
//!    a candle that takes both is an outside candle, not a rejection of a
//!    side, and is discarded.
//! 3. **Candle 3** is the trade. A swept high is sold, a swept low is bought,
//!    the stop sits beyond candle 2's sweep extreme plus a buffer, and the
//!    target is candle 1's **opposite** extreme — a reward that is large when
//!    the sweep was deep and small when it was shallow, which is what makes
//!    this a different exit from a fixed multiple of risk rather than a
//!    different parameter.
//!
//! Registered in `docs/hypotheses/2026-10-04-crt.md` before this file existed.
//! Everything it declares is here and nothing it closed is a parameter:
//!
//! * `rangeTf` — how many entry bars make one range candle. 4 is 1H on 15m
//!   bars, 16 is 4H, 96 is 1D. Buckets are cut on absolute epoch time, the
//!   idiom [`crate::ict`] already uses, so a 1D bucket runs UTC midnight to
//!   UTC midnight and is **not** `pdhl`'s New York day.
//! * `target` — `0` candle 1's opposite extreme, `1` candle 1's 50% midpoint,
//!   `2` a constant `riskReward` multiple of the risk, which is `pdhl` mode
//!   0's exit and is in the batch as a control arm.
//! * `flipSides` — `1` reverses the direction while holding the risk and the
//!   reward distance identical, so the flipped row is the same trade shape
//!   pointing the other way. Falsifier F4's instrument; not a cell.
//!
//! **Where the entry is, and why it is not look-ahead.** The signal fires on
//! the last entry bar of candle 2's bucket, which the engine fills at the
//! **open of the next bar** — candle 2's close, one bar late at worst, never
//! early. "Last bar of the bucket" is decided from the bar's own timestamp and
//! the series' own step, by arithmetic: no later bar is read. If the next bar
//! is missing (a holiday, a gap in the feed) the setup is silently skipped
//! rather than entered at whatever reopens, which costs trades and cannot
//! invent one.
//!
//! **Not a lower-timeframe trigger, not a limit order, not a retrace entry.**
//! All three are forks CRT has and the registration closed; adding one would
//! be a tenth cell.
//!
//! Stateless, like every strategy here: both candles are rebuilt from the
//! closed bars on every call, so a replay from any bar gives the same answer.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct CandleRangeTheory;

/// One completed bucket of the range timeframe.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Candle {
    high: f64,
    low: f64,
    close: f64,
}

impl Strategy for CandleRangeTheory {
    fn id(&self) -> &'static str {
        "crt"
    }
    fn name(&self) -> &'static str {
        "Candle Range Theory"
    }
    fn description(&self) -> &'static str {
        "A higher-timeframe candle's range is swept on one side by the next candle, which closes back inside it; the \
         trade is taken against the sweep, stop beyond it, target candle 1's opposite extreme (target 0), its \
         midpoint (1), or a multiple of the risk (2)."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 4 = 1H on 15m bars, 16 = 4H, 96 = 1D. The batch sets it.
            ("rangeTf", 16.0),
            // 0 opposite extreme, 1 midpoint, 2 riskReward x risk.
            ("target", 0.0),
            ("flipSides", 0.0),
            // pdhl mode 0's own defaults, so a row differs from pdhl on the
            // range candle and the exit and on nothing else.
            ("bufferPips", 3.0),
            ("riskReward", 1.5),
            ("maxRiskAtr", 3.0),
            ("atrPeriod", 14.0),
            ("pipSize", 0.1),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // EMPTY ON PURPOSE. The registration declares nine cells on two axes
        // and forbids a tenth; a grid here would let a sweep add cells that
        // were never declared, which is the trap this whole programme is
        // about. The nine cells are rows of
        // `docs/hypotheses/2026-10-04-crt.toml`, not points of a grid.
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 2 * p.period("rangeTf") + 2
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let factor = p.period("rangeTf").max(1) as i64;
        let Some(step) = bar_step(ctx.bars) else { return Intent::None };
        let htf_ms = step * factor;
        let bar = ctx.bar;

        // Is this the last entry bar of its bucket? Decided from this bar's
        // own time and the series' own step; no later bar is read.
        let bucket = bar.time.div_euclid(htf_ms);
        if (bar.time + step).div_euclid(htf_ms) == bucket {
            return Intent::None;
        }

        // Candle 2 is the bucket now closing; candle 1 is the one before it.
        // Both must be complete in the bars we hold.
        let Some(c2) = bucket_ohlc(&ctx.bars[..=ctx.i], bucket, htf_ms) else { return Intent::None };
        let Some(c1) = bucket_ohlc(&ctx.bars[..=ctx.i], bucket - 1, htf_ms) else { return Intent::None };
        if !(c1.high > c1.low) {
            return Intent::None;
        }

        let took_high = c2.high > c1.high;
        let took_low = c2.low < c1.low;
        // An outside candle took both sides and rejected neither.
        if took_high == took_low {
            return Intent::None;
        }
        let closed_inside = if took_high { c2.close < c1.high } else { c2.close > c1.low };
        if !closed_inside {
            return Intent::None;
        }

        // The side the structure asks for, before any flip.
        let side = if took_high { Side::Short } else { Side::Long };
        let sweep_extreme = if took_high { c2.high } else { c2.low };
        let buffer = p.get("bufferPips") * p.get("pipSize");
        let stop = if took_high { sweep_extreme + buffer } else { sweep_extreme - buffer };

        // The reference price is candle 2's close, which is what `pdhl` uses
        // and is one bar before the fill.
        let entry = c2.close;
        let risk = (entry - stop).abs();
        if !(risk > 0.0) {
            return Intent::None;
        }
        let atr = ctx.s(0);
        if atr.is_finite() && risk > p.get("maxRiskAtr") * atr {
            return Intent::None;
        }

        // How far the target is, as a distance, so the flip can mirror it.
        let mode = p.period("target");
        let reward = match mode {
            // CRT's own target: candle 1's opposite extreme.
            0 => {
                let level = if took_high { c1.low } else { c1.high };
                (entry - level).abs()
            }
            // The 50% of candle 1's range. It can sit on the wrong side of
            // candle 2's close, in which case there is no trade.
            1 => {
                let mid = (c1.high + c1.low) / 2.0;
                let ahead = if took_high { mid < entry } else { mid > entry };
                if !ahead {
                    return Intent::None;
                }
                (entry - mid).abs()
            }
            // The control arm: a constant multiple of the risk, pdhl's exit.
            _ => risk * p.get("riskReward"),
        };
        if !(reward > 0.0) {
            return Intent::None;
        }

        // F4's mirror: the same risk and the same reward distance, pointing
        // the other way, so only the direction differs.
        let side = if p.get("flipSides") >= 0.5 {
            if side.is_long() { Side::Short } else { Side::Long }
        } else {
            side
        };
        let (stop, target) = if side.is_long() {
            (entry - risk, entry + reward)
        } else {
            (entry + risk, entry - reward)
        };

        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "swept the range candle's {} at {:.2}, closed back inside at {:.2}, {}",
                if took_high { "high" } else { "low" },
                if took_high { c1.high } else { c1.low },
                entry,
                match mode {
                    0 => format!("target the opposite extreme {:.2}", if took_high { c1.low } else { c1.high }),
                    1 => format!("target the range midpoint {:.2}", (c1.high + c1.low) / 2.0),
                    _ => format!("target {:.1}x the risk", p.get("riskReward")),
                }
            ),
        }
    }
}

/// High, low and close of one completed bucket, or `None` when this slice does
/// not hold the whole of it.
///
/// "The whole of it" means at least one bar inside the bucket and at least one
/// bar strictly before it, so a bucket cut short at the start of the series is
/// refused rather than measured from the bars that happen to be there.
fn bucket_ohlc(bars: &[Bar], bucket: i64, htf_ms: i64) -> Option<Candle> {
    let (mut high, mut low) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut close = None;
    let mut saw_earlier = false;
    for b in bars.iter().rev() {
        let k = b.time.div_euclid(htf_ms);
        if k > bucket {
            continue;
        }
        if k < bucket {
            saw_earlier = true;
            break;
        }
        if close.is_none() {
            close = Some(b.close);
        }
        high = high.max(b.high);
        low = low.min(b.low);
    }
    if !saw_earlier {
        return None;
    }
    close.map(|close| Candle { high, low, close })
}

/// The bar interval, from the first two bars that differ.
fn bar_step(bars: &[Bar]) -> Option<i64> {
    bars.windows(2).map(|w| w[1].time - w[0].time).find(|d| *d > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_indicators::compute_indicators;

    const MIN: i64 = 60_000;
    const FACTOR: i64 = 4; // 1H buckets on 15-minute bars

    /// `n` fifteen-minute bars from epoch, flat at 4400, so a bucket boundary
    /// falls every four bars at an exact hour.
    fn flat(n: usize) -> Vec<Bar> {
        (0..n)
            .map(|k| Bar {
                time: k as i64 * 15 * MIN,
                open: 4400.0,
                high: 4401.0,
                low: 4399.0,
                close: 4400.0,
                volume: None,
            })
            .collect()
    }

    fn params(target: f64) -> Params {
        let mut p = CandleRangeTheory.default_params();
        p.set("rangeTf", FACTOR as f64);
        p.set("target", target);
        p.set("maxRiskAtr", 1000.0); // the synthetic ATR is tiny; its own test is below
        p
    }

    fn intent_at(bars: &[Bar], p: &Params, i: usize) -> Intent {
        let s = CandleRangeTheory;
        let ind = compute_indicators(bars, &s.indicators(p)).unwrap();
        let keys = s.series(p);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &series, options: None, position: None, params: p };
        s.on_bar(&ctx)
    }

    /// The setup is placed late enough that ATR(14) is ready, because a NaN
    /// ATR silently disables the risk ceiling and a short fixture hid that
    /// from the first version of these tests.
    ///
    /// Candle 1 = bars 28..32, high 4420 and low 4380. Candle 2 = bars 32..36,
    /// which trades to 4425 and closes at 4410. The signal is bar 35, the last
    /// bar of candle 2's bucket.
    const C1: usize = 28;
    const C2: usize = 32;
    const SIGNAL: usize = 35;

    fn swept_high() -> Vec<Bar> {
        let mut bars = flat(40);
        bars[C1 + 1].high = 4420.0;
        bars[C1 + 2].low = 4380.0; // candle 1's low, the opposite extreme
        bars[C2 + 1].high = 4425.0;
        bars[SIGNAL].close = 4410.0;
        bars
    }

    #[test]
    fn the_signal_is_the_last_bar_of_candle_two_and_targets_the_opposite_extreme() {
        let bars = swept_high();
        // Not on the earlier bars of candle 2.
        for i in C2..SIGNAL {
            assert_eq!(intent_at(&bars, &params(0.0), i), Intent::None, "bar {i} is not the last of its bucket");
        }
        let intent = intent_at(&bars, &params(0.0), SIGNAL);
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Short);
        // Stop beyond candle 2's sweep extreme plus 3 pips at 0.1.
        assert!((stop.unwrap() - 4425.3).abs() < 1e-9, "stop was {stop:?}");
        // Target candle 1's low, which is 4380.
        assert!((target.unwrap() - 4380.0).abs() < 1e-9, "target was {target:?}");
    }

    #[test]
    fn a_candle_two_that_closes_outside_the_range_is_not_a_rejection() {
        let mut bars = swept_high();
        bars[SIGNAL].close = 4422.0; // above candle 1's high
        assert_eq!(intent_at(&bars, &params(0.0), SIGNAL), Intent::None);
    }

    #[test]
    fn an_outside_candle_took_both_sides_and_rejected_neither() {
        let mut bars = swept_high();
        bars[C2 + 2].low = 4379.0; // candle 2 also takes candle 1's low of 4380
        assert_eq!(intent_at(&bars, &params(0.0), SIGNAL), Intent::None);
    }

    #[test]
    fn a_candle_two_that_never_left_the_range_is_not_a_sweep() {
        let mut bars = swept_high();
        bars[C2 + 1].high = 4419.0; // inside candle 1's high of 4420
        assert_eq!(intent_at(&bars, &params(0.0), SIGNAL), Intent::None);
    }

    #[test]
    fn the_three_targets_are_three_different_prices_on_the_same_setup() {
        let bars = swept_high();
        let price = |target: f64| {
            let Intent::Enter { target: t, .. } = intent_at(&bars, &params(target), SIGNAL) else { panic!("no entry at target {target}") };
            t.unwrap()
        };
        let opposite = price(0.0);
        let mid = price(1.0);
        let rr = price(2.0);
        assert!((opposite - 4380.0).abs() < 1e-9);
        assert!((mid - 4400.0).abs() < 1e-9, "the midpoint of 4380..4420, which is 4400; got {mid}");
        // risk = 4425.3 - 4410 = 15.3; 1.5x below the close.
        assert!((rr - (4410.0 - 1.5 * 15.3)).abs() < 1e-9, "got {rr}");
        // On this setup the three are genuinely three prices, and CRT's own
        // target is the FURTHEST of them: 30.0 away against 22.95 for 1.5x the
        // risk and 10.0 for the midpoint. That is the whole point of the
        // variable target and the reason it is not a parameter tweak.
        assert!(opposite < rr && rr < mid, "expected opposite < rr < mid, got {opposite} {rr} {mid}");
        assert!((4410.0 - opposite - 30.0).abs() < 1e-9);
        assert!((4410.0 - rr - 22.95).abs() < 1e-9);
    }

    #[test]
    fn a_midpoint_already_behind_the_close_is_not_a_trade() {
        let mut bars = swept_high();
        // Close below the midpoint of 4380..4420: the midpoint is now above a
        // short's entry, so there is no reward in the trade's direction.
        bars[SIGNAL].close = 4390.0;
        assert_eq!(intent_at(&bars, &params(1.0), SIGNAL), Intent::None);
        // The opposite extreme is still ahead, so that cell still trades.
        assert!(matches!(intent_at(&bars, &params(0.0), SIGNAL), Intent::Enter { .. }));
    }

    #[test]
    fn the_flip_reverses_the_side_and_holds_the_risk_and_the_reward_distance() {
        let bars = swept_high();
        let Intent::Enter { side, stop, target, .. } = intent_at(&bars, &params(0.0), SIGNAL) else { panic!("no entry") };
        let mut flipped = params(0.0);
        flipped.set("flipSides", 1.0);
        let Intent::Enter { side: fside, stop: fstop, target: ftarget, .. } = intent_at(&bars, &flipped, SIGNAL) else {
            panic!("no flipped entry")
        };
        assert_eq!(side, Side::Short);
        assert_eq!(fside, Side::Long);
        let entry = 4410.0;
        assert!(((stop.unwrap() - entry) + (fstop.unwrap() - entry)).abs() < 1e-9, "the stops mirror");
        assert!(((target.unwrap() - entry) + (ftarget.unwrap() - entry)).abs() < 1e-9, "the targets mirror");
    }

    #[test]
    fn a_swept_low_is_bought_and_targets_the_range_high() {
        let mut bars = flat(40);
        bars[C1 + 1].low = 4380.0;
        bars[C1 + 2].high = 4420.0;
        bars[C2 + 1].low = 4375.0;
        bars[SIGNAL].close = 4390.0;
        let intent = intent_at(&bars, &params(0.0), SIGNAL);
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Long);
        assert!((stop.unwrap() - 4374.7).abs() < 1e-9);
        assert!((target.unwrap() - 4420.0).abs() < 1e-9);
    }

    #[test]
    fn the_risk_ceiling_in_atr_refuses_a_sweep_that_is_too_deep() {
        let bars = swept_high();
        let mut p = params(0.0);
        p.set("maxRiskAtr", 0.01); // ATR here is about 2 price units
        assert_eq!(intent_at(&bars, &p, SIGNAL), Intent::None);
    }

    #[test]
    fn the_signal_on_bar_i_does_not_change_when_later_bars_change() {
        let bars = swept_high();
        let before = intent_at(&bars, &params(0.0), SIGNAL);
        assert!(matches!(before, Intent::Enter { .. }));
        let mut altered = bars.clone();
        for b in &mut altered[SIGNAL + 1..] {
            b.high = 4600.0;
            b.low = 4200.0;
            b.close = 4590.0;
        }
        assert_eq!(intent_at(&altered, &params(0.0), SIGNAL), before);
    }

    #[test]
    fn a_range_candle_the_series_does_not_fully_hold_is_refused() {
        let mut bars = flat(40);
        bars[1].high = 4420.0;
        bars[2].high = 4425.0; // "candle 2" would be bucket 0 with no bucket before it
        bars[3].close = 4410.0;
        assert_eq!(intent_at(&bars, &params(0.0), 3), Intent::None, "bucket 0 has nothing before it");
    }

    #[test]
    fn the_bucket_boundary_is_utc_and_a_daily_range_is_not_a_new_york_day() {
        // 96 fifteen-minute bars is one UTC day. Its boundary falls at
        // 00:00 UTC, not at 00:00 New York, which is what pdhl uses. This is
        // asserted rather than described so the difference cannot be lost.
        let htf_ms = 96 * 15 * MIN;
        assert_eq!(htf_ms, 86_400_000);
        let midnight_utc = 0_i64;
        assert_eq!(midnight_utc.div_euclid(htf_ms), 0);
        // 05:00 UTC on the same day is still bucket 0, although New York is
        // still on the previous day at that moment.
        assert_eq!((5 * 3_600_000_i64).div_euclid(htf_ms), 0);
        // 23:45 UTC is the last bar of bucket 0.
        assert_eq!((86_400_000_i64 - 15 * MIN).div_euclid(htf_ms), 0);
    }
}
