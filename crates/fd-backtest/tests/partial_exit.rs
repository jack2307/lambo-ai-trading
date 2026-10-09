//! Partial exit: bank part of a position at a level, carry the rest.
//!
//! The rule lives in the RULES (`TradingRules::partial`), beside the trail,
//! and not in any strategy. That is the whole design: `control::RandomEntry`
//! emits an `Intent::Enter` the engine sizes and targets exactly like a
//! method's, so the matched null takes the same partial without one line of
//! code written for the control — the test at the bottom of this file is what
//! holds that true. `docs/decisions/2026-10-09-partial-exit.md`.
//!
//! Every number below is known by arithmetic before the engine runs:
//! risk 10.0 points, 1-ounce contracts, a 100 USD risk unit, so 10 lots and
//! one R = 100 USD. A bank of half at +0.5R and a remainder at the 1.8R
//! target is `0.5 x 0.5 + 0.5 x 1.8 = 1.15 R`, and that is what the one trade
//! must read — ONE trade, because a position is a position whether or not it
//! left in two pieces.

use fd_backtest::engine::{ExitKind, Range, TradingRules, run_backtest};
use fd_core::config::PartialExitConfig;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;

/// Enters long once, on bar 20, with a $10 sizing stop, and never exits by
/// signal. Engine exits — the default — so the engine's stop, target and clock
/// are the only ways out, which is what makes this a test of the rule and not
/// of a strategy.
struct EnterOnce;

impl Strategy for EnterOnce {
    fn id(&self) -> &'static str {
        "test-enter-once"
    }
    fn name(&self) -> &'static str {
        "enter once"
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
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_none() && ctx.i == 20 {
            Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "once".into() }
        } else {
            Intent::None
        }
    }
}

/// No spread, no commission, no financing: the arithmetic of the rule and
/// nothing else. One-ounce contracts and a 10,000 USD book put the risk unit
/// at 100 USD, so 10 lots of a $10 stop, and `lot_step` divides 10 lots
/// cleanly — the granularity question is a separate test below.
fn rules(partial: Option<(f64, f64)>) -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        reward_risk: 1.8,
        // Long enough that the clock never acts in these fixtures.
        max_hold_ms: 400 * MINUTE,
        partial: match partial {
            Some((at_r, fraction)) => PartialExitConfig { enabled: true, at_r, fraction },
            None => PartialExitConfig::default(),
        },
        ..TradingRules::default()
    }
}

/// Flat at 4000 through the entry, then whatever `path` says, one bar at a
/// time, as (low, high) pairs around a 4000 open.
///
/// The entry fills at the open of bar 21 at 4000.00, so the stop is 3990.00
/// and the engine's own target is `4000 + 1.8 x 10 = 4018.00`.
fn bars(path: &[(f64, f64)]) -> Vec<Bar> {
    let mut out: Vec<Bar> = (0..=21)
        .map(|i| Bar { time: i * 15 * MINUTE, open: 4000.0, high: 4001.0, low: 3999.0, close: 4000.0, volume: None })
        .collect();
    for (i, (low, high)) in path.iter().enumerate() {
        let time = (22 + i as i64) * 15 * MINUTE;
        out.push(Bar { time, open: 4000.0, high: *high, low: *low, close: (low + high) / 2.0, volume: None });
    }
    out
}

fn one_trade(path: &[(f64, f64)], partial: Option<(f64, f64)>) -> fd_backtest::Trade {
    let rules = rules(partial);
    let result = run_backtest(&bars(path), &EnterOnce, &EnterOnce.default_params(), &rules, None, Range::default());
    assert_eq!(
        result.trades.len(),
        1,
        "one position, one trade — a partial exit must not book a second one: {:?}",
        result.trades.iter().map(|t| (t.exit_reason.clone(), t.r)).collect::<Vec<_>>()
    );
    result.trades.into_iter().next().expect("checked")
}

