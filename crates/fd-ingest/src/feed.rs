//! Live websocket feeds.
//!
//! Each feed owns one socket, reconnects with exponential backoff, and reports
//! what it sees on a channel. Reconnection is not optional decoration: a feed
//! that dies quietly at 3am and leaves a hole in the tape is worse than one
//! that never started, because the hole is invisible.
//!
//! What a feed does **not** do is decide what to keep. It emits what arrived and
//! what happened to the connection; filling gaps after a reconnect is
//! [`crate::gap`]'s job, and de-duplicating is the store's.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Longest wait between reconnection attempts.
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// What a feed reports.
#[derive(Debug, Clone)]
pub enum FeedEvent {
    /// The socket is up and the subscription acknowledged.
    Connected {
        /// Epoch milliseconds.
        at: i64,
    },
    /// The socket went away. A reconnect is already scheduled unless the feed
    /// was stopped.
    Disconnected {
        at: i64,
        reason: String,
    },
    /// Raw venue rows, exactly as they arrived. Normalizing them is the
    /// caller's job so that a feed stays a transport.
    Rows(Vec<Value>),
    /// The venue said no. Carried rather than logged because a subscription
    /// rejection is a permanent condition a caller must see.
    VenueError(String),
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(1000u64.saturating_mul(1u64 << attempt.min(6))).min(MAX_BACKOFF)
}

/// Deribit option prints.
///
/// The `raw` channel needs an authenticated session
/// (`raw_subscriptions_not_available_for_unauthorized`), so this subscribes to
/// the public `100ms` aggregation. For a system whose decisions are measured in
/// seconds that is not a meaningful loss, and it keeps the pipeline key-free
/// and read-only.
///
/// Deribit closes an idle socket, so a heartbeat is negotiated and its
/// `test_request` answered. Without that the feed dies quietly after a few
/// minutes of a slow tape — which looks exactly like a quiet market.
pub struct DeribitFeed {
    url: String,
    channel: String,
}

impl DeribitFeed {
    #[must_use]
    pub fn new(currency: &str) -> Self {
        Self { url: crate::deribit::WS_URL.to_string(), channel: format!("trades.option.{currency}.100ms") }
    }

    #[must_use]
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Run until the receiver is dropped, reporting everything seen.
    pub async fn run(self, tx: mpsc::Sender<FeedEvent>) {
        let mut attempt = 0u32;
        loop {
            match self.session(&tx).await {
                Ok(()) => attempt = 0,
                Err(reason) => {
                    if tx.send(FeedEvent::Disconnected { at: now_ms(), reason }).await.is_err() {
                        return;
                    }
                }
            }
            if tx.is_closed() {
                return;
            }
            tokio::time::sleep(backoff(attempt)).await;
            attempt = attempt.saturating_add(1);
        }
    }

    async fn session(&self, tx: &mpsc::Sender<FeedEvent>) -> Result<(), String> {
        let (mut socket, _) =
            tokio_tungstenite::connect_async(&self.url).await.map_err(|e| format!("connect: {e}"))?;

        let mut next_id = 1u64;
        let mut send = |method: &str, params: Value| {
            let id = next_id;
            next_id += 1;
            Message::Text(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string())
        };

        socket
            .send(send("public/subscribe", json!({"channels": [self.channel]})))
            .await
            .map_err(|e| format!("subscribe: {e}"))?;
        socket
            .send(send("public/set_heartbeat", json!({"interval": 30})))
            .await
            .map_err(|e| format!("heartbeat: {e}"))?;

        if tx.send(FeedEvent::Connected { at: now_ms() }).await.is_err() {
            return Ok(());
        }

        while let Some(frame) = socket.next().await {
            let frame = frame.map_err(|e| format!("stream: {e}"))?;
            let text = match frame {
                Message::Text(text) => text,
                Message::Ping(payload) => {
                    let _ = socket.send(Message::Pong(payload)).await;
                    continue;
                }
                Message::Close(_) => return Err("closed by venue".to_string()),
                _ => continue,
            };
            let Ok(message) = serde_json::from_str::<Value>(&text) else { continue };

            // The server asks us to prove we are alive; failing to answer is
            // how the connection gets dropped.
            if message["method"] == "heartbeat" {
                if message["params"]["type"] == "test_request" {
                    let reply = {
                        let id = next_id;
                        next_id += 1;
                        Message::Text(
                            json!({"jsonrpc": "2.0", "id": id, "method": "public/test", "params": {}}).to_string(),
                        )
                    };
                    socket.send(reply).await.map_err(|e| format!("test: {e}"))?;
                }
                continue;
            }

            if let Some(error) = message.get("error").filter(|e| !e.is_null()) {
                if tx.send(FeedEvent::VenueError(error.to_string())).await.is_err() {
                    return Ok(());
                }
                continue;
            }

            if message["method"] == "subscription"
                && let Some(rows) = message["params"]["data"].as_array()
                && !rows.is_empty()
                && tx.send(FeedEvent::Rows(rows.clone())).await.is_err()
            {
                return Ok(());
            }
        }
        Err("stream ended".to_string())
    }
}

