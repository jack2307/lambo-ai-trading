//! Whale support and resistance — **unverified**.
//!
//! Naming first, because the prototype's research report got this wrong: the
//! reference feed calls these `whale_sup` / `whale_res`. They are anchored by
//! **large prints**, not by a weighted blend of open interest and premium as
//! "weighted support/resistance" would suggest.
//!
//! They are also sticky. On the observed monthly contract the resistance level
//! did not move across thousands of prints, which is why [`StickyWhaleTracker`]
//! keeps the last known level until a stronger print re-anchors it. One partial
//! confirmation exists — `whale_sup` equalled the strike with the largest
//! net-long put positioning inside the sampled window — and that is the whole
//! evidence base, hence three candidate models rather than one answer.

use std::collections::HashMap;

use fd_core::classify::position_sign;
use fd_core::types::{OptionTrade, OptionType};
use serde::{Deserialize, Serialize};

use crate::bigtrades::{BigTradeConfig, WhaleFootprint, select_big_trades};

/// Candidate whale-level formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Model {
    /// Strike with the largest net-long whale positioning on each side of spot.
    #[default]
    NetLongWhale,
    /// Strike of the single largest print on each side of spot.
    LargestPrint,
    /// Strike with the largest aggregate premium on each side, all prints.
    PremiumConcentration,
}

/// A support/resistance pair with the evidence that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct WhaleLevels {
    pub support: Option<f64>,
    pub resistance: Option<f64>,
}

impl Model {
    pub const ALL: [Self; 3] = [Self::NetLongWhale, Self::LargestPrint, Self::PremiumConcentration];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NetLongWhale => "net-long-whale",
            Self::LargestPrint => "largest-print",
            Self::PremiumConcentration => "premium-concentration",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::NetLongWhale => "Strike with the largest net-long whale positioning (puts below spot, calls above).",
            Self::LargestPrint => "Strike of the single largest put print below spot / call print above spot.",
            Self::PremiumConcentration => "Strike with the largest aggregate put premium below spot / call premium above spot.",
        }
    }

    /// Compute both levels.
    ///
    /// Every comparison is a strict `>`, so on a tie the strike encountered
    /// first wins. "First" means insertion order, which is why the footprint
    /// preserves it.
    #[must_use]
    pub fn calculate(self, trades: &[OptionTrade], spot: f64, config: &BigTradeConfig) -> WhaleLevels {
        match self {
            Self::NetLongWhale => {
                let big = select_big_trades(trades, &BigTradeConfig { limit: usize::MAX, ..*config });
                let footprint = WhaleFootprint::build(&big);
                let mut levels = WhaleLevels::default();
                let (mut best_put, mut best_call) = (0.0, 0.0);
                for row in footprint.rows() {
                    if row.strike <= spot && row.put_net > best_put {
                        best_put = row.put_net;
                        levels.support = Some(row.strike);
                    }
                    if row.strike >= spot && row.call_net > best_call {
                        best_call = row.call_net;
                        levels.resistance = Some(row.strike);
                    }
                }
                levels
            }

            Self::LargestPrint => {
                let big = select_big_trades(trades, &BigTradeConfig { limit: usize::MAX, ..*config });
                let mut levels = WhaleLevels::default();
                let (mut best_put, mut best_call) = (0.0, 0.0);
                for trade in &big {
                    if trade.option_type == OptionType::Put && trade.strike <= spot && trade.premium_usd > best_put {
                        best_put = trade.premium_usd;
                        levels.support = Some(trade.strike);
                    }
                    if trade.option_type == OptionType::Call && trade.strike >= spot && trade.premium_usd > best_call {
                        best_call = trade.premium_usd;
                        levels.resistance = Some(trade.strike);
                    }
                }
                levels
            }

            Self::PremiumConcentration => {
                let mut order: Vec<u64> = Vec::new();
                let mut totals: HashMap<u64, (f64, f64, f64)> = HashMap::new(); // strike, call, put
                for trade in trades {
                    let key = trade.strike.to_bits();
                    let entry = totals.entry(key).or_insert_with(|| {
                        order.push(key);
                        (trade.strike, 0.0, 0.0)
                    });
                    match trade.option_type {
                        OptionType::Call => entry.1 += trade.premium_usd,
                        OptionType::Put => entry.2 += trade.premium_usd,
                    }
                }
                let mut levels = WhaleLevels::default();
                let (mut best_put, mut best_call) = (0.0, 0.0);
                for key in order {
                    let (strike, call_premium, put_premium) = totals[&key];
                    if strike <= spot && put_premium > best_put {
                        best_put = put_premium;
                        levels.support = Some(strike);
                    }
                    if strike >= spot && call_premium > best_call {
                        best_call = call_premium;
                        levels.resistance = Some(strike);
                    }
                }
                levels
            }
        }
    }
}

/// Keeps a whale level until something stronger replaces it.
///
/// Without this, a level vanishes the moment the window it was found in stops
/// containing its anchoring print — which is not how the reference feed behaves
/// and not how a trader would read the level either.
#[derive(Debug, Clone)]
pub struct StickyWhaleTracker {
    model: Model,
    config: BigTradeConfig,
    current: WhaleLevels,
    seen: Vec<OptionTrade>,
}

impl StickyWhaleTracker {
    #[must_use]
    pub fn new(model: Model, config: BigTradeConfig) -> Self {
        Self { model, config, current: WhaleLevels::default(), seen: Vec::new() }
    }

