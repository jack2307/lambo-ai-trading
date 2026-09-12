//! The gold reference feed.
//!
//! Public, unauthenticated JSON endpoints of an observed dashboard. It exists
//! for two reasons: as a data source while no direct CME feed is wired up, and
//! as the calibration target for the level formulas this project has not been
//! able to verify independently.
//!
//! Every row is tagged with its source so that nothing downstream can mistake
//! it for exchange-native data. That matters more here than for Deribit: the
//! feed publishes its own `max_pain`, break-even and whale levels alongside the
//! tape, and those are **calibration targets, never inputs**. Feeding a site's
//! own answer back into this project's analytics would turn a comparison into a
//! copy.
//!
//! What this feed does not carry:
//!
//! * **No open interest, anywhere.** Max pain computed from it is a
//!   flow-positioning proxy, not the textbook open-interest quantity.
//! * **No venue trade id.** Prints are identified by content — see
//!   [`fd_core::assign_content_ids`].
//! * **Closes only** on the underlying series, so a one-minute bar from it has
//!   `open == high == low == close`.

use std::time::Duration;

use fd_core::classify::{FlowClass, classify_flow, infer_aggressor};
use fd_core::types::{AggressorSide, Bar, OptionTrade, OptionType, TradeFlags};
use serde_json::Value;

use crate::error::IngestError;
use crate::http::Http;

/// Contract listing row.
#[derive(Debug, Clone)]
pub struct ActiveContract {
    pub symbol: String,
    pub expiration: Option<String>,
    pub kind: Option<String>,
    pub trades: i64,
    pub total_premium: f64,
}

#[derive(Debug, Clone)]
pub struct OtlClient {
    http: Http,
    base: String,
}

impl OtlClient {
    #[must_use]
    pub fn new(base_url: &str, min_delay: Duration) -> Self {
        Self { http: Http::new(min_delay), base: base_url.trim_end_matches('/').to_string() }
    }

    async fn get(&self, path: &str) -> Result<Value, IngestError> {
        self.http.get_json(&format!("{}{path}", self.base)).await
    }

