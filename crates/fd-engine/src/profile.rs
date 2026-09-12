//! Strike profile — the "premium profile" of the reference dashboard.
//!
//! A histogram of activity across strikes, split into calls and puts, plus the
//! point of control and a value area. Calibration against the reference feed
//! showed the premium-weighted mode reproducing its POC on three of four
//! contracts, so [`ProfileMode::Premium`] is the default — but every candidate
//! metric stays available, because one market's answer is not proof.

use std::collections::BTreeMap;

use fd_core::classify::position_sign;
use fd_core::types::{OptionTrade, OptionType};
use serde::{Deserialize, Serialize};

/// What a print contributes to its strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileMode {
    /// USD premium. CALIBRATED — closest to the reference POC.
    #[default]
    Premium,
    /// Traded contracts.
    Volume,
    /// Premium signed by which side aggressed.
    NetPremium,
    /// Magnitude of the signed premium.
    AbsNetPremium,
    /// Signed contracts. A proxy only — no feed here publishes open interest.
    OpenInterest,
}

impl ProfileMode {
    pub const ALL: [Self; 5] =
        [Self::Premium, Self::Volume, Self::NetPremium, Self::AbsNetPremium, Self::OpenInterest];

    /// Inverse of [`Self::as_str`], for reading a configured mode.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == id)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Premium => "PREMIUM",
            Self::Volume => "VOLUME",
            Self::NetPremium => "NET_PREMIUM",
            Self::AbsNetPremium => "ABS_NET_PREMIUM",
            Self::OpenInterest => "OPEN_INTEREST",
        }
    }

    fn contribution(self, trade: &OptionTrade) -> f64 {
        match self {
            Self::Premium => trade.premium_usd,
            Self::Volume => trade.contracts,
            Self::NetPremium => f64::from(position_sign(trade.flow_class)) * trade.premium_usd,
            Self::AbsNetPremium => (f64::from(position_sign(trade.flow_class)) * trade.premium_usd).abs(),
            Self::OpenInterest => f64::from(position_sign(trade.flow_class)) * trade.contracts,
        }
    }
}

/// A completed profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrikeProfile {
    pub mode: ProfileMode,
    /// Ascending.
    pub strikes: Vec<f64>,
    pub call: Vec<f64>,
    pub put: Vec<f64>,
    /// `|call| + |put|` per strike — magnitude, so signed modes still rank.
    pub total: Vec<f64>,
    /// Strike carrying the largest total.
    pub poc: Option<f64>,
    /// Value-area high and low.
    pub vah: Option<f64>,
    pub val: Option<f64>,
    /// Next profile node above and below the POC — what the dashboard draws as
    /// the next magnet in each direction.
    pub above_poc: Option<f64>,
    pub under_poc: Option<f64>,
    pub value_area_pct: f64,
}

/// Options for [`build_profile`].
#[derive(Debug, Clone, Copy)]
pub struct ProfileOptions {
    pub mode: ProfileMode,
    pub value_area_pct: f64,
    /// Bucket strikes to a step before aggregating. `None` keeps them as traded.
    pub strike_step: Option<f64>,
}

impl Default for ProfileOptions {
    fn default() -> Self {
        Self { mode: ProfileMode::Premium, value_area_pct: 0.7, strike_step: None }
    }
}

/// Round to two decimals, matching the oracle's serialisation of profile rows.
///
/// `Math.round` semantics, not Rust's: a negative half goes towards zero there
/// and away from it here, and net premium is signed.
fn round2(v: f64) -> f64 {
    fd_core::js_round_to(v, 2)
}

