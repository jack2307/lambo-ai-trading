//! The paper book: the engine's per-bar machinery, one bar at a time.
//!
//! `run_backtest_guarded` walks a whole series; a paper run receives one
//! closed bar at a time and must do, for that bar, exactly what the engine
//! would have done for it — fill last bar's signal at this open with the
//! guards consulted, manage the open position against this bar's range, ask
//! the strategy. [`PaperBook`] is that loop body over the engine's own
//! functions (`open_position`, `check_exit`, `guard_exit`, `close_position`,
//! `track_excursion`), not a copy of them, so there is one fill model and the
//! parity test (`tests/paper_parity.rs`) can say the paper record and the
//! backtest are the same computation.
//!
//! What is deliberately *not* here: indicators, the strategy call and the
//! range. The caller (the API's paper run) computes the indicators on its
//! window, builds the `BarContext` with the book's [`PaperBook::open`] view,
//! and hands the intent in — because the strategy must see the position
//! **after** this bar's fill and exit, which is why `step` takes a closure
//! rather than a value: an intent computed before the fill would see last
//! bar's position, and the book would drift from the engine on every bar a
//! position closed.
//!
//! The book is plain data (`Serialize`/`Deserialize`), so a run persists it
//! after every bar and a restart reloads it whole, guard memory included.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_strategy::registry::{Intent, OpenPosition};
use serde::{Deserialize, Serialize};

use crate::engine::{
    ExitKind, Live, Metrics, Refused, Trade, TradingRules, apply_costs, check_exit, close_position, metrics_of,
    open_position, round2, track_excursion, trail_stop,
};
use crate::guards::{Exposure, GuardState, Guards, guard_exit};

/// What one bar did to the book.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StepReport {
    /// Trades closed on this bar, in the order they closed (a signal exit at
    /// the open before a stop on the same bar's range, as in the engine).
    pub trades: Vec<Trade>,
    /// The label of the guard that refused the pending entry, if one did.
    pub refused: Option<String>,
    /// The pending entry had no stop and no ATR to size from.
    pub no_risk: bool,
    /// The notional cap cut the lots of the entry that opened.
    pub sized_down: bool,
    /// A position opened on this bar.
    pub opened: bool,
}

/// The engine's state between two bars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperBook {
    pub equity: f64,
    pub position: Option<Live>,
    pub trades: Vec<Trade>,
    /// `(time, equity)` after each closed trade — the engine's curve.
    pub equity_curve: Vec<(i64, f64)>,
    pub skipped_no_atr: usize,
    pub skipped_by_guard: BTreeMap<String, usize>,
    pub closed_by_guard: BTreeMap<String, usize>,
    pub sized_down_by_guard: usize,
    /// The signal from the previous bar, filled at this bar's open.
    pending: Option<Intent>,
    guard_state: GuardState,
    /// The last bar stepped: the signal bar for the calendar guards, and the
    /// price a `stop` closes at.
    last_bar: Option<Bar>,
    self_managed: bool,
}

impl PaperBook {
    /// An empty book at the starting equity.
    #[must_use]
    pub fn new(rules: &TradingRules, self_managed: bool) -> Self {
        Self {
            equity: rules.starting_equity_usd,
            position: None,
            trades: Vec::new(),
            equity_curve: Vec::new(),
            skipped_no_atr: 0,
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
            pending: None,
            guard_state: GuardState::default(),
            last_bar: None,
            self_managed,
        }
    }

    /// The open position as a strategy sees it.
    #[must_use]
    pub fn open(&self) -> Option<OpenPosition> {
        self.position.as_ref().map(Live::view)
    }

    /// The last bar the book was stepped with.
    #[must_use]
    pub fn last_bar(&self) -> Option<&Bar> {
        self.last_bar.as_ref()
    }

    /// The signal waiting to fill at the next bar's open.
    #[must_use]
    pub fn pending(&self) -> Option<&Intent> {
        self.pending.as_ref()
    }

    #[must_use]
    pub fn is_self_managed(&self) -> bool {
        self.self_managed
    }

    /// One closed bar, the engine's way.
    ///
    /// `atr_prev` is the sizing ATR **at the previous bar** — the bar that
    /// produced the pending signal — or `None` when it is not warm; the
    /// engine refuses an entry rather than reading this bar's ATR. `bar_ms`
    /// is the feed's bar interval, read by the weekend guard only. `decide`
    /// is asked once, after the fill and the exits, with the position as it
    /// then stands, and must return `Intent::None` while the strategy is
    /// still warming up (the engine's `i >= warmup`).
    ///
    /// Order: (1) fill the pending intent at this bar's open, guards
    /// consulted; (2) manage the open position against this bar —
    /// `check_exit`, then `guard_exit`, then `track_excursion`; (3) store
    /// what `decide` said as the next bar's pending intent.
    pub fn step(
        &mut self,
        bar: &Bar,
        atr_prev: Option<f64>,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
        decide: impl FnOnce(Option<OpenPosition>) -> Intent,
    ) -> StepReport {
        let report = self.advance(bar, atr_prev, rules, guards, bar_ms);
        self.decide(decide(self.open()));
        report
    }

