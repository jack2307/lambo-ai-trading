//! Measurement harness for `agent/vprofile-gold`
//! (`docs/decisions/2026-10-10-vprofile-gold.md`).
//!
//! Arm A is the COMMITTED `activity_profile`, called here directly, so "the
//! existing profile" in the receipt is the function in the tree and not a
//! re-implementation of it. Arm B1/B2 is `volume_profile`, which differs from
//! it in exactly one line (the weight added per touched bucket).
//!
//! Bars come in as CSV via `FD_VPROFILE_BARS` so this crate gains no
//! dependency on `fd-store` / parquet. The CSV is dumped from
//! `data/bars/<SYMBOL>.parquet` by `vprofile-gold-dump.py` and carries the
//! file's own columns unchanged: `time,open,high,low,close,volume`, with an
//! empty `volume` field meaning the feed published none.
//!
//! Output: one CSV row per (window length x bucket scheme x arm) x window, to
//! `FD_VPROFILE_OUT`. Nothing here decides anything; the reading is done on
//! the rows.

use std::io::Write as _;

use fd_core::types::Bar;
use fd_engine::{VolumeWeighting, activity_profile, volume_profile};

const VALUE_AREA_PCT: f64 = 0.70; // fd-api/src/levels.rs:158
const ATR_PERIOD: usize = 14; // fd-api/src/levels.rs:221
const BUCKETS_PER_ATR: f64 = 4.0; // fd-api/src/levels.rs:154

fn read_bars(path: &str) -> Vec<Bar> {
    let text = std::fs::read_to_string(path).expect("bars csv");
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 6, "line {i}: {line}");
        out.push(Bar {
            time: f[0].parse().expect("time"),
            open: f[1].parse().expect("open"),
            high: f[2].parse().expect("high"),
            low: f[3].parse().expect("low"),
            close: f[4].parse().expect("close"),
            // EMPTY means the feed published no volume. Parsed to `None` and
            // never to `0.0` or `1.0`.
            volume: if f[5].trim().is_empty() { None } else { Some(f[5].parse().expect("volume")) },
        });
    }
    out
}

/// Split bars into runs separated by a gap of at least `gap_ms`, the same
/// idea `price_levels::runs_split_by_gap` uses for trading days.
fn day_runs(bars: &[Bar], gap_ms: i64) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    for i in 1..bars.len() {
        if bars[i].time - bars[i - 1].time >= gap_ms {
            runs.push((start, i));
            start = i;
        }
    }
    if start < bars.len() {
        runs.push((start, bars.len()));
    }
    runs
}

/// `activity_profile`'s own concentration measure, recomputed from the
/// histogram the same way for both arms so the pair stays comparable: the
/// share of the total in the POC bucket, and the normalised entropy.
///
/// Rebuilt here rather than returned from the two profile functions because
/// adding a field to `BarProfile` would change the committed route's JSON, and
/// arm A must stay byte-identical to what is in the tree.
fn shape(bars: &[Bar], bucket: f64, volume_weight: Option<VolumeWeighting>) -> Option<(f64, f64, f64, f64)> {
    let min_low = bars.iter().map(|b| b.low).filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
    let max_high = bars.iter().map(|b| b.high).filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    if !min_low.is_finite() || !max_high.is_finite() || max_high < min_low {
        return None;
    }
    let span = ((max_high - min_low) / bucket).floor();
    if !span.is_finite() || span < 0.0 || span > 5_000.0 {
        return None;
    }
    let count = span as usize + 1;
    let index_of = |p: f64| ((((p - min_low) / bucket).floor()).max(0.0) as usize).min(count - 1);
    let mut w = vec![0.0f64; count];
    for bar in bars {
        if !(bar.low.is_finite() && bar.high.is_finite()) || bar.high < bar.low {
            continue;
        }
        let lo = index_of(bar.low);
        let hi = index_of(bar.high);
        let per = match volume_weight {
            None => 1.0,
            Some(mode) => {
                let Some(v) = bar.volume.filter(|v| v.is_finite()) else { continue };
                match mode {
                    VolumeWeighting::PerTouchedBucket => v,
                    VolumeWeighting::SpreadOverTouchedBuckets => v / (hi - lo + 1) as f64,
                }
            }
        };
        for k in lo..=hi {
            w[k] += per;
        }
    }
    let total: f64 = w.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let peak = w.iter().copied().fold(0.0f64, f64::max) / total;
    let mut h = 0.0f64;
    for v in &w {
        let p = v / total;
        if p > 0.0 {
            h -= p * p.ln();
        }
    }
    let entropy = if count > 1 { h / (count as f64).ln() } else { 0.0 };
    Some((peak, entropy, min_low, max_high))
}

