//! Binance spot klines.
//!
//! The BTC market's price series. Unlike the gold feed this is real OHLCV, so
//! nothing here is synthetic.

use std::collections::BTreeMap;
use std::time::Duration;

use fd_core::types::Bar;
use serde_json::Value;

use crate::error::IngestError;
use crate::http::Http;

const BASE: &str = "https://api.binance.com";
pub const WS_BASE: &str = "wss://stream.binance.com:9443/ws";

/// A page of klines; the venue's maximum.
const PAGE: usize = 1000;

/// Intervals this project uses, in milliseconds.
#[must_use]
pub fn interval_ms(interval: &str) -> Option<i64> {
    Some(match interval {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        _ => return None,
    })
}

#[derive(Debug, Clone)]
pub struct BinanceClient {
    http: Http,
    base: String,
}

impl Default for BinanceClient {
    fn default() -> Self {
        Self::new(BASE, Duration::from_millis(250))
    }
}

impl BinanceClient {
    #[must_use]
    pub fn new(base: &str, min_delay: Duration) -> Self {
        Self { http: Http::new(min_delay), base: base.to_string() }
    }

    /// One page of klines.
    pub async fn klines(
        &self,
        symbol: &str,
        interval: &str,
        start: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Bar>, IngestError> {
        let mut url =
            format!("{}/api/v3/klines?symbol={symbol}&interval={interval}&limit={}", self.base, limit.min(PAGE));
        if let Some(start) = start {
            url.push_str(&format!("&startTime={start}"));
        }
        let rows: Vec<Value> = self.http.get_json(&url).await?;
        Ok(rows.iter().filter_map(parse_kline).collect())
    }

    /// Page forward until `days` of history is collected.
    ///
    /// A page that does not advance the cursor ends the loop rather than
    /// retrying: without that, a venue returning the same page forever turns
    /// into an infinite request loop against a rate-limited endpoint.
    pub async fn backfill(
        &self,
        symbol: &str,
        interval: &str,
        days: i64,
        now_ms: i64,
        max_requests: usize,
    ) -> Result<Vec<Bar>, IngestError> {
        let step = interval_ms(interval).ok_or_else(|| IngestError::Venue(format!("interval: {interval}")))?;
        let end = now_ms;
        let mut cursor = end - days * 86_400_000;
        let mut all: Vec<Bar> = Vec::new();

        for _ in 0..max_requests {
            if cursor >= end {
                break;
            }
            let page = self.klines(symbol, interval, Some(cursor), PAGE).await?;
            if page.is_empty() {
                break;
            }
            let last = page[page.len() - 1].time;
            let short_page = page.len() < PAGE;
            all.extend(page);
            if last <= cursor {
                break;
            }
            cursor = last + step;
            if short_page {
                break;
            }
        }
        Ok(dedupe(all))
    }

    /// Latest price, used as the spot quote for a live snapshot.
    pub async fn price(&self, symbol: &str) -> Result<f64, IngestError> {
        let row: Value = self.http.get_json(&format!("{}/api/v3/ticker/price?symbol={symbol}", self.base)).await?;
        row["price"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| IngestError::Venue("binance: no price".into()))
    }
}

/// Binance kline tuple to a bar.
///
/// Index order is fixed by the API:
/// `[openTime, open, high, low, close, volume, closeTime, ...]`. The numbers
/// arrive as strings, which is why every field goes through a parse rather than
/// `as_f64`.
#[must_use]
pub fn parse_kline(row: &Value) -> Option<Bar> {
    let cells = row.as_array()?;
    let number = |index: usize| -> Option<f64> {
        match cells.get(index)? {
            Value::String(s) => s.parse().ok(),
            other => other.as_f64(),
        }
    };
    Some(Bar {
        time: cells.first()?.as_i64()?,
        open: number(1)?,
        high: number(2)?,
        low: number(3)?,
        close: number(4)?,
        volume: number(5),
    })
}

/// Last bar wins for a repeated open time, then sorted.
///
/// A backfill's pages overlap at their seams, and the later copy of a bar is
/// the more complete one — the earlier may have been mid-formation.
fn dedupe(bars: Vec<Bar>) -> Vec<Bar> {
    let mut seen: BTreeMap<i64, Bar> = BTreeMap::new();
    for bar in bars {
        seen.insert(bar.time, bar);
    }
    seen.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kline_tuple_becomes_a_bar() {
        let row = serde_json::json!([
            1_788_000_000_000_i64, "80000.10", "80500.00", "79900.00", "80250.50", "12.5",
            1_788_000_059_999_i64, "1000000.0", 42, "6.0", "480000.0", "0"
        ]);
        let bar = parse_kline(&row).expect("a well-formed tuple");
        assert_eq!(bar.time, 1_788_000_000_000);
        assert!((bar.close - 80_250.50).abs() < 1e-9);
        assert_eq!(bar.volume, Some(12.5));
        assert!(!bar.is_synthetic());
    }

    #[test]
    fn a_truncated_tuple_is_refused_rather_than_half_read() {
        assert!(parse_kline(&serde_json::json!([1_788_000_000_000_i64, "80000.10"])).is_none());
        assert!(parse_kline(&serde_json::json!("not a kline")).is_none());
    }

    #[test]
    fn overlapping_pages_keep_the_later_copy_of_a_bar() {
        let early = Bar { time: 60_000, open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: Some(1.0) };
        let late = Bar { time: 60_000, open: 1.0, high: 2.0, low: 1.0, close: 1.8, volume: Some(9.0) };
        let out = dedupe(vec![early, late]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], late, "the later, more complete copy of a bar must win");
    }

    #[test]
    fn intervals_are_the_ones_the_project_uses() {
        assert_eq!(interval_ms("15m"), Some(900_000));
        assert_eq!(interval_ms("3m"), None);
    }
}
