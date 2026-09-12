//! Call BE / Put BE — **unverified**.
//!
//! The reference platform publishes these levels, nothing in its API explains
//! how, and a five-day slice of a monthly contract's tape cannot reproduce them
//! because the level is already established at the window's first print. So
//! every candidate lives behind one interface, selectable at runtime, and none
//! of them is called "the" break-even.
//!
//! # A multiplier bug carried over from the prototype
//!
//! The JavaScript [`Model::AggregatePayoff`] divides total premium by a
//! hard-coded `100` — gold's contract multiplier — when solving for the price at
//! which payout repays cost. That is right for gold and wrong for BTC, whose
//! contracts are 1 BTC and whose premium is already in USD, so the prototype
//! overstates BTC's aggregate-payoff break-even by a factor of a hundred.
//!
//! Here the multiplier is an explicit argument. Production passes the market's
//! own value; the parity gate passes `100.0` for both markets, because its job
//! is to prove the port did not change the arithmetic — not to hide that the
//! arithmetic was wrong. The correction is tested separately.

use std::collections::HashMap;

use fd_core::classify::position_sign;
use fd_core::types::{AggressorSide, OptionTrade, OptionType};
use serde::{Deserialize, Serialize};

/// The candidate break-even formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Model {
    /// Premium-weighted mean of `strike ± trade price`, all prints.
    #[default]
    PremiumWeighted,
    /// The same, restricted to buyer-aggressed prints.
    PremiumWeightedLong,
    /// Contract-weighted rather than premium-weighted.
    SizeWeighted,
    /// Settlement price at which total intrinsic payout repays total premium.
    AggregatePayoff,
    /// Break-even of net-long positioning per strike.
    NetPositioning,
}

impl Model {
    pub const ALL: [Self; 5] = [
        Self::PremiumWeighted,
        Self::PremiumWeightedLong,
        Self::SizeWeighted,
        Self::AggregatePayoff,
        Self::NetPositioning,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PremiumWeighted => "premium-weighted",
            Self::PremiumWeightedLong => "premium-weighted-long",
            Self::SizeWeighted => "size-weighted",
            Self::AggregatePayoff => "aggregate-payoff",
            Self::NetPositioning => "net-positioning",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::PremiumWeighted => "Weighted mean of (strike ± trade price) using premium as the weight, all prints.",
            Self::PremiumWeightedLong => "Premium-weighted break-even over buyer-aggressed prints only.",
            Self::SizeWeighted => "Contract-weighted mean of (strike ± trade price).",
            Self::AggregatePayoff => "Settlement price where total intrinsic payout repays total premium.",
            Self::NetPositioning => "Break-even of net-long positioning per strike, weighted by net contracts.",
        }
    }

    /// Call break-even, or `None` when the tape offers nothing to compute from.
    #[must_use]
    pub fn call_break_even(self, trades: &[OptionTrade], multiplier: f64) -> Option<f64> {
        let calls: Vec<&OptionTrade> = trades.iter().filter(|t| t.option_type == OptionType::Call).collect();
        self.solve(&calls, 1.0, multiplier)
    }

    /// Put break-even.
    #[must_use]
    pub fn put_break_even(self, trades: &[OptionTrade], multiplier: f64) -> Option<f64> {
        let puts: Vec<&OptionTrade> = trades.iter().filter(|t| t.option_type == OptionType::Put).collect();
        self.solve(&puts, -1.0, multiplier)
    }

    /// `sign` is +1 for calls (premium is paid above the strike) and −1 for puts.
    fn solve(self, selection: &[&OptionTrade], sign: f64, multiplier: f64) -> Option<f64> {
        match self {
            Self::PremiumWeighted => {
                weighted_mean(selection, |t| t.strike + sign * t.trade_price, |t| t.premium_usd)
            }
            Self::PremiumWeightedLong => {
                let buyers: Vec<&OptionTrade> =
                    selection.iter().copied().filter(|t| t.aggressor_side == AggressorSide::Buy).collect();
                weighted_mean(&buyers, |t| t.strike + sign * t.trade_price, |t| t.premium_usd)
            }
            Self::SizeWeighted => weighted_mean(selection, |t| t.strike + sign * t.trade_price, |t| t.contracts),
            Self::AggregatePayoff => aggregate_payoff(selection, sign, multiplier),
            Self::NetPositioning => net_positioning(selection, sign),
        }
    }
}

