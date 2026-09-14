//! The news guard flattens the book around a scheduled release.
//!
//! Its own test binary: the calendar behind `fd_strategy::news` is installed
//! once per process and every reader shares it, so these tests install one
//! list and no other test in this process may install a different one.

use fd_backtest::engine::{ExitKind, Range, TradingRules, run_backtest, run_backtest_guarded};
use fd_backtest::{GuardKind, Guards};
use fd_core::clock::days_from_civil;
use fd_core::types::{Bar, NewsEvent};
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const BAR: i64 = 15 * MINUTE;

/// Long whenever flat, never exits: a hold only a guard can end.
struct AlwaysLong;

impl Strategy for AlwaysLong {
    fn id(&self) -> &'static str {
        "test-always-long"
    }
    fn name(&self) -> &'static str {
        "always long"
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
        Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "always".into() }
    }
}

fn rules() -> TradingRules {
    TradingRules { contract_size: 1.0, spread: 0.0, commission_per_lot: 0.0, starting_equity_usd: 10_000.0, ..TradingRules::default() }
}

/// Monday 2026-09-14, UTC.
fn at(hour: i64, minute: i64) -> i64 {
    days_from_civil(2026, 9, 14) * 86_400_000 + hour * HOUR + minute * MINUTE
}

/// A high-impact release at 12:30 UTC and a medium one at 18:00.
fn calendar() -> Vec<NewsEvent> {
    vec![
        NewsEvent { time: at(12, 30), impact: 3, currency: "USD".into(), name: "CPI m/m".into() },
        NewsEvent { time: at(18, 0), impact: 2, currency: "EUR".into(), name: "ECB President Speaks".into() },
    ]
}

/// One day of fifteen-minute bars, rising a little so the close differs
/// from the open.
fn day() -> Vec<Bar> {
    (0..96)
        .map(|i| {
            let px = 4000.0 + i as f64;
            Bar { time: at(0, 0) + i as i64 * BAR, open: px, high: px + 1.5, low: px - 0.5, close: px + 1.0, volume: None }
        })
        .collect()
}

fn guarded(bars: &[Bar], guards: &Guards) -> fd_backtest::BacktestResult {
    run_backtest_guarded(bars, &AlwaysLong, &AlwaysLong.default_params(), &rules(), Some(guards), None, Range::default(), None)
}

fn news_guards(min_impact: u8) -> Guards {
    Guards { news_flat_before_min: 60, news_flat_after_min: 30, news_min_impact: min_impact, ..Guards::unbounded() }
}

#[test]
fn the_news_guard_closes_on_the_first_bar_inside_the_window_and_reopens_after_it() {
    fd_strategy::news::install(calendar()).expect("this binary's one calendar");
    let bars = day();

    // Control: unguarded, the hold runs to the end of the day.
    let plain = run_backtest(&bars, &AlwaysLong, &AlwaysLong.default_params(), &rules(), None, Range::default());
    assert_eq!(plain.trades.len(), 1);
    assert_eq!(plain.trades[0].exit_kind, ExitKind::EndOfData);

    let result = guarded(&bars, &news_guards(3));
    assert_eq!(result.trades.len(), 2, "{:?}", result.trades.iter().map(|t| (t.entry_time, t.exit_reason.clone())).collect::<Vec<_>>());

    // The window is [11:30, 13:00). The 11:30 bar is the first inside it and
    // closes the hold at its close.
    let first = &result.trades[0];
    assert_eq!(first.exit_kind, ExitKind::Guard(GuardKind::News));
    assert_eq!(first.exit_reason, "NEWS_FLAT");
    assert_eq!(first.exit_time, at(11, 30));
    let closing_bar = bars.iter().find(|b| b.time == at(11, 30)).unwrap();
    assert_eq!(first.exit_price, closing_bar.close, "at the bar's close");
    assert_eq!(result.closed_by_guard.get("NEWS_FLAT"), Some(&1));

    // Every signal from 11:30 to 12:45 would fill inside the window or was
    // produced inside it: six refusals (fills at 11:45 … 13:00). The 13:00
    // signal — exactly `after` minutes past the release, outside the
    // half-open window — fills at 13:15.
    assert_eq!(result.skipped_by_guard.get("NEWS_FLAT"), Some(&6));
    let second = &result.trades[1];
    assert_eq!(second.entry_time, at(13, 15));
    // The 18:00 release is medium impact: invisible at the default threshold.
    assert_eq!(second.exit_kind, ExitKind::EndOfData);
}

#[test]
fn the_impact_threshold_selects_which_releases_count() {
    fd_strategy::news::install(calendar()).expect("this binary's one calendar");
    let bars = day();
    let result = guarded(&bars, &news_guards(2));
    // High at 12:30 and medium at 18:00 both close a hold now: 11:30, then
    // 17:00 (the first bar at or after 18:00 − 60).
    assert_eq!(result.trades.len(), 3, "{:?}", result.trades.iter().map(|t| (t.entry_time, t.exit_reason.clone())).collect::<Vec<_>>());
    assert_eq!(result.trades[1].exit_kind, ExitKind::Guard(GuardKind::News));
    assert_eq!(result.trades[1].exit_time, at(17, 0));
    assert_eq!(result.trades[2].entry_time, at(18, 45), "the 18:30 signal, first outside [17:00, 18:30)");
    assert_eq!(result.closed_by_guard.get("NEWS_FLAT"), Some(&2));
}

#[test]
fn the_currency_list_scopes_the_guard_to_the_markets_releases() {
    fd_strategy::news::install(calendar()).expect("this binary's one calendar");
    let bars = day();
    // A EUR-only market at impact >= 2: the 12:30 USD release is not its
    // business, the 18:00 EUR one is. One close, at 17:00.
    let eur = Guards { news_currencies: vec!["eur".into()], ..news_guards(2) };
    let result = guarded(&bars, &eur);
    assert_eq!(result.trades.len(), 2, "{:?}", result.trades.iter().map(|t| (t.entry_time, t.exit_reason.clone())).collect::<Vec<_>>());
    assert_eq!(result.trades[0].exit_kind, ExitKind::Guard(GuardKind::News));
    assert_eq!(result.trades[0].exit_time, at(17, 0));
    assert_eq!(result.trades[1].entry_time, at(18, 45));
    assert_eq!(result.closed_by_guard.get("NEWS_FLAT"), Some(&1));
    // A USD market at the same threshold sees only the 12:30 release.
    let usd = Guards { news_currencies: vec!["USD".into()], ..news_guards(2) };
    let result = guarded(&bars, &usd);
    assert_eq!(result.trades.len(), 2);
    assert_eq!(result.trades[0].exit_time, at(11, 30));
    assert_eq!(result.trades[1].exit_kind, ExitKind::EndOfData);
    // A CAD market sees neither: the guarded run is the unguarded one.
    let cad = Guards { news_currencies: vec!["CAD".into()], ..news_guards(2) };
    let plain = run_backtest(&bars, &AlwaysLong, &AlwaysLong.default_params(), &rules(), None, Range::default());
    assert_eq!(guarded(&bars, &cad).trades, plain.trades);
}

#[test]
fn a_zero_window_is_inert_even_with_a_calendar_installed() {
    fd_strategy::news::install(calendar()).expect("this binary's one calendar");
    let bars = day();
    let plain = run_backtest(&bars, &AlwaysLong, &AlwaysLong.default_params(), &rules(), None, Range::default());
    let result = guarded(&bars, &Guards::unbounded());
    assert_eq!(plain.trades, result.trades);
    assert!(result.skipped_by_guard.is_empty() && result.closed_by_guard.is_empty());
}
