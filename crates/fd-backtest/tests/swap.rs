//! Overnight financing is charged, and charged the way the broker charges it.
//!
//! Swap was the largest cost the model did not know about: −$82.76 per lot per
//! night on a 1 oz gold contract, tripled on Wednesday. A backtest that holds
//! positions for days and charges nothing for it is measuring a market that
//! does not exist. These tests hold one position across a known set of
//! rollovers and check the bill.

use fd_backtest::engine::{Range, TradingRules, run_backtest};
use fd_core::clock::days_from_civil;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

/// Enters long on the first bar it may and never leaves; the end of data
/// closes it. The one trade's hold is therefore the whole series.
struct HoldForever;

impl Strategy for HoldForever {
    fn id(&self) -> &'static str {
        "test-hold"
    }
    fn name(&self) -> &'static str {
        "hold"
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
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 50.0), target: None, reason: "hold".into() }
    }
}

/// Flat 15-minute bars from `from` to `to` (UTC ms). Flat so no stop can fire.
fn bars_between(from: i64, to: i64) -> Vec<Bar> {
    (from..to).step_by(15 * MINUTE as usize).map(|t| Bar::flat(t, 4000.0)).collect()
}

fn utc(year: i64, month: u32, day: u32, hour: i64) -> i64 {
    days_from_civil(year, month, day) * 24 * HOUR + hour * HOUR
}

fn rules() -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        max_hold_ms: 0, // no timeout: the hold is the test
        swap_long_per_lot: -82.76,
        swap_short_per_lot: 31.98,
        ..TradingRules::default()
    }
}

#[test]
fn a_position_held_monday_to_friday_pays_six_nights_with_wednesday_tripled() {
    // 2026-09-14 is a Monday. Bars run Monday 10:00 to Friday 10:00 New York
    // (14:00 UTC in September). Warm-up puts the entry ~4 hours in, still
    // Monday, so the rollovers crossed are Mon, Tue, Wed (×3), Thu = 6.
    let bars = bars_between(utc(2026, 9, 14, 14), utc(2026, 9, 18, 14));
    let result = run_backtest(&bars, &HoldForever, &HoldForever.default_params(), &rules(), None, Range::default());
    assert_eq!(result.trades.len(), 1, "one trade, closed by the end of data");
    let trade = &result.trades[0];
    let expected = -82.76 * trade.lots * 6.0;
    assert!(
        (trade.swap_usd - (expected * 100.0).round() / 100.0).abs() < 0.011,
        "swap {} vs expected {expected:.2} for {} lots",
        trade.swap_usd,
        trade.lots
    );
    // The bill is inside the P&L, not beside it: a flat price series with no
    // spread nets exactly the swap.
    assert!((trade.pnl_usd - trade.swap_usd).abs() < 0.011, "pnl {} vs swap {}", trade.pnl_usd, trade.swap_usd);
}

#[test]
fn a_position_closed_inside_its_session_pays_nothing() {
    // Monday 10:00 to Monday 16:00 New York: no 17:00 crossed.
    let bars = bars_between(utc(2026, 9, 14, 14), utc(2026, 9, 14, 20));
    let result = run_backtest(&bars, &HoldForever, &HoldForever.default_params(), &rules(), None, Range::default());
    assert_eq!(result.trades.len(), 1);
    assert_eq!(result.trades[0].swap_usd, 0.0);
    assert_eq!(result.trades[0].pnl_usd, 0.0, "flat price, no spread, no swap: nothing happened");
}

#[test]
fn with_no_swap_configured_the_oracle_s_numbers_are_untouched() {
    let bars = bars_between(utc(2026, 9, 14, 14), utc(2026, 9, 18, 14));
    let free = TradingRules { swap_long_per_lot: 0.0, swap_short_per_lot: 0.0, ..rules() };
    let result = run_backtest(&bars, &HoldForever, &HoldForever.default_params(), &free, None, Range::default());
    assert_eq!(result.trades[0].swap_usd, 0.0);
    assert_eq!(result.trades[0].pnl_usd, 0.0);
}
