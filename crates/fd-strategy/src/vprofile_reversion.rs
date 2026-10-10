//! Value-area reversion on the price profile.
//!
//! A bar that closes outside the prior profile window's value area is a bar
//! that left the band where most of the period's activity sat; the rule enters
//! back toward the point of control and takes profit AT it. That makes the
//! target a PRICE LEVEL rather than a multiple of the risk, which is the only
//! one of the three stop classes with a clean answer to "what does moving the
//! stop do" (`AGENT-BRIEF-ADDENDUM-8` section III, class L3).
//!
//! ## The measure is a parameter, and that is the whole point
//!
//! `measure` chooses which histogram the value area comes from:
//!
//! * `0` — [`fd_engine::activity_profile`], the COMMITTED time-at-price
//!   profile, which does not read `volume` at all. This is the control arm.
//! * `1` — [`fd_engine::volume_profile`] under
//!   [`VolumeWeighting::PerTouchedBucket`]: the same histogram with the bar's
//!   `volume` as the weight instead of `1.0`. One variable changed.
//! * `2` — the same under [`VolumeWeighting::SpreadOverTouchedBuckets`].
//!
//! Both volume arms read MT5 **tick** volume — the number of price changes in
//! the bar, not size; `py/ingest/mt5_export.py` records that in the parquet
//! file's own metadata because CFD real volume is always zero. Every Dukascopy
//! feed on this desk publishes `volume = 0` or `null`, so arms 1 and 2 exist
//! only on `xauusd:15m`, which is 2022-06-16 onwards. Registered in
//! `docs/decisions/2026-10-10-vprofile-gold.md`.
//!
//! ## Causality
//!
//! The profile is built from COMPLETE prior trading days only — the current
//! day is never in its own window. `warmup` covers the deepest window the
//! parameters allow.
//!
//! One trade per side per day: the first qualifying bar at each edge is the
//! signal, and a later bar that qualifies the same way is not.

use std::collections::BTreeMap;

use fd_core::clock::new_york_offset_ms;
use fd_core::types::Bar;
use fd_engine::{VolumeWeighting, activity_profile, bucket_size_price, volume_profile};
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const DAY_MS: i64 = 86_400_000;

pub struct ValueAreaReversion;

/// The three levels a profile window yields, whichever measure built it.
struct Area {
    poc: f64,
    vah: f64,
    val: f64,
}

impl Strategy for ValueAreaReversion {
    fn id(&self) -> &'static str {
        "vprofile-reversion"
    }
    fn name(&self) -> &'static str {
        "Value-area reversion"
    }
    fn description(&self) -> &'static str {
        "Enter back toward the prior profile window's point of control when a bar closes outside its value area; \
         target the point of control, stop beyond the signal bar. `measure` picks the time profile (0) or the \
         tick-volume profile (1, 2)."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 0 = time at price (control), 1 = tick volume per touched bucket,
            // 2 = tick volume spread over touched buckets.
            ("measure", 1.0),
            ("windowDays", 1.0),
            // How far outside the value area the close must be, in ATR of this
            // timeframe. 0 = any close outside.
            ("thresholdAtr", 0.0),
            ("bucketsPerAtr", 4.0),
            ("valueAreaPct", 0.70),
            ("bufferAtr", 0.10),
            ("maxRiskAtr", 3.0),
            // The target is the POC, so a window whose POC is closer than this
            // many ATR is not worth the spread and is skipped.
            ("minTargetAtr", 0.25),
            ("atrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("thresholdAtr".to_string(), vec![0.0, 0.25, 0.50]),
            ("windowDays".to_string(), vec![1.0, 5.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", p.get("atrPeriod"))]
    }
    fn warmup(&self, p: &Params) -> usize {
        // The deepest window the parameters allow, in bars, plus the ATR.
        // A trading day of 15m gold is 96 bars; take 100 to be safe on a
        // short holiday session.
        p.period("atrPeriod") + 100 * p.period("windowDays").max(1) + 5
    }
    fn exits(&self) -> Exits {
        // Enforced: the engine holds the stop and the target. `check_exit`
        // returns early on `self_managed` positions, so an `Exits::Strategy`
        // method's "stop" would only be the denominator of R
        // (`AGENT-BRIEF-ADDENDUM-7` section B). This one is a real stop.
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
        let bars = &ctx.bars[..=ctx.i];
        let bar = ctx.bar;
        let today = ny_day(bar.time);
        let days = p.period("windowDays").max(1);

        let Some(window) = prior_days(bars, today, days) else { return Intent::None };
        let Some(area) = area_of(&bars[window], atr, p) else { return Intent::None };

        let threshold = p.get("thresholdAtr") * atr;
        let buffer = p.get("bufferAtr") * atr;

        // Which edge, if any, a bar qualifies at. Pure in the bar, so the
        // once-per-day rule below can ask it about earlier bars too.
        let edge = |b: &Bar| -> Option<Side> {
            if b.close > area.vah + threshold {
                Some(Side::Short)
            } else if b.close < area.val - threshold {
                Some(Side::Long)
            } else {
                None
            }
        };
        let Some(side) = edge(bar) else { return Intent::None };

        // First signal of this side today only.
        for b in bars[..ctx.i].iter().rev() {
            if ny_day(b.time) != today {
                break;
            }
            if edge(b) == Some(side) {
                return Intent::None;
            }
        }

        // The POC must be on the far side of the entry, far enough to be worth
        // the round trip. A close above the value area whose POC is also above
        // the close is not a reversion setup — it is a window whose histogram
        // disagrees with its own value area, and taking it would be reading the
        // rule backwards.
        let distance = area.poc - bar.close;
        match side {
            Side::Short if distance >= -p.get("minTargetAtr") * atr => return Intent::None,
            Side::Long if distance <= p.get("minTargetAtr") * atr => return Intent::None,
            _ => {}
        }

        let stop = match side {
            Side::Short => bar.high + buffer,
            Side::Long => bar.low - buffer,
        };
        let risk = (bar.close - stop).abs();
        if !(risk > 0.0) || risk > p.get("maxRiskAtr") * atr {
            return Intent::None;
        }

        Intent::Enter {
            side,
            stop: Some(stop),
            target: Some(area.poc),
            reason: format!(
                "closed {} the {} value area ({:.2}..{:.2}) of the prior {days} day(s) on measure {}; target POC {:.2}",
                if side.is_long() { "below" } else { "above" },
                measure_label(p.get("measure")),
                area.val,
                area.vah,
                measure_label(p.get("measure")),
                area.poc
            ),
        }
    }
}

fn measure_label(measure: f64) -> &'static str {
    match measure_mode(measure) {
        None => "TIME_AT_PRICE",
        Some(VolumeWeighting::PerTouchedBucket) => "TICK_VOLUME_AT_PRICE/PER_TOUCHED_BUCKET",
        Some(VolumeWeighting::SpreadOverTouchedBuckets) => "TICK_VOLUME_AT_PRICE/SPREAD_OVER_TOUCHED_BUCKETS",
    }
}

