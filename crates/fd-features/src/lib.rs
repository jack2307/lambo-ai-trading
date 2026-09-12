//! Feature extraction.
//!
//! One implementation, called from both sides. That is the entire reason this
//! is its own crate rather than a module inside a training script: a feature
//! computed one way to train a model and another way to run it produces a
//! model that is correct, a pipeline that lies to it, and no error anywhere.
//! Train/serve skew is the quietest expensive bug in machine-learning trading,
//! and the only reliable defence is to make the two paths the same code.
//!
//! One rule governs everything here: **a feature may only read information that
//! existed at the snapshot's `as_of`.** The engine already guarantees that for
//! flow and levels; price-derived features must slice the candle series the
//! same way, which [`price_context`] does.
//!
//! Features are deliberately scale-free — ratios and ATR-normalised distances —
//! so that a model trained with gold at 4,300 is not useless at 5,200.

pub mod dataset;

use fd_core::classify::{is_bear, is_bull};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_engine::engine::Snapshot;

/// Ordered feature names.
///
/// The model reads a vector, not a map, so this order *is* part of the model.
/// Reordering it silently retargets every weight; adding to the end is the only
/// safe edit.
pub const FEATURE_NAMES: [&str; 24] = [
    "insideCluster",
    "clusterDistanceAtr",
    "clusterScore",
    "clusterTypes",
    "clusterExpirations",
    "clusterIsSupport",
    "bullRatioAll",
    "bullRatio15m",
    "bullRatio60m",
    "netFlowVelocityNorm",
    "bigTradeImbalance",
    "maxPainDistAtr",
    "pocDistAtr",
    "wSupDistAtr",
    "wResDistAtr",
    "callBeDistAtr",
    "putBeDistAtr",
    "frontDte",
    "expiryWeightFront",
    "atrPct",
    "premiumRateNorm",
    "returns15mAtr",
    "returns60mAtr",
    "positionInDayRange",
];

/// Everything price-related a feature row needs, sliced strictly at `now_ms`.
#[derive(Debug, Clone)]
pub struct PriceContext {
    pub price: f64,
    pub atr: f64,
    pub close_15m: f64,
    pub close_60m: f64,
    pub day_high: f64,
    pub day_low: f64,
}

/// Resample a close series into fixed-width buckets.
///
/// Distinct from [`fd_store::resample`]: this one builds its range from the
/// *closes* it sees, because the series it is given may carry nothing else.
/// The two must not be merged — one describes bars, this one describes what can
/// be recovered when there are none.
fn resample_closes(candles: &[Bar], bar_ms: i64) -> Vec<Bar> {
    if bar_ms <= 0 {
        return candles.to_vec();
    }
    let mut out: Vec<Bar> = Vec::new();
    for candle in candles {
        let start = candle.time.div_euclid(bar_ms) * bar_ms;
        match out.last_mut() {
            Some(bucket) if bucket.time == start => {
                bucket.close = candle.close;
                bucket.high = bucket.high.max(candle.close);
                bucket.low = bucket.low.min(candle.close);
            }
            _ => out.push(Bar {
                time: start,
                open: candle.close,
                high: candle.close,
                low: candle.close,
                close: candle.close,
                volume: None,
            }),
        }
    }
    out
}

/// Average true range over a close-only series.
///
/// The bar width matters more than the period. ATR on one-minute closes
/// measures one-minute noise, and a stop sized from it is swept long before an
/// hour-horizon thesis can play out — so callers pass a `bar_ms` matched to the
/// trade horizon.
#[must_use]
pub fn atr_from_closes(candles: &[Bar], period: usize, bar_ms: i64) -> f64 {
    let series = if bar_ms > 0 { resample_closes(candles, bar_ms) } else { candles.to_vec() };
    if series.len() < 2 {
        return f64::NAN;
    }
    let take = (period + 1).min(series.len());
    let slice = &series[series.len() - take..];

    let mut sum = 0.0;
    for i in 1..slice.len() {
        let current = &slice[i];
        let previous_close = slice[i - 1].close;
        // Resampling gives highs and lows, so the true range is available;
        // without them only the close-to-close move can be measured.
        let range = (current.high - current.low)
            .max((current.high - previous_close).abs())
            .max((current.low - previous_close).abs());
        sum += range;
    }
    sum / (slice.len() - 1) as f64
}

