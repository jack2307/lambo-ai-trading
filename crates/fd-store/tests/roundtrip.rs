//! Phase 3 gate: a tape that has been through the store is the same tape.
//!
//! The migration plan words this as "reading back a migrated tape gives the
//! same levels". That is checked here in the strong form and then in the
//! literal one:
//!
//! 1. **Every field of every print survives.** If the prints are identical then
//!    everything derived from them is identical by construction — there is no
//!    room for a level to differ.
//! 2. **The engine agrees.** Run over the in-memory tape and over the tape read
//!    back from Parquet, the snapshot matches. This is the weaker check, but it
//!    is the one that fails loudly if a column the engine reads is ever dropped
//!    from the schema.
//!
//! The engine settings below are fixed values, not the oracle's configuration.
//! The claim under test is that storage is lossless for what the engine reads,
//! and any consistent settings prove that equally well; reproducing the
//! oracle's numbers is [`fd-engine`]'s parity gate, not this one.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use fd_core::classify::classify_flow;
use fd_core::parity_eq;
use fd_core::types::{AggressorSide, Bar, OptionTrade, OptionType, TradeFlags};
use fd_engine::bigtrades::BigTradeConfig;
use fd_engine::breakeven;
use fd_engine::engine::{EngineSettings, OptionsEngine};
use fd_engine::flow::BiasThresholds;
use fd_engine::maxpain::PositioningMode;
use fd_engine::profile::ProfileMode;
use fd_engine::whale;
use fd_store::{TapeStore, read_bars, read_tape, write_bars, write_tape};
use serde_json::Value;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden")
}

fn read_golden(market: &str, name: &str) -> Option<Value> {
    let path = golden_dir().join(format!("{market}-{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn num(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) if s == "NaN" => f64::NAN,
        _ => f64::NAN,
    }
}

/// Rebuild the oracle's normalized tape from its columnar export.
fn load_tape(market: &str) -> Option<Vec<OptionTrade>> {
    let tape = read_golden(market, "tape")?;
    let count = tape["count"].as_u64()? as usize;
    let column = |name: &str| tape[name].as_array().unwrap_or_else(|| panic!("tape column {name}")).clone();

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
            id: format!("{i}"),
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
            flags: TradeFlags {
                multi_leg: multi_leg[i].as_bool().unwrap_or(false),
                ..TradeFlags::default()
            },
            source: "golden".to_string(),
        });
    }
    Some(out)
}

/// Fixed settings; see the module note on why these are not the oracle's.
fn settings() -> EngineSettings {
    EngineSettings {
        multiplier: 1.0,
        profile_mode: ProfileMode::Premium,
        value_area_pct: 0.7,
        break_even: breakeven::Model::PremiumWeighted,
        whale: whale::Model::PremiumConcentration,
        positioning: PositioningMode::Volume,
        big_trades: BigTradeConfig::default(),
        bias: BiasThresholds {
            strong_bull: 0.65,
            moderate_bull: 0.55,
            moderate_bear: 0.45,
            strong_bear: 0.35,
        },
        flow_windows: BTreeMap::from([("15m".to_string(), 900_000_i64)]),
        velocity_window_ms: 900_000,
        cluster_floor: 5.0,
        cluster_atr_fraction: 0.25,
        max_clusters: 12,
        type_weights: BTreeMap::new(),
    }
}

/// Compare two prints field by field, reporting what differs rather than just
/// that something did.
fn diff_trade(index: usize, a: &OptionTrade, b: &OptionTrade) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |field: &str, left: f64, right: f64| {
        if !parity_eq(left, right) {
            out.push(format!("[{index}].{field}: {left} vs {right}"));
        }
    };
    check("timestamp", a.timestamp as f64, b.timestamp as f64);
    check("expiration", a.expiration as f64, b.expiration as f64);
    check("dte", a.dte, b.dte);
    check("strike", a.strike, b.strike);
    check("tradePrice", a.trade_price, b.trade_price);
    check("contracts", a.contracts, b.contracts);
    check("premiumUsd", a.premium_usd, b.premium_usd);
    check("underlyingPrice", a.underlying_price, b.underlying_price);

    let mut same = |field: &str, left: String, right: String| {
        if left != right {
            out.push(format!("[{index}].{field}: {left} vs {right}"));
        }
    };
    same("id", a.id.clone(), b.id.clone());
    same("symbol", a.symbol.clone(), b.symbol.clone());
    same("instrument", format!("{:?}", a.instrument), format!("{:?}", b.instrument));
    same("underlying", a.underlying.clone(), b.underlying.clone());
    same("optionType", format!("{:?}", a.option_type), format!("{:?}", b.option_type));
    same("aggressorSide", format!("{:?}", a.aggressor_side), format!("{:?}", b.aggressor_side));
    same("flowClass", a.flow_class.as_str().to_string(), b.flow_class.as_str().to_string());
    same("flags", format!("{:?}", a.flags), format!("{:?}", b.flags));
    same("source", a.source.clone(), b.source.clone());
    same("bid", format!("{:?}", a.bid), format!("{:?}", b.bid));
    same("ask", format!("{:?}", a.ask), format!("{:?}", b.ask));
    same("exchange", format!("{:?}", a.exchange), format!("{:?}", b.exchange));
    same("sequenceId", format!("{:?}", a.sequence_id), format!("{:?}", b.sequence_id));
    same("impliedVolatility", format!("{:?}", a.implied_volatility), format!("{:?}", b.implied_volatility));
    out
}

