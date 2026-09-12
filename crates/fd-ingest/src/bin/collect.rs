//! Accumulate the options tape, because nobody will sell it to us later.
//!
//! Deribit's public trade endpoint reaches back about a day. That is not a
//! paging bug and no amount of retrying widens it: history that is not captured
//! as it happens is gone. So the options-derived strategies cannot be researched
//! over anything longer than a day *unless a process like this has been running*.
//!
//! It writes the websocket feed straight into the Parquet store, flushing on a
//! timer so that a kill leaves everything up to the last flush. Prints are keyed
//! by the venue's own trade id, so a restart that overlaps what is already
//! stored costs nothing — the store de-duplicates on read.
//!
//! ```text
//! cargo run --release -p fd-ingest --bin collect -- --market=btc
//! ```
//!
//! Intended to run for weeks. Nothing here decides anything or trades anything;
//! it only writes down what happened.

use std::path::PathBuf;
use std::time::Duration;

use fd_core::types::OptionTrade;
use fd_ingest::deribit::trades_from_deribit;
use fd_ingest::{DeribitFeed, FeedEvent};
use fd_store::TapeStore;
use tokio::sync::mpsc;

/// How often buffered prints are written.
///
/// A compromise with two costs on either side: flushing per print would make a
/// Parquet part per print, and flushing hourly would lose an hour to a crash.
const FLUSH: Duration = Duration::from_secs(300);

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "btc");
    let currency = arg("currency", "BTC");
    let out = PathBuf::from(arg("out", "data"));

    let store = TapeStore::open(&out, &market)?;
    let started_with = store.all()?.len();
    println!("collecting {currency} options into {}", store.root().display());
    println!("store already holds {started_with} prints; flushing every {}s", FLUSH.as_secs());
    println!("stop with ctrl-c — the next flush is the only thing at risk\n");

    let (tx, mut rx) = mpsc::channel(4096);
    let feed = DeribitFeed::new(&currency);
    tokio::spawn(feed.run(tx));

    let mut buffer: Vec<OptionTrade> = Vec::new();
    let mut written = 0usize;
    let mut disconnects = 0usize;
    let mut ticker = tokio::time::interval(FLUSH);
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nstopping; flushing {} buffered print(s)", buffer.len());
                flush(&store, &mut buffer, &mut written)?;
                break;
            }
            _ = ticker.tick() => {
                flush(&store, &mut buffer, &mut written)?;
                println!("{}  {written} written, {disconnects} disconnect(s)", stamp(now_ms()));
            }
            event = rx.recv() => match event {
                Some(FeedEvent::Rows(rows)) => buffer.extend(trades_from_deribit(&rows, "deribit-live")),
                Some(FeedEvent::Disconnected { reason, .. }) => {
                    disconnects += 1;
                    // Flush before the gap rather than after: whatever arrives
                    // next belongs to a different stretch of tape.
                    flush(&store, &mut buffer, &mut written)?;
                    println!("{}  disconnected: {reason}", stamp(now_ms()));
                }
                Some(FeedEvent::VenueError(message)) => println!("{}  venue error: {message}", stamp(now_ms())),
                Some(FeedEvent::Connected { .. }) => println!("{}  connected", stamp(now_ms())),
                None => break,
            },
        }
    }

    let held = store.all()?.len();
    println!("store now holds {held} prints ({} more than at start)", held.saturating_sub(started_with));
    Ok(())
}

fn flush(
    store: &TapeStore,
    buffer: &mut Vec<OptionTrade>,
    written: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if buffer.is_empty() {
        return Ok(());
    }
    store.append(buffer)?;
    *written += buffer.len();
    buffer.clear();
    Ok(())
}

fn stamp(ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    let rest = ms.rem_euclid(DAY_MS);
    format!("{:02}:{:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60, (rest / 1000) % 60)
}
