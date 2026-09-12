//! Max pain, and the positioning table that stands in for open interest.
//!
//! Classic max pain needs open interest. **Neither feed in this system
//! publishes any**, so the table is built from the tape instead: buyer-aggressed
//! prints add contracts at a strike, seller-aggressed prints subtract them.
//!
//! That is a proxy, and a weak one — it cannot see positions opened before the
//! tape window starts, and it cannot tell an opening trade from a closing one.
//! Calibration against the reference feed showed the *unsigned* variant (traded
//! volume) tracking the published number more closely than the net variant on
//! three of four contracts, which is why [`PositioningMode::Volume`] is the
//! default. Both stay available because four contracts is not a proof.
//!
//! Whatever this returns, it is a structural reference, never a prediction that
//! price must travel to it.

use std::collections::BTreeMap;

use fd_core::classify::position_sign;
use fd_core::types::{OptionTrade, OptionType};
use serde::{Deserialize, Serialize};

/// Positioning at one strike.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StrikePositioning {
    pub strike: f64,
    /// Non-negative contracts used by the payout sum.
    pub call_oi: f64,
    pub put_oi: f64,
    /// The signed values before clamping, kept for models that want direction.
    pub net_call: f64,
    pub net_put: f64,
}

/// Strikes and their positioning, ascending by strike.
pub type PositioningTable = Vec<StrikePositioning>;

/// How a print contributes contracts to its strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PositioningMode {
    /// Traded contracts, unsigned. CALIBRATED — closest to the reference.
    #[default]
    Volume,
    /// Buyer-aggressed minus seller-aggressed.
    Net,
}

/// Build a positioning table from a tape.
///
/// Negative net positioning is meaningless in an intrinsic-payout sum, so it is
/// clamped at zero for the payout while the raw net survives in `net_call` /
/// `net_put`.
#[must_use]
pub fn positioning_table(trades: &[OptionTrade], mode: PositioningMode) -> PositioningTable {
    let mut buckets: BTreeMap<u64, StrikePositioning> = BTreeMap::new();

    for trade in trades {
        if !trade.strike.is_finite() {
            continue;
        }
        let entry = buckets.entry(trade.strike.to_bits()).or_insert(StrikePositioning {
            strike: trade.strike,
            call_oi: 0.0,
            put_oi: 0.0,
            net_call: 0.0,
            net_put: 0.0,
        });
        let qty = match mode {
            PositioningMode::Volume => trade.contracts,
            PositioningMode::Net => f64::from(position_sign(trade.flow_class)) * trade.contracts,
        };
        match trade.option_type {
            OptionType::Call => entry.net_call += qty,
            OptionType::Put => entry.net_put += qty,
        }
    }

    let mut rows: Vec<StrikePositioning> = buckets.into_values().collect();
    rows.sort_by(|a, b| a.strike.partial_cmp(&b.strike).expect("strikes are finite"));
    for row in &mut rows {
        row.call_oi = row.net_call.max(0.0);
        row.put_oi = row.net_put.max(0.0);
    }
    rows
}

/// Settlement price minimising total intrinsic payout to option holders.
///
/// Ties resolve to the **lower** strike, matching the oracle: with a symmetric
/// board the choice is arbitrary, so it has to be fixed somewhere or the two
/// implementations disagree at random.
#[must_use]
pub fn max_pain(rows: &[StrikePositioning], multiplier: f64, candidates: Option<&[f64]>) -> Option<f64> {
    if rows.is_empty() {
        return None;
    }
    let owned: Vec<f64>;
    let prices: &[f64] = match candidates {
        Some(c) if !c.is_empty() => c,
        _ => {
            owned = rows.iter().map(|r| r.strike).collect();
            &owned
        }
    };

    let mut best: Option<f64> = None;
    let mut min_payout = f64::INFINITY;

    for &settlement in prices {
        let payout = payout_at(rows, settlement, multiplier);
        let improves = payout < min_payout;
        let ties_lower = payout == min_payout && best.is_some_and(|b| settlement < b);
        if improves || ties_lower {
            min_payout = payout;
            best = Some(settlement);
        }
    }
    best
}

fn payout_at(rows: &[StrikePositioning], settlement: f64, multiplier: f64) -> f64 {
    rows.iter()
        .map(|row| {
            let call_intrinsic = (settlement - row.strike).max(0.0);
            let put_intrinsic = (row.strike - settlement).max(0.0);
            call_intrinsic * row.call_oi * multiplier + put_intrinsic * row.put_oi * multiplier
        })
        .sum()
}

/// Full payout curve, for charting and for explaining a max-pain value.
#[must_use]
pub fn payout_curve(rows: &[StrikePositioning], multiplier: f64) -> Vec<(f64, f64)> {
    let mut strikes: Vec<f64> = rows.iter().map(|r| r.strike).collect();
    strikes.dedup();
    strikes.iter().map(|&s| (s, payout_at(rows, s, multiplier))).collect()
}