#[test]
fn dump_profile_rows_for_vprofile_gold() {
    let Ok(bars_path) = std::env::var("FD_VPROFILE_BARS") else {
        eprintln!("FD_VPROFILE_BARS unset: measurement harness skipped");
        return;
    };
    let out_path = std::env::var("FD_VPROFILE_OUT").expect("FD_VPROFILE_OUT");
    let bars = read_bars(&bars_path);
    assert!(bars.len() > 1_000, "only {} bars", bars.len());

    let atr = fd_indicators::atr(&bars, ATR_PERIOD);
    let bar_ms = bars[1].time - bars[0].time;
    // A trading day boundary is a hole of at least four bars; a week is two
    // days. Same shape as `trading_day_runs` in the levels route, written here
    // over the slice indices the harness needs.
    let runs = day_runs(&bars, bar_ms * 4);

    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path).expect("out"));
    writeln!(
        out,
        "window_days,bucket_scheme,arm,start_ms,end_ms,bars,bars_no_volume,bucket_size,buckets,\
poc,vah,val,min_low,max_high,atr14,total_weight,in_va_weight,peak_share,entropy_norm"
    )
    .unwrap();

    let mut rows = 0usize;
    for window_days in [1usize, 5usize] {
        let mut groups: Vec<(usize, usize)> = Vec::new();
        let mut i = 0usize;
        while i < runs.len() {
            let j = (i + window_days).min(runs.len());
            groups.push((runs[i].0, runs[j - 1].1));
            i = j;
        }

        for (lo, hi) in groups {
            let window = &bars[lo..hi];
            if window.len() < 10 {
                continue;
            }
            // ATR(14) at the window's LAST bar, which is what the levels route
            // uses (`atr14` = the last finite value of the series it publishes).
            let atr14 = atr[..hi].iter().rev().copied().find(|v| v.is_finite());
            let Some(atr14) = atr14 else { continue };
            if !(atr14 > 0.0) {
                continue;
            }

            for (scheme, bucket) in [
                ("ATR14_OVER_4", atr14 / BUCKETS_PER_ATR),
                ("FIXED_0.10_USD", 0.10),
                ("FIXED_0.01_USD_ONE_TICK", 0.01),
            ] {
                // Arm A: the committed function, untouched.
                if let Some(p) = activity_profile(window, bucket, VALUE_AREA_PCT, BUCKETS_PER_ATR) {
                    let s = shape(window, bucket, None);
                    writeln!(
                        out,
                        "{window_days},{scheme},A_TIME_AT_PRICE,{},{},{},0,{bucket},{},{},{},{},{},{},{atr14},{},{},{},{}",
                        p.window_start_bar_ms,
                        p.window_end_bar_ms,
                        p.window_bars,
                        p.buckets,
                        p.poc.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                        p.vah.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                        p.val.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                        s.map(|v| v.2).unwrap_or(f64::NAN),
                        s.map(|v| v.3).unwrap_or(f64::NAN),
                        p.activity_total_bar_buckets,
                        p.activity_in_value_area_bar_buckets,
                        s.map(|v| v.0).unwrap_or(f64::NAN),
                        s.map(|v| v.1).unwrap_or(f64::NAN),
                    )
                    .unwrap();
                    rows += 1;
                }

                for (label, mode) in [
                    ("B1_TICK_VOLUME_PER_BUCKET", VolumeWeighting::PerTouchedBucket),
                    ("B2_TICK_VOLUME_SPREAD", VolumeWeighting::SpreadOverTouchedBuckets),
                ] {
                    if let Some(p) = volume_profile(window, bucket, VALUE_AREA_PCT, BUCKETS_PER_ATR, mode) {
                        let s = shape(window, bucket, Some(mode));
                        writeln!(
                            out,
                            "{window_days},{scheme},{label},{},{},{},{},{bucket},{},{},{},{},{},{},{atr14},{},{},{},{}",
                            p.window_start_bar_ms,
                            p.window_end_bar_ms,
                            p.window_bars,
                            p.bars_without_volume,
                            p.buckets,
                            p.poc.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                            p.vah.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                            p.val.as_ref().and_then(|l| l.price).unwrap_or(f64::NAN),
                            s.map(|v| v.2).unwrap_or(f64::NAN),
                            s.map(|v| v.3).unwrap_or(f64::NAN),
                            p.total_weight,
                            p.in_value_area_weight,
                            s.map(|v| v.0).unwrap_or(f64::NAN),
                            s.map(|v| v.1).unwrap_or(f64::NAN),
                        )
                        .unwrap();
                        rows += 1;
                    }
                }
            }
        }
    }
    out.flush().unwrap();
    eprintln!("wrote {rows} rows to {out_path}");
    assert!(rows > 100, "only {rows} rows");
}