    /// Steps (1) and (2) of [`PaperBook::step`], without the decision.
    pub fn advance(
        &mut self,
        bar: &Bar,
        atr_prev: Option<f64>,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
    ) -> StepReport {
        let mut report = StepReport::default();

        // 1. Fill whatever the previous bar decided, at this bar's open.
        if let Some(intent) = self.pending.take() {
            match intent {
                Intent::Enter { side, stop, target, reason } if self.position.is_none() => {
                    let atr = atr_prev.filter(|v| v.is_finite());
                    // The guards are asked before any sizing happens. The
                    // signal bar is the previous bar, as in the engine; with
                    // no previous bar there is no calendar check, and no
                    // pending signal either.
                    let refused = guards.and_then(|g| {
                        self.guard_state
                            .refusal(g, bar.time, 0)
                            .or_else(|| self.last_bar.as_ref().and_then(|s| g.calendar_refusal(s, bar, bar_ms)))
                    });
                    if let Some(why) = refused {
                        *self.skipped_by_guard.entry(why.label().to_string()).or_default() += 1;
                        report.refused = Some(why.label().to_string());
                    } else {
                        match open_position(side, stop, target, reason, bar, atr, self.equity, rules, self.self_managed, guards) {
                            Ok((opened, sized_down)) => {
                                self.sized_down_by_guard += usize::from(sized_down);
                                report.sized_down = sized_down;
                                report.opened = true;
                                self.guard_state.opened(opened.entry_time);
                                self.position = Some(opened);
                            }
                            Err(Refused::NoRisk) => {
                                self.skipped_no_atr += 1;
                                report.no_risk = true;
                            }
                            Err(Refused::Guard(why)) => {
                                *self.skipped_by_guard.entry(why.label().to_string()).or_default() += 1;
                                report.refused = Some(why.label().to_string());
                            }
                        }
                    }
                }
                Intent::Exit { reason } if self.position.is_some() => {
                    let open = self.position.take().expect("checked");
                    let exit = apply_costs(bar.open, open.side, false, rules);
                    let trade = self.book(open, exit, bar.time, ExitKind::Signal, &reason, rules);
                    report.trades.push(trade);
                }
                _ => {}
            }
        }

        // 2. Manage an open position against this bar's range: the engine's
        // own stop, target and clock first, then the position guards.
        if let Some(open) = self.position.as_mut() {
            let exit = check_exit(open, bar, rules).or_else(|| {
                let exposure = Exposure { side: open.side, entry_price: open.entry_price, risk: open.risk };
                guards.and_then(|g| guard_exit(&exposure, bar, bar_ms, rules, g))
            });
            if let Some((price, kind)) = exit {
                let open = self.position.take().expect("checked");
                if let ExitKind::Guard(_) = kind {
                    *self.closed_by_guard.entry(kind.label().to_string()).or_default() += 1;
                }
                let trade = self.book(open, price, bar.time, kind, kind.label(), rules);
                report.trades.push(trade);
            } else {
                track_excursion(open, bar);
                // Same order as the backtest, for the same reason: the stop a
                // bar is tested against was fixed by the close before it.
                trail_stop(open, bar, rules);
            }
        }

        self.last_bar = Some(*bar);
        report
    }

    /// Step (3) of [`PaperBook::step`]: what the strategy said on the bar
    /// just advanced, to fill at the next bar's open. `Intent::None` leaves
    /// nothing pending.
    pub fn decide(&mut self, intent: Intent) {
        self.pending = match intent {
            Intent::None => None,
            intent => Some(intent),
        };
    }

    /// Close the open position at the last bar's close, with exit costs, as
    /// `kind`/`reason` — the engine's end-of-data close, or a run's stop.
    /// Drops any pending signal. `None` when the book is flat or has seen no
    /// bar.
    pub fn close_at_last_close(&mut self, kind: ExitKind, reason: &str, rules: &TradingRules) -> Option<Trade> {
        self.pending = None;
        let last = self.last_bar?;
        let open = self.position.take()?;
        let exit = apply_costs(last.close, open.side, false, rules);
        Some(self.book(open, exit, last.time, kind, reason, rules))
    }

    /// A run's stop: the position, if any, is closed at the last close as a
    /// `SIGNAL` exit with the reason `STOPPED`.
    pub fn stop(&mut self, rules: &TradingRules) -> Option<Trade> {
        self.close_at_last_close(ExitKind::Signal, "STOPPED", rules)
    }

