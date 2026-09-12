//! Parity gate for the backtest engine.
//!
//! Runs every strategy over the oracle's own bars and options timeline, then
//! compares **trade for trade**: entry and exit times, both prices, the exit
//! reason, size, PnL, R, and the excursions. Aggregate metrics agreeing while
//! individual fills differ is the failure mode this is built to catch — two
//! different sets of trades can produce the same profit factor.

use std::collections::BTreeMap;

use fd_backtest::engine::{Range, run_backtest};
use fd_core::parity_eq;
use fd_strategy::registry::{Registry, Side};
use serde_json::Value;

#[path = "support/golden.rs"]
mod golden;

use golden::{load_bars, load_timeline, num, opt, params_from_golden, read_golden, rules_from_manifest};

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
    let (Some(bars), Some(rules), Some(expected)) =
        (load_bars(market), rules_from_manifest(market), read_golden(market, "backtests"))
    else {
        eprintln!("skipping {market}: golden files missing — run research/export-golden.js");
        return;
    };
    let timeline = load_timeline(market);
    assert!(!bars.is_empty(), "{market}: the golden bar series is empty");

    let registry = Registry::with_builtins();
    let strategies = expected["strategies"].as_object().expect("strategies object");
    assert!(!strategies.is_empty(), "{market}: no strategies in the golden file");

    let mut diffs = Diffs::default();
    let mut compared = 0usize;

    for (id, want) in strategies {
        let strategy = registry.get(id).unwrap_or_else(|_| panic!("{id} is in the oracle but not in the registry"));
        let params = params_from_golden(&want["params"]);
        let result = run_backtest(
            &bars,
            strategy,
            &params,
            &rules,
            if strategy.needs_options() { timeline.as_ref() } else { None },
            Range::default(),
        );

        let want_trades = want["trades"].as_array().expect("trades array");
        assert_eq!(
            result.trades.len(),
            want_trades.len(),
            "{market}/{id}: {} trades vs golden {}",
            result.trades.len(),
            want_trades.len()
        );

        for (n, (got, expect)) in result.trades.iter().zip(want_trades).enumerate() {
            let at = |field: &str| format!("{market}/{id}[{n}].{field}");
            let want_side = expect["direction"].as_str().unwrap_or_default();
            assert_eq!(got.direction.as_str(), want_side, "{}", at("direction"));
            assert_eq!(
                got.exit_reason.as_str(),
                expect["exitReason"].as_str().unwrap_or_default(),
                "{}",
                at("exitReason")
            );

            diffs.check(&at("entryTime"), got.entry_time as f64, &expect["entryTime"]);
            diffs.check(&at("entryPrice"), got.entry_price, &expect["entryPrice"]);
            diffs.check(&at("exitTime"), got.exit_time as f64, &expect["exitTime"]);
            diffs.check(&at("exitPrice"), got.exit_price, &expect["exitPrice"]);
            diffs.check(&at("stop"), got.stop, &expect["stop"]);
            diffs.check_opt(&at("target"), got.target, &expect["target"]);
            diffs.check(&at("lots"), got.lots, &expect["lots"]);
            diffs.check(&at("pnlUsd"), got.pnl_usd, &expect["pnlUsd"]);
            diffs.check(&at("r"), got.r, &expect["r"]);
            diffs.check(&at("mae"), got.mae, &expect["mae"]);
            diffs.check(&at("mfe"), got.mfe, &expect["mfe"]);
            diffs.check(&at("holdMs"), got.hold_ms as f64, &expect["holdMs"]);
        }

        // Aggregates too: they are derived, but a difference here with matching
        // trades would mean the metric maths drifted.
        let metrics = &want["metrics"];
        let at = |field: &str| format!("{market}/{id}.metrics.{field}");
        diffs.check(&at("trades"), result.metrics.trades as f64, &metrics["trades"]);
        if result.metrics.trades > 0 {
            diffs.check(&at("winRate"), result.metrics.win_rate, &metrics["winRate"]);
            diffs.check(&at("avgR"), result.metrics.avg_r, &metrics["avgR"]);
            diffs.check(&at("totalR"), result.metrics.total_r, &metrics["totalR"]);
            diffs.check(&at("netPnlUsd"), result.metrics.net_pnl_usd, &metrics["netPnlUsd"]);
            diffs.check(&at("maxDrawdownUsd"), result.metrics.max_drawdown_usd, &metrics["maxDrawdownUsd"]);
            if metrics["profitFactor"].as_f64().is_some_and(f64::is_finite) {
                diffs.check(&at("profitFactor"), result.metrics.profit_factor, &metrics["profitFactor"]);
            }
        }
        diffs.check(&at("warmup"), result.warmup as f64, &want["warmup"]);
        diffs.check(&at("skippedNoAtr"), result.skipped_no_atr as f64, &want["skippedNoAtr"]);

        compared += result.trades.len();
    }

    assert!(
        diffs.0.is_empty(),
        "{market}: {} difference(s) against the oracle:\n  {}",
        diffs.0.len(),
        diffs.0.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")
    );

    eprintln!("{market}: {} strategies match the oracle trade for trade ({compared} fills)", strategies.len());
}

