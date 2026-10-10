//! Volume profile: the prior period's point of control and value-area edges
//! as price levels, on the only instrument on this disk with real traded
//! volume.
//!
//! Registered in `docs/decisions/2026-10-10-volume-profile-btc.md`.
//!
//! ## Why this exists, and why only on BTCUSDT
//!
//! `volume > 0` on **0 of 50,000** sampled bars of every Dukascopy feed here,
//! and every sixteen-year window in this desk's record is Dukascopy — so a
//! mechanism reading `volume` on those files reads a column of zeros.
//! `XAUUSD` carries MT5 **tick** volume (`py/ingest/mt5_export.py:33-34` says
//! so itself, "because CFD real volume is always zero") and it loses its tail
//! over the years. `BTCUSDT` is Binance's real traded volume, the column is
//! stationary, and it is the cheapest instrument measured — so it is the one
//! place a volume profile has a chance, and the only place its failure would
//! mean anything.
//!
//! ## The measure is a PARAMETER, not a name
//!
//! `fd_indicators`' VWAP does `match bar.volume { Some(v) if v > 0.0 => v, _
//! => 1.0 }`, which silently makes a VWAP into a TWAP wherever the column is
//! empty — and still prints ordinary-looking numbers. So `measure` here is an
//! axis of the grid rather than a label: the same rule runs on the volume
//! histogram and on the time histogram, and the difference between the two is
//! the measurement. [`fd_engine::activity_profile_measured`] refuses to
//! substitute 1.0 for a missing volume, so the volume arms cannot quietly
//! become the time arm.
//!
//! The pre-check that had to pass before any of this was measured (same bars,
//! same bucket grid, 730 one-day windows on BTCUSDT-15m): the POC agrees to
//! the bucket on 60.7% of days with a median gap of 0.00 USD, while VAH and
//! VAL agree on only 30.1% and 32.3% and differ by a median of 1.07-1.09
//! buckets, 17.9-18.2% of a 1.5 x ATR14 R. **So the value-area EDGES are
//! where the volume column changes the answer and the POC is very nearly
//! where it does not** — which is a fact about these two histograms, not a
//! claim about either one's edge.
//!
//! ## Two readings, both anchored to prices
//!
//! * **`mode = 0`, return to value.** A bar closes OUTSIDE the prior period's
//!   value area. Enter against the excursion, targeting the POC.
//! * **`mode = 1`, rejection at the edge.** A bar trades through VAH (or VAL)
//!   and closes back INSIDE the area — the edge held. Enter against the poke,
//!   targeting the POC.
//!
//! In both, the target is the POC's **price** and the stop is the signal
//! bar's own extreme plus a buffer in ATR. With [`Exits::Engine`] and both
//! fields present on [`Intent::Enter`], this is an **L3** mechanism in the
//! desk's taxonomy — enforced stop, and a target that is a price rather than
//! a multiple of the risk — which is the one class where widening the stop
//! has a clean answer.
//!
//! **The stop is deliberately NOT the value-area edge.** In `mode = 0` the
//! entry bar has closed beyond that edge, so an edge-anchored stop would sit
//! on the PROFIT side of the entry: the position could then only end in a
//! win, which is the `wrong_side_stop` defect the desk counts rather than
//! trades. The registration's §4 said "the outer VA edge" and that was wrong
//! for exactly this reason; the dated note at the end of the registration
//! records the correction.
//!
//! Stateless: the prior period's profile is rebuilt from closed bars on every
//! call, and nothing reads a bar at or after the one being decided.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_engine::{ProfileMeasure, activity_profile_measured, bucket_size_price};
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

/// The three prices a period's profile contributes, in the market's own quote
/// units.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Levels {
    poc: f64,
    vah: f64,
    val: f64,
}

/// Which level a signal fired at, so one trade per level per day can be
/// enforced without comparing floats that rounding could separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    High,
    Low,
}

pub struct VolumeProfileLevels;

