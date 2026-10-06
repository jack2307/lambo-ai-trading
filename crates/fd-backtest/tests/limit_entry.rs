//! A resting entry order fills where it asked, or it never fills at all.
//!
//! The engine has filled every signal at the next bar's open since it existed,
//! paying half the spread each way. `rules.limit_entry` replaces that with an
//! order that rests on the pullback side of the signal bar's close — and the
//! whole measurement in `docs/decisions/2026-10-06-limit-entry.md` rests on
//! these mechanics being right, because a fill model that is wrong by half a
//! spread is wrong by more than the gate asks for.
//!
//! So each claim is pinned on its own:
//!
//! * off is the old engine, to the bit;
//! * a fill is PAID half the spread rather than charged it;
//! * an order price never comes to takes NO trade, and the market arm would
//!   have taken it — that is the adverse selection, visible;
//! * the TTL bounds the wait;
//! * a gap through the level does not improve the fill;
//! * a fresh signal cancels a working order rather than queueing behind it;
//! * **the matched null rests its orders too**, which is the one that makes a
//!   percentile from this family mean anything.

use fd_backtest::RandomEntry;
use fd_backtest::engine::{LimitEntry, LimitFills, Range, RestSide, TradingRules, run_backtest};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const SIGNAL_BAR: usize = 30;
const SPREAD: f64 = 0.4;
/// Half the spread, which is what a market entry pays and a resting order is
/// paid. Every price assertion below is a multiple of this.
const HALF: f64 = SPREAD / 2.0;

/// Signals LONG on the bars named by `at`, with a structural stop 30 under the
/// close — deep enough that no test here confuses a fill with a stop-out on
/// the same bar.
struct LongAt(&'static [usize]);

impl Strategy for LongAt {
    fn id(&self) -> &'static str {
        "test-long-at"
    }
    fn name(&self) -> &'static str {
        "long at named bars"
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
        if ctx.position.is_some() || !self.0.contains(&ctx.i) {
            return Intent::None;
        }
        Intent::Enter {
            side: Side::Long,
            stop: Some(ctx.bar.close - 30.0),
            target: None,
            reason: "test".into(),
        }
    }
}

fn rules(limit: Option<LimitEntry>) -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: SPREAD,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        // No target, so a fill is only ever closed by its stop or by the clock
        // — and both are far away in these fixtures.
        reward_risk: 0.0,
        max_hold_ms: 0,
        limit_entry: limit,
        ..TradingRules::default()
    }
}

/// Thirty-one flat 15m bars at 4000 with a range of exactly 2.00, so the true
/// range is 2.00 on every bar and **ATR(14) is exactly 2.00** at the signal
/// bar. Every level quoted in these tests follows from that: offset 0.5 ATR is
/// 3999.00, offset 1.0 ATR is 3998.00.
fn flat_then(tail: &[[f64; 4]]) -> Vec<Bar> {
    let mut bars: Vec<Bar> = (0..=SIGNAL_BAR as i64)
        .map(|i| Bar {
            time: i * 15 * MINUTE,
            open: 4000.0,
            high: 4001.0,
            low: 3999.0,
            close: 4000.0,
            volume: None,
        })
        .collect();
    for (k, b) in tail.iter().enumerate() {
        bars.push(Bar {
            time: (SIGNAL_BAR as i64 + 1 + k as i64) * 15 * MINUTE,
            open: b[0],
            high: b[1],
            low: b[2],
            close: b[3],
            volume: None,
        });
    }
    bars
}

/// Ten bars well above the signal close: a long LIMIT under it never fills,
/// and a long STOP above it fills at once.
fn runs_away() -> Vec<[f64; 4]> {
    (0..10).map(|_| [4010.0, 4012.0, 4009.0, 4011.0]).collect()
}

/// Ten bars well below the signal close: the mirror of `runs_away`. A long
/// STOP above the close never fills here; a long LIMIT under it fills at once.
/// This is the fixture that makes the two sides' selection visible on ONE tape.
fn falls_away() -> Vec<[f64; 4]> {
    (0..10).map(|_| [3990.0, 3991.0, 3988.0, 3989.0]).collect()
}

fn run(bars: &[Bar], at: &'static [usize], limit: Option<LimitEntry>) -> fd_backtest::BacktestResult {
    let s = LongAt(at);
    run_backtest(bars, &s, &s.default_params(), &rules(limit), None, Range::default())
}

