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
//! cargo run --release -p fd-ingest --bin collect -- --market=gold
//! ```
//!
//! Gold has no websocket: the OTL feed publishes a rolling window of a few
//! days per contract, so the gold collector *polls* — a full pull at start to
//! close the gap since the last run, then a short window every few minutes —
//! and appends only prints whose id it has not seen. Ids are content hashes,
//! so a print the feed re-serves is the same print. GC minute closes from the
//! same payload accumulate into `data/bars/GC-1m.parquet` the way the
//! prototype's bar store did.
//!
//! Intended to run for weeks. Nothing here decides anything or trades anything;
//! it only writes down what happened.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use fd_core::config::Config;
use fd_core::types::{Bar, OptionTrade};
use fd_ingest::deribit::trades_from_deribit;
use fd_ingest::otl::{OtlClient, bars_from_chart_data, trades_from_chart_data};
use fd_ingest::{DeribitFeed, FeedEvent};
use fd_store::{TapeStore, read_bars, write_bars};
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
    if market == "gold" {
        let poll = Duration::from_secs(arg("poll-secs", "600").parse()?);
        return collect_gold(&store, &out, &Config::load(arg("config", "config"))?, poll).await;
    }
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

/// Poll the OTL feed and keep what is new.
async fn collect_gold(
    store: &TapeStore,
    out: &Path,
    config: &Config,
    poll: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = config.sources.get("reference").ok_or("no [sources.reference] in config")?;
    let client = OtlClient::new(&source.base_url, Duration::from_millis(source.min_delay_ms));
    let utc_offset_ms = source.utc_offset_ms();
    let bars_path = out.join("bars").join("GC-1m.parquet");

    let mut seen: HashSet<String> = store.all()?.iter().map(|t| t.id.clone()).collect();
    let started_with = seen.len();
    println!("collecting gold options from {} into {}", source.base_url, store.root().display());
    println!("store already holds {started_with} prints; polling every {}s, feed clock offset {}h", poll.as_secs(), utc_offset_ms / 3_600_000);
    println!("stop with ctrl-c — nothing is buffered, every poll is written as it lands\n");

    // First pull takes the whole window the feed still has; later ones only a
    // few hours, which is many polls' worth of overlap.
    let mut hours = 0u32;
    let mut written = 0usize;
    let mut failures = 0usize;
    let mut ticker = tokio::time::interval(poll);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nstopping");
                break;
            }
            _ = ticker.tick() => {
                match poll_gold(&client, store, &bars_path, utc_offset_ms, hours, &mut seen).await {
                    Ok((prints, bars)) => {
                        written += prints;
                        println!("{}  {prints} new print(s), {bars} minute bar(s) merged; {written} written this run", stamp(now_ms()));
                    }
                    Err(e) => {
                        failures += 1;
                        println!("{}  poll failed ({failures} so far): {e}", stamp(now_ms()));
                    }
                }
                hours = 3;
            }
        }
    }
    println!("store now holds {} prints ({} more than at start)", seen.len(), seen.len().saturating_sub(started_with));
    Ok(())
}

/// One pass over every active contract. Returns (new prints, bars merged).
async fn poll_gold(
    client: &OtlClient,
    store: &TapeStore,
    bars_path: &Path,
    utc_offset_ms: i64,
    hours: u32,
    seen: &mut HashSet<String>,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let contracts = client.active_contracts().await?;
    let mut fresh: Vec<OptionTrade> = Vec::new();
    let mut bars: BTreeMap<i64, Bar> =
        if bars_path.exists() { read_bars(bars_path)?.into_iter().map(|b| (b.time, b)).collect() } else { BTreeMap::new() };
    let bars_before = bars.len();

    for contract in &contracts {
        let chart = client.chart_data(&contract.symbol, hours, 1).await?;
        for trade in trades_from_chart_data(&chart, &contract.symbol, contract.expiration.as_deref(), utc_offset_ms) {
            if seen.insert(trade.id.clone()) {
                fresh.push(trade);
            }
        }
        // Every contract's payload carries the same underlying minute series;
        // merging each is harmless and covers a contract that has more of it.
        for bar in bars_from_chart_data(&chart, utc_offset_ms) {
            bars.insert(bar.time, bar);
        }
    }

    if !fresh.is_empty() {
        store.append(&fresh)?;
    }
    let merged = bars.len() - bars_before;
    if merged > 0 {
        let series: Vec<Bar> = bars.into_values().collect();
        write_bars(bars_path, &series)?;
    }
    Ok((fresh.len(), merged))
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