/// THE RULE OFF IS THE RECORD AS IT STANDS.
///
/// Off, `banked_r` is absent rather than zero — "not managed" is not the same
/// fact as "managed and banked nothing" — and the position reads its whole
/// 1.8 R target.
#[test]
fn with_the_rule_off_a_position_reads_its_whole_target_and_banks_nothing() {
    let trade = one_trade(&[(3999.0, 4006.0), (3999.0, 4020.0)], None);
    assert_eq!(trade.exit_kind, ExitKind::Target);
    assert!((trade.r - 1.8).abs() < 1e-9, "R {}", trade.r);
    assert_eq!(trade.banked_r, None, "the rule was not asked; absent, not zero");
    assert!((trade.lots - 10.0).abs() < 1e-9, "lots {}", trade.lots);
    assert!((trade.pnl_usd - 180.0).abs() < 1e-6, "USD {}", trade.pnl_usd);
}

/// HALF AT +0.5R AND THE REST TO TARGET: 0.5 x 0.5 + 0.5 x 1.8 = 1.15 R.
///
/// One trade, not two. The R of the parts is weighted by the share of the
/// position each one held, so they sum to exactly one risk unit — which is
/// what keeps `n` meaning positions and the gate's trade-count leg meaning
/// what it meant.
#[test]
fn a_position_banks_half_at_the_level_and_the_rest_runs_to_its_own_target() {
    let trade = one_trade(&[(3999.0, 4006.0), (3999.0, 4020.0)], Some((0.5, 0.5)));
    assert_eq!(trade.exit_kind, ExitKind::Target);
    assert_eq!(trade.banked_r, Some(0.25), "half of the position at +0.5R");
    assert!((trade.r - 1.15).abs() < 1e-9, "R {} — expected 0.5x0.5 + 0.5x1.8", trade.r);
    // 5 lots banked $5 a lot, 5 lots took $18 a lot.
    assert!((trade.pnl_usd - 115.0).abs() < 1e-6, "USD {}", trade.pnl_usd);
    assert!((trade.lots - 10.0).abs() < 1e-9, "the lots TRADED, not the remainder: {}", trade.lots);
    // The identity every receipt is read through, on a position that changed
    // size mid-flight: `pnl_usd = r x risk_usd`.
    let unit = trade.risk_usd.expect("a risk unit");
    assert!((unit - 100.0).abs() < 1e-6, "one R in USD is the position AS OPENED: {unit}");
    assert!((trade.pnl_usd - trade.r * unit).abs() < 1e-6, "pnl {} vs r x unit {}", trade.pnl_usd, trade.r * unit);
}

/// AND THE SAME POSITION STOPPED OUT LOSES LESS, WHICH IS THE WHOLE POINT —
/// and the whole danger.
///
/// `0.5 x 0.5 + 0.5 x (-1.0) = -0.25 R` instead of -1.00 R. A smaller loss is
/// a smaller `Lbar`, and `E = Lbar x (PF_r - 1)` makes the gate's expectancy
/// leg HARDER as `Lbar` falls below 0.250 R. This test is here so that the
/// mechanism is pinned, not just reported.
#[test]
fn a_banked_position_that_then_stops_out_loses_only_the_part_it_still_held() {
    let trade = one_trade(&[(3999.0, 4006.0), (3985.0, 4001.0)], Some((0.5, 0.5)));
    assert_eq!(trade.exit_kind, ExitKind::Stop);
    assert_eq!(trade.banked_r, Some(0.25));
    assert!((trade.r + 0.25).abs() < 1e-9, "R {} — expected 0.5x0.5 + 0.5x(-1.0)", trade.r);
    assert!((trade.pnl_usd + 25.0).abs() < 1e-6, "USD {}", trade.pnl_usd);
}

/// A BAR THAT REACHED BOTH THE LEVEL AND THE STOP CLOSES WHOLE AT THE STOP.
///
/// OHLC does not record intrabar order, so the pessimistic reading is the only
/// honest one: the trade is not handed the good half of a bar that may have
/// gone to the stop first. `banked_r` is `Some(0.0)` — the rule was asked and
/// did not fire — which is exactly the distinction `partials` reports.
#[test]
fn a_bar_that_reached_the_level_and_the_stop_books_the_whole_loss() {
    let trade = one_trade(&[(3985.0, 4010.0)], Some((0.5, 0.5)));
    assert_eq!(trade.exit_kind, ExitKind::Stop);
    assert_eq!(trade.banked_r, Some(0.0), "asked and did not fire — not absent, not banked");
    assert!((trade.r + 1.0).abs() < 1e-9, "R {} — the full stop, no credit for the level", trade.r);
}

