//! Deribit BTC options tape.
//!
//! Public, unauthenticated market data. Two things it gives away that the gold
//! feed does not:
//!
//! * `direction` is the **taker** side — a real exchange aggressor flag rather
//!   than an inference, so LC/LP/SC/SP classification stops being a guess.
//! * `combo_id` marks legs of a multi-leg structure, so multi-leg flagging is
//!   data rather than a timestamp heuristic.
//!
//! Premium convention, and why a fixed multiplier will not do here: an option is
//! quoted in BTC per contract and one contract is 1 BTC, so
//!
//! ```text
//! premium_usd = price(BTC) * index_price(USD/BTC) * amount(contracts)
//! ```

use std::collections::HashMap;
use std::time::Duration;

use fd_core::classify::classify_flow;
use fd_core::types::{AggressorSide, OptionTrade, OptionType, TradeFlags};
use serde_json::Value;

use crate::error::IngestError;
use crate::http::Http;

const BASE: &str = "https://www.deribit.com/api/v2";
pub const WS_URL: &str = "wss://www.deribit.com/ws/api/v2";

/// Deribit settles options at 08:00 UTC on the expiry date.
const EXPIRY_HOUR_UTC: i64 = 8;

/// One page of trades; the venue's maximum.
const PAGE: usize = 1000;

/// What an instrument name decomposes into.
#[derive(Debug, Clone, PartialEq)]
pub struct Instrument {
    pub currency: String,
    /// The expiry (`BTC-18SEP26`) — see the note in [`parse_instrument`].
    pub contract: String,
    pub expiration: i64,
    pub strike: f64,
    pub option_type: OptionType,
}

/// Parse `BTC-18SEP26-83000-C`.
///
/// Note what the contract is and is not. Deribit names an instrument by expiry
/// *and* strike, but every engine here treats a contract as an **expiry**
/// holding many strikes — that is what makes a max pain or a strike profile
/// mean anything. Using the full instrument name would make every strike its
/// own expiration context and reduce a profile to a single point.
#[must_use]
pub fn parse_instrument(name: &str) -> Option<Instrument> {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    let (currency, expiry_raw, strike_raw, type_raw) = (parts[0], parts[1], parts[2], parts[3]);

    // `<day><MON><yy>`, e.g. 18SEP26.
    let split = expiry_raw.find(|c: char| c.is_ascii_alphabetic())?;
    let day: i64 = expiry_raw[..split].parse().ok()?;
    let month = month_from_name(&expiry_raw[split..split + 3])?;
    let year: i64 = 2000 + expiry_raw[split + 3..].parse::<i64>().ok()?;

    let strike: f64 = strike_raw.parse().ok()?;
    if !strike.is_finite() {
        return None;
    }
    Some(Instrument {
        currency: currency.to_string(),
        contract: format!("{currency}-{expiry_raw}"),
        expiration: utc_ms(year, month, day, EXPIRY_HOUR_UTC),
        strike,
        option_type: if type_raw == "P" { OptionType::Put } else { OptionType::Call },
    })
}

fn month_from_name(name: &str) -> Option<i64> {
    const MONTHS: [&str; 12] =
        ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
    MONTHS.iter().position(|m| *m == name).map(|i| i as i64 + 1)
}

/// Epoch milliseconds for a UTC civil date and hour (Howard Hinnant's
/// days-from-civil).
fn utc_ms(year: i64, month: i64, day: i64, hour: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400_000 + hour * 3_600_000
}

/// Public REST client with a minimum delay between calls.
#[derive(Debug, Clone)]
pub struct DeribitClient {
    http: Http,
    base: String,
}

impl Default for DeribitClient {
    fn default() -> Self {
        Self::new(BASE, Duration::from_millis(400))
    }
}

impl DeribitClient {
    #[must_use]
    pub fn new(base: &str, min_delay: Duration) -> Self {
        Self { http: Http::new(min_delay), base: base.to_string() }
    }

