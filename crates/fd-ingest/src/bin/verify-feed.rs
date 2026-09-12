//! Phase 3 gate: does the websocket actually deliver the tape?
//!
//! Runs the Deribit option feed for a while, then asks REST what happened over
//! the same window and compares print for print. Three things can go wrong and
//! all three are reported separately, because they have different causes:
//!
//! * **missing** — REST has a print the socket never delivered. A silent hole,
//!   the failure that makes a live tape untrustworthy.
//! * **duplicated** — the socket delivered one print twice. Harmless once the
//!   store de-duplicates, and fatal if anything counts prints before it does.
//! * **extra** — the socket delivered a print REST does not list. Usually a
//!   window-edge artefact, which is why the comparison trims its own edges.
//!
//! This cannot be a unit test: it needs a live venue and an hour. Run it.
//!
//! ```text
//! cargo run -p fd-ingest --bin verify-feed -- --minutes=60
//! ```

use std::collections::HashMap;
use std::time::Duration;

use fd_ingest::deribit::{DeribitClient, trades_from_deribit};
use fd_ingest::{DeribitFeed, FeedEvent};
use tokio::sync::mpsc;

/// Ignored at each end of the window.
///
/// A print landing either side of the boundary may be on one source and not
/// the other purely because of clock and aggregation lag. Comparing the edges
/// measures that lag, not the feed.
const EDGE_MS: i64 = 30_000;

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
    let minutes: u64 = arg("minutes", "60").parse()?;
    let currency = arg("currency", "BTC");

    let (tx, mut rx) = mpsc::channel(1024);
    let feed = DeribitFeed::new(&currency);
    println!("listening on {} for {minutes} minutes", feed.channel());
    tokio::spawn(feed.run(tx));

    // Times by trade id, and how many times each arrived.
    let mut seen: HashMap<String, (i64, usize)> = HashMap::new();
    let mut disconnects = 0usize;
    let started = now_ms();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(minutes * 60);
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    ticker.tick().await;

    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => break,
            _ = ticker.tick() => {
                let elapsed = (now_ms() - started) / 60_000;
                println!("  {elapsed:>3}m  {} prints, {disconnects} disconnects", seen.len());
            }
            event = rx.recv() => {
                match event {
                    Some(FeedEvent::Rows(rows)) => {
                        for trade in trades_from_deribit(&rows, "deribit-ws") {
                            let entry = seen.entry(trade.id).or_insert((trade.timestamp, 0));
                            entry.1 += 1;
                        }
                    }
                    Some(FeedEvent::Disconnected { reason, .. }) => {
                        disconnects += 1;
                        println!("  disconnected: {reason}");
                    }
                    Some(FeedEvent::VenueError(message)) => println!("  venue error: {message}"),
                    Some(FeedEvent::Connected { .. }) => println!("  connected"),
                    None => break,
                }
            }
        }
    }
    let ended = now_ms();
    drop(rx);

    // Let the venue's own indexing settle before asking it what happened.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let (from, to) = (started + EDGE_MS, ended - EDGE_MS);
    if to <= from {
        println!("window too short to compare; run for longer");
        return Ok(());
    }

    let client = DeribitClient::default();
    let rest = trades_from_deribit(&client.trades(&currency, from, to, 200).await?, "deribit-rest");
    println!("\nwindow {from}..{to} ({} minutes after trimming edges)", (to - from) / 60_000);
    println!("  websocket: {} prints in the window", seen.values().filter(|(t, _)| *t >= from && *t < to).count());
    println!("  rest:      {} prints", rest.len());

    let duplicated: Vec<&String> = seen
        .iter()
        .filter(|(_, (t, count))| *t >= from && *t < to && *count > 1)
        .map(|(id, _)| id)
        .collect();
    let missing: Vec<&str> =
        rest.iter().filter(|t| !seen.contains_key(&t.id)).map(|t| t.id.as_str()).collect();
    let rest_ids: std::collections::HashSet<&str> = rest.iter().map(|t| t.id.as_str()).collect();
    let extra: Vec<&String> = seen
        .iter()
        .filter(|(id, (t, _))| *t >= from && *t < to && !rest_ids.contains(id.as_str()))
        .map(|(id, _)| id)
        .collect();

    println!("\n  missing:    {} (REST had it, the socket did not)", missing.len());
    println!("  duplicated: {} (the socket delivered it twice)", duplicated.len());
    println!("  extra:      {} (the socket had it, REST did not)", extra.len());
    println!("  disconnects: {disconnects}");

    for id in missing.iter().take(5) {
        println!("    missing example: {id}");
    }

    if missing.is_empty() && duplicated.is_empty() {
        println!("\nGATE PASS: the socket delivered every print REST knows about, none twice.");
        Ok(())
    } else {
        println!("\nGATE FAIL: the live tape disagrees with REST.");
        std::process::exit(1);
    }
}
