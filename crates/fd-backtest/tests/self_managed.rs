//! A strategy that manages its own exits owns every exit.
//!
//! The stop it hands the engine is a risk unit — lots and R are sized on it —
//! not an order. The first tsmom-2 run (2026-09-13) found the engine firing
//! that stop, a target derived from it, and the four-hour timeout on a hold
//! meant to last weeks; 1,237 trades where the claim had perhaps 50. These
//! tests pin the contract: with `Exits::Strategy`, price through the stop
//! and time past the limit change nothing until the strategy says so.

use fd_backtest::engine::{ExitKind, Range, TradingRules, run_backtest};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;

/// Enters long with a sizing stop $10 under the close, exits on bar 60.
struct SizedHold;

impl Strategy for SizedHold {
    fn id(&self) -> &'static str {
        "test-sized-hold"
    }
    fn name(&self) -> &'static str {
        "sized hold"
    }
    fn description(&self) -> &'static str {
        ""
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
        Exits::Strategy
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        match ctx.position {
            Some(_) if ctx.i == 60 => Intent::Exit { reason: "bar 60".into() },
            Some(_) => Intent::None,
            None if ctx.i < 60 => Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "hold".into() },
            None => Intent::None,
        }
    }
}

fn rules() -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        max_hold_ms: 4 * 60 * MINUTE,
        ..TradingRules::default()
    }
}

/// Fifteen-minute bars: flat at 4000 for 30 bars, then a $40 drop, then
/// flat at 3960 — through the $10 sizing stop and past the four-hour limit.
fn bars() -> Vec<Bar> {
    (0..80)
        .map(|i| {
            let px = if i < 30 { 4000.0 } else { 3960.0 };
            Bar { time: i * 15 * MINUTE, open: px, high: px + 1.0, low: px - 1.0, close: px, volume: None }
        })
        .collect()
}

#[test]
fn the_sizing_stop_and_the_clock_do_not_close_a_strategy_managed_position() {
    let result = run_backtest(&bars(), &SizedHold, &SizedHold.default_params(), &rules(), None, Range::default());
    assert_eq!(result.trades.len(), 1, "one hold, closed by the strategy: {:?}", result.trades.iter().map(|t| t.exit_kind).collect::<Vec<_>>());
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::Signal);
    // The signal on bar 60 fills at the open of bar 61.
    assert_eq!(trade.exit_time, 61 * 15 * MINUTE);
}

#[test]
fn the_sizing_stop_still_sets_the_lots_and_the_r() {
    let result = run_backtest(&bars(), &SizedHold, &SizedHold.default_params(), &rules(), None, Range::default());
    let trade = &result.trades[0];
    // $100 of risk over a $10 unit: 10 lots (1 oz contracts).
    assert!((trade.lots - 10.0).abs() < 1e-9, "lots {}", trade.lots);
    // The hold lost $40 on a $10 unit: −4R.
    assert!((trade.r + 4.0).abs() < 1e-6, "R {}", trade.r);
}

#[test]
fn a_window_that_ends_mid_hold_flattens_the_book_at_the_boundary() {
    // The range ends at bar 40 (`to` is inclusive); the strategy would only
    // exit at bar 60. The hold must close at the open of the first bar past
    // the range, bar 41, not at the end of data.
    let range = Range { from: None, to: Some(40 * 15 * MINUTE) };
    let result = run_backtest(&bars(), &SizedHold, &SizedHold.default_params(), &rules(), None, range);
    assert_eq!(result.trades.len(), 1);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::EndOfData);
    assert_eq!(trade.exit_time, 41 * 15 * MINUTE);
    assert_eq!(trade.exit_price, 3960.0, "bar 41's open, no spread");
}
