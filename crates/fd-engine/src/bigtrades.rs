//! Big-print detection, and the per-strike footprint the whale levels rest on.
//!
//! What counts as "big" is a property of the market, not of the world: gold
//! premiums run roughly three orders of magnitude above BTC's, so a single
//! shared floor reports either everything or nothing. The absolute floor and the
//! rolling percentile are both configurable, and both are judgement calls.

use std::collections::HashMap;

use fd_core::types::{AggressorSide, OptionTrade, OptionType};
use serde::{Deserialize, Serialize};

/// Thresholds for calling a print large.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BigTradeConfig {
    /// Absolute USD floor.
    pub min_premium_usd: f64,
    pub min_contracts: Option<f64>,
    /// Rolling percentile, 0..1. `None` disables it.
    pub percentile: Option<f64>,
    /// How many recent prints the percentile is computed over.
    pub percentile_window: usize,
    /// Cap on how many prints [`select_big_trades`] returns.
    pub limit: usize,
}

impl Default for BigTradeConfig {
    fn default() -> Self {
        Self {
            min_premium_usd: 100_000.0,
            min_contracts: None,
            percentile: Some(0.95),
            percentile_window: 2000,
            limit: 200,
        }
    }
}

/// Linear-interpolated quantile over an ascending slice.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = (sorted.len() - 1) as f64 * q;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Percentile threshold over the most recent prints.
///
/// Returns infinity when the percentile is disabled, so callers can compare
/// against it unconditionally.
#[must_use]
pub fn percentile_threshold(trades: &[OptionTrade], config: &BigTradeConfig) -> f64 {
    let Some(q) = config.percentile else {
        return f64::INFINITY;
    };
    let start = trades.len().saturating_sub(config.percentile_window);
    let mut recent: Vec<f64> = trades[start..].iter().map(|t| t.premium_usd).collect();
    recent.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let value = quantile(&recent, q);
    if value.is_finite() { value } else { f64::INFINITY }
}

/// Is this print large, given a precomputed percentile threshold?
#[must_use]
pub fn is_big_trade(trade: &OptionTrade, config: &BigTradeConfig, percentile_threshold: f64) -> bool {
    if let Some(min) = config.min_contracts
        && trade.contracts < min
    {
        return false;
    }
    trade.premium_usd >= config.min_premium_usd || trade.premium_usd >= percentile_threshold
}

/// Large prints, **newest first**, capped at `limit`.
#[must_use]
pub fn select_big_trades(trades: &[OptionTrade], config: &BigTradeConfig) -> Vec<OptionTrade> {
    let threshold = percentile_threshold(trades, config);
    let mut out = Vec::new();
    for trade in trades.iter().rev() {
        if out.len() >= config.limit {
            break;
        }
        if is_big_trade(trade, config, threshold) {
            out.push(trade.clone());
        }
    }
    out
}

/// Aggregated large-print activity at one strike.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct WhaleStrike {
    pub strike: f64,
    /// Buyer-aggressed minus seller-aggressed contracts.
    pub call_net: f64,
    pub put_net: f64,
    pub call_premium: f64,
    pub put_premium: f64,
    /// Largest single print seen at this strike.
    pub max_premium: f64,
}

/// Per-strike footprint of large prints.
///
/// Insertion order is preserved rather than sorted by strike, because the whale
/// models pick a maximum with a strict `>` comparison: on a tie the strike seen
/// first wins, and "first" has to mean the same thing here as in the oracle.
#[derive(Debug, Clone, Default)]
pub struct WhaleFootprint {
    rows: Vec<WhaleStrike>,
    index: HashMap<u64, usize>,
}

impl WhaleFootprint {
    #[must_use]
    pub fn build(big_trades: &[OptionTrade]) -> Self {
        let mut footprint = Self::default();
        for trade in big_trades {
            let slot = *footprint.index.entry(trade.strike.to_bits()).or_insert_with(|| {
                footprint.rows.push(WhaleStrike { strike: trade.strike, ..Default::default() });
                footprint.rows.len() - 1
            });
            let row = &mut footprint.rows[slot];
            let sign = match trade.aggressor_side {
                AggressorSide::Buy => 1.0,
                AggressorSide::Sell => -1.0,
                AggressorSide::Unknown => 0.0,
            };
            match trade.option_type {
                OptionType::Call => {
                    row.call_net += sign * trade.contracts;
                    row.call_premium += trade.premium_usd;
                }
                OptionType::Put => {
                    row.put_net += sign * trade.contracts;
                    row.put_premium += trade.premium_usd;
                }
            }
            row.max_premium = row.max_premium.max(trade.premium_usd);
        }
        footprint
    }

