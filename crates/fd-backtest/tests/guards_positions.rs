//! The position guards close what nothing else could.
//!
//! Until 2026-09-14 a self-managed hold (`Exits::Strategy`) had no stop, no
//! clock and no guard that could close it — the risk role's finding on the
//! close-reopen and tsmom records. These tests drive an always-long hold
//! through the engine with one guard on at a time and prove each acts where
//! it is specified to, at the price it is specified to, and not at all when
//! it is zero. The last test is the parity promise: every guard at zero and
//! `Some(&guards)` is the unguarded run, trade for trade.
//!
//! The news guard lives in `guards_news.rs`: its calendar is installed once
//! per process, so it gets a process of its own.

use fd_backtest::engine::{ExitKind, Range, TradingRules, run_backtest, run_backtest_guarded};
use fd_backtest::{GuardKind, Guards};
use fd_core::clock::days_from_civil;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const BAR: i64 = 15 * MINUTE;

/// Long whenever flat, from the first bar after warmup; never exits on its
/// own. With `Exits::Strategy` that is a hold only a guard can end; with
/// `Exits::Engine` the engine's stop and target apply too.
struct AlwaysLong {
    exits: Exits,
    stop_distance: f64,
    target_distance: Option<f64>,
}

impl Strategy for AlwaysLong {
    fn id(&self) -> &'static str {
        "test-always-long"
    }
    fn name(&self) -> &'static str {
        "always long"
    }
    fn description(&self) -> &'static str {
        "Enters long whenever flat and never exits. A test fixture."
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
        self.exits
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter {
            side: Side::Long,
            stop: Some(ctx.bar.close - self.stop_distance),
            target: self.target_distance.map(|d| ctx.bar.close + d),
            reason: "always".into(),
        }
    }
}

fn hold() -> AlwaysLong {
    AlwaysLong { exits: Exits::Strategy, stop_distance: 10.0, target_distance: None }
}

fn rules() -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        // A four-hour clock that must never fire on a self-managed hold.
        max_hold_ms: 4 * HOUR,
        ..TradingRules::default()
    }
}

fn flat_bar(time: i64, px: f64) -> Bar {
    Bar { time, open: px, high: px + 1.0, low: px - 1.0, close: px, volume: None }
}

fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> i64 {
    days_from_civil(year, month, day) * 86_400_000 + hour * HOUR + minute * MINUTE
}

fn guarded(bars: &[Bar], strategy: &dyn Strategy, rules: &TradingRules, guards: &Guards) -> fd_backtest::BacktestResult {
    run_backtest_guarded(bars, strategy, &strategy.default_params(), rules, Some(guards), None, Range::default(), None)
}

// ---------------------------------------------------------------- open-loss cap

/// Flat at 4000 for 30 bars, then `breach` as bar 30, then flat at its close.
/// The hold enters at bar 17's open (signal on bar 16, the first after
/// warmup) with a $10 sizing unit: a 2R cap sits at 3980.
fn cap_series(breach: Bar) -> Vec<Bar> {
    let mut bars: Vec<Bar> = (0..30).map(|i| flat_bar(i * BAR, 4000.0)).collect();
    let after = breach.close;
    bars.push(Bar { time: 30 * BAR, ..breach });
    bars.extend((31..60).map(|i| flat_bar(i * BAR, after)));
    bars
}

#[test]
fn the_open_loss_cap_closes_a_self_managed_hold_at_the_level() {
    // Bar 30 opens above the level and trades down through it: filled at 3980.
    let bars = cap_series(Bar { time: 0, open: 4000.0, high: 4001.0, low: 3975.0, close: 3978.0, volume: None });
    let guards = Guards { max_open_loss_r: 2.0, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &rules(), &guards);
    let trade = &result.trades[0];
    assert_eq!(trade.entry_time, 17 * BAR, "signal on bar 16, filled at bar 17's open");
    assert_eq!(trade.exit_kind, ExitKind::Guard(GuardKind::OpenLoss));
    assert_eq!(trade.exit_reason, "OPEN_LOSS_CAP");
    assert_eq!(trade.exit_time, 30 * BAR, "closed on the breaching bar, not the one after");
    assert_eq!(trade.exit_price, 3980.0, "the level itself");
    assert!((trade.r + 2.0).abs() < 1e-9, "exactly −2R: {}", trade.r);
    assert_eq!(result.closed_by_guard.get("OPEN_LOSS_CAP"), Some(&1));
    assert_eq!(result.metrics.exits.get("OPEN_LOSS_CAP"), Some(&1), "the metrics count it under the same label");
}