#[test]
fn backtests_match_the_oracle_on_gold() {
    check_market("gold");
}

#[test]
fn backtests_match_the_oracle_on_btc() {
    check_market("btc");
}

/// Guards the harness: a silently empty timeline would make the options
/// strategies trivially "match" by never trading.
#[test]
fn the_golden_timeline_loads_with_frames_and_clusters() {
    for market in ["gold", "btc"] {
        let Some(timeline) = load_timeline(market) else { continue };
        assert!(!timeline.is_empty(), "{market}: timeline is empty");
        assert!(
            timeline.frames().iter().any(|f| !f.clusters.is_empty()),
            "{market}: no frame carries a cluster, so level strategies could never fire"
        );
        assert!(
            timeline.frames().windows(2).all(|w| w[0].t <= w[1].t),
            "{market}: timeline is not in ascending time order"
        );
    }
}

/// The direction enum must serialise the way the oracle spelled it, or the
/// comparison above would pass on a mismatch it could not see.
#[test]
fn side_labels_match_the_oracle_spelling() {
    assert_eq!(Side::Long.as_str(), "LONG");
    assert_eq!(Side::Short.as_str(), "SHORT");
}

/// Keeps the metric map honest: every strategy in the oracle must exist here.
#[test]
fn every_oracle_strategy_exists_in_the_registry() {
    let registry = Registry::with_builtins();
    for market in ["gold", "btc"] {
        let Some(expected) = read_golden(market, "backtests") else { continue };
        let strategies = expected["strategies"].as_object().expect("strategies object");
        let missing: Vec<&String> = strategies.keys().filter(|id| registry.get(id).is_err()).collect();
        assert!(missing.is_empty(), "{market}: strategies missing from the Rust registry: {missing:?}");
    }
}

/// Sanity: the rules the oracle ran under must have loaded, not defaulted.
#[test]
fn trading_rules_come_from_the_manifest() {
    for market in ["gold", "btc"] {
        let Some(rules) = rules_from_manifest(market) else { continue };
        assert!(rules.contract_size > 0.0, "{market}: contract size did not load");
        assert!(rules.starting_equity_usd > 0.0, "{market}: starting equity did not load");
        assert!(rules.spread >= 0.0);
    }
    // Gold and BTC must not share a contract size, or the manifest was not read.
    if let (Some(gold), Some(btc)) = (rules_from_manifest("gold"), rules_from_manifest("btc")) {
        assert_ne!(gold.contract_size, btc.contract_size);
    }
}

/// Unused-import guard for the BTreeMap alias used by metric comparison.
const _: Option<BTreeMap<String, usize>> = None;