    async fn get(&self, path: &str) -> Result<Value, IngestError> {
        let body: Value = self.http.get_json(&format!("{}{path}", self.base)).await?;
        if let Some(error) = body.get("error").filter(|e| !e.is_null()) {
            return Err(IngestError::Venue(format!("deribit: {error}")));
        }
        body.get("result").cloned().ok_or_else(|| IngestError::Venue("deribit: no result".into()))
    }

    /// Option trades in `[from, to)`, paged forward.
    ///
    /// The cursor advances by the newest timestamp seen rather than by an
    /// offset, because the venue reports `has_more` but no page token. A page
    /// that fails to advance the cursor ends the loop: without that check, a
    /// burst of prints sharing one millisecond would page forever.
    pub async fn trades(
        &self,
        currency: &str,
        from: i64,
        to: i64,
        max_pages: usize,
    ) -> Result<Vec<Value>, IngestError> {
        let mut out = Vec::new();
        let mut cursor = from;
        for _ in 0..max_pages {
            if cursor >= to {
                break;
            }
            let path = format!(
                "/public/get_last_trades_by_currency_and_time?currency={currency}&kind=option\
                 &start_timestamp={cursor}&end_timestamp={to}&count={PAGE}&sorting=asc"
            );
            let result = self.get(&path).await?;
            let rows = result.get("trades").and_then(Value::as_array).cloned().unwrap_or_default();
            if rows.is_empty() {
                break;
            }
            let newest = rows.last().and_then(|r| r["timestamp"].as_i64()).unwrap_or(cursor);
            out.extend(rows);
            if newest <= cursor {
                break;
            }
            cursor = newest + 1;
            if !result.get("has_more").and_then(Value::as_bool).unwrap_or(false) {
                break;
            }
        }
        Ok(out)
    }

    /// Index price of the underlying, e.g. `btc_usd`.
    pub async fn index_price(&self, index_name: &str) -> Result<f64, IngestError> {
        let result = self.get(&format!("/public/get_index_price?index_name={index_name}")).await?;
        result["index_price"].as_f64().ok_or_else(|| IngestError::Venue("deribit: no index_price".into()))
    }
}

/// Deribit rows to normalized prints, ascending by timestamp.
///
/// Rows that cannot be understood are skipped rather than defaulted: a print
/// with an unparseable instrument or a missing index price would otherwise
/// enter the tape with a zero premium and quietly drag every ratio downward.
#[must_use]
pub fn trades_from_deribit(rows: &[Value], source: &str) -> Vec<OptionTrade> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(name) = row["instrument_name"].as_str() else { continue };
        let Some(parsed) = parse_instrument(name) else { continue };
        let (Some(index), Some(size), Some(price)) =
            (row["index_price"].as_f64(), row["amount"].as_f64(), row["price"].as_f64())
        else {
            continue;
        };
        if !index.is_finite() || !size.is_finite() || !price.is_finite() {
            continue;
        }
        let timestamp = row["timestamp"].as_i64().unwrap_or_default();
        let side = match row["direction"].as_str().unwrap_or_default().to_ascii_uppercase().as_str() {
            "BUY" => AggressorSide::Buy,
            _ => AggressorSide::Sell,
        };

        out.push(OptionTrade {
            id: row["trade_id"].as_str().map_or_else(|| format!("{name}:{timestamp}"), str::to_string),
            timestamp,
            symbol: parsed.contract,
            instrument: Some(name.to_string()),
            underlying: format!("{}-PERP", parsed.currency),
            expiration: parsed.expiration,
            dte: (parsed.expiration - timestamp) as f64 / fd_core::MS_PER_DAY,
            strike: parsed.strike,
            option_type: parsed.option_type,
            trade_price: price,
            contracts: size,
            bid: None,
            ask: None,
            aggressor_side: side,
            flow_class: classify_flow(parsed.option_type, side),
            premium_usd: price * index * size,
            underlying_price: index,
            exchange: Some("deribit".to_string()),
            sequence_id: row["trade_seq"].as_i64().map(|s| s.to_string()),
            // Deribit quotes IV in percent.
            implied_volatility: row["iv"].as_f64().filter(|v| v.is_finite()).map(|v| v / 100.0),
            flags: TradeFlags {
                // A combo id means this print is one leg of a structure — the
                // very thing that makes a single leg's direction misleading.
                multi_leg: !row["combo_id"].is_null() || !row["combo_trade_id"].is_null(),
                block: row["liquidation"].as_str() == Some("M"),
                ..TradeFlags::default()
            },
            source: source.to_string(),
        });
    }
    out.sort_by_key(|t| t.timestamp);
    out
}