#[test]
fn the_open_loss_cap_fills_at_the_open_when_the_bar_gaps_through_it() {
    // Bar 30 opens at 3970, already through 3980: the fill is the open and
    // the loss is more than the cap — as a gap through a stop is.
    let bars = cap_series(Bar { time: 0, open: 3970.0, high: 3972.0, low: 3965.0, close: 3968.0, volume: None });
    let guards = Guards { max_open_loss_r: 2.0, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &rules(), &guards);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::Guard(GuardKind::OpenLoss));
    assert_eq!(trade.exit_time, 30 * BAR);
    assert_eq!(trade.exit_price, 3970.0, "the open, not the level");
    assert!((trade.r + 3.0).abs() < 1e-9, "−3R through a 2R cap: {}", trade.r);
}

#[test]
fn the_open_loss_cap_charges_the_exit_costs() {
    let bars = cap_series(Bar { time: 0, open: 4000.0, high: 4001.0, low: 3975.0, close: 3978.0, volume: None });
    let costed = TradingRules { spread: 0.4, ..rules() };
    let guards = Guards { max_open_loss_r: 2.0, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &costed, &guards);
    let trade = &result.trades[0];
    // Entry paid half the spread (4000.2); the unit is 10.2 from a 3990 stop
    // set off the signal bar's close; the level is 4000.2 − 20.4 = 3979.8,
    // and the exit pays the other half: 3979.6.
    assert_eq!(trade.entry_price, 4000.2);
    assert_eq!(trade.exit_price, 3979.6);
}

#[test]
fn a_cap_of_zero_closes_nothing() {
    let bars = cap_series(Bar { time: 0, open: 3970.0, high: 3972.0, low: 3965.0, close: 3968.0, volume: None });
    let result = guarded(&bars, &hold(), &rules(), &Guards::unbounded());
    assert_eq!(result.trades.len(), 1, "the hold ran to the end of the data");
    assert_eq!(result.trades[0].exit_kind, ExitKind::EndOfData);
    assert!(result.closed_by_guard.is_empty());
}

#[test]
fn the_engine_stop_is_taken_before_a_tighter_cap_when_one_bar_covers_both() {
    // Engine-managed, stop $10 under the signal close (1R by construction),
    // cap at 0.5R = $5. Bar 30 trades through both; the stop is the
    // pessimistic reading and is the one recorded — the documented order.
    let bars = cap_series(Bar { time: 0, open: 4000.0, high: 4001.0, low: 3975.0, close: 3978.0, volume: None });
    let engine = AlwaysLong { exits: Exits::Engine, stop_distance: 10.0, target_distance: Some(100.0) };
    let no_clock = TradingRules { max_hold_ms: 0, ..rules() };
    let guards = Guards { max_open_loss_r: 0.5, ..Guards::unbounded() };
    let result = guarded(&bars, &engine, &no_clock, &guards);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::Stop, "{}", trade.exit_reason);
    assert_eq!(trade.exit_price, 3990.0);
    assert!(result.closed_by_guard.is_empty());

    // With a bar that reaches the cap but not the stop, the cap acts on an
    // engine-managed position as well.
    let shallow = cap_series(Bar { time: 0, open: 4000.0, high: 4001.0, low: 3993.0, close: 3994.0, volume: None });
    let result = guarded(&shallow, &engine, &no_clock, &guards);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_kind, ExitKind::Guard(GuardKind::OpenLoss));
    assert_eq!(trade.exit_price, 3995.0);
}

// ---------------------------------------------------------------- notional cap

#[test]
fn the_notional_cap_reduces_the_lots_to_the_cap() {
    // 100 oz contract at 4000: risk sizing wants $100 / ($10 × 100) = 0.10
    // lots = $40,000 of notional, 400% of a $10,000 account. A 300% cap
    // allows $30,000 = 0.075 lots, rounded down to the step: 0.07.
    let bars: Vec<Bar> = (0..40).map(|i| flat_bar(i * BAR, 4000.0)).collect();
    let big = TradingRules { contract_size: 100.0, ..rules() };
    let plain = run_backtest(&bars, &hold(), &hold().default_params(), &big, None, Range::default());
    assert!((plain.trades[0].lots - 0.10).abs() < 1e-9, "unguarded lots {}", plain.trades[0].lots);

    let guards = Guards { max_notional_pct_equity: 300.0, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &big, &guards);
    assert_eq!(result.trades.len(), 1);
    let expected = ((300.0 / 100.0 * 10_000.0 / (100.0 * 4000.0)) / big.lot_step).floor() * big.lot_step;
    assert!((expected - 0.07).abs() < 1e-9, "the arithmetic in the comment: {expected}");
    assert!((result.trades[0].lots - expected).abs() < 1e-9, "capped lots {}", result.trades[0].lots);
    assert_eq!(result.sized_down_by_guard, 1);
    assert!(result.skipped_by_guard.is_empty(), "sized down is not refused");
}