/// Build a profile over one contract's prints.
#[must_use]
pub fn build_profile(trades: &[OptionTrade], options: ProfileOptions) -> StrikeProfile {
    // BTreeMap keyed by the bit pattern keeps strikes ordered without requiring
    // f64: Ord, and gives a deterministic iteration order — a profile whose key
    // order varies would make parity comparisons meaningless.
    let mut buckets: BTreeMap<u64, (f64, f64, f64)> = BTreeMap::new();

    for trade in trades {
        if !trade.strike.is_finite() {
            continue;
        }
        let strike = match options.strike_step {
            Some(step) if step > 0.0 => (trade.strike / step).round() * step,
            _ => trade.strike,
        };
        let value = options.mode.contribution(trade);
        let entry = buckets.entry(strike.to_bits()).or_insert((strike, 0.0, 0.0));
        match trade.option_type {
            OptionType::Call => entry.1 += value,
            OptionType::Put => entry.2 += value,
        }
    }

    let mut rows: Vec<(f64, f64, f64)> = buckets.into_values().collect();
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("strikes are finite"));

    let strikes: Vec<f64> = rows.iter().map(|r| r.0).collect();
    let call: Vec<f64> = rows.iter().map(|r| round2(r.1)).collect();
    let put: Vec<f64> = rows.iter().map(|r| round2(r.2)).collect();
    let total: Vec<f64> = call.iter().zip(&put).map(|(c, p)| round2(c.abs() + p.abs())).collect();

    let area = value_area(&strikes, &total, options.value_area_pct);
    let above_poc = area.poc_index.and_then(|i| next_node(&strikes, &total, i, 1));
    let under_poc = area.poc_index.and_then(|i| next_node(&strikes, &total, i, -1));

    StrikeProfile {
        mode: options.mode,
        strikes,
        call,
        put,
        total,
        poc: area.poc,
        vah: area.vah,
        val: area.val,
        above_poc,
        under_poc,
        value_area_pct: options.value_area_pct,
    }
}

/// Result of the value-area walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueArea {
    pub poc: Option<f64>,
    pub vah: Option<f64>,
    pub val: Option<f64>,
    pub poc_index: Option<usize>,
}

/// Classic value-area walk: start at the point of control and repeatedly take
/// the heavier of the two neighbouring rows until the target share of total
/// value is enclosed.
#[must_use]
pub fn value_area(strikes: &[f64], total: &[f64], pct: f64) -> ValueArea {
    if strikes.is_empty() {
        return ValueArea { poc: None, vah: None, val: None, poc_index: None };
    }

    let mut poc_index = 0usize;
    for i in 1..total.len() {
        if total[i] > total[poc_index] {
            poc_index = i;
        }
    }

    let sum: f64 = total.iter().sum();
    let target = sum * pct;
    let (mut lo, mut hi) = (poc_index, poc_index);
    let mut acc = total[poc_index];

    while acc < target && (lo > 0 || hi < total.len() - 1) {
        let below = if lo > 0 { total[lo - 1] } else { f64::NEG_INFINITY };
        let above = if hi < total.len() - 1 { total[hi + 1] } else { f64::NEG_INFINITY };
        if above >= below {
            hi += 1;
            acc += total[hi];
        } else {
            lo -= 1;
            acc += total[lo];
        }
    }

    ValueArea {
        poc: Some(strikes[poc_index]),
        vah: Some(strikes[hi]),
        val: Some(strikes[lo]),
        poc_index: Some(poc_index),
    }
}

