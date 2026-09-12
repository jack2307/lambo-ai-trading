//! Parity gate for the feature dataset.
//!
//! Every row, every column, at the same instants. This one matters more than
//! the gates before it: a level that is wrong shows up as a strange chart, and
//! a feature that is wrong shows up as a model that trains cleanly, validates
//! cleanly and loses money for reasons nothing in the pipeline can explain.
//!
//! Comparing only the aggregate — row count, or a mean per column — would pass
//! while individual rows were shifted by one step. So the comparison is cell by
//! cell, and the first mismatches are printed with their column names rather
//! than their indices.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fd_core::classify::classify_flow;
use fd_core::config::Config;
use fd_core::parity_eq;
use fd_core::types::{AggressorSide, Bar, ExpirationType, OptionTrade, OptionType, TradeFlags};
use fd_engine::engine::{ContractMeta, EngineSettings};
use fd_features::FEATURE_NAMES;
use fd_features::dataset;
use serde_json::Value;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden")
}

fn read_golden(market: &str, name: &str) -> Option<Value> {
    let path = golden_dir().join(format!("{market}-{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The exporter writes non-finite numbers as strings; JSON has no other way.
fn num(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("unexpected string where a number was expected: {other}"),
        },
        Value::Null => f64::NAN,
        other => panic!("unexpected value: {other}"),
    }
}

fn load_trades(market: &str) -> Option<Vec<OptionTrade>> {
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
            flags: TradeFlags { multi_leg: multi_leg[i].as_bool().unwrap_or(false), ..TradeFlags::default() },
            source: "golden".to_string(),
        });
    }
    Some(out)
}

/// The candle series the oracle built its rows against.
///
/// Closes only, exactly as the feature code expects: the gold feed has no highs
/// or lows and the extractor recovers a range by resampling.
fn load_candles(market: &str) -> Option<Vec<Bar>> {
    let value = read_golden(market, "features")?;
    let times = value["candles"]["time"].as_array()?;
    let closes = value["candles"]["close"].as_array()?;
    let mut out: Vec<Bar> = times
        .iter()
        .zip(closes)
        .filter_map(|(time, close)| Some(Bar::flat(time.as_i64()?, num(close))))
        .collect();
    out.sort_by_key(|b| b.time);
    Some(out)
}

fn load_meta(market: &str) -> HashMap<String, ContractMeta> {
    let Some(value) = read_golden(market, "meta") else { return HashMap::new() };
    let Some(entries) = value["meta"].as_object() else { return HashMap::new() };
    entries
        .iter()
        .map(|(symbol, row)| {
            (
                symbol.clone(),
                ContractMeta {
                    expiration: row["expiration"].as_str().map(str::to_string),
                    expiration_type: match row["expirationType"].as_str().unwrap_or_default() {
                        "DAILY" => Some(ExpirationType::Daily),
                        "WEEKLY" => Some(ExpirationType::Weekly),
                        "MONTHLY" => Some(ExpirationType::Monthly),
                        _ => None,
                    },
                },
            )
        })
        .collect()
}

fn config() -> Config {
    Config::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"))
        .expect("the workspace config")
}

fn check_market(market: &str) {
    let (Some(expected), Some(trades), Some(candles)) =
        (read_golden(market, "features"), load_trades(market), load_candles(market))
    else {
        eprintln!("skipping {market}: golden files missing — run research/export-features.js");
        return;
    };

    let mut config = config();
    // The oracle ran with the market overlaid, which is what decides the
    // multiplier every premium in the tape was priced with.
    config.market = market.to_string();
    let settings = EngineSettings::from_config(&config, market).expect("engine settings");

    let built = dataset::build(&trades, &candles, &config, &settings, &load_meta(market));
    let want_count = expected["count"].as_u64().unwrap_or_default() as usize;

    assert_eq!(
        built.rows.len(),
        want_count,
        "{market}: row count differs — rust {} vs golden {want_count}",
        built.rows.len()
    );

    let want_names: Vec<&str> =
        expected["featureNames"].as_array().expect("names").iter().map(|n| n.as_str().unwrap()).collect();
    assert_eq!(want_names, FEATURE_NAMES.to_vec(), "{market}: the feature order differs");

    let times = expected["t"].as_array().expect("t");
    let columns = expected["x"].as_array().expect("x");
    let spot = expected["spot"].as_array().expect("spot");
    let atr = expected["atr"].as_array().expect("atr");

    let mut diffs: Vec<String> = Vec::new();
    for (i, row) in built.rows.iter().enumerate() {
        if row.t != times[i].as_i64().unwrap_or_default() {
            diffs.push(format!("row[{i}].t: rust {} vs golden {}", row.t, times[i]));
            break; // once the rows are out of step, every later diff is noise
        }
        if !parity_eq(row.spot, num(&spot[i])) {
            diffs.push(format!("row[{i}].spot: rust {} vs golden {}", row.spot, num(&spot[i])));
        }
        if !parity_eq(row.atr, num(&atr[i])) {
            diffs.push(format!("row[{i}].atr: rust {} vs golden {}", row.atr, num(&atr[i])));
        }
        for (column, name) in FEATURE_NAMES.iter().enumerate() {
            let want = num(&columns[column].as_array().expect("column")[i]);
            if !parity_eq(row.x[column], want) {
                diffs.push(format!("row[{i}].{name}: rust {} vs golden {want}", row.x[column]));
            }
        }
        if diffs.len() > 20 {
            break;
        }
    }

    assert!(diffs.is_empty(), "{market}: the feature table differs:\n  {}", diffs.join("\n  "));
    println!(
        "{market}: {} rows x {} features match the oracle cell for cell",
        built.rows.len(),
        FEATURE_NAMES.len()
    );
}

#[test]
fn the_gold_feature_table_matches_the_oracle() {
    check_market("gold");
}

#[test]
fn the_btc_feature_table_matches_the_oracle() {
    check_market("btc");
}
