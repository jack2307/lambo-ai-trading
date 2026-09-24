//! The hold cap reaches the engine, and what lifting it still cannot express.
//!
//! `[trading] max_hold_ms` is four hours and until 2026-09-24 there was no
//! override path, so **every `Exits::Engine` method ever measured on this desk
//! was force-closed after at most sixteen fifteen-minute bars whatever its
//! logic intended** (`docs/decisions/2026-09-24-designed-methods.md`, defect 4).
//! `[markets.<id>.trading] max_hold_ms` now states it per market with the
//! default unchanged; `trading_rules.rs` holds the config side of that and this
//! file holds the engine side — that the number the config resolves is the
//! number a position is actually cut on.
//!
//! The second half pins defect 5, which is the reason lifting the cap alone is
//! not enough: `Exits::Engine` **cannot express "a stop and no target"**. It is
//! a separate limit with a separate cause and it is measured here rather than
//! asserted in prose, because the record has twice mistaken an intention for a
//! fact.

use fd_backtest::engine::{ExitKind, Range, TradingRules, run_backtest};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

/// Engine-managed. Enters long with a stop $500 away and **no target**, then
/// never speaks again: every exit in this file is the engine's.
///
/// The stop is far enough that the flat tape below cannot reach it, so a trade
/// that closes at all closed on the clock.
struct EngineHold;

impl Strategy for EngineHold {
    fn id(&self) -> &'static str {
        "test-engine-hold"
    }
    fn name(&self) -> &'static str {
        "engine hold"
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
        Exits::Engine
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 500.0), target: None, reason: "hold".into() }
    }
}

/// Rules with no target derived and no spread, so the only thing that can close
/// a position is the clock. `reward_risk: 0.0` is the ONLY way to say "no
/// target" here, and that is the point of `a_no_target_entry_is_given_one`.
fn rules(max_hold_ms: i64) -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        reward_risk: 0.0,
        max_hold_ms,
        ..TradingRules::default()
    }
}

/// 800 fifteen-minute bars — 200 hours — flat at 4,000, so no stop and no
/// target is reachable and the clock is the only exit.
fn bars() -> Vec<Bar> {
    (0..800)
        .map(|i| Bar { time: i * 15 * MINUTE, open: 4000.0, high: 4000.5, low: 3999.5, close: 4000.0, volume: None })
        .collect()
}

/// Four hours is sixteen bars, and this states the number rather than a
/// direction: the first trade is cut 255 minutes after it filled.
///
/// A signal on bar 0 fills at the open of bar 1 (t = 15 min), and
/// `check_exit` cuts on the first bar with `bar.time - entry_time >
/// max_hold_ms`, which is t = 270 min. That is 255 minutes held — seventeen
/// bars of exposure from a cap of sixteen, and the off-by-one is the fill rule,
/// not a bug.
#[test]
fn four_hours_cuts_the_first_hold_at_255_minutes() {
    let result = run_backtest(&bars(), &EngineHold, &EngineHold.default_params(), &rules(4 * HOUR), None, Range::default());
    let first = result.trades.first().expect("at least one trade");
    assert_eq!(first.exit_kind, ExitKind::Timeout, "the clock, not the price: {}", first.exit_reason);
    assert_eq!(first.hold_ms, 255 * MINUTE);
}

/// The cap is the only thing separating a 4-hour method from a 168-hour one on
/// identical bars and an identical strategy.
///
/// 200 hours of tape: at four hours it is chopped into dozens of holds, at a
/// week into one that runs past 100 hours. Nothing about the strategy changed.
#[test]
fn the_same_strategy_holds_for_a_week_when_the_cap_says_a_week() {
    let bars = bars();
    let four = run_backtest(&bars, &EngineHold, &EngineHold.default_params(), &rules(4 * HOUR), None, Range::default());
    let week = run_backtest(&bars, &EngineHold, &EngineHold.default_params(), &rules(168 * HOUR), None, Range::default());

    assert!(four.trades.len() > 40, "four hours over 200 hours of tape: {} trades", four.trades.len());
    assert!(week.trades.len() <= 2, "a week over 200 hours of tape: {} trades", week.trades.len());
    let long = week.trades.first().expect("a trade");
    assert_eq!(long.exit_kind, ExitKind::Timeout);
    assert!(long.hold_ms > 100 * HOUR, "held {} h", long.hold_ms / HOUR);
    // A longer hold takes fewer trades. Stated here because a horizon that
    // cannot reach the gate's 30 trades on a window is unscoreable, not
    // promising, and the arithmetic that makes that true is this ratio.
    assert!(week.trades.len() < four.trades.len());
}

/// A cap of zero is **no cap**, not four hours and not "unset".
///
/// `check_exit` guards on `rules.max_hold_ms > 0`, so a zero is a hold that
/// runs to a stop, a target or the end of data. It is a distinct third meaning
/// and `null` is not `0`: the per-market key is `Option<i64>` precisely so that
/// "says nothing" cannot be confused with "says no limit".
#[test]
fn a_cap_of_zero_is_no_cap_at_all() {
    let result = run_backtest(&bars(), &EngineHold, &EngineHold.default_params(), &rules(0), None, Range::default());
    assert_eq!(result.trades.len(), 1, "one hold, to the end of data");
    assert_eq!(result.trades[0].exit_kind, ExitKind::EndOfData);
}

/// **Defect 5, measured.** `Intent::Enter { target: None }` under
/// `Exits::Engine` means "derive one from `reward_risk`", not "none", so a
/// method that wants a stop and no target cannot say so.
///
/// This is why lifting the hold cap alone does not make a multi-day
/// engine-managed method expressible. The only two escapes are `Exits::Strategy`
/// — which surrenders the engine's stop and clock as well, and costs the audited
/// path — and `reward_risk = 0.0`, which lives in the shared `[trading]` table
/// with no per-market override of its own and so cannot be set for one method
/// without restating every receipt in `docs/decisions/` and reaching the funded
/// books. Neither is an expression of the method.
#[test]
fn a_no_target_entry_is_given_one() {
    // The registry's own default reward_risk, and the shipped config's: 1.8.
    let with_derived = TradingRules { reward_risk: 1.8, ..rules(168 * HOUR) };
    let result = run_backtest(&bars(), &EngineHold, &EngineHold.default_params(), &with_derived, None, Range::default());
    let trade = result.trades.first().expect("a trade");
    let target = trade.target.expect("the engine derived a target the strategy did not ask for");
    // Entry 4,000 with a $500 stop is $500 of risk; 1.8 R is $900 above it.
    assert!((target - 4900.0).abs() < 1e-6, "target {target}");

    // And the only way to refuse it is a GLOBAL setting, which is the defect.
    let with_none = run_backtest(&bars(), &EngineHold, &EngineHold.default_params(), &rules(168 * HOUR), None, Range::default());
    assert_eq!(with_none.trades.first().expect("a trade").target, None);
}
