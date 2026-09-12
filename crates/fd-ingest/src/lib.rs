//! Getting market data in: REST backfill and live websocket feeds.
//!
//! The division of labour here is deliberate and is what makes a live tape
//! trustworthy:
//!
//! * A **feed** is a transport. It reconnects, answers heartbeats, and reports
//!   what arrived — including that it went away. It never decides what to keep.
//! * A **client** backfills over REST, which is the only way to recover what a
//!   socket missed while it was down.
//! * [`gap`] finds the holes, by looking inside the series rather than at its
//!   end, because a reconnect leaves the tail looking perfectly healthy.
//! * De-duplicating belongs to the store, not here. A backfill deliberately
//!   overlaps what is already on disk — asking the feed to avoid overlap would
//!   trade a harmless duplicate for a permanent hole.

pub mod binance;
pub mod deribit;
pub mod error;
pub mod feed;
pub mod gap;
pub mod http;
pub mod otl;

pub use binance::BinanceClient;
pub use deribit::DeribitClient;
pub use error::IngestError;
pub use feed::{BinanceKlineFeed, DeribitFeed, FeedEvent, bar_from_kline_event};
pub use gap::{gaps, resync_start};
pub use otl::OtlClient;