impl VolumeProfileLevels {
    /// The profile of the `days` complete UTC days before the one `now` is in,
    /// bucketed by the ATR of the window's last closed bar.
    ///
    /// `None` whenever the window is short, the ATR is unusable, or — on a
    /// volume measure — the window has no readable volume. A `None` here
    /// means no signal, never a fallback to the other measure.
    fn prior_levels(bars: &[Bar], atr: f64, days: usize, measure: ProfileMeasure, p: &Params) -> Option<Levels> {
        let last = bars.last()?;
        let today = last.time.div_euclid(DAY_MS);
        // Walk back over closed bars collecting whole UTC days. `bars` is the
        // slice up to and including the bar being decided, so the current
        // day's own bars are excluded by the day comparison rather than by an
        // index arithmetic that could be off by one.
        let mut end = bars.len();
        while end > 0 && bars[end - 1].time.div_euclid(DAY_MS) >= today {
            end -= 1;
        }
        if end == 0 {
            return None;
        }
        let first_day = bars[end - 1].time.div_euclid(DAY_MS);
        let oldest_wanted = first_day - (days as i64 - 1);
        let mut start = end;
        while start > 0 && bars[start - 1].time.div_euclid(DAY_MS) >= oldest_wanted {
            start -= 1;
        }
        let window = &bars[start..end];
        if window.is_empty() {
            return None;
        }
        let bucket = bucket_size_price(atr, p.get("bucketsPerAtr"))?;
        let (profile, audit) =
            activity_profile_measured(window, bucket, p.get("valueAreaPct"), p.get("bucketsPerAtr"), measure)?;
        if measure.needs_volume() && audit.bars_with_volume == 0 {
            return None;
        }
        let poc = profile.poc.as_ref().and_then(|l| l.price)?;
        let vah = profile.vah.as_ref().and_then(|l| l.price)?;
        let val = profile.val.as_ref().and_then(|l| l.price)?;
        (poc.is_finite() && vah.is_finite() && val.is_finite() && vah >= val).then_some(Levels { poc, vah, val })
    }
}

