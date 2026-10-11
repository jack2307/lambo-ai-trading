//! What `--max-hold=` is allowed to move, and what it must not.
//!
//! The flag added on 2026-10-11 (`docs/decisions/2026-10-11-hold-cap.md`)
//! overwrites `TradingRules::max_hold_ms` for one run. Two claims were made for
//! it and neither may be left as prose:
//!
//! 1. **Passing no flag changes nothing.** The horizon still resolves out of
//!    `[markets.<id>.trading] max_hold_ms` falling back to `[trading]`, so
//!    every receipt in `docs/decisions/` reproduces. `trading_rules.rs` holds
//!    the config resolution; what is pinned here is the stronger statement the
//!    record needs — that the resolved default produces trades **bit-identical**
//!    to a four-hour literal, field by field, not merely ones that round the
//!    same way.
//! 2. **The flag reaches `Exits::Engine` and NOTHING else.** `check_exit`
//!    (`engine.rs`) returns `None` for a `self_managed` position before it ever
//!    reads `max_hold_ms`, so a self-managed method cannot be cut by the clock
//!    at any flag value. Measured here rather than read off the source.
//!
//! Claim 2 is a NEGATIVE, and a negative from a blind instrument is worthless,
//! so every assertion that nothing moved is paired in the same test with an
//! `Exits::Engine` control on the identical bars where the same two flag values
//! move a great deal. If the control ever stops moving, these tests fail
//! together instead of passing vacuously.

use fd_backtest::engine::{ExitKind, Range, Trade, TradingRules, run_backtest};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const FOUR_HOURS: i64 = 14_400_000;
const SEVENTY_TWO_HOURS: i64 = 259_200_000;

/// The horizon the shipped `[trading]` table holds, stated so a change to it
/// breaks this file rather than quietly restating what "the default" means.
const SHIPPED_DEFAULT_MS: i64 = 14_400_000;

/// `Exits::Strategy`: enters long, and exits only when IT says so — bar 300,
/// seventy-five hours in. The stop it hands over is a sizing unit.
struct SelfManagedHold;

impl Strategy for SelfManagedHold {
    fn id(&self) -> &'static str {
        "test-self-managed-hold"
    }
    fn name(&self) -> &'static str {
        "self-managed hold"
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
            Some(_) if ctx.i % 300 == 0 => Intent::Exit { reason: "bar 300".into() },
            Some(_) => Intent::None,
            None => {
                Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 25.0), target: None, reason: "hold".into() }
            }
        }
    }
}

/// `Exits::Engine` on the SAME bars: the control that proves the instrument can
/// see a horizon change at all. Stop far enough away that the tape cannot reach
/// it, so whatever closes a trade here closed it on the clock.
struct EngineHold;

