//! The live stream.
//!
//! Server-Sent Events rather than a websocket: the flow is one-way, the browser
//! reconnects on its own, and it costs no dependency on either side.
//!
//! One upstream connection per market, shared by every browser tab through a
//! broadcast channel. Opening a socket to the venue per viewer would get the IP
//! rate-limited by the third tab, and would also mean two tabs could disagree
//! about what the tape said.
//!
//! Three event types, matching what the client already reads:
//!
//! * `bar` — a forming minute from the exchange kline stream.
//! * `prints` — how many option prints just arrived. A heartbeat with meaning:
//!   it is what tells a viewer the tape is alive during a quiet hour.
//! * `levels` — a rebuilt options frame, throttled.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use fd_core::types::OptionTrade;
use fd_ingest::{BinanceKlineFeed, DeribitFeed, FeedEvent, bar_from_kline_event};
use futures_util::Stream;
use serde_json::json;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::routes::MarketQuery;
use crate::state::AppState;

/// How often a rebuilt options frame is published.
///
/// The engine is cheap but not free, and a frame that changes faster than a
/// viewer can read it is not information.
const LEVELS_EVERY: Duration = Duration::from_secs(2);

/// Dropped messages are better than an unbounded queue: a slow tab must not be
/// able to make the process hold the whole tape in memory.
const CHANNEL: usize = 256;

#[derive(Default)]
pub struct LiveHub {
    channels: Mutex<HashMap<String, broadcast::Sender<String>>>,
}

impl LiveHub {
    /// Subscribe to a market, starting its upstream feeds on first use.
    pub fn subscribe(&self, state: &Arc<AppState>, market: &str) -> broadcast::Receiver<String> {
        let mut channels = self.channels.lock().expect("live hub");
        if let Some(sender) = channels.get(market) {
            return sender.subscribe();
        }
        let (sender, receiver) = broadcast::channel(CHANNEL);
        channels.insert(market.to_string(), sender.clone());
        spawn_feeds(Arc::clone(state), market.to_string(), sender);
        receiver
    }
}

fn spawn_feeds(state: Arc<AppState>, market: String, sender: broadcast::Sender<String>) {
    let Ok(spec) = state.config.market(&market) else { return };

    if spec.bar_source == fd_core::market::BarSource::Binance {
        let feed = BinanceKlineFeed::new(&spec.bar_symbol, "1m");
        let bars = sender.clone();
        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(256);
            tokio::spawn(feed.run(tx));
            while let Some(event) = rx.recv().await {
                if let FeedEvent::Rows(rows) = event {
                    for row in rows {
                        if let Some((bar, _closed)) = bar_from_kline_event(&row) {
                            let payload = json!({
                                "type": "bar",
                                "market": market,
                                "time": bar.time,
                                "open": bar.open,
                                "high": bar.high,
                                "low": bar.low,
                                "close": bar.close,
                                "volume": bar.volume,
                            });
                            // A send failure means nobody is listening any
                            // more, which is not an error worth logging.
                            let _ = bars.send(payload.to_string());
                        }
                    }
                }
            }
        });
    }

    if spec.options_source == fd_core::market::OptionsSource::Deribit {
        let currency = spec.underlying.split('-').next().unwrap_or("BTC").to_string();
        spawn_options(state, currency, sender);
    }
}

fn spawn_options(state: Arc<AppState>, currency: String, sender: broadcast::Sender<String>) {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        tokio::spawn(DeribitFeed::new(&currency).run(tx));

        // Prints accumulate here between frame rebuilds. The engine is fed the
        // whole run's tape rather than the last batch, because a level built
        // from two minutes of prints is not the same level.
        let mut tape: Vec<OptionTrade> = Vec::new();
        let mut last_frame = tokio::time::Instant::now() - LEVELS_EVERY;

        while let Some(event) = rx.recv().await {
            let FeedEvent::Rows(rows) = event else { continue };
            let trades = fd_ingest::deribit::trades_from_deribit(&rows, "deribit-live");
            if trades.is_empty() {
                continue;
            }
            let _ = sender.send(json!({ "type": "prints", "count": trades.len() }).to_string());
            tape.extend(trades);

            if last_frame.elapsed() < LEVELS_EVERY {
                continue;
            }
            last_frame = tokio::time::Instant::now();
            if let Some(frame) = rebuild(&state, &tape) {
                let _ = sender.send(json!({ "type": "levels", "frame": frame }).to_string());
            }
        }
    });
}

/// Rebuild the newest frame from the prints seen so far.
fn rebuild(state: &Arc<AppState>, tape: &[OptionTrade]) -> Option<serde_json::Value> {
    let settings = fd_engine::engine::EngineSettings::from_config(&state.config, "btc").ok()?;
    let mut engine = fd_engine::engine::OptionsEngine::new(settings);
    engine.ingest(tape);
    let snapshot = engine.snapshot(&HashMap::new(), None);
    if snapshot.contexts.is_empty() {
        return None;
    }
    let frame = fd_backtest::frame_from_snapshot(&snapshot, state.config.ai.big_trade_window_ms);
    serde_json::to_value(crate::routes::frame_dto(&frame)).ok()
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MarketQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let market = query.market.clone().unwrap_or_else(|| state.config.market.clone());
    let receiver = state.live.subscribe(&state, &market);

    let stream = BroadcastStream::new(receiver).filter_map(|message| match message {
        Ok(text) => Some(Ok(Event::default().data(text))),
        // A lagging subscriber has missed messages. Dropping them is the right
        // call for a live view — the next frame supersedes whatever was lost —
        // and closing the stream over it would be worse.
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)).text("ping"))
}

pub async fn status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let markets: Vec<_> = state
        .config
        .markets
        .keys()
        .map(|id| {
            let live = state.is_live(id);
            json!({ "market": id, "bars": live, "options": state.timeline(id).is_some() })
        })
        .collect();
    Json(json!({ "markets": markets }))
}