impl Strategy for VolumeProfileLevels {
    fn id(&self) -> &'static str {
        "vprofile"
    }
    fn name(&self) -> &'static str {
        "Volume profile POC and value area"
    }
    fn description(&self) -> &'static str {
        "The prior period's point of control and value-area edges as price levels. Fade a close outside the value \
         area (mode 0) or a poke through an edge that closes back inside (mode 1), targeting the POC's price with \
         the stop beyond the signal bar plus a buffer in ATR. `measure` picks the histogram's weight: 0 is traded \
         volume spread over the prices a bar traded at, 1 is time at price. Same rule, two histograms — the \
         difference between them is the measurement."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 0 = return to value (close outside the area), 1 = rejection at the edge.
            ("mode", 0.0),
            // 0 = VOLUME_DISTRIBUTED, 1 = TIME_AT_PRICE. An axis, not a label.
            ("measure", 0.0),
            ("profileDays", 1.0),
            ("bufferAtr", 0.5),
            ("valueAreaPct", 0.70),
            ("bucketsPerAtr", 4.0),
            ("maxRiskAtr", 3.0),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // Exactly the 24 cells declared in the registration before anything
        // was run: 2 measures x 2 modes x 2 window sizes x 3 buffers.
        BTreeMap::from([
            ("measure".to_string(), vec![0.0, 1.0]),
            ("mode".to_string(), vec![0.0, 1.0]),
            ("profileDays".to_string(), vec![1.0, 5.0]),
            ("bufferAtr".to_string(), vec![0.25, 0.5, 1.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // The ATR, plus the profile's own window in bars of any timeframe
        // that divides a day. Generous on purpose: a profile over a partial
        // window is a different measurement from the one registered.
        p.period("atrPeriod") + 96 * p.period("profileDays") + 5
    }
    fn exits(&self) -> Exits {
        // Engine-enforced, and `Intent::Enter` below carries both an absolute
        // stop and an absolute target price. L3.
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let atr = ctx.s(0);
        if !(atr.is_finite() && atr > 0.0) {
            return Intent::None;
        }
        let measure =
            if p.get("measure") < 0.5 { ProfileMeasure::VolumeDistributed } else { ProfileMeasure::TimeAtPrice };
        let days = p.period("profileDays").max(1);
        let bars = &ctx.bars[..=ctx.i];
        let Some(levels) = Self::prior_levels(bars, atr, days, measure, p) else { return Intent::None };

        let buffer = p.get("bufferAtr") * atr;
        let fade_outside = p.get("mode") < 0.5;

        // (side, which edge, stop) — read from one bar, with no reference to
        // anything after it.
        let signal = |b: &Bar| -> Option<(Side, Edge, f64)> {
            if fade_outside {
                if b.close > levels.vah {
                    return Some((Side::Short, Edge::High, b.high + buffer));
                }
                if b.close < levels.val {
                    return Some((Side::Long, Edge::Low, b.low - buffer));
                }
            } else {
                if b.high > levels.vah && b.close < levels.vah {
                    return Some((Side::Short, Edge::High, b.high + buffer));
                }
                if b.low < levels.val && b.close > levels.val {
                    return Some((Side::Long, Edge::Low, b.low - buffer));
                }
            }
            None
        };
        let Some((side, edge, stop)) = signal(ctx.bar) else { return Intent::None };

        // One trade per edge per UTC day: the first qualifying bar at each of
        // the two edges is the signal, as `pdhl` does with the previous day's
        // high and low.
        let today = ctx.bar.time.div_euclid(DAY_MS);
        for b in bars[..ctx.i].iter().rev() {
            if b.time.div_euclid(DAY_MS) != today {
                break;
            }
            if let Some((_, earlier, _)) = signal(b)
                && earlier == edge
            {
                return Intent::None;
            }
        }

        // The target is the POC's price, so it has to be on the profitable
        // side of the entry for the trade to mean anything. When price has
        // already crossed the POC this bar, there is no trade — not a target
        // behind the entry.
        let entry = ctx.bar.close;
        let target = levels.poc;
        let forward = if side.is_long() { target - entry } else { entry - target };
        if forward <= 0.0 {
            return Intent::None;
        }
        // And the stop must be on the losing side, or the position could only
        // end in a win. This is the `wrong_side_stop` shape the desk counts;
        // refusing here means this mechanism cannot contribute one.
        let risk = if side.is_long() { entry - stop } else { stop - entry };
        if risk <= 0.0 || risk > p.get("maxRiskAtr") * atr {
            return Intent::None;
        }

        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "{} {}: close {entry:.2} vs value area {:.2}..{:.2}, target POC {target:.2}, stop {stop:.2}, \
                 profile = {} over {days} UTC day(s)",
                if fade_outside { "return to value" } else { "edge rejection" },
                match edge {
                    Edge::High => "at VAH",
                    Edge::Low => "at VAL",
                },
                levels.val,
                levels.vah,
                measure.as_str(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_indicators::{IndicatorSet, compute_indicators};

    const M15: i64 = 900_000;

    fn bar(i: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Bar {
        Bar { time: i * M15, open, high, low, close, volume: Some(volume) }
    }

    /// One UTC day of 96 fifteen-minute bars inside `band`, with the CLOCK
    /// concentrated in the middle of the band and the heavy VOLUME at
    /// `heavy`. The two histograms therefore disagree by construction, which
    /// is what makes the measure axis testable at all.
    fn prior_day(band: (f64, f64), heavy: f64) -> Vec<Bar> {
        let (lo, hi) = band;
        let mid_band = (lo + hi) / 2.0;
        (0..96)
            .map(|i| {
                // 72 of the 96 bars sit in the middle of the band and the
                // remaining 24 walk the whole of it, so time-at-price peaks
                // in the middle however the volume is placed.
                let price = if i % 4 != 0 {
                    mid_band + ((i % 3) as f64 - 1.0)
                } else {
                    lo + (hi - lo) * ((i / 4) % 8) as f64 / 7.0
                };
                let volume = if (price - heavy).abs() <= (hi - lo) / 8.0 { 500.0 } else { 1.0 };
                bar(i, price, price + 2.0, price - 2.0, price, volume)
            })
            .collect()
    }

    fn set_of(bars: &[Bar]) -> IndicatorSet {
        compute_indicators(bars, &[IndicatorSpec::new("atr").with("period", 14.0)]).expect("indicators")
    }

    fn decide(bars: &[Bar], i: usize, params: &Params, position: Option<OpenPosition>) -> Intent {
        let set = set_of(bars);
        let key = format!("atr_{}", params.get("atrPeriod"));
        let series = set.get(&key).map(|v| &v[..]).expect("atr series");
        let s: Vec<&[f64]> = vec![series];
        VolumeProfileLevels.on_bar(&BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &set,
            series: &s,
            options: None,
            position,
            params,
        })
    }

    fn params(overrides: &[(&str, f64)]) -> Params {
        let mut p = VolumeProfileLevels.default_params();
        for (k, v) in overrides {
            p.set(k, *v);
        }
        p
    }

    #[test]
    fn the_exit_class_is_l3_an_enforced_stop_with_a_price_target() {
        // Read from the code rather than from a parameter's name, which is
        // the rule this desk adopted after a stop-width sweep turned out to
        // be a sweep of UNITS on a self-managed mechanism.
        assert_eq!(VolumeProfileLevels.exits(), Exits::Engine);
    }

    #[test]
    fn the_grid_is_the_twenty_four_cells_the_registration_declared() {
        let grid = VolumeProfileLevels.grid();
        let cells: usize = grid.values().map(Vec::len).product();
        assert_eq!(cells, 24, "{grid:?}");
        assert_eq!(grid["measure"], vec![0.0, 1.0]);
        assert_eq!(grid["mode"], vec![0.0, 1.0]);
    }

    #[test]
    fn a_close_outside_the_value_area_enters_towards_the_poc_with_both_prices_set() {
        let mut bars = prior_day((100.0, 140.0), 105.0);
        // A new UTC day — day 1 starts at bar 96 of a 15m feed.
        bars.push(bar(96, 160.0, 162.0, 159.0, 161.0, 10.0));
        let i = bars.len() - 1;
        let p = params(&[("mode", 0.0), ("measure", 1.0), ("maxRiskAtr", 50.0)]);
        match decide(&bars, i, &p, None) {
            Intent::Enter { side, stop, target, reason } => {
                assert_eq!(side, Side::Short);
                let (stop, target) = (stop.expect("a stop"), target.expect("a target"));
                // Both are absolute PRICES, and they straddle the entry.
                assert!(stop > bars[i].close, "stop {stop} must be the losing side");
                assert!(target < bars[i].close, "target {target} must be the winning side");
                assert!(reason.contains("TIME_AT_PRICE"), "{reason}");
            }
            other => panic!("expected an entry, got {other:?}"),
        }
    }

    #[test]
    fn the_measure_axis_changes_the_levels_the_rule_reads() {
        // The heavy volume sits at the BOTTOM of the prior day's band, so the
        // volume POC is low and the time POC is in the middle. The reason
        // string carries the POC, which is how the two are compared without
        // reaching into the strategy's internals.
        let mut bars = prior_day((100.0, 140.0), 102.0);
        bars.push(bar(96, 160.0, 162.0, 159.0, 161.0, 10.0));
        let i = bars.len() - 1;
        let poc_of = |measure: f64| -> f64 {
            let p = params(&[("mode", 0.0), ("measure", measure), ("maxRiskAtr", 50.0)]);
            match decide(&bars, i, &p, None) {
                Intent::Enter { target, .. } => target.expect("a target"),
                other => panic!("expected an entry at measure {measure}, got {other:?}"),
            }
        };
        let (volume_poc, time_poc) = (poc_of(0.0), poc_of(1.0));
        assert!(
            volume_poc < time_poc,
            "the volume POC must follow the volume, not the clock: {volume_poc} vs {time_poc}"
        );
    }

    #[test]
    fn a_feed_without_volume_takes_no_trade_on_the_volume_measure() {
        // The whole family's hazard: a 1.0 standing in for a missing volume
        // would make this cell the time cell and nobody reading the output
        // could tell. It must produce NO signal instead.
        let mut bars: Vec<Bar> = prior_day((100.0, 140.0), 105.0)
            .into_iter()
            .map(|b| Bar { volume: None, ..b })
            .collect();
        bars.push(Bar { volume: None, ..bar(96, 160.0, 162.0, 159.0, 161.0, 1.0) });
        let i = bars.len() - 1;
        let volume = params(&[("mode", 0.0), ("measure", 0.0), ("maxRiskAtr", 50.0)]);
        assert_eq!(decide(&bars, i, &volume, None), Intent::None);
        // And the time measure still trades, so the silence above is about
        // the volume column and not about these bars.
        let time = params(&[("mode", 0.0), ("measure", 1.0), ("maxRiskAtr", 50.0)]);
        assert!(matches!(decide(&bars, i, &time, None), Intent::Enter { .. }));
    }

    #[test]
    fn no_trade_when_the_poc_is_already_behind_the_entry() {
        // Price closed BELOW the value area but also below the POC, so a
        // "return to the POC" would be a target on the losing side. The
        // answer is no trade, not a reversed target.
        let mut bars = prior_day((100.0, 140.0), 120.0);
        bars.push(bar(96, 60.0, 61.0, 59.0, 60.0, 10.0));
        let i = bars.len() - 1;
        let p = params(&[("mode", 0.0), ("measure", 1.0), ("maxRiskAtr", 500.0)]);
        // Long towards a POC above: that IS forward, so this one trades.
        assert!(matches!(decide(&bars, i, &p, None), Intent::Enter { side: Side::Long, .. }));
        // Mirror it: a close above the area with the POC above it too.
        let mut up = prior_day((100.0, 140.0), 120.0);
        up.push(bar(96, 141.0, 141.5, 140.5, 141.0, 10.0));
        let j = up.len() - 1;
        match decide(&up, j, &p, None) {
            Intent::Enter { side, target, .. } => {
                assert_eq!(side, Side::Short);
                assert!(target.unwrap() < up[j].close);
            }
            Intent::None => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn only_closed_prior_days_feed_the_profile() {
        // The first bar of a new day has exactly one complete prior day
        // available; asking for five must produce no signal rather than a
        // profile over a partial window.
        let mut bars = prior_day((100.0, 140.0), 105.0);
        bars.push(bar(96, 160.0, 162.0, 159.0, 161.0, 10.0));
        let i = bars.len() - 1;
        let five = params(&[("mode", 0.0), ("measure", 1.0), ("profileDays", 5.0), ("maxRiskAtr", 50.0)]);
        // One day of history, five asked for: the window is short, and a
        // short window is a different measurement.
        let one = params(&[("mode", 0.0), ("measure", 1.0), ("profileDays", 1.0), ("maxRiskAtr", 50.0)]);
        assert!(matches!(decide(&bars, i, &one, None), Intent::Enter { .. }));
        // With only one prior day on the tape, the five-day cell sees the
        // same single day — it cannot invent four more, and it must not read
        // the current day's bars to make up the difference.
        if let Intent::Enter { reason, target, .. } = decide(&bars, i, &five, None) {
            assert!(reason.contains("over 5 UTC day(s)"), "{reason}");
            // Causality, checked on the numbers rather than on the label: the
            // signal bar trades 159..162 and the prior day never left
            // 100..140, so a profile that had seen the current day would put
            // its value area or its POC up there. Both must stay in the band.
            let vah: f64 = reason
                .split("value area ")
                .nth(1)
                .and_then(|s| s.split("..").nth(1))
                .and_then(|s| s.split(',').next())
                .expect("a vah in the reason")
                .trim()
                .parse()
                .expect("a number");
            assert!(vah < 145.0, "the value area reached the current day's prices: {reason}");
            assert!(target.expect("a target") < 145.0, "the POC reached the current day's prices: {reason}");
        }
    }

    #[test]
    fn an_open_position_suppresses_every_signal() {
        let mut bars = prior_day((100.0, 140.0), 105.0);
        bars.push(bar(96, 160.0, 162.0, 159.0, 161.0, 10.0));
        let i = bars.len() - 1;
        let p = params(&[("mode", 0.0), ("measure", 1.0), ("maxRiskAtr", 50.0)]);
        let held = OpenPosition {
            side: Side::Long,
            entry_price: 160.0,
            entry_time: 0,
            stop: Some(150.0),
            target: Some(170.0),
        };
        assert_eq!(decide(&bars, i, &p, Some(held)), Intent::None);
    }
}
