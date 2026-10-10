//! The pre-check that decides whether a volume profile is a thing at all.
//!
//! Registered in `docs/decisions/2026-10-10-volume-profile-btc.md` §3. Three
//! profiles over the SAME bars on the SAME bucket grid, differing only in the
//! per-bar weight — time at price (what `activity_profile` has always
//! published), volume distributed over the buckets a bar touched (the standard
//! volume profile), and volume per touched bucket. Then the one question:
//! **are the POC and the value-area edges of the volume profile distinguishable
//! from the time profile's?**
//!
//! Measured as a price distance, not by eye, and reported in four units —
//! USD, buckets, ticks, and percent of 1.5 x ATR14, which is a typical R on
//! this desk. A distance far below one R cannot be separated by any gate on a
//! 40-trade sample, and that is a result rather than a nuisance: it would mean
//! the volume column adds nothing and `vwap_fade`'s silent fallback
//! (`fd-indicators/src/lib.rs:800`, `_ => 1.0`) is the whole family's story.
//!
//!   vprofile_precheck --market=btc --interval=15m --data=E:/rust/flowdesk/data \
//!     --profile-days=1 --buckets-per-atr=4 --value-area=0.70 --tick=0.01

use fd_core::types::Bar;
use fd_engine::{ProfileMeasure, activity_profile_measured, bucket_size_price};
use fd_indicators::{IndicatorSpec, compute_indicators};
use fd_store::read_bars;

const DAY_MS: i64 = 86_400_000;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn argf(name: &str, fallback: f64) -> f64 {
    arg(name, &fallback.to_string()).parse().unwrap_or(fallback)
}