/// Price context at `now_ms`, reading nothing after it.
#[must_use]
pub fn price_context(candles: &[Bar], now_ms: i64, atr_period: usize, atr_bar_ms: i64) -> PriceContext {
    let cut = candles.partition_point(|c| c.time <= now_ms);
    let past = &candles[..cut];
    let price = past.last().map_or(f64::NAN, |c| c.close);
    let atr = atr_from_closes(past, atr_period, atr_bar_ms);

    let close_at = |ms_ago: i64| -> f64 {
        let target = now_ms - ms_ago;
        for candle in past.iter().rev() {
            if candle.time <= target {
                return candle.close;
            }
        }
        past.first().map_or(f64::NAN, |c| c.close)
    };

    let day_from = now_ms - 86_400_000;
    let (mut day_high, mut day_low) = (f64::NEG_INFINITY, f64::INFINITY);
    for candle in past.iter().rev() {
        if candle.time < day_from {
            break;
        }
        day_high = day_high.max(candle.close);
        day_low = day_low.min(candle.close);
    }

    PriceContext {
        price,
        atr: if atr.is_finite() && atr > 0.0 { atr } else { f64::NAN },
        close_15m: close_at(900_000),
        close_60m: close_at(3_600_000),
        day_high: if day_high.is_finite() { day_high } else { f64::NAN },
        day_low: if day_low.is_finite() { day_low } else { f64::NAN },
    }
}

/// What a row carries besides its feature values.
#[derive(Debug, Clone)]
pub struct FeatureMeta {
    pub as_of: i64,
    pub spot: f64,
    pub atr: f64,
    pub front_symbol: String,
    pub front_dte: f64,
    pub cluster_score: f64,
}

#[derive(Debug, Clone)]
pub struct FeatureRow {
    /// In [`FEATURE_NAMES`] order.
    pub values: Vec<f64>,
    pub meta: FeatureMeta,
}

/// Extract one feature row, or `None` when the instant cannot support one.
///
/// Returning `None` rather than a row of zeros is deliberate: a moment with no
/// price, no ATR or no contract is not a moment where everything was zero, and
/// letting those rows into a dataset teaches a model that the absence of data
/// is a market state.
#[must_use]
pub fn extract(snapshot: &Snapshot, price: &PriceContext, config: &Config) -> Option<FeatureRow> {
    let spot = if price.price.is_finite() { price.price } else { snapshot.spot };
    let atr = price.atr;
    if !spot.is_finite() || !atr.is_finite() || atr <= 0.0 {
        return None;
    }
    let front = snapshot.contexts.first()?;

    let near = snapshot.nearest.as_ref();
    let cluster = near.map(|n| &n.cluster);
    let norm = |level: Option<f64>| level.filter(|v| v.is_finite()).map_or(0.0, |v| (v - spot) / atr);

    let w15 = snapshot.flow_windows.get("15m").unwrap_or(&snapshot.flow_overall);
    let w60 = snapshot.flow_windows.get("60m").unwrap_or(&snapshot.flow_overall);
    let total_premium =
        (snapshot.flow_overall.bull_premium + snapshot.flow_overall.bear_premium).max(1.0);

    // Big-print imbalance over the recent window only: cumulative flow hides
    // the moment a tape turns, which is the only moment it is useful.
    let cutoff = snapshot.as_of - config.ai.big_trade_window_ms;
    let (mut big_bull, mut big_bear) = (0.0, 0.0);
    for print in &snapshot.big_trades {
        if print.timestamp < cutoff {
            continue;
        }
        if is_bull(print.flow_class) {
            big_bull += print.premium_usd;
        } else if is_bear(print.flow_class) {
            big_bear += print.premium_usd;
        }
    }
    let big_total = big_bull + big_bear;
    let premium_rate = w15.bull_premium + w15.bear_premium;

    let values = vec![
        sanitize(near.map_or(0.0, |n| f64::from(u8::from(n.inside)))),
        sanitize(near.map_or(10.0, |n| (n.distance / atr).min(10.0))),
        sanitize(cluster.map_or(0.0, |c| c.score.min(20.0))),
        sanitize(cluster.map_or(0.0, |c| c.distinct_types as f64)),
        sanitize(cluster.map_or(0.0, |c| c.distinct_expirations as f64)),
        sanitize(cluster.map_or(0.0, |c| if c.center <= spot { 1.0 } else { -1.0 })),
        sanitize(snapshot.flow_overall.bull_ratio),
        sanitize(w15.bull_ratio),
        sanitize(w60.bull_ratio),
        sanitize(clamp(snapshot.velocity.velocity / total_premium, -1.0, 1.0)),
        sanitize(if big_total > 0.0 { (big_bull - big_bear) / big_total } else { 0.0 }),
        sanitize(clamp(norm(front.max_pain), -10.0, 10.0)),
        sanitize(clamp(norm(front.poc), -10.0, 10.0)),
        sanitize(clamp(norm(front.w_sup), -10.0, 10.0)),
        sanitize(clamp(norm(front.w_res), -10.0, 10.0)),
        sanitize(clamp(norm(front.call_be), -10.0, 10.0)),
        sanitize(clamp(norm(front.put_be), -10.0, 10.0)),
        sanitize(if front.dte.is_finite() { front.dte.min(60.0) } else { 30.0 }),
        sanitize(expiry_weight_of(front.dte)),
        sanitize((atr / spot) * 100.0),
        sanitize(premium_rate.max(1.0).log10() / 10.0),
        sanitize(if price.close_15m.is_finite() {
            clamp((spot - price.close_15m) / atr, -10.0, 10.0)
        } else {
            0.0
        }),
        sanitize(if price.close_60m.is_finite() {
            clamp((spot - price.close_60m) / atr, -10.0, 10.0)
        } else {
            0.0
        }),
        sanitize(day_position(spot, price.day_low, price.day_high)),
    ];
    debug_assert_eq!(values.len(), FEATURE_NAMES.len());

    Some(FeatureRow {
        values,
        meta: FeatureMeta {
            as_of: snapshot.as_of,
            spot,
            atr,
            front_symbol: front.symbol.clone(),
            front_dte: front.dte,
            cluster_score: cluster.map_or(0.0, |c| c.score),
        },
    })
}

