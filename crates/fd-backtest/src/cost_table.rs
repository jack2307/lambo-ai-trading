//! What a round turn costs as a fraction of the trade's own risk, per instrument.
//!
//! One quantity: `spread / stop`. It is the share of a trade's R that its
//! round-trip cost consumes, and it is a property of **the instrument and its
//! spread**, not of any rule — which is why every one of the four designed
//! angles of 2026-09-23 died on it and why nobody had ever measured it across
//! instruments. Registered as task D of
//! `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`.
//!
//! ## Why the ratio is the right thing to quote, and why points are not
//!
//! A position pays the spread **once** for the round turn (the two half-spreads
//! are one spread; `config/accounts.toml` is explicit about it). In money:
//!
//! ```text
//! cost  = spread [price/unit] x lots x contract_size [units/lot]
//! R     = stop   [price/unit] x lots x contract_size [units/lot]
//! cost/R = spread / stop      [dimensionless]
//! ```
//!
//! `lots` and `contract_size` cancel, and so does the price unit. That is the
//! whole reason this is comparable across instruments priced in dollars per
//! ounce, dollars per euro and dollars per bitcoin on contract sizes spanning
//! 0.01 to 1000. A figure in "points" or in dollars is not comparable and must
//! not be put in the same column. [`cost_fraction_of_r`] performs the
//! cancellation explicitly rather than asserting it, and a test pins it.
//!
//! ## What is missing is missing
//!
//! An instrument whose configured spread cannot be established has **no**
//! ratio — not a ratio of zero and not gold's. An empty bar file is an empty
//! bar file, not an instrument with no volatility. Both come back as `None`
//! here and are reported as exclusions, never as rows.

use fd_core::types::Bar;

/// Milliseconds in a day.
const DAY_MS: i64 = 86_400_000;

/// The distribution of one instrument's `spread / stop` at one stop rule.
///
/// `n` counts the bars that produced a ratio — i.e. those whose ATR had warmed
/// up and was strictly positive. Bars inside the ATR warmup, and bars whose
/// range collapsed to zero, produce no ratio and are not counted as zeros.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dist {
    pub n: usize,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    /// Median stop in the instrument's own price units — the denominator, kept
    /// beside the ratio so a reader can see what was divided by what.
    pub median_stop: f64,
}

/// The cost of a round turn as a fraction of the trade's R, with the units
/// carried through and cancelled in the open rather than assumed away.
///
/// `None` when the spread is unknown, when the stop is not a positive distance,
/// or when either side is not finite. A zero denominator is not a large cost;
/// it is an absent measurement.
#[must_use]
pub fn cost_fraction_of_r(spread: Option<f64>, stop: f64, lots: f64, contract_size: f64) -> Option<f64> {
    let spread = spread?;
    if !spread.is_finite() || spread < 0.0 || !stop.is_finite() || stop <= 0.0 {
        return None;
    }
    if !lots.is_finite() || lots <= 0.0 || !contract_size.is_finite() || contract_size <= 0.0 {
        return None;
    }
    // price/unit x lots x units/lot = money, on both lines. The division is
    // dimensionless and independent of both lots and contract size.
    let cost_money = spread * lots * contract_size;
    let r_money = stop * lots * contract_size;
    Some(cost_money / r_money)
}

/// `q`-quantile of an already-sorted ascending sample, nearest-rank.
///
/// Nearest-rank rather than interpolated, declared here once so every figure in
/// the table is the same statistic: the value at index `round(q x (n - 1))`.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64) -> Option<f64> {
    if sorted.is_empty() || !(0.0..=1.0).contains(&q) {
        return None;
    }
    let idx = (q * (sorted.len() - 1) as f64).round() as usize;
    sorted.get(idx.min(sorted.len() - 1)).copied()
}

