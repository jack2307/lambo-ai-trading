//! Parity gate for the level engine.
//!
//! Feeds the oracle's own normalized tape through the Rust engine and compares
//! every derived number: max pain, profile POC and value area, break-even and
//! whale levels under *every* registered model, flow aggregation, and the
//! confluence clusters.
//!
//! Two notes on what is and is not an input here:
//!
//! * Contract labels (`expiration`, `expirationType`) come from the feed, not
//!   from the engine, so they are neither supplied from the golden snapshot nor
//!   compared against it. Supplying them would make the test circular; the
//!   derived values under test do not depend on them.
//! * The oracle's `aggregate-payoff` break-even divides premium by a hard-coded
//!   `100`. That is gold's multiplier and wrong for BTC. The gate reproduces it
//!   deliberately — its job is to prove the port did not change the arithmetic.
//!   The corrected behaviour is asserted in the unit tests instead.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use fd_core::classify::{FlowClass, classify_flow};
use fd_core::parity_eq;
use fd_core::types::{AggressorSide, OptionTrade, OptionType, TradeFlags};
use fd_engine::bigtrades::BigTradeConfig;
use fd_engine::breakeven;
use fd_engine::engine::{EngineSettings, OptionsEngine};
use fd_engine::flow::BiasThresholds;
use fd_engine::maxpain::{PositioningMode, flow_max_pain};
use fd_engine::profile::{ProfileMode, ProfileOptions, build_profile};
use fd_engine::whale;
use serde_json::Value;

/// The multiplier the oracle hard-codes inside its aggregate-payoff solver.
const ORACLE_PAYOFF_MULTIPLIER: f64 = 100.0;

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

fn opt(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        other => {
            let v = num(other);
            v.is_finite().then_some(v)
        }
    }
}

/// Rebuild the normalized tape the oracle exported.
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
        let option_type = match option_type[i].as_str().unwrap_or("CALL") {
            "PUT" => OptionType::Put,
            _ => OptionType::Call,
        };
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
            flags: TradeFlags { multi_leg: multi_leg[i].as_bool().unwrap_or(false), ..Default::default() },
            source: "golden".into(),
        });
    }
    Some(out)
}

/// Build engine settings from the config the oracle recorded in its manifest.
fn settings_from_manifest(market: &str) -> Option<EngineSettings> {
    let manifest = read_golden(market, "manifest")?;
    let config = &manifest["config"];

    let mut flow_windows = BTreeMap::new();
    if let Some(windows) = config["flow"]["windows"].as_object() {
        for (name, ms) in windows {
            flow_windows.insert(name.clone(), num(ms) as i64);
        }
    }

    let mut type_weights = BTreeMap::new();
    if let Some(weights) = config["levels"]["typeWeights"].as_object() {
        for (name, weight) in weights {
            type_weights.insert(name.clone(), num(weight));
        }
    }

    let thresholds = &config["flow"]["thresholds"];
    let big = &config["bigTrades"];

    Some(EngineSettings {
        multiplier: num(&config["multiplier"]),
        profile_mode: match config["profile"]["mode"].as_str().unwrap_or("PREMIUM") {
            "VOLUME" => ProfileMode::Volume,
            "NET_PREMIUM" => ProfileMode::NetPremium,
            "ABS_NET_PREMIUM" => ProfileMode::AbsNetPremium,
            "OPEN_INTEREST" => ProfileMode::OpenInterest,
            _ => ProfileMode::Premium,
        },
        value_area_pct: num(&config["profile"]["valueAreaPct"]),
        break_even: breakeven::Model::from_id(config["models"]["breakEven"].as_str().unwrap_or_default())
            .expect("configured break-even model"),
        whale: whale::Model::from_id(config["models"]["whale"].as_str().unwrap_or_default())
            .expect("configured whale model"),
        positioning: if config["models"]["maxPainAbsolute"].as_bool().unwrap_or(true) {
            PositioningMode::Volume
        } else {
            PositioningMode::Net
        },
        big_trades: BigTradeConfig {
            min_premium_usd: num(&big["minPremiumUsd"]),
            min_contracts: opt(&big["minContracts"]),
            percentile: opt(&big["percentile"]),
            percentile_window: num(&big["percentileWindow"]) as usize,
            limit: num(&big["limit"]) as usize,
        },
        bias: BiasThresholds {
            strong_bull: num(&thresholds["strongBull"]),
            moderate_bull: num(&thresholds["moderateBull"]),
            moderate_bear: num(&thresholds["moderateBear"]),
            strong_bear: num(&thresholds["strongBear"]),
        },
        flow_windows,
        velocity_window_ms: num(&config["flow"]["velocityWindowMs"]) as i64,
        cluster_floor: num(&config["levels"]["cluster"]["floor"]),
        cluster_atr_fraction: num(&config["levels"]["cluster"]["atrFraction"]),
        max_clusters: num(&config["levels"]["maxClusters"]) as usize,
        type_weights,
    })
}

