//! The matched null has to be matched, and repairing it must not move a gate.
//!
//! `hypotheses::matched_rate` calibrates the random-entry control's
//! `entryRate` so the control takes about as many trades as the method it is
//! a control for: a control with five times the method's trades has a much
//! tighter profit-factor distribution, and a percentile read against it is
//! not the method's percentile. The calibration existed and its comment said
//! the rate was pinned — and it was not. The walk-forward paths wrapped the
//! control in `Preset::bare`, which pins nothing, so `RandomEntry::grid`'s
//! `entryRate` axis put a grid rate back into every cell the sweep could
//! choose. Nineteen of twenty-two rows on the 2026-09-23 rebate rescore,
//! trade counts from 2 to 1,297, were measured against the same null
//! distribution. See `docs/decisions/2026-09-23-matched-null-repair.md`.
//!
//! Two things are held here and they are different things.
//!
//! 1. **The match is achieved, not intended.** The control's median
//!    out-of-sample trade count is inside a quarter of the method's, on a
//!    real walk-forward through the real engine. This is the check the
//!    registration asked for by name: "the re-run must report the achieved
//!    match, not assume it".
//! 2. **The gate figures do not depend on the null at all.** Profit factor,
//!    trade count and expectancy come out of the method's own walk-forward
//!    and are bit-identical to the same walk-forward run with no control
//!    anywhere near it. The defect was in the control; if a repair to the
//!    control could move a gate figure, every receipt in `docs/decisions/`
//!    would be silently restated by it.
//!
//! **What would reopen this.** A change to what the control calibrates. If
//! another of the control's parameters is ever matched to the method, it
//! must be pinned the same way and named in `matched_control_for`, or it
//! will be swept away exactly as the rate was.

use fd_backtest::engine::TradingRules;
use fd_backtest::hypotheses::{COUNT_MATCH_BAND, Hypothesis, Preset, median_count, run_hypothesis_guarded};
use fd_backtest::sweep::{PromisingGate, SelectBy, walk_forward_guarded};
use fd_core::types::Bar;
use fd_strategy::filter::Filtered;
use fd_strategy::registry::{Registry, Strategy};

const BAR: i64 = 15 * 60_000;
const FOLDS: usize = 4;
const MIN_TRADES_PER_CELL: usize = 10;

/// Gold as `config/default.toml` declares the Vantage market: one ounce a
/// lot, a 0.28 round-turn spread, no commission and no swap.
fn rules() -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.28,
        commission_per_lot: 0.0,
        swap_long_per_lot: 0.0,
        swap_short_per_lot: 0.0,
        starting_equity_usd: 100.0,
        lot_step: 0.01,
        min_lot: 0.01,
        ..TradingRules::default()
    }
}

/// One round of SplitMix64, so the fixture is a random walk rather than a
/// sum of sine waves — a breakout method finds no new highs in a periodic
/// series and takes no trade at all, which would make this test vacuous.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seeded random walk with a wick on each bar. Not a market and not a
/// claim about one: a bar stream that wanders far enough for a real method
/// to find signals in it, reproducible from the seed alone.
fn bars(n: usize) -> Vec<Bar> {
    let mut price = 3600.0f64;
    (0..n)
        .map(|i| {
            let u = |stream: u64| (mix(0xF10D ^ mix((i as u64 * 0x1B3) ^ stream)) >> 11) as f64 / (1u64 << 53) as f64;
            let open = price;
            let step = (u(0) - 0.5) * 6.0;
            let close = open + step;
            let wick = 0.4 + u(1) * 2.4;
            price = close;
            Bar {
                time: i as i64 * BAR,
                open,
                high: open.max(close) + wick,
                low: open.min(close) - wick,
                close,
                volume: Some(1.0),
            }
        })
        .collect()
}

fn hypothesis(label: &str, base: &str) -> Hypothesis {
    Hypothesis {
        label: label.to_string(),
        base: base.to_string(),
        filters: Vec::new(),
        overrides: Vec::new(),
        why: "a fixture for the matched null, not a claim about the market".to_string(),
    }
}

/// The structural half, in one line: a calibrated parameter leaves the grid,
/// and the parameter that was NOT calibrated stays in it.
#[test]
fn a_calibrated_control_loses_the_calibrated_axis_and_keeps_the_others() {
    let control = fd_backtest::RandomEntry;
    let mut p = control.default_params();
    p.set("entryRate", 0.0073);

    let bare = Preset::bare(&control, p.clone());
    assert!(bare.grid().contains_key("entryRate"), "this is the defect: a bare control still sweeps its rate");

    let calibrated = Preset::calibrated(&control, p.clone(), vec!["entryRate".to_string()]);
    assert!(!calibrated.grid().contains_key("entryRate"), "the calibrated rate must survive the sweep");
    assert!(
        calibrated.grid().contains_key("stopAtr"),
        "and the axis that was not calibrated stays: the control keeps the same room to find something flattering"
    );
    assert_eq!(calibrated.default_params().get("entryRate"), 0.0073, "at the calibrated value, not the default");
}