fn check_market(market: &str) {
    let Some(tape) = load_tape(market) else {
        eprintln!("skipping {market}: golden tape missing — run research/export-golden.js");
        return;
    };
    assert!(!tape.is_empty(), "{market}: the golden tape is empty");

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(format!("{market}-tape.parquet"));
    write_tape(&path, &tape).expect("write");
    let restored = read_tape(&path).expect("read");

    assert_eq!(restored.len(), tape.len(), "{market}: row count changed in storage");
    let mut diffs = Vec::new();
    for (i, (want, got)) in tape.iter().zip(&restored).enumerate() {
        diffs.extend(diff_trade(i, want, got));
        if diffs.len() > 20 {
            break;
        }
    }
    assert!(diffs.is_empty(), "{market}: storage is not lossless:\n  {}", diffs.join("\n  "));

    // And the literal form of the gate.
    let snapshot_of = |trades: &[OptionTrade]| {
        let mut engine = OptionsEngine::new(settings());
        engine.ingest(trades);
        engine.snapshot(&HashMap::new(), None)
    };
    let from_memory = snapshot_of(&tape);
    let from_disk = snapshot_of(&restored);

    assert!(parity_eq(from_memory.spot, from_disk.spot), "{market}: spot differs after a round trip");
    assert_eq!(from_memory.as_of, from_disk.as_of, "{market}: asOf differs after a round trip");
    assert_eq!(
        from_memory.contexts.len(),
        from_disk.contexts.len(),
        "{market}: contract count differs after a round trip"
    );
    assert_eq!(
        from_memory.clusters.len(),
        from_disk.clusters.len(),
        "{market}: cluster count differs after a round trip"
    );
    for (i, (want, got)) in from_memory.clusters.iter().zip(&from_disk.clusters).enumerate() {
        assert!(
            parity_eq(want.center, got.center) && parity_eq(want.score, got.score),
            "{market}: cluster {i} moved after a round trip"
        );
    }
    assert!(
        parity_eq(from_memory.flow_overall.bull_ratio, from_disk.flow_overall.bull_ratio),
        "{market}: bull ratio differs after a round trip"
    );

    let size = std::fs::metadata(&path).expect("stat").len();
    println!(
        "{market}: {} prints survive Parquet intact — {} contracts, {} clusters, {size} bytes on disk",
        tape.len(),
        from_disk.contexts.len(),
        from_disk.clusters.len()
    );
}

#[test]
fn the_gold_tape_survives_storage() {
    check_market("gold");
}

#[test]
fn the_btc_tape_survives_storage() {
    check_market("btc");
}

#[test]
fn bars_survive_storage_including_the_absence_of_volume() {
    let bars = vec![
        Bar { time: 0, open: 1.0, high: 2.0, low: 0.5, close: 1.5, volume: Some(10.0) },
        // The gold feed has no volume. A stored zero would be a lie: it would be
        // indistinguishable from a bar that genuinely did not trade.
        Bar { time: 60_000, open: 1.5, high: 1.5, low: 1.5, close: 1.5, volume: None },
    ];
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("bars.parquet");
    write_bars(&path, &bars).expect("write");
    assert_eq!(read_bars(&path).expect("read"), bars);
}

#[test]
fn a_store_partitions_by_day_and_reads_a_range_back() {
    let Some(tape) = load_tape("btc") else { return };
    let dir = tempfile::tempdir().expect("temp dir");
    let store = TapeStore::open(dir.path(), "btc").expect("open");
    store.append(&tape).expect("append");

    let all = store.all().expect("all");
    assert_eq!(all.len(), tape.len(), "a stored tape changed size");
    assert!(all.windows(2).all(|w| w[0].timestamp <= w[1].timestamp), "reads must come back in time order");

    let (first, last) = (tape.iter().map(|t| t.timestamp).min().unwrap(), tape.iter().map(|t| t.timestamp).max().unwrap());
    let mid = first + (last - first) / 2;
    let head = store.range(first, mid).expect("range");
    let tail = store.range(mid, last + 1).expect("range");
    assert_eq!(head.len() + tail.len(), tape.len(), "a split range lost or duplicated prints");
    assert!(head.iter().all(|t| t.timestamp < mid), "range returned a print past its end");
}

#[test]
fn appending_the_same_prints_twice_does_not_duplicate_them() {
    let Some(tape) = load_tape("btc") else { return };
    let dir = tempfile::tempdir().expect("temp dir");
    let store = TapeStore::open(dir.path(), "btc").expect("open");

    // What a websocket reconnect does: backfill re-delivers prints already on
    // disk. The store must be idempotent under that, or every premium total
    // downstream is inflated.
    store.append(&tape).expect("first");
    store.append(&tape).expect("second");

    assert_eq!(store.all().expect("all").len(), tape.len(), "a re-delivered backfill duplicated prints");
}

#[test]
fn an_empty_append_leaves_no_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = TapeStore::open(dir.path(), "btc").expect("open");
    assert!(store.append(&[]).expect("append").is_empty());
    assert!(store.all().expect("all").is_empty());
}

#[test]
fn prints_identical_in_content_are_still_two_prints_after_storage() {
    // The store de-duplicates by id, so a tape whose prints were given ids by
    // content alone arrives on disk shorter than it left. On the prototype's
    // own tapes that lost about 3.6% of the prints — real fills at the same
    // millisecond, price, size and side.
    let Some(mut tape) = load_tape("btc") else { return };
    fd_core::assign_content_ids(&mut tape);

    let dir = tempfile::tempdir().expect("temp dir");
    let store = TapeStore::open(dir.path(), "btc").expect("open");
    store.append(&tape).expect("append");

    assert_eq!(
        store.all().expect("all").len(),
        tape.len(),
        "storage dropped prints that are identical in content but are separate fills"
    );
}