/// First local peak away from the POC in `direction`, falling back to the
/// largest row on that side when the profile has no clean peak.
fn next_node(strikes: &[f64], total: &[f64], poc_index: usize, direction: i32) -> Option<f64> {
    let step = |i: usize| -> Option<usize> {
        if direction > 0 {
            let next = i + 1;
            (next < strikes.len()).then_some(next)
        } else {
            i.checked_sub(1)
        }
    };

    let mut i = step(poc_index);
    while let Some(index) = i {
        let prev = if direction > 0 { total.get(index.wrapping_sub(1)) } else { total.get(index + 1) };
        let next = if direction > 0 { total.get(index + 1) } else { index.checked_sub(1).and_then(|j| total.get(j)) };
        let is_peak = total[index] >= prev.copied().unwrap_or(f64::NEG_INFINITY)
            && total[index] >= next.copied().unwrap_or(f64::NEG_INFINITY);
        if is_peak {
            return Some(strikes[index]);
        }
        i = step(index);
    }

    // No peak: take the largest row on that side instead of returning nothing.
    let mut best: Option<(f64, f64)> = None;
    let mut i = step(poc_index);
    while let Some(index) = i {
        let candidate = (total[index], strikes[index]);
        if best.is_none_or(|(value, _)| candidate.0 > value) {
            best = Some(candidate);
        }
        i = step(index);
    }
    best.map(|(_, strike)| strike)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::{FlowClass, classify_flow};
    use fd_core::types::{AggressorSide, TradeFlags};

    fn trade(strike: f64, option_type: OptionType, side: AggressorSide, contracts: f64, premium: f64) -> OptionTrade {
        OptionTrade {
            id: format!("{strike}-{}", option_type.letter()),
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
    fn premium_poc_is_the_strike_with_the_most_premium() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 1000.0),
            trade(4500.0, OptionType::Call, AggressorSide::Buy, 1.0, 3000.0),
            trade(4500.0, OptionType::Put, AggressorSide::Buy, 1.0, 500.0),
            trade(4300.0, OptionType::Put, AggressorSide::Buy, 1.0, 2000.0),
        ];
        let profile = build_profile(&trades, ProfileOptions::default());
        assert_eq!(profile.poc, Some(4500.0));
        assert_eq!(profile.strikes, vec![4300.0, 4400.0, 4500.0]);
        assert_eq!(profile.call, vec![0.0, 1000.0, 3000.0]);
        assert_eq!(profile.put, vec![2000.0, 0.0, 500.0]);
        assert_eq!(profile.total, vec![2000.0, 1000.0, 3500.0]);
    }

    #[test]
    fn volume_mode_counts_contracts_not_dollars() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 50.0, 10.0),
            trade(4500.0, OptionType::Call, AggressorSide::Buy, 1.0, 100_000.0),
        ];
        let premium = build_profile(&trades, ProfileOptions::default());
        let volume = build_profile(&trades, ProfileOptions { mode: ProfileMode::Volume, ..Default::default() });
        assert_eq!(premium.poc, Some(4500.0));
        assert_eq!(volume.poc, Some(4400.0));
    }

    #[test]
    fn signed_modes_still_rank_by_magnitude() {
        // One buyer-aggressed and one seller-aggressed print at the same strike
        // cancel in NET_PREMIUM but not in its absolute variant.
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 1000.0),
            trade(4400.0, OptionType::Call, AggressorSide::Sell, 1.0, 1000.0),
            trade(4500.0, OptionType::Call, AggressorSide::Buy, 1.0, 400.0),
        ];
        let net = build_profile(&trades, ProfileOptions { mode: ProfileMode::NetPremium, ..Default::default() });
        assert_eq!(net.poc, Some(4500.0), "cancelled strike must not be the POC");
        let abs = build_profile(&trades, ProfileOptions { mode: ProfileMode::AbsNetPremium, ..Default::default() });
        assert_eq!(abs.poc, Some(4400.0));
    }

    #[test]
    fn value_area_walks_outward_from_the_poc() {
        let strikes = [1.0, 2.0, 3.0, 4.0, 5.0];
        let total = [1.0, 2.0, 10.0, 2.0, 1.0];
        let area = value_area(&strikes, &total, 0.7);
        assert_eq!(area.poc, Some(3.0));
        assert!(area.val.unwrap() <= 3.0 && area.vah.unwrap() >= 3.0);
    }

    #[test]
    fn an_empty_tape_produces_an_empty_profile_not_a_panic() {
        let profile = build_profile(&[], ProfileOptions::default());
        assert!(profile.strikes.is_empty());
        assert_eq!(profile.poc, None);
        assert_eq!(profile.above_poc, None);
    }

    #[test]
    fn neighbouring_nodes_sit_on_opposite_sides_of_the_poc() {
        let trades = vec![
            trade(4200.0, OptionType::Put, AggressorSide::Buy, 1.0, 900.0),
            trade(4300.0, OptionType::Put, AggressorSide::Buy, 1.0, 200.0),
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 5000.0),
            trade(4500.0, OptionType::Call, AggressorSide::Buy, 1.0, 300.0),
            trade(4600.0, OptionType::Call, AggressorSide::Buy, 1.0, 1200.0),
        ];
        let profile = build_profile(&trades, ProfileOptions::default());
        assert_eq!(profile.poc, Some(4400.0));
        assert!(profile.above_poc.unwrap() > 4400.0);
        assert!(profile.under_poc.unwrap() < 4400.0);
    }

    #[test]
    fn strike_bucketing_merges_nearby_strikes() {
        let trades = vec![
            trade(4401.0, OptionType::Call, AggressorSide::Buy, 1.0, 100.0),
            trade(4399.0, OptionType::Call, AggressorSide::Buy, 1.0, 100.0),
        ];
        let bucketed =
            build_profile(&trades, ProfileOptions { strike_step: Some(50.0), ..Default::default() });
        assert_eq!(bucketed.strikes, vec![4400.0]);
        assert_eq!(bucketed.call, vec![200.0]);
    }

    #[test]
    fn flow_class_drives_the_sign_in_signed_modes() {
        let buy = trade(4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 100.0);
        let sell = trade(4400.0, OptionType::Call, AggressorSide::Sell, 1.0, 100.0);
        assert_eq!(buy.flow_class, FlowClass::Lc);
        assert_eq!(sell.flow_class, FlowClass::Sc);
        assert!(ProfileMode::NetPremium.contribution(&buy) > 0.0);
        assert!(ProfileMode::NetPremium.contribution(&sell) < 0.0);
    }
}