#[test]
fn off_leaves_the_engine_exactly_as_it_was() {
    let bars = flat_then(&[[4000.0, 4001.0, 3999.0, 4000.0]; 4]);
    let market = run(&bars, &[SIGNAL_BAR], None);
    assert_eq!(market.fills, LimitFills::default(), "no order was asked for, so none was placed");
    assert_eq!(market.trades.len(), 1);
    // The signal came on bar 30 and the fill is bar 31's open, charged half
    // the spread. This is the line every receipt before 2026-10-06 was
    // measured under.
    assert!(
        (market.trades[0].entry_price - (4000.0 + HALF)).abs() < 1e-9,
        "market entry {} should be the next open plus half the spread",
        market.trades[0].entry_price
    );
}

#[test]
fn a_resting_order_is_paid_half_the_spread_instead_of_paying_it() {
    let bars = flat_then(&[[4000.0, 4001.0, 3999.0, 4000.0]; 4]);
    // Offset zero: the order rests AT the signal bar's close, so the only
    // thing that separates the two arms is the sign of the half spread.
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 0.0, ttl_bars: 1, carry_stop: false }));
    let market = run(&bars, &[SIGNAL_BAR], None);
    assert_eq!(limit.trades.len(), 1, "bar 31 traded down through 4000, so the order filled");
    assert_eq!(limit.fills, LimitFills { placed: 1, filled: 1, expired: 0, replaced: 0, no_atr: 0, no_room: 0, exit_priced_before_the_fill: 0 });
    let (l, m) = (limit.trades[0].entry_price, market.trades[0].entry_price);
    assert!((l - (4000.0 - HALF)).abs() < 1e-9, "limit entry {l} should be the level less half the spread");
    // THE WHOLE ARITHMETIC OF THE FAMILY, in one assertion: at the same level,
    // the resting order's entry is a FULL spread better than the market
    // order's, because one is paid the half the other pays.
    assert!((m - l - SPREAD).abs() < 1e-9, "market {m} minus limit {l} should be one whole spread {SPREAD}");
}

#[test]
fn an_order_price_never_comes_to_takes_no_trade_and_the_market_arm_takes_it() {
    let bars = flat_then(&runs_away());
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: false }));
    // THE COST THE FAMILY HAS TO PAY, and it is not a cost at all in R — it is
    // a missing trade. The signal was right about direction (price left
    // without it) and the order is adversely selected by construction.
    assert!(limit.trades.is_empty(), "the level 3998.00 was never touched");
    assert_eq!(limit.fills, LimitFills { placed: 1, filled: 0, expired: 1, replaced: 0, no_atr: 0, no_room: 0, exit_priced_before_the_fill: 0 });
    assert_eq!(limit.fills.rate(), Some(0.0));
    let market = run(&bars, &[SIGNAL_BAR], None);
    assert_eq!(market.trades.len(), 1, "the market arm is always in");
}

#[test]
fn the_ttl_bounds_the_wait() {
    // Three bars away from the level, then a dip through it on the fourth.
    let mut tail = vec![[4010.0, 4012.0, 4009.0, 4011.0]; 3];
    tail.push([4010.0, 4011.0, 3990.0, 3995.0]);
    tail.extend(vec![[3995.0, 3996.0, 3994.0, 3995.0]; 4]);
    let bars = flat_then(&tail);
    let short_wait = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 2, carry_stop: false }));
    assert!(short_wait.trades.is_empty(), "two bars of life, and the dip came on the fourth");
    assert_eq!(short_wait.fills.expired, 1);
    let long_wait = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: false }));
    assert_eq!(long_wait.trades.len(), 1, "four bars of life reaches the dip");
    assert!((long_wait.trades[0].entry_price - (3998.0 - HALF)).abs() < 1e-9);
}

#[test]
fn a_gap_through_the_level_does_not_improve_the_fill() {
    // Bar 31 opens 8.00 BELOW the order and never trades back up to it. A
    // resting order gets the price it asked for and nothing better — the same
    // direction `check_exit` takes a gapped stop in: gaps do not pay here.
    let mut tail = vec![[3990.0, 3992.0, 3985.0, 3991.0]];
    tail.extend(vec![[3991.0, 3992.0, 3990.0, 3991.0]; 4]);
    let bars = flat_then(&tail);
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 1, carry_stop: false }));
    assert_eq!(limit.trades.len(), 1);
    assert!(
        (limit.trades[0].entry_price - (3998.0 - HALF)).abs() < 1e-9,
        "filled at {} — a gap to 3990 must not be handed to the order",
        limit.trades[0].entry_price
    );
}

