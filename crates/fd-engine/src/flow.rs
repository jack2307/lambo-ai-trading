//! Bull / bear premium aggregation.
//!
//! ```text
//! bull = LC + SP        bear = LP + SC
//! bullRatio = bull / (bull + bear)
//! ```
//!
//! Prints whose aggressor is unknown land in neither bucket. Dropping them is
//! deliberate: assigning them a side would put invented conviction into the one
//! number the whole dashboard leans on.

use fd_core::classify::{FlowClass, is_bear, is_bull};
use fd_core::types::OptionTrade;
use serde::{Deserialize, Serialize};

/// Aggregated flow over some set of prints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FlowSummary {
    pub lc_premium: f64,
    pub lp_premium: f64,
    pub sc_premium: f64,
    pub sp_premium: f64,
    pub bull_premium: f64,
    pub bear_premium: f64,
    pub net_directional_premium: f64,
    /// 0..1. Exactly 0.5 when there is no flow at all, rather than NaN — a
    /// missing reading and a balanced one must not look the same downstream.
    pub bull_ratio: f64,
    pub trades: usize,
    pub contracts: f64,
}

impl Default for FlowSummary {
    fn default() -> Self {
        Self {
            lc_premium: 0.0,
            lp_premium: 0.0,
            sc_premium: 0.0,
            sp_premium: 0.0,
            bull_premium: 0.0,
            bear_premium: 0.0,
            net_directional_premium: 0.0,
            bull_ratio: 0.5,
            trades: 0,
            contracts: 0.0,
        }
    }
}

impl FlowSummary {
    pub fn add(&mut self, trade: &OptionTrade) {
        let premium = trade.premium_usd;
        match trade.flow_class {
            FlowClass::Lc => self.lc_premium += premium,
            FlowClass::Lp => self.lp_premium += premium,
            FlowClass::Sc => self.sc_premium += premium,
            FlowClass::Sp => self.sp_premium += premium,
            FlowClass::Unknown => {}
        }
        if is_bull(trade.flow_class) {
            self.bull_premium += premium;
        } else if is_bear(trade.flow_class) {
            self.bear_premium += premium;
        }
        self.trades += 1;
        self.contracts += trade.contracts;
        self.finalize();
    }

    /// Recompute the derived fields.
    pub fn finalize(&mut self) {
        self.net_directional_premium = self.bull_premium - self.bear_premium;
        let total = self.bull_premium + self.bear_premium;
        self.bull_ratio = if total > 0.0 { self.bull_premium / total } else { 0.5 };
    }
}

/// Summarise a whole slice.
#[must_use]
pub fn summarize(trades: &[OptionTrade]) -> FlowSummary {
    let mut acc = FlowSummary::default();
    for trade in trades {
        acc.add(trade);
    }
    acc.finalize();
    acc
}

/// Summarise the prints inside `[now - window, now]`.
///
/// Walks backwards from the end and stops at the window edge, so a long tape
/// costs the window rather than its whole length. Prints after `now` are
/// skipped — that is the replay guard: a window can never see the future.
#[must_use]
pub fn summarize_window(trades: &[OptionTrade], now_ms: i64, window_ms: i64) -> FlowSummary {
    let from = now_ms - window_ms;
    let mut acc = FlowSummary::default();
    for trade in trades.iter().rev() {
        if trade.timestamp > now_ms {
            continue;
        }
        if trade.timestamp < from {
            break;
        }
        acc.add(trade);
    }
    acc.finalize();
    acc
}

/// Change in net directional premium between the last window and the one before
/// it.
///
/// Cumulative flow hides regime changes: a tape can be overwhelmingly bullish
/// all week and turning hard right now. This is hypothesis H4 in measurable
/// form.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FlowVelocity {
    pub current: f64,
    pub previous: f64,
    pub velocity: f64,
    pub bull_velocity: f64,
    pub bear_velocity: f64,
}

#[must_use]
pub fn net_flow_velocity(trades: &[OptionTrade], now_ms: i64, window_ms: i64) -> FlowVelocity {
    let current = summarize_window(trades, now_ms, window_ms);
    let previous = summarize_window(trades, now_ms - window_ms, window_ms);
    FlowVelocity {
        current: current.net_directional_premium,
        previous: previous.net_directional_premium,
        velocity: current.net_directional_premium - previous.net_directional_premium,
        bull_velocity: current.bull_premium - previous.bull_premium,
        bear_velocity: current.bear_premium - previous.bear_premium,
    }
}

/// Bias label for a bull ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Bias {
    StrongBull,
    ModerateBull,
    Balanced,
    ModerateBear,
    StrongBear,
}

impl Bias {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StrongBull => "STRONG_BULL",
            Self::ModerateBull => "MODERATE_BULL",
            Self::Balanced => "BALANCED",
            Self::ModerateBear => "MODERATE_BEAR",
            Self::StrongBear => "STRONG_BEAR",
        }
    }
}

