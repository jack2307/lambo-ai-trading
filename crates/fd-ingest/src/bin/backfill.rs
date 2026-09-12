//! Pull real history into the Parquet store.
//!
//! The prototype held a few days because JSON could not hold more. The point of
//! the store is that this is no longer the constraint, and the point of *this*
//! is that a walk-forward over 673 bars is not evidence of anything.
//!
//! ```text
//! cargo run --release -p fd-ingest --bin backfill -- --market=btc --days=180
//! ```
//!
//! Written to be safe to interrupt. Bars are rewritten whole (a bar series is
//! one coherent thing), but the tape is appended a day at a time, so killing
//! this halfway leaves the days it already wrote intact and a later run simply
//! adds the rest — the store de-duplicates on read.

use std::path::PathBuf;
use std::time::Duration;

use fd_ingest::binance::BinanceClient;
use fd_ingest::deribit::{DeribitClient, trades_from_deribit};
use fd_ingest::otl::{OtlClient, trades_from_chart_data};
use fd_store::{TapeStore, write_bars};

const DAY_MS: i64 = 86_400_000;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn has(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
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
    let days: i64 = arg("days", "180").parse()?;
    let interval = arg("interval", "15m");
    let out = PathBuf::from(arg("out", "data"));
    let now = now_ms();

    if !has("tape-only") {
        match market.as_str() {
            "btc" => backfill_binance(&out, "BTCUSDT", &interval, days, now).await?,
            _ => println!("bars: the gold feed serves only a rolling window; nothing to backfill"),
        }
    }
    if !has("bars-only") {
        match market.as_str() {
            "btc" => backfill_deribit(&out, "BTC", days, now).await?,
            _ => backfill_gold(&out, days).await?,
        }
    }
    Ok(())
}

async fn backfill_binance(
    out: &std::path::Path,
    symbol: &str,
    interval: &str,
    days: i64,
    now: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = BinanceClient::default();
    // Each request returns at most 1000 bars, so the budget has to cover the
    // whole span or the series comes back silently short.
    let step = fd_ingest::binance::interval_ms(interval).ok_or("unsupported interval")?;
    let requests = ((days * DAY_MS / step) as usize / 1000) + 4;

    println!("bars: {symbol} {interval}, {days} days, up to {requests} requests");
    let bars = client.backfill(symbol, interval, days, now, requests).await?;
    if bars.is_empty() {
        println!("bars: nothing returned");
        return Ok(());
    }

    let path = out.join("bars").join(format!("{symbol}-{interval}.parquet"));
    write_bars(&path, &bars)?;

    // A count alone hides holes. The span and the gap count say whether the
    // series is actually continuous, which is what a backtest depends on.
    let holes = fd_ingest::gaps(&bars, step);
    println!(
        "bars: {} bars, {} -> {}, {} gap(s), {} KiB",
        bars.len(),
        iso(bars[0].time),
        iso(bars[bars.len() - 1].time),
        holes.len(),
        std::fs::metadata(&path)?.len() / 1024
    );
    for (after, before) in holes.iter().take(5) {
        println!("  gap: {} -> {} ({} bars missing)", iso(*after), iso(*before), (before - after) / step - 1);
    }
    Ok(())
}

async fn backfill_deribit(
    out: &std::path::Path,
    currency: &str,
    days: i64,
    now: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = DeribitClient::default();
    let store = TapeStore::open(out, "btc")?;
    let start = now - days * DAY_MS;

    println!("tape: {currency} options, {days} days");
    let mut total = 0usize;
    let mut empty_days = 0usize;

    // A day at a time: it bounds how much is lost to an interruption, and it
    // keeps each REST window small enough that the venue's paging behaves.
    let mut day = start;
    while day < now {
        let end = (day + DAY_MS).min(now);
        let rows = match client.trades(currency, day, end, 400).await {
            Ok(rows) => rows,
            Err(error) => {
                println!("  {}: {error}", iso(day));
                day = end;
                continue;
            }
        };
        let trades = trades_from_deribit(&rows, "deribit-backfill");
        if trades.is_empty() {
            empty_days += 1;
        } else {
            store.append(&trades)?;
            total += trades.len();
            println!("  {}: {} prints (running total {total})", iso(day), trades.len());
        }
        day = end;
    }

    let stored = store.all()?.len();
    println!("tape: {total} prints fetched, {stored} readable in the store, {empty_days} empty day(s)");
    if empty_days > 0 {
        println!("      empty days are normal at the far end: the public endpoint does not");
        println!("      serve unlimited history, so the reach is whatever it answers for.");
    }
    Ok(())
}

async fn backfill_gold(out: &std::path::Path, days: i64) -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("OTL_BASE_URL").unwrap_or_else(|_| "https://live.otldata.com".to_string());
    let client = OtlClient::new(&base, Duration::from_millis(1500));
    // The feed's clock is not UTC; the offset is measured and lives in config.
    let config = fd_core::config::Config::load(arg("config", "config"))?;
    let utc_offset_ms = config.sources.get("reference").map_or(0, |s| s.utc_offset_ms());
    let store = TapeStore::open(out, "gold")?;

    let contracts = client.active_contracts().await?;
    println!("tape: gold, {} active contract(s) from {base}", contracts.len());

    // The feed serves a rolling window rather than history: asking for more
    // hours than it keeps returns what it has, not an error.
    let hours = (days * 24).min(720) as u32;
    let mut total = 0usize;
    for contract in &contracts {
        let chart = client.chart_data(&contract.symbol, hours, 1).await?;
        let trades =
            trades_from_chart_data(&chart, &contract.symbol, contract.expiration.as_deref(), utc_offset_ms);
        if trades.is_empty() {
            println!("  {}: nothing", contract.symbol);
            continue;
        }
        store.append(&trades)?;
        total += trades.len();
        println!(
            "  {}: {} prints, {} -> {}",
            contract.symbol,
            trades.len(),
            iso(trades[0].timestamp),
            iso(trades[trades.len() - 1].timestamp)
        );
    }
    println!("tape: {total} prints fetched, {} readable in the store", store.all()?.len());
    Ok(())
}

/// `YYYY-MM-DD HH:MM` for a UTC instant.
fn iso(ms: i64) -> String {
    let days = ms.div_euclid(DAY_MS);
    let rest = ms.rem_euclid(DAY_MS);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60)
}