/// Collects mismatches so one run reports everything rather than the first.
#[derive(Default)]
struct Diffs(Vec<String>);

impl Diffs {
    fn check(&mut self, label: &str, actual: Option<f64>, expected: &Value) {
        let expected_opt = opt(expected);
        match (actual, expected_opt) {
            (None, None) => {}
            (Some(a), Some(e)) if parity_eq(a, e) => {}
            (a, e) => self.0.push(format!("{label}: rust {a:?} vs golden {e:?}")),
        }
    }

    fn check_num(&mut self, label: &str, actual: f64, expected: &Value) {
        let e = num(expected);
        if !parity_eq(actual, e) {
            self.0.push(format!("{label}: rust {actual} vs golden {e}"));
        }
    }

    fn assert_clean(self, market: &str, what: &str) {
        assert!(
            self.0.is_empty(),
            "{market}: {} {what} mismatch(es) against the oracle:\n  {}",
            self.0.len(),
            self.0.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")
        );
    }
}

fn check_snapshot(market: &str) {
    let (Some(tape), Some(settings), Some(expected)) =
        (load_tape(market), settings_from_manifest(market), read_golden(market, "snapshot"))
    else {
        eprintln!("skipping {market}: golden files missing — run research/export-golden.js");
        return;
    };
    assert!(!tape.is_empty(), "{market}: the golden tape is empty");

    let mut engine = OptionsEngine::new(settings);
    engine.ingest(&tape);
    // The oracle took its snapshot with no ATR, so the cluster width is the floor.
    let snapshot = engine.snapshot(&HashMap::new(), None);

    let mut diffs = Diffs::default();
    diffs.check_num("spot", snapshot.spot, &expected["spot"]);
    diffs.check_num("asOf", snapshot.as_of as f64, &expected["asOf"]);
    diffs.check_num("clusterDistance", snapshot.cluster_distance, &expected["clusterDistance"]);

    let expected_contexts = expected["contexts"].as_array().expect("contexts array");
    assert_eq!(
        snapshot.contexts.len(),
        expected_contexts.len(),
        "{market}: {} contracts vs golden {}",
        snapshot.contexts.len(),
        expected_contexts.len()
    );

    for (ctx, want) in snapshot.contexts.iter().zip(expected_contexts) {
        let symbol = want["symbol"].as_str().unwrap_or_default();
        assert_eq!(ctx.symbol, symbol, "{market}: contract order differs from the oracle");
        let at = |field: &str| format!("{symbol}.{field}");

        diffs.check(&at("maxPain"), ctx.max_pain, &want["maxPain"]);
        diffs.check(&at("poc"), ctx.poc, &want["poc"]);
        diffs.check(&at("abovePoc"), ctx.above_poc, &want["abovePoc"]);
        diffs.check(&at("underPoc"), ctx.under_poc, &want["underPoc"]);
        diffs.check(&at("vah"), ctx.vah, &want["vah"]);
        diffs.check(&at("val"), ctx.val, &want["val"]);
        diffs.check(&at("callBE"), ctx.call_be, &want["callBE"]);
        diffs.check(&at("putBE"), ctx.put_be, &want["putBE"]);
        diffs.check(&at("wSup"), ctx.w_sup, &want["wSup"]);
        diffs.check(&at("wRes"), ctx.w_res, &want["wRes"]);
        diffs.check_num(&at("dte"), ctx.dte, &want["dte"]);
        diffs.check_num(&at("trades"), ctx.trades as f64, &want["trades"]);

        let flow = &want["flow"];
        diffs.check_num(&at("flow.bullPremium"), ctx.flow.bull_premium, &flow["bullPremium"]);
        diffs.check_num(&at("flow.bearPremium"), ctx.flow.bear_premium, &flow["bearPremium"]);
        diffs.check_num(&at("flow.lcPremium"), ctx.flow.lc_premium, &flow["lcPremium"]);
        diffs.check_num(&at("flow.spPremium"), ctx.flow.sp_premium, &flow["spPremium"]);
        diffs.check_num(&at("flow.bullRatio"), ctx.flow.bull_ratio, &flow["bullRatio"]);
        diffs.check_num(&at("flow.contracts"), ctx.flow.contracts, &flow["contracts"]);
    }

    let overall = &expected["flow"]["overall"];
    diffs.check_num("flow.bullPremium", snapshot.flow_overall.bull_premium, &overall["bullPremium"]);
    diffs.check_num("flow.bearPremium", snapshot.flow_overall.bear_premium, &overall["bearPremium"]);
    diffs.check_num("flow.bullRatio", snapshot.flow_overall.bull_ratio, &overall["bullRatio"]);
    assert_eq!(
        snapshot.bias.as_str(),
        expected["flow"]["bias"].as_str().unwrap_or_default(),
        "{market}: bias label differs"
    );

    let expected_clusters = expected["clusters"].as_array().expect("clusters array");
    assert_eq!(
        snapshot.clusters.len(),
        expected_clusters.len(),
        "{market}: {} clusters vs golden {}",
        snapshot.clusters.len(),
        expected_clusters.len()
    );
    for (i, (cluster, want)) in snapshot.clusters.iter().zip(expected_clusters).enumerate() {
        diffs.check_num(&format!("cluster[{i}].low"), cluster.low, &want["low"]);
        diffs.check_num(&format!("cluster[{i}].high"), cluster.high, &want["high"]);
        diffs.check_num(&format!("cluster[{i}].center"), cluster.center, &want["center"]);
        diffs.check_num(&format!("cluster[{i}].score"), cluster.score, &want["score"]);
        diffs.check_num(&format!("cluster[{i}].types"), cluster.distinct_types as f64, &want["distinctTypes"]);
        diffs.check_num(
            &format!("cluster[{i}].expirations"),
            cluster.distinct_expirations as f64,
            &want["distinctExpirations"],
        );
    }

    eprintln!(
        "{market}: snapshot matches the oracle — {} contracts, {} clusters, {} prints",
        snapshot.contexts.len(),
        snapshot.clusters.len(),
        tape.len()
    );
    diffs.assert_clean(market, "snapshot");
}

