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
use fd_strategy::registry::{Intent, OpenPosition, Side};
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
    /// `(size_factor, reason)` when an advisor changed this bar's entry —
    /// zero when it refused it. `None` when no advisor spoke, which is the
    /// same thing as no advisor running.
    pub advice: Option<(f64, String)>,
}

/// How an entry is to be priced: at the next open, or at a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    /// Today's path: fills at the next bar's open (or at the open the poller
    /// posts). Never rests as an order.
    Market,
    /// Rests until the touched side reaches the price: a LONG limit fills when
    /// `ask <= price`, a SHORT when `bid >= price`, at the order price.
    Limit,
    /// Rests until the touched side trades through: a LONG stop fills when
    /// `ask >= price`, a SHORT when `bid <= price`, at the touched side.
    Stop,
}

impl EntryType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
            Self::Stop => "stop",
        }
    }
}

/// An entry waiting for a PRICE rather than for the next open.
///
/// The second thing a book can have pending, beside the market intent in
/// `pending`, and never at the same time as it: placing one replaces the
/// other, so the book still has one committed entry at most. It is filled
/// from the tick feed by [`PaperBook::fill_order`], and only from ticks - a
/// closed bar whose range covers the price is NOT a fill, because a bar says
/// nothing about the order in which its prices were traded and a tick gap is
/// a gap, not a fill (`docs/plans/2026-09-18-staged-ai-entry.md`, stage 1).
///
/// Every price here is on the bar's own axis (the instrument's quote, the
/// same number the bars carry) and BEFORE the half-spread entry cost: the fill
/// path applies that cost exactly as it does to a market open, so a LONG limit
/// at `price` records `entry_price = price + spread / 2`, which is what a
/// market fill at an open of `price` records too. No wall clock lives here;
/// the API keeps `decided_at` beside the run, as it does for the intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingOrder {
    pub entry_type: EntryType,
    /// The level, in the instrument's quote (bar price axis).
    pub price: f64,
    pub side: Side,
    /// Absolute stop and target, the same fields the market intent carries.
    /// The API requires the stop for an order; the engine does not, and
    /// refuses the fill for `NoRisk` exactly as it would a market intent.
    pub stop: Option<f64>,
    pub target: Option<f64>,
    pub reason: String,
    /// Informational: where the decider said the entry is still valid.
    pub zone: Option<(f64, f64)>,
    /// Closed bars the order may wait through after the decision bar; when
    /// this many have arrived without a fill it is cancelled as expired.
    pub valid_bars: u32,
    /// Closed bars that have arrived since the decision bar.
    pub bars_waited: u32,
    /// The bar whose close produced it, the same stamp `pending_id` names.
    pub decided_bar_time: i64,
    /// A tick beyond either cancels the order before it can fill: `ask` above
    /// the first, `bid` below the second - the same sides a stop order reads,
    /// so "beyond" means the market has traded past the level, not merely
    /// quoted across it.
    pub invalidate_above: Option<f64>,
    pub invalidate_below: Option<f64>,
}

impl PendingOrder {
    /// The price this order fills at on a quote, or `None` when the quote has
    /// not reached it. A limit fills AT its price; a stop at the side that
    /// traded through, which is the price a broker would give.
    #[must_use]
    pub fn touched(&self, bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
        let long = self.side.is_long();
        match (self.entry_type, long) {
            (EntryType::Limit, true) => ask.filter(|a| *a <= self.price).map(|_| self.price),
            (EntryType::Limit, false) => bid.filter(|b| *b >= self.price).map(|_| self.price),
            (EntryType::Stop, true) => ask.filter(|a| *a >= self.price),
            (EntryType::Stop, false) => bid.filter(|b| *b <= self.price),
            (EntryType::Market, _) => None,
        }
    }

    /// True when a quote has gone past an invalidation level.
    #[must_use]
    pub fn invalidated_by(&self, bid: Option<f64>, ask: Option<f64>) -> bool {
        let above = self.invalidate_above.zip(ask).is_some_and(|(level, a)| a > level);
        let below = self.invalidate_below.zip(bid).is_some_and(|(level, b)| b < level);
        above || below
    }

    /// The side a fill forced NOW would take: the ask for a LONG, the bid for
    /// a SHORT. `None` when the quote lacks that side.
    #[must_use]
    pub fn trigger_side(&self, bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
        if self.side.is_long() { ask } else { bid }
    }
}

/// The part of the fill bar a tick-filled position has actually lived
/// through, so the bar's close can be managed against THAT and not against
/// prices from before the position existed.
///
/// A LONG limit fills when the ask has come DOWN to it, so every price before
/// the fill was above the level: the bar's high can be above the target
/// without the position ever having been there. A LONG stop fills when the
/// ask has come UP through it, so the bar's low can be under the stop without
/// the position ever having been there. Managing the fill bar against its full
/// range would book a target on the first and a stop-out on the second, each
/// on a price the trade never saw. Tracked from the ticks that follow the fill
/// inside the same bucket; sampled, so a spike between two ticks is missed
/// rather than invented, which is the same rule the fill itself follows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SinceFill {
    /// The bucket of the bar the fill happened inside.
    pub bar_time: i64,
    /// The bar's last price at the fill instant, and the extremes since.
    pub open: f64,
    pub high: f64,
    pub low: f64,
}

