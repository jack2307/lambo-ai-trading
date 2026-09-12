//! Move the prototype's JSON data into the Parquet store.
//!
//! Two sources, because the prototype kept two kinds of thing:
//!
//! * **Tapes** come from the oracle's columnar export (`tests/golden/*-tape.json`),
//!   which is the complete normalized tape rather than a sample of it — the
//!   exporter publishes its inputs precisely so a port does not have to rebuild
//!   an ingest layer before it can be checked.
//! * **Bar series** come from the prototype's own `data/bars/*.json`.
//!
//! Prints are given [`OptionTrade::content_id`] identities. The exported tape
//! carries no venue trade id, and numbering by position would mean a second
//! export produced a whole new set of ids — which the store, de-duplicating by
//! id, would happily store a second time.
//!
//! ```text
//! cargo run -p fd-store --bin migrate-json -- \
//!     --golden=tests/golden --bars=E:/nodejs/gold-options-flow/data/bars --out=data
//! ```

use std::path::{Path, PathBuf};

use fd_core::classify::classify_flow;
use fd_core::types::{AggressorSide, Bar, OptionTrade, OptionType, TradeFlags};
use fd_store::{TapeStore, write_bars};
use serde_json::Value;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn num(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) if s == "NaN" => f64::NAN,
        _ => f64::NAN,
    }
}

/// Rebuild the normalized tape from the oracle's columnar export.
fn tape_from_columns(value: &Value) -> Vec<OptionTrade> {
    let count = value["count"].as_u64().unwrap_or_default() as usize;
    let column = |name: &str| value[name].as_array().cloned().unwrap_or_default();

    let (timestamp, symbol, instrument, underlying) =
        (column("timestamp"), column("symbol"), column("instrument"), column("underlying"));
    let (expiration, strike, option_type) = (column("expiration"), column("strike"), column("optionType"));
    let (trade_price, contracts, aggressor) =
        (column("tradePrice"), column("contracts"), column("aggressorSide"));
    let (premium, underlying_price, multi_leg) =
        (column("premiumUsd"), column("underlyingPrice"), column("multiLeg"));

    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let option_type =
            if option_type[i].as_str() == Some("PUT") { OptionType::Put } else { OptionType::Call };
        let side = AggressorSide::parse(aggressor[i].as_str().unwrap_or("UNKNOWN"));
        let expiration_ms = num(&expiration[i]) as i64;
        let timestamp_ms = num(&timestamp[i]) as i64;

        out.push(OptionTrade {
            id: String::new(),
            timestamp: timestamp_ms,
            symbol: symbol[i].as_str().unwrap_or_default().to_string(),
            instrument: instrument[i].as_str().map(str::to_string),
            underlying: underlying[i].as_str().unwrap_or_default().to_string(),
            expiration: expiration_ms,
            dte: (expiration_ms - timestamp_ms) as f64 / fd_core::MS_PER_DAY,
            strike: num(&strike[i]),
            option_type,
            trade_price: num(&trade_price[i]),
            contracts: num(&contracts[i]),
            bid: None,
            ask: None,
            aggressor_side: side,
            flow_class: classify_flow(option_type, side),
            premium_usd: num(&premium[i]),
            underlying_price: num(&underlying_price[i]),
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags { multi_leg: multi_leg[i].as_bool().unwrap_or(false), ..TradeFlags::default() },
            source: "migrated".to_string(),
        });
    }
    // Ids last, over the whole tape: identical prints need to be told apart,
    // which cannot be decided one print at a time.
    fd_core::assign_content_ids(&mut out);
    out
}

/// The prototype's bar file: `{ symbol, updatedAt, series: [...] }`.
///
/// The gold series carries closes only, so a bar from it is flat by
/// construction — `Bar::flat` says so rather than letting a zero range look
/// like a quiet minute.
fn bars_from_json(value: &Value) -> Vec<Bar> {
    let Some(rows) = value["series"].as_array() else { return Vec::new() };
    let mut out: Vec<Bar> = rows
        .iter()
        .filter_map(|row| {
            let time = row["time"].as_i64()?;
            let close = row["close"].as_f64()?;
            Some(match (row["open"].as_f64(), row["high"].as_f64(), row["low"].as_f64()) {
                (Some(open), Some(high), Some(low)) => {
                    Bar { time, open, high, low, close, volume: row["volume"].as_f64() }
                }
                _ => Bar::flat(time, close),
            })
        })
        .collect();
    out.sort_by_key(|b| b.time);
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let golden = PathBuf::from(arg("golden", "tests/golden"));
    let bars_dir = PathBuf::from(arg("bars", "../../nodejs/gold-options-flow/data/bars"));
    let out = PathBuf::from(arg("out", "data"));

    // `--market=gold` to redo one market; a re-migration after the tape's
    // stamps change must not touch a store the collector has been filling.
    let markets = arg("market", "gold,btc");
    for market in markets.split(',').map(str::trim).filter(|m| !m.is_empty()) {
        let path = golden.join(format!("{market}-tape.json"));
        if !path.exists() {
            println!("{market}: no tape at {} — skipped", path.display());
            continue;
        }
        let trades = tape_from_columns(&serde_json::from_str(&std::fs::read_to_string(&path)?)?);
        if trades.is_empty() {
            println!("{market}: tape is empty — skipped");
            continue;
        }
        let store = TapeStore::open(&out, market)?;
        let files = store.append(&trades)?;
        let bytes: u64 = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum();

        // Read back and count. Running this twice writes a second set of parts,
        // and the store is only idempotent because the ids are derived from the
        // prints rather than from their position — so the number that matters
        // is what comes back, not what went in.
        let stored = store.all()?.len();
        println!(
            "{market}: {} prints -> {} day partitions, {} KiB ({} KiB as JSON); {stored} readable{}",
            trades.len(),
            files.len(),
            bytes / 1024,
            std::fs::metadata(&path)?.len() / 1024,
            if stored == trades.len() { "" } else { "  <-- MIGRATION CHANGED THE COUNT" }
        );
    }

    if bars_dir.is_dir() {
        for entry in std::fs::read_dir(&bars_dir)? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            migrate_bars(&path, &out)?;
        }
    } else {
        println!("no bar directory at {} — skipped", bars_dir.display());
    }
    Ok(())
}

fn migrate_bars(path: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("bars").to_string();
    let bars = bars_from_json(&serde_json::from_str(&std::fs::read_to_string(path)?)?);
    if bars.is_empty() {
        println!("{stem}: no bars — skipped");
        return Ok(());
    }
    let target = out.join("bars").join(format!("{stem}.parquet"));
    write_bars(&target, &bars)?;
    let synthetic = bars.iter().filter(|b| b.is_synthetic()).count();
    println!(
        "{stem}: {} bars -> {} KiB ({} KiB as JSON){}",
        bars.len(),
        std::fs::metadata(&target)?.len() / 1024,
        std::fs::metadata(path)?.len() / 1024,
        if synthetic == bars.len() { ", all synthetic (closes only)" } else { "" }
    );
    Ok(())
}