/// Every profile mode on every contract.
fn check_profiles(market: &str) {
    let (Some(tape), Some(settings), Some(expected)) =
        (load_tape(market), settings_from_manifest(market), read_golden(market, "profiles"))
    else {
        eprintln!("skipping {market}: golden files missing");
        return;
    };

    let mut diffs = Diffs::default();
    let mut checked = 0usize;
    for (symbol, modes) in expected.as_object().expect("profiles object") {
        let subset: Vec<OptionTrade> = tape.iter().filter(|t| &t.symbol == symbol).cloned().collect();
        for (mode_name, want) in modes.as_object().expect("modes object") {
            let mode = match mode_name.as_str() {
                "VOLUME" => ProfileMode::Volume,
                "NET_PREMIUM" => ProfileMode::NetPremium,
                "ABS_NET_PREMIUM" => ProfileMode::AbsNetPremium,
                "OPEN_INTEREST" => ProfileMode::OpenInterest,
                _ => ProfileMode::Premium,
            };
            let profile = build_profile(
                &subset,
                ProfileOptions { mode, value_area_pct: settings.value_area_pct, strike_step: None },
            );
            let at = |field: &str| format!("{symbol}/{mode_name}.{field}");
            diffs.check(&at("poc"), profile.poc, &want["poc"]);
            diffs.check(&at("vah"), profile.vah, &want["vah"]);
            diffs.check(&at("val"), profile.val, &want["val"]);
            diffs.check(&at("abovePoc"), profile.above_poc, &want["abovePoc"]);
            diffs.check(&at("underPoc"), profile.under_poc, &want["underPoc"]);

            let want_strikes = want["strikes"].as_array().expect("strikes");
            diffs.check_num(&at("strikeCount"), profile.strikes.len() as f64, &Value::from(want_strikes.len()));
            for (i, (got, expect)) in profile.total.iter().zip(want["total"].as_array().expect("total")).enumerate() {
                if !parity_eq(*got, num(expect)) {
                    diffs.0.push(format!("{}[{i}]: rust {got} vs golden {}", at("total"), num(expect)));
                    break;
                }
            }
            checked += 1;
        }
    }
    eprintln!("{market}: {checked} profile(s) match the oracle");
    diffs.assert_clean(market, "profile");
}