#[test]
fn the_notional_cap_refuses_an_entry_the_minimum_lot_already_exceeds() {
    // 3% of $10,000 is $300; the minimum lot is $40,000 of notional.
    let bars: Vec<Bar> = (0..40).map(|i| flat_bar(i * BAR, 4000.0)).collect();
    let big = TradingRules { contract_size: 100.0, ..rules() };
    let guards = Guards { max_notional_pct_equity: 3.0, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &big, &guards);
    assert!(result.trades.is_empty(), "{} trades", result.trades.len());
    // Refused on every fill bar after warmup: bars 17..=39.
    assert_eq!(result.skipped_by_guard.get("NOTIONAL_CAP"), Some(&23));
    assert_eq!(result.sized_down_by_guard, 0);
}

// ---------------------------------------------------------------- weekend flat

/// Fifteen-minute bars on New York trading hours 09:00–16:45 (EDT: UTC−4)
/// on Thursday 2026-09-10, Friday 2026-09-11 with an extra 17:00 bar, and
/// Monday 2026-09-14 — no Saturday, no Sunday, like a real feed. Price
/// drifts up a little each bar so the close differs from the open.
fn week_series() -> Vec<Bar> {
    let mut bars = Vec::new();
    let mut px = 4000.0;
    let mut day = |bars: &mut Vec<Bar>, d: u32, last_minute: i64| {
        let start = utc(2026, 9, d, 13, 0); // 09:00 New York
        let mut t = start;
        while t <= start + last_minute * MINUTE {
            bars.push(Bar { time: t, open: px, high: px + 1.5, low: px - 0.5, close: px + 1.0, volume: None });
            px += 1.0;
            t += BAR;
        }
    };
    day(&mut bars, 10, 7 * 60 + 45); // Thursday 09:00 … 16:45
    day(&mut bars, 11, 8 * 60); // Friday 09:00 … 17:00
    day(&mut bars, 14, 7 * 60 + 45); // Monday 09:00 … 16:45
    bars
}

#[test]
fn the_weekend_guard_flattens_on_friday_and_lets_monday_in() {
    let bars = week_series();
    let friday_1645 = utc(2026, 9, 11, 20, 45);
    let friday_1700 = utc(2026, 9, 11, 21, 0);
    let monday_0900 = utc(2026, 9, 14, 13, 0);
    assert!(bars.iter().any(|b| b.time == friday_1700), "the fixture has the Friday 17:00 bar");

    // Control: without the guard the Thursday hold runs through the weekend.
    let plain = run_backtest(&bars, &hold(), &hold().default_params(), &rules(), None, Range::default());
    assert_eq!(plain.trades.len(), 1);
    assert_eq!(plain.trades[0].exit_kind, ExitKind::EndOfData);

    let guards = Guards { flat_before_weekend_hhmm: 1655, ..Guards::unbounded() };
    let result = guarded(&bars, &hold(), &rules(), &guards);
    assert_eq!(result.trades.len(), 2, "{:?}", result.trades.iter().map(|t| (t.entry_time, t.exit_reason.clone())).collect::<Vec<_>>());

    // Opened Thursday; closed on the 16:45 Friday bar — the first whose
    // close instant (17:00) is at or after 16:55 — at that bar's close.
    let thursday = &result.trades[0];
    assert!(thursday.entry_time < utc(2026, 9, 11, 0, 0), "opened on Thursday");
    assert_eq!(thursday.exit_kind, ExitKind::Guard(GuardKind::Weekend));
    assert_eq!(thursday.exit_reason, "WEEKEND_FLAT");
    assert_eq!(thursday.exit_time, friday_1645);
    let closing_bar = bars.iter().find(|b| b.time == friday_1645).unwrap();
    assert_eq!(thursday.exit_price, closing_bar.close, "at the bar's close, not its open");
    assert_eq!(result.closed_by_guard.get("WEEKEND_FLAT"), Some(&1));

    // The 16:45 signal would fill on the 17:00 bar and the 17:00 signal on
    // Monday's open: both refused, because both were produced past the
    // cut-off. Monday's first signal fills on Monday's second bar.
    assert_eq!(result.skipped_by_guard.get("WEEKEND_FLAT"), Some(&2));
    let monday = &result.trades[1];
    assert_eq!(monday.entry_time, monday_0900 + BAR, "Monday's 09:00 signal, filled at 09:15");
    assert_eq!(monday.exit_kind, ExitKind::EndOfData);
}

