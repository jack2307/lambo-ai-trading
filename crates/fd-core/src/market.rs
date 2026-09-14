//! Market conventions.
//!
//! The engines are market-agnostic. What actually differs between COMEX gold
//! and Deribit BTC is a short list of conventions, and they all live here:
//! how a premium is denominated, what counts as a large print, how wide a level
//! cluster is in price terms, and what the traded instrument looks like.
//!
//! Keeping this as data rather than branches is what let the JavaScript
//! prototype add BTC without touching a single engine.

use serde::{Deserialize, Serialize};

use crate::premium::{premium_usd, premium_usd_in_underlying};

/// Identifier of a configured market.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MarketId(pub String);

impl MarketId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MarketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a market's bars come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarSource {
    /// The OTL feed: closes only, rolling window.
    Reference,
    /// Binance klines: real OHLCV, deep history.
    Binance,
    /// A MetaTrader 5 terminal, exported by `py/ingest/mt5_export.py`: the
    /// broker's own OHLC with tick volume, as deep as the terminal holds. No
    /// live stream yet — the export is re-run to extend it.
    Mt5,
    /// Dukascopy's free historical bid feed, converted by
    /// `py/ingest/dukascopy_to_parquet.py`. A different venue from the one
    /// traded: useful as out-of-sample *structure*, not as a quote.
    Dukascopy,
}

/// Where a market's option prints come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionsSource {
    Reference,
    Deribit,
    /// No options feed. Levels and options strategies are unavailable; only
    /// the technical strategies run. Used for a CFD whose bars are not the
    /// options' underlying (XAUUSD spot vs COMEX GC carries a basis of tens of
    /// dollars, wider than the strike spacing the levels are built from).
    None,
}

/// Trading conventions for the instrument actually bought and sold.
/// Two decimals: what gold, BTC and the JavaScript oracle use.
const fn default_price_decimals() -> u32 {
    2
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradingSpec {
    pub symbol: String,
    /// Units of underlying per lot. Venue-specific: 100 oz for a standard
    /// XAUUSD contract, 1 oz on Vantage's `.sc` (cent) symbols, 0.01 BTC on
    /// `BTCUSD.sc`. Read it from the broker (`symbol_info.trade_contract_size`),
    /// never assume it.
    pub contract_size: f64,
    pub spread: f64,
    pub lot_step: f64,
    pub min_lot: f64,
    /// USD per lot per rollover night, sign as the broker charges it
    /// (negative = paid). Zero when unknown — which is worse than a number,
    /// because it makes holding overnight look free.
    pub swap_long_per_lot: f64,
    pub swap_short_per_lot: f64,
    /// The calendar currencies whose releases flatten this market (`news:`
    /// filters and the news guard). Empty = every currency; an event marked
    /// `All` matches any list.
    pub news_currencies: Vec<String>,
    /// Decimals a recorded price is rounded to.
    ///
    /// Two suits gold and BTC, and is what the JavaScript oracle used — the
    /// golden parity files depend on it. A EURUSD trade rounded to two
    /// decimals reports 1.16 for 1.15843 and its entry and exit become the
    /// same number, which is how a record came to derive pips from P&L
    /// instead of reading them (2026-09-14). Five for FX, three for silver.
    #[serde(default = "default_price_decimals")]
    pub price_decimals: u32,
}

/// One market's conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Market {
    pub id: MarketId,
    pub label: String,
    pub bar_symbol: String,
    pub bar_source: BarSource,
    pub options_source: OptionsSource,
    /// The tape store this market reads (`data/<tape>`), when it is not its
    /// own id. Two markets can share one options feed — Binance BTC and
    /// Vantage BTC both read the Deribit tape — and a tape is collected once.
    pub tape: Option<String>,
    /// True when the option is quoted in the underlying rather than in USD.
    pub premium_in_underlying: bool,
    /// Contract multiplier for fixed-multiplier markets. Ignored when
    /// `premium_in_underlying` is set.
    pub multiplier: f64,
    pub underlying: String,
    pub trading: TradingSpec,
    /// Absolute floor for a "big" print, in USD. BTC premiums run roughly three
    /// orders of magnitude below gold's, so one shared number reports either
    /// everything or nothing.
    pub big_trade_min_premium_usd: f64,
    /// Minimum width of a level cluster, in price units. `$5` is meaningful on
    /// gold at 4,300 and meaningless on BTC at 77,000.
    pub cluster_floor: f64,
    pub cluster_atr_fraction: f64,
}