/// Binance kline stream.
///
/// The stream pushes the *forming* bar many times a second and then the same
/// bar once more with `x: true` when it closes. Both are forwarded: a chart
/// wants the forming one, a backtest wants only the closed one, and deciding
/// here would take that choice away from both.
pub struct BinanceKlineFeed {
    url: String,
}

impl BinanceKlineFeed {
    #[must_use]
    pub fn new(symbol: &str, interval: &str) -> Self {
        Self { url: format!("{}/{}@kline_{interval}", crate::binance::WS_BASE, symbol.to_lowercase()) }
    }

    pub async fn run(self, tx: mpsc::Sender<FeedEvent>) {
        let mut attempt = 0u32;
        loop {
            match self.session(&tx).await {
                Ok(()) => attempt = 0,
                Err(reason) => {
                    if tx.send(FeedEvent::Disconnected { at: now_ms(), reason }).await.is_err() {
                        return;
                    }
                }
            }
            if tx.is_closed() {
                return;
            }
            tokio::time::sleep(backoff(attempt)).await;
            attempt = attempt.saturating_add(1);
        }
    }

    async fn session(&self, tx: &mpsc::Sender<FeedEvent>) -> Result<(), String> {
        let (mut socket, _) =
            tokio_tungstenite::connect_async(&self.url).await.map_err(|e| format!("connect: {e}"))?;
        if tx.send(FeedEvent::Connected { at: now_ms() }).await.is_err() {
            return Ok(());
        }

        while let Some(frame) = socket.next().await {
            let frame = frame.map_err(|e| format!("stream: {e}"))?;
            let text = match frame {
                Message::Text(text) => text,
                Message::Ping(payload) => {
                    let _ = socket.send(Message::Pong(payload)).await;
                    continue;
                }
                Message::Close(_) => return Err("closed by venue".to_string()),
                _ => continue,
            };
            let Ok(message) = serde_json::from_str::<Value>(&text) else { continue };
            if message.get("k").is_some() && tx.send(FeedEvent::Rows(vec![message])).await.is_err() {
                return Ok(());
            }
        }
        Err("stream ended".to_string())
    }
}

/// A `kline` payload to a bar, plus whether the venue considers it closed.
#[must_use]
pub fn bar_from_kline_event(message: &Value) -> Option<(fd_core::types::Bar, bool)> {
    let k = message.get("k")?;
    let number = |name: &str| -> Option<f64> {
        match k.get(name)? {
            Value::String(s) => s.parse().ok(),
            other => other.as_f64(),
        }
    };
    Some((
        fd_core::types::Bar {
            time: k["t"].as_i64()?,
            open: number("o")?,
            high: number("h")?,
            low: number("l")?,
            close: number("c")?,
            volume: number("v"),
        },
        k["x"].as_bool().unwrap_or(false),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_stops_growing() {
        assert_eq!(backoff(0), Duration::from_secs(1));
        assert_eq!(backoff(3), Duration::from_secs(8));
        assert_eq!(backoff(20), MAX_BACKOFF, "an outage must not push the retry past the cap");
    }

    #[test]
    fn a_kline_event_carries_its_closed_flag() {
        let message = serde_json::json!({
            "e": "kline", "s": "BTCUSDT",
            "k": {"t": 1_788_000_000_000_i64, "o": "1.0", "h": "2.0", "l": "0.5", "c": "1.5", "v": "10", "x": false}
        });
        let (bar, closed) = bar_from_kline_event(&message).expect("a kline event");
        assert_eq!(bar.time, 1_788_000_000_000);
        assert!(!closed, "a forming bar must not claim to be closed");
    }

    #[test]
    fn a_message_that_is_not_a_kline_yields_nothing() {
        assert!(bar_from_kline_event(&serde_json::json!({"result": null, "id": 1})).is_none());
    }
}