    /// Rows in the order their strike was first seen.
    #[must_use]
    pub fn rows(&self) -> &[WhaleStrike] {
        &self.rows
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::TradeFlags;

    fn trade(ts: i64, strike: f64, option_type: OptionType, side: AggressorSide, contracts: f64, premium: f64) -> OptionTrade {
        OptionTrade {
            id: format!("{ts}"),
            timestamp: ts,
            symbol: "TEST".into(),
            instrument: None,
            underlying: "TEST".into(),
            expiration: 0,
            dte: 1.0,
            strike,
            option_type,
            trade_price: 1.0,
            contracts,
            bid: None,
            ask: None,
            aggressor_side: side,
            flow_class: classify_flow(option_type, side),
            premium_usd: premium,
            underlying_price: 0.0,
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags::default(),
            source: "test".into(),
        }
    }

    #[test]
    fn quantile_interpolates_between_samples() {
        assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.5);
        assert_eq!(quantile(&[1.0, 2.0, 3.0], 0.0), 1.0);
        assert_eq!(quantile(&[1.0, 2.0, 3.0], 1.0), 3.0);
        assert!(quantile(&[], 0.5).is_nan());
    }

    #[test]
    fn the_absolute_floor_and_the_percentile_both_qualify_a_print() {
        let mut trades: Vec<OptionTrade> = (0..100)
            .map(|i| trade(i, 4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 100.0))
            .collect();
        trades.push(trade(100, 4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 200_000.0));

        let config = BigTradeConfig { percentile_window: 200, ..Default::default() };
        let big = select_big_trades(&trades, &config);
        assert!(!big.is_empty());
        assert_eq!(big[0].premium_usd, 200_000.0, "results are newest first");
    }

    #[test]
    fn a_contract_floor_rejects_a_small_but_expensive_print() {
        let config = BigTradeConfig {
            min_premium_usd: 1000.0,
            min_contracts: Some(10.0),
            percentile: None,
            ..Default::default()
        };
        let small = trade(0, 4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 50_000.0);
        assert!(!is_big_trade(&small, &config, f64::INFINITY));
        let large = trade(1, 4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 50_000.0);
        assert!(is_big_trade(&large, &config, f64::INFINITY));
    }

    #[test]
    fn the_limit_caps_the_result_and_keeps_the_newest() {
        let trades: Vec<OptionTrade> = (0..50)
            .map(|i| trade(i, 4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 500_000.0))
            .collect();
        let config = BigTradeConfig { limit: 5, percentile: None, ..Default::default() };
        let big = select_big_trades(&trades, &config);
        assert_eq!(big.len(), 5);
        assert_eq!(big[0].timestamp, 49);
        assert_eq!(big[4].timestamp, 45);
    }

    #[test]
    fn the_footprint_nets_aggression_per_strike() {
        let big = vec![
            trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 60.0, 240_000.0),
            trade(1, 4200.0, OptionType::Put, AggressorSide::Sell, 10.0, 40_000.0),
            trade(2, 5000.0, OptionType::Call, AggressorSide::Buy, 80.0, 240_000.0),
        ];
        let footprint = WhaleFootprint::build(&big);
        let rows = footprint.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].strike, 4200.0, "insertion order, not sorted order");
        assert_eq!(rows[0].put_net, 50.0);
        assert_eq!(rows[0].put_premium, 280_000.0);
        assert_eq!(rows[0].max_premium, 240_000.0);
        assert_eq!(rows[1].call_net, 80.0);
    }

    #[test]
    fn an_unknown_aggressor_adds_premium_but_no_direction() {
        let big = vec![trade(0, 4200.0, OptionType::Put, AggressorSide::Unknown, 60.0, 240_000.0)];
        let rows = WhaleFootprint::build(&big);
        assert_eq!(rows.rows()[0].put_net, 0.0);
        assert_eq!(rows.rows()[0].put_premium, 240_000.0);
    }
}