/// Keep only the contracts carrying real premium.
///
/// Deribit lists far more strikes and expiries than a gold board, and most
/// carry a handful of prints. Keeping every one turns the level engine into
/// noise, so contracts are ranked by traded premium and the quiet tail dropped.
#[must_use]
pub fn top_contracts(trades: &[OptionTrade], keep: usize) -> (Vec<OptionTrade>, Vec<(String, f64)>) {
    let mut premium: HashMap<&str, f64> = HashMap::new();
    for trade in trades {
        *premium.entry(trade.symbol.as_str()).or_default() += trade.premium_usd;
    }
    let mut ranked: Vec<(String, f64)> =
        premium.into_iter().map(|(symbol, total)| (symbol.to_string(), total)).collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    ranked.truncate(keep);

    let allowed: std::collections::HashSet<&str> = ranked.iter().map(|(s, _)| s.as_str()).collect();
    let kept = trades.iter().filter(|t| allowed.contains(t.symbol.as_str())).cloned().collect();
    (kept, ranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_names_decompose_to_an_expiry_and_a_strike() {
        let parsed = parse_instrument("BTC-18SEP26-83000-C").expect("valid name");
        assert_eq!(parsed.contract, "BTC-18SEP26");
        assert_eq!(parsed.strike, 83_000.0);
        assert_eq!(parsed.option_type, OptionType::Call);
        // 2026-09-18T08:00:00Z, Deribit's settlement hour.
        assert_eq!(parsed.expiration, 1_789_718_400_000);
    }

    #[test]
    fn the_epoch_and_a_leap_day_anchor_the_calendar_arithmetic() {
        // Hand-rolled date maths deserves fixed points that can be checked
        // against any calendar, including the leap day a naive rule gets wrong.
        assert_eq!(utc_ms(1970, 1, 1, 0), 0);
        assert_eq!(utc_ms(2000, 2, 29, 0), 951_782_400_000);
        assert_eq!(utc_ms(1969, 12, 31, 0), -86_400_000);
    }

    #[test]
    fn a_put_and_a_single_digit_day_both_parse() {
        let parsed = parse_instrument("BTC-3JAN25-90000-P").expect("valid name");
        assert_eq!(parsed.contract, "BTC-3JAN25");
        assert_eq!(parsed.option_type, OptionType::Put);
    }

    #[test]
    fn names_that_are_not_options_are_refused() {
        assert!(parse_instrument("BTC-PERPETUAL").is_none());
        assert!(parse_instrument("BTC-18XXX26-83000-C").is_none());
        assert!(parse_instrument("").is_none());
    }

    #[test]
    fn a_row_without_an_index_price_is_skipped_rather_than_priced_at_zero() {
        let rows = vec![serde_json::json!({
            "instrument_name": "BTC-18SEP26-83000-C",
            "amount": 1.0,
            "price": 0.01,
            "direction": "buy",
            "timestamp": 1_788_000_000_000_i64,
        })];
        assert!(trades_from_deribit(&rows, "test").is_empty());
    }

    #[test]
    fn premium_is_price_times_index_times_size() {
        let rows = vec![serde_json::json!({
            "instrument_name": "BTC-18SEP26-83000-C",
            "trade_id": "abc",
            "amount": 2.0,
            "price": 0.01,
            "index_price": 80_000.0,
            "direction": "buy",
            "timestamp": 1_788_000_000_000_i64,
        })];
        let trades = trades_from_deribit(&rows, "test");
        assert_eq!(trades.len(), 1);
        assert!((trades[0].premium_usd - 1600.0).abs() < 1e-9);
        assert_eq!(trades[0].aggressor_side, AggressorSide::Buy);
    }
}
