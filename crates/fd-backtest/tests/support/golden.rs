//! Reading the oracle's golden files.
//!
//! Shared by the parity gate and the sweep benchmark so that both measure the
//! same tape the JavaScript prototype published. Lives under `tests/support/`
//! rather than `tests/` so cargo does not build it as a test binary of its own.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use fd_backtest::context::{Frame, OptionsTimeline};
use fd_backtest::engine::TradingRules;
use fd_core::classify::classify_flow;
use fd_core::types::{AggressorSide, Bar, OptionTrade, OptionType, TradeFlags};
use fd_strategy::registry::{ClusterView, ContextView, Params};
use serde_json::Value;

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden")
}

pub fn read_golden(market: &str, name: &str) -> Option<Value> {
    let path = golden_dir().join(format!("{market}-{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn num(value: &Value) -> f64 {
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

pub fn opt(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        other => {
            let v = num(other);
            v.is_finite().then_some(v)
        }
    }
}

pub fn load_bars(market: &str) -> Option<Vec<Bar>> {
    let value = read_golden(market, "bars")?;
    let rows = value.get("bars")?.as_array()?;
    Some(
        rows.iter()
            .map(|row| Bar {
                time: row["time"].as_i64().expect("bar time"),
                open: num(&row["open"]),
                high: num(&row["high"]),
                low: num(&row["low"]),
                close: num(&row["close"]),
                volume: row.get("volume").map(num).filter(|v| v.is_finite()),
            })
            .collect(),
    )
}

pub fn load_timeline(market: &str) -> Option<OptionsTimeline> {
    let value = read_golden(market, "timeline")?;
    let frames = value.get("frames")?.as_array()?;
    let parsed: Vec<Frame> = frames
        .iter()
        .map(|f| Frame {
            t: f["t"].as_i64().unwrap_or_default(),
            spot: num(&f["spot"]),
            bull_ratio: num(&f["bullRatio"]),
            bull_ratio_15m: num(&f["bullRatio15m"]),
            net_flow_velocity_norm: num(&f["netFlowVelocityNorm"]),
            big_trade_imbalance: num(&f["bigTradeImbalance"]),
            clusters: f["clusters"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|c| ClusterView {
                            low: num(&c["low"]),
                            high: num(&c["high"]),
                            center: num(&c["center"]),
                            score: num(&c["score"]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            contexts: f["contexts"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|c| ContextView {
                            symbol: c["symbol"].as_str().unwrap_or_default().to_string(),
                            dte: num(&c["dte"]),
                            max_pain: opt(&c["maxPain"]),
                            poc: opt(&c["poc"]),
                            w_sup: opt(&c["wSup"]),
                            w_res: opt(&c["wRes"]),
                            call_be: opt(&c["callBE"]),
                            put_be: opt(&c["putBE"]),
                            bull_ratio: num(&c["bullRatio"]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    Some(OptionsTimeline::new(parsed))
}

/// Trading rules exactly as the oracle was configured.
pub fn rules_from_manifest(market: &str) -> Option<TradingRules> {
    let manifest = read_golden(market, "manifest")?;
    let trading = &manifest["config"]["trading"];
    let backtest = &manifest["config"]["backtest"];
    Some(TradingRules {
        // The golden files were produced by the JS oracle, which has no trail;
        // parity would be meaningless with one switched on here.
        trail: fd_core::config::TrailConfig::default(),
        // Display only; the oracle had no notion of an account currency and
        // parity is arithmetic, which these never touch.
        account_currency: "USD".to_string(),
        units_per_usd: 1.0,
        leverage: 1.0,
        contract_size: num(&trading["contractSize"]),
        spread: num(&trading["spread"]),
        commission_per_lot: num(&trading["commissionPerLot"]),
        starting_equity_usd: num(&trading["startingEquityUsd"]),
        risk_per_trade_pct: num(&trading["riskPerTradePct"]),
        stop_atr: num(&trading["stopAtr"]),
        reward_risk: num(&trading["rewardRisk"]),
        max_hold_ms: num(&trading["maxHoldMs"]) as i64,
        lot_step: num(&trading["lotStep"]),
        min_lot: num(&trading["minLot"]),
        fallback_atr_period: num(&backtest["fallbackAtrPeriod"]) as usize,
        // The oracle never charged financing; the golden trades carry none.
        swap_long_per_lot: 0.0,
        swap_short_per_lot: 0.0,
        // And no news calendar, so the scope is moot; every currency.
        news_currencies: Vec::new(),
        // The oracle rounded every price to two decimals; parity depends on it.
        price_decimals: 2,
    })
}

pub fn params_from_golden(value: &Value) -> Params {
    let mut params = Params::default();
    if let Some(object) = value.as_object() {
        for (key, v) in object {
            params.set(key, num(v));
        }
    }
    params
}


/// Rebuild the oracle's normalized tape from its columnar export.
///
/// The exporter publishes its inputs, not only its outputs, which is what lets
/// the timeline be rebuilt here rather than trusted.
pub fn load_trades(market: &str) -> Option<Vec<OptionTrade>> {
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

/// Contract labels the feed supplied alongside the tape.
///
/// Not decoration: a contract's expiration *type* weights every level it
/// contributes to a cluster, so building a timeline with an empty map produces
/// clusters at the right prices with the wrong scores.
pub fn load_meta(market: &str) -> std::collections::HashMap<String, fd_engine::engine::ContractMeta> {
    let Some(value) = read_golden(market, "meta") else { return std::collections::HashMap::new() };
    let Some(entries) = value["meta"].as_object() else { return std::collections::HashMap::new() };
    entries
        .iter()
        .map(|(symbol, row)| {
            (
                symbol.clone(),
                fd_engine::engine::ContractMeta {
                    expiration: row["expiration"].as_str().map(str::to_string),
                    expiration_type: match row["expirationType"].as_str().unwrap_or_default() {
                        "DAILY" => Some(fd_core::types::ExpirationType::Daily),
                        "WEEKLY" => Some(fd_core::types::ExpirationType::Weekly),
                        "MONTHLY" => Some(fd_core::types::ExpirationType::Monthly),
                        _ => None,
                    },
                },
            )
        })
        .collect()
}
