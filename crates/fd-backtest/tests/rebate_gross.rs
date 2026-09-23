//! The rebate is a credit BESIDE the book, and the book does not move.
//!
//! The owner was offered three ways of recording the introducing-broker
//! rebate on 2026-09-21 and chose a separate credit line over a change to the
//! cost model, so that a reader can see how much of a result is the strategy
//! and how much is the commercial arrangement
//! (`docs/decisions/2026-09-21-rebate-credit-line.md`). Every receipt in
//! `docs/decisions/` was measured with no rebate at all; if crediting one
//! could move a gross figure, all of them would be silently restated by a
//! commercial term that has nothing to do with what the market did.
//!
//! So this drives a real backtest through the real engine and asserts the
//! gross book is bit-identical either side of the credit — the trades, the
//! metrics, and the configured spread the costs were charged at.
//!
//! **What would reopen this.** Only a decision to fold the rebate into the
//! cost model, which would be a restatement of every published number and
//! needs its own record. `--spread=` remains the way to ask what a repricing
//! would be worth, and it prints what it did.

use fd_backtest::engine::{Range, TradingRules, metrics_of, run_backtest};
use fd_backtest::rebate::{Rebate, usd_per_r};
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const BAR: i64 = 15 * 60_000;

/// Enters long on every flat bar with a fixed two-point stop and target. A
/// fixture, not a claim: what matters is that it produces a few hundred
/// trades of both signs through the ordinary engine path.
struct Fixture;

impl Strategy for Fixture {
    fn id(&self) -> &'static str {
        "test-rebate-fixture"
    }
    fn name(&self) -> &'static str {
        "Rebate fixture"
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
            reason: "fixture".into(),
        }
    }
}

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

/// A series that wanders both ways, so the run books winners and losers.
fn bars() -> Vec<Bar> {
    (0..1200)
        .map(|i| {
            let drift = (i as f64 * 0.11).sin() * 6.0 + (i as f64 * 0.017).cos() * 3.0;
            let price = 3600.0 + drift;
            Bar { time: i as i64 * BAR, open: price, high: price + 1.6, low: price - 1.6, close: price + 0.2, volume: Some(1.0) }
        })
        .collect()
}

#[test]
fn the_gross_book_is_bit_identical_either_side_of_the_credit() {
    let rules = rules();
    let bars = bars();
    let run = run_backtest(&bars, &Fixture, &Fixture.default_params(), &rules, None, Range::default());
    assert!(run.trades.len() > 100, "the fixture must produce a book worth measuring, got {}", run.trades.len());

    let before = metrics_of(&run.trades, rules.starting_equity_usd);
    let snapshot = run.trades.clone();
    let spread_before = rules.spread;

    let rebate = Rebate::new(0.45).expect("a rate");
    let credited = rebate.credited(&run.trades, &rules);
    let net = metrics_of(&credited, rules.starting_equity_usd);

    assert_eq!(run.trades, snapshot, "the credit borrowed the book and must not have written to it");
    assert_eq!(rules.spread, spread_before, "the cost model is not where a rebate lives");

    let after = metrics_of(&run.trades, rules.starting_equity_usd);
    assert_eq!(before, after, "every gross figure survives the credit unchanged");
    for (a, b) in [
        (before.profit_factor, after.profit_factor),
        (before.expectancy, after.expectancy),
        (before.total_r, after.total_r),
        (before.net_pnl_usd, after.net_pnl_usd),
        (before.max_drawdown_usd, after.max_drawdown_usd),
        (before.sharpe, after.sharpe),
    ] {
        assert_eq!(a.to_bits(), b.to_bits(), "bit-identical, not merely equal: {a} vs {b}");
    }

    // And the credited copy is a different, better book — by exactly the
    // credit and by nothing else.
    assert_eq!(credited.len(), run.trades.len());
    assert!(net.profit_factor >= before.profit_factor);
    let total = rebate.total(&run.trades, &rules);
    assert!(total > 0.0, "a book that paid spread has something to be credited");
    let moved: f64 = credited.iter().zip(&run.trades).map(|(c, g)| c.pnl_usd - g.pnl_usd).sum();
    // `total` is the sum rounded to the cent for a report; `moved` is the
    // unrounded sum the credited book actually carries. They agree to that
    // rounding and to nothing worse.
    assert!((moved - total).abs() < 0.01, "the book moved by the credit and nothing else: {moved} vs {total}");
    for (c, g) in credited.iter().zip(&run.trades) {
        assert_eq!(c.entry_time, g.entry_time);
        assert_eq!(c.exit_time, g.exit_time);
        assert_eq!(c.lots.to_bits(), g.lots.to_bits(), "the credit does not resize a trade");
        assert_eq!(c.entry_price.to_bits(), g.entry_price.to_bits(), "nor reprice its fills");
        assert_eq!(c.exit_price.to_bits(), g.exit_price.to_bits());
        assert_eq!(c.exit_kind, g.exit_kind);
    }
}

/// The registration's own arithmetic, on a real book: the credit is 45% of
/// the round-turn spread the book paid, and the spread it paid is what the
/// configured cost model says it paid.
#[test]
fn the_credit_is_forty_five_percent_of_the_spread_the_book_actually_paid() {
    let rules = rules();
    let run = run_backtest(&bars(), &Fixture, &Fixture.default_params(), &rules, None, Range::default());
    let rebate = Rebate::new(0.45).expect("a rate");

    let spread_paid: f64 = run.trades.iter().map(|t| rules.spread * t.lots * rules.contract_size).sum();
    let credited: f64 = run.trades.iter().filter_map(|t| rebate.on_trade(t, &rules)).sum();
    assert!(
        // Each credit is rounded to four decimals where the live side rounds
        // it, so the ratio is 0.45 to that rounding and not beyond it.
        (credited / spread_paid - 0.45).abs() < 1e-4,
        "the book paid {spread_paid} in spread and was credited {credited}"
    );

    // Every trade carries the basis it was booked under, so the credit is
    // priced from the cost the trade actually paid rather than from whatever
    // the config holds when somebody reads the book.
    for trade in &run.trades {
        assert_eq!(trade.spread, Some(0.28));
        assert_eq!(trade.contract_size, Some(1.0));
        assert!(usd_per_r(trade, &rules).is_some(), "an engine-stopped trade has a recoverable risk unit");
    }

    // And in the unit the registration states its break-even in: the credit
    // is a fixed fraction of the spread, so as a fraction of R it is
    // `0.45 x spread / stop distance` and nothing else. The fixture asks for
    // a two-point stop from the close and is filled at the next open plus
    // half the spread, so the distance it actually risks is its own and the
    // identity is asserted against that rather than against a round number.
    let fraction = rebate.mean_fraction_of_r(&run.trades, &rules);
    let expected: f64 = run.trades.iter().map(|t| 0.45 * rules.spread / (t.entry_price - t.stop).abs()).sum::<f64>()
        / run.trades.len() as f64;
    assert!(
        (fraction - expected).abs() < 1e-3,
        "the credit is {fraction} of R, and 0.45 x spread over the stop distance is {expected}"
    );
    // The registration's own figures bracket it: 5.8% of R at a two-point
    // stop, 1.0% at twelve. A stop of about 1.7 points sits just above the
    // first, and that is the whole reason a 45% refund cannot rescue a
    // scalper — it grows with the tightness of the stop, and so does the
    // cost it is refunding.
    assert!(fraction > 0.058, "a stop tighter than two points pays more than 5.8% of R in spread, not less");
}