/// The one property that makes the A/B pair a one-variable comparison: with a
/// CONSTANT volume on every bar, `volume_profile` under
/// [`VolumeWeighting::PerTouchedBucket`] must land on the same POC, VAH and
/// VAL as `activity_profile`, because a constant weight per touched bucket IS
/// the time profile scaled.
///
/// This is also the measurement of the `unwrap_or(1.0)` defect in
/// `fd_store::resample` and in `fd_indicators::vwap`: substituting a constant
/// for a missing volume does not approximate a volume profile, it REPRODUCES
/// the time profile.
#[test]
fn a_constant_volume_reproduces_the_time_profile_exactly() {
    let mut bars = Vec::new();
    let mut price = 100.0f64;
    for i in 0..400i64 {
        let drift = ((i as f64) * 0.37).sin() * 2.0;
        let open = price;
        let close = price + drift * 0.4;
        let high = open.max(close) + 0.3;
        let low = open.min(close) - 0.3;
        bars.push(Bar { time: i * 900_000, open, high, low, close, volume: Some(7.0) });
        price = close;
    }

    let time = activity_profile(&bars, 0.25, 0.70, 4.0).expect("time profile");
    let vol = volume_profile(&bars, 0.25, 0.70, 4.0, VolumeWeighting::PerTouchedBucket).expect("volume profile");

    assert_eq!(
        time.poc.as_ref().and_then(|l| l.price),
        vol.poc.as_ref().and_then(|l| l.price),
        "a constant volume must not move the POC"
    );
    assert_eq!(time.vah.as_ref().and_then(|l| l.price), vol.vah.as_ref().and_then(|l| l.price));
    assert_eq!(time.val.as_ref().and_then(|l| l.price), vol.val.as_ref().and_then(|l| l.price));
    // And the totals differ by exactly the constant, which is what "the time
    // profile scaled" means as a number.
    let ratio = vol.total_weight / time.activity_total_bar_buckets;
    assert!((ratio - 7.0).abs() < 1e-9, "ratio {ratio}");
}

/// A bar the feed published no volume for is counted and skipped, never
/// substituted. On a window where NO bar carries volume there is no profile at
/// all — `None`, not an empty histogram and not a time profile in disguise.
#[test]
fn a_window_with_no_published_volume_has_no_volume_profile() {
    let bars: Vec<Bar> = (0..100i64)
        .map(|i| Bar {
            time: i * 900_000,
            open: 100.0,
            high: 100.5,
            low: 99.5,
            close: 100.0,
            volume: None,
        })
        .collect();
    assert!(activity_profile(&bars, 0.25, 0.70, 4.0).is_some(), "time profile still exists");
    assert!(
        volume_profile(&bars, 0.25, 0.70, 4.0, VolumeWeighting::PerTouchedBucket).is_none(),
        "no volume published means no volume profile"
    );

    // Half published: the profile exists and says how many bars it left out.
    let mut mixed = bars.clone();
    for (i, bar) in mixed.iter_mut().enumerate() {
        if i % 2 == 0 {
            bar.volume = Some(1.0 + i as f64);
        }
    }
    let p = volume_profile(&mixed, 0.25, 0.70, 4.0, VolumeWeighting::PerTouchedBucket).expect("profile");
    assert_eq!(p.window_bars, 50);
    assert_eq!(p.bars_without_volume, 50);
    assert_eq!(p.measure, "TICK_VOLUME_AT_PRICE");
}

/// The weight is the only difference: a single bar with a large volume must be
/// able to move the POC away from where the time profile puts it, or the
/// comparison in precheck 1 could never detect anything.
#[test]
fn volume_can_move_the_poc_away_from_the_time_poc() {
    let mut bars: Vec<Bar> = Vec::new();
    // Twenty quiet bars parked at 100.0 — the time POC.
    for i in 0..20i64 {
        bars.push(Bar { time: i * 900_000, open: 100.0, high: 100.2, low: 99.8, close: 100.0, volume: Some(1.0) });
    }
    // One bar parked at 110.0 carrying a thousand times the ticks.
    bars.push(Bar { time: 20 * 900_000, open: 110.0, high: 110.2, low: 109.8, close: 110.0, volume: Some(1_000.0) });

    let time = activity_profile(&bars, 0.1, 0.70, 4.0).expect("time");
    let vol = volume_profile(&bars, 0.1, 0.70, 4.0, VolumeWeighting::PerTouchedBucket).expect("volume");
    let tp = time.poc.as_ref().and_then(|l| l.price).unwrap();
    let vp = vol.poc.as_ref().and_then(|l| l.price).unwrap();
    assert!((tp - 100.0).abs() < 0.3, "time poc {tp}");
    assert!((vp - 110.0).abs() < 0.3, "volume poc {vp}");
}