/// The engine's state between two bars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperBook {
    pub equity: f64,
    pub position: Option<Live>,
    /// Every closed trade. **Not persisted** — see `equity_curve` below.
    #[serde(default, skip_serializing)]
    pub trades: Vec<Trade>,
    /// `(time, equity)` after each closed trade — the engine's curve.
    ///
    /// Neither this nor `trades` is written into the book's state file, and
    /// that is the whole point. The state file is rewritten on EVERY bar —
    /// ninety-six times a day per book — and these two grow without bound. At
    /// the guards' four trades a day, ten years is ten thousand trades, about
    /// 3.9 MB a book; rewriting that ninety-six times a day across fourteen
    /// books is 5.2 GB of writes a day to record one bar. The cost of a bar
    /// must not depend on how long the book has been running.
    ///
    /// They live in `trades.jsonl` beside the book instead, appended once per
    /// close, and are rebuilt from it at load. Append-only is also the safer
    /// shape for a file written by a process that gets killed: a torn last
    /// line costs one trade, not the history.
    #[serde(default, skip_serializing)]
    pub equity_curve: Vec<(i64, f64)>,
    pub skipped_no_atr: usize,
    pub skipped_by_guard: BTreeMap<String, usize>,
    pub closed_by_guard: BTreeMap<String, usize>,
    pub sized_down_by_guard: usize,
    /// The signal from the previous bar, filled at this bar's open.
    pending: Option<Intent>,
    /// An entry waiting for a price, filled from ticks. Never set while
    /// `pending` is: placing one takes the other. Defaulted so every state
    /// file written before orders existed still loads.
    #[serde(default)]
    pending_order: Option<PendingOrder>,
    /// The fill bar's range since a tick fill, see [`SinceFill`]. `None` on
    /// every market fill and on every backtest path, which is what keeps the
    /// parity goldens the same computation.
    #[serde(default)]
    since_fill: Option<SinceFill>,
    /// Intents an advisor refused outright, and the size it cut from the rest.
    /// Defaulted so a state file written before advisors existed still loads.
    #[serde(default)]
    pub advisor_vetoed: usize,
    #[serde(default)]
    pub advisor_reduced: usize,
    guard_state: GuardState,
    /// The last bar stepped: the signal bar for the calendar guards, and the
    /// price a `stop` closes at.
    last_bar: Option<Bar>,
    self_managed: bool,
}

/// What an advisor panel decided about one pending intent.
///
/// The whole of its power is one number in `[0, 1]`. It may shrink a trade the
/// strategy already decided to take, or refuse it entirely at zero. It cannot
/// choose a side, move a stop, set a price, take a trade the strategy did not
/// ask for, or make one larger — not because the prompt asks it not to, but
/// because there is no field here in which to say any of those things. A model
/// that argues for a long it invented has nowhere to put the argument.
///
/// That asymmetry is deliberate and it is the reason an advisor is safe to
/// have at all: the worst a broken or hostile advisor can do is stop the desk
/// trading, which is the same thing as not running it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Advice {
    /// The intent this verdict belongs to. An advisor answering about an
    /// intent that has already filled or been withdrawn is simply ignored.
    pub intent_id: String,
    /// 1.0 leaves the trade alone; 0.0 refuses it; between, it is cut.
    pub size_factor: f64,
    /// What the panel said, in one line, for the fill's own record.
    pub reason: String,
}

impl Advice {
    /// Clamped on construction, so nothing downstream has to trust the wire.
    #[must_use]
    pub fn new(intent_id: String, size_factor: f64, reason: String) -> Self {
        let size_factor = if size_factor.is_finite() { size_factor.clamp(0.0, 1.0) } else { 1.0 };
        Self { intent_id, size_factor, reason }
    }

    #[must_use]
    pub fn vetoes(&self) -> bool {
        self.size_factor <= 0.0
    }
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
            pending_order: None,
            since_fill: None,
            advisor_vetoed: 0,
            advisor_reduced: 0,
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

    /// The entry waiting for a price, if one is.
    #[must_use]
    pub fn pending_order(&self) -> Option<&PendingOrder> {
        self.pending_order.as_ref()
    }

    /// Rest an order, and hand back whatever it displaced: the order that
    /// was resting, or nothing. A market intent that was pending is dropped
    /// silently, since it was never a record - the API writes the
    /// replacement row for an order, because an order that waited and was
    /// replaced is a decision that did not happen and must be counted.
    pub fn place_order(&mut self, order: PendingOrder) -> Option<PendingOrder> {
        self.pending = None;
        self.pending_order.replace(order)
    }

    /// Take the resting order back without filling it.
    pub fn cancel_order(&mut self) -> Option<PendingOrder> {
        self.pending_order.take()
    }