/// A BAR THAT REACHED BOTH THE LEVEL AND THE TARGET BANKS FIRST.
///
/// The other side of the same pessimism: `f x at_r + (1 - f) x rr` is less
/// than `rr`, so the smaller reading is the one taken. A run that booked the
/// whole target here would be crediting the position for a tail it had already
/// sold part of.
#[test]
fn a_bar_that_reached_the_level_and_the_target_banks_before_it_books() {
    let trade = one_trade(&[(3999.0, 4020.0)], Some((0.5, 0.5)));
    assert_eq!(trade.exit_kind, ExitKind::Target);
    assert_eq!(trade.banked_r, Some(0.25));
    assert!((trade.r - 1.15).abs() < 1e-9, "R {} — not the whole 1.8R", trade.r);
}

/// ONE BANK PER POSITION, NOT A LADDER.
///
/// The level is crossed on three separate bars; it is taken once.
#[test]
fn the_level_is_banked_once_however_many_bars_cross_it() {
    let trade = one_trade(&[(3999.0, 4006.0), (3999.0, 4007.0), (3999.0, 4008.0), (3999.0, 4020.0)], Some((0.5, 0.5)));
    assert_eq!(trade.banked_r, Some(0.25), "banked once");
    assert!((trade.r - 1.15).abs() < 1e-9, "R {}", trade.r);
}

/// A FRACTION OUTSIDE (0, 1) IS A CONFIGURATION ERROR, NEVER A FULL EXIT
/// UNDER ANOTHER NAME.
#[test]
fn a_fraction_of_one_or_of_zero_banks_nothing() {
    for fraction in [1.0, 0.0, -0.5, f64::NAN] {
        let trade = one_trade(&[(3999.0, 4006.0), (3999.0, 4020.0)], Some((0.5, fraction)));
        assert!((trade.r - 1.8).abs() < 1e-9, "fraction {fraction}: R {}", trade.r);
        assert_eq!(trade.banked_r, Some(0.0), "fraction {fraction}: asked, refused, did not fire");
    }
}

/// BOTH PARTS HAVE TO BE TRADABLE, OR THE POSITION CARRIES ON WHOLE.
///
/// A 100 USD book on one-ounce gold sizes a $10 stop at `1.00 / 10 = 0.10`
/// raw lots, floored to the 0.01 step. Half of it is 0.05 and both parts clear
/// `min_lot`. Raise `min_lot` to 0.10 and the bank would leave nothing
/// tradable behind, so it does not happen — a partial exit that becomes a full
/// one is not a partial exit.
#[test]
fn a_bank_that_would_leave_less_than_one_minimum_lot_does_not_happen() {
    let path = [(3999.0, 4006.0), (3999.0, 4020.0)];
    let divisible = TradingRules { starting_equity_usd: 100.0, ..rules(Some((0.5, 0.5))) };
    let result = run_backtest(&bars(&path), &EnterOnce, &EnterOnce.default_params(), &divisible, None, Range::default());
    let trade = &result.trades[0];
    assert!((trade.lots - 0.1).abs() < 1e-9, "lots {}", trade.lots);
    // 0.05 of 0.10 lots is half the position, and half a position at +0.5R
    // is +0.25 R — the weight is a share of the position, not a number of
    // lots, which is exactly why it is comparable across book sizes.
    assert_eq!(trade.banked_r, Some(0.25), "half of 0.10 lots banked at +0.5R");

    let indivisible = TradingRules { min_lot: 0.1, ..divisible };
    let result =
        run_backtest(&bars(&path), &EnterOnce, &EnterOnce.default_params(), &indivisible, None, Range::default());
    let trade = &result.trades[0];
    assert_eq!(trade.banked_r, Some(0.0), "nothing tradable to leave behind");
    assert!((trade.r - 1.8).abs() < 1e-9, "R {} — the whole position ran to target", trade.r);
}