impl Market {
    /// The tape store to read, or `None` for a market with no options feed.
    #[must_use]
    pub fn tape_id(&self) -> Option<&str> {
        match self.options_source {
            OptionsSource::None => None,
            _ => Some(self.tape.as_deref().unwrap_or(self.id.as_str())),
        }
    }

    /// Premium in USD for one print of this market.
    ///
    /// `index_price` is the underlying at the time of the print; it is only
    /// consulted for markets quoting in the underlying.
    #[must_use]
    pub fn premium_usd(&self, trade_price: f64, contracts: f64, index_price: f64) -> f64 {
        if self.premium_in_underlying {
            premium_usd_in_underlying(trade_price, contracts, index_price)
        } else {
            premium_usd(trade_price, contracts, self.multiplier)
        }
    }

    /// USD moved by a one-unit adverse price move on one lot.
    #[must_use]
    pub fn usd_per_point_per_lot(&self) -> f64 {
        self.trading.contract_size
    }

    /// Lot size for a risk budget, rounded down to the venue's lot step.
    #[must_use]
    pub fn size_for_risk(&self, risk_usd: f64, stop_distance: f64) -> f64 {
        if stop_distance <= 0.0 || self.trading.lot_step <= 0.0 {
            return self.trading.min_lot;
        }
        let raw = risk_usd / (stop_distance * self.usd_per_point_per_lot());
        let stepped = (raw / self.trading.lot_step).floor() * self.trading.lot_step;
        stepped.max(self.trading.min_lot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gold() -> Market {
        Market {
            id: MarketId("gold".into()),
            label: "COMEX gold".into(),
            bar_symbol: "GC".into(),
            bar_source: BarSource::Reference,
            options_source: OptionsSource::Reference,
            tape: None,
            premium_in_underlying: false,
            multiplier: 100.0,
            underlying: "GC".into(),
            trading: TradingSpec { symbol: "XAUUSD".into(), contract_size: 100.0, spread: 0.3, lot_step: 0.01, min_lot: 0.01, swap_long_per_lot: 0.0, swap_short_per_lot: 0.0, news_currencies: Vec::new(), price_decimals: 2 },
            big_trade_min_premium_usd: 100_000.0,
            cluster_floor: 5.0,
            cluster_atr_fraction: 0.15,
        }
    }

    fn btc() -> Market {
        Market {
            id: MarketId("btc".into()),
            label: "BTC".into(),
            bar_symbol: "BTCUSDT".into(),
            bar_source: BarSource::Binance,
            options_source: OptionsSource::Deribit,
            tape: None,
            premium_in_underlying: true,
            multiplier: 1.0,
            underlying: "BTC".into(),
            trading: TradingSpec { symbol: "BTCUSD".into(), contract_size: 1.0, spread: 5.0, lot_step: 0.001, min_lot: 0.001, swap_long_per_lot: 0.0, swap_short_per_lot: 0.0, news_currencies: Vec::new(), price_decimals: 2 },
            big_trade_min_premium_usd: 25_000.0,
            cluster_floor: 100.0,
            cluster_atr_fraction: 0.15,
        }
    }

    #[test]
    fn each_market_applies_its_own_premium_convention() {
        // Gold ignores the index and uses the multiplier.
        assert!((gold().premium_usd(21.9, 5.0, 4_350.0) - 10_950.0).abs() < 1e-9);
        // BTC ignores the multiplier and uses the index.
        assert!((btc().premium_usd(0.002, 3.0, 77_000.0) - 462.0).abs() < 1e-9);
    }

    #[test]
    fn sizing_respects_the_risk_budget_and_the_lot_step() {
        // $100 of risk over a $10 stop on 100 oz per lot = 0.1 lots.
        let lots = gold().size_for_risk(100.0, 10.0);
        assert!((lots - 0.1).abs() < 1e-9, "got {lots}");
        let risked = lots * 10.0 * gold().usd_per_point_per_lot();
        assert!(risked <= 100.0 + 1e-9);
    }

    #[test]
    fn sizing_never_returns_less_than_the_minimum_lot() {
        assert_eq!(gold().size_for_risk(0.01, 10_000.0), 0.01);
        assert_eq!(gold().size_for_risk(100.0, 0.0), 0.01);
    }

    #[test]
    fn btc_sizes_on_a_finer_step() {
        let lots = btc().size_for_risk(100.0, 500.0);
        assert!((lots - 0.2).abs() < 1e-9, "got {lots}");
    }
}