    /// A closed bar has arrived while an order rests: count it, and take the
    /// order back when it has waited its `valid_bars`. Called by the run after
    /// the bar is stepped - the bar itself never fills an order, see
    /// [`PendingOrder`].
    pub fn order_saw_bar_close(&mut self) -> Option<PendingOrder> {
        let order = self.pending_order.as_mut()?;
        order.bars_waited = order.bars_waited.saturating_add(1);
        if order.bars_waited >= order.valid_bars { self.pending_order.take() } else { None }
    }

    /// A tick inside the fill bar: extend the range the position has lived
    /// through. True when something changed and the book is worth writing.
    pub fn observe_tick(&mut self, bar_time: i64, last: f64) -> bool {
        let Some(range) = self.since_fill.as_mut() else { return false };
        if range.bar_time != bar_time || !last.is_finite() {
            return false;
        }
        let before = *range;
        range.high = range.high.max(last);
        range.low = range.low.min(last);
        *range != before
    }

    /// Fill the resting order at `price`, inside the bar stamped `time`,
    /// through the SAME path a market intent takes.
    ///
    /// Not a second fill model. The order is moved into the `pending` slot
    /// and [`PaperBook::fill_pending`] is called with the order's price where
    /// the market path passes the bar's open - so the half-spread cost, the
    /// ATR-sized risk unit, the guards, the notional cap and the advisor's cut
    /// are the market path's by construction, and the test
    /// `a_limit_fill_and_a_market_fill_at_the_same_price_are_the_same_trade`
    /// says so to the bit. `last` is the bar's last price at the fill
    /// instant, which seeds [`SinceFill`] when the position opens.
    ///
    /// The order is consumed whether it filled or a guard refused it, as a
    /// market intent is: the report says which. `None` when nothing rests, or
    /// when the book already holds a position - an order cannot add to one,
    /// and the API refuses to place one over a position for that reason; this
    /// is the engine's own guard against the same thing.
    // The same eight-plus arguments `fill_pending` takes, for the same
    // reason it takes them.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_order(
        &mut self,
        time: i64,
        price: f64,
        last: f64,
        atr_prev: Option<f64>,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
        advice: Option<&Advice>,
    ) -> Option<(PendingOrder, StepReport)> {
        if !price.is_finite() || price <= 0.0 || self.position.is_some() {
            return None;
        }
        let order = self.pending_order.take()?;
        self.pending = Some(Intent::Enter {
            side: order.side,
            stop: order.stop,
            target: order.target,
            reason: order.reason.clone(),
        });
        let report = self.fill_pending(time, price, atr_prev, rules, guards, bar_ms, advice);
        if report.opened {
            let last = if last.is_finite() { last } else { price };
            self.since_fill = Some(SinceFill { bar_time: time, open: last, high: last, low: last });
        }
        Some((order, report))
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
        advice: Option<&Advice>,
        decide: impl FnOnce(Option<OpenPosition>) -> Intent,
    ) -> StepReport {
        let report = self.advance(bar, atr_prev, rules, guards, bar_ms, advice);
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
        advice: Option<&Advice>,
    ) -> StepReport {
        // 1. Fill whatever the previous bar decided, at this bar's open —
        //    unless [`PaperBook::fill_open`] already did it when this bar
        //    opened, in which case `pending` is already `None` and this is a
        //    no-op. That is the whole of the idempotency: there is no flag to
        //    keep in step, because the thing being consumed is the evidence.
        let mut report = self.fill_pending(bar.time, bar.open, atr_prev, rules, guards, bar_ms, advice);

        // 2. Manage an open position against this bar's range: the engine's
        // own stop, target and clock first, then the position guards.
        //
        // This runs whether the fill happened here or at the open, and it must:
        // a position opened at this bar's open can be stopped out on this same
        // bar's low, exactly as it could when both halves were one call.
        //
        // A position filled from a TICK inside this bar is the one exception,
        // and only on this bar: it is managed against the range since the
        // fill, not the bar's whole range - see `SinceFill` for the two false
        // exits the whole range would book. `since_fill` is `None` on every
        // other path, so `managed` is the bar itself and nothing here changes
        // for a backtest or a market fill.
        let managed = match self.since_fill.take() {
            Some(range) if range.bar_time == bar.time => Some(Bar {
                time: bar.time,
                open: range.open,
                // The close came after the fill by definition, so it is
                // always part of the lived range; the extremes are clamped
                // to the bar's own, which they cannot honestly exceed.
                high: range.high.max(bar.close).min(bar.high),
                low: range.low.min(bar.close).max(bar.low),
                close: bar.close,
                volume: bar.volume,
            }),
            // A bar from BEFORE the fill's bucket, arriving late: the position
            // did not exist during it, so there is nothing to manage it
            // against. Kept for the bar that is its own.
            Some(range) if range.bar_time > bar.time => {
                self.since_fill = Some(range);
                None
            }
            _ => Some(*bar),
        };
        if let (Some(open), Some(bar)) = (self.position.as_mut(), managed.as_ref()) {
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

    /// Fill the pending intent at the open of the bar that has just STARTED,
    /// before it closes, and report what happened — or `None` when this is not
    /// that bar.
    ///
    /// Why this exists. A paper book learns about a bar when the bar CLOSES,
    /// because that is when the poller can post it, so an intent decided at
    /// the close of bar `t` was filled at the open of bar `t+1` and the book
    /// did not know it held the position until `t+1` closed — a bar later than
    /// the price it holds it at. Measured on the VPS 2026-09-17, six live
    /// positions out of six surfaced to the executor exactly one bar after the
    /// book's own stamp. See `docs/decisions/2026-09-17-entry-lag.md`.
    ///
    /// The PRICE is unchanged and that is the point: this fills at
    /// `apply_costs(open)` exactly as [`PaperBook::advance`] does, so a
    /// backtest replaying the same tape produces the same trades to the byte.
    /// What moves is the wall clock at which the book, and therefore the
    /// mirror, finds out.
    ///
    /// ORDERING. Valid only for a bar strictly newer than the last one
    /// advanced over, which is the same rule the closed-bar path already
    /// applies (`PaperRun::accept` refuses a bar at or before its last). Not
    /// "exactly one bar later": the broker's tape has a weekend and an hour a
    /// day with no bars at all, so requiring adjacency would refuse the first
    /// open of every session and hand the lag straight back on the entry taken
    /// into the thinnest book of the day. The strict-newer test still refuses
    /// the case that matters — an open replayed after its own bar has closed,
    /// which would otherwise fill the NEXT bar's decision at the previous
    /// bar's price.
    ///
    /// It does not manage the position and does not set `last_bar`: there is
    /// no range to manage against yet, and the bar has not happened. Both are
    /// [`PaperBook::advance`]'s when the bar closes.
    ///
    /// Guards and the advisor are asked HERE rather than at the close, and
    /// they answer the same: both calendar tests read only the two bar times
    /// (`Guards::calendar_refusal`), the session guard reads only `time`, and
    /// the advisor's verdict was posted before either. Asked earlier, same
    /// answer.
    // Eight arguments, inherited verbatim from the call this was extracted
    // from: the same allow `open_position` carries, for the same reason.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_open(
        &mut self,
        time: i64,
        open: f64,
        atr_prev: Option<f64>,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
        advice: Option<&Advice>,
    ) -> Option<StepReport> {
        if !open.is_finite() || open <= 0.0 {
            return None;
        }
        // No bar advanced over yet means nothing can be pending, and there is
        // no `signal` bar for the calendar check to read.
        if self.last_bar.is_none_or(|last| time <= last.time) {
            return None;
        }
        Some(self.fill_pending(time, open, atr_prev, rules, guards, bar_ms, advice))
    }

    /// Step (1) of [`PaperBook::advance`], on its own: the pending intent
    /// against a bar's time and open, which are the only two things filling
    /// one has ever needed.
    ///
    /// Deliberately takes no `&Bar`. At the moment a live book can first act
    /// on a bar, its high, low and close do not exist; a `Bar` here would be
    /// three invented numbers travelling with two real ones through the code
    /// path that opens a position against a funded account.
    // Eight arguments, inherited verbatim from the call this was extracted
    // from: the same allow `open_position` carries, for the same reason.
    #[allow(clippy::too_many_arguments)]
    fn fill_pending(
        &mut self,
        time: i64,
        open: f64,
        atr_prev: Option<f64>,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
        advice: Option<&Advice>,
    ) -> StepReport {
        let mut report = StepReport::default();
        let Some(intent) = self.pending.take() else { return report };
        match intent {
            Intent::Enter { side, stop, target, reason } if self.position.is_none() => {
                let atr = atr_prev.filter(|v| v.is_finite());
                // The advisor is asked before the guards, and it is the
                // only one of the two that can be absent: with no advice
                // the trade is exactly the trade the strategy decided,
                // which is the default a broken advisor must fall back to.
                let cut = advice.map_or(1.0, |a| a.size_factor);
                // The guards are asked before any sizing happens. The
                // signal bar is the previous bar, as in the engine; with
                // no previous bar there is no calendar check, and no
                // pending signal either.
                let refused = guards.and_then(|g| {
                    self.guard_state
                        .refusal(g, time, 0)
                        .or_else(|| self.last_bar.as_ref().and_then(|s| g.calendar_refusal(s.time, time, bar_ms)))
                });
                if cut <= 0.0 {
                    // A veto is recorded as its own refusal rather than as
                    // a guard: a reader must be able to tell a rule the
                    // desk wrote from an opinion a model had.
                    self.advisor_vetoed += 1;
                    report.refused = Some("ADVISOR".to_string());
                    report.advice = advice.map(|a| (0.0, a.reason.clone()));
                } else if let Some(why) = refused {
                    *self.skipped_by_guard.entry(why.label().to_string()).or_default() += 1;
                    report.refused = Some(why.label().to_string());
                } else {
                    match open_position(side, stop, target, reason, time, open, atr, self.equity, rules, self.self_managed, guards) {
                        Ok((mut opened, sized_down)) => {
                            // The cut is applied to the lots and to nothing
                            // else: risk is a price distance, so R, the
                            // stop and the target are untouched and a
                            // half-size trade is the same trade at half the
                            // money. Rounded DOWN to the venue's step, and
                            // a cut that lands under the minimum lot is a
                            // veto rather than a trade the broker refuses.
                            //
                            // That veto used to `return` from `advance`,
                            // setting `last_bar` on the way out and skipping
                            // the position management below. It cannot return
                            // from here, and it does not need to: the branch
                            // is inside `self.position.is_none()` and sets no
                            // position, so the management step it skipped was
                            // a no-op every time it ran.
                            let mut vetoed = false;
                            if cut < 1.0 {
                                let scaled = ((opened.lots * cut) / rules.lot_step).floor() * rules.lot_step;
                                if scaled < rules.min_lot {
                                    self.advisor_vetoed += 1;
                                    report.refused = Some("ADVISOR".to_string());
                                    report.advice = advice.map(|a| (0.0, a.reason.clone()));
                                    vetoed = true;
                                } else {
                                    opened.lots = scaled;
                                    self.advisor_reduced += 1;
                                    report.advice = advice.map(|a| (a.size_factor, a.reason.clone()));
                                }
                            }
                            if !vetoed {
                                self.sized_down_by_guard += usize::from(sized_down);
                                report.sized_down = sized_down;
                                report.opened = true;
                                self.guard_state.opened(opened.entry_time);
                                self.position = Some(opened);
                            }
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
                let open_position = self.position.take().expect("checked");
                let exit = apply_costs(open, open_position.side, false, rules);
                let trade = self.book(open_position, exit, time, ExitKind::Signal, &reason, rules);
                report.trades.push(trade);
            }
            _ => {}
        }
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
        // A resting order goes the same way. The API takes it first with
        // `cancel_order` so the cancellation is a row; this is the engine's
        // own guarantee that a stopped book rests nothing.
        self.pending_order = None;
        self.since_fill = None;
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
        // The position's own sizing basis, for the reason `close_position`
        // gives: marking `lots` against a contract size it was not sized
        // under is the same defect one bar earlier.
        Some(round2(points * open.lots * open.contract_size.unwrap_or(rules.contract_size)))
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
        let r = book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, None, |p| {
            assert!(p.is_none());
            Intent::Enter { side: Side::Long, stop: Some(90.0), target: Some(120.0), reason: "t".into() }
        });
        assert!(r.trades.is_empty() && !r.opened);
        assert!(book.pending().is_some());
        // Bar 1: fills at the open; the strategy now sees the position.
        let r = book.step(&bar(60_000, 102.0, 103.0, 101.0, 102.0), Some(1.0), &rules, None, 0, None, |p| {
            assert_eq!(p.map(|p| p.entry_price), Some(102.0));
            Intent::None
        });
        assert!(r.opened && book.position.is_some());
        // Bar 2: the range covers the stop; closed, and the strategy sees flat.
        let r = book.step(&bar(120_000, 95.0, 96.0, 89.0, 91.0), Some(1.0), &rules, None, 0, None, |p| {
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
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, None, |_| Intent::Enter {
            side: Side::Short,
            stop: None,
            target: None,
            reason: "t".into(),
        });
        book.step(&bar(60_000, 100.0, 101.0, 99.0, 98.0), Some(2.0), &rules, None, 0, None, |_| Intent::None);
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
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, None, |_| Intent::Enter {
            side: Side::Long,
            stop: Some(90.0),
            target: None,
            reason: "t".into(),
        });
        book.step(&bar(60_000, 102.0, 103.0, 101.0, 102.0), Some(1.0), &rules, Some(&Guards::unbounded()), 0, None, |_| Intent::Exit {
            reason: "done".into(),
        });
        let text = serde_json::to_string(&book).expect("serialise");
        let back: PaperBook = serde_json::from_str(&text).expect("deserialise");
        assert_eq!(back, book);
        assert!(back.position.is_some() && back.pending().is_some());
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use fd_strategy::registry::Side;

    fn bar(time: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time, open, high, low, close, volume: None }
    }

    fn rules() -> TradingRules {
        TradingRules { spread: 0.4, commission_per_lot: 0.0, contract_size: 1.0, max_hold_ms: 0, ..TradingRules::default() }
    }

    fn order(entry_type: EntryType, side: Side, price: f64) -> PendingOrder {
        let (stop, target) = if side.is_long() { (price - 10.0, price + 20.0) } else { (price + 10.0, price - 20.0) };
        PendingOrder {
            entry_type,
            price,
            side,
            stop: Some(stop),
            target: Some(target),
            reason: "t".into(),
            zone: None,
            valid_bars: 2,
            bars_waited: 0,
            decided_bar_time: 0,
            invalidate_above: None,
            invalidate_below: None,
        }
    }

    /// A book that has advanced over bar 0 and rests nothing.
    fn warm() -> (TradingRules, PaperBook) {
        let rules = rules();
        let mut book = PaperBook::new(&rules, false);
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, None, |_| Intent::None);
        (rules, book)
    }

    #[test]
    fn a_limit_fill_and_a_market_fill_at_the_same_price_are_the_same_trade() {
        // The contract's own test: the order path is not a second fill
        // model. Same price in, same lots, stop, target, risk and entry
        // price out - to the bit, not to a tolerance, for the reason
        // `the_open_fills_at_the_same_price_the_close_would_have` gives.
        let (rules, mut market) = warm();
        market.decide(Intent::Enter { side: Side::Long, stop: Some(92.0), target: Some(122.0), reason: "t".into() });
        let r = market.fill_open(60_000, 102.0, Some(3.0), &rules, None, 0, None).expect("that bar is next");
        assert!(r.opened);

        let (_, mut limit) = warm();
        let mut o = order(EntryType::Limit, Side::Long, 102.0);
        o.stop = Some(92.0);
        o.target = Some(122.0);
        limit.place_order(o);
        // The ask has come down to the level.
        let (_, r) = limit.fill_order(60_000, 102.0, 101.8, Some(3.0), &rules, None, 0, None).expect("rests");
        assert!(r.opened);

        let a = market.position.as_ref().expect("market filled");
        let b = limit.position.as_ref().expect("limit filled");
        assert_eq!((a.lots, a.stop, a.target, a.risk, a.entry_price, a.entry_time), (b.lots, b.stop, b.target, b.risk, b.entry_price, b.entry_time));
        // And the price is the order price plus the same half spread a
        // market open pays, which is the unit `PendingOrder::price` states.
        assert_eq!(b.entry_price, 102.0 + rules.spread / 2.0);
        assert!(limit.pending_order().is_none() && limit.pending().is_none(), "consumed");
    }

    #[test]
    fn the_touched_side_is_the_fill_rule_and_a_stop_fills_at_the_side_that_traded_through() {
        let long_limit = order(EntryType::Limit, Side::Long, 100.0);
        assert_eq!(long_limit.touched(Some(99.5), Some(100.2)), None, "ask still above the level");
        assert_eq!(long_limit.touched(Some(99.5), Some(100.0)), Some(100.0), "at the level, at the order price");
        assert_eq!(long_limit.touched(Some(99.0), Some(99.4)), Some(100.0), "through it is still the order price");
        assert_eq!(long_limit.touched(Some(99.0), None), None, "no ask, no fill");

        let short_limit = order(EntryType::Limit, Side::Short, 100.0);
        assert_eq!(short_limit.touched(Some(99.9), Some(100.3)), None);
        assert_eq!(short_limit.touched(Some(100.0), Some(100.3)), Some(100.0));

        let long_stop = order(EntryType::Stop, Side::Long, 100.0);
        assert_eq!(long_stop.touched(Some(99.5), Some(99.9)), None);
        assert_eq!(long_stop.touched(Some(100.1), Some(100.4)), Some(100.4), "the ask that traded through");

        let short_stop = order(EntryType::Stop, Side::Short, 100.0);
        assert_eq!(short_stop.touched(Some(100.1), Some(100.4)), None);
        assert_eq!(short_stop.touched(Some(99.7), Some(100.0)), Some(99.7), "the bid that traded through");

        // Invalidation reads the same sides a stop reads, strictly beyond.
        let mut guarded = order(EntryType::Limit, Side::Long, 100.0);
        guarded.invalidate_above = Some(105.0);
        guarded.invalidate_below = Some(95.0);
        assert!(!guarded.invalidated_by(Some(104.8), Some(105.0)), "at the level is not beyond it");
        assert!(guarded.invalidated_by(Some(104.8), Some(105.1)));
        assert!(guarded.invalidated_by(Some(94.9), Some(95.2)));
        assert!(!guarded.invalidated_by(None, None), "no quote, no verdict");
    }

    #[test]
    fn an_order_expires_after_its_valid_bars_and_a_closed_bar_never_fills_it() {
        let (rules, mut book) = warm();
        book.place_order(order(EntryType::Limit, Side::Long, 95.0));
        // A bar whose range covers the level, twice. Not a fill: a bar does
        // not say in which order its prices traded.
        book.step(&bar(60_000, 100.0, 101.0, 90.0, 100.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        assert!(book.position.is_none());
        assert!(book.order_saw_bar_close().is_none(), "one bar waited of two");
        assert_eq!(book.pending_order().map(|o| o.bars_waited), Some(1));
        book.step(&bar(120_000, 100.0, 101.0, 90.0, 100.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        let expired = book.order_saw_bar_close().expect("the second bar expires it");
        assert_eq!(expired.bars_waited, 2);
        assert!(book.pending_order().is_none() && book.position.is_none());
    }

    #[test]
    fn placing_an_order_replaces_the_one_resting_and_drops_a_market_intent() {
        let (_, mut book) = warm();
        book.decide(Intent::Enter { side: Side::Short, stop: Some(110.0), target: None, reason: "market".into() });
        assert!(book.place_order(order(EntryType::Stop, Side::Long, 103.0)).is_none(), "nothing rested before");
        assert!(book.pending().is_none(), "one committed entry at most");
        let replaced = book.place_order(order(EntryType::Limit, Side::Long, 98.0)).expect("the stop order");
        assert_eq!((replaced.entry_type, replaced.price), (EntryType::Stop, 103.0));
        assert_eq!(book.pending_order().map(|o| o.price), Some(98.0));
        assert_eq!(book.cancel_order().map(|o| o.price), Some(98.0));
        assert!(book.pending_order().is_none());
    }

    #[test]
    fn the_fill_bar_is_managed_from_the_fill_and_not_from_its_open() {
        // LONG limit at 100, target 120. The bar opened at 125, fell to 100
        // (the fill), and closed at 101. The whole bar's high of 125 is above
        // the target; the position never saw it. Books that manage the fill
        // bar against its full range record a winner here.
        let (rules, mut book) = warm();
        book.place_order(order(EntryType::Limit, Side::Long, 100.0));
        let (_, r) = book.fill_order(60_000, 100.0, 99.9, Some(1.0), &rules, None, 0, None).expect("rests");
        assert!(r.opened);
        assert!(book.observe_tick(60_000, 101.5), "a new high since the fill");
        assert!(!book.observe_tick(60_000, 100.5), "inside the range: nothing to write");
        assert!(!book.observe_tick(120_000, 130.0), "another bucket is not this bar");
        let r = book.step(&bar(60_000, 125.0, 125.0, 99.8, 101.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        assert!(r.trades.is_empty(), "no target on a price from before the fill: {r:?}");
        let open = book.position.as_ref().expect("still open");
        assert!((open.mfe - (101.5 - open.entry_price)).abs() < 1e-9, "excursion measured from the fill: {}", open.mfe);

        // The next bar is managed whole, as every bar is.
        let r = book.step(&bar(120_000, 101.0, 121.0, 100.5, 118.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        assert_eq!(r.trades.len(), 1);
        assert_eq!(r.trades[0].exit_kind, ExitKind::Target);
    }

    #[test]
    fn a_stop_order_fill_bar_does_not_stop_out_on_prices_from_before_the_fill() {
        // LONG stop at 100, stop-loss 90. The bar opened at 85 and rose
        // through 100 (the fill), closing at 102. The bar's low of 85 is under
        // the stop-loss; the position never saw it.
        let (rules, mut book) = warm();
        book.place_order(order(EntryType::Stop, Side::Long, 100.0));
        let (_, r) = book.fill_order(60_000, 100.3, 100.1, Some(1.0), &rules, None, 0, None).expect("rests");
        assert!(r.opened);
        let r = book.step(&bar(60_000, 85.0, 102.5, 85.0, 102.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        assert!(r.trades.is_empty(), "no stop-out on a price from before the fill: {r:?}");
        assert!(book.position.is_some());
    }

    #[test]
    fn an_order_cannot_fill_over_a_position_and_a_stopped_book_rests_nothing() {
        let (rules, mut book) = warm();
        book.decide(Intent::Enter { side: Side::Long, stop: Some(90.0), target: None, reason: "t".into() });
        book.step(&bar(60_000, 100.0, 101.0, 99.0, 100.0), Some(1.0), &rules, None, 0, None, |_| Intent::None);
        assert!(book.position.is_some());
        book.place_order(order(EntryType::Limit, Side::Long, 98.0));
        assert!(book.fill_order(120_000, 98.0, 98.0, Some(1.0), &rules, None, 0, None).is_none(), "a position is held");
        assert!(book.pending_order().is_some(), "and the order is left where it was for the caller to cancel");
        book.stop(&rules);
        assert!(book.pending_order().is_none() && book.position.is_none());
    }

    #[test]
    fn a_book_with_an_order_round_trips_and_an_old_state_file_loads_without_one() {
        let (_, mut book) = warm();
        let mut o = order(EntryType::Limit, Side::Short, 104.0);
        o.zone = Some((103.5, 104.5));
        o.invalidate_below = Some(101.0);
        book.place_order(o);
        let text = serde_json::to_string(&book).expect("serialise");
        assert!(text.contains("\"entry_type\":\"limit\""), "the type is spelled as the wire spells it: {text}");
        let back: PaperBook = serde_json::from_str(&text).expect("deserialise");
        assert_eq!(back, book);

        // A state file from before orders existed carries neither field.
        let mut v: serde_json::Value = serde_json::from_str(&text).expect("json");
        v.as_object_mut().expect("object").remove("pending_order");
        v.as_object_mut().expect("object").remove("since_fill");
        let old: PaperBook = serde_json::from_value(v).expect("an old state file still loads");
        assert!(old.pending_order().is_none());
    }
}

#[cfg(test)]
mod advice_tests {
    use super::*;
    use fd_strategy::registry::Side;

    fn bar(time: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time, open, high, low, close, volume: None }
    }

    fn rules() -> TradingRules {
        let mut rules = TradingRules::default();
        rules.spread = 0.0;
        rules.commission_per_lot = 0.0;
        rules.contract_size = 1.0;
        rules.lot_step = 0.01;
        rules.min_lot = 0.01;
        rules
    }

    /// Decide on bar 0, fill on bar 1, with whatever advice is handed in.
    fn run_with(advice: Option<Advice>) -> PaperBook {
        let rules = rules();
        let mut book = PaperBook::new(&rules, false);
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, None, |_| Intent::Enter {
            side: Side::Long,
            stop: Some(90.0),
            target: Some(130.0),
            reason: "t".into(),
        });
        book.step(&bar(60_000, 100.0, 101.0, 99.0, 100.0), Some(5.0), &rules, None, 0, advice.as_ref(), |_| {
            Intent::None
        });
        book
    }

    #[test]
    fn no_advice_is_the_trade_the_strategy_asked_for() {
        let book = run_with(None);
        let open = book.position.as_ref().expect("the entry filled");
        assert_eq!(open.side, Side::Long);
        assert_eq!(book.advisor_vetoed, 0);
        assert_eq!(book.advisor_reduced, 0);
    }

    #[test]
    fn a_veto_refuses_the_entry_and_says_who_refused_it() {
        let advice = Advice::new("r:0".into(), 0.0, "news in nine minutes".into());
        let book = run_with(Some(advice));
        assert!(book.position.is_none(), "nothing opened");
        assert_eq!(book.advisor_vetoed, 1);
        // Not counted as a guard: a reader must be able to tell a rule the desk
        // wrote from an opinion a model had.
        assert!(book.skipped_by_guard.is_empty());
    }

    #[test]
    fn a_cut_halves_the_lots_and_touches_nothing_else() {
        let full = run_with(None);
        let full_lots = full.position.as_ref().expect("filled").lots;

        let half = run_with(Some(Advice::new("r:0".into(), 0.5, "thin hour".into())));
        let open = half.position.as_ref().expect("filled");
        assert!(
            (open.lots - (full_lots * 0.5 / 0.01).floor() * 0.01).abs() < 1e-9,
            "lots {} against half of {full_lots}",
            open.lots
        );
        // Risk is a price distance, so R, the stop and the target are the
        // strategy's own numbers and a half-size trade is the same trade at
        // half the money.
        assert_eq!(open.stop, full.position.as_ref().and_then(|o| o.stop));
        assert_eq!(open.target, full.position.as_ref().and_then(|o| o.target));
        assert!((open.risk - full.position.as_ref().expect("filled").risk).abs() < 1e-9);
        assert_eq!(half.advisor_reduced, 1);
        assert_eq!(half.advisor_vetoed, 0);
    }

    #[test]
    fn a_cut_below_the_minimum_lot_is_a_veto_and_not_a_trade_the_broker_would_bounce() {
        let book = run_with(Some(Advice::new("r:0".into(), 0.000_01, "as good as no".into())));
        assert!(book.position.is_none());
        assert_eq!(book.advisor_vetoed, 1);
        assert_eq!(book.advisor_reduced, 0);
    }

    /// The asymmetry the whole design rests on.
    #[test]
    fn an_advisor_can_never_make_a_position_larger() {
        let full = run_with(None).position.as_ref().expect("filled").lots;
        for greedy in [1.5, 10.0, f64::INFINITY, f64::NAN] {
            let book = run_with(Some(Advice::new("r:0".into(), greedy, "more".into())));
            let lots = book.position.as_ref().expect("filled").lots;
            assert!((lots - full).abs() < 1e-9, "size_factor {greedy} changed {full} to {lots}");
            assert_eq!(book.advisor_reduced, 0);
        }
    }

    #[test]
    fn an_advisor_cannot_open_a_trade_the_strategy_did_not_ask_for() {
        let rules = rules();
        let mut book = PaperBook::new(&rules, false);
        // The strategy says nothing at all; the advisor is as loud as it likes.
        let advice = Advice::new("r:0".into(), 1.0, "I like this one".into());
        book.step(&bar(0, 100.0, 101.0, 99.0, 100.0), None, &rules, None, 0, Some(&advice), |_| Intent::None);
        book.step(&bar(60_000, 100.0, 101.0, 99.0, 100.0), Some(5.0), &rules, None, 0, Some(&advice), |_| {
            Intent::None
        });
        assert!(book.position.is_none(), "there is no field in which to ask for a trade");
        assert!(book.trades.is_empty());
    }

    #[test]
    fn a_verdict_is_clamped_on_construction_so_nothing_downstream_trusts_the_wire() {
        assert_eq!(Advice::new("x".into(), 2.0, String::new()).size_factor, 1.0);
        assert_eq!(Advice::new("x".into(), -1.0, String::new()).size_factor, 0.0);
        assert_eq!(Advice::new("x".into(), f64::NAN, String::new()).size_factor, 1.0);
        assert!(Advice::new("x".into(), 0.0, String::new()).vetoes());
        assert!(!Advice::new("x".into(), 0.01, String::new()).vetoes());
    }
}
