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
use fd_backtest::engine::{LimitEntry, LimitFills, Range, TradingRules, run_backtest};
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

/// Ten bars well above the signal close: a long order under it never fills.
fn runs_away() -> Vec<[f64; 4]> {
    (0..10).map(|_| [4010.0, 4012.0, 4009.0, 4011.0]).collect()
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
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { offset_atr: 0.0, ttl_bars: 1 }));
    let market = run(&bars, &[SIGNAL_BAR], None);
    assert_eq!(limit.trades.len(), 1, "bar 31 traded down through 4000, so the order filled");
    assert_eq!(limit.fills, LimitFills { placed: 1, filled: 1, expired: 0, replaced: 0, no_atr: 0 });
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
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { offset_atr: 1.0, ttl_bars: 4 }));
    // THE COST THE FAMILY HAS TO PAY, and it is not a cost at all in R — it is
    // a missing trade. The signal was right about direction (price left
    // without it) and the order is adversely selected by construction.
    assert!(limit.trades.is_empty(), "the level 3998.00 was never touched");
    assert_eq!(limit.fills, LimitFills { placed: 1, filled: 0, expired: 1, replaced: 0, no_atr: 0 });
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
    let short_wait = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { offset_atr: 1.0, ttl_bars: 2 }));
    assert!(short_wait.trades.is_empty(), "two bars of life, and the dip came on the fourth");
    assert_eq!(short_wait.fills.expired, 1);
    let long_wait = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { offset_atr: 1.0, ttl_bars: 4 }));
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
    let limit = run(&bars, &[SIGNAL_BAR], Some(LimitEntry { offset_atr: 1.0, ttl_bars: 1 }));
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
    let limit = run(&bars, &[SIGNAL_BAR, SIGNAL_BAR + 1], Some(LimitEntry { offset_atr: 1.0, ttl_bars: 4 }));
    // Two orders placed, the first cancelled by the second, the second left to
    // expire. One order at a time is what `pending` has always been, and the
    // replacement is counted apart because it never got its full life.
    assert_eq!(limit.fills, LimitFills { placed: 2, filled: 0, expired: 1, replaced: 1, no_atr: 0 });
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
        &cycling(Some(LimitEntry { offset_atr: 1.0, ttl_bars: 4 })),
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