    /// The open position marked at the last close, less exit costs, in USD
    /// — what closing now would book before commission and swap.
    #[must_use]
    pub fn unrealised_usd(&self, rules: &TradingRules) -> Option<f64> {
        let open = self.position.as_ref()?;
        let last = self.last_bar?;
        let exit = apply_costs(last.close, open.side, false, rules);
        let points = if open.side.is_long() { exit - open.entry_price } else { open.entry_price - exit };
        Some(round2(points * open.lots * rules.contract_size))
    }

    /// The engine's metrics over the closed trades.
    #[must_use]
    pub fn metrics(&self, rules: &TradingRules) -> Metrics {
        metrics_of(&self.trades, rules.starting_equity_usd)
    }

    /// Record a closed trade exactly as the engine does: equity, the curve,
    /// the guard's memory, the list.
    fn book(&mut self, open: Live, exit_price: f64, exit_time: i64, kind: ExitKind, reason: &str, rules: &TradingRules) -> Trade {
        let entry_reason = open.reason.clone();
        let trade = close_position(open, exit_price, exit_time, kind, reason, rules, &entry_reason);
        self.equity += trade.pnl_usd;
        self.equity_curve.push((exit_time, round2(self.equity)));
        self.guard_state.closed(trade.exit_time, trade.pnl_usd);
        self.trades.push(trade.clone());
        trade
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_strategy::registry::Side;

    fn bar(time: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time, open, high, low, close, volume: None }
    }

    fn rules() -> TradingRules {
        TradingRules { spread: 0.0, commission_per_lot: 0.0, contract_size: 1.0, max_hold_ms: 0, ..TradingRules::default() }
    }

    #[test]
    fn a_signal_fills_at_the_next_open_and_a_stop_closes_it() {
        let rules = rules();
        let mut book = PaperBook::new(&rules, false);
        // Bar 0: the strategy wants long, stop 10 below.
        let r = book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, |p| {
            assert!(p.is_none());
            Intent::Enter { side: Side::Long, stop: Some(90.0), target: Some(120.0), reason: "t".into() }
        });
        assert!(r.trades.is_empty() && !r.opened);
        assert!(book.pending().is_some());
        // Bar 1: fills at the open; the strategy now sees the position.
        let r = book.step(&bar(60_000, 102.0, 103.0, 101.0, 102.0), Some(1.0), &rules, None, 0, |p| {
            assert_eq!(p.map(|p| p.entry_price), Some(102.0));
            Intent::None
        });
        assert!(r.opened && book.position.is_some());
        // Bar 2: the range covers the stop; closed, and the strategy sees flat.
        let r = book.step(&bar(120_000, 95.0, 96.0, 89.0, 91.0), Some(1.0), &rules, None, 0, |p| {
            assert!(p.is_none());
            Intent::None
        });
        assert_eq!(r.trades.len(), 1);
        assert_eq!(r.trades[0].exit_kind, ExitKind::Stop);
        assert_eq!(r.trades[0].exit_price, 90.0);
        assert_eq!(book.trades.len(), 1);
        assert_eq!(book.equity_curve.len(), 1);
        assert!((book.equity - (rules.starting_equity_usd + book.trades[0].pnl_usd)).abs() < 1e-9);
    }

    #[test]
    fn stop_closes_at_the_last_close_as_a_signal_exit() {
        let rules = rules();
        let mut book = PaperBook::new(&rules, true);
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, |_| Intent::Enter {
            side: Side::Short,
            stop: None,
            target: None,
            reason: "t".into(),
        });
        book.step(&bar(60_000, 100.0, 101.0, 99.0, 98.0), Some(2.0), &rules, None, 0, |_| Intent::None);
        assert_eq!(book.unrealised_usd(&rules).map(|v| v > 0.0), Some(true));
        let trade = book.stop(&rules).expect("a position to close");
        assert_eq!((trade.exit_kind, trade.exit_reason.as_str(), trade.exit_price), (ExitKind::Signal, "STOPPED", 98.0));
        assert!(book.position.is_none() && book.pending().is_none());
        assert!(book.stop(&rules).is_none(), "nothing left to close");
    }

    #[test]
    fn the_book_round_trips_through_json() {
        let rules = rules();
        let mut book = PaperBook::new(&rules, false);
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, |_| Intent::Enter {
            side: Side::Long,
            stop: Some(90.0),
            target: None,
            reason: "t".into(),
        });
        book.step(&bar(60_000, 102.0, 103.0, 101.0, 102.0), Some(1.0), &rules, Some(&Guards::unbounded()), 0, |_| Intent::Exit {
            reason: "done".into(),
        });
        let text = serde_json::to_string(&book).expect("serialise");
        let back: PaperBook = serde_json::from_str(&text).expect("deserialise");
        assert_eq!(back, book);
        assert!(back.position.is_some() && back.pending().is_some());
    }
}