/// Percentile of a sorted slice, by the nearest-rank rule. Stated rather than
/// inherited so two readers get the same number from the same data.
fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn describe(label: &str, mut values: Vec<f64>, bucket: f64, tick: f64, r_usd: f64) {
    if values.is_empty() {
        println!("  {label:<26} no windows — NOT MEASURED, and that is not a zero");
        return;
    }
    values.sort_by(f64::total_cmp);
    let p50 = pct(&values, 50.0);
    let p90 = pct(&values, 90.0);
    let p99 = pct(&values, 99.0);
    let max = *values.last().unwrap();
    let zero = values.iter().filter(|v| **v <= 0.0).count();
    println!(
        "  {label:<26} n={:<5} same-bucket {:>5.1}%  |d| USD p50 {:>8.2} p90 {:>9.2} p99 {:>9.2} max {:>9.2}",
        values.len(),
        100.0 * zero as f64 / values.len() as f64,
        p50,
        p90,
        p99,
        max
    );
    println!(
        "  {:<26} in buckets p50 {:>6.2} p90 {:>6.2} | in ticks p50 {:>10.0} p90 {:>11.0} | % of 1.5xATR p50 {:>6.2}% p90 {:>6.2}%",
        "",
        p50 / bucket,
        p90 / bucket,
        p50 / tick,
        p90 / tick,
        100.0 * p50 / r_usd,
        100.0 * p90 / r_usd
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = arg("data", "E:/rust/flowdesk/data");
    let symbol = arg("symbol", "BTCUSDT");
    let interval = arg("interval", "15m");
    let profile_days = argf("profile-days", 1.0).max(1.0) as i64;
    let buckets_per_atr = argf("buckets-per-atr", 4.0);
    let value_area = argf("value-area", 0.70);
    let tick = argf("tick", 0.01);
    let stop_atr = argf("stop-atr", 1.5);

    let path = std::path::Path::new(&data).join("bars").join(format!("{symbol}-{interval}.parquet"));
    let bars = read_bars(&path)?;
    println!("vprofile pre-check — registered 2026-10-10, spends ZERO gate cells");
    println!("bars:     {} from {}", bars.len(), path.display());
    if bars.is_empty() {
        println!("no bars — NOT MEASURED");
        return Ok(());
    }
    println!(
        "span:     {} -> {} (epoch ms), profile window = {profile_days} UTC day(s)",
        bars[0].time,
        bars[bars.len() - 1].time
    );

    // The one counter that has to print before any distance is read: the
    // registration's F3. A volume arm reading an empty column would still
    // produce a histogram if it substituted 1.0 per bar, and that histogram
    // would be the time profile wearing the other name.
    let with_volume = bars.iter().filter(|b| b.volume.is_some_and(|v| v.is_finite() && v > 0.0)).count();
    let volume_sum: f64 = bars.iter().filter_map(|b| b.volume).filter(|v| v.is_finite() && *v > 0.0).sum();
    println!(
        "F3 audit: volume readable on {with_volume}/{} bars ({:.3}%), sum = {volume_sum:.3} in the feed's units",
        bars.len(),
        100.0 * with_volume as f64 / bars.len() as f64
    );
    if with_volume == 0 {
        println!("F3 FIRED: no readable volume on this feed — the volume arms CANNOT be built, and any");
        println!("          histogram claiming to be one here would be the time profile under another name.");
        return Ok(());
    }

    let atr_period = 14.0;
    let set = compute_indicators(&bars, &[IndicatorSpec::new("atr").with("period", atr_period)])?;
    // The key the set actually holds, not the key I guessed: a naming mistake
    // here would otherwise hand every window a NaN ATR and print a pre-check
    // over zero windows.
    let atr = set
        .get("atr_14")
        .or_else(|| set.get("atr_14:atr"))
        .map(|v| &v[..])
        .ok_or_else(|| {
            let mut keys: Vec<&String> = set.keys().collect();
            keys.sort();
            format!("no ATR series; the set holds {keys:?}")
        })?;
    let finite_atr = atr.iter().filter(|v| v.is_finite() && **v > 0.0).count();
    println!("atr:      {finite_atr}/{} bars with a finite positive ATR14", atr.len());

    // Group bars into UTC day buckets, then walk day by day: the profile is
    // built on the PRIOR `profile_days` of CLOSED bars only. Nothing here
    // reads a bar at or after the day it would be traded in.
    let day_of = |t: i64| t.div_euclid(DAY_MS);
    let mut days: Vec<(i64, usize, usize)> = Vec::new(); // (day, start, end exclusive)
    for (i, b) in bars.iter().enumerate() {
        let d = day_of(b.time);
        match days.last_mut() {
            Some(last) if last.0 == d => last.2 = i + 1,
            _ => days.push((d, i, i + 1)),
        }
    }
    println!("days:     {} UTC day groups", days.len());

    let mut poc_d: Vec<f64> = Vec::new();
    let mut vah_d: Vec<f64> = Vec::new();
    let mut val_d: Vec<f64> = Vec::new();
    let mut poc_t: Vec<f64> = Vec::new();
    let mut buckets_seen: Vec<f64> = Vec::new();
    let mut r_seen: Vec<f64> = Vec::new();
    let mut skipped_no_atr = 0usize;
    let mut skipped_no_profile = 0usize;
    let mut windows = 0usize;
    // Does the volume measure ever change which bucket is the POC? The share
    // of windows where it does is the headline of this pre-check.
    let mut poc_moved = 0usize;

    for w in (profile_days as usize)..days.len() {
        let start = days[w - profile_days as usize].1;
        let end = days[w - 1].2;
        if end <= start {
            continue;
        }
        let window: &[Bar] = &bars[start..end];
        // ATR at the LAST closed bar of the window — the same bar a strategy
        // would have, and the series whose value the bucket size is derived
        // from, so the derivation can be checked rather than believed.
        let a = atr.get(end - 1).copied().unwrap_or(f64::NAN);
        let Some(bucket) = bucket_size_price(a, buckets_per_atr) else {
            skipped_no_atr += 1;
            continue;
        };
        let r_usd = stop_atr * a;

        let time = activity_profile_measured(window, bucket, value_area, buckets_per_atr, ProfileMeasure::TimeAtPrice);
        let dist =
            activity_profile_measured(window, bucket, value_area, buckets_per_atr, ProfileMeasure::VolumeDistributed);
        let touch =
            activity_profile_measured(window, bucket, value_area, buckets_per_atr, ProfileMeasure::VolumeTouched);
        let (Some((t, _)), Some((d, da)), Some((h, _))) = (time, dist, touch) else {
            skipped_no_profile += 1;
            continue;
        };
        // The volume arm must have read volume on this window, or its numbers
        // are not about volume.
        if da.bars_with_volume == 0 {
            skipped_no_profile += 1;
            continue;
        }
        windows += 1;
        buckets_seen.push(bucket);
        r_seen.push(r_usd);

        let price = |l: &Option<fd_engine::PriceLevel>| l.as_ref().and_then(|x| x.price).unwrap_or(f64::NAN);
        let (tp, dp, hp) = (price(&t.poc), price(&d.poc), price(&h.poc));
        if tp.is_finite() && dp.is_finite() {
            let gap = (dp - tp).abs();
            poc_d.push(gap);
            if gap > bucket * 0.5 {
                poc_moved += 1;
            }
        }
        if tp.is_finite() && hp.is_finite() {
            poc_t.push((hp - tp).abs());
        }
        let (tv, dv) = (price(&t.vah), price(&d.vah));
        if tv.is_finite() && dv.is_finite() {
            vah_d.push((dv - tv).abs());
        }
        let (tl, dl) = (price(&t.val), price(&d.val));
        if tl.is_finite() && dl.is_finite() {
            val_d.push((dl - tl).abs());
        }
    }

    let med = |v: &[f64]| {
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        pct(&s, 50.0)
    };
    let bucket_med = med(&buckets_seen);
    let r_med = med(&r_seen);
    println!(
        "windows:  {windows} usable ({skipped_no_atr} without ATR, {skipped_no_profile} without all three profiles)"
    );
    println!(
        "scale:    bucket = ATR14({interval})/{buckets_per_atr}, median {bucket_med:.2} USD; \
         1R = {stop_atr} x ATR14({interval}), median {r_med:.2} USD; tick {tick}"
    );
    println!("value area: {:.0}% by the classic walk, ties to the lower bucket", value_area * 100.0);
    println!();
    println!("DISTANCE, volume-distributed profile vs time profile (same bars, same grid):");
    describe("POC", poc_d.clone(), bucket_med, tick, r_med);
    describe("VAH", vah_d, bucket_med, tick, r_med);
    describe("VAL", val_d, bucket_med, tick, r_med);
    println!();
    println!("DISTANCE, volume-touched profile vs time profile:");
    describe("POC", poc_t, bucket_med, tick, r_med);
    println!();
    if !poc_d.is_empty() {
        println!(
            "POC in a DIFFERENT bucket than the time profile's: {poc_moved}/{} windows ({:.1}%)",
            poc_d.len(),
            100.0 * poc_moved as f64 / poc_d.len() as f64
        );
    }
    Ok(())
}