    pub async fn active_contracts(&self) -> Result<Vec<ActiveContract>, IngestError> {
        let rows = self.get("/api/options/active-contracts").await?;
        Ok(rows
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|row| {
                        Some(ActiveContract {
                            symbol: row["symbol"].as_str()?.to_string(),
                            expiration: row["expiration"].as_str().map(str::to_string),
                            kind: row["type"].as_str().map(str::to_string),
                            trades: row["trades"].as_i64().unwrap_or_default(),
                            total_premium: row["total_premium"].as_f64().unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Columnar tape plus underlying candles for one contract.
    pub async fn chart_data(&self, symbol: &str, hours: u32, candles: u32) -> Result<Value, IngestError> {
        self.get(&format!("/api/options/chart-data/{symbol}?hours={hours}&candles={candles}")).await
    }
}

/// Milliseconds from the feed's several time spellings.
///
/// A bare number may be seconds or milliseconds; `2026-09-07 05:12:37` is UTC
/// with no zone marker, which is the spelling most likely to be read an hour
/// out by a naive parser.
#[must_use]
pub fn to_ms(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => {
            let v = n.as_f64()?;
            Some(if v > 1e12 { v as i64 } else { (v * 1000.0) as i64 })
        }
        Value::String(s) => parse_utc(s),
        _ => None,
    }
}

/// `YYYY-MM-DD[ T]HH:MM:SS[.fff][Z]`, always read as UTC.
fn parse_utc(text: &str) -> Option<i64> {
    let text = text.trim().trim_end_matches('Z');
    let (date, time) = text.split_once(['T', ' ']).unwrap_or((text, "00:00:00"));
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;

    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    let minute: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    let seconds: f64 = time_parts.next().unwrap_or("0").parse().unwrap_or(0.0);

    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400_000 + hour * 3_600_000 + minute * 60_000 + (seconds * 1000.0) as i64)
}

/// The columnar tape to normalized prints, ascending by timestamp.
///
/// The flow class is recomputed from option type and aggressor rather than read
/// from the feed, so that one classification rule governs both markets. The
/// feed's own `class` column is the instrument class ("C"/"P"), which is what
/// it is used for here.
#[must_use]
pub fn trades_from_chart_data(chart: &Value, symbol: &str, expiration: Option<&str>) -> Vec<OptionTrade> {
    let trades = &chart["trades"];
    let Some(x) = trades["x"].as_array() else { return Vec::new() };
    let expiration_ms =
        expiration.and_then(parse_utc).or_else(|| to_ms(&chart["expiration"])).unwrap_or_default();
    let underlying = chart["underlying"].as_str().unwrap_or_default().to_string();

    let column = |name: &str| trades[name].as_array().cloned().unwrap_or_default();
    let (strike, class, price) = (column("strike"), column("class"), column("price"));
    let (size, side, premium) = (column("size"), column("side"), column("premium"));
    let (underlying_price, n_fills) = (column("underlying_price"), column("n_fills"));

    let at = |col: &Vec<Value>, i: usize| col.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN);

    let mut out = Vec::with_capacity(x.len());
    // Indexed rather than zipped: this is a columnar payload and every other
    // column is addressed by the same `i`.
    for (i, when) in x.iter().enumerate() {
        let timestamp = to_ms(when).unwrap_or_default();
        // The feed's `class` column is the instrument class — "C" or "P" — not
        // the LC/LP/SC/SP flow class that shares the word elsewhere in this
        // codebase. Anything unreadable is treated as a call, matching the
        // prototype rather than dropping the print.
        let option_type = class
            .get(i)
            .and_then(Value::as_str)
            .and_then(OptionType::from_letter)
            .unwrap_or(OptionType::Call);
        let trade_price = at(&price, i);
        let side_text = side.get(i).and_then(Value::as_str).unwrap_or("");
        let aggressor = match AggressorSide::parse(side_text) {
            AggressorSide::Unknown => infer_aggressor(trade_price, None, None, None),
            known => known,
        };
        let contracts = at(&size, i);

        out.push(OptionTrade {
            id: String::new(),
            timestamp,
            symbol: symbol.to_string(),
            instrument: None,
            underlying: underlying.clone(),
            expiration: expiration_ms,
            dte: (expiration_ms - timestamp) as f64 / fd_core::MS_PER_DAY,
            strike: at(&strike, i),
            option_type,
            trade_price,
            contracts,
            bid: None,
            ask: None,
            aggressor_side: aggressor,
            flow_class: match aggressor {
                AggressorSide::Unknown => FlowClass::Unknown,
                known => classify_flow(option_type, known),
            },
            premium_usd: premium.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN),
            underlying_price: at(&underlying_price, i),
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags {
                // `n_fills > 1` means the print was assembled from several
                // fills — a sweep-like footprint. Kept as a flag, never as a
                // conclusion: the feed does not say the fills were one order.
                sweep: n_fills.get(i).and_then(Value::as_f64).is_some_and(|n| n > 1.0),
                ..TradeFlags::default()
            },
            source: "reference-feed".to_string(),
        });
    }
    out.sort_by_key(|t| t.timestamp);
    fd_core::assign_content_ids(&mut out);
    out
}

/// Underlying candles from the same payload.
///
/// The feed publishes closes, so every bar is flat. [`Bar::is_synthetic`] says
/// so rather than letting a zero range read as a quiet minute.
#[must_use]
pub fn bars_from_chart_data(chart: &Value) -> Vec<Bar> {
    let ohlcv = &chart["ohlcv"];
    let Some(x) = ohlcv["x"].as_array() else { return Vec::new() };
    let column = |name: &str| ohlcv[name].as_array().cloned().unwrap_or_default();
    let (open, high, low, close) = (column("open"), column("high"), column("low"), column("close"));
    let volume = column("volume");

    let mut out: Vec<Bar> = (0..x.len())
        .filter_map(|i| {
            let time = to_ms(&x[i])?;
            let value = |col: &Vec<Value>| col.get(i).and_then(Value::as_f64);
            let close_price = value(&close).or_else(|| value(&open))?;
            Some(match (value(&open), value(&high), value(&low)) {
                (Some(o), Some(h), Some(l)) => {
                    Bar { time, open: o, high: h, low: l, close: close_price, volume: value(&volume) }
                }
                _ => Bar::flat(time, close_price),
            })
        })
        .collect();
    out.sort_by_key(|b| b.time);
    out
}

/// The levels the site published alongside the tape, run-length encoded as
/// `[trade_index, value]` pairs.
///
/// Calibration targets. Never inputs — see the module note.
#[must_use]
pub fn reference_level_at(rle: &Value, index: usize) -> Option<f64> {
    let pairs = rle.as_array()?;
    let mut value = None;
    for pair in pairs {
        let at = pair.get(0)?.as_u64()? as usize;
        if at > index {
            break;
        }
        value = pair.get(1)?.as_f64();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feeds_time_spellings_all_land_on_utc() {
        assert_eq!(parse_utc("1970-01-01 00:00:00"), Some(0));
        assert_eq!(parse_utc("2026-09-07 05:12:37"), Some(1_788_757_957_000));
        assert_eq!(parse_utc("2026-09-07T05:12:37Z"), Some(1_788_757_957_000));
        // A bare number may be seconds or milliseconds.
        assert_eq!(to_ms(&serde_json::json!(1_788_757_957_i64)), Some(1_788_757_957_000));
        assert_eq!(to_ms(&serde_json::json!(1_788_757_957_000_i64)), Some(1_788_757_957_000));
    }

    #[test]
    fn a_columnar_tape_becomes_prints_with_stable_ids() {
        let chart = serde_json::json!({
            "underlying": "GC",
            "trades": {
                "x": [1_788_757_957_000_i64, 1_789_038_758_000_i64],
                "strike": [4500.0, 4600.0],
                "class": ["C", "P"],
                "price": [12.5, 8.0],
                "size": [10.0, 5.0],
                "side": ["LONG", "SHORT"],
                "premium": [12500.0, 4000.0],
                "underlying_price": [4480.0, 4482.0],
                "n_fills": [1, 3],
            }
        });
        let trades = trades_from_chart_data(&chart, "OGV6", Some("2026-11-24 00:00:00"));
        assert_eq!(trades.len(), 2);
        // "C"/"P" is what the feed actually sends in this column.
        assert_eq!(trades[0].option_type, OptionType::Call);
        assert_eq!(trades[0].aggressor_side, AggressorSide::Buy);
        assert_eq!(trades[0].flow_class, FlowClass::Lc);
        assert_eq!(trades[1].option_type, OptionType::Put);
        assert_eq!(trades[1].aggressor_side, AggressorSide::Sell);
        assert!(trades[1].flags.sweep, "n_fills > 1 is a sweep-like footprint");
        assert!(!trades[0].id.is_empty() && trades[0].id != trades[1].id);
    }

    #[test]
    fn an_empty_payload_yields_nothing_rather_than_panicking() {
        assert!(trades_from_chart_data(&serde_json::json!({}), "OGV6", None).is_empty());
        assert!(bars_from_chart_data(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn closes_only_candles_are_marked_synthetic() {
        let chart = serde_json::json!({"ohlcv": {"x": [0, 60_000], "close": [4500.0, 4501.0]}});
        let bars = bars_from_chart_data(&chart);
        assert_eq!(bars.len(), 2);
        assert!(bars.iter().all(Bar::is_synthetic), "a close-only bar has no range of its own");
    }

    #[test]
    fn a_run_length_encoded_level_holds_its_value_until_the_next_change() {
        let rle = serde_json::json!([[0, 4500.0], [10, 4520.0]]);
        assert_eq!(reference_level_at(&rle, 0), Some(4500.0));
        assert_eq!(reference_level_at(&rle, 9), Some(4500.0));
        assert_eq!(reference_level_at(&rle, 10), Some(4520.0));
        assert_eq!(reference_level_at(&rle, 999), Some(4520.0));
    }
}