/// Max pain derived from a tape rather than from open interest.
#[must_use]
pub fn flow_max_pain(trades: &[OptionTrade], multiplier: f64, mode: PositioningMode) -> Option<f64> {
    let rows = positioning_table(trades, mode);
    max_pain(&rows, multiplier, None)
}

/// Freshness metadata that must travel with any max-pain value, so a screen can
/// never imply a live number computed from stale inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataState {
    /// Epoch milliseconds of the newest input.
    pub as_of: i64,
    pub source: String,
    pub is_preliminary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::{AggressorSide, TradeFlags};

    fn row(strike: f64, call: f64, put: f64) -> StrikePositioning {
        StrikePositioning { strike, call_oi: call, put_oi: put, net_call: call, net_put: put }
    }

    fn trade(strike: f64, option_type: OptionType, side: AggressorSide, contracts: f64) -> OptionTrade {
        OptionTrade {
            id: "t".into(),
            timestamp: 0,
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
            premium_usd: contracts * 100.0,
            underlying_price: 0.0,
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags::default(),
            source: "test".into(),
        }
    }

    #[test]
    fn max_pain_minimises_total_intrinsic_payout() {
        let rows = vec![row(4300.0, 10.0, 100.0), row(4400.0, 50.0, 50.0), row(4500.0, 100.0, 10.0)];
        let answer = max_pain(&rows, 100.0, None).unwrap();

        // Brute force the same question independently.
        let mut best = f64::NAN;
        let mut min = f64::INFINITY;
        for &s in &[4300.0, 4400.0, 4500.0] {
            let payout: f64 = rows
                .iter()
                .map(|r| (s - r.strike).max(0.0) * r.call_oi * 100.0 + (r.strike - s).max(0.0) * r.put_oi * 100.0)
                .sum();
            if payout < min {
                min = payout;
                best = s;
            }
        }
        assert_eq!(answer, best);
    }

    #[test]
    fn an_empty_table_returns_none_rather_than_a_number() {
        assert_eq!(max_pain(&[], 100.0, None), None);
        assert_eq!(flow_max_pain(&[], 100.0, PositioningMode::Volume), None);
    }

    #[test]
    fn ties_resolve_to_the_lower_strike() {
        // A symmetric board: both strikes produce the same payout.
        let rows = vec![row(100.0, 1.0, 1.0), row(200.0, 1.0, 1.0)];
        assert_eq!(max_pain(&rows, 1.0, None), Some(100.0));
    }

    #[test]
    fn net_mode_cancels_opposing_prints_and_volume_mode_does_not() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 10.0),
            trade(4400.0, OptionType::Call, AggressorSide::Sell, 4.0),
        ];
        let net = positioning_table(&trades, PositioningMode::Net);
        assert_eq!(net[0].call_oi, 6.0);
        let volume = positioning_table(&trades, PositioningMode::Volume);
        assert_eq!(volume[0].call_oi, 14.0);
    }

    #[test]
    fn negative_net_positioning_is_clamped_but_preserved() {
        let trades = vec![trade(4400.0, OptionType::Call, AggressorSide::Sell, 3.0)];
        let rows = positioning_table(&trades, PositioningMode::Net);
        assert_eq!(rows[0].call_oi, 0.0, "a negative cannot enter a payout sum");
        assert_eq!(rows[0].net_call, -3.0, "but the raw net survives for models that want it");
    }

    #[test]
    fn the_payout_curve_bottoms_out_at_max_pain() {
        let rows = vec![row(4300.0, 10.0, 100.0), row(4400.0, 50.0, 50.0), row(4500.0, 100.0, 10.0)];
        let curve = payout_curve(&rows, 100.0);
        let lowest = curve
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(strike, _)| *strike)
            .unwrap();
        assert_eq!(Some(lowest), max_pain(&rows, 100.0, None));
    }

    #[test]
    fn candidate_settlements_can_sit_between_strikes() {
        let rows = vec![row(4300.0, 10.0, 100.0), row(4500.0, 100.0, 10.0)];
        let candidates = [4300.0, 4350.0, 4400.0, 4450.0, 4500.0];
        let answer = max_pain(&rows, 100.0, Some(&candidates)).unwrap();
        assert!(candidates.contains(&answer));
    }

    #[test]
    fn the_table_is_sorted_by_strike() {
        let trades = vec![
            trade(4500.0, OptionType::Call, AggressorSide::Buy, 1.0),
            trade(4300.0, OptionType::Put, AggressorSide::Buy, 1.0),
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 1.0),
        ];
        let rows = positioning_table(&trades, PositioningMode::Volume);
        let strikes: Vec<f64> = rows.iter().map(|r| r.strike).collect();
        assert_eq!(strikes, vec![4300.0, 4400.0, 4500.0]);
    }
}