/// Every swappable model on every contract.
fn check_models(market: &str) {
    let (Some(tape), Some(settings), Some(expected)) =
        (load_tape(market), settings_from_manifest(market), read_golden(market, "models"))
    else {
        eprintln!("skipping {market}: golden files missing");
        return;
    };

    // The oracle computed whale levels against the spot at the end of the tape.
    let spot = tape.iter().rev().map(|t| t.underlying_price).find(|p| p.is_finite()).unwrap_or(f64::NAN);

    let mut diffs = Diffs::default();
    let mut checked = 0usize;

    for (symbol, want) in expected["breakEven"].as_object().expect("breakEven object") {
        let subset: Vec<OptionTrade> = tape.iter().filter(|t| &t.symbol == symbol).cloned().collect();
        for model in breakeven::Model::ALL {
            let entry = &want[model.id()];
            if entry.is_null() {
                continue;
            }
            // See the module docs: the oracle hard-codes gold's multiplier here.
            let multiplier = ORACLE_PAYOFF_MULTIPLIER;
            diffs.check(
                &format!("{symbol}/breakEven/{}.call", model.id()),
                model.call_break_even(&subset, multiplier),
                &entry["call"],
            );
            diffs.check(
                &format!("{symbol}/breakEven/{}.put", model.id()),
                model.put_break_even(&subset, multiplier),
                &entry["put"],
            );
            checked += 1;
        }
    }

    for (symbol, want) in expected["whale"].as_object().expect("whale object") {
        let subset: Vec<OptionTrade> = tape.iter().filter(|t| &t.symbol == symbol).cloned().collect();
        for model in whale::Model::ALL {
            let entry = &want[model.id()];
            if entry.is_null() {
                continue;
            }
            let levels = model.calculate(&subset, spot, &settings.big_trades);
            diffs.check(&format!("{symbol}/whale/{}.support", model.id()), levels.support, &entry["support"]);
            diffs.check(&format!("{symbol}/whale/{}.resistance", model.id()), levels.resistance, &entry["resistance"]);
            checked += 1;
        }
    }

    for (symbol, want) in expected["maxPain"].as_object().expect("maxPain object") {
        let subset: Vec<OptionTrade> = tape.iter().filter(|t| &t.symbol == symbol).cloned().collect();
        diffs.check(
            &format!("{symbol}/maxPain.netPositioning"),
            flow_max_pain(&subset, settings.multiplier, PositioningMode::Net),
            &want["netPositioning"],
        );
        diffs.check(
            &format!("{symbol}/maxPain.volumeWeighted"),
            flow_max_pain(&subset, settings.multiplier, PositioningMode::Volume),
            &want["volumeWeighted"],
        );
        checked += 2;
    }

    eprintln!("{market}: {checked} model result(s) match the oracle");
    diffs.assert_clean(market, "model");
}

#[test]
fn snapshot_matches_the_oracle_on_gold() {
    check_snapshot("gold");
}

#[test]
fn snapshot_matches_the_oracle_on_btc() {
    check_snapshot("btc");
}

#[test]
fn profiles_match_the_oracle_on_gold() {
    check_profiles("gold");
}

#[test]
fn profiles_match_the_oracle_on_btc() {
    check_profiles("btc");
}

#[test]
fn models_match_the_oracle_on_gold() {
    check_models("gold");
}

#[test]
fn models_match_the_oracle_on_btc() {
    check_models("btc");
}

/// Guards the tape loader itself: a silently empty tape would make every gate
/// above pass for the wrong reason.
#[test]
fn the_golden_tape_loads_with_the_prints_the_manifest_claims() {
    for market in ["gold", "btc"] {
        let (Some(tape), Some(manifest)) = (load_tape(market), read_golden(market, "manifest")) else {
            continue;
        };
        let claimed = manifest["tape"]["prints"].as_u64().expect("print count") as usize;
        assert_eq!(tape.len(), claimed, "{market}: tape length disagrees with the manifest");
        assert!(tape.iter().all(|t| t.flow_class != FlowClass::Unknown), "{market}: unclassified prints in the tape");
    }
}