/// Summarise a sample of ratios and the stops they came from.
///
/// An empty sample summarises to `None`. It does not summarise to zero.
#[must_use]
pub fn summarise(ratios: &[f64], stops: &[f64]) -> Option<Dist> {
    if ratios.is_empty() {
        return None;
    }
    let mut r: Vec<f64> = ratios.iter().copied().filter(|v| v.is_finite()).collect();
    let mut s: Vec<f64> = stops.iter().copied().filter(|v| v.is_finite()).collect();
    if r.is_empty() {
        return None;
    }
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(Dist {
        n: r.len(),
        p10: quantile(&r, 0.10)?,
        p50: quantile(&r, 0.50)?,
        p90: quantile(&r, 0.90)?,
        median_stop: quantile(&s, 0.50).unwrap_or(f64::NAN),
    })
}

/// Bucket a fine series into calendar days offset from UTC by `offset_ms`.
///
/// The offset exists because the metals and FX session breaks at 17:00 New York
/// — 21:00 UTC in summer — so a plain UTC-day bucket cuts a session in half and
/// the "daily" range it reports is two half-sessions glued together. Shifting by
/// the broker's clock (UTC+3) puts the boundary on the break. The bucket's
/// reported `time` is shifted back, so it is still a UTC instant.
///
/// This is a **grouping by timestamp, not a count of consecutive bars**, which
/// is what makes it safe across the 2013 Dukascopy bar-rule change: the longest
/// run of consecutive 15m bars drops from 476 to exactly 92 there, so anything
/// that counts bars silently becomes a 2010–2012 measurement. A day that lost
/// bars is a day with a smaller range, not a day that vanishes.
///
/// Input must be ascending in time. Volume is not used by anything downstream
/// here and is summed only so the bars are well formed.
#[must_use]
pub fn daily_bars(bars: &[Bar], offset_ms: i64) -> Vec<Bar> {
    bucket(bars, DAY_MS, offset_ms)
}

/// Bucket a fine series into `step_ms` buckets whose boundary is `offset_ms`
/// ahead of the UTC epoch grid.
///
/// The general form of [`daily_bars`]; a four-hour horizon takes `offset_ms` 0
/// because four hours is a wall-clock horizon, not a trading day. See
/// [`daily_bars`] for why grouping by timestamp is safe across the 2013
/// Dukascopy bar-rule change and counting bars is not.
#[must_use]
pub fn bucket(bars: &[Bar], step_ms: i64, offset_ms: i64) -> Vec<Bar> {
    if step_ms <= 0 {
        return bars.to_vec();
    }
    let mut out: Vec<Bar> = Vec::new();
    for row in bars {
        let open = if row.open.is_finite() { row.open } else { row.close };
        let high = if row.high.is_finite() { row.high } else { row.close };
        let low = if row.low.is_finite() { row.low } else { row.close };
        let start = (row.time + offset_ms).div_euclid(step_ms) * step_ms - offset_ms;
        match out.last_mut() {
            Some(current) if current.time == start => {
                current.high = current.high.max(high);
                current.low = current.low.min(low);
                current.close = row.close;
                current.volume = Some(current.volume.unwrap_or_default() + row.volume.unwrap_or(1.0));
            }
            _ => out.push(Bar {
                time: start,
                open,
                high,
                low,
                close: row.close,
                volume: Some(row.volume.unwrap_or(1.0)),
            }),
        }
    }
    out
}

/// One bar's stop and the ratio it implies, for every bar whose ATR has warmed.
///
/// Returns `(time, stop, ratio)`. The stop is `stop_atr x ATR` in the
/// instrument's own price units; the ratio is dimensionless. A bar inside the
/// warmup, or one whose ATR is zero, contributes nothing — a stop of zero is not
/// an infinitely expensive trade, it is an unmeasurable one.
#[must_use]
pub fn per_bar(bars: &[Bar], atr: &[f64], stop_atr: f64, spread: Option<f64>) -> Vec<(i64, f64, f64)> {
    let mut out = Vec::new();
    for (i, bar) in bars.iter().enumerate() {
        let Some(&a) = atr.get(i) else { continue };
        if !a.is_finite() || a <= 0.0 {
            continue;
        }
        let stop = stop_atr * a;
        let Some(ratio) = cost_fraction_of_r(spread, stop, 1.0, 1.0) else { continue };
        out.push((bar.time, stop, ratio));
    }
    out
}

/// A named span of years, `[from, to)` in UTC milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct Era {
    pub label: &'static str,
    pub from_ms: i64,
    pub to_ms: i64,
}