/// THE RULE DOES NOT TOUCH A STRATEGY-MANAGED POSITION.
///
/// `check_exit` returns before anything on a `self_managed` position because
/// its stop is a sizing unit and not an order; there is no engine-managed
/// position to take a part of, and the same rule the trail follows applies
/// here. A zero on such a family is NOT MEASURED, not a refutation.
#[test]
fn a_self_managed_position_is_never_partialled() {
    struct SelfManaged;
    impl Strategy for SelfManaged {
        fn id(&self) -> &'static str {
            "test-self-managed"
        }
        fn name(&self) -> &'static str {
            "self managed"
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
        fn exits(&self) -> fd_strategy::registry::Exits {
            fd_strategy::registry::Exits::Strategy
        }
        fn on_bar(&self, ctx: &BarContext) -> Intent {
            if ctx.position.is_none() && ctx.i == 20 {
                Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "hold".into() }
            } else {
                Intent::None
            }
        }
    }
    let path = [(3999.0, 4006.0), (3999.0, 4020.0)];
    let result = run_backtest(
        &bars(&path),
        &SelfManaged,
        &SelfManaged.default_params(),
        &rules(Some((0.5, 0.5))),
        None,
        Range::default(),
    );
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::EndOfData, "the strategy owns every exit");
    assert_eq!(trade.banked_r, Some(0.0), "asked, and declined because the engine manages nothing here");
}

/// THE METRICS SAY WHETHER THE RULE WAS ASKED AND WHETHER IT FIRED.
///
/// `None` is "off"; `Some(0)` is "on and never fired". A rule that fires zero
/// times while the numbers move is a row measuring something else —
/// `tsmom/120d` printed `SURVIVES` on exactly that (brief 2026-10-07, §6a).
#[test]
fn the_metrics_tell_off_apart_from_on_and_never_fired() {
    let reached = [(3999.0, 4006.0), (3999.0, 4020.0)];
    let never = [(3999.0, 4002.0), (3985.0, 4001.0)];
    let run = |path: &[(f64, f64)], partial| {
        let rules = rules(partial);
        run_backtest(&bars(path), &EnterOnce, &EnterOnce.default_params(), &rules, None, Range::default()).metrics
    };
    let off = run(&reached, None);
    assert_eq!((off.partials, off.banked_r), (None, None), "off: absent, never zero");
    let fired = run(&reached, Some((0.5, 0.5)));
    assert_eq!(fired.partials, Some(1));
    assert_eq!(fired.banked_r, Some(0.25));
    let asked = run(&never, Some((0.5, 0.5)));
    assert_eq!(asked.partials, Some(0), "on, asked, and the level was never reached");
    assert_eq!(asked.banked_r, Some(0.0));
}

/// THE MATCHED NULL TAKES THE SAME PARTIAL, AND NOT ONE LINE WAS WRITTEN FOR
/// IT.
///
/// `control::RandomEntry` declares no `exits()`, so it inherits `Exits::Engine`
/// and the engine gives it the same `reward_risk` target it gives a method.
/// The rule lives in `TradingRules`, so the control is managed by it for the
/// same reason the method is. If this test needed a control-specific branch
/// anywhere in `engine.rs`, the design would be wrong — the comparison would
/// not be a comparison.
#[test]
fn the_matched_null_takes_the_same_partial() {
    // A long sawtooth: every leg rises far enough to clear a +0.5R level and
    // falls far enough to stop, so a random entry on any bar is managed.
    let bars: Vec<Bar> = (0..600)
        .map(|i| {
            let drift = f64::from(i % 40) * 2.0;
            let px = 4000.0 + drift;
            Bar { time: i64::from(i) * 15 * MINUTE, open: px, high: px + 12.0, low: px - 12.0, close: px, volume: None }
        })
        .collect();
    let control = fd_backtest::control::RandomEntry;
    let mut params = control.default_params();
    params.set("entryRate", 0.2);
    params.set("seed", 7.0);
    let metrics = |partial| {
        run_backtest(&bars, &control, &params, &rules(partial), None, Range::default()).metrics
    };
    let off = metrics(None);
    let on = metrics(Some((0.5, 0.5)));
    assert!(off.trades >= 20, "the control has to actually trade for this to say anything: {}", off.trades);
    assert_eq!(off.partials, None, "the control run with the rule off reports nothing, not zero");
    let fired = on.partials.expect("the control run with the rule on reports a count");
    assert!(fired > 0, "the matched null never banked: the rule is not reaching the control");
    // And it is the SAME positions: a rules-level exit rule does not change
    // when a position is opened, only what happens to it afterwards.
    assert_eq!(on.trades, off.trades, "the partial changed the control's ENTRIES, which it must not");
}