/// Thresholds are placeholders from the research report and have never been
/// backtested — which is why they arrive as a parameter rather than as
/// constants in here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiasThresholds {
    pub strong_bull: f64,
    pub moderate_bull: f64,
    pub moderate_bear: f64,
    pub strong_bear: f64,
}

#[must_use]
pub fn bias_label(bull_ratio: f64, t: BiasThresholds) -> Bias {
    if bull_ratio >= t.strong_bull {
        Bias::StrongBull
    } else if bull_ratio >= t.moderate_bull {
        Bias::ModerateBull
    } else if bull_ratio <= t.strong_bear {
        Bias::StrongBear
    } else if bull_ratio <= t.moderate_bear {
        Bias::ModerateBear
    } else {
        Bias::Balanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::{AggressorSide, OptionType, TradeFlags};

    fn trade(ts: i64, option_type: OptionType, side: AggressorSide, premium: f64) -> OptionTrade {
        OptionTrade {
            id: "t".into(),
            timestamp: ts,
            symbol: "TEST".into(),
            instrument: None,
            underlying: "TEST".into(),
            expiration: 0,
            dte: 1.0,
            strike: 4400.0,
            option_type,
            trade_price: 1.0,
            contracts: 1.0,
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

    const THRESHOLDS: BiasThresholds =
        BiasThresholds { strong_bull: 0.6, moderate_bull: 0.55, moderate_bear: 0.45, strong_bear: 0.4 };

    #[test]
    fn bull_is_lc_plus_sp_and_bear_is_lp_plus_sc() {
        let trades = vec![
            trade(0, OptionType::Call, AggressorSide::Buy, 1000.0),  // LC
            trade(1, OptionType::Put, AggressorSide::Sell, 1000.0),  // SP
            trade(2, OptionType::Put, AggressorSide::Buy, 400.0),    // LP
            trade(3, OptionType::Call, AggressorSide::Sell, 100.0),  // SC
        ];
        let s = summarize(&trades);
        assert_eq!(s.lc_premium, 1000.0);
        assert_eq!(s.sp_premium, 1000.0);
        assert_eq!(s.bull_premium, 2000.0);
        assert_eq!(s.bear_premium, 500.0);
        assert_eq!(s.net_directional_premium, 1500.0);
        assert_eq!(s.bull_ratio, 0.8);
        assert_eq!(s.trades, 4);
    }

    #[test]
    fn an_empty_tape_reads_balanced_rather_than_nan() {
        let s = summarize(&[]);
        assert_eq!(s.bull_ratio, 0.5);
        assert_eq!(s.trades, 0);
    }

    #[test]
    fn unknown_aggressors_count_as_prints_but_pick_no_side() {
        let trades = vec![trade(0, OptionType::Call, AggressorSide::Unknown, 5000.0)];
        let s = summarize(&trades);
        assert_eq!(s.trades, 1);
        assert_eq!(s.bull_premium, 0.0);
        assert_eq!(s.bear_premium, 0.0);
        assert_eq!(s.bull_ratio, 0.5);
    }

    #[test]
    fn a_window_ignores_prints_outside_it_and_after_now() {
        let now = 10_000_000;
        let trades = vec![
            trade(now - 3_600_000, OptionType::Call, AggressorSide::Buy, 1000.0),
            trade(now - 60_000, OptionType::Call, AggressorSide::Buy, 2000.0),
            trade(now + 60_000, OptionType::Call, AggressorSide::Buy, 9999.0),
        ];
        let w = summarize_window(&trades, now, 300_000);
        assert_eq!(w.trades, 1, "only the print inside the window counts");
        assert_eq!(w.bull_premium, 2000.0);
    }

    #[test]
    fn velocity_compares_the_last_window_with_the_one_before_it() {
        let now = 10_000_000;
        let trades = vec![
            trade(now - 1_000_000, OptionType::Call, AggressorSide::Buy, 500.0),
            trade(now - 60_000, OptionType::Call, AggressorSide::Buy, 2000.0),
        ];
        let v = net_flow_velocity(&trades, now, 900_000);
        assert_eq!(v.current, 2000.0);
        assert_eq!(v.previous, 500.0);
        assert_eq!(v.velocity, 1500.0);
    }

    #[test]
    fn bias_labels_sit_where_the_thresholds_put_them() {
        assert_eq!(bias_label(0.7, THRESHOLDS), Bias::StrongBull);
        assert_eq!(bias_label(0.57, THRESHOLDS), Bias::ModerateBull);
        assert_eq!(bias_label(0.5, THRESHOLDS), Bias::Balanced);
        assert_eq!(bias_label(0.44, THRESHOLDS), Bias::ModerateBear);
        assert_eq!(bias_label(0.3, THRESHOLDS), Bias::StrongBear);
        // Boundaries resolve to the stronger label, as in the prototype.
        assert_eq!(bias_label(0.6, THRESHOLDS), Bias::StrongBull);
        assert_eq!(bias_label(0.4, THRESHOLDS), Bias::StrongBear);
    }
}