#[test]
fn a_fresh_signal_cancels_a_working_order() {
    let bars = flat_then(&runs_away());
    let limit = run(&bars, &[SIGNAL_BAR, SIGNAL_BAR + 1], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: false }));
    // Two orders placed, the first cancelled by the second, the second left to
    // expire. One order at a time is what `pending` has always been, and the
    // replacement is counted apart because it never got its full life.
    assert_eq!(limit.fills, LimitFills { placed: 2, filled: 0, expired: 1, replaced: 1, no_atr: 0, no_room: 0, exit_priced_before_the_fill: 0 });
}

#[test]
fn the_matched_null_rests_its_orders_too() {
    // The claim that makes a percentile from this family mean anything, and
    // the reason the order lives in the RULES and not in a strategy: the
    // control emits `Intent::Enter` like every method, so it is routed through
    // the identical order at the identical offset with the identical fill
    // selection, and not one line of this was written for it.
    let px = |i: i64| 4000.0 + (i % 17) as f64 - 8.0;
    let bars: Vec<Bar> = (0..600)
        .map(|i| {
            let c = px(i);
            Bar { time: i * 15 * MINUTE, open: px(i - 1), high: c + 3.0, low: c - 3.0, close: c, volume: None }
        })
        .collect();
    let mut p = RandomEntry.default_params();
    p.set("entryRate", 0.2);
    p.set("seed", 7.0);
    // A four-hour clock here and nowhere else in this file: the fixtures above
    // want a position that only its stop can close, and this one wants the
    // control to cycle often enough that its fill rate has a denominator.
    let cycling = |limit| TradingRules { max_hold_ms: 4 * 60 * MINUTE, ..rules(limit) };
    let market = run_backtest(&bars, &RandomEntry, &p, &cycling(None), None, Range::default());
    let limit = run_backtest(
        &bars,
        &RandomEntry,
        &p,
        &cycling(Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: false })),
        None,
        Range::default(),
    );
    assert_eq!(market.fills, LimitFills::default(), "the control's market arm places no orders");
    assert!(limit.fills.placed > 20, "the control placed {} orders", limit.fills.placed);
    assert!(limit.fills.filled > 0, "and some of them filled");
    assert_eq!(limit.trades.len(), limit.fills.filled, "every fill is a trade and every trade is a fill");
    let rate = limit.fills.rate().expect("orders were placed");
    assert!(rate > 0.0 && rate < 1.0, "the control's own fill rate is {rate}, which is the base rate to read a method's against");
}

#[test]
fn an_order_beyond_its_own_stop_is_refused_and_not_filled_at_a_profit() {
    // THE DEFECT THAT INVALIDATED THE FIRST RUN OF THIS FAMILY, pinned.
    //
    // `LongAt` stops 30.00 under the close. An order resting 20 ATR under it
    // (ATR is 2.00, so 40.00) lands BELOW that stop. Fill it and the "stop"
    // sits ABOVE a long entry: `(entry - stop).abs()` is still a positive risk
    // unit, and `check_exit` books hitting the stop as an exit at a PROFIT.
    // On `trend-pullback` that read 139 STOP exits and a profit factor of
    // 1.512 where the market arm read 0.544 - every one of them free money.
    let mut tail = vec![[3960.0, 3962.0, 3950.0, 3955.0]];
    tail.extend(vec![[3955.0, 3956.0, 3954.0, 3955.0]; 4]);
    let bars = flat_then(&tail);
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 20.0, ttl_bars: 4, carry_stop: false }));
    assert!(limit.trades.is_empty(), "an order below its own stop is not an order a desk can place");
    assert_eq!(limit.fills, LimitFills { placed: 0, filled: 0, expired: 0, replaced: 0, no_atr: 0, no_room: 1, exit_priced_before_the_fill: 0 });
    assert_eq!(limit.wrong_side_stop, 0);
}