    /// Feed the prints that arrived since the last call.
    pub fn update(&mut self, new_trades: &[OptionTrade], spot: f64) -> WhaleLevels {
        self.seen.extend_from_slice(new_trades);
        let next = self.model.calculate(&self.seen, spot, &self.config);
        if next.support.is_some() {
            self.current.support = next.support;
        }
        if next.resistance.is_some() {
            self.current.resistance = next.resistance;
        }
        self.current
    }

    #[must_use]
    pub fn current(&self) -> WhaleLevels {
        self.current
    }

    #[must_use]
    pub fn model(&self) -> Model {
        self.model
    }
}

/// Net positioning per strike, exported for research and calibration.
#[must_use]
pub fn net_positioning_by_strike(trades: &[OptionTrade]) -> Vec<(f64, f64, f64)> {
    let mut order: Vec<u64> = Vec::new();
    let mut totals: HashMap<u64, (f64, f64, f64)> = HashMap::new();
    for trade in trades {
        let sign = f64::from(position_sign(trade.flow_class));
        if sign == 0.0 {
            continue;
        }
        let key = trade.strike.to_bits();
        let entry = totals.entry(key).or_insert_with(|| {
            order.push(key);
            (trade.strike, 0.0, 0.0)
        });
        match trade.option_type {
            OptionType::Call => entry.1 += sign * trade.contracts,
            OptionType::Put => entry.2 += sign * trade.contracts,
        }
    }
    order.into_iter().map(|k| totals[&k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::{AggressorSide, TradeFlags};

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

    fn config() -> BigTradeConfig {
        BigTradeConfig { min_premium_usd: 100_000.0, percentile: None, ..Default::default() }
    }

    #[test]
    fn ids_round_trip() {
        for model in Model::ALL {
            assert_eq!(Model::from_id(model.id()), Some(model));
        }
        assert_eq!(Model::from_id("weighted-support"), None, "the old misreading is not a model");
    }

    #[test]
    fn whale_levels_sit_on_the_right_side_of_spot() {
        let trades = vec![
            trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 60.0, 240_000.0),
            trade(1, 5000.0, OptionType::Call, AggressorSide::Buy, 80.0, 240_000.0),
            trade(2, 4350.0, OptionType::Call, AggressorSide::Buy, 1.0, 100.0), // noise
        ];
        let levels = Model::NetLongWhale.calculate(&trades, 4350.0, &config());
        assert_eq!(levels.support, Some(4200.0));
        assert_eq!(levels.resistance, Some(5000.0));
    }

    #[test]
    fn the_largest_print_model_follows_premium_not_net_size() {
        let trades = vec![
            // Bigger net size, smaller single print.
            trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 50.0, 150_000.0),
            trade(1, 4200.0, OptionType::Put, AggressorSide::Buy, 50.0, 150_000.0),
            // One huge print.
            trade(2, 4100.0, OptionType::Put, AggressorSide::Buy, 10.0, 400_000.0),
        ];
        assert_eq!(Model::LargestPrint.calculate(&trades, 4350.0, &config()).support, Some(4100.0));
        assert_eq!(Model::NetLongWhale.calculate(&trades, 4350.0, &config()).support, Some(4200.0));
    }

    #[test]
    fn premium_concentration_reads_every_print_not_just_the_big_ones() {
        // Many small prints that no whale filter would keep.
        let trades: Vec<OptionTrade> = (0..20)
            .map(|i| trade(i, 4250.0, OptionType::Put, AggressorSide::Buy, 1.0, 5_000.0))
            .collect();
        assert_eq!(Model::PremiumConcentration.calculate(&trades, 4350.0, &config()).support, Some(4250.0));
        assert_eq!(Model::NetLongWhale.calculate(&trades, 4350.0, &config()).support, None);
    }

    #[test]
    fn a_tape_with_nothing_large_produces_no_levels() {
        let trades = vec![trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 1.0, 100.0)];
        let levels = Model::NetLongWhale.calculate(&trades, 4350.0, &config());
        assert_eq!(levels, WhaleLevels::default());
    }

    #[test]
    fn the_tracker_holds_a_level_until_something_stronger_arrives() {
        let mut tracker = StickyWhaleTracker::new(Model::NetLongWhale, config());

        let first = tracker.update(&[trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 60.0, 240_000.0)], 4350.0);
        assert_eq!(first.support, Some(4200.0));

        // A later batch with no qualifying put must not erase it.
        let second = tracker.update(&[trade(1, 4360.0, OptionType::Call, AggressorSide::Buy, 1.0, 50.0)], 4350.0);
        assert_eq!(second.support, Some(4200.0), "the level is sticky");

        // A bigger put re-anchors it.
        let third = tracker.update(&[trade(2, 4100.0, OptionType::Put, AggressorSide::Buy, 200.0, 800_000.0)], 4350.0);
        assert_eq!(third.support, Some(4100.0));
    }

    #[test]
    fn a_tie_goes_to_the_strike_seen_first() {
        let trades = vec![
            trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 50.0, 200_000.0),
            trade(1, 4100.0, OptionType::Put, AggressorSide::Buy, 50.0, 200_000.0),
        ];
        // Prints are scanned newest-first, so 4100 is encountered first.
        assert_eq!(Model::NetLongWhale.calculate(&trades, 4350.0, &config()).support, Some(4100.0));
    }

    #[test]
    fn net_positioning_skips_prints_with_no_aggressor() {
        let trades = vec![
            trade(0, 4200.0, OptionType::Put, AggressorSide::Buy, 10.0, 1000.0),
            trade(1, 4200.0, OptionType::Put, AggressorSide::Unknown, 99.0, 1000.0),
        ];
        let rows = net_positioning_by_strike(&trades);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, 10.0);
    }
}
