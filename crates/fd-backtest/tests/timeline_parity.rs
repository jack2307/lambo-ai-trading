//! Parity gate for the options timeline.
//!
//! Until now the timeline was an *input* to the port: the golden file was read
//! and trusted. That left the one thing every options strategy actually reads
//! unverified — a max pain or a cluster could have been wrong in Rust and every
//! backtest would still have matched, because both sides were handed the same
//! frames.
//!
//! This builds the timeline from the oracle's own tape and compares it frame
//! for frame. It is also what makes backfilled data usable: without a builder,
//! new tape has no timeline and the four options strategies have nothing to
//! read.

use std::collections::BTreeMap;

use fd_backtest::timeline::{TimelineOptions, build_timeline};
use fd_core::parity_eq;
use fd_engine::bigtrades::BigTradeConfig;
use fd_engine::breakeven;
use fd_engine::engine::EngineSettings;
use fd_engine::flow::BiasThresholds;
use fd_engine::maxpain::PositioningMode;
use fd_engine::profile::ProfileMode;
use fd_engine::whale;
use serde_json::Value;

#[path = "support/golden.rs"]
mod golden;

use golden::{load_meta, num, opt, read_golden};

/// The prototype's `ai.bigTradeWindowMs`.
///
/// Not in the manifest — the exporter publishes the engine's configuration, and
/// this one lives under the AI section. Hard-coded here with its provenance
/// stated rather than guessed at.
const BIG_TRADE_WINDOW_MS: i64 = 1_800_000;

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

#[derive(Default)]
struct Diffs(Vec<String>);

impl Diffs {
    fn check(&mut self, label: &str, actual: f64, expected: &Value) {
        let want = num(expected);
        if !parity_eq(actual, want) {
            self.0.push(format!("{label}: rust {actual} vs golden {want}"));
        }
    }

    fn check_opt(&mut self, label: &str, actual: Option<f64>, expected: &Value) {
        match (actual, opt(expected)) {
            (None, None) => {}
            (Some(a), Some(e)) if parity_eq(a, e) => {}
            (a, e) => self.0.push(format!("{label}: rust {a:?} vs golden {e:?}")),
        }
    }
}

fn check_market(market: &str) {
    let (Some(trades), Some(settings), Some(expected)) =
        (golden::load_trades(market), settings_from_manifest(market), read_golden(market, "timeline"))
    else {
        eprintln!("skipping {market}: golden files missing — run research/export-golden.js");
        return;
    };
    let step_ms = expected["stepMs"].as_i64().unwrap_or(300_000);
    let options = TimelineOptions { step_ms, big_trade_window_ms: BIG_TRADE_WINDOW_MS };

    let built = build_timeline(&trades, &settings, &load_meta(market), &options);
    let want_frames = expected["frames"].as_array().expect("golden frames");

    assert_eq!(
        built.len(),
        want_frames.len(),
        "{market}: frame count differs — rust {} vs golden {}",
        built.len(),
        want_frames.len()
    );

    let mut diffs = Diffs::default();
    for (i, (got, want)) in built.frames().iter().zip(want_frames).enumerate() {
        let at = |field: &str| format!("{market}/frame[{i}].{field}");
        diffs.check(&at("t"), got.t as f64, &want["t"]);
        diffs.check(&at("spot"), got.spot, &want["spot"]);
        diffs.check(&at("bullRatio"), got.bull_ratio, &want["bullRatio"]);
        diffs.check(&at("bullRatio15m"), got.bull_ratio_15m, &want["bullRatio15m"]);
        diffs.check(&at("netFlowVelocityNorm"), got.net_flow_velocity_norm, &want["netFlowVelocityNorm"]);
        diffs.check(&at("bigTradeImbalance"), got.big_trade_imbalance, &want["bigTradeImbalance"]);

        let want_clusters = want["clusters"].as_array().cloned().unwrap_or_default();
        if got.clusters.len() != want_clusters.len() {
            diffs.0.push(format!(
                "{}: rust {} vs golden {}",
                at("clusters.len"),
                got.clusters.len(),
                want_clusters.len()
            ));
        } else {
            for (j, (cluster, wanted)) in got.clusters.iter().zip(&want_clusters).enumerate() {
                diffs.check(&at(&format!("clusters[{j}].low")), cluster.low, &wanted["low"]);
                diffs.check(&at(&format!("clusters[{j}].high")), cluster.high, &wanted["high"]);
                diffs.check(&at(&format!("clusters[{j}].center")), cluster.center, &wanted["center"]);
                diffs.check(&at(&format!("clusters[{j}].score")), cluster.score, &wanted["score"]);
            }
        }

        let want_contexts = want["contexts"].as_array().cloned().unwrap_or_default();
        if got.contexts.len() != want_contexts.len() {
            diffs.0.push(format!(
                "{}: rust {} vs golden {}",
                at("contexts.len"),
                got.contexts.len(),
                want_contexts.len()
            ));
        } else {
            for (j, (context, wanted)) in got.contexts.iter().zip(&want_contexts).enumerate() {
                let field = |name: &str| at(&format!("contexts[{j}].{name}"));
                if context.symbol != wanted["symbol"].as_str().unwrap_or_default() {
                    diffs.0.push(format!(
                        "{}: rust {} vs golden {}",
                        field("symbol"),
                        context.symbol,
                        wanted["symbol"]
                    ));
                }
                diffs.check(&field("dte"), context.dte, &wanted["dte"]);
                diffs.check_opt(&field("maxPain"), context.max_pain, &wanted["maxPain"]);
                diffs.check_opt(&field("poc"), context.poc, &wanted["poc"]);
                diffs.check_opt(&field("wSup"), context.w_sup, &wanted["wSup"]);
                diffs.check_opt(&field("wRes"), context.w_res, &wanted["wRes"]);
                diffs.check_opt(&field("callBE"), context.call_be, &wanted["callBE"]);
                diffs.check_opt(&field("putBE"), context.put_be, &wanted["putBE"]);
                diffs.check(&field("bullRatio"), context.bull_ratio, &wanted["bullRatio"]);
            }
        }

        if diffs.0.len() > 25 {
            break;
        }
    }

    assert!(
        diffs.0.is_empty(),
        "{market}: the rebuilt timeline differs from the oracle:\n  {}",
        diffs.0.join("\n  ")
    );
    println!("{market}: {} frames rebuilt from the tape match the oracle", built.len());
}

#[test]
fn the_gold_timeline_rebuilds_from_its_tape() {
    check_market("gold");
}

#[test]
fn the_btc_timeline_rebuilds_from_its_tape() {
    check_market("btc");
}