/// The measured half: a real walk-forward, on the real engine, through the
/// real `run_hypothesis_guarded` path, and the control comes out the
/// method's size.
#[test]
fn the_matched_null_takes_about_as_many_trades_as_the_method() {
    let rules = rules();
    let bars = bars(4000);
    let registry = Registry::with_builtins();
    let gate = PromisingGate::default();

    // Two methods of deliberately different trade frequency. The defect hid
    // precisely because one uncalibrated null served every row regardless of
    // size, so one row proves nothing: what has to hold is that the control
    // FOLLOWS the method.
    let mut seen: Vec<(String, usize, f64, f64)> = Vec::new();
    for (label, base) in [("thin", "donchian-breakout"), ("busy", "rsi-reversion")] {
        let report = run_hypothesis_guarded(
            &registry,
            &hypothesis(label, base),
            &bars,
            &rules,
            FOLDS,
            SelectBy::Expectancy,
            MIN_TRADES_PER_CELL,
            &gate,
            24,
            None,
        )
        .expect("the hypothesis is well formed")
        .expect("four folds fit in four thousand bars");

        assert!(report.oos.trades > 0, "{label}: a fixture that takes no trade cannot test a count match");
        assert_eq!(report.null_trades.len(), report.null_pf.len(), "{label}: one count per null run");
        assert!(report.null_trades.len() >= 20, "{label}: the null must actually have run, got {}", report.null_trades.len());

        let ratio = report.count_match();
        assert!(
            report.count_matched(),
            "{label}: the control took a median of {:.0} trades against the method's {} — ratio {ratio:.2}, \
             outside the registered band of 1 ± {COUNT_MATCH_BAND}. The calibrated rate is being swept away again.",
            median_count(&report.null_trades),
            report.oos.trades,
        );
        seen.push((label.to_string(), report.oos.trades, median_count(&report.null_trades), ratio));
    }

    // And the two controls are different sizes, because the two methods are.
    // A single null distribution serving both is the defect itself.
    let (thin, busy) = (&seen[0], &seen[1]);
    assert!(
        thin.1 != busy.1,
        "the fixture is wrong, not the code: both methods took {} trades and cannot show a control following one",
        thin.1
    );
    let spread = (thin.2 - busy.2).abs();
    assert!(
        spread > 1.0,
        "the two controls came out the same size ({:.0} and {:.0}) although their methods did not ({} and {}) — \
         that is one null serving every row, which is what this test exists to catch",
        thin.2,
        busy.2,
        thin.1,
        busy.1,
    );
}

/// The gate figures are the method's own walk-forward and nothing else
/// touches them. Bit-identical, not merely close: a restatement of every
/// published profit factor is what "close" would hide.
#[test]
fn repairing_the_null_cannot_move_a_gate_figure() {
    let rules = rules();
    let bars = bars(4000);
    let registry = Registry::with_builtins();
    let gate = PromisingGate::default();

    for (label, base) in [("thin", "donchian-breakout"), ("busy", "rsi-reversion")] {
        let hypothesis = hypothesis(label, base);

        // The method on its own: the walk-forward `run_hypothesis_guarded`
        // runs before it builds any control at all.
        let inner = registry.get(base).expect("a registered base");
        let preset = Preset::new(inner, &hypothesis.overrides).expect("no overrides is a valid preset");
        let filtered = Filtered { inner: &preset, filters: Vec::new() };
        let alone = walk_forward_guarded(&filtered, &bars, &rules, None, FOLDS, SelectBy::Expectancy, MIN_TRADES_PER_CELL, None)
            .expect("four folds fit in four thousand bars");

        // The same method with the repaired matched null beside it.
        let report = run_hypothesis_guarded(
            &registry,
            &hypothesis,
            &bars,
            &rules,
            FOLDS,
            SelectBy::Expectancy,
            MIN_TRADES_PER_CELL,
            &gate,
            8,
            None,
        )
        .expect("the hypothesis is well formed")
        .expect("four folds fit in four thousand bars");

        assert!(alone.oos.trades > 0, "{label}: a fixture that takes no trade cannot show a gate figure holding still");
        assert_eq!(report.oos.trades, alone.oos.trades, "{label}: the trade count is the method's, not the null's");
        assert_eq!(report.oos.exits, alone.oos.exits, "{label}: and so is the breakdown of how they ended");
        for (name, a, b) in [
            ("profit factor", report.oos.profit_factor, alone.oos.profit_factor),
            ("expectancy", report.oos.expectancy, alone.oos.expectancy),
            ("total R", report.oos.total_r, alone.oos.total_r),
            ("net P&L", report.oos.net_pnl_usd, alone.oos.net_pnl_usd),
            ("max drawdown", report.oos.max_drawdown_usd, alone.oos.max_drawdown_usd),
            ("sharpe", report.oos.sharpe, alone.oos.sharpe),
            ("win rate", report.oos.win_rate, alone.oos.win_rate),
            ("average R", report.oos.avg_r, alone.oos.avg_r),
        ] {
            assert_eq!(a.to_bits(), b.to_bits(), "{label}: {name} must be bit-identical, not merely equal: {a} vs {b}");
        }
        assert_eq!(
            report.verdict.promising,
            fd_backtest::sweep::verdict(&alone.oos, &gate).promising,
            "{label}: and therefore the verdict is unchanged too",
        );
    }
}
