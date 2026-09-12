//! Flow classification.
//!
//! The four buckets the whole methodology rests on:
//!
//! ```text
//! call + buyer aggression  -> LC        call + seller aggression -> SC
//! put  + buyer aggression  -> LP        put  + seller aggression -> SP
//!
//! bull = LC + SP                        bear = LP + SC
//! ```
//!
//! What these mean, precisely: **which side crossed the spread**. They do not
//! mean a position was opened. A buyer-aggressed call may be opening a long or
//! closing a short, and no tape in this system distinguishes the two. Names
//! like "long call" are the industry's shorthand, not a claim about intent.

use serde::{Deserialize, Serialize};

use crate::types::{AggressorSide, OptionType};

/// Buyer- or seller-aggressed, per option type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FlowClass {
    /// Call, buyer-aggressed.
    Lc,
    /// Put, buyer-aggressed.
    Lp,
    /// Call, seller-aggressed.
    Sc,
    /// Put, seller-aggressed.
    Sp,
    Unknown,
}

impl FlowClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lc => "LC",
            Self::Lp => "LP",
            Self::Sc => "SC",
            Self::Sp => "SP",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// `call + buy -> LC`, and the three mirrors of it.
#[must_use]
pub const fn classify_flow(option_type: OptionType, side: AggressorSide) -> FlowClass {
    match (option_type, side) {
        (OptionType::Call, AggressorSide::Buy) => FlowClass::Lc,
        (OptionType::Put, AggressorSide::Buy) => FlowClass::Lp,
        (OptionType::Call, AggressorSide::Sell) => FlowClass::Sc,
        (OptionType::Put, AggressorSide::Sell) => FlowClass::Sp,
        (_, AggressorSide::Unknown) => FlowClass::Unknown,
    }
}

/// Bull bucket: buyer-aggressed calls and seller-aggressed puts.
#[must_use]
pub const fn is_bull(class: FlowClass) -> bool {
    matches!(class, FlowClass::Lc | FlowClass::Sp)
}

/// Bear bucket: buyer-aggressed puts and seller-aggressed calls.
#[must_use]
pub const fn is_bear(class: FlowClass) -> bool {
    matches!(class, FlowClass::Lp | FlowClass::Sc)
}

/// Direction a print pushes positioning at its strike.
///
/// Used by the positioning proxy that stands in for open interest — the feeds
/// carry none, so "max pain" here is derived from the tape and must never be
/// presented as the exchange's OI max pain.
#[must_use]
pub const fn position_sign(class: FlowClass) -> i8 {
    match class {
        FlowClass::Lc | FlowClass::Lp => 1,
        FlowClass::Sc | FlowClass::Sp => -1,
        FlowClass::Unknown => 0,
    }
}

/// Quote-rule aggressor inference, for feeds that publish no aggressor flag.
///
/// An exchange-provided flag always wins over this; Deribit gives one, the gold
/// tape does not. Order of evidence: trade at the ask or bid, then which half of
/// the spread it landed in, then the tick rule against the previous print.
#[must_use]
pub fn infer_aggressor(trade_price: f64, bid: Option<f64>, ask: Option<f64>, prev_price: Option<f64>) -> AggressorSide {
    const EPS: f64 = 1e-8;

    if let (Some(bid), Some(ask)) = (bid, ask)
        && ask >= bid
    {
        if trade_price >= ask - EPS {
            return AggressorSide::Buy;
        }
        if trade_price <= bid + EPS {
            return AggressorSide::Sell;
        }
        let mid = (bid + ask) / 2.0;
        if trade_price > mid + EPS {
            return AggressorSide::Buy;
        }
        if trade_price < mid - EPS {
            return AggressorSide::Sell;
        }
    }

    if let Some(prev) = prev_price {
        if trade_price > prev + EPS {
            return AggressorSide::Buy;
        }
        if trade_price < prev - EPS {
            return AggressorSide::Sell;
        }
    }

    AggressorSide::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_buckets_match_the_reference_dashboard() {
        assert_eq!(classify_flow(OptionType::Call, AggressorSide::Buy), FlowClass::Lc);
        assert_eq!(classify_flow(OptionType::Put, AggressorSide::Buy), FlowClass::Lp);
        assert_eq!(classify_flow(OptionType::Call, AggressorSide::Sell), FlowClass::Sc);
        assert_eq!(classify_flow(OptionType::Put, AggressorSide::Sell), FlowClass::Sp);
        assert_eq!(classify_flow(OptionType::Call, AggressorSide::Unknown), FlowClass::Unknown);
    }

    #[test]
    fn bull_is_lc_plus_sp_and_bear_is_lp_plus_sc() {
        assert!(is_bull(FlowClass::Lc) && is_bull(FlowClass::Sp));
        assert!(is_bear(FlowClass::Lp) && is_bear(FlowClass::Sc));
        assert!(!is_bull(FlowClass::Lp) && !is_bear(FlowClass::Lc));
        assert!(!is_bull(FlowClass::Unknown) && !is_bear(FlowClass::Unknown));
    }

    #[test]
    fn position_sign_follows_who_crossed_the_spread() {
        assert_eq!(position_sign(FlowClass::Lc), 1);
        assert_eq!(position_sign(FlowClass::Lp), 1);
        assert_eq!(position_sign(FlowClass::Sc), -1);
        assert_eq!(position_sign(FlowClass::Sp), -1);
        assert_eq!(position_sign(FlowClass::Unknown), 0);
    }

    #[test]
    fn quote_rule_then_tick_rule() {
        assert_eq!(infer_aggressor(10.5, Some(10.0), Some(10.5), None), AggressorSide::Buy);
        assert_eq!(infer_aggressor(10.0, Some(10.0), Some(10.5), None), AggressorSide::Sell);
        // Exactly at the mid tells us nothing.
        assert_eq!(infer_aggressor(10.25, Some(10.0), Some(10.5), None), AggressorSide::Unknown);
        assert_eq!(infer_aggressor(10.4, None, None, Some(10.1)), AggressorSide::Buy);
        assert_eq!(infer_aggressor(10.0, None, None, Some(10.0)), AggressorSide::Unknown);
    }

    #[test]
    fn inverted_quotes_are_ignored_rather_than_trusted() {
        // A crossed book is bad data; fall through to the tick rule.
        assert_eq!(infer_aggressor(10.4, Some(11.0), Some(9.0), Some(10.1)), AggressorSide::Buy);
    }
}
