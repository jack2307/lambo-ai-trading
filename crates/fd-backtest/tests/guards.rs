//! Risk guards are enforced, not intended.
//!
//! `[trading.guards]` existed in the config and nothing read it. These tests
//! drive a strategy that wants to enter on every bar through the engine with
//! guards on, and prove that the daily cap, the daily loss limit and the
//! cooldown each stop it — and that with guards off the same strategy is not
//! stopped, which is what keeps the parity gate honest.

use std::collections::BTreeMap;

use fd_backtest::engine::{Range, TradingRules, run_backtest, run_backtest_guarded};
use fd_backtest::Guards;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const BAR: i64 = 15 * MINUTE;

/// Wants a position on every bar, long, with a fixed stop. The most impatient
/// strategy possible — exactly what a guard exists to bound.
struct EveryBar;

impl Strategy for EveryBar {
    fn id(&self) -> &'static str {
        "test-every-bar"
    }
    fn name(&self) -> &'static str {
        "Every bar"
    }
    fn description(&self) -> &'static str {
        "Enters on every bar. A test fixture."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("atrPeriod", 14.0)])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, _: &Params) -> usize {
        16
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter {
            side: Side::Long,
            stop: Some(ctx.bar.close - 2.0),
            target: Some(ctx.bar.close + 2.0),
            reason: "every bar".into(),
        }
    }
}

/// A gently falling series: every long hits its stop within a bar or two, so
/// the run produces many short losing trades — the shape that trips every
/// guard at once.
fn falling(bars: usize) -> Vec<Bar> {
    (0..bars)
        .map(|i| {
            let price = 1000.0 - i as f64 * 1.5;
            Bar { time: i as i64 * BAR, open: price, high: price + 0.5, low: price - 3.0, close: price - 1.0, volume: Some(1.0) }
        })
        .collect()
}

fn rules() -> TradingRules {
    TradingRules { contract_size: 1.0, spread: 0.0, commission_per_lot: 0.0, starting_equity_usd: 10_000.0, ..TradingRules::default() }
}

fn refusals(result: &fd_backtest::BacktestResult) -> BTreeMap<String, usize> {
    result.skipped_by_guard.clone()
}

#[test]
fn without_guards_the_impatient_strategy_trades_constantly() {
    let bars = falling(400);
    let result = run_backtest(&bars, &EveryBar, &EveryBar.default_params(), &rules(), None, Range::default());
    assert!(result.trades.len() > 100, "the fixture must trade freely when unguarded: {}", result.trades.len());
    assert!(result.skipped_by_guard.is_empty(), "no guard was on, so nothing may be recorded as refused");
}

#[test]
fn the_daily_trade_cap_bounds_the_run() {
    let bars = falling(400); // 400 bars of 15m = ~4.2 days
    let guards = Guards { max_concurrent_positions: 1, max_trades_per_day: 4, daily_loss_limit_usd: 1e9, cooldown_ms: 0, ..Guards::unbounded() };
    let result = run_backtest_guarded(
        &bars, &EveryBar, &EveryBar.default_params(), &rules(), Some(&guards), None, Range::default(), None,
    );
    // Four per rolling day over a bit over four days: a hard ceiling far below
    // the unguarded count.
    assert!(result.trades.len() <= 4 * 5, "cap not enforced: {} trades", result.trades.len());
    assert!(result.trades.len() >= 4, "the cap must still allow trading: {} trades", result.trades.len());
    assert!(refusals(&result).get("DAILY_TRADE_CAP").copied().unwrap_or(0) > 0, "refusals must be recorded: {:?}", result.skipped_by_guard);
}

#[test]
fn the_daily_loss_limit_stops_a_losing_day() {
    let bars = falling(400);
    let unguarded = run_backtest(&bars, &EveryBar, &EveryBar.default_params(), &rules(), None, Range::default());
    let worst_day_loss = unguarded.trades.iter().take(96).map(|t| t.pnl_usd).sum::<f64>().abs();
    assert!(worst_day_loss > 0.0, "the fixture must lose money for the limit to have anything to do");

    // A limit smaller than what one unguarded day loses must stop the bleeding.
    let guards = Guards {
        max_concurrent_positions: 1,
        max_trades_per_day: 10_000,
        daily_loss_limit_usd: worst_day_loss * 0.25,
        cooldown_ms: 0,
        ..Guards::unbounded()
    };
    let result = run_backtest_guarded(
        &bars, &EveryBar, &EveryBar.default_params(), &rules(), Some(&guards), None, Range::default(), None,
    );
    assert!(result.trades.len() < unguarded.trades.len(), "the loss limit did not reduce trading");
    assert!(refusals(&result).get("DAILY_LOSS_LIMIT").copied().unwrap_or(0) > 0, "refusals must be recorded: {:?}", result.skipped_by_guard);
}

#[test]
fn the_cooldown_spaces_entries_apart() {
    let bars = falling(400);
    let cooldown = 4 * BAR;
    let guards = Guards { max_concurrent_positions: 1, max_trades_per_day: 10_000, daily_loss_limit_usd: 1e9, cooldown_ms: cooldown, ..Guards::unbounded() };
    let result = run_backtest_guarded(
        &bars, &EveryBar, &EveryBar.default_params(), &rules(), Some(&guards), None, Range::default(), None,
    );
    for pair in result.trades.windows(2) {
        assert!(
            pair[1].entry_time - pair[0].entry_time >= cooldown,
            "two entries closer than the cooldown: {} then {}",
            pair[0].entry_time,
            pair[1].entry_time
        );
    }
    assert!(refusals(&result).get("COOLDOWN").copied().unwrap_or(0) > 0, "refusals must be recorded: {:?}", result.skipped_by_guard);
}

#[test]
fn guards_off_and_guards_none_are_the_same_run() {
    // `run_backtest` is the oracle-faithful path. It must be byte-for-byte the
    // guarded path with no guards, or the parity gate is testing a different
    // engine from the one the API runs.
    let bars = falling(200);
    let plain = run_backtest(&bars, &EveryBar, &EveryBar.default_params(), &rules(), None, Range::default());
    let guarded_none = run_backtest_guarded(
        &bars, &EveryBar, &EveryBar.default_params(), &rules(), None, None, Range::default(), None,
    );
    assert_eq!(plain.trades, guarded_none.trades);
    // Field by field with a NaN-aware comparison: `Metrics` carries NaN for a
    // win-rate average with no wins, and `assert_eq!` on a NaN is always false
    // however identical the two sides are.
    assert_eq!(plain.metrics.trades, guarded_none.metrics.trades);
    assert_eq!(plain.metrics.exits, guarded_none.metrics.exits);
    for (label, a, b) in [
        ("net_pnl_usd", plain.metrics.net_pnl_usd, guarded_none.metrics.net_pnl_usd),
        ("total_r", plain.metrics.total_r, guarded_none.metrics.total_r),
        ("expectancy", plain.metrics.expectancy, guarded_none.metrics.expectancy),
        ("max_drawdown_usd", plain.metrics.max_drawdown_usd, guarded_none.metrics.max_drawdown_usd),
        ("sharpe", plain.metrics.sharpe, guarded_none.metrics.sharpe),
    ] {
        assert!(fd_core::parity_eq(a, b), "{label}: {a} vs {b}");
    }
}