#[test]
fn carrying_the_stop_preserves_the_risk_unit_and_anchoring_it_does_not() {
    // The two arms of the amendment, side by side. The signal stops 30.00
    // under its close; the order rests 1.0 ATR = 2.00 under it.
    //
    // ANCHORED leaves the stop at 3970.00, so the risk unit shrinks from 30.00
    // to 27.80 - a tighter-stopped version of the mechanism, not a cheaper
    // entry. CARRIED moves the stop to 3968.00, so the risk is the 30.00 the
    // signal designed LESS the half spread the order was paid: 29.80. That
    // residual IS the benefit, and it is the only thing carrying leaves.
    let mut tail = vec![[4000.0, 4001.0, 3997.0, 3999.0]];
    tail.extend(vec![[3999.0, 4000.0, 3998.0, 3999.0]; 6]);
    let bars = flat_then(&tail);
    let anchored = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: false }));
    let carried = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 1.0, ttl_bars: 4, carry_stop: true }));
    assert_eq!(anchored.trades.len(), 1);
    assert_eq!(carried.trades.len(), 1);
    // Same fill price in both arms: the order rests at the same level.
    let entry = 3998.0 - HALF;
    for r in [&anchored, &carried] {
        assert!((r.trades[0].entry_price - entry).abs() < 1e-9, "entry {}", r.trades[0].entry_price);
    }
    // And a different risk unit, which is the whole point of the distinction.
    assert!(((anchored.trades[0].entry_price - anchored.trades[0].stop) - (entry - 3970.0)).abs() < 1e-9, "anchored risk {}", anchored.trades[0].entry_price - anchored.trades[0].stop);
    assert!(((carried.trades[0].entry_price - carried.trades[0].stop) - (30.0 - HALF)).abs() < 1e-9, "carried risk {}", carried.trades[0].entry_price - carried.trades[0].stop);
}

// ---------------------------------------------------------------------------
// THE BREAKOUT SIDE — docs/decisions/2026-10-07-stop-entry.md
//
// Same struct, same place in the engine, same null. Only the sign of the
// offset, the touch test and the sign of the half spread differ, and each of
// those three is pinned on its own below.
// ---------------------------------------------------------------------------

#[test]
fn a_breakout_order_pays_the_half_spread_a_market_order_pays() {
    // THE OTHER SIGN OF THE WHOLE FAMILY, in one assertion. At offset zero and
    // with no gap, a stop order on the breakout side fills at the same level a
    // limit on the pullback side does — and the two are half a spread apart in
    // opposite directions, with the market arm sitting on the breakout side of
    // that pair, not in the middle of it.
    let bars = flat_then(&[[4000.0, 4001.0, 3999.0, 4000.0]; 4]);
    let market = run(&bars, &[SIGNAL_BAR], None);
    let pullback = run(
        &bars,
        &[SIGNAL_BAR],
        Some(LimitEntry { side_of_close: RestSide::Pullback, offset_atr: 0.0, ttl_bars: 1, carry_stop: false }),
    );
    let breakout = run(
        &bars,
        &[SIGNAL_BAR],
        Some(LimitEntry { side_of_close: RestSide::Breakout, offset_atr: 0.0, ttl_bars: 1, carry_stop: false }),
    );
    assert_eq!(breakout.fills, LimitFills { placed: 1, filled: 1, expired: 0, replaced: 0, no_atr: 0, no_room: 0, exit_priced_before_the_fill: 0 });
    let (b, l, m) = (breakout.trades[0].entry_price, pullback.trades[0].entry_price, market.trades[0].entry_price);
    assert!((b - (4000.0 + HALF)).abs() < 1e-9, "breakout entry {b} should be the level PLUS half the spread");
    assert!((l - (4000.0 - HALF)).abs() < 1e-9, "pullback entry {l} should be the level LESS half the spread");
    assert!((b - l - SPREAD).abs() < 1e-9, "the two sides are a whole spread apart: {b} vs {l}");
    assert!((b - m).abs() < 1e-9, "at offset zero with no gap a breakout order pays exactly what the market arm pays");
}