/// Weighted mean, skipping non-positive or non-finite weights.
fn weighted_mean<V, W>(selection: &[&OptionTrade], value: V, weight: W) -> Option<f64>
where
    V: Fn(&OptionTrade) -> f64,
    W: Fn(&OptionTrade) -> f64,
{
    let mut num = 0.0;
    let mut den = 0.0;
    for trade in selection {
        let w = weight(trade);
        if !w.is_finite() || w <= 0.0 {
            continue;
        }
        num += value(trade) * w;
        den += w;
    }
    (den > 0.0).then(|| num / den)
}

/// Solve for the settlement price at which intrinsic payout equals total cost.
fn aggregate_payoff(selection: &[&OptionTrade], sign: f64, multiplier: f64) -> Option<f64> {
    if selection.is_empty() {
        return None;
    }
    let cost: f64 = selection.iter().map(|t| t.premium_usd).sum::<f64>() / multiplier;
    let strikes: Vec<f64> = selection.iter().map(|t| t.strike).collect();
    let min_strike = strikes.iter().copied().fold(f64::INFINITY, f64::min);
    let max_strike = strikes.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Payout rises with settlement for calls and falls for puts, so a plain
    // bisection converges; 120 halvings is far past f64 resolution.
    let payout_at = |s: f64| -> f64 {
        selection
            .iter()
            .map(|t| {
                let intrinsic = if sign > 0.0 { s - t.strike } else { t.strike - s };
                intrinsic.max(0.0) * t.contracts
            })
            .sum()
    };

    let (mut lo, mut hi) = if sign > 0.0 { (min_strike, max_strike * 3.0) } else { (0.0, max_strike) };
    for _ in 0..120 {
        let mid = (lo + hi) / 2.0;
        let value = payout_at(mid);
        let below_target = if sign > 0.0 { value < cost } else { value > cost };
        if below_target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

/// Break-even of the net-long book: only strikes where buyers out-aggressed
/// sellers define a cost basis at all.
fn net_positioning(selection: &[&OptionTrade], sign: f64) -> Option<f64> {
    struct Slot {
        qty: f64,
        cost: f64,
        strike: f64,
    }
    let mut order: Vec<u64> = Vec::new();
    let mut slots: HashMap<u64, Slot> = HashMap::new();

    for trade in selection {
        let s = f64::from(position_sign(trade.flow_class));
        if s == 0.0 {
            continue;
        }
        let key = trade.strike.to_bits();
        let slot = slots.entry(key).or_insert_with(|| {
            order.push(key);
            Slot { qty: 0.0, cost: 0.0, strike: trade.strike }
        });
        slot.qty += s * trade.contracts;
        slot.cost += s * trade.trade_price * trade.contracts;
    }

    let mut num = 0.0;
    let mut den = 0.0;
    for key in order {
        let slot = &slots[&key];
        if slot.qty <= 0.0 {
            continue;
        }
        let avg_price = slot.cost / slot.qty;
        num += (slot.strike + sign * avg_price) * slot.qty;
        den += slot.qty;
    }
    (den > 0.0).then(|| num / den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::TradeFlags;

    fn trade(strike: f64, option_type: OptionType, side: AggressorSide, price: f64, contracts: f64, multiplier: f64) -> OptionTrade {
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
            trade_price: price,
            contracts,
            bid: None,
            ask: None,
            aggressor_side: side,
            flow_class: classify_flow(option_type, side),
            premium_usd: price * contracts * multiplier,
            underlying_price: 0.0,
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags::default(),
            source: "test".into(),
        }
    }

    #[test]
    fn ids_round_trip_and_cover_every_model() {
        for model in Model::ALL {
            assert_eq!(Model::from_id(model.id()), Some(model));
        }
        assert_eq!(Model::from_id("nope"), None);
    }

    #[test]
    fn a_single_call_breaks_even_at_strike_plus_price() {
        let trades = vec![trade(4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 5.0, 100.0)];
        assert_eq!(Model::PremiumWeighted.call_break_even(&trades, 100.0), Some(4420.0));
        let aggregate = Model::AggregatePayoff.call_break_even(&trades, 100.0).unwrap();
        assert!((aggregate - 4420.0).abs() < 0.01, "got {aggregate}");
    }

    #[test]
    fn a_single_put_breaks_even_at_strike_minus_price() {
        let trades = vec![trade(4300.0, OptionType::Put, AggressorSide::Buy, 15.0, 5.0, 100.0)];
        assert_eq!(Model::PremiumWeighted.put_break_even(&trades, 100.0), Some(4285.0));
        let aggregate = Model::AggregatePayoff.put_break_even(&trades, 100.0).unwrap();
        assert!((aggregate - 4285.0).abs() < 0.01, "got {aggregate}");
    }

    #[test]
    fn every_model_returns_a_number_or_none_but_never_nan() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 5.0, 100.0),
            trade(4500.0, OptionType::Call, AggressorSide::Sell, 10.0, 5.0, 100.0),
            trade(4300.0, OptionType::Put, AggressorSide::Buy, 15.0, 5.0, 100.0),
        ];
        for model in Model::ALL {
            for v in [model.call_break_even(&trades, 100.0), model.put_break_even(&trades, 100.0)]
                .into_iter()
                .flatten()
            {
                assert!(v.is_finite(), "{} produced {v}", model.id());
            }
        }
    }

    #[test]
    fn an_empty_tape_gives_none_from_every_model() {
        for model in Model::ALL {
            assert_eq!(model.call_break_even(&[], 100.0), None);
            assert_eq!(model.put_break_even(&[], 100.0), None);
        }
    }

    #[test]
    fn the_long_only_model_ignores_seller_aggressed_prints() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 5.0, 100.0),
            // A seller-aggressed print far away would drag an all-prints mean.
            trade(5000.0, OptionType::Call, AggressorSide::Sell, 5.0, 100.0, 100.0),
        ];
        assert_eq!(Model::PremiumWeightedLong.call_break_even(&trades, 100.0), Some(4420.0));
        let all = Model::PremiumWeighted.call_break_even(&trades, 100.0).unwrap();
        assert!(all > 4420.0, "the all-prints model must be pulled by the far strike");
    }

    #[test]
    fn net_positioning_ignores_strikes_where_sellers_dominate() {
        let trades = vec![
            trade(4400.0, OptionType::Call, AggressorSide::Buy, 20.0, 10.0, 100.0),
            // Net short at 5000: contributes no cost basis.
            trade(5000.0, OptionType::Call, AggressorSide::Sell, 5.0, 50.0, 100.0),
        ];
        let be = Model::NetPositioning.call_break_even(&trades, 100.0).unwrap();
        assert!((be - 4420.0).abs() < 1e-9, "got {be}");
    }

    /// The prototype's hard-coded `/100` is right for gold and wrong for BTC.
    #[test]
    fn aggregate_payoff_respects_the_market_multiplier() {
        // One BTC call: 0.01 BTC premium at an index of 80,000 is $800 for one
        // contract of 1 BTC, so the break-even is 80,800 — not 80,008, which is
        // what dividing by gold's 100 would produce.
        let btc = vec![OptionTrade {
            premium_usd: 800.0,
            contracts: 1.0,
            strike: 80_000.0,
            trade_price: 0.01,
            ..trade(80_000.0, OptionType::Call, AggressorSide::Buy, 0.01, 1.0, 1.0)
        }];
        let correct = Model::AggregatePayoff.call_break_even(&btc, 1.0).unwrap();
        assert!((correct - 80_800.0).abs() < 0.01, "got {correct}");

        let prototype_bug = Model::AggregatePayoff.call_break_even(&btc, 100.0).unwrap();
        assert!((prototype_bug - 80_008.0).abs() < 0.01, "the oracle's behaviour, reproduced: {prototype_bug}");
    }
}