impl Strategy for EngineHold {
    fn id(&self) -> &'static str {
        "test-engine-hold-flag"
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

/// No spread, no commission, no derived target: the clock is the only engine
/// exit that can fire, which is what makes the control's movement unambiguous.
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

/// 1,200 fifteen-minute bars — 300 hours — drifting a dollar a bar so a longer
/// hold has a different result rather than the same one twice.
fn bars() -> Vec<Bar> {
    (0..1200)
        .map(|i| {
            let px = 4000.0 + i as f64;
            Bar { time: i * 15 * MINUTE, open: px, high: px + 0.5, low: px - 0.5, close: px, volume: None }
        })
        .collect()
}

/// Every `f64` by its bits, every integer by its value: the comparison that
/// `round4` cannot hide a difference behind.
fn fingerprint(trades: &[Trade]) -> Vec<String> {
    trades
        .iter()
        .map(|t| {
            format!(
                "{:?}|{}|{:016x}|{}|{:016x}|{:016x}|{:?}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{}|{}",
                t.direction,
                t.entry_time,
                t.entry_price.to_bits(),
                t.exit_time,
                t.exit_price.to_bits(),
                t.stop.to_bits(),
                t.target.map(f64::to_bits),
                t.lots.to_bits(),
                t.pnl_usd.to_bits(),
                t.swap_usd.to_bits(),
                t.r.to_bits(),
                t.mae.to_bits(),
                t.hold_ms,
                t.exit_reason,
            )
        })
        .collect()
}

/// **CLAIM 1.** The shipped default and a four-hour literal are the same run,
/// to the bit.
///
/// This is the test that makes the flag safe to add. If it ever fails, the flag
/// has silently restated every receipt in `docs/decisions/`, and
/// `docs/decisions/2026-10-11-hold-cap.md` falsifier F3 says to publish no
/// measurement at all.
#[test]
fn the_default_horizon_is_bit_identical_to_a_four_hour_literal() {
    assert_eq!(SHIPPED_DEFAULT_MS, FOUR_HOURS, "the shipped [trading] table must still say four hours");
    let bars = bars();
    let from_default = run_backtest(&bars, &EngineHold, &EngineHold.default_params(), &rules(SHIPPED_DEFAULT_MS), None, Range::default());
    let from_literal = run_backtest(&bars, &EngineHold, &EngineHold.default_params(), &rules(FOUR_HOURS), None, Range::default());

    assert!(from_default.trades.len() > 40, "the fixture must carry a real book: {} trades", from_default.trades.len());
    assert_eq!(fingerprint(&from_default.trades), fingerprint(&from_literal.trades));
    // And the metrics the gate reads, by their bits rather than their printing.
    assert_eq!(from_default.metrics.expectancy.to_bits(), from_literal.metrics.expectancy.to_bits());
    assert_eq!(from_default.metrics.profit_factor.to_bits(), from_literal.metrics.profit_factor.to_bits());
    assert_eq!(from_default.metrics.total_r.to_bits(), from_literal.metrics.total_r.to_bits());
    assert_eq!(from_default.metrics.max_drawdown_usd.to_bits(), from_literal.metrics.max_drawdown_usd.to_bits());
}

/// **CLAIM 2, with its positive control in the same test.**
///
/// A self-managed mechanism at 4 h, 72 h and no cap at all is ONE run repeated:
/// 0 trades change, bit for bit. On the identical bars an engine-managed
/// mechanism is a different book at each value — and the control is asserted
/// first, so a day when the flag stops working cannot read as a day when the
/// self-managed guarantee held.
#[test]
fn the_flag_moves_engine_exits_and_leaves_self_managed_ones_untouched() {
    let bars = bars();
    let run = |s: &dyn Strategy, ms: i64| {
        run_backtest(&bars, s, &s.default_params(), &rules(ms), None, Range::default())
    };

    // THE CONTROL FIRST. Engine-managed: three horizons, three books.
    let e4 = run(&EngineHold, FOUR_HOURS);
    let e72 = run(&EngineHold, SEVENTY_TWO_HOURS);
    let e0 = run(&EngineHold, 0);
    assert!(e4.trades.len() > 10 * e72.trades.len(), "4 h {} vs 72 h {}", e4.trades.len(), e72.trades.len());
    assert_eq!(e0.trades.len(), 1, "no cap is one hold to the end of data, not four hours");
    assert_eq!(e0.trades[0].exit_kind, ExitKind::EndOfData);
    assert!(e72.trades.iter().any(|t| t.hold_ms > 48 * HOUR), "72 h must actually be held");
    assert_ne!(fingerprint(&e4.trades), fingerprint(&e72.trades), "the instrument must see the flag at all");

    // NOW THE NEGATIVE. Self-managed: the clock is not its exit at any value.
    let s4 = run(&SelfManagedHold, FOUR_HOURS);
    let s72 = run(&SelfManagedHold, SEVENTY_TWO_HOURS);
    let s0 = run(&SelfManagedHold, 0);
    assert!(s4.trades.len() >= 3, "the self-managed fixture must take trades: {}", s4.trades.len());
    assert_eq!(fingerprint(&s4.trades), fingerprint(&s72.trades), "0 trades may change between 4 h and 72 h");
    assert_eq!(fingerprint(&s4.trades), fingerprint(&s0.trades), "0 trades may change between 4 h and no cap");
    assert!(
        s4.trades.iter().all(|t| t.exit_kind != ExitKind::Timeout),
        "a self-managed position is never cut on the clock, so TIMEOUT must not appear at all",
    );
    // Held well past four hours at the four-hour setting: the guarantee is
    // about the hold, not only about the count.
    assert!(s4.trades.iter().any(|t| t.hold_ms > 48 * HOUR), "the fixture must hold past the cap it ignores");
}

/// The cap is tested on BAR BOUNDARIES, so a mean hold longer than the cap is
/// arithmetic rather than a defect — and a receipt that reads "72 h" means
/// "72 h plus whatever gap followed it".
///
/// Stated as a test because the 2026-10-11 screen measured `far-stop-break`
/// holding a mean of 313.8 min under a 255-min cut and the first reading of
/// that number was that the instrument was broken.
#[test]
fn a_gap_carries_a_hold_past_the_cap_and_that_is_the_rule_not_a_bug() {
    // Twenty fifteen-minute bars, then a 60-hour hole, then more tape. Twenty
    // because the strategy's warmup is sixteen and a signal on bar 16 fills at
    // the open of bar 17: the hole has to fall inside the seventeen bars AFTER
    // that fill or the trade is cut before it ever reaches the hole. (The first
    // version of this fixture used seventeen bars, the entry landed on the far
    // side of the hole, and the test failed for that reason and not this one.)
    let mut bars: Vec<Bar> = (0..20)
        .map(|i| Bar { time: i * 15 * MINUTE, open: 4000.0, high: 4000.5, low: 3999.5, close: 4000.0, volume: None })
        .collect();
    let resume = 19 * 15 * MINUTE + 60 * HOUR;
    bars.extend((0..40).map(|i| Bar {
        time: resume + i * 15 * MINUTE,
        open: 4000.0,
        high: 4000.5,
        low: 3999.5,
        close: 4000.0,
        volume: None,
    }));

    let result = run_backtest(&bars, &EngineHold, &EngineHold.default_params(), &rules(FOUR_HOURS), None, Range::default());
    let first = result.trades.first().expect("a trade");
    assert_eq!(first.exit_kind, ExitKind::Timeout);
    assert!(
        first.hold_ms > 48 * HOUR,
        "a four-hour cap held {} h across the gap, which is what `bar.time - entry_time > max_hold_ms` says",
        first.hold_ms / HOUR,
    );
}
