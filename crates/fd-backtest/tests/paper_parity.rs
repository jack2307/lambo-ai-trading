//! The paper book is the engine, one bar at a time.
//!
//! `PaperBook::step` is driven bar by bar with the strategy's intents
//! computed the way the engine computes them — indicators on the full
//! series, a `BarContext` per bar with the book's own position view, filters
//! through `Filtered` — and the trade list, the equity curve, the final
//! equity and every counter must be identical to `run_backtest_guarded` on
//! the same bars. An engine-managed method and a self-managed one, each with
//! the guards off and with the configured guards on.
//!
//! What the driver mirrors, so the comparison is exact:
//! * the sizing ATR at `i - 1` (`i` itself only on the first bar, where
//!   nothing is pending anyway), `None` when not finite;
//! * `bar_ms` measured only when the weekend guard is on, else zero;
//! * the strategy is not asked before `warmup` bars;
//! * the position still open after the last bar is closed at that bar's
//!   close as `END_OF_DATA`.

use fd_backtest::engine::{ExitKind, Range, TradingRules, ensure_fallback_atr, run_backtest_guarded, sizing_atr_key};
use fd_backtest::guards::bar_interval_ms;
use fd_backtest::{Guards, PaperBook};
use fd_core::clock::days_from_civil;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_indicators::compute_indicators;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Registry, Strategy};

const MINUTE: i64 = 60_000;
const BAR: i64 = 15 * MINUTE;

fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> i64 {
    days_from_civil(year, month, day) * 86_400_000 + hour * 60 * MINUTE + minute * MINUTE
}

/// A random walk with a mild trend and a daily rhythm, 15-minute bars on a
/// 24-hour clock from a Sunday evening — the fixture `guards_positions.rs`
/// uses, so every exit kind and both calendar guards appear.
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