/// How much weight a contract's expiry carries.
///
/// ⚠ Unverified: the breakpoints are the prototype's, calibrated against a
/// reference feed rather than derived from anything. Kept identical here so the
/// port is a port; changing them is a modelling decision, not a porting one.
fn expiry_weight_of(dte: f64) -> f64 {
    if !dte.is_finite() {
        return 0.8;
    }
    if dte <= 0.25 {
        1.5
    } else if dte <= 1.0 {
        1.3
    } else if dte <= 3.0 {
        1.15
    } else if dte <= 7.0 {
        1.0
    } else {
        0.8
    }
}

fn day_position(spot: f64, low: f64, high: f64) -> f64 {
    if !low.is_finite() || !high.is_finite() || high <= low {
        return 0.5;
    }
    clamp((spot - low) / (high - low), 0.0, 1.0)
}

/// Clamp, treating a non-finite reading as zero.
#[must_use]
pub fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() { value.clamp(low, high) } else { 0.0 }
}

/// A non-finite feature becomes zero rather than reaching the model.
///
/// Not defensive padding: a NaN propagates through a dot product and turns an
/// entire prediction into NaN, which downstream reads as "no signal" — the same
/// thing a balanced market looks like.
fn sanitize(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closes(times_and_prices: &[(i64, f64)]) -> Vec<Bar> {
        times_and_prices.iter().map(|(t, p)| Bar::flat(*t, *p)).collect()
    }

    const MINUTE: i64 = 60_000;

    #[test]
    fn the_price_context_reads_nothing_after_its_instant() {
        let candles = closes(&[(0, 100.0), (MINUTE, 101.0), (2 * MINUTE, 999.0)]);
        let context = price_context(&candles, MINUTE, 14, 0);
        assert_eq!(context.price, 101.0, "a future candle must not be visible");
    }

    #[test]
    fn atr_over_closes_is_the_mean_absolute_move() {
        // Without resampling every bar is flat, so the true range collapses to
        // the close-to-close move.
        let candles = closes(&[(0, 100.0), (MINUTE, 102.0), (2 * MINUTE, 101.0), (3 * MINUTE, 104.0)]);
        let atr = atr_from_closes(&candles, 3, 0);
        assert!((atr - (2.0 + 1.0 + 3.0) / 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_series_shorter_than_two_bars_has_no_atr() {
        assert!(atr_from_closes(&closes(&[(0, 100.0)]), 14, 0).is_nan());
        assert!(atr_from_closes(&[], 14, 0).is_nan());
    }

    #[test]
    fn resampling_recovers_a_range_from_closes_alone() {
        let candles = closes(&[(0, 100.0), (MINUTE, 105.0), (2 * MINUTE, 98.0)]);
        let buckets = resample_closes(&candles, 15 * MINUTE);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].high, 105.0);
        assert_eq!(buckets[0].low, 98.0);
        assert_eq!(buckets[0].close, 98.0, "the bucket closes on its last candle");
    }

    #[test]
    fn the_day_range_position_is_a_half_when_there_is_no_range() {
        assert_eq!(day_position(100.0, 100.0, 100.0), 0.5);
        assert_eq!(day_position(100.0, f64::NAN, 110.0), 0.5);
        assert_eq!(day_position(105.0, 100.0, 110.0), 0.5);
    }

    #[test]
    fn a_non_finite_feature_becomes_zero_rather_than_poisoning_the_vector() {
        assert_eq!(sanitize(f64::NAN), 0.0);
        assert_eq!(sanitize(f64::INFINITY), 0.0);
        assert_eq!(clamp(f64::NAN, -1.0, 1.0), 0.0);
    }

    #[test]
    fn the_feature_order_is_the_length_the_model_expects() {
        assert_eq!(FEATURE_NAMES.len(), 24);
        // Guards against a rename that would silently retarget a weight.
        assert_eq!(FEATURE_NAMES[0], "insideCluster");
        assert_eq!(FEATURE_NAMES[23], "positionInDayRange");
    }
}