/// `measure` as a weighting, or `None` for the committed time profile.
///
/// Anything outside the declared set falls back to the time profile, which is
/// the CONTROL and not the candidate — a typo in a batch file then reads as
/// "no volume was used" rather than quietly picking a volume arm.
fn measure_mode(measure: f64) -> Option<VolumeWeighting> {
    if measure >= 1.5 {
        Some(VolumeWeighting::SpreadOverTouchedBuckets)
    } else if measure >= 0.5 {
        Some(VolumeWeighting::PerTouchedBucket)
    } else {
        None
    }
}

/// The value area of `window`, from whichever histogram `measure` names.
fn area_of(window: &[Bar], atr: f64, p: &Params) -> Option<Area> {
    let bucket = bucket_size_price(atr, p.get("bucketsPerAtr"))?;
    let pct = p.get("valueAreaPct");
    let (poc, vah, val) = match measure_mode(p.get("measure")) {
        None => {
            let prof = activity_profile(window, bucket, pct, p.get("bucketsPerAtr"))?;
            (prof.poc?.price?, prof.vah?.price?, prof.val?.price?)
        }
        Some(mode) => {
            let prof = volume_profile(window, bucket, pct, p.get("bucketsPerAtr"), mode)?;
            (prof.poc?.price?, prof.vah?.price?, prof.val?.price?)
        }
    };
    (poc.is_finite() && vah.is_finite() && val.is_finite() && vah >= val).then_some(Area { poc, vah, val })
}

/// Index range of the last `days` COMPLETE New York days before `today`.
///
/// Walks back from the newest closed bar rather than splitting the whole
/// series, so the cost is the window and not the history — `on_bar` runs once
/// per bar per parameter cell.
fn prior_days(bars: &[Bar], today: i64, days: usize) -> Option<std::ops::Range<usize>> {
    let mut end = None;
    let mut start = None;
    let mut seen = 0usize;
    let mut current = i64::MIN;
    for i in (0..bars.len()).rev() {
        let d = ny_day(bars[i].time);
        if d >= today {
            continue;
        }
        if end.is_none() {
            end = Some(i + 1);
            current = d;
            seen = 1;
        } else if d != current {
            seen += 1;
            current = d;
            if seen > days {
                start = Some(i + 1);
                break;
            }
        }
        if seen <= days {
            start = Some(i);
        }
    }
    match (start, end) {
        (Some(s), Some(e)) if e > s && seen >= days => Some(s..e),
        _ => None,
    }
}