#[test]
fn the_two_sides_select_opposite_subsets_of_the_same_signals() {
    // The reason this is one enum and not two features. ONE signal, ONE tape,
    // one offset: the side of the close decides which of the two arms trades
    // at all. Nothing about the strategy changed between the four runs.
    for (tail, who, label) in [
        (runs_away(), RestSide::Breakout, "price ran UP"),
        (falls_away(), RestSide::Pullback, "price fell AWAY"),
    ] {
        let bars = flat_then(&tail);
        let fills = run(
            &bars,
            &[SIGNAL_BAR],
            Some(LimitEntry { side_of_close: who, offset_atr: 1.0, ttl_bars: 4, carry_stop: false }),
        );
        let misses = run(
            &bars,
            &[SIGNAL_BAR],
            Some(LimitEntry {
                side_of_close: if who == RestSide::Breakout { RestSide::Pullback } else { RestSide::Breakout },
                offset_atr: 1.0,
                ttl_bars: 4,
                carry_stop: false,
            }),
        );
        assert_eq!(fills.trades.len(), 1, "{label}: the {} side had to fill", who.label());
        assert_eq!(misses.trades.len(), 0, "{label}: the other side had to miss entirely");
        assert_eq!(misses.fills.expired, 1, "{label}: and the miss is counted, not dropped");
    }
}

#[test]
fn a_gap_through_a_breakout_level_fills_at_the_open_and_not_at_the_level() {
    // The pessimistic side of the gap, which for a STOP points the OTHER way
    // from a limit. Bar 31 opens 8.00 ABOVE the order at 4002.00: the order is
    // triggered by that open and a desk is filled THERE, not at the level it
    // asked for. An engine that handed it 4002.00 would be inventing 8.00 of
    // price per fill — about 29 half-spreads on this fixture.
    let bars = flat_then(&runs_away());
    let breakout = run(
        &bars,
        &[SIGNAL_BAR],
        Some(LimitEntry { side_of_close: RestSide::Breakout, offset_atr: 1.0, ttl_bars: 1, carry_stop: false }),
    );
    assert_eq!(breakout.trades.len(), 1);
    assert!(
        (breakout.trades[0].entry_price - (4010.0 + HALF)).abs() < 1e-9,
        "filled at {} — a gap to 4010 must be CHARGED to the order, and the level 4002 must not be",
        breakout.trades[0].entry_price
    );
}

#[test]
fn a_translated_breakout_keeps_the_risk_unit_whether_it_gapped_or_not() {
    // What `carry_stop` decides, and the one thing a BREAKOUT fill makes harder
    // than a pullback fill: a gap through a stop order fills PAST the level, so
    // "shift the stop by the amount the entry moved" and "shift it to the
    // level" are different numbers. Only the first keeps the geometry the
    // signal designed, and this pins it on both bars.
    //
    // `LongAt` stops 30.00 under the close and ATR is 2.00, so a 1.0-ATR
    // breakout long rests at 4002.00 with a designed risk of 30.00 on the tape.
    //
    //   clean fill: bar 31 trades up THROUGH 4002 from an open of 4000.00,
    //               so the order gets 4002.00 and the stop belongs at 3972.00.
    //   gapped fill: bar 31 OPENS at 4010.00, so the order is triggered there
    //               and the stop belongs at 3980.00 - NOT at 3972.00, which is
    //               what shifting to the level alone would have left behind and
    //               which would be a 38.00 risk unit nobody asked for.
    let clean = flat_then(&[[4000.0, 4003.0, 3999.0, 4002.0], [4002.0, 4003.0, 4001.0, 4002.0]]);
    let gapped = flat_then(&runs_away());
    let go = |bars: &Vec<Bar>, carry: bool| {
        run(
            bars,
            &[SIGNAL_BAR],
            Some(LimitEntry { side_of_close: RestSide::Breakout, offset_atr: 1.0, ttl_bars: 1, carry_stop: carry }),
        )
    };
    for (bars, tape, label) in [(&clean, 4002.0, "clean"), (&gapped, 4010.0, "gapped")] {
        let translated = go(bars, true);
        let anchored = go(bars, false);
        assert_eq!(translated.trades.len(), 1, "{label}: the breakout order had to fill");
        let t = &translated.trades[0];
        let a = &anchored.trades[0];
        assert!((t.entry_price - (tape + HALF)).abs() < 1e-9, "{label}: entry {}", t.entry_price);
        // THE CLAIM: the stop is a fixed 30.00 from the TAPE price the order
        // actually got, gap or no gap. Anchored leaves it at 3970.00 and the
        // risk unit grows with the gap instead.
        assert!(
            (t.stop - (tape - 30.0)).abs() < 1e-9,
            "{label}: translated stop {:?} should be 30.00 under the fill",
            t.stop
        );
        assert!((a.stop - 3970.0).abs() < 1e-9, "{label}: anchored stop {:?}", a.stop);
        // And the risk unit itself, which is the number the whole arm exists to
        // hold constant: the designed 30.00 plus the half spread the order paid
        // to get in, identical on both bars.
        assert!(
            ((t.entry_price - t.stop) - (30.0 + HALF)).abs() < 1e-9,
            "{label}: translated risk unit {}",
            t.entry_price - t.stop
        );
    }
    // The anchored arm's risk unit, by contrast, is 30.2 on the clean bar and
    // 40.2 on the gapped one: a different method on each bar.
    let clean_anchored = go(&clean, false);
    let gapped_anchored = go(&gapped, false);
    let risk = |r: &fd_backtest::BacktestResult| r.trades[0].entry_price - r.trades[0].stop;
    assert!((risk(&clean_anchored) - (32.0 + HALF)).abs() < 1e-9, "{}", risk(&clean_anchored));
    assert!((risk(&gapped_anchored) - (40.0 + HALF)).abs() < 1e-9, "{}", risk(&gapped_anchored));
}