fn config() -> Config {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

/// Drive the book the way the API's paper run will, over the whole series.
fn paper_run(bars: &[Bar], strategy: &dyn Strategy, params: &Params, rules: &TradingRules, guards: Option<&Guards>) -> PaperBook {
    let mut ind = compute_indicators(bars, &strategy.indicators(params)).unwrap_or_default();
    ensure_fallback_atr(bars, params, rules, &mut ind);
    let atr = ind.get(&sizing_atr_key(params, rules)).cloned().unwrap_or_default();
    let keys = strategy.series(params);
    let resolved: Vec<&[f64]> = keys.iter().map(|k| ind.get(k).map_or(&[][..], |s| &s[..])).collect();
    let warmup = strategy.warmup(params);
    let bar_ms = guards.filter(|g| g.flat_before_weekend_hhmm > 0).map_or(0, |_| bar_interval_ms(bars));

    let mut book = PaperBook::new(rules, strategy.exits() == Exits::Strategy);
    for (i, bar) in bars.iter().enumerate() {
        let atr_prev = match i.checked_sub(1) {
            Some(previous) => atr.get(previous).copied(),
            None => atr.get(i).copied(),
        }
        .filter(|v| v.is_finite());
        book.step(bar, atr_prev, rules, guards, bar_ms, None, |position| {
            if i < warmup {
                return Intent::None;
            }
            let ctx = BarContext { bar, i, bars, ind: &ind, series: &resolved, options: None, position, params };
            strategy.on_bar(&ctx)
        });
    }
    book.close_at_last_close(ExitKind::EndOfData, ExitKind::EndOfData.label(), rules);
    book
}

fn same(strategy: &dyn Strategy, filters: &[&str], guards: Option<&Guards>, rules: &TradingRules) {
    let bars = random_walk(3000);
    let params = strategy.default_params();
    let filters: Vec<Filter> = filters.iter().map(|f| Filter::parse(f).expect("filter")).collect();
    let gated = Filtered { inner: strategy, filters };
    let strategy: &dyn Strategy = if gated.filters.is_empty() { strategy } else { &gated };

    let engine = run_backtest_guarded(&bars, strategy, &params, rules, guards, None, Range::default(), None);
    let book = paper_run(&bars, strategy, &params, rules, guards);

    assert!(engine.trades.len() > 5, "{}: the fixture must trade ({} trades)", strategy.id(), engine.trades.len());
    // Through JSON rather than `==`: a self-managed trade records its stop
    // as NaN, and NaN is never equal to itself. Finite floats print their
    // shortest round-trip form, so equal text is bit-equal numbers.
    let json = |v: &dyn erased::Ser| v.json();
    assert_eq!(json(&book.trades), json(&engine.trades), "{}: trade for trade", strategy.id());
    assert_eq!(book.equity_curve, engine.equity_curve, "{}: the equity curve", strategy.id());
    // The engine's equity is the starting equity plus each trade's PnL in
    // order; the book adds in the same order, so the bits must match.
    let final_equity = engine.trades.iter().fold(rules.starting_equity_usd, |e, t| e + t.pnl_usd);
    assert_eq!(book.equity.to_bits(), final_equity.to_bits(), "{}: equity {} vs {final_equity}", strategy.id(), book.equity);
    assert_eq!(book.skipped_no_atr, engine.skipped_no_atr);
    assert_eq!(book.skipped_by_guard, engine.skipped_by_guard);
    assert_eq!(book.closed_by_guard, engine.closed_by_guard);
    assert_eq!(book.sized_down_by_guard, engine.sized_down_by_guard);
    assert_eq!(json(&book.metrics(rules)), json(&engine.metrics), "{}: metrics", strategy.id());
    assert!(book.position.is_none());
}

mod erased {
    /// `serde::Serialize` as a trait object, for the closure above.
    pub trait Ser {
        fn json(&self) -> String;
    }
    impl<T: serde::Serialize> Ser for T {
        fn json(&self) -> String {
            serde_json::to_string(self).expect("serialise")
        }
    }
}

#[test]
fn an_engine_managed_method_is_the_same_trade_for_trade() {
    let registry = Registry::with_builtins();
    let ema = registry.get("ema-cross").expect("ema-cross");
    let rules = TradingRules { swap_long_per_lot: -1.2, swap_short_per_lot: 0.4, ..TradingRules::default() };
    same(ema, &[], None, &rules);
    let guards = Guards::for_market(&config(), "gold").expect("gold guards");
    same(ema, &[], Some(&guards), &rules);
}

#[test]
fn a_self_managed_method_is_the_same_trade_for_trade() {
    let registry = Registry::with_builtins();
    let hold = registry.get("session-hold").expect("session-hold");
    assert_eq!(hold.exits(), Exits::Strategy);
    let rules = TradingRules::default();
    same(hold, &[], None, &rules);
    let guards = Guards::for_market(&config(), "gold").expect("gold guards");
    same(hold, &[], Some(&guards), &rules);
}

#[test]
fn filters_wrap_the_book_as_they_wrap_the_engine() {
    let registry = Registry::with_builtins();
    let ema = registry.get("ema-cross").expect("ema-cross");
    let rules = TradingRules::default();
    let guards = Guards::for_market(&config(), "gold").expect("gold guards");
    same(ema, &["weekdays", "hours:0300-1600", "flat:1645-1815", "vol:14/100:0.5-99"], None, &rules);
    same(ema, &["weekdays", "hours:0300-1600", "flat:1645-1815", "vol:14/100:0.5-99"], Some(&guards), &rules);
}

#[test]
fn the_guards_bite_in_the_fixture() {
    // The parity above would also hold if the guards never fired; this says
    // they did, so the guarded comparison is not vacuous.
    let registry = Registry::with_builtins();
    let ema = registry.get("ema-cross").expect("ema-cross");
    let rules = TradingRules::default();
    let guards = Guards::for_market(&config(), "gold").expect("gold guards");
    let bars = random_walk(3000);
    let params = ema.default_params();
    let engine = run_backtest_guarded(&bars, ema, &params, &rules, Some(&guards), None, Range::default(), None);
    assert!(
        !engine.skipped_by_guard.is_empty() || !engine.closed_by_guard.is_empty(),
        "no guard fired: skipped {:?}, closed {:?}",
        engine.skipped_by_guard,
        engine.closed_by_guard
    );
}