impl Era {
    #[must_use]
    pub fn contains(&self, time: i64) -> bool {
        time >= self.from_ms && time < self.to_ms
    }
}

/// UTC midnight of a civil date, in milliseconds.
#[must_use]
pub fn date_ms(year: i64, month: u32, day: u32) -> i64 {
    fd_core::clock::days_from_civil(year, month, day) * DAY_MS
}

/// `YYYY-MM-DD` for a UTC millisecond instant.
#[must_use]
pub fn ymd(ms: i64) -> String {
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(DAY_MS));
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ratio_is_dimensionless_so_lots_and_contract_size_cancel() {
        // The whole comparability claim in one assertion. Gold's 0.28 on a
        // 2.00-point stop is 14% of R whether it is sized at one ounce or at a
        // hundred, and whether the book trades 0.01 lots or 2.563.
        let spread = Some(0.28);
        let stop = 2.00;
        let plain = cost_fraction_of_r(spread, stop, 1.0, 1.0).unwrap();
        assert!((plain - 0.14).abs() < 1e-12, "0.28 / 2.00 = 14.0% of R, got {plain}");
        for (lots, contract) in [(0.01, 1.0), (2.563, 1.0), (0.07, 100.0), (1.0, 0.01), (0.5, 1000.0), (1.0, 50.0)] {
            let got = cost_fraction_of_r(spread, stop, lots, contract).unwrap();
            assert!(
                (got - plain).abs() < 1e-12,
                "spread/stop must not depend on lots={lots} contract={contract}: {got} vs {plain}"
            );
        }
    }

    #[test]
    fn an_absent_spread_has_no_ratio_rather_than_a_ratio_of_zero() {
        // `null` is not `0`. An instrument whose configured spread could not be
        // established is excluded from the table; it does not appear as free.
        assert_eq!(cost_fraction_of_r(None, 2.0, 1.0, 1.0), None);
        assert_eq!(cost_fraction_of_r(Some(f64::NAN), 2.0, 1.0, 1.0), None);
        // And a spread of zero IS a measurement — a swap-free, zero-spread
        // instrument costs nothing — so it is 0.0 and not None.
        assert_eq!(cost_fraction_of_r(Some(0.0), 2.0, 1.0, 1.0), Some(0.0));
    }

    #[test]
    fn a_zero_or_absent_stop_is_unmeasurable_not_infinite() {
        assert_eq!(cost_fraction_of_r(Some(0.28), 0.0, 1.0, 1.0), None);
        assert_eq!(cost_fraction_of_r(Some(0.28), -1.0, 1.0, 1.0), None);
        assert_eq!(cost_fraction_of_r(Some(0.28), f64::NAN, 1.0, 1.0), None);
    }

    #[test]
    fn an_empty_sample_summarises_to_nothing() {
        // An empty bar file is empty, not an instrument with no volatility.
        assert_eq!(summarise(&[], &[]), None);
    }

    #[test]
    fn quantiles_are_nearest_rank_on_the_sorted_sample() {
        let s = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(quantile(&s, 0.0), Some(1.0));
        assert_eq!(quantile(&s, 1.0), Some(10.0));
        // round(0.5 x 9) = round(4.5) = 5 -> the sixth element.
        assert_eq!(quantile(&s, 0.5), Some(6.0));
        // round(0.1 x 9) = round(0.9) = 1 -> the second.
        assert_eq!(quantile(&s, 0.1), Some(2.0));
        assert_eq!(quantile(&[], 0.5), None);
        assert_eq!(quantile(&s, 1.5), None);
    }

    #[test]
    fn a_session_that_straddles_midnight_is_one_day_not_two() {
        // Three 15m bars at 22:00, 23:00 UTC and 01:00 the next UTC date. With
        // the metals break at 21:00 UTC they are one session; a UTC-day bucket
        // would split them and report two half-ranges.
        let day = date_ms(2024, 3, 5);
        let bars = vec![
            Bar { time: day + 22 * 3_600_000, open: 2100.0, high: 2110.0, low: 2095.0, close: 2105.0, volume: Some(1.0) },
            Bar { time: day + 23 * 3_600_000, open: 2105.0, high: 2120.0, low: 2100.0, close: 2115.0, volume: Some(1.0) },
            Bar { time: day + 25 * 3_600_000, open: 2115.0, high: 2125.0, low: 2090.0, close: 2092.0, volume: Some(1.0) },
        ];
        let sessions = daily_bars(&bars, 3 * 3_600_000);
        assert_eq!(sessions.len(), 1, "one session, got {sessions:?}");
        assert_eq!(sessions[0].high, 2125.0);
        assert_eq!(sessions[0].low, 2090.0);
        assert_eq!(sessions[0].close, 2092.0);
        // Plain UTC days would have made it two.
        assert_eq!(daily_bars(&bars, 0).len(), 2);
    }

    #[test]
    fn four_hour_buckets_sit_on_the_utc_grid() {
        let day = date_ms(2024, 3, 5);
        let bars: Vec<Bar> = (0..16)
            .map(|i| Bar { time: day + i * 900_000, open: 1.0, high: 1.0 + i as f64, low: 1.0, close: 1.0, volume: Some(1.0) })
            .collect();
        let h4 = bucket(&bars, 14_400_000, 0);
        assert_eq!(h4.len(), 1, "sixteen 15m bars are exactly one 4h bar");
        assert_eq!(h4[0].time, day);
        assert_eq!(h4[0].high, 16.0);
    }

    #[test]
    fn a_day_missing_bars_is_a_smaller_range_not_a_missing_day() {
        // The 2013 Dukascopy bar-rule trap, stated as a test: grouping by
        // timestamp cannot lose a day to a gap, the way a scan gated on N
        // consecutive bars does.
        let day = date_ms(2015, 7, 1);
        let bars = vec![
            Bar { time: day, open: 1.0, high: 1.1, low: 0.9, close: 1.0, volume: Some(1.0) },
            // a nine-hour hole
            Bar { time: day + 10 * 3_600_000, open: 1.0, high: 1.2, low: 0.8, close: 1.1, volume: Some(1.0) },
            Bar { time: day + DAY_MS + 3_600_000, open: 1.1, high: 1.3, low: 1.0, close: 1.2, volume: Some(1.0) },
        ];
        let sessions = daily_bars(&bars, 0);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].high, 1.2);
        assert_eq!(sessions[0].low, 0.8);
    }

    #[test]
    fn warmup_bars_contribute_no_ratio() {
        let bars: Vec<Bar> = (0..5).map(|i| Bar::flat(i * 900_000, 100.0)).collect();
        let atr = vec![f64::NAN, f64::NAN, 0.0, 2.0, 4.0];
        let rows = per_bar(&bars, &atr, 1.5, Some(0.28));
        assert_eq!(rows.len(), 2, "two warmed, one zero-ATR, two NaN: {rows:?}");
        assert!((rows[0].1 - 3.0).abs() < 1e-12, "1.5 x 2.0");
        assert!((rows[0].2 - 0.28 / 3.0).abs() < 1e-12);
    }

    /// The silver trap, locked: every instrument's spread must come from its own
    /// market block. An agent charged silver gold's 0.28 instead of its
    /// configured 0.021 and voided sixteen of its own forty-eight cells
    /// (`docs/decisions/2026-09-24-designed-methods.md`, defect 8's neighbour).
    /// The configured spreads differ by five orders of magnitude and nothing may
    /// substitute one for another.
    #[test]
    fn each_market_carries_its_own_spread_and_none_of_them_is_golds() {
        let config = fd_core::config::Config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../config"))
            .expect("config/default.toml");
        let spread = |id: &str| config.market(id).expect(id).trading.spread;
        assert!((spread("xauusd") - 0.28).abs() < 1e-12);
        assert!((spread("xauduka") - 0.28).abs() < 1e-12);
        assert!((spread("xagduka") - 0.021).abs() < 1e-12, "silver is 0.021, NOT gold's 0.28");
        assert!((spread("eurduka") - 0.00014).abs() < 1e-15);
        assert!((spread("eurusd") - 0.00014).abs() < 1e-15);
        assert!((spread("btcusd") - 17.05).abs() < 1e-12);
        assert!((spread("btc") - 5.0).abs() < 1e-12);
        assert!((spread("gold") - 0.3).abs() < 1e-12, "the COMEX tape's 0.3 is not the CFD's 0.28 either");
    }

    /// `MarketTradingOverride::spread` has no `#[serde(default)]`, so a market
    /// block that omits it FAILS TO PARSE rather than quietly inheriting the
    /// top-level `[trading] spread = 0.3`. That is the right behaviour and it is
    /// load-bearing for this table; pin it so nobody adds a default later.
    #[test]
    fn a_market_block_without_a_spread_is_refused_not_given_golds() {
        let text = r#"
multiplier = 100
underlying = "GC"
market = "x"
[models]
break_even = "premium-weighted"
whale = "net-long-whale"
max_pain_absolute = true
[profile]
mode = "PREMIUM"
value_area_pct = 0.7
[big_trades]
min_premium_usd = 100000
percentile = 0.95
percentile_window = 2000
limit = 200
[flow]
velocity_window_ms = 900000
[flow.thresholds]
strong_bull = 0.6
moderate_bull = 0.55
moderate_bear = 0.45
strong_bear = 0.4
[flow.windows]
"15m" = 900000
[levels]
max_clusters = 12
[levels.cluster]
floor = 5.0
atr_fraction = 0.15
[levels.type_weights]
POC = 1.0
[backtest]
timeframe = "15m"
options_step_ms = 300000
fallback_atr_period = 14
min_trades_per_cell = 5
walk_forward_folds = 4
select_by = "expectancy"
[backtest.promising]
min_trades = 30
min_profit_factor = 1.2
min_expectancy_r = 0.05
[trading]
symbol = "XAUUSD"
contract_size = 100.0
spread = 0.3
commission_per_lot = 0.0
starting_equity_usd = 10000.0
risk_per_trade_pct = 0.01
stop_atr = 1.2
reward_risk = 1.8
max_hold_ms = 14400000
lot_step = 0.01
min_lot = 0.01
[trading.trail]
enabled = false
distance_r = 1.0
activate_r = 1.0
[trading.guards]
max_concurrent_positions = 1
max_trades_per_day = 20
daily_loss_limit_usd = 300.0
cooldown_ms = 1800000
max_open_loss_r = 2.0
max_notional_pct_equity = 300.0
flat_before_weekend_hhmm = 1640
news_flat_before_min = 60
news_flat_after_min = 30
news_min_impact = 3
[ai]
atr_bar_ms = 900000
atr_period = 14
dataset_step_ms = 300000
warmup_ms = 7200000
horizon = "60m"
label_band_atr = 0.5
folds = 4
big_trade_window_ms = 1800000
long_threshold = 0.58
short_threshold = 0.42
cycle_ms = 300000
live_contracts = 4
[ai.horizons_ms]
"15m" = 900000
[ai.train]
epochs = 800
lr = 0.1
l2 = 2.0
class_weight = true
[ai.gate]
min_auc = 0.55
min_trades = 30
min_profit_factor = 1.2
min_expectancy_r = 0.05
[ai.analyst]
enabled = true
model = "claude-opus-5"
effort = "high"
max_tokens = 4000
can_veto = true
required_for_signal = false
min_confidence = 0.4
scales_size = true
full_size_confidence = 0.8
min_size_scale = 0.4
[server]
host = "127.0.0.1"
port = 8137
[markets.x]
label = "a market with no spread of its own"
bar_symbol = "X"
bar_source = "mt5"
options_source = "none"
premium_in_underlying = false
multiplier = 1
underlying = "X"
[markets.x.trading]
symbol = "X"
contract_size = 1.0
lot_step = 0.01
min_lot = 0.01
[markets.x.big_trades]
min_premium_usd = 1.0
[markets.x.levels.cluster]
floor = 1.0
atr_fraction = 0.15
[sources.mt5]
base_url = "none"
min_delay_ms = 1
"#;
        let err = fd_core::config::Config::from_toml(text, "no-spread")
            .expect_err("a market with no spread must not parse");
        let text = err.to_string();
        assert!(text.contains("spread"), "the error must name the missing key: {text}");
    }
}