#[test]
fn the_matched_null_rests_its_breakout_orders_too() {
    // THE PROPERTY THE BRIEF SAYS NOT TO BREAK, pinned for the new side as it
    // was for the old one. The control emits `Intent::Enter` like every method
    // and the order lives in the RULES, so a breakout run routes the null
    // through the identical order at the identical offset with the identical
    // touch test — and not one line of this was written for it. If this test
    // needed a null-specific branch anywhere, the design would be wrong.
    let px = |i: i64| 4000.0 + (i % 17) as f64 - 8.0;
    let bars: Vec<Bar> = (0..600)
        .map(|i| {
            let c = px(i);
            Bar { time: i * 15 * MINUTE, open: px(i - 1), high: c + 3.0, low: c - 3.0, close: c, volume: None }
        })
        .collect();
    let mut p = RandomEntry.default_params();
    p.set("entryRate", 0.2);
    p.set("seed", 7.0);
    let cycling = |limit| TradingRules { max_hold_ms: 4 * 60 * MINUTE, ..rules(limit) };
    let breakout = run_backtest(
        &bars,
        &RandomEntry,
        &p,
        &cycling(Some(LimitEntry {
            side_of_close: RestSide::Breakout,
            offset_atr: 1.0,
            ttl_bars: 4,
            carry_stop: false,
        })),
        None,
        Range::default(),
    );
    assert!(breakout.fills.placed > 20, "the control placed {} breakout orders", breakout.fills.placed);
    assert!(breakout.fills.filled > 0, "and some of them filled");
    assert_eq!(breakout.trades.len(), breakout.fills.filled, "every fill is a trade and every trade is a fill");
    let rate = breakout.fills.rate().expect("orders were placed");
    assert!(
        rate > 0.0 && rate < 1.0,
        "the control own breakout fill rate is {rate}, and THAT is the base rate a method confirmation rate has to beat"
    );
}

#[test]
fn a_breakout_order_beyond_its_own_target_is_refused() {
    // The `no_room` refusal is symmetric and this is the breakout half of it.
    // A long whose own absolute target sits at 4001.00 cannot have a buy stop
    // placed at 4002.00: the order would open a position already past its exit
    // and `check_exit` would book the target on the fill bar — free money, the
    // same class of defect that invalidated the first run of the pullback arm.
    struct LongWithNearTarget;
    impl Strategy for LongWithNearTarget {
        fn id(&self) -> &'static str {
            "test-long-near-target"
        }
        fn name(&self) -> &'static str {
            "long with a target one point away"
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
            if ctx.position.is_some() || ctx.i != SIGNAL_BAR {
                return Intent::None;
            }
            Intent::Enter {
                side: Side::Long,
                stop: Some(ctx.bar.close - 30.0),
                target: Some(ctx.bar.close + 1.0),
                reason: "test".into(),
            }
        }
    }
    let bars = flat_then(&runs_away());
    let r = run_backtest(
        &bars,
        &LongWithNearTarget,
        &LongWithNearTarget.default_params(),
        &rules(Some(LimitEntry {
            side_of_close: RestSide::Breakout,
            offset_atr: 1.0,
            ttl_bars: 4,
            carry_stop: false,
        })),
        None,
        Range::default(),
    );
    assert_eq!(r.fills, LimitFills { placed: 0, filled: 0, expired: 0, replaced: 0, no_atr: 0, no_room: 1, exit_priced_before_the_fill: 0 });
    assert!(r.trades.is_empty(), "no order, so no trade, and certainly no target booked on the fill bar");
}