fn ny_day(utc_ms: i64) -> i64 {
    (utc_ms + new_york_offset_ms(utc_ms)).div_euclid(DAY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `days` consecutive CALENDAR days, each a short session of
    /// `bars_per_day` 15m bars starting at the same hour. The gap between
    /// sessions is therefore a real day boundary under [`ny_day`] — a fixture
    /// whose "days" are only a few hours apart would collapse into one New
    /// York day and test nothing.
    fn bars_over_days(days: usize, bars_per_day: usize) -> Vec<Bar> {
        let mut out = Vec::new();
        let first = 1_655_375_400_000i64; // 2022-06-16 10:30 UTC, the feed's first bar
        for d in 0..days {
            let mut t = first + d as i64 * DAY_MS;
            for k in 0..bars_per_day {
                let price = 1800.0 + d as f64 * 10.0 + (k % 7) as f64 * 0.5;
                out.push(Bar {
                    time: t,
                    open: price,
                    high: price + 0.4,
                    low: price - 0.4,
                    close: price,
                    volume: Some(100.0 + k as f64),
                });
                t += 900_000;
            }
        }
        out
    }

    #[test]
    fn the_window_is_prior_complete_days_and_never_today() {
        let bars = bars_over_days(4, 20);
        let today = ny_day(bars[bars.len() - 1].time);
        let r = prior_days(&bars, today, 1).expect("one prior day");
        // Every bar in the window is from a strictly earlier New York day.
        for b in &bars[r.clone()] {
            assert!(ny_day(b.time) < today, "bar at {} is not a prior day", b.time);
        }
        let r5 = prior_days(&bars, today, 3).expect("three prior days");
        assert!(r5.len() > r.len(), "a deeper window must hold more bars");
        for b in &bars[r5] {
            assert!(ny_day(b.time) < today);
        }
        // Not enough history: no window, so no trade, rather than a short one.
        assert!(prior_days(&bars[..20], ny_day(bars[5].time), 1).is_none());
    }

    #[test]
    fn measure_zero_is_the_committed_time_profile_and_an_unknown_value_falls_back_to_it() {
        assert!(measure_mode(0.0).is_none());
        assert_eq!(measure_mode(1.0), Some(VolumeWeighting::PerTouchedBucket));
        assert_eq!(measure_mode(2.0), Some(VolumeWeighting::SpreadOverTouchedBuckets));
        // Below the first threshold, including a negative typo, is the control.
        assert!(measure_mode(-3.0).is_none());
        assert!(measure_mode(0.4).is_none());
        assert_eq!(measure_label(0.0), "TIME_AT_PRICE");
    }

    #[test]
    fn the_three_measures_can_disagree_about_the_value_area() {
        // A window where one bar carries almost all the ticks at a price the
        // bar count barely visits. If the three measures could not disagree
        // here, the gate would be measuring one arm three times.
        let mut bars: Vec<Bar> = (0..40i64)
            .map(|i| Bar {
                time: i * 900_000,
                open: 1800.0,
                high: 1800.3,
                low: 1799.7,
                close: 1800.0,
                volume: Some(1.0),
            })
            .collect();
        bars.push(Bar {
            time: 40 * 900_000,
            open: 1810.0,
            high: 1810.3,
            low: 1809.7,
            close: 1810.0,
            volume: Some(100_000.0),
        });
        let atr = 1.0;
        let mk = |measure: f64| {
            let p = Params::new(&[("measure", measure), ("bucketsPerAtr", 4.0), ("valueAreaPct", 0.70)]);
            area_of(&bars, atr, &p).map(|a| a.poc)
        };
        let time = mk(0.0).expect("time poc");
        let b1 = mk(1.0).expect("b1 poc");
        assert!((time - 1800.0).abs() < 0.5, "time poc {time}");
        assert!((b1 - 1810.0).abs() < 0.5, "tick-volume poc {b1}");
    }

    #[test]
    fn a_window_with_no_published_volume_yields_no_volume_area_but_still_a_time_area() {
        let bars: Vec<Bar> = (0..40i64)
            .map(|i| Bar { time: i * 900_000, open: 1800.0, high: 1800.5, low: 1799.5, close: 1800.0, volume: None })
            .collect();
        let time = Params::new(&[("measure", 0.0), ("bucketsPerAtr", 4.0), ("valueAreaPct", 0.70)]);
        let vol = Params::new(&[("measure", 1.0), ("bucketsPerAtr", 4.0), ("valueAreaPct", 0.70)]);
        assert!(area_of(&bars, 1.0, &time).is_some());
        assert!(area_of(&bars, 1.0, &vol).is_none(), "no volume means no volume value area");
    }
}
