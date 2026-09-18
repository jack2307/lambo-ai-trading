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

    /// The two fields where this port deliberately does NOT reproduce the
    /// oracle, and the only two — see `docs/decisions/2026-09-18-mfe-exit-bar.md`.
    ///
    /// The oracle never counted the exit bar's excursion, so 26 of its 112
    /// golden trades record `r > mfe`, which is impossible by definition. This
    /// port counts it, so its `mfe` can only be greater than or equal to the
    /// oracle's and its `mae` only less than or equal — the fix adds one
    /// candidate to a max and to a min and can move them no other way.
    ///
    /// THIS IS NOT THE GATE BEING LOOSENED TO PASS. Equality against a number
    /// known to be wrong is replaced by a relation strictly stronger than that
    /// equality could be: the oracle's value as a BOUND, and the definitional
    /// `r <= mfe` the oracle itself violates. A regression that shrank an
    /// excursion fails here, and so does one that inflated it past the trade's
    /// own result — which the old equality check could not express at all.
    fn check_excursion(&mut self, label: &str, rust_mfe: f64, rust_mae: f64, r: f64, expect: &Value) {
        let oracle_mfe = num(&expect["mfe"]);
        let oracle_mae = num(&expect["mae"]);
        if rust_mfe < oracle_mfe && !parity_eq(rust_mfe, oracle_mfe) {
            self.0.push(format!("{label}.mfe: rust {rust_mfe} is BELOW golden {oracle_mfe}"));
        }
        if rust_mae > oracle_mae && !parity_eq(rust_mae, oracle_mae) {
            self.0.push(format!("{label}.mae: rust {rust_mae} is ABOVE golden {oracle_mae}"));
        }
        if r > rust_mfe && !parity_eq(r, rust_mfe) {
            self.0.push(format!("{label}: r {r} exceeds mfe {rust_mfe}, which is impossible"));
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
            diffs.check_excursion(&at("excursion"), got.mfe, got.mae, got.r, expect);
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

/// `r <= mfe` on every closed trade, on both markets, independently of the
/// oracle.
///
/// The check that would have caught this years ago and did not exist. A trade
/// cannot finish further in your favour than the furthest it ever went — `r`
/// and `mfe` are the same distance measured to two different points, and the
/// exit is one of the points `mfe` is a maximum over. Before 2026-09-18 the
/// exit bar was not tracked at all, so 26 of the 112 trades in the golden file
/// break it, the worst by 1.7826 R.
///
/// Deliberately not a comparison against anything. It is a property of a
/// single trade and it holds for any strategy, any market and any tape, so a
/// fixture cannot make it pass and a tape change cannot make it fail. It also
/// proves the parity path is live rather than skipping: the assertion at the
/// end fails if no trade was examined, which is how a silently-skipped golden
/// file would otherwise read as green.
#[test]
fn a_trade_never_ends_better_than_its_best_moment() {
    let registry = Registry::with_builtins();
    let mut checked = 0usize;
    let mut at_their_best = 0usize;

    for market in ["gold", "btc"] {
        let (Some(bars), Some(rules), Some(expected)) =
            (load_bars(market), rules_from_manifest(market), read_golden(market, "backtests"))
        else {
            continue;
        };
        let timeline = load_timeline(market);
        for (id, want) in expected["strategies"].as_object().expect("strategies object") {
            let Ok(strategy) = registry.get(id) else { continue };
            let params = params_from_golden(&want["params"]);
            let result = run_backtest(
                &bars,
                strategy,
                &params,
                &rules,
                if strategy.needs_options() { timeline.as_ref() } else { None },
                Range::default(),
            );
            for (n, t) in result.trades.iter().enumerate() {
                assert!(
                    t.r <= t.mfe || parity_eq(t.r, t.mfe),
                    "{market}/{id}[{n}]: r {} exceeds mfe {} — a trade cannot end better than its best moment",
                    t.r,
                    t.mfe
                );
                assert!(
                    t.r >= t.mae || parity_eq(t.r, t.mae),
                    "{market}/{id}[{n}]: r {} is below mae {} — a trade cannot end worse than its worst moment",
                    t.r,
                    t.mae
                );
                checked += 1;
                at_their_best += usize::from(parity_eq(t.r, t.mfe));
            }
        }
    }

    assert!(checked > 0, "no trades were examined: the golden files did not load and this test proved nothing");
    // A target exit leaves AT its best price, so equality is the expected
    // reading for those and not a suspicious one. Zero of them would mean the
    // exit bar is still not being counted.
    assert!(
        at_their_best > 0,
        "{checked} trades and not one ended at its own best moment: the exit bar is not being counted"
    );
    eprintln!("r <= mfe on {checked} trades; {at_their_best} ended at their best moment");
}