#[test]
fn the_weekend_guard_at_zero_changes_nothing() {
    let bars = week_series();
    let plain = run_backtest(&bars, &hold(), &hold().default_params(), &rules(), None, Range::default());
    let result = guarded(&bars, &hold(), &rules(), &Guards::unbounded());
    assert_eq!(plain.trades, result.trades);
    assert!(result.skipped_by_guard.is_empty() && result.closed_by_guard.is_empty());
}

// ---------------------------------------------------------------- parity

/// A random walk with a mild trend and a daily rhythm, 15-minute bars on a
/// 24-hour clock from a Sunday evening: enough turns that every exit kind
/// appears, deterministic so the two runs see the same tape.
fn random_walk(n: usize) -> Vec<Bar> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let start = utc(2026, 9, 6, 22, 0);
    let mut px = 3600.0;
    (0..n)
        .map(|i| {
            let open = px;
            let drift = 0.02 + 0.6 * ((i as f64) / 96.0 * std::f64::consts::TAU).sin();
            let close = open + drift + 4.0 * next();
            let high = open.max(close) + 2.0 * next().abs();
            let low = open.min(close) - 2.0 * next().abs();
            px = close;
            Bar { time: start + i as i64 * BAR, open, high, low, close, volume: Some(1.0) }
        })
        .collect()
}

fn same_result(plain: &fd_backtest::BacktestResult, guarded: &fd_backtest::BacktestResult) {
    assert!(plain.trades.len() > 20, "the fixture must trade: {} trades", plain.trades.len());
    assert_eq!(plain.trades, guarded.trades, "trade for trade");
    assert_eq!(plain.equity_curve, guarded.equity_curve);
    assert_eq!(plain.skipped_no_atr, guarded.skipped_no_atr);
    assert_eq!(plain.metrics.exits, guarded.metrics.exits);
    assert!(guarded.skipped_by_guard.is_empty(), "{:?}", guarded.skipped_by_guard);
    assert!(guarded.closed_by_guard.is_empty(), "{:?}", guarded.closed_by_guard);
    assert_eq!(guarded.sized_down_by_guard, 0);
    for (label, a, b) in [
        ("net_pnl_usd", plain.metrics.net_pnl_usd, guarded.metrics.net_pnl_usd),
        ("total_r", plain.metrics.total_r, guarded.metrics.total_r),
        ("max_drawdown_usd", plain.metrics.max_drawdown_usd, guarded.metrics.max_drawdown_usd),
        ("sharpe", plain.metrics.sharpe, guarded.metrics.sharpe),
    ] {
        assert!(fd_core::parity_eq(a, b), "{label}: {a} vs {b}");
    }
}

#[test]
fn every_guard_at_zero_is_the_unguarded_run() {
    let bars = random_walk(4000);
    let realistic = TradingRules { swap_long_per_lot: -1.2, swap_short_per_lot: 0.4, ..TradingRules::default() };
    let off = Guards::unbounded();

    // Engine-managed: stops, targets and the clock all fire on this tape.
    let engine = AlwaysLong { exits: Exits::Engine, stop_distance: 6.0, target_distance: Some(9.0) };
    let plain = run_backtest(&bars, &engine, &engine.default_params(), &realistic, None, Range::default());
    same_result(&plain, &guarded(&bars, &engine, &realistic, &off));
    assert!(plain.metrics.exits.len() >= 3, "stop, target and timeout all present: {:?}", plain.metrics.exits);

    // Self-managed: one hold to the end, or many if the strategy flips —
    // here a strategy that exits every 20 bars so there are trades to compare.
    struct Flip;
    impl Strategy for Flip {
        fn id(&self) -> &'static str {
            "test-flip"
        }
        fn name(&self) -> &'static str {
            "flip"
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
                Some(open) if ctx.bar.time - open.entry_time >= 20 * BAR => Intent::Exit { reason: "twenty bars".into() },
                Some(_) => Intent::None,
                None => {
                    let side = if ctx.i % 40 < 20 { Side::Long } else { Side::Short };
                    let stop = if side.is_long() { ctx.bar.close - 8.0 } else { ctx.bar.close + 8.0 };
                    Intent::Enter { side, stop: Some(stop), target: None, reason: "flip".into() }
                }
            }
        }
    }
    let plain = run_backtest(&bars, &Flip, &Flip.default_params(), &realistic, None, Range::default());
    same_result(&plain, &guarded(&bars, &Flip, &realistic, &off));
}
