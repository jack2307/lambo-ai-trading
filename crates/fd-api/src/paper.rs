//! The paper loop's runs: strategies on closed bars, the books on disk.
//!
//! `docs/paper/DESIGN.md`. A run is one `market:tf`, one registered
//! strategy with its filters, the configured guards, and a
//! [`PaperBook`] — the engine's own fill model one bar at a time. Nothing
//! here can send an order; the only thing that executes is the JSON file
//! under `<data>/paper/<id>/`.
//!
//! Runs are keyed by an id, not by `market:tf`: the owner runs several
//! demo accounts on the same bar stream, each with its own strategy, and
//! one posted bar feeds every run whose `market:tf` matches, in id order.
//! The id defaults to `<market>-<tf>-<strategy>`; the first layout (one
//! run per `market:tf`, directory `<market>-<tf>`, no `id` in the state
//! file) reloads under that directory's name — see [`PaperConfig::id`].
//!
//! Per accepted bar: the window (the last `window` bars) gains the bar,
//! the strategy's indicators are recomputed on the window exactly as the
//! engine computes them on a series (`compute_indicators` plus the sizing
//! ATR fallback), the `BarContext` for the last bar is built with the
//! book's position view, and `PaperBook::step` fills last bar's signal,
//! manages the position and stores this bar's intent. Then the state is
//! written, so a restart reloads the book and the window as they were.
//!
//! The Desk also shows the bar that is still forming, posted to
//! `POST /api/paper/tick` by the same poller and kept in
//! `AppState::live_bars` — never in a run, never on disk, and never read by
//! anything in this module's decision path. It exists because a screen whose
//! newest number is fifteen minutes old reads as broken; see [`tick`].
//!
//! One thing to know when reading a paper record against a backtest: the
//! indicators are computed on the window, not on the whole history, so an
//! indicator with memory (an EMA) differs from the backtest's by however
//! much it has not converged in `window` bars. Six hundred bars is more
//! than ten times the slowest default EMA; the residue is well below a
//! tick. The fills, given the same intents, are the engine's exactly
//! (`fd-backtest/tests/paper_parity.rs`).

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as PathParam, Query, State};
use fd_backtest::engine::{ensure_fallback_atr, sizing_atr_key};
use fd_backtest::paper::{Advice, EntryType, PendingOrder};
use fd_backtest::{Guards, PaperBook, StepReport, Trade, TradingRules};
use fd_core::types::Bar;
use fd_indicators::{IndicatorSpec, compute_indicators};
use fd_store::timeframe_ms;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Registry, Strategy, Side};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::{Point, TradeDto};
use crate::error::ApiError;
use crate::state::{AppState, TIMEFRAMES};

/// Bars kept when a start does not say.
pub const DEFAULT_WINDOW: usize = 600;

/// The longest run id accepted.
pub const MAX_ID_LEN: usize = 40;

/// How old a forming bar may be and still be shown as the current price.
///
/// Ninety seconds is a comfortable multiple of the poller's five-second
/// cycle, so one missed read does not blank the screen; past it the poller
/// is gone or the market is shut. A stale tick drawn as "now" is worse than
/// no price at all — it is the exact lie this endpoint exists to stop.
pub const MAX_LIVE_AGE_MS: i64 = 90_000;

/// What a run was started with. `params` are the full parameters after the
/// overrides, so a reload needs no registry lookup to know what ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperConfig {
    /// The run's id, the key everything else hangs off. Empty in a state
    /// file written before ids existed; [`PaperConfig::id`] derives the
    /// old key then, so that file reloads where it was.
    #[serde(default)]
    pub id: String,
    /// Free text for the owner's eyes ("demo 12345 — macd asia"); nothing
    /// reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub market: String,
    pub tf: String,
    pub strategy: String,
    pub params: BTreeMap<String, f64>,
    pub filters: Vec<String>,
    pub guards: bool,
    pub window: usize,
}

impl PaperConfig {
    /// The run's id: the stored one, or for a state file from the
    /// one-run-per-`market:tf` layout (no `id` key) the key that layout
    /// used, `<market>-<tf>` — which is also the directory it sits in.
    #[must_use]
    pub fn id(&self) -> String {
        if self.id.is_empty() { legacy_run_id(&self.market, &self.tf) } else { self.id.clone() }
    }

    /// The run's bar stream.
    #[must_use]
    pub fn stream(&self) -> String {
        stream_key(&self.market, &self.tf)
    }
}

/// The run id of the first layout, and the default's prefix.
#[must_use]
pub fn legacy_run_id(market: &str, tf: &str) -> String {
    format!("{market}-{tf}")
}

/// The id a start gets when it does not name one.
#[must_use]
pub fn default_run_id(market: &str, tf: &str, strategy: &str) -> String {
    format!("{market}-{tf}-{strategy}")
}

/// `market:tf`, the key a posted bar is matched on.
#[must_use]
pub fn stream_key(market: &str, tf: &str) -> String {
    format!("{market}:{tf}")
}

/// The bar still forming on a stream, plus the quote it was read with.
///
/// **Never decided on.** The paper loop steps on closed bars only
/// (`PaperRun::accept`); this is what the screen draws so the owner can see
/// the price move between them. It is held in `AppState::live_bars`, not in
/// any run, and it is not persisted: a forming bar is worth nothing after a
/// restart, and writing one per poll would rewrite ten state files a second
/// for a number no book ever reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveBar {
    /// The bar's **open**, in epoch milliseconds — the bucket it belongs to,
    /// not the moment it was read. The chart appends it as a candle.
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(default)]
    pub volume: Option<f64>,
    /// The quote at the read, when the source had one. A non-finite one is
    /// dropped rather than refused: the price is the point, the spread is
    /// decoration.
    #[serde(default)]
    pub bid: Option<f64>,
    #[serde(default)]
    pub ask: Option<f64>,
    /// When **the server** received it, in epoch milliseconds. The age the
    /// client shows and the staleness rule are both measured from this and
    /// not from `time`, so a clock the poller disagrees about cannot make a
    /// dead feed look fresh.
    pub at: i64,
}

/// `[a-z0-9_-]{1,40}`: a directory name that needs no escaping anywhere,
/// and short enough to read in a status line.
fn check_id(id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!("run id `{id}` must match [a-z0-9_-]{{1,{MAX_ID_LEN}}}")))
    }
}

/// How a fill was priced: the `entry` object on the `opened` row and on the
/// `trade` row that closes it, so the record says how the fill happened.
///
/// `price` is the number the fill was priced FROM, on the bar's own axis
/// (the instrument's quote) and before the half-spread entry cost - the bar's
/// open for a market fill, the order price for a limit, the touched side for
/// a stop. `entry_price` beside it on the same rows is after the cost. `null`
/// only on a position that was open before this record existed.
/// `requested_price` is the order's level, `null` for a market fill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryRecord {
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub price: Option<f64>,
    pub requested_price: Option<f64>,
}

impl EntryRecord {
    fn market(open: f64) -> Self {
        Self { entry_type: EntryType::Market, price: Some(open), requested_price: None }
    }

    /// The row for a position that predates the record: a market fill, since
    /// no other path existed, at an open nobody wrote down.
    fn unknown() -> Self {
        Self { entry_type: EntryType::Market, price: None, requested_price: None }
    }
}

/// One paper run: the config, the rolling window, the book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperRun {
    pub config: PaperConfig,
    /// Wall-clock epoch milliseconds the run was started.
    pub started_at: i64,
    /// The last `window` bars: the warm-up from the store, then what was
    /// posted.
    pub bars: Vec<Bar>,
    pub book: PaperBook,
    /// **The counterfactual.** The same strategy, on the same bars, under the
    /// same guards, that never hears an advisor.
    ///
    /// This is what makes an advisor answerable. Without it a veto is an
    /// opinion with no outcome: the trade did not happen, so nobody can say
    /// whether refusing it saved money or cost it, and a log of such opinions
    /// teaches nothing however long it runs. With it, every intervention has a
    /// measurable price — the difference between these two books IS the
    /// advisor's effect, in dollars, over exactly the same market.
    ///
    /// It holds its own position and can therefore diverge: once one book
    /// takes a trade the other refused, the strategy is asked from two
    /// different states. That divergence is the answer, not a bug in it.
    /// `None` until the first bar after this run was created or reloaded, then
    /// **a clone of the advised book at that instant**.
    ///
    /// Cloning rather than starting empty is what makes the comparison honest
    /// on a run that already has history: no advisor has ever spoken to it, so
    /// the two books were identical up to this moment by construction, and
    /// seeding the counterfactual with that shared past means the difference
    /// between them from here on is caused by advice and by nothing else. A
    /// shadow started empty beside a book with fourteen trades would read as a
    /// $200 advisor bill on its first day.
    #[serde(default)]
    pub shadow: Option<PaperBook>,
    /// The verdict waiting for the pending intent, if an advisor has posted
    /// one. Taken when the intent fills, so a stale verdict cannot outlive
    /// the intent it was about.
    #[serde(default)]
    pub advice: Option<Advice>,
    /// Wall clock at which the open position was first seen by this process -
    /// `OpenDto::learned_at`, kept here because it is a fact about the RECORD
    /// and not about the book.
    ///
    /// Deliberately not on `fd_backtest::Live`. The engine's position struct
    /// is replayed by the parity goldens and a backtest has no wall clock at
    /// all, so putting it there would mean either a field the backtest has to
    /// invent or a struct that no longer round-trips. The lag being measured
    /// is a property of how this API learns about a fill, so it belongs to
    /// this API.
    #[serde(default)]
    pub opened_learned_at: Option<i64>,
    /// Wall clock at which a DECIDER posted the pending intent, when one did.
    ///
    /// `None` on every rule-based book, and correctly so: a rule decides at
    /// the close of the bar it read, instantly, and the fill is priced at the
    /// next open. There is no gap to record.
    ///
    /// A model is different and the difference is measurable. Measured
    /// 2026-09-17 over 321 decisions in `decisions.jsonl`, a decision is
    /// posted 24.2 s after its bar closed at the median and 199 s at p90,
    /// against a model latency of 8.5 s p50 and 21.6 s p90. The intent then
    /// fills at the open of the bar stamped at that close - a price that
    /// already existed while the model was still thinking. It is look-ahead,
    /// small, and biased in the book's favour: if the model has any skill, the
    /// price before it spoke is better in the direction it chose.
    ///
    /// Recorded rather than corrected. Correcting it means filling at the tick
    /// the decision arrived on, which is a different fill model and a
    /// different hypothesis; see `docs/decisions/2026-09-17-entry-lag.md`.
    /// What this field buys is that the bias is visible in the record from
    /// now on instead of being invisible in it.
    #[serde(default)]
    pub pending_decided_at: Option<i64>,
    /// The H4 bar the pending intent's decider read, when it said so. Taken
    /// with the pending it belongs to, exactly as `pending_decided_at` is.
    #[serde(default)]
    pub pending_htf_bar_time: Option<i64>,
    /// How the OPEN position was entered, for the row its close will write.
    ///
    /// Beside `opened_learned_at` and for the same reason: it is a fact about
    /// the record, not about the book. `fd_backtest::Trade` is replayed by the
    /// parity goldens and a backtest only ever fills at the open, so the
    /// field lives here and is attached to the `trade` line when the position
    /// closes. `None` on a position opened before the field existed - which,
    /// since no other path existed then, was a market fill at an unknown
    /// open, and is written as exactly that.
    #[serde(default)]
    pub open_entry: Option<EntryRecord>,
    /// Who has been posting this book's entries, if anyone has. `None` on
    /// every rule-based run and on an `external` run nobody has driven yet.
    #[serde(default)]
    pub decider: Option<Decider>,
    /// Bars posted and accepted since the start.
    pub bars_seen: usize,
    /// Bars the store supplied at the start.
    pub warmup_bars: usize,
    /// Holes wider than two bar intervals between accepted bars.
    pub gaps: usize,
    /// Turned off by hand: the book takes no new position.
    ///
    /// It keeps being fed - bars arrive, the chart stays live, the gap counter
    /// stays honest - and an OPEN position is still managed to its stop, its
    /// target and its maximum hold. Pausing is not abandoning: dropping a live
    /// position the moment someone flicks a switch would leave real risk on a
    /// book nobody is watching, and on the mirror side it would leave a real
    /// position on a broker.
    ///
    /// Distinct from a driver that has died. This is deliberate and says so.
    #[serde(default)]
    pub paused: bool,
}

/// A strategy with its entries taken away.
///
/// Used for a paused run, and preferred to a flag threaded through the engine
/// because it needs no cooperation from anything: the book is stepped exactly
/// as before, so its exits, guards, gap counting and shadow all behave
/// identically, and the single thing that cannot happen is a new position.
///
/// An Exit the strategy wanted is passed through untouched. A paused book that
/// is holding must still be able to get out.
struct Halted<'a> {
    inner: &'a dyn Strategy,
}

impl Strategy for Halted<'_> {
    fn id(&self) -> &'static str {
        self.inner.id()
    }
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn description(&self) -> &'static str {
        self.inner.description()
    }
    fn default_params(&self) -> Params {
        self.inner.default_params()
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        self.inner.grid()
    }
    fn series(&self, p: &Params) -> Vec<String> {
        self.inner.series(p)
    }
    fn needs_options(&self) -> bool {
        self.inner.needs_options()
    }
    fn exits(&self) -> Exits {
        self.inner.exits()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        self.inner.indicators(p)
    }
    fn warmup(&self, p: &Params) -> usize {
        self.inner.warmup(p)
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        match self.inner.on_bar(ctx) {
            Intent::Enter { .. } => Intent::None,
            other => other,
        }
    }
}

/// What a posted bar did.
pub enum Accepted {
    /// The bar's time is the last bar's: already seen, nothing done.
    Seen,
    /// Appended and stepped.
    Stepped {
        report: StepReport,
        gap: Option<usize>,
        advice: Option<Advice>,
        /// What the counterfactual book closed on this same bar. Carried out
        /// rather than dropped, because the shadow's trades need appending to
        /// the run's history exactly as the advised book's do — it is a book
        /// with a P&L, not a scratch calculation.
        shadow_closed: Vec<fd_backtest::engine::Trade>,
    },
}

/// A bar's prices must be finite and ordered; checked once per POST, not
/// per run, since it is the bar that is wrong and not any run.
fn check_bar(bar: &Bar) -> Result<(), ApiError> {
    if ![bar.open, bar.high, bar.low, bar.close].iter().all(|v| v.is_finite()) || bar.high < bar.low {
        return Err(ApiError::BadRequest(format!("malformed bar at {}: prices must be finite and high >= low", bar.time)));
    }
    Ok(())
}

impl PaperRun {
    /// A stable name for the intent currently pending.
    ///
    /// The run, and the bar whose close produced the intent. An advisor asks
    /// about this string and answers with it, so a verdict that arrives after
    /// the intent has filled names a bar that is no longer last and is
    /// dropped rather than applied to whatever is pending now. That is the
    /// whole of the staleness protection and it needs no clock.
    #[must_use]
    pub fn pending_id(&self) -> String {
        format!("{}:{}", self.config.id, self.bars.last().map_or(0, |b| b.time))
    }

    /// The open of a bar that has just STARTED, before it closes: fill the
    /// pending intent at it, and nothing else.
    ///
    /// `None` when there is nothing to do — no bar advanced over yet, a time
    /// at or before the last bar, or no pending intent. The window is NOT
    /// pushed to and the strategy is NOT asked: this bar has not happened, and
    /// the only thing that can be known about it is where it opened.
    ///
    /// The counterfactual is seeded here as well as in [`PaperRun::accept`],
    /// and for the same reason it is seeded there: a shadow cloned from a book
    /// that has already taken an advised fill inherits that fill, and the
    /// comparison it exists to make is gone. It must be taken before anything
    /// on this bar touches the book, and that is now here.
    ///
    /// The shadow itself is not filled early. It fills when the bar closes, at
    /// the same `apply_costs(open)` — the same price, the same stamp, the same
    /// trade. What the counterfactual measures is advice, not wall clock, so
    /// there is nothing for it to learn from filling sooner.
    // Eight arguments, inherited verbatim from the call this was extracted
    // from: the same allow `open_position` carries, for the same reason.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_open(
        &mut self,
        time: i64,
        open: f64,
        strategy: &dyn Strategy,
        params: &Params,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
    ) -> Option<StepReport> {
        // The same ordering rule the closed-bar path applies, deliberately:
        // strictly newer than the last bar, gaps included. See
        // `PaperBook::fill_open` for why it is not adjacency.
        let last = self.bars.last()?;
        if time <= last.time {
            return None;
        }
        self.book.pending()?;

        let atr_prev = self.atr_for_next_fill(strategy, params, rules);
        let pending_id = self.pending_id();
        if self.shadow.is_none() {
            self.shadow = Some(self.book.clone());
        }
        let advice = self.advice.take().filter(|a| a.intent_id == pending_id);
        self.book.fill_open(time, open, atr_prev, rules, guards, bar_ms, advice.as_ref())
    }

    /// The sizing ATR a fill BEFORE the next close gets: the last CLOSED bar's,
    /// computed over the same bars the closed-bar path will use - not over the
    /// window as it stands.
    ///
    /// `accept` pushes the arriving bar and then TRIMS the window's head to
    /// `window` bars, and the indicators are computed on that window rather
    /// than on the whole history. An indicator with memory never entirely
    /// forgets its seed, so a window starting one bar later gives a slightly
    /// different value: measured here, an ATR-derived stop moved by 4e-6 of a
    /// point, which is nothing to a trade and is still the same fill getting
    /// two different stops depending on which half of the split filled it.
    /// This is the residue the module docstring names, and the point of the
    /// split is that ONLY the clock moves.
    ///
    /// So: compute over the window the trim will leave. The test that caught
    /// this is `the_open_fills_at_the_same_price_the_close_would_have`. One
    /// function for the open and for a tick fill, so an order filling inside
    /// the bar is sized exactly as a market fill at that bar's open would be.
    fn atr_for_next_fill(&self, strategy: &dyn Strategy, params: &Params, rules: &TradingRules) -> Option<f64> {
        let window = self.config.window.max(1);
        let start = (self.bars.len() + 1).saturating_sub(window);
        let bars = &self.bars[start..];
        let mut ind = compute_indicators(bars, &strategy.indicators(params)).unwrap_or_default();
        ensure_fallback_atr(bars, params, rules, &mut ind);
        ind.get(&sizing_atr_key(params, rules)).and_then(|s| s.last().copied()).filter(|v| v.is_finite())
    }

    /// The name of the resting order, the way an advisor addresses it: the
    /// run and the bar whose close produced it. Unlike [`PaperRun::pending_id`]
    /// it does not move as bars arrive, because the order outlives them.
    #[must_use]
    pub fn order_id(&self) -> Option<String> {
        self.book.pending_order().map(|o| format!("{}:{}", self.config.id, o.decided_bar_time))
    }

    /// Fill the resting order at `price` inside the bar stamped `time`, the
    /// tick's bucket, and nothing else - the same shape as
    /// [`PaperRun::accept_open`], with the same ordering rule (the bucket must
    /// be strictly newer than the last closed bar, so a tick from a bucket the
    /// run has already closed over cannot fill anything) and the same
    /// counterfactual seeding, for the reason given there.
    ///
    /// `None` when nothing rests, the bucket is not newer, or the book holds
    /// a position.
    // Nine arguments, the eight `accept_open` carries plus the tick's last
    // price for the fill-bar range: the same allow, for the same reason.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_order_fill(
        &mut self,
        time: i64,
        price: f64,
        last: f64,
        strategy: &dyn Strategy,
        params: &Params,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
    ) -> Option<(PendingOrder, StepReport)> {
        let last_bar = self.bars.last()?;
        if time <= last_bar.time {
            return None;
        }
        self.book.pending_order()?;

        let atr_prev = self.atr_for_next_fill(strategy, params, rules);
        let order_id = self.order_id()?;
        if self.shadow.is_none() {
            self.shadow = Some(self.book.clone());
        }
        let advice = self.advice.take().filter(|a| a.intent_id == order_id);
        self.book.fill_order(time, price, last, atr_prev, rules, guards, bar_ms, advice.as_ref())
    }

    /// One closed bar. `Err` for a bar older than the last or malformed;
    /// `Ok(Seen)` for the last bar again.
    pub fn accept(
        &mut self,
        bar: Bar,
        strategy: &dyn Strategy,
        params: &Params,
        rules: &TradingRules,
        guards: Option<&Guards>,
        bar_ms: i64,
    ) -> Result<Accepted, ApiError> {
        check_bar(&bar)?;
        let mut gap = None;
        if let Some(last) = self.bars.last() {
            if bar.time == last.time {
                return Ok(Accepted::Seen);
            }
            if bar.time < last.time {
                return Err(ApiError::BadRequest(format!("bar {} is older than the last bar {}", bar.time, last.time)));
            }
            if bar_ms > 0 && bar.time - last.time > 2 * bar_ms {
                // Recorded, not filled: an invented bar is worse than a hole.
                let missing = usize::try_from((bar.time - last.time) / bar_ms - 1).unwrap_or(0);
                self.gaps += 1;
                gap = Some(missing);
            }
        }

        // Read BEFORE the arriving bar is pushed: the pending intent belongs to
        // the bar that is still last, and that is the name the advisor saw.
        let pending_id = self.pending_id();

        // And the counterfactual is taken BEFORE this bar is stepped, so it
        // never inherits a fill that advice has already touched.
        if self.shadow.is_none() {
            self.shadow = Some(self.book.clone());
        }

        self.bars.push(bar);
        if self.bars.len() > self.config.window.max(1) {
            let excess = self.bars.len() - self.config.window.max(1);
            self.bars.drain(..excess);
        }
        self.bars_seen += 1;

        // Indicators on the window, as the engine computes them on a series.
        let bars = &self.bars;
        let i = bars.len() - 1;
        let mut ind = compute_indicators(bars, &strategy.indicators(params)).unwrap_or_default();
        ensure_fallback_atr(bars, params, rules, &mut ind);
        let atr_prev = i
            .checked_sub(1)
            .and_then(|previous| ind.get(&sizing_atr_key(params, rules)).and_then(|s| s.get(previous).copied()))
            .filter(|v| v.is_finite());
        let keys = strategy.series(params);
        let resolved: Vec<&[f64]> = keys.iter().map(|k| ind.get(k).map_or(&[][..], |s| &s[..])).collect();
        let warmup = strategy.warmup(params);

        // The advice is consumed here and nowhere else: an advisor that posted
        // about an intent which has since been withdrawn finds it already
        // gone, and an intent nobody advised on fills exactly as the strategy
        // decided it. The only failure mode of a dead advisor is no advice.
        let advice = self.advice.take().filter(|a| a.intent_id == pending_id);
        let report = self.book.step(&bars[i], atr_prev, rules, guards, bar_ms, advice.as_ref(), |position| {
            if i < warmup {
                return Intent::None;
            }
            let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &resolved, options: None, position, params };
            strategy.on_bar(&ctx)
        });

        // The same bar through the book that never hears anyone. Stepped after
        // the advised one and never before, so a panic here could not leave the
        // real book half-advanced.
        let shadow = self.shadow.as_mut().expect("seeded above");
        let shadow_report = shadow.step(&bars[i], atr_prev, rules, guards, bar_ms, None, |position| {
            if i < warmup {
                return Intent::None;
            }
            let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &resolved, options: None, position, params };
            strategy.on_bar(&ctx)
        });

        let shadow_closed = shadow_report.trades.clone();
        Ok(Accepted::Stepped { report, gap, advice, shadow_closed })
    }
}

/* ---------------- the strategy behind a run ---------------- */

/// The filtered strategy and the full parameters a config names. Checked
/// the way the backtest route checks a request: an unknown strategy, a
/// parameter the strategy never declared or a misspelt filter is refused.
/// `news_currencies` is the market's list, so a `news:` filter with no
/// currencies of its own reads the market's releases and not everyone's.
fn resolve<'a>(registry: &'a Registry, config: &PaperConfig, news_currencies: &[String]) -> Result<(Filtered<'a>, Params), ApiError> {
    let inner = registry.get(&config.strategy).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let mut params = inner.default_params();
    for (name, value) in &config.params {
        if !params.contains(name) {
            return Err(ApiError::BadRequest(format!("{} has no parameter `{name}`", config.strategy)));
        }
        params.set(name, *value);
    }
    let filters = config
        .filters
        .iter()
        .filter(|f| !f.trim().is_empty())
        .map(|f| Filter::parse_for_market(f, news_currencies))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::BadRequest)?;
    Ok((Filtered { inner, filters }, params))
}

/* ---------------- persistence ---------------- */

/// Whatever process is driving this book, as it last said so.
///
/// A decision only arrives on a closed bar, so until this existed a driver that
/// had been killed looked exactly like one thinking about the current bar - for
/// up to two bars, half an hour on a 15m book. The driver writes this every
/// poll instead, independent of the market, so the same question is answerable
/// in about a minute and answerable with the market shut.
///
/// Absent means nobody has ever written one for this book - a rule-driven run,
/// or a driver older than the feature. It does NOT mean stopped, and the client
/// falls back to reading the decider's bar gap in that case.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DriverDto {
    /// When the driver last said it was alive, epoch ms.
    pub at: i64,
    pub model: Option<String>,
    /// The book this process is primarily driving; a control book names its
    /// model's run here, which is how the desk knows the pair stops together.
    pub run: Option<String>,
    pub pid: Option<i64>,
    /// The driver's own poll interval in seconds, so the client can judge
    /// staleness against the cadence the driver actually keeps rather than
    /// against a number baked into the UI.
    pub poll_s: Option<f64>,
}

fn driver_of(data: &Path, id: &str) -> Option<DriverDto> {
    let text = std::fs::read_to_string(run_dir(data, id).join("driver.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Every account that has mirrored this book, newest snapshot first.
///
/// A list and not an option, because one book can legitimately run on several
/// accounts at once - a demo and a small real one side by side is the
/// comparison most worth making - and the single `broker.json` this replaced
/// let the second executor overwrite the first's record.
///
/// Absent, unreadable and malformed are all the same answer - skipped - on
/// purpose: this is a view of somebody else's files, and no state of them
/// should be able to fail a status request for every other book.
fn brokers_of(data: &Path, id: &str) -> Vec<BrokerDto> {
    let Ok(entries) = std::fs::read_dir(data.join("live")) else { return Vec::new() };
    let mut out: Vec<BrokerDto> = entries
        .flatten()
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path().join(id).join("broker.json")).ok()?;
            let mut dto: BrokerDto = serde_json::from_str(&text).ok()?;
            // The directory is the authority on whose record this is. The file
            // names its account too, but a file that has been moved or copied
            // would then claim to belong somewhere it does not.
            dto.account = e.file_name().to_string_lossy().into_owned();
            Some(dto)
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

fn run_dir(data: &Path, id: &str) -> PathBuf {
    data.join("paper").join(id)
}

/// Write the run's state whole, through a temporary file so a crash
/// mid-write leaves the previous state rather than half of the new one.
fn persist(data: &Path, run: &PaperRun) -> Result<(), ApiError> {
    let dir = run_dir(data, &run.config.id());
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", dir.display())))?;
    let text = serde_json::to_string(run).map_err(|e| ApiError::Internal(format!("paper: serialise: {e}")))?;
    let tmp = dir.join("state.json.tmp");
    let path = dir.join("state.json");
    std::fs::write(&tmp, text).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, &path).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))
}

/// Append one event line to the run's `fills.jsonl`.
fn record(data: &Path, id: &str, event: &serde_json::Value) -> Result<(), ApiError> {
    let dir = run_dir(data, id);
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", dir.display())))?;
    let path = dir.join("fills.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))?;
    writeln!(file, "{event}").map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))
}

/// Just the `kind` of a line, so a caller that only wants to know whether a
/// line is a trade does not build the whole event to find out.
///
/// `kind` is a `Value` and not a `String` on purpose: a line whose `kind` is
/// not a string, or which has none, must be counted exactly as it was when
/// this read every line into a `serde_json::Value` — anything else would
/// quietly change a number on the Desk.
#[derive(Deserialize)]
struct KindOnly {
    #[serde(default)]
    kind: serde_json::Value,
}

/// Is this line one of the run's events rather than one of its trades?
///
/// The same test the full read used — an object, whose `kind` is not `trade`
/// — with the rest of the line skipped instead of allocated.
fn is_event(line: &str) -> bool {
    serde_json::from_str::<KindOnly>(line).is_ok_and(|k| k.kind != "trade")
}

/// The last `limit` of the run's `fills.jsonl` lines that are not trades,
/// oldest first: the `started`, `gap`, `refused`, `guard_close` and `stopped`
/// events. A line that is not a JSON object (a write cut short by a crash) is
/// skipped, not fatal — the book in `state.json` is the record, this file is
/// its narration. No file (a run that never wrote one) is no events.
///
/// Read from the end since 2026-09-17. It read the file whole and then threw
/// away all but the last [`MAX_DETAIL_EVENTS`], which is affordable at the
/// size these files are — the largest `fills.jsonl` on this desk is 8.5 KB —
/// and stops being affordable on a schedule nobody is watching, because
/// nothing rotates it. Measured 2026-09-17 on a 2 MB file: 8.7 ms whole
/// against 350 us for the last two hundred events.
///
/// This is now the ONLY reader of `fills.jsonl` on a served route. There was a
/// second, `event_count`, which read the whole file on every poll of
/// `/api/paper/status` to produce a number no component, script or test ever
/// read; it went with the `events` field it fed. The Desk's event count comes
/// from `detail.events.length`, which is this function.
///
/// Rotation is not the fix for this file and the arithmetic says so. Measured
/// 2026-09-17 across all twenty books: `fills.jsonl` grows at 60 to 470 bytes
/// an hour, because it is written per trade and per event and not per bar —
/// 0.5 to 4 MB a year, so twenty-five years to the 32 MiB the per-bar logs
/// roll at. It is not the same disease as `decisions.jsonl`; it was only read
/// wastefully, twice.
///
/// The window has to grow on the count of EVENTS, not of lines, because
/// trades are interleaved with them and are a third of the file. Growing on
/// lines would be the cheaper loop and would quietly return fewer than
/// `limit` events on a book that trades a lot.
fn events_of(data: &Path, id: &str, limit: usize) -> Vec<serde_json::Value> {
    let path = run_dir(data, id).join("fills.jsonl");
    let Some((window, from_start)) =
        tail_window(&path, |lines| lines.iter().filter(|l| is_event(l)).count() >= limit)
    else {
        return Vec::new();
    };
    let mut out: Vec<serde_json::Value> = complete_lines(&window, from_start)
        .iter()
        .filter(|l| is_event(l))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let skip = out.len().saturating_sub(limit);
    out.drain(..skip);
    out
}

/// One closed trade as `trades.jsonl` carries it: the trade itself, whole, and
/// which of the run's two books it belongs to.
///
/// The engine's own `Trade` is serialised rather than a display shape, so a
/// reload reconstructs exactly what was closed — `fills.jsonl` is written for
/// a reader and drops `exit_kind` and `swap_usd`, which metrics need.
#[derive(Debug, Deserialize, Serialize)]
struct HistoryLine {
    /// `main` or `shadow`.
    book: String,
    trade: fd_backtest::engine::Trade,
    /// Book equity immediately after this trade closed, so the curve is read
    /// back rather than re-derived from a fold that could drift.
    equity: f64,
}

/// Append every trade closed on this step to the run's history.
fn append_history(data: &Path, id: &str, book: &str, trades: &[fd_backtest::engine::Trade], equity: f64) {
    if trades.is_empty() {
        return;
    }
    let dir = run_dir(data, id);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("trades.jsonl")) else {
        return;
    };
    use std::io::Write as _;
    for trade in trades {
        let line = HistoryLine { book: book.to_string(), trade: trade.clone(), equity };
        if let Ok(text) = serde_json::to_string(&line) {
            let _ = writeln!(file, "{text}");
        }
    }
}

/// Put `trades` and `equity_curve` back on a book loaded from its state file.
///
/// A half-written last line is skipped rather than fatal: the writer is a
/// process that gets killed, and one lost trade must not cost the history.
fn restore_history(data: &Path, run: &mut PaperRun) {
    let path = run_dir(data, &run.config.id()).join("trades.jsonl");

    // A book written before the history moved out still carries its trades in
    // `state.json`. Migrate them across on first sight rather than letting the
    // next `persist` drop them: this change must cost nobody their record.
    if !path.exists() {
        let id = run.config.id();
        if !run.book.trades.is_empty() {
            append_history(data, &id, "main", &run.book.trades, run.book.equity);
        }
        if let Some(shadow) = run.shadow.as_ref() {
            if !shadow.trades.is_empty() {
                append_history(data, &id, "shadow", &shadow.trades, shadow.equity);
            }
        }
        // Already in memory from the state file; nothing further to read.
        if !run.book.trades.is_empty() || run.shadow.as_ref().is_some_and(|s| !s.trades.is_empty()) {
            return;
        }
    }

    let Ok(text) = std::fs::read_to_string(&path) else { return };

    // `trades.jsonl` is the record; drop anything the state file still carries.
    //
    // Without this the two sources ADD. A book written by an older binary keeps
    // its trades in `state.json`, the migration above copies them into the
    // history, and the next load then read both — every trade counted twice,
    // every net doubled, and profit factor unchanged because it is a ratio, so
    // the one number that would have looked wrong looked right. Caught by
    // comparing fourteen live books across a restart, not by the unit test,
    // which starts from a book that never had a legacy state file.
    run.book.trades.clear();
    run.book.equity_curve.clear();
    if let Some(shadow) = run.shadow.as_mut() {
        shadow.trades.clear();
        shadow.equity_curve.clear();
    }

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<HistoryLine>(line) else { continue };
        let book = if entry.book == "shadow" { run.shadow.as_mut() } else { Some(&mut run.book) };
        if let Some(book) = book {
            book.equity_curve.push((entry.trade.exit_time, entry.equity));
            book.trades.push(entry.trade);
        }
    }
}

/// Every `state.json` under `<data>/paper/`, keyed by run id. A file that
/// does not parse is reported on stderr and skipped rather than taking the
/// process down: the other runs are still worth keeping.
///
/// Both layouts load: a file with an `id` keys by it; one without (written
/// when there was one run per `market:tf`, in `<market>-<tf>/`) keys by
/// `<market>-<tf>`, and the id is filled in so the next write says it.
#[must_use]
pub fn reload(data: &Path) -> BTreeMap<String, PaperRun> {
    let mut runs = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(data.join("paper")) else { return runs };
    for entry in entries.flatten() {
        let path = entry.path().join("state.json");
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<PaperRun>(&t).map_err(|e| e.to_string())) {
            Ok(mut run) => {
                let id = run.config.id();
                run.config.id.clone_from(&id);
                restore_history(data, &mut run);
                if let Some(previous) = runs.insert(id.clone(), run) {
                    eprintln!("paper: two state files claim run `{id}`; keeping {}, the earlier one was for {}", path.display(), previous.config.stream());
                }
            }
            Err(e) => eprintln!("paper: could not reload {}: {e}", path.display()),
        }
    }
    runs
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn trade_event(kind: &str, trade: &Trade) -> serde_json::Value {
    let mut event = json!({ "kind": kind, "time": trade.exit_time, "trade": TradeDto::from(trade) });
    if let Some(reason) = matches!(trade.exit_kind, fd_backtest::ExitKind::Guard(_)).then(|| trade.exit_reason.clone()) {
        event["reason"] = json!(reason);
    }
    event
}

/* ---------------- wire shapes ---------------- */

#[derive(Debug, Deserialize)]
pub struct StartRequest {
    /// `[a-z0-9_-]{1,40}`; default `<market>-<tf>-<strategy>`.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    pub market: String,
    pub tf: String,
    pub strategy: String,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(default)]
    pub filters: Vec<String>,
    /// Off only for a control; the design says a paper run with guards
    /// off is not a paper run, and the status says which this is.
    #[serde(default = "default_true")]
    pub guards: bool,
    #[serde(default = "default_window")]
    pub window: usize,
}

const fn default_true() -> bool {
    true
}

const fn default_window() -> usize {
    DEFAULT_WINDOW
}

#[derive(Debug, Deserialize)]
pub struct BarIn {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(default)]
    pub volume: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct BarRequest {
    pub market: String,
    pub tf: String,
    pub bar: BarIn,
}

/// `POST /api/paper/tick`: the forming bar, and the quote it was read at.
#[derive(Debug, Deserialize)]
pub struct TickRequest {
    pub market: String,
    pub tf: String,
    pub bar: BarIn,
    #[serde(default)]
    pub bid: Option<f64>,
    #[serde(default)]
    pub ask: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct TickResponse {
    /// Always true on a 200: the tick is stored or the request was refused.
    pub stored: bool,
    /// What the tick did to resting orders on the stream - absent, not
    /// empty, when it reached none, which is nearly every tick.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub orders: Vec<TickOrderResponse>,
}

/// Which run to stop: by `id`, or by `market` + `tf` when exactly one run
/// is on that stream (the first client's spelling).
#[derive(Debug, Deserialize)]
pub struct StopRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub market: Option<String>,
    #[serde(default)]
    pub tf: Option<String>,
}

/// What one run did with a posted bar.
#[derive(Debug, Serialize)]
pub struct RunBarResponse {
    pub id: String,
    /// False when the run had this bar already (`reason: "seen"`) or
    /// refused it (`reason` says why); the other runs are unaffected.
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Bars in the run's window after this one.
    pub bars: usize,
    /// Trades closed on this bar.
    pub closed: Vec<TradeDto>,
    /// A position opened on this bar.
    pub opened: bool,
    /// The guard that refused the pending entry, if one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
    /// Bars missing before this one, when the feed skipped some.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct BarResponse {
    /// True when at least one run stepped on the bar. The poller's log
    /// line reads this and `bars`; the rest is per run.
    pub accepted: bool,
    /// Why no run stepped, when none did: the first run's reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The widest window among the runs fed, after this bar.
    pub bars: usize,
    /// One entry per run on this `market:tf`, in id order.
    pub runs: Vec<RunBarResponse>,
}

#[derive(Debug, Serialize)]
pub struct StopResponse {
    pub id: String,
    pub stopped: bool,
    /// The position the stop closed, if there was one.
    pub closed: Option<TradeDto>,
    pub trades: usize,
    pub equity: f64,
    pub net_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct OpenDto {
    pub side: &'static str,
    pub entry_time: i64,
    pub entry_price: f64,
    pub lots: f64,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    /// Worst and best excursion so far, in R.
    pub mae: f64,
    pub mfe: f64,
    /// One R, in price.
    pub risk: f64,
    /// Wall clock, epoch ms, at which this desk LEARNED the position was open
    /// - as against `entry_time`, which is the bar the fill is PRICED at.
    ///
    /// They are not the same instant and the gap is structural, not jitter.
    /// An intent decided on the bar closing at `t` fills at the open of the
    /// bar stamped `t`, but that bar is only posted once it CLOSES at `t+1`,
    /// so the book first holds the position a whole bar after the price it
    /// holds it at. Measured on the VPS 2026-09-17, six positions out of six
    /// surfaced to the executor exactly one bar after the book's stamp.
    ///
    /// Published because a mirror, a slippage comparison and a drift alert all
    /// read `entry_time` and none of them could previously tell how old it was
    /// by the time anyone saw it. `null` on a position that was open before
    /// this field existed, or reloaded from a state file written without it -
    /// which is absent, not zero.
    ///
    /// See `docs/decisions/2026-09-17-entry-lag.md`. The lag is NOT a
    /// systematic cost: measured over 30 live entries it is +0.01 R at the
    /// median and -0.08 R at the mean with a standard error of 0.08. What it
    /// is, is 0.42 R of dispersion per entry that the record does not show.
    pub learned_at: Option<i64>,
    /// Marked at the last close less exit costs, before commission and swap.
    pub unrealised_usd_at_last_close: f64,
    /// Dollars per one unit of price movement: `lots x contract_size`.
    ///
    /// Sent so the desk can mark the position against the LIVE tick instead of
    /// the last close. Without it the client can only repeat a number that is
    /// up to fifteen minutes old, which is the wrong number to show beside a
    /// price that is moving — and it cannot be derived from the fields above
    /// either, because `unrealised / (close - entry)` divides by zero exactly
    /// when a position opens.
    pub usd_per_point: f64,
}

/// An entry that has been decided but has not filled.
///
/// Between the close that decided it and the open that fills it, a trade is
/// real — it will happen, at a price nobody knows yet — and the desk showed
/// nothing at all for that whole bar. On a fifteen-minute book that is fifteen
/// minutes of a book looking flat while a position is already committed.
#[derive(Debug, Serialize)]
pub struct PendingDto {
    pub side: String,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    /// The sentence whoever decided it wrote for taking the trade.
    pub reason: String,
    /// The bar whose close produced it. The fill is the NEXT bar's open.
    pub decided_on: Option<i64>,
}

/// An entry resting at a PRICE, waiting on the tick feed.
///
/// The order's own fields as [`PendingOrder`] holds them, plus the two the run
/// keeps beside it. Every price is on the bar's axis and before the entry
/// cost, as the order's are. `pending` (the market intent) is `null` while
/// this is set: the book has one committed entry at most.
#[derive(Debug, Serialize)]
pub struct PendingOrderDto {
    /// `limit` or `stop`; a market entry never rests.
    #[serde(rename = "type")]
    pub entry_type: &'static str,
    pub price: f64,
    pub side: &'static str,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    pub reason: String,
    /// `[lo, hi]` as the decider stated it; informational.
    pub zone: Option<(f64, f64)>,
    /// The stamp of the bar whose close is EXPECTED to expire it:
    /// `decided_bar_time + valid_bars x bar_ms`. The rule is the count of
    /// CLOSED bars, so across a gap in the tape the cancel comes at a later
    /// stamp than this; `valid_bars` and `bars_waited` are the rule itself.
    pub valid_until_bar_ms: i64,
    pub valid_bars: u32,
    pub bars_waited: u32,
    /// The bar whose close produced it; also the tail of `intent_id` on
    /// `/api/paper/pending`, which an advisor answers with.
    pub decided_bar_time: i64,
    /// Wall clock, epoch ms, at which the decider posted it. `null` after a
    /// reload from a state file written without it.
    pub decided_at: Option<i64>,
    pub invalidate_above: Option<f64>,
    pub invalidate_below: Option<f64>,
    /// The lots the fill WOULD take, by the engine's own sizing rule
    /// (`open_position`: risk per trade over the stop distance, floored to
    /// the lot step, then the notional cap) on the book's equity as it stands
    /// and the order price as the entry. Recomputed on every read, so it
    /// follows the equity; the ATR does not enter it, because an order always
    /// carries its stop and the ATR only sizes a stop-less entry. Two things
    /// can still move the number at the fill: a STOP order fills at the side
    /// that traded through, not at its level, so its risk unit is measured
    /// from there; and an advisor's cut is applied at the fill and never
    /// before. `null` when the rule would refuse the entry outright (no risk
    /// unit, or the cap refuses), which is what the fill would do too.
    pub lots: Option<f64>,
}

impl PendingOrderDto {
    fn of(order: &PendingOrder, run: &PaperRun, rules: &TradingRules, guards: Option<&Guards>) -> Self {
        let bar_ms = timeframe_ms(&run.config.tf).unwrap_or(0);
        let lots = fd_backtest::engine::open_position(
            order.side,
            order.stop,
            order.target,
            order.reason.clone(),
            order.decided_bar_time,
            order.price,
            None,
            run.book.equity,
            rules,
            run.book.is_self_managed(),
            guards,
        )
        .ok()
        .map(|(live, _)| live.lots);
        Self {
            lots,
            entry_type: order.entry_type.as_str(),
            price: order.price,
            side: order.side.as_str(),
            stop: order.stop,
            target: order.target,
            reason: order.reason.clone(),
            zone: order.zone,
            valid_until_bar_ms: order.decided_bar_time + i64::from(order.valid_bars) * bar_ms,
            valid_bars: order.valid_bars,
            bars_waited: order.bars_waited,
            decided_bar_time: order.decided_bar_time,
            decided_at: run.pending_decided_at,
            invalidate_above: order.invalidate_above,
            invalidate_below: order.invalidate_below,
        }
    }
}

/// The order as a `fills.jsonl` row carries it, whole: the `entry` object on
/// a `cancelled_unfilled` line, and the fields an `intent` line adds.
fn order_json(order: &PendingOrder) -> serde_json::Value {
    json!({
        "type": order.entry_type.as_str(),
        "price": order.price,
        "side": order.side.as_str(),
        "stop": order.stop,
        "target": order.target,
        "reason": order.reason,
        "zone": order.zone,
        "valid_bars": order.valid_bars,
        "decided_bar_time": order.decided_bar_time,
        "invalidate_above": order.invalidate_above,
        "invalidate_below": order.invalidate_below,
    })
}

#[derive(Debug, Serialize)]
pub struct BlackoutDto {
    pub time: i64,
    pub currency: String,
    pub impact: u8,
    /// The release's name; empty when the calendar did not carry one.
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct NewsDto {
    pub events_loaded: usize,
    pub next_blackout: Option<BlackoutDto>,
    /// The LAST event this run's guards would ever act on, and how far away it
    /// is in days.
    ///
    /// A calendar is a finite list, and the day after its last entry the news
    /// guard stops guarding without failing, without logging, and without
    /// anybody noticing. The scheduled US layer runs out on 2026-12-10 while
    /// the Fed's own dates run to 2027-12 — so the number that matters is not
    /// how long the FILE lasts but how long it lasts **for this run's
    /// currencies**, which is what this is.
    pub horizon: Option<i64>,
    pub horizon_days: Option<i64>,
    /// Which series runs out first — the thing to go and refresh.
    pub horizon_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunStatus {
    pub id: String,
    pub label: Option<String>,
    pub market: String,
    pub tf: String,
    pub strategy: String,
    /// Who has actually driven this book, for runs driven from outside. `null`
    /// on a rule-based run. See [`Decider`].
    pub decider: Option<Decider>,
    pub params: BTreeMap<String, f64>,
    pub filters: Vec<String>,
    pub guards: bool,
    pub started_at: i64,
    /// Bars in the window.
    pub bars: usize,
    pub bars_seen: usize,
    pub warmup_bars: usize,
    pub last_bar_time: Option<i64>,
    /// The close of that bar. The reference the live price is read against:
    /// without it the client can say the price but not whether it has moved
    /// up or down since the bot last decided anything.
    pub last_bar_close: Option<f64>,
    pub equity: f64,
    /// What this book's account is denominated in, and how many of those units
    /// make a dollar. Every money field above and below is in USD; these two
    /// let the client show the number the account holder actually sees, without
    /// the conversion ever touching the arithmetic.
    pub account_currency: String,
    pub units_per_usd: f64,
    /// Account leverage and units per lot, so the client can say what margin a
    /// position ties up without re-deriving either from a price.
    pub leverage: f64,
    pub contract_size: f64,
    pub open: Option<OpenDto>,
    /// An entry decided at the last close and waiting for the next open. Never
    /// set at the same time as a fill on the same bar: the book takes one
    /// position, so a pending entry means the book is flat right now and will
    /// not be after the next bar opens.
    pub pending: Option<PendingDto>,
    /// An entry resting at a price, waiting on the tick feed - `null` on
    /// every rule-based run, and on an external run between orders. Never set
    /// beside `pending`; see [`PendingOrderDto`].
    pub pending_order: Option<PendingOrderDto>,
    pub trades: usize,
    pub net_usd: f64,
    /// The introducing-broker rebate on this book's own spread, and the same
    /// book net of it. A SEPARATE LINE: `net_usd` above is untouched by this
    /// and means exactly what it meant before the rebate existed.
    ///
    /// `null` when `config/accounts.toml` declares no arrangement - which is
    /// not a rebate of zero. See [`RebateDto`].
    pub rebate: Option<RebateDto>,
    /// `null` with no losing trade yet (the engine's infinity).
    pub profit_factor: f64,
    pub skipped_by_guard: BTreeMap<String, usize>,
    pub closed_by_guard: BTreeMap<String, usize>,
    pub sized_down: usize,
    pub skipped_no_atr: usize,
    pub gaps: usize,
    pub news: NewsDto,
    /// The bar forming right now on this run's `market:tf`, or `null` when
    /// no tick has arrived within [`MAX_LIVE_AGE_MS`]. Shown, never traded:
    /// the run's own decisions are all in the fields above, which move only
    /// when a bar closes.
    pub live: Option<LiveBar>,
    /// The last ten closed trades, oldest first.
    pub last_fills: Vec<TradeDto>,
    /// Points on the book's equity curve (one per closed trade). The curve
    /// itself is on `GET /api/paper/run/{id}`; the status stays light.
    pub equity_curve: usize,
    /// The process driving this book, as it last reported. `null` when none
    /// has ever written one, which is not the same as stopped - see
    /// [`DriverDto`].
    pub driver: Option<DriverDto>,
    /// Turned off by hand. An open position is still being managed; what has
    /// stopped is new entries. Deliberate, and not to be confused with a
    /// driver that died.
    pub paused: bool,
    /// The broker accounts this book is mirrored into - empty when no
    /// executor has ever run it, and more than one when it runs on several
    /// accounts at once. See [`BrokerDto`]: a present entry is not a connected
    /// one, so read its `at`.
    pub brokers: Vec<BrokerDto>,
}

/// What the broker's account looks like for one book, as the executor last saw it.
///
/// This API is Rust and cannot ask a MetaTrader terminal anything: the only
/// process that can see the account is the executor mirroring the book, so it
/// writes what it sees to `data/live/<account>/<run>/broker.json` each poll
/// and this serves it back. Every field is optional because the file is
/// written by another program - a shape that drifts should cost a field, not
/// the whole account.
///
/// Two corrections to that sentence, both made 2026-09-17 after it was read
/// as a promise it does not keep.
///
/// The path was `data/paper/<run>/broker.json` and has not been since the
/// live side moved out into its own directory per account.
///
/// And it said "serves the file back unchanged", which it does not: the file
/// is parsed INTO this struct and the struct is what is serialised, so a
/// field the executor writes and this does not declare never reaches the
/// browser. That is not a bug - unknown fields are ignored rather than
/// refused, which is what keeps the account view alive when the executor is
/// deployed ahead of the API, and `broker_snapshot_tests` pins both halves.
/// It does mean that surfacing something new the executor reports takes a
/// field here, a field in `ui/src/lib/api.ts`, and somewhere on the account
/// view to show it. `server_offset_ms` and `drift` were dropped here for
/// exactly that reason until the third part found an owner; both are declared
/// below now. Anything the executor writes and this does not declare still
/// stops at this boundary, which is the state to check before assuming a
/// reader can see a new field.
///
/// `at` is the part that matters. A stale file is a stopped executor, not a
/// live account, and the client decides what counts as stale rather than
/// having a threshold baked in here.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrokerDto {
    /// The account id this record belongs to - the directory it was read from,
    /// and `login-<n>` for an executor started outside the registry.
    pub account: String,
    /// When the executor last looked, epoch ms.
    pub at: i64,
    pub login: Option<i64>,
    pub server: Option<String>,
    /// False would mean a real-money account, which the executor refuses to
    /// trade; carried so the client can say so rather than assume.
    pub demo: Option<bool>,
    pub currency: Option<String>,
    pub balance: Option<f64>,
    pub equity: Option<f64>,
    pub margin: Option<f64>,
    pub margin_free: Option<f64>,
    /// `null` on a flat account rather than zero, which would read as a
    /// stop-out.
    pub margin_level: Option<f64>,
    pub symbol: Option<String>,
    pub contract_size: Option<f64>,
    /// How far the terminal's clock runs ahead of UTC, in milliseconds,
    /// measured from the terminal on the poll that wrote this - not derived
    /// from a timezone.
    ///
    /// Here because the book's times are UTC and the deal history's are the
    /// server's, and a reader comparing the two without knowing the offset is
    /// making the mistake that kept `already_taken` inert until 2026-09-17: a
    /// position the account already held read as absent, and the mirror bought
    /// it again. It reads 10,800,000 while Vantage is on +3h, measured on all
    /// five live books after that fix deployed.
    ///
    /// `null` means it could not be measured - no tick, or a tick too stale to
    /// be an offset - and that is also when the mirror refuses to open, so
    /// this field is the reason a book is sitting out rather than a detail.
    pub server_offset_ms: Option<i64>,
    pub magic: Option<i64>,
    pub lot_scale: Option<f64>,
    /// True while the executor is reconciling but sending nothing.
    pub dry_run: Option<bool>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    /// What the BOOK wanted at that moment, kept beside what the account
    /// holds. Two fields rather than one "in sync" flag, because the useful
    /// state is which of them is ahead and a boolean throws that away.
    pub book_side: Option<String>,
    pub book_lots: Option<f64>,
    /// What this book has actually banked on the account, in the account's
    /// currency, and over how many exits. The direct counterpart to the paper
    /// book's `net_usd` and `trades`, and the pair the mirror exists to
    /// compare.
    pub realised: Option<f64>,
    pub closed: Option<usize>,
    /// The introducing-broker rebate on what this book paid in spread ON THIS
    /// ACCOUNT, beside `realised` and never inside it.
    ///
    /// Computed by the executor, because nothing else can: the rebate needs
    /// the spread each fill actually paid and the terminal is the only thing
    /// that ever saw it. `null` when the executor is older than this field or
    /// `config/accounts.toml` declares no arrangement. See
    /// [`BrokerRebateDto`] for the estimate that most of it still is.
    pub rebate: Option<BrokerRebateDto>,
    /// The account's own closed trades for this book, oldest first - what the
    /// BROKER did, as against the paper book's `fills`, which are the rule
    /// executed perfectly at the bar's price. Kept separate rather than
    /// merged: the gap between the two entry prices is the slippage, and it is
    /// only visible while neither stands in for the other.
    #[serde(default)]
    pub fills: Vec<BrokerFillDto>,
    pub position: Option<BrokerPositionDto>,
    /// Something is wrong and a person has to act: the broker refused the
    /// order, or a desk guard stopped one that was wrong by a factor.
    pub blocked: Option<String>,
    /// The mirror is deliberately sitting this trade out - the book opened it
    /// too long ago, or too far from here, to be worth copying. Working as
    /// designed, and kept apart from `blocked` so that an alert on one is not
    /// an alert on the other.
    pub standing_out: Option<String>,
    /// The account holds the book's SIDE but not its shape: more than one
    /// position on the book, or a volume that is not the book's size times
    /// `lot_scale`. Human-readable, because every one of them names which.
    ///
    /// Reported and deliberately never corrected - the executor's own comment
    /// is the authority on why, and the short version is that a part-close or
    /// a close-and-reopen crystallises a result the book never took at a price
    /// it never saw, which destroys the one measurement this mirror exists to
    /// produce. It clears itself when the book next goes flat.
    ///
    /// A third string beside `blocked` and `standing_out` rather than a
    /// boolean, and a third one rather than folding into either: `blocked` is
    /// somebody must act now, `standing_out` is the guard working as designed,
    /// and this is neither - the account is wrong in a way nothing will fix on
    /// its own, and it is not an emergency. An alert that could not tell the
    /// three apart would be muted within a day.
    ///
    /// Recomputed every poll and cleared the moment it stops being true, which
    /// is why it lives in the snapshot and not in `executor.jsonl`: that file
    /// records that it happened, this field says whether it is happening.
    pub drift: Option<String>,
}

/// What this book's trading on this account earned back in rebate.
///
/// THE HONEST PART, AND IT IS THE WHOLE REASON THIS TYPE HAS THREE COUNTS.
/// The desk did not record the spread at the moment of a real fill until
/// 2026-09-21. `broker.json` carries `bid` and `ask` as of the SNAPSHOT,
/// which is whenever the executor last polled and has nothing to do with
/// when a trade filled. So for a trade whose spread was not captured, the
/// credit can only be worked out from the CONFIGURED spread, and that is an
/// estimate - the logger has measured XAUUSD.sc as low as 0.210 against a
/// configured 0.280, so an estimate on this market is high by about a
/// quarter.
///
/// The executor now reads the quote immediately before every order it sends
/// and keeps it against the deal ticket, so those fills are exact. It still
/// cannot be exact about an exit the BROKER took - a stop or a target fires
/// with no order from this desk and no quote read - so `estimated` will stay
/// the larger number on any book that exits at its stop.
///
/// `exact` and `estimated` are never added into one figure without both
/// counts beside it. A field that mixed measured and estimated values
/// without saying which is the kind of thing this desk treats as a defect.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrokerRebateDto {
    /// The credit, in the ACCOUNT's currency - USC on the funded cent
    /// account, like `realised` and unlike every USD figure on the paper
    /// book beside it. `currency` below says which, in the record rather
    /// than only in this comment.
    pub amount: Option<f64>,
    /// `realised + amount`, in the same currency. Published beside
    /// `realised` and never folded into it.
    pub realised_with_rebate: Option<f64>,
    /// The account's currency, so the two figures above are never read as
    /// dollars on a cent account.
    pub currency: Option<String>,
    /// The share of the round-turn spread the terms give, as the executor
    /// read it from `config/accounts.toml`.
    pub share_of_spread: Option<f64>,
    /// The spread the ESTIMATED trades were priced from, per unit of price:
    /// the book's own configured spread, as the paper desk charges it.
    /// `null` when the executor could not reach the API to ask for it, in
    /// which case nothing could be estimated at all.
    pub configured_spread: Option<f64>,
    /// Trades priced from a spread read at the moment the order was sent.
    pub exact: Option<usize>,
    /// Trades priced from the configured spread because no quote was
    /// captured for them - every trade closed before 2026-09-21, and every
    /// exit the broker took at a stop or a target since.
    pub estimated: Option<usize>,
    /// Trades with no usable basis at all, credited nothing and counted.
    pub unpriced: Option<usize>,
}

/// One trade the account actually completed.
///
/// Deliberately NOT the same shape as a paper trade. A broker has no stop
/// distance, so it has no R, no MAE and no MFE, and a field carrying a zero
/// for those would read as a measurement rather than as an absence.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrokerFillDto {
    pub direction: Option<String>,
    #[serde(rename = "entryTime")]
    pub entry_time: Option<i64>,
    #[serde(rename = "entryPrice")]
    pub entry_price: Option<f64>,
    #[serde(rename = "exitTime")]
    pub exit_time: Option<i64>,
    #[serde(rename = "exitPrice")]
    pub exit_price: Option<f64>,
    pub lots: Option<f64>,
    /// The broker's own word for why it ended - `sl`/`tp` from MT5 itself, or
    /// the comment the executor wrote. Empty rather than guessed at.
    #[serde(rename = "exitReason")]
    pub exit_reason: Option<String>,
    /// In the ACCOUNT's currency, commission and swap included.
    pub pnl: Option<f64>,
    /// This trade's own rebate, in the ACCOUNT's currency, beside `pnl` and
    /// never inside it. `null` when it could not be priced.
    pub rebate: Option<f64>,
    /// `EXACT` when the credit came from a spread read at the moment the
    /// order was sent, `ESTIMATED` when it came from the configured spread,
    /// absent when the trade was not priced. A number without this word
    /// beside it would be a measured value and a guessed one sharing a
    /// column.
    #[serde(rename = "rebateBasis")]
    pub rebate_basis: Option<String>,
}

/// The position the account actually holds for this book, as opposed to the
/// one the book says it holds.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrokerPositionDto {
    pub ticket: Option<i64>,
    pub side: Option<String>,
    pub lots: Option<f64>,
    pub entry_price: Option<f64>,
    pub price_now: Option<f64>,
    pub sl: Option<f64>,
    pub tp: Option<f64>,
    /// In the ACCOUNT's currency, which is the broker's number and not the
    /// book's - the whole reason for showing both sides.
    pub profit: Option<f64>,
    pub swap: Option<f64>,
    pub opened_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub runs: Vec<RunStatus>,
}

/// `GET /api/paper/run/{id}`: the status entry and what it leaves out.
#[derive(Debug, Serialize)]
pub struct RunDetail {
    /// The same entry `/api/paper/status` carries for this run.
    pub run: RunStatus,
    /// The same forming bar `run.live` carries, hoisted so the chart reads it
    /// beside `bars` rather than through the status entry.
    pub live: Option<LiveBar>,
    /// `[time_ms, equity]` after every closed trade, oldest first, led by
    /// `[started_at, starting equity]` so a run with no trade still draws
    /// a point.
    pub equity_curve: Vec<(i64, f64)>,
    /// Every closed trade, oldest first — the last [`MAX_DETAIL_FILLS`].
    pub fills: Vec<TradeDto>,
    /// The `fills.jsonl` lines that are not trades, oldest first — the
    /// last [`MAX_DETAIL_EVENTS`]. Each is the line as written: `kind`,
    /// `time`, and the kind's own fields.
    pub events: Vec<serde_json::Value>,
    /// `[time, open, high, low, close]` for the last `bars` of the run's
    /// window, oldest first.
    pub bars: Vec<(i64, f64, f64, f64, f64)>,
    /// The indicators the strategy declared, in the order it declared them.
    pub indicators: Vec<IndicatorDto>,
    /// Qualified key to its points over the returned `bars`, the time in
    /// seconds the way the chart wants it. A non-finite value is left out
    /// rather than sent as a null, so a warm-up is a gap in the line.
    pub series: BTreeMap<String, Vec<Point>>,
}

/// One indicator the run's strategy declared, as the chart needs it.
///
/// `outputs` are the qualified keys of its series in [`RunDetail::series`] —
/// one for a single-output indicator (`ema_21.ema`), several for one that
/// draws a band or a histogram (`macd_12_26_9.macd`, `.signal`, `.histogram`).
#[derive(Debug, Clone, Serialize)]
pub struct IndicatorDto {
    /// The indicator's id, e.g. `ema`.
    pub id: String,
    /// The instance key its outputs are qualified by, e.g. `ema_21`.
    pub key: String,
    /// The values the strategy asked for, in the definition's own order.
    pub params: BTreeMap<String, f64>,
    /// Qualified series keys, in the order the definition declares them.
    pub outputs: Vec<String>,
    /// `true` when the definition says the values belong over the candles;
    /// `false` for an oscillator, which wants its own pane.
    pub overlay: bool,
}

/// Closed trades a detail carries at most.
pub const MAX_DETAIL_FILLS: usize = 500;

/// Event lines a detail carries at most.
pub const MAX_DETAIL_EVENTS: usize = 200;

/// Window bars a detail carries when the query does not say.
pub const DEFAULT_DETAIL_BARS: usize = 120;

#[derive(Debug, Default, Deserialize)]
pub struct DetailQuery {
    /// Window bars to return, from the newest back; default
    /// [`DEFAULT_DETAIL_BARS`], at most the run's window.
    #[serde(default)]
    pub bars: Option<usize>,
}

/* ------------------------------------------- the introducing-broker rebate */

/// The rebate's terms, read from `config/accounts.toml`.
///
/// The owner is the introducing broker on his own accounts, so part of the
/// spread his trading pays comes back. He chose a SEPARATE CREDIT LINE rather
/// than a change to the cost model, and that choice is what every type below
/// is shaped by: nothing here ever alters `net_usd`, `equity` or a trade's
/// `pnl_usd`, and a reader can always see how much of a result is the
/// strategy and how much is the commercial arrangement.
///
/// Read per REQUEST, for the same reason [`account_registry`] is: changing a
/// commercial term should be editing one file, not restarting a desk that is
/// carrying live positions.
///
/// `None` from [`rebate_terms`] means the file declares no arrangement at
/// all. Every figure derived from it is then `null` and not zero - a rebate
/// of zero is one that was calculated and came to nothing, which is a
/// different fact about the world.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RebateTerms {
    /// The share of the ROUND-TURN spread credited back, 0.45 today.
    ///
    /// ROUND TURN, and the word decides the number. A position pays the
    /// spread once, between the ask it enters at and the bid it leaves at;
    /// the engine charges half on each side (`apply_costs`) and the two
    /// halves are ONE spread. So this multiplies the whole spread once per
    /// trade and never once per side, which would be twice the truth.
    ///
    /// `config/accounts.toml` also carries a commented-out `per_lot` form
    /// with the reason it is not implemented. An IB rebate is usually quoted
    /// per standard lot and the two forms are not the same number: a share of
    /// the spread moves with the spread, a per-lot figure does not.
    pub share_of_spread: f64,
}

#[derive(Debug, Deserialize)]
struct RebateFile {
    rebate: Option<RebateTerms>,
}

impl RebateTerms {
    /// The credit on one trade, in USD, or `None` when it cannot be priced.
    ///
    /// `lots x contract_size` is what one unit of price is worth in USD on
    /// this market - the engine's own P&L identity - so the spread the trade
    /// paid, in money, is `spread x lots x contract_size`, and this is the
    /// configured share of it.
    ///
    /// Refuses rather than returning zero when the spread or the size is not
    /// a positive number. A market configured with no spread charges nothing
    /// and therefore has nothing to rebate, but saying so with a `0.0` would
    /// make "not priced" and "priced at nothing" the same figure.
    fn on(self, lots: f64, spread: f64, contract_size: f64) -> Option<f64> {
        let finite = lots.is_finite() && spread.is_finite() && contract_size.is_finite();
        (finite && lots > 0.0 && spread > 0.0 && contract_size > 0.0)
            .then(|| fd_core::js_round_to(self.share_of_spread * spread * lots * contract_size, 4))
    }
}

/// The `[rebate]` table, or `None` when the file declares none.
///
/// A missing or malformed registry costs the rebate line and nothing else,
/// exactly as it costs the account labels and nothing else in
/// [`account_registry`]: a broken config file must not take down the view of
/// what is trading.
fn rebate_terms(dir: &Path) -> Option<RebateTerms> {
    let text = std::fs::read_to_string(dir.join("accounts.toml")).ok()?;
    let terms = toml::from_str::<RebateFile>(&text).ok()?.rebate?;
    terms.share_of_spread.is_finite().then_some(terms)
}

/// What one paper book was credited, beside what it made.
///
/// `null` on a run when no arrangement is configured. Present, it sits NEXT
/// TO [`RunStatus::net_usd`], which is untouched and still means what it has
/// always meant.
#[derive(Debug, Clone, Serialize)]
pub struct RebateDto {
    /// The configured share of the round-turn spread, as read.
    pub share_of_spread: f64,
    /// The spread the credit was computed from, per unit of price - the same
    /// number `config/default.toml` charges this market and the engine takes
    /// out of every fill. Carried here so the figure and the basis it came
    /// from travel together (`docs/decisions/2026-09-17-unit-carrying.md`).
    pub spread: f64,
    /// The credit over the book's closed trades, USD. In USD like every other
    /// money field on this response; `account_currency` and `units_per_usd`
    /// beside them are how a client shows the account's own number.
    ///
    /// The sum over the trades that could be priced. `unpriced_trades` says
    /// how many are not in it.
    pub usd: f64,
    /// `net_usd + usd`, USD. Published rather than left to the client so that
    /// two screens cannot disagree about it - and published SEPARATELY so
    /// that `net_usd` never quietly becomes this.
    pub net_of_rebate_usd: f64,
    /// Trades whose credit came from the spread actually charged. On a paper
    /// book that is every priced trade: the engine took the configured spread
    /// out of the fill, so the credit is arithmetic on a known cost and not
    /// an estimate of anything.
    pub exact_trades: usize,
    /// Trades priced from a configured spread when the spread actually paid
    /// was not recorded. Always zero on paper, and the reason this field
    /// exists here at all is so that the paper shape and the account's are
    /// the same shape - on the account it is usually most of them.
    pub estimated_trades: usize,
    /// Trades with no usable basis, credited nothing and counted instead. A
    /// total that silently dropped them would read as complete.
    pub unpriced_trades: usize,
}

/// The rebate over a book's closed trades.
///
/// `trades` is the whole book, so this total is over every closed trade and
/// not over the tail any one response carries. The per-trade figure on a
/// `TradeDto` is the same arithmetic on one trade.
fn rebate_of(trades: &[fd_backtest::Trade], rules: &TradingRules, net_usd: f64, terms: RebateTerms) -> RebateDto {
    let mut usd = 0.0;
    let mut exact = 0;
    let mut unpriced = 0;
    for t in trades {
        match terms.on(t.lots, rules.spread, rules.contract_size) {
            Some(credit) => {
                usd += credit;
                exact += 1;
            }
            None => unpriced += 1,
        }
    }
    let usd = fd_core::js_round_to(usd, 2);
    RebateDto {
        share_of_spread: terms.share_of_spread,
        spread: rules.spread,
        usd,
        net_of_rebate_usd: fd_core::js_round_to(net_usd + usd, 2),
        exact_trades: exact,
        // A paper book pays the spread the engine charged it and nothing
        // else, so nothing here is ever an estimate. The field is a zero the
        // client can rely on rather than an absence it has to special-case.
        estimated_trades: 0,
        unpriced_trades: unpriced,
    }
}

fn status_of(data: &Path, run: &PaperRun, rules: &TradingRules, guards: Option<&Guards>, live: Option<LiveBar>, rebate: Option<RebateTerms>) -> RunStatus {
    let book = &run.book;
    let metrics = book.metrics(rules);
    let open = book.position.as_ref().map(|p| OpenDto {
        side: p.side.as_str(),
        entry_time: p.entry_time,
        entry_price: p.entry_price,
        lots: p.lots,
        stop: p.stop,
        target: p.target,
        mae: fd_core::js_round_to(p.mae / p.risk, 4),
        mfe: fd_core::js_round_to(p.mfe / p.risk, 4),
        risk: p.risk,
        learned_at: run.opened_learned_at,
        unrealised_usd_at_last_close: book.unrealised_usd(rules).unwrap_or(0.0),
        usd_per_point: p.lots * rules.contract_size,
    });
    // The next blackout the run's guards would act on: the first installed
    // event at or after now with impact at or above the guards' threshold
    // **of the market's currencies** (`rules.news_currencies`; `All` events
    // count). A Canadian rate decision is not a gold run's next blackout.
    let min_impact = guards.map_or(3, |g| g.news_min_impact);
    let now = now_ms();
    let events = fd_strategy::news::events();
    let relevant = |e: &&fd_strategy::news::NewsEvent| {
        e.impact >= min_impact && e.concerns(Some(&rules.news_currencies))
    };
    let next_blackout = events
        .iter()
        .find(|e| e.time >= now && relevant(e))
        .map(|e| BlackoutDto { time: e.time, currency: e.currency.clone(), impact: e.impact, name: e.name.clone() });
    // How much calendar this run has left — and the FIRST series to run out,
    // not the last.
    //
    // The obvious version of this number is a lie. Taking the latest relevant
    // event gives 2027-12-08, because the Fed publishes its own meeting dates
    // two years ahead; meanwhile the BLS layer ends 2026-12-10, so CPI and the
    // employment report stop being blacked out in eighty-six days while the
    // desk displays four hundred and forty-nine. A long series masks a short
    // one, and the guard fails per release and not per calendar.
    //
    // So: group by event name, take the last date of each series that still
    // has a future entry, and report the SMALLEST — with the name attached,
    // because "US CPI ends in 86d" is actionable and "the calendar ends" is
    // not. A series with no future entry at all is skipped rather than
    // reported as overdue: `FOMC (unscheduled)` is a record of things that
    // happened, not a schedule, and its last entry is always in the past.
    //
    // And only a series that RECURS has a horizon at all. The live
    // ForexFactory layer is one week of whatever was on the wire, so each of
    // its names appears once or twice and every one of them "runs out" in a
    // few days by design; reported naively, the desk warns that `FOMC
    // Economic Projections ends in 1d` for ever and the real expiry is buried
    // under the noise. Twelve entries is a year of a monthly release, and it
    // separates the four scheduled series (143 to 203 entries each) from the
    // weekly feed (one or two) and from `FOMC (unscheduled)` (nine, and not a
    // schedule). A `source` field on NewsEvent would be the exact
    // discriminator; the in-memory type does not carry one, and a count is
    // both honest about what it measures and impossible to get wrong.
    const RECURRING: usize = 12;
    let mut last_of: BTreeMap<&str, i64> = BTreeMap::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut has_future: BTreeMap<&str, bool> = BTreeMap::new();
    for e in events.iter().filter(relevant) {
        let slot = last_of.entry(e.name.as_str()).or_insert(e.time);
        *slot = (*slot).max(e.time);
        *seen.entry(e.name.as_str()).or_insert(0) += 1;
        *has_future.entry(e.name.as_str()).or_insert(false) |= e.time >= now;
    }
    let soonest = last_of
        .iter()
        .filter(|(name, _)| {
            has_future.get(*name).copied().unwrap_or(false)
                && seen.get(*name).copied().unwrap_or(0) >= RECURRING
        })
        .min_by_key(|(_, t)| **t)
        .map(|(name, t)| ((*name).to_string(), *t));
    let horizon_name = soonest.as_ref().map(|(n, _)| n.clone());
    let horizon = soonest.as_ref().map(|(_, t)| *t);
    let horizon_days = horizon.map(|t| (t - now) / 86_400_000);
    let skip = book.trades.len().saturating_sub(10);
    let pending = match run.book.pending() {
        Some(Intent::Enter { side, stop, target, reason }) => Some(PendingDto {
            side: format!("{side:?}").to_uppercase(),
            stop: *stop,
            target: *target,
            reason: reason.clone(),
            decided_on: run.bars.last().map(|b| b.time),
        }),
        _ => None,
    };
    let pending_order = run.book.pending_order().map(|o| PendingOrderDto::of(o, run, rules, guards));
    RunStatus {
        id: run.config.id(),
        label: run.config.label.clone(),
        market: run.config.market.clone(),
        tf: run.config.tf.clone(),
        strategy: run.config.strategy.clone(),
        decider: run.decider.clone(),
        params: run.config.params.clone(),
        filters: run.config.filters.clone(),
        guards: run.config.guards,
        started_at: run.started_at,
        bars: run.bars.len(),
        bars_seen: run.bars_seen,
        warmup_bars: run.warmup_bars,
        last_bar_time: run.bars.last().map(|b| b.time),
        last_bar_close: run.bars.last().map(|b| b.close),
        equity: fd_core::js_round_to(book.equity, 2),
        account_currency: rules.account_currency.clone(),
        units_per_usd: rules.units_per_usd,
        leverage: rules.leverage,
        contract_size: rules.contract_size,
        open,
        pending,
        pending_order,
        trades: book.trades.len(),
        net_usd: metrics.net_pnl_usd,
        rebate: rebate.map(|terms| rebate_of(&book.trades, rules, metrics.net_pnl_usd, terms)),
        profit_factor: metrics.profit_factor,
        skipped_by_guard: book.skipped_by_guard.clone(),
        closed_by_guard: book.closed_by_guard.clone(),
        sized_down: book.sized_down_by_guard,
        skipped_no_atr: book.skipped_no_atr,
        gaps: run.gaps,
        news: NewsDto { events_loaded: events.len(), next_blackout, horizon, horizon_days, horizon_name },
        live,
        last_fills: book.trades[skip..]
            .iter()
            .map(|t| {
                TradeDto::from(t)
                    .with_rebate(rebate.and_then(|terms| terms.on(t.lots, rules.spread, rules.contract_size)))
            })
            .collect(),
        equity_curve: book.equity_curve.len(),
        brokers: brokers_of(data, &run.config.id()),
        driver: driver_of(data, &run.config.id()),
        paused: run.paused,
    }
}

/// The rules and guards a run steps with, from the config the process
/// loaded — not persisted with the run, so a config change applies at the
/// next bar, and the status reflects it.
/// The strategy's indicator series over `bars`, for the Desk's chart.
///
/// Computed on the **whole** window and returned from `from` onwards: the
/// strategy read its series over the window, so an indicator recomputed on a
/// shorter slice would be a different series — and a 55-period average would
/// be empty on a 30-bar one. Whether a series sits on the price or in its own
/// pane comes from the indicator's own definition (`Pane`), never from a guess
/// about its magnitude.
///
/// A strategy whose series cannot be built yields no overlays rather than an
/// error: a chart without lines beats no detail at all, and a bad parameter
/// was already refused at `start`.
fn overlays(
    registry: &Registry,
    config: &PaperConfig,
    rules: &TradingRules,
    bars: &[Bar],
    from: usize,
) -> (Vec<IndicatorDto>, BTreeMap<String, Vec<Point>>) {
    let mut drawn = Vec::new();
    let mut series: BTreeMap<String, Vec<Point>> = BTreeMap::new();
    if bars.is_empty() {
        return (drawn, series);
    }
    let Ok(strategy) = registry.get(&config.strategy) else {
        return (drawn, series);
    };
    let mut params = strategy.default_params();
    for (name, value) in &config.params {
        if params.contains(name) {
            params.set(name, *value);
        }
    }
    let specs = strategy.indicators(&params);
    let Ok(mut computed) = compute_indicators(bars, &specs) else {
        return (drawn, series);
    };
    // The sizing ATR is not one of the strategy's own series, but it is the
    // unit its stop is written in, so the chart may as well be able to draw it.
    ensure_fallback_atr(bars, &params, rules, &mut computed);

    for spec in &specs {
        let Some(def) = fd_indicators::definition(&spec.id) else { continue };
        // The key is built from the definition's parameters in the definition's
        // order, with the spec's overrides applied — the same way the engine
        // built it when the strategy read the series.
        let values: Vec<f64> =
            def.params.iter().map(|(name, default)| spec.params.get(*name).copied().unwrap_or(*default)).collect();
        let key = fd_indicators::indicator_key(def, &values);
        let mut outputs = Vec::new();
        for output in def.outputs {
            let qualified = format!("{key}.{output}");
            let Some(points) = computed.get(&qualified) else { continue };
            let drawn_points: Vec<Point> = bars
                .iter()
                .zip(points.iter())
                .skip(from)
                .filter(|(_, value)| value.is_finite())
                .map(|(bar, value)| Point { time: bar.time / 1000, value: *value })
                .collect();
            if drawn_points.is_empty() {
                continue;
            }
            series.insert(qualified.clone(), drawn_points);
            outputs.push(qualified);
        }
        if outputs.is_empty() {
            continue;
        }
        drawn.push(IndicatorDto {
            id: spec.id.clone(),
            key,
            params: def
                .params
                .iter()
                .map(|(name, default)| ((*name).to_string(), spec.params.get(*name).copied().unwrap_or(*default)))
                .collect(),
            outputs,
            overlay: def.pane == fd_indicators::Pane::Overlay,
        });
    }
    (drawn, series)
}

fn rules_and_guards(state: &AppState, config: &PaperConfig) -> Result<(TradingRules, Option<Guards>), ApiError> {
    let rules = state.trading_rules(&config.market)?;
    let guards = config.guards.then(|| state.guards_for(&config.market)).transpose()?;
    Ok((rules, guards))
}

/// Guard values set from the desk, each `None` meaning "leave the config's".
///
/// Every field is optional so that an edit says what it changed and nothing
/// else - a payload of whole-struct values would silently pin the fields the
/// user never touched to whatever the form happened to hold.
///
/// `max_concurrent_positions` is deliberately absent. One position per book is
/// not a risk setting here, it is an assumption the reconciler, the mirror and
/// the whole desk are built on; exposing a control that breaks them would be
/// offering a switch with no wiring behind it.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GuardEdit {
    pub max_trades_per_day: Option<usize>,
    pub daily_loss_limit_usd: Option<f64>,
    /// Minutes, because that is what a person types. Stored as ms in `Guards`.
    pub cooldown_min: Option<f64>,
    pub max_open_loss_r: Option<f64>,
    pub max_notional_pct_equity: Option<f64>,
    pub flat_before_weekend_hhmm: Option<u32>,
    pub news_flat_before_min: Option<u32>,
    pub news_flat_after_min: Option<u32>,
    pub news_min_impact: Option<u8>,
}

impl GuardEdit {
    pub fn apply(&self, g: &mut Guards) {
        if let Some(v) = self.max_trades_per_day {
            g.max_trades_per_day = v;
        }
        if let Some(v) = self.daily_loss_limit_usd {
            g.daily_loss_limit_usd = v;
        }
        if let Some(v) = self.cooldown_min {
            g.cooldown_ms = (v * 60_000.0).round() as i64;
        }
        if let Some(v) = self.max_open_loss_r {
            g.max_open_loss_r = v;
        }
        if let Some(v) = self.max_notional_pct_equity {
            g.max_notional_pct_equity = v;
        }
        if let Some(v) = self.flat_before_weekend_hhmm {
            g.flat_before_weekend_hhmm = v;
        }
        if let Some(v) = self.news_flat_before_min {
            g.news_flat_before_min = v;
        }
        if let Some(v) = self.news_flat_after_min {
            g.news_flat_after_min = v;
        }
        if let Some(v) = self.news_min_impact {
            g.news_min_impact = v;
        }
    }

    /// Rejects values that are not a setting but a mistake.
    ///
    /// Deliberately narrow. This refuses the impossible - a negative loss
    /// limit, an hour that is not on a clock - and allows everything that is
    /// merely aggressive, including turning a guard off with 0, because which
    /// risks to run is the desk owner's to decide and not this function's.
    pub fn check(&self) -> Result<(), ApiError> {
        let bad = |m: &str| Err(ApiError::BadRequest(m.to_string()));
        if self.daily_loss_limit_usd.is_some_and(|v| v < 0.0 || !v.is_finite()) {
            return bad("daily loss limit must be zero or more (0 turns it off)");
        }
        if self.cooldown_min.is_some_and(|v| v < 0.0 || !v.is_finite()) {
            return bad("cooldown must be zero or more minutes (0 turns it off)");
        }
        if self.max_open_loss_r.is_some_and(|v| v < 0.0 || !v.is_finite()) {
            return bad("open-loss cap must be zero or more R (0 turns it off)");
        }
        if self.max_notional_pct_equity.is_some_and(|v| v < 0.0 || !v.is_finite()) {
            return bad("notional cap must be zero or more percent (0 turns it off)");
        }
        if self.flat_before_weekend_hhmm.is_some_and(|v| v != 0 && (v > 2359 || v % 100 > 59)) {
            return bad("weekend-flat time must be an HHMM on the clock, or 0 to turn it off");
        }
        if self.news_min_impact.is_some_and(|v| v > 3) {
            return bad("news impact must be 0-3");
        }
        Ok(())
    }
}

/// Read the desk's guard edits, or an empty set.
///
/// A missing file is the normal case and means "the config's values stand". A
/// malformed one is treated the same rather than refusing to start: the desk
/// coming up under the CONFIGURED guards is always safe, while a desk that
/// will not come up at all is not.
pub fn load_guard_edits(dir: &Path) -> GuardEdit {
    std::fs::read_to_string(dir.join("guards.toml"))
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default()
}

/* ---------------- handlers ---------------- */

/// `POST /api/paper/start`
pub async fn start(State(state): State<Arc<AppState>>, Json(request): Json<StartRequest>) -> Result<Json<RunStatus>, ApiError> {
    if !TIMEFRAMES.contains(&request.tf.as_str()) {
        return Err(ApiError::BadRequest(format!("unknown timeframe: {}", request.tf)));
    }
    state.config.market(&request.market).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let id = request.id.unwrap_or_else(|| default_run_id(&request.market, &request.tf, &request.strategy));
    check_id(&id)?;
    let label = request.label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
    let window = request.window.max(1);
    let config = PaperConfig {
        id,
        label,
        market: request.market,
        tf: request.tf,
        strategy: request.strategy,
        params: request.params,
        filters: request.filters,
        guards: request.guards,
        window,
    };
    // Rules and guards before the strategy: the filters are scoped to the
    // market's news currencies, which the rules carry. Neither reads the
    // parameters, so the full-parameter config below needs no second look.
    let (rules, guards) = rules_and_guards(&state, &config)?;
    let (strategy, params) = resolve(&state.registry, &config, &rules.news_currencies)?;
    if strategy.needs_options() {
        return Err(ApiError::BadRequest(format!(
            "{} reads the options frame, and the paper loop has no live options timeline",
            config.strategy
        )));
    }
    let warmup = strategy.warmup(&params);
    if window < warmup + 2 {
        return Err(ApiError::BadRequest(format!(
            "window {window} is too short: {} needs {warmup} warm-up bars and one more to fill on",
            config.strategy
        )));
    }
    // The full parameters, so the state file says what ran.
    let config = PaperConfig { params: params.0.clone(), ..config };

    // Warm-up from the store: the last `window` bars, or none when the
    // store has nothing for this market yet (no file at any timeframe,
    // `ApiError::NoData`) — the poller sends the warm-up bars itself.
    let history = match state.bars(&config.market, &config.tf) {
        Ok(series) => {
            let skip = series.bars.len().saturating_sub(window);
            series.bars[skip..].to_vec()
        }
        Err(ApiError::NoData(_)) => Vec::new(),
        Err(e) => return Err(e),
    };

    let id = config.id();
    let mut runs = state.paper.lock().expect("paper runs");
    if runs.contains_key(&id) {
        return Err(ApiError::Conflict(format!("paper run `{id}` exists; stop it first or start under another id")));
    }
    let run = PaperRun {
        // A new book starts running. Switching it off is a deliberate act and
        // has to be one; a desk where runs arrive dormant would quietly grow a
        // population of books nobody notices are doing nothing.
        paused: false,
        book: PaperBook::new(&rules, strategy.exits() == Exits::Strategy),
        // Cloned from the book on its first bar; see the field's own note.
        shadow: None,
        advice: None,
        // A new book holds nothing, so there is no fill to have learned about.
        opened_learned_at: None,
        pending_decided_at: None,
        pending_htf_bar_time: None,
        open_entry: None,
        // Claimed by whoever posts the first accepted intent, never at start.
        decider: None,
        started_at: now_ms(),
        warmup_bars: history.len(),
        bars: history,
        bars_seen: 0,
        gaps: 0,
        config,
    };
    persist(&state.data, &run)?;
    record(
        &state.data,
        &id,
        &json!({
            "kind": "started",
            "time": run.started_at,
            "config": run.config,
            "guards": guards.as_ref().map(Guards::describe),
            "news": fd_strategy::news::summary("data/news/events.parquet"),
        }),
    )?;
    let status = status_of(
        &state.data,
        &run,
        &rules,
        guards.as_ref(),
        state.live_bar(&run.config.market, &run.config.tf),
        rebate_terms(&state.config_dir),
    );
    runs.insert(id, run);
    Ok(Json(status))
}

/// One run's step on a posted bar: accept, persist, record. An `Err` is
/// the run's own refusal (the bar is older than its last), reported in
/// its entry and not to the caller.
fn feed_run(state: &AppState, run: &mut PaperRun, bar: Bar) -> Result<RunBarResponse, ApiError> {
    let id = run.config.id();
    let (rules, guards) = rules_and_guards(state, &run.config)?;
    let (strategy, params) = resolve(&state.registry, &run.config, &rules.news_currencies)?;
    let bar_ms = timeframe_ms(&run.config.tf).unwrap_or(0);

    // A paused run is stepped through a strategy that cannot enter. Everything
    // else about the step is unchanged, which is the point: a paused book's
    // chart, gaps, guards and open position all behave exactly as they would
    // have, and only the entries are gone.
    let halted = Halted { inner: &strategy };
    let driver: &dyn Strategy = if run.paused { &halted } else { &strategy };

    let (report, gap, applied, shadow_closed) = match run.accept(bar, driver, &params, &rules, guards.as_ref(), bar_ms)? {
        Accepted::Seen => {
            return Ok(RunBarResponse {
                id,
                accepted: false,
                reason: Some("seen".to_string()),
                bars: run.bars.len(),
                closed: Vec::new(),
                opened: false,
                refused: None,
                gap: None,
            });
        }
        Accepted::Stepped { report, gap, advice, shadow_closed } => (report, gap, advice, shadow_closed),
    };

    // When this process learned the book holds a position, as against the bar
    // the fill is priced at. Set before `persist` so a restart keeps it, and
    // cleared the moment the book is flat so it can never describe the
    // position after the one it belongs to. See `OpenDto::learned_at`.
    let learned_at = now_ms();
    if report.opened {
        run.opened_learned_at = Some(learned_at);
        run.open_entry = Some(EntryRecord::market(bar.open));
    } else if run.book.position.is_none() {
        run.opened_learned_at = None;
    }
    // Taken, not read: `accept` above has already consumed the pending intent,
    // whether it filled or a guard refused it, so the stamp must not survive
    // into the next bar and describe a decision that is no longer waiting.
    // Unless what is waiting is an ORDER, which a closed bar never consumes:
    // its stamps stay with it until it fills, expires or is replaced.
    let (decided_at, htf_bar_time) = if run.book.pending_order().is_some() {
        (None, None)
    } else {
        (run.pending_decided_at.take(), run.pending_htf_bar_time.take())
    };
    // A closed bar has arrived while an order rests: one more waited, and
    // past `valid_bars` the order is taken back. Counted here and not in
    // `accept`, because the bar itself never fills an order and the book's
    // step must stay the engine's step.
    let expired = run.book.order_saw_bar_close();
    let expired_decided_at = expired.as_ref().and_then(|_| run.pending_decided_at.take());
    if expired.is_some() {
        run.pending_htf_bar_time = None;
    }
    // The book takes one position, so a bar closes at most one trade and it
    // is the position whose entry record the run holds - taken here so it
    // cannot describe the next position, and before the write so the state
    // on disk agrees with the row.
    let entry = (!report.trades.is_empty()).then(|| run.open_entry.take().unwrap_or_else(EntryRecord::unknown));

    persist(&state.data, run)?;
    if let Some(missing) = gap {
        record(&state.data, &id, &json!({ "kind": "gap", "time": bar.time, "missing_bars": missing }))?;
    }
    // An entry gets its own line. Until 2026-09-17 a fill appeared in
    // `fills.jsonl` only when it CLOSED, inside the trade record, so the one
    // moment worth timestamping - the desk learning it was in - was the one
    // moment the file did not record. `time` is the bar the fill is priced at
    // and `learned_at` is when this process found out; on a 15m book they are
    // a bar apart, every time.
    if report.opened {
        if let Some(p) = run.book.position.as_ref() {
            record(
                &state.data,
                &id,
                &json!({
                    "kind": "opened",
                    "time": p.entry_time,
                    "learned_at": learned_at,
                    "side": p.side.as_str(),
                    "entry_price": p.entry_price,
                    "lots": p.lots,
                    // Absent on a rule book, which decides instantly. On a
                    // model book this is AFTER `time`, and the difference is
                    // how much of the fill price the model had not yet earned.
                    "decided_at": decided_at,
                    // The H4 state the decision saw, carried from the intent
                    // to the fill so the trade record answers "what did it
                    // know" without joining two files on a timestamp.
                    "htf_bar_time": htf_bar_time,
                    // How the fill was priced - `market` here, always: this
                    // is the closed-bar path. See `EntryRecord`.
                    "entry": run.open_entry,
                    "filled_from": "bar_close",
                }),
            )?;
        }
    }
    if let Some(why) = &report.refused {
        record(&state.data, &id, &json!({ "kind": "refused", "time": bar.time, "reason": why }))?;
    }
    if let Some(order) = &expired {
        record_cancelled(&state.data, &id, order, bar.time, "expired", expired_decided_at)?;
    }
    // The advisor's own line, and the shadow book beside it. Written whether
    // the verdict changed the trade or waved it through, because "the panel
    // looked and allowed it" is the record that makes an allow answerable
    // later — a log of only the interventions cannot be scored.
    if let Some(advice) = &applied {
        record(
            &state.data,
            &id,
            &json!({
                "kind": "advice",
                "time": bar.time,
                "intent_id": advice.intent_id,
                "size_factor": advice.size_factor,
                "reason": advice.reason,
                "applied": report.advice.is_some(),
                "vetoed": advice.vetoes(),
                // The counterfactual's running total at this instant. The
                // difference between the two is the advisor's bill to date.
                "book_net_usd": fd_core::js_round_to(run.book.equity - rules.starting_equity_usd, 2),
                "shadow_net_usd": run.shadow.as_ref().map(|s| fd_core::js_round_to(s.equity - rules.starting_equity_usd, 2)),
                "shadow_trades": run.shadow.as_ref().map_or(0, |s| s.trades.len()),
            }),
        )?;
    }
    if report.no_risk {
        record(&state.data, &id, &json!({ "kind": "refused", "time": bar.time, "reason": "NO_RISK_UNIT" }))?;
    }
    // The durable history, appended once per close. `fills.jsonl` beside it is
    // written for a READER and drops `exit_kind` and `swap_usd`; this carries
    // the engine's own `Trade`, so a reload reconstructs exactly what closed.
    append_history(&state.data, &id, "main", &report.trades, run.book.equity);
    if let Some(shadow) = run.shadow.as_ref() {
        append_history(&state.data, &id, "shadow", &shadow_closed, shadow.equity);
    }
    for trade in &report.trades {
        record(&state.data, &id, &trade_row(trade, entry.as_ref()))?;
        if matches!(trade.exit_kind, fd_backtest::ExitKind::Guard(_)) {
            record(&state.data, &id, &trade_event("guard_close", trade))?;
        }
    }

    Ok(RunBarResponse {
        id,
        accepted: true,
        reason: None,
        bars: run.bars.len(),
        closed: report.trades.iter().map(TradeDto::from).collect(),
        opened: report.opened,
        refused: report.refused,
        gap,
    })
}

/// The `trade` line with how the position was entered beside the trade: a
/// closed trade's own record says nothing about whether it was a market fill
/// or an order, and that is the comparison stage 1 exists to make.
fn trade_row(trade: &Trade, entry: Option<&EntryRecord>) -> serde_json::Value {
    let mut row = trade_event("trade", trade);
    row["entry"] = json!(entry);
    row
}

/// The `cancelled_unfilled` line: an order that waited and did not fill is
/// a decision that did not become a trade, and it is COUNTED, because a
/// fill rate is a fraction and this is its denominator. `time` is on the bar
/// clock - the closed bar for an expiry, the tick's bucket otherwise - and
/// `at` is the wall clock the process wrote it, the same two clocks every
/// other line carries.
fn record_cancelled(data: &Path, id: &str, order: &PendingOrder, time: i64, reason: &str, decided_at: Option<i64>) -> Result<(), ApiError> {
    record(
        data,
        id,
        &json!({
            "kind": "cancelled_unfilled",
            "time": time,
            "at": now_ms(),
            "reason": reason,
            "entry": order_json(order),
            "decided_at": decided_at,
            "bars_waited": order.bars_waited,
        }),
    )
}

/// `POST /api/paper/open`: the open of a bar that has just started.
///
/// Deliberately not folded into `/api/paper/tick`. That route is documented as
/// "never in a run, never on disk, never read by anything in this module's
/// decision path", it is posted for streams no run is on, and it fires every
/// few seconds. A route that can open a position against a funded account
/// should be the one thing it is, and be auditable by its name.
#[derive(Debug, Deserialize)]
pub struct OpenRequest {
    pub market: String,
    pub tf: String,
    /// The bar's own stamp — the bucket it belongs to, not the moment it was
    /// read. The same `time` the closed bar will carry when it is posted.
    pub time: i64,
    pub open: f64,
}

/// What one run did with an open.
#[derive(Debug, Serialize)]
pub struct RunOpenResponse {
    pub id: String,
    /// True when this open filled a pending entry. False is the ordinary
    /// answer: most opens arrive with nothing waiting.
    pub opened: bool,
    /// The guard or advisor that refused the entry this open would have
    /// filled, if one did — the refusal happens here now, a bar earlier than
    /// it used to, and answers the same.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

/// One run, one open. The record-keeping is deliberately the same as the
/// closed-bar path's: an entry filled here writes the same `opened` line with
/// the same two clocks, so nothing downstream can tell which half filled it
/// except by the gap between them, which is the whole point.
fn feed_open(state: &AppState, run: &mut PaperRun, time: i64, open: f64) -> Result<RunOpenResponse, ApiError> {
    let id = run.config.id();
    let (rules, guards) = rules_and_guards(state, &run.config)?;
    let (strategy, params) = resolve(&state.registry, &run.config, &rules.news_currencies)?;
    let bar_ms = timeframe_ms(&run.config.tf).unwrap_or(0);
    let halted = Halted { inner: &strategy };
    let driver: &dyn Strategy = if run.paused { &halted } else { &strategy };

    let Some(report) = run.accept_open(time, open, driver, &params, &rules, guards.as_ref(), bar_ms) else {
        return Ok(RunOpenResponse { id, opened: false, refused: None });
    };

    let learned_at = now_ms();
    if report.opened {
        run.opened_learned_at = Some(learned_at);
        run.open_entry = Some(EntryRecord::market(open));
    }
    let decided_at = run.pending_decided_at.take();
    let htf_bar_time = run.pending_htf_bar_time.take();
    persist(&state.data, run)?;

    if let Some(why) = &report.refused {
        record(&state.data, &id, &json!({ "kind": "refused", "time": time, "reason": why }))?;
    }
    if report.opened {
        if let Some(p) = run.book.position.as_ref() {
            record(
                &state.data,
                &id,
                &json!({
                    "kind": "opened",
                    "time": p.entry_time,
                    "learned_at": learned_at,
                    "side": p.side.as_str(),
                    "entry_price": p.entry_price,
                    "lots": p.lots,
                    "decided_at": decided_at,
                    // The H4 state the decision saw, carried from the intent
                    // to the fill so the trade record answers "what did it
                    // know" without joining two files on a timestamp.
                    "htf_bar_time": htf_bar_time,
                    "entry": run.open_entry,
                    "filled_from": "bar_open",
                }),
            )?;
        }
    }
    Ok(RunOpenResponse { id, opened: report.opened, refused: report.refused })
}

/// What one run did with a tick that reached its resting order.
#[derive(Debug, Serialize)]
pub struct TickOrderResponse {
    pub id: String,
    /// `filled`, `refused` (the order was consumed and a guard or the advisor
    /// refused the entry) or `cancelled` (invalidated, or the book held a
    /// position the order could not add to).
    pub event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One run, one tick that its order says is a fill: the record-keeping of
/// [`feed_open`], with the order's own `entry` on the line.
///
/// `price` is the order's fill price on the bar's axis - the level for a
/// limit, the touched side for a stop - and `filled_from` says whether the
/// tick itself reached it or `pending/act` forced it.
fn feed_order_fill(
    state: &AppState,
    run: &mut PaperRun,
    live: &LiveBar,
    price: f64,
    filled_from: &'static str,
) -> Result<Option<TickOrderResponse>, ApiError> {
    let id = run.config.id();
    let (rules, guards) = rules_and_guards(state, &run.config)?;
    let (strategy, params) = resolve(&state.registry, &run.config, &rules.news_currencies)?;
    let bar_ms = timeframe_ms(&run.config.tf).unwrap_or(0);
    let halted = Halted { inner: &strategy };
    let driver: &dyn Strategy = if run.paused { &halted } else { &strategy };

    let Some((order, report)) =
        run.accept_order_fill(live.time, price, live.close, driver, &params, &rules, guards.as_ref(), bar_ms)
    else {
        return Ok(None);
    };

    let learned_at = now_ms();
    if report.opened {
        run.opened_learned_at = Some(learned_at);
        run.open_entry = Some(EntryRecord {
            entry_type: order.entry_type,
            price: Some(price),
            requested_price: Some(order.price),
        });
    }
    let decided_at = run.pending_decided_at.take();
    let htf_bar_time = run.pending_htf_bar_time.take();
    persist(&state.data, run)?;

    if let Some(why) = &report.refused {
        record(&state.data, &id, &json!({ "kind": "refused", "time": live.time, "reason": why, "entry": order_json(&order) }))?;
    }
    if report.no_risk {
        record(&state.data, &id, &json!({ "kind": "refused", "time": live.time, "reason": "NO_RISK_UNIT", "entry": order_json(&order) }))?;
    }
    if report.opened {
        if let Some(p) = run.book.position.as_ref() {
            record(
                &state.data,
                &id,
                &json!({
                    "kind": "opened",
                    // The tick's BUCKET, the bar clock every other `time`
                    // is on; the instant is `learned_at`, which for a tick
                    // fill is also when it happened.
                    "time": p.entry_time,
                    "learned_at": learned_at,
                    "side": p.side.as_str(),
                    "entry_price": p.entry_price,
                    "lots": p.lots,
                    "decided_at": decided_at,
                    "htf_bar_time": htf_bar_time,
                    "entry": run.open_entry,
                    "filled_from": filled_from,
                    "bars_waited": order.bars_waited,
                    "bid": live.bid,
                    "ask": live.ask,
                }),
            )?;
        }
    }
    let (event, reason) = match &report.refused {
        Some(why) => ("refused", Some(why.clone())),
        None if report.no_risk => ("refused", Some("NO_RISK_UNIT".to_string())),
        None => ("filled", None),
    };
    Ok(Some(TickOrderResponse { id, event, reason }))
}

/// Every run on the stream with an order resting, against one tick: cancel
/// what the tick invalidates, fill what it reaches, and extend the fill-bar
/// range of a position filled earlier in the same bucket. The ONE call the
/// tick handler makes into the order code.
///
/// A tick from a bucket the run has already closed over does nothing
/// (`accept_order_fill`'s ordering rule), which is what makes a tick and a
/// closed bar racing harmless: whichever lands second finds either the order
/// gone or the bucket stale. Nothing here writes a book that nothing changed
/// on, so a tick on a stream of rule-based runs still touches no file - the
/// test `a_tick_is_reported_and_touches_nothing` holds.
fn settle_orders_on_tick(state: &AppState, stream: &str, live: &LiveBar) -> Result<Vec<TickOrderResponse>, ApiError> {
    let mut runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::new();
    for run in runs.values_mut().filter(|r| r.config.stream() == stream) {
        if run.book.observe_tick(live.time, live.close) {
            persist(&state.data, run)?;
        }
        let Some(order) = run.book.pending_order() else { continue };
        if run.bars.last().is_none_or(|last| live.time <= last.time) {
            continue;
        }
        let id = run.config.id();
        // Invalidation is read BEFORE the fill on the same tick: a quote that
        // has gapped through both levels at once cancels rather than fills,
        // which is the order the contract states and the safer of the two.
        if order.invalidated_by(live.bid, live.ask) {
            let order = run.book.cancel_order().expect("checked");
            let decided_at = run.pending_decided_at.take();
            run.pending_htf_bar_time = None;
            persist(&state.data, run)?;
            record_cancelled(&state.data, &id, &order, live.time, "invalidated", decided_at)?;
            out.push(TickOrderResponse { id, event: "cancelled", reason: Some("invalidated".to_string()) });
            continue;
        }
        let Some(price) = order.touched(live.bid, live.ask) else { continue };
        if run.book.position.is_some() {
            // The engine refuses to fill over a position and the intent route
            // refuses to place one; this is the seam between the two, and it
            // is written down rather than left silent.
            let order = run.book.cancel_order().expect("checked");
            let decided_at = run.pending_decided_at.take();
            run.pending_htf_bar_time = None;
            persist(&state.data, run)?;
            record_cancelled(&state.data, &id, &order, live.time, "cancelled:position_open", decided_at)?;
            out.push(TickOrderResponse { id, event: "cancelled", reason: Some("position_open".to_string()) });
            continue;
        }
        if let Some(done) = feed_order_fill(state, run, live, price, "tick")? {
            out.push(done);
        }
    }
    Ok(out)
}

/// `POST /api/paper/open`
///
/// The bar stamped `time` has just opened at `open`; fill anything waiting on
/// it. Every run on the stream is offered it, and a run with nothing pending,
/// or one already past this bar, simply answers `opened: false` — an open is
/// posted four times an hour whether or not anybody is waiting for it, so
/// "nothing to do" is the ordinary answer and is not an error.
///
/// 404 when no run is on the stream, as `/api/paper/bar` does. There is no
/// 400-when-all-refused equivalent: a refusal here is a guard doing its job on
/// one run, not a malformed post.
pub async fn open_now(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !request.open.is_finite() || request.open <= 0.0 {
        return Err(ApiError::BadRequest(format!("open {} is not a price", request.open)));
    }
    if request.time <= 0 {
        return Err(ApiError::BadRequest(format!("time {} is not a bar stamp", request.time)));
    }
    let stream = stream_key(&request.market, &request.tf);
    let mut runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::new();
    for run in runs.values_mut().filter(|r| r.config.stream() == stream) {
        out.push(feed_open(&state, run, request.time, request.open)?);
    }
    if out.is_empty() {
        return Err(ApiError::NotFound(format!("no paper run for {stream}")));
    }
    Ok(Json(json!({ "time": request.time, "open": request.open, "runs": out })))
}

/// `POST /api/paper/bar`
///
/// Feeds every run on the bar's `market:tf`, in id order. One run
/// refusing the bar (older than its last) does not stop the others; each
/// entry says what its run did. The response is 404 when no run is on
/// the stream, 400 when the bar is malformed or when every run refused
/// it — a bar no one could use is the old single-run answer — and 200
/// otherwise, `accepted` saying whether anyone stepped.
pub async fn bar(State(state): State<Arc<AppState>>, Json(request): Json<BarRequest>) -> Result<Json<BarResponse>, ApiError> {
    let stream = stream_key(&request.market, &request.tf);
    let bar = Bar {
        time: request.bar.time,
        open: request.bar.open,
        high: request.bar.high,
        low: request.bar.low,
        close: request.bar.close,
        volume: request.bar.volume,
    };
    check_bar(&bar)?;
    let mut runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::new();
    let mut refusals = 0;
    for run in runs.values_mut().filter(|r| r.config.stream() == stream) {
        match feed_run(&state, run, bar) {
            Ok(entry) => out.push(entry),
            // The run's own verdict on the bar; a failure to write its
            // state is the caller's problem and stops the post.
            Err(ApiError::BadRequest(why)) => {
                refusals += 1;
                out.push(RunBarResponse {
                    id: run.config.id(),
                    accepted: false,
                    reason: Some(why),
                    bars: run.bars.len(),
                    closed: Vec::new(),
                    opened: false,
                    refused: None,
                    gap: None,
                });
            }
            Err(e) => return Err(e),
        }
    }
    if out.is_empty() {
        return Err(ApiError::NotFound(format!("no paper run for {stream}")));
    }
    if refusals == out.len() {
        let why = out.iter().find_map(|r| r.reason.clone()).unwrap_or_default();
        return Err(ApiError::BadRequest(format!("every run on {stream} refused the bar: {why}")));
    }
    let accepted = out.iter().any(|r| r.accepted);
    let reason = if accepted { None } else { out.iter().find_map(|r| r.reason.clone()) };
    let bars = out.iter().map(|r| r.bars).max().unwrap_or(0);
    Ok(Json(BarResponse { accepted, reason, bars, runs: out }))
}

/// `POST /api/paper/tick`
///
/// The bar still **forming** on a `market:tf`, for the screen - and, since
/// stage 1 of `docs/plans/2026-09-18-staged-ai-entry.md`, for the one thing a
/// resting order needs, which is a quote.
///
/// It validates the bar exactly as [`bar`] validates a closed one — finite
/// prices, `high >= low`, a timeframe the API serves — stores it in
/// `AppState::live_bars` under `market:tf` with the server's own clock, and
/// then makes ONE call, [`settle_orders_on_tick`], which is the only place a
/// tick can reach a book: a LIMIT or STOP order resting on a run of this
/// stream fills or is invalidated by the quote. Nothing else a tick carries
/// reaches a decision - the forming bar's open, high, low and close are never
/// read by a book, and the only thing that appends to a run's window is
/// [`PaperRun::accept`], from [`bar`]. The paper mutex is taken for that call
/// and for nothing else, and a stream whose runs rest no order writes no
/// file.
///
/// A stream with no run is accepted, not 404'd: whether anyone is trading a
/// symbol is not the poller's business, and a run started later wants the
/// price already there.
pub async fn tick(State(state): State<Arc<AppState>>, Json(request): Json<TickRequest>) -> Result<Json<TickResponse>, ApiError> {
    if timeframe_ms(&request.tf).is_none() {
        return Err(ApiError::BadRequest(format!("unknown timeframe: {}", request.tf)));
    }
    let bar = Bar {
        time: request.bar.time,
        open: request.bar.open,
        high: request.bar.high,
        low: request.bar.low,
        close: request.bar.close,
        volume: request.bar.volume,
    };
    check_bar(&bar)?;
    let live = LiveBar {
        time: bar.time,
        open: bar.open,
        high: bar.high,
        low: bar.low,
        close: bar.close,
        volume: bar.volume.filter(|v| v.is_finite()),
        bid: request.bid.filter(|v| v.is_finite()),
        ask: request.ask.filter(|v| v.is_finite()),
        at: now_ms(),
    };
    let key = stream_key(&request.market, &request.tf);
    state.live_bars.lock().expect("live bars").insert(key.clone(), live.clone());
    // The same quote folds into the market's minute bars (m1.rs); a second
    // poller's copy of it is recognised there and not counted twice.
    let _ = crate::m1::on_tick(&state, &request.market, &live);
    // Anyone watching hears it now rather than on their next poll. `send`
    // fails only when nobody is subscribed, which is the normal case and not
    // an error: the bar is already stored and `/status` will carry it.
    let _ = state.ticks.send(TickEvent { market: request.market, tf: request.tf, stream: key.clone(), live: live.clone() });
    let orders = settle_orders_on_tick(&state, &key, &live)?;
    Ok(Json(TickResponse { stored: true, orders }))
}

/// One forming bar, as it reached the desk.
///
/// Carries the stream it belongs to because one connection sees every market:
/// a screen filters client-side rather than opening a socket per book.
#[derive(Debug, Clone, Serialize)]
pub struct TickEvent {
    pub market: String,
    pub tf: String,
    /// `market:tf`, the same key `/status` uses.
    pub stream: String,
    pub live: LiveBar,
}

/// `GET /api/paper/stream` — every forming bar, pushed.
///
/// Server-sent events rather than a websocket. The traffic is one-way, the
/// browser reconnects on its own, it survives a proxy that does not know about
/// upgrades, and it is about fifteen lines. A websocket would buy the ability
/// to send upward, which nothing here wants: the only thing that reaches this
/// process from outside is a bar, and that already has a POST.
///
/// A keep-alive comment goes down the wire every fifteen seconds so a quiet
/// market is distinguishable from a dead connection — by the browser, which
/// reconnects, and by a reader, who would otherwise be looking at a stopped
/// clock with no way to tell.
pub async fn stream(
    State(state): State<Arc<AppState>>,
) -> axum::response::Sse<impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    use axum::response::sse::{Event, KeepAlive};
    use tokio_stream::StreamExt as _;

    let rx = state.ticks.subscribe();
    let events = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|tick| {
        // A lagged receiver is a browser that fell behind; it is dropped and
        // picks up from the newest tick, which is what a price wants.
        let tick = tick.ok()?;
        Some(Ok(Event::default().event("tick").json_data(&tick).ok()?))
    });
    axum::response::Sse::new(events).keep_alive(KeepAlive::default().interval(std::time::Duration::from_secs(15)))
}

/// `POST /api/paper/stop`
pub async fn stop(State(state): State<Arc<AppState>>, Json(request): Json<StopRequest>) -> Result<Json<StopResponse>, ApiError> {
    let mut runs = state.paper.lock().expect("paper runs");
    let id = match (request.id, request.market, request.tf) {
        (Some(id), _, _) => id,
        (None, Some(market), Some(tf)) => {
            let stream = stream_key(&market, &tf);
            let mut on_stream = runs.keys().filter(|id| runs[*id].config.stream() == stream);
            match (on_stream.next(), on_stream.next()) {
                (Some(only), None) => only.clone(),
                (None, _) => return Err(ApiError::NotFound(format!("no paper run for {stream}"))),
                (Some(_), Some(_)) => {
                    let ids: Vec<&String> = runs.keys().filter(|id| runs[*id].config.stream() == stream).collect();
                    return Err(ApiError::Conflict(format!(
                        "{} paper runs on {stream}; say which by id: {}",
                        ids.len(),
                        ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                    )));
                }
            }
        }
        _ => return Err(ApiError::BadRequest("stop needs an id, or a market and tf".to_string())),
    };
    let mut run = runs.remove(&id).ok_or_else(|| ApiError::NotFound(format!("no paper run `{id}`")))?;
    let (rules, _) = match rules_and_guards(&state, &run.config) {
        Ok(found) => found,
        Err(e) => {
            runs.insert(id, run);
            return Err(e);
        }
    };
    // An order resting on a stopped run is withdrawn, and says so: the row is
    // the only record that it waited.
    if let Some(order) = run.book.cancel_order() {
        let decided_at = run.pending_decided_at.take();
        record_cancelled(&state.data, &id, &order, run.bars.last().map_or(0, |b| b.time), "cancelled:stopped", decided_at)?;
    }
    let closed = run.book.stop(&rules);
    if let Some(trade) = &closed {
        let entry = run.open_entry.take().unwrap_or_else(EntryRecord::unknown);
        record(&state.data, &id, &trade_row(trade, Some(&entry)))?;
    }
    let metrics = run.book.metrics(&rules);
    record(
        &state.data,
        &id,
        &json!({ "kind": "stopped", "time": now_ms(), "trades": run.book.trades.len(), "equity": run.book.equity, "net_usd": metrics.net_pnl_usd }),
    )?;
    // The final book stays readable as `final.json`; `state.json` goes, so a
    // restart does not resurrect a run that was stopped.
    let dir = run_dir(&state.data, &id);
    let text = serde_json::to_string(&run).map_err(|e| ApiError::Internal(format!("paper: serialise: {e}")))?;
    std::fs::write(dir.join("final.json"), text).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", dir.display())))?;
    match std::fs::remove_file(dir.join("state.json")) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(ApiError::Internal(format!("paper: {}: {e}", dir.display()))),
    }
    Ok(Json(StopResponse {
        id,
        stopped: true,
        closed: closed.as_ref().map(TradeDto::from),
        trades: run.book.trades.len(),
        equity: fd_core::js_round_to(run.book.equity, 2),
        net_usd: metrics.net_pnl_usd,
    }))
}

/// One `[[account]]` block of `config/accounts.toml`.
///
/// The registry is read per REQUEST rather than loaded at startup, so adding
/// an account is editing one file - not editing a file and restarting a desk
/// that is carrying live positions.
#[derive(Debug, Deserialize)]
struct AccountSpec {
    id: String,
    label: Option<String>,
    login: i64,
    server: Option<String>,
    #[serde(default = "yes")]
    enabled: bool,
    /// The registry's intent. Whether an executor is actually running dry is
    /// its own business and is reported separately - see [`AccountDto`].
    #[serde(default = "yes")]
    dry_run: bool,
    /// Whether this account is permitted to spend real money.
    ///
    /// Read here so the desk can SAY so, in as many words, on a screen that
    /// otherwise looks identical to a demo. It is NOT the permission: that
    /// lives in `mt5_executor.py`, which reads this same file for itself and
    /// additionally demands `--allow-real` on its command line. A screen
    /// showing a flag that something else enforced elsewhere would be a screen
    /// worth distrusting, so both read the one file.
    #[serde(default)]
    real_money: bool,
    /// Terminal volume = the book's lots x this. Shown because on a funded
    /// account it is the number that decides what being wrong costs.
    #[serde(default = "one")]
    lot_scale: f64,
    #[serde(default)]
    runs: Vec<String>,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct AccountFile {
    #[serde(default)]
    account: Vec<AccountSpec>,
}

/// One broker account: what the registry says it should be, and what it is.
///
/// The two are kept apart on purpose. `runs` is the books the registry names;
/// `mirroring` is the books whose executor is actually reporting right now.
/// An account configured and not running shows as itself with `at: 0` and an
/// empty `mirroring`, which is a state worth seeing - it is how a mirror that
/// died at three in the morning looks.
#[derive(Debug, Serialize)]
pub struct AccountDto {
    /// The registry id, or `login-<n>` for an executor started outside it.
    pub id: String,
    pub label: String,
    pub login: i64,
    pub server: Option<String>,
    /// False when no `[[account]]` block claims this login - someone started
    /// an executor by hand. Shown rather than hidden: an account trading
    /// without an entry in the registry is exactly the thing the registry
    /// exists to make visible.
    pub configured: bool,
    pub enabled: bool,
    /// What `config/accounts.toml` says this account should mirror.
    pub runs: Vec<String>,
    pub demo: Option<bool>,
    pub currency: Option<String>,
    pub balance: Option<f64>,
    pub equity: Option<f64>,
    pub margin: Option<f64>,
    pub margin_level: Option<f64>,
    /// True when EVERY reporting book here is in dry run - the account is
    /// connected and watching, but nothing it shows was ever sent. Falls back
    /// to the registry's intent when nothing is reporting.
    pub dry_run: bool,
    /// What the registry permits, not what is happening. An account can be
    /// `real_money` and flat, or `real_money` and dry - the flag says only
    /// that nothing in the configuration stands between this account and a
    /// real order. Shown on its own for that reason.
    pub real_money: bool,
    /// Terminal volume = the book's lots x this, from the registry.
    pub lot_scale: f64,
    /// The newest snapshot across this account's books; 0 when none has ever
    /// reported. Whether that counts as connected is the client's call.
    pub at: i64,
    /// Books whose executor is reporting into this account.
    pub mirroring: Vec<String>,
    /// How many of those hold a position on the account.
    pub positions: usize,
}

/// The `[[account]]` blocks, or an empty list.
///
/// A missing or malformed registry is not an error here. The desk must still
/// list the accounts that are actually reporting - which it can do from the
/// snapshots alone - and a broken config file should cost the labels, not the
/// view of what is trading.
fn account_registry(dir: &Path) -> Vec<AccountSpec> {
    let Ok(text) = std::fs::read_to_string(dir.join("accounts.toml")) else { return Vec::new() };
    toml::from_str::<AccountFile>(&text).map(|f| f.account).unwrap_or_default()
}

/// How far behind the freshest snapshot on an account a book may be and still
/// count as mirroring it.
///
/// Every executor on one account polls at the same cadence, so a book more than
/// a few polls behind the newest one has stopped rather than slowed. This is a
/// RELATIVE window on purpose: it says "this book stopped while its neighbours
/// kept going", which is true whether the whole desk has been down for an hour
/// or is running normally. Whether the ACCOUNT itself is live is a separate
/// question the client answers from `at`.
const MIRROR_LAG_MS: i64 = 60_000;

/// `GET /api/paper/accounts` - the broker side, one entry per account.
///
/// Separate from `/status` because it answers a question that is not about any
/// one book: which accounts exist, and is anything connected. The app bar asks
/// it on every screen, including the ones that never load a book, so it is
/// kept small deliberately - an account summary, not a copy of every run.
pub async fn accounts(State(state): State<Arc<AppState>>) -> Result<Json<AccountsResponse>, ApiError> {
    // Collected first, then summarised, because `mirroring` is decided against
    // the freshest snapshot on the account and that is not known until every
    // book has been read. Reported as they were found, a book whose executor
    // stopped hours ago counted the same as one reporting now - the desk said
    // "mirroring 8/6" after two books were dropped from the registry and left
    // their last file behind.
    let seen: Vec<(String, BrokerDto)> = {
        let runs = state.paper.lock().expect("paper runs");
        runs.values()
            .flat_map(|run| {
                let id = run.config.id();
                brokers_of(&state.data, &id).into_iter().map(move |b| (id.clone(), b))
            })
            .collect()
    };
    let mut newest: BTreeMap<i64, i64> = BTreeMap::new();
    for (_, b) in &seen {
        if let Some(login) = b.login {
            let slot = newest.entry(login).or_insert(b.at);
            *slot = (*slot).max(b.at);
        }
    }

    let mut by_login: BTreeMap<i64, AccountDto> = BTreeMap::new();

    // The registry first, so a configured account appears whether or not
    // anything is running it.
    for spec in account_registry(&state.config_dir) {
        by_login.entry(spec.login).or_insert(AccountDto {
            label: spec.label.unwrap_or_else(|| spec.id.clone()),
            id: spec.id,
            login: spec.login,
            server: spec.server,
            configured: true,
            enabled: spec.enabled,
            runs: spec.runs,
            demo: None,
            currency: None,
            balance: None,
            equity: None,
            margin: None,
            margin_level: None,
            dry_run: spec.dry_run,
            real_money: spec.real_money,
            lot_scale: spec.lot_scale,
            at: 0,
            mirroring: Vec::new(),
            positions: 0,
        });
    }

    // Then what is actually reporting, which overwrites the registry's guesses
    // about the account and never its intent.
    let mut said: BTreeMap<i64, bool> = BTreeMap::new();
    for (id, b) in seen {
        let Some(login) = b.login else { continue };
        let entry = by_login.entry(login).or_insert_with(|| AccountDto {
            id: b.account.clone(),
            label: b.account.clone(),
            login,
            server: b.server.clone(),
            configured: false,
            enabled: true,
            runs: Vec::new(),
            demo: b.demo,
            currency: b.currency.clone(),
            balance: b.balance,
            equity: b.equity,
            margin: b.margin,
            margin_level: b.margin_level,
            dry_run: true,
            // An executor running outside the registry has no entry granting
            // it anything, so nothing here claims it was permitted. What it
            // IS - demo or real - comes from the snapshot's `demo` field
            // above, which is the account itself talking.
            real_money: false,
            lot_scale: 1.0,
            at: 0,
            mirroring: Vec::new(),
            positions: 0,
        });
        // A book left behind by a stopped executor describes an account as it
        // was, so it must not contribute to how the account is now.
        if b.at < newest.get(&login).copied().unwrap_or(0) - MIRROR_LAG_MS {
            continue;
        }
        // The freshest snapshot wins the account's numbers: two executors on
        // one account both report the same balance, and the older one would
        // otherwise overwrite the newer with a remembered figure.
        if b.at > entry.at {
            entry.at = b.at;
            entry.balance = b.balance;
            entry.equity = b.equity;
            entry.margin = b.margin;
            entry.margin_level = b.margin_level;
            entry.demo = b.demo;
            entry.currency = b.currency.clone();
            if entry.server.is_none() {
                entry.server = b.server.clone();
            }
        }
        // Reset the registry's intent the first time a live executor speaks for
        // this account, then AND across the rest: one book sending real orders
        // makes the account a live one, however many others are only watching.
        let told = said.entry(login).or_insert(false);
        if !*told {
            entry.dry_run = true;
            *told = true;
        }
        entry.dry_run &= b.dry_run.unwrap_or(false);
        if b.position.is_some() {
            entry.positions += 1;
        }
        entry.mirroring.push(id);
    }
    for a in by_login.values_mut() {
        a.mirroring.sort();
    }
    Ok(Json(AccountsResponse { accounts: by_login.into_values().collect() }))
}

#[derive(Debug, Serialize)]
pub struct AccountsResponse {
    pub accounts: Vec<AccountDto>,
}

#[derive(Debug, Deserialize)]
pub struct PauseRequest {
    pub run: String,
    pub paused: bool,
}

#[derive(Debug, Serialize)]
pub struct PauseResponse {
    pub id: String,
    pub paused: bool,
    /// True when the book is holding a position at the moment it was paused.
    ///
    /// Worth answering in the response rather than leaving to be discovered:
    /// pausing does NOT close it, and somebody flicking the switch to stop
    /// trading should be told immediately that a trade is still live.
    pub holding: bool,
}

/// `POST /api/paper/pause` - turn one book off, or back on.
///
/// Persisted, so it survives a restart of this process. A book that was
/// switched off on Friday is still off on Monday, which is the only behaviour
/// that makes the switch trustworthy.
pub async fn pause(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PauseRequest>,
) -> Result<Json<PauseResponse>, ApiError> {
    let mut runs = state.paper.lock().expect("paper runs");
    let run = runs
        .get_mut(&body.run)
        .ok_or_else(|| ApiError::NotFound(format!("no paper run `{}`", body.run)))?;
    let changed = run.paused != body.paused;
    run.paused = body.paused;
    let holding = run.book.position.is_some();
    let id = run.config.id();
    persist(&state.data, run)?;
    if changed {
        record(
            &state.data,
            &id,
            &json!({
                "kind": if body.paused { "paused" } else { "resumed" },
                "time": now_ms(),
                // Recorded because it changes how the book's next lines read:
                // a pause taken while holding is followed by an exit that the
                // strategy chose under a switch that was already off.
                "holding": holding,
            }),
        )?;
    }
    Ok(Json(PauseResponse { id, paused: body.paused, holding }))
}

/// One guard as the desk shows it: what it is now, and what the file says.
#[derive(Debug, Serialize)]
pub struct GuardsView {
    /// The values in force, config plus edits.
    pub effective: GuardValues,
    /// The values `config/default.toml` holds, so the panel can offer "put it
    /// back" without the client having to remember what back was.
    pub configured: GuardValues,
    /// The edits themselves - which fields the desk has taken over.
    pub edited: GuardEdit,
    /// Books running under these guards right now.
    pub guarded_runs: usize,
}

#[derive(Debug, Serialize)]
pub struct GuardValues {
    pub max_concurrent_positions: usize,
    pub max_trades_per_day: usize,
    pub daily_loss_limit_usd: f64,
    pub cooldown_min: f64,
    pub max_open_loss_r: f64,
    pub max_notional_pct_equity: f64,
    pub flat_before_weekend_hhmm: u32,
    pub news_flat_before_min: u32,
    pub news_flat_after_min: u32,
    pub news_min_impact: u8,
}

impl From<&Guards> for GuardValues {
    fn from(g: &Guards) -> Self {
        Self {
            max_concurrent_positions: g.max_concurrent_positions,
            max_trades_per_day: g.max_trades_per_day,
            daily_loss_limit_usd: g.daily_loss_limit_usd,
            cooldown_min: fd_core::js_round_to(g.cooldown_ms as f64 / 60_000.0, 2),
            max_open_loss_r: g.max_open_loss_r,
            max_notional_pct_equity: g.max_notional_pct_equity,
            flat_before_weekend_hhmm: g.flat_before_weekend_hhmm,
            news_flat_before_min: g.news_flat_before_min,
            news_flat_after_min: g.news_flat_after_min,
            news_min_impact: g.news_min_impact,
        }
    }
}

/// The market the panel reads guards from.
///
/// Guards are global except for `news_currencies`, which is per market, so one
/// has to be named to show a complete set. The desk's own default market is
/// used and the client may ask for another; either way every value on the
/// panel except the currency list is the same for every book.
fn guard_market(state: &AppState) -> String {
    state.config.market.clone()
}

/// `GET /api/paper/guards`
pub async fn guards_get(State(state): State<Arc<AppState>>) -> Result<Json<GuardsView>, ApiError> {
    let market = guard_market(&state);
    let configured = Guards::for_market(&state.config, &market)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let effective = state.guards_for(&market)?;
    let guarded_runs = state.paper.lock().expect("paper runs").values().filter(|r| r.config.guards).count();
    Ok(Json(GuardsView {
        effective: (&effective).into(),
        configured: (&configured).into(),
        edited: state.guard_edits.read().expect("guard edits").clone(),
        guarded_runs,
    }))
}

/// `POST /api/paper/guards` - change the guards every guarded book runs under.
///
/// Two things happen besides the change itself, and both matter more than it
/// does.
///
/// It is WRITTEN INTO EVERY GUARDED BOOK'S RECORD. A book's numbers only mean
/// something under a stated set of rules, and a desk where the rules could move
/// without leaving a mark would produce a P&L curve that cannot be read: trades
/// before and after the change were taken under different constraints and
/// nothing would say where the line is. The event carries the values, so the
/// record is self-contained.
///
/// And it is persisted to `config/guards.toml`, so it survives a restart.
/// Guards that quietly reverted to the file's values on the next restart would
/// be worse than no control at all - the desk would be running rules nobody
/// chose and everybody assumed.
pub async fn guards_set(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GuardEdit>,
) -> Result<Json<GuardsView>, ApiError> {
    body.check()?;

    let before = state.guards_for(&guard_market(&state))?;
    // The lock is taken, used and released before anything is awaited: a guard
    // held across an await makes the whole handler non-Send and axum refuses
    // it, which is the compiler catching a real hazard rather than a nuisance.
    let unchanged = {
        let mut edits = state.guard_edits.write().expect("guard edits");
        let same = *edits == body;
        if !same {
            *edits = body.clone();
        }
        same
    };
    if unchanged {
        return guards_get(State(state)).await;
    }
    let after = state.guards_for(&guard_market(&state))?;

    let path = state.config_dir.join("guards.toml");
    let text = toml::to_string_pretty(&body).map_err(|e| ApiError::Internal(format!("guards: {e}")))?;
    const HEADER: &str = concat!(
        "# Guard values changed from the desk. Written by the API, not by hand.\n",
        "# `config/default.toml` holds the standing values and the reasoning behind\n",
        "# each one; this file is only what the desk has taken over. Deleting it puts\n",
        "# every guard back to the file's value.\n\n",
    );
    std::fs::write(&path, format!("{HEADER}{text}"))
    .map_err(|e| ApiError::Internal(format!("guards: {}: {e}", path.display())))?;

    // Every guarded book is told, by id, in its own file. A single central log
    // would be one more place to remember to look when reading a book later.
    let ids: Vec<String> = {
        let runs = state.paper.lock().expect("paper runs");
        runs.values().filter(|r| r.config.guards).map(|r| r.config.id()).collect()
    };
    let entry = json!({
        "kind": "guards_changed",
        "time": now_ms(),
        "from": GuardValues::from(&before),
        "to": GuardValues::from(&after),
    });
    for id in &ids {
        record(&state.data, id, &entry)?;
    }

    guards_get(State(state)).await
}

/// `GET /api/paper/status`
pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<StatusResponse>, ApiError> {
    let runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::with_capacity(runs.len());
    // Read once for the whole response rather than once per run: the terms
    // are one file and one arrangement, and ten books quoting ten reads of it
    // could disagree with each other mid-edit.
    let rebate = rebate_terms(&state.config_dir);
    for run in runs.values() {
        let (rules, guards) = rules_and_guards(&state, &run.config)?;
        let live = state.live_bar(&run.config.market, &run.config.tf);
        out.push(status_of(&state.data, run, &rules, guards.as_ref(), live, rebate));
    }
    Ok(Json(StatusResponse { runs: out }))
}

/// `GET /api/paper/run/{id}?bars=N`
///
/// One run in full for the Desk's drill-down: the status entry, the equity
/// curve, every closed trade (the last 500), the event lines (the last
/// 200) and the tail of the bar window (`bars`, default 120, at most the
/// window). 404 for an id no run has.
pub async fn detail(
    State(state): State<Arc<AppState>>,
    PathParam(id): PathParam<String>,
    Query(query): Query<DetailQuery>,
) -> Result<Json<RunDetail>, ApiError> {
    let runs = state.paper.lock().expect("paper runs");
    let run = runs.get(&id).ok_or_else(|| ApiError::NotFound(format!("no paper run `{id}`")))?;
    let (rules, guards) = rules_and_guards(&state, &run.config)?;
    let live = state.live_bar(&run.config.market, &run.config.tf);
    let rebate = rebate_terms(&state.config_dir);
    let status = status_of(&state.data, run, &rules, guards.as_ref(), live.clone(), rebate);

    let book = &run.book;
    let mut equity_curve = Vec::with_capacity(book.equity_curve.len() + 1);
    equity_curve.push((run.started_at, rules.starting_equity_usd));
    equity_curve.extend_from_slice(&book.equity_curve);

    let skip = book.trades.len().saturating_sub(MAX_DETAIL_FILLS);
    let fills = book.trades[skip..]
        .iter()
        .map(|t| {
            TradeDto::from(t).with_rebate(rebate.and_then(|terms| terms.on(t.lots, rules.spread, rules.contract_size)))
        })
        .collect();

    let events = events_of(&state.data, &id, MAX_DETAIL_EVENTS);

    let wanted = query.bars.unwrap_or(DEFAULT_DETAIL_BARS).min(run.config.window.max(1));
    let skip = run.bars.len().saturating_sub(wanted);
    let bars = run.bars[skip..].iter().map(|b| (b.time, b.open, b.high, b.low, b.close)).collect();
    // The indicators are computed on the whole window and only then cut to the
    // returned bars: an indicator read on a short slice is a different series
    // from the one the strategy read, and a long period would be empty on it.
    let (indicators, series) = overlays(&state.registry, &run.config, &rules, &run.bars, skip);

    Ok(Json(RunDetail { run: status, live, equity_curve, fills, events, bars, indicators, series }))
}

/* ------------------------------------------------ the advisor's two routes */

/// One run with an entry waiting to fill, as an advisor needs to see it.
///
/// Everything here is already known to the run: nothing is computed for the
/// advisor's benefit and nothing is hidden from it. The one thing it never
/// receives is a way to act — there is no field on the reply it posts back in
/// which a side, a price, or a larger size could be expressed.
#[derive(Debug, Serialize)]
pub struct PendingEntry {
    pub run: String,
    pub intent_id: String,
    pub market: String,
    pub tf: String,
    pub strategy: String,
    pub label: Option<String>,
    pub params: BTreeMap<String, f64>,
    pub filters: Vec<String>,
    /// LONG or SHORT, the stop and target the strategy set, and the sentence
    /// it wrote for taking the trade.
    pub side: String,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    pub reason: String,
    /// The bar whose close produced the intent; the fill is the next open.
    pub signal_bar: (i64, f64, f64, f64, f64),
    /// The tail of the window, oldest first, so a model can see the shape.
    pub bars: Vec<(i64, f64, f64, f64, f64)>,
    /// What this book has done so far, and what the unadvised one has.
    pub book_trades: usize,
    pub book_net_usd: f64,
    pub shadow_trades: usize,
    pub shadow_net_usd: f64,
    /// A verdict already posted for this same intent, if one has been.
    pub advised: bool,
    /// Set when the entry is an order resting at a price rather than a
    /// market intent waiting for the open; `side`, `stop`, `target` and
    /// `reason` above are then the order's, and `intent_id` is the order's
    /// own name.
    pub pending_order: Option<PendingOrderDto>,
}

/// `GET /api/paper/pending` — every run whose next bar would open a trade.
///
/// The advisor polls this. It is deliberately a poll and not a push: the model
/// lives outside this process, on the other side of a socket that can be down
/// for an hour without the desk noticing, and a bar that fills unadvised is
/// the correct behaviour rather than an error to retry.
pub async fn pending(State(state): State<Arc<AppState>>) -> Result<Json<Vec<PendingEntry>>, ApiError> {
    let runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::new();
    for run in runs.values() {
        // An order resting at a price, or a market intent waiting for the
        // open: one entry either way, and the advisor answers with the id it
        // is given here.
        let (rules, guards) = rules_and_guards(&state, &run.config)?;
        let (side, stop, target, reason, intent_id, pending_order) = match (run.book.pending_order(), run.book.pending()) {
            (Some(order), _) => (
                order.side.as_str().to_string(),
                order.stop,
                order.target,
                order.reason.clone(),
                run.order_id().expect("an order rests"),
                Some(PendingOrderDto::of(order, run, &rules, guards.as_ref())),
            ),
            (None, Some(Intent::Enter { side, stop, target, reason })) => {
                (format!("{side:?}").to_uppercase(), *stop, *target, reason.clone(), run.pending_id(), None)
            }
            _ => continue,
        };
        let Some(last) = run.bars.last() else { continue };
        let tail: Vec<_> =
            run.bars.iter().rev().take(120).rev().map(|b| (b.time, b.open, b.high, b.low, b.close)).collect();
        out.push(PendingEntry {
            run: run.config.id(),
            advised: run.advice.as_ref().is_some_and(|a| a.intent_id == intent_id),
            intent_id,
            pending_order,
            market: run.config.market.clone(),
            tf: run.config.tf.clone(),
            strategy: run.config.strategy.clone(),
            label: run.config.label.clone(),
            params: run.config.params.clone(),
            filters: run.config.filters.clone(),
            side,
            stop,
            target,
            reason,
            signal_bar: (last.time, last.open, last.high, last.low, last.close),
            bars: tail,
            book_trades: run.book.trades.len(),
            book_net_usd: fd_core::js_round_to(run.book.equity - rules.starting_equity_usd, 2),
            shadow_trades: run.shadow.as_ref().map_or(0, |s| s.trades.len()),
            shadow_net_usd: run
                .shadow
                .as_ref()
                .map_or(0.0, |s| fd_core::js_round_to(s.equity - rules.starting_equity_usd, 2)),
        });
    }
    Ok(Json(out))
}

/// One agent's turn in the panel, exactly as it happened.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Turn {
    /// Which advisor spoke: `risk`, `news`, `arbiter`.
    pub agent: String,
    pub model: String,
    /// The prompt as sent and the reply as received, both whole.
    ///
    /// Stored unabridged on purpose. A summary of a prompt cannot be replayed
    /// against a changed prompt, and replay is the only way a past mistake
    /// gets fixed rather than merely counted.
    pub prompt: String,
    pub response: String,
    #[serde(default)]
    pub latency_ms: i64,
    /// What this agent alone would have done, before the panel reconciled.
    #[serde(default = "one")]
    pub size_factor: f64,
    #[serde(default)]
    pub reason: String,
}

fn one() -> f64 {
    1.0
}

/// `POST /api/paper/advice` — a panel's verdict on one pending intent.
#[derive(Debug, Deserialize)]
pub struct AdviceRequest {
    pub run: String,
    pub intent_id: String,
    /// The panel's reconciled number. Clamped to `[0, 1]` on arrival, so a
    /// service that tries to scale a trade up is capped rather than trusted:
    /// this route is incapable of making a position larger than the strategy
    /// asked for.
    pub size_factor: f64,
    pub reason: String,
    /// Every turn, in order. May be empty for a rule-based advisor.
    #[serde(default)]
    pub transcript: Vec<Turn>,
}

#[derive(Debug, Serialize)]
pub struct AdviceResponse {
    pub accepted: bool,
    /// The number actually stored, after clamping.
    pub size_factor: f64,
    pub reason: String,
}

pub async fn advice(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AdviceRequest>,
) -> Result<Json<AdviceResponse>, ApiError> {
    let mut runs = state.paper.lock().expect("paper runs");
    let run =
        runs.get_mut(&body.run).ok_or_else(|| ApiError::NotFound(format!("no paper run `{}`", body.run)))?;

    let verdict = Advice::new(body.intent_id.clone(), body.size_factor, body.reason.clone());
    // Stale or mistaken: the intent this names is not the one pending. Logged
    // anyway and applied to nothing — a verdict that arrived too late is a
    // fact about the advisor's latency and is worth keeping.
    // A resting order is addressed by its own id, which does not move as bars
    // arrive; a market intent by the bar it was decided on, as before.
    let current = run.order_id().unwrap_or_else(|| run.pending_id());
    let fresh = current == body.intent_id
        && (run.book.pending_order().is_some() || matches!(run.book.pending(), Some(Intent::Enter { .. })));

    log_consultation(
        &state.data,
        &body.run,
        &json!({
            "kind": "consultation",
            "at": now_ms(),
            "intent_id": body.intent_id,
            "pending_now": current,
            "applied": fresh,
            "size_factor": verdict.size_factor,
            "raw_size_factor": body.size_factor,
            "reason": verdict.reason,
            "transcript": body.transcript,
        }),
    )?;

    if fresh {
        run.advice = Some(verdict.clone());
        // Same reason as the intent route: a verdict the advisor was told was
        // accepted must still be there when the intent it judges fills.
        persist(&state.data, run)?;
    }
    Ok(Json(AdviceResponse { accepted: fresh, size_factor: verdict.size_factor, reason: verdict.reason }))
}

/// Who has actually been posting a run's entries.
///
/// **Earned, not declared.** Nothing writes this at `start`: it is written
/// only when an intent is ACCEPTED, so a name here means a decision this book
/// really took, on a bar it really was on. A run configured as `external`
/// that nobody ever posted to carries `None`, which is the truth about it —
/// the alternative, a model name typed into a config field, would put a
/// gpt-5 badge on a book gpt-5 never traded.
///
/// `decisions` is a map and not a counter on purpose. If two different things
/// drive one book — a model swapped mid-campaign, a coin posting to the
/// model's run by mistake — the book's net is not attributable to either of
/// them, and a single `name` field would hide exactly that. More than one key
/// here means the number on this row cannot be read as one decider's record.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Decider {
    /// What the most recent poster called itself: a model id (`gpt-5`), or
    /// `coin` for the control book.
    pub last: String,
    /// Epoch ms of the last accepted intent.
    pub last_at: i64,
    /// Accepted ENTRIES per decider name, over the life of the run.
    pub decisions: BTreeMap<String, usize>,
    /// Times a decider looked at a bar and asked for nothing.
    ///
    /// Counted because without it the desk cannot tell a model that is
    /// standing aside from a model that is not running at all — both show
    /// zero trades, and on an instrument that spends most of its day going
    /// nowhere, standing aside is the common correct answer. `last_at` moves
    /// on a stand-aside too, so a badge that has gone quiet means the process
    /// has, not that the market did.
    pub stood_aside: usize,
}

/// `POST /api/paper/intent` — an entry proposed from outside the process.
///
/// The mailbox for a run whose strategy is `external`. See
/// `crates/fd-strategy/src/external.rs` for why a decider that needs a network
/// call cannot live inside `on_bar`, and `docs/paper/AI-TRADER.md` for the
/// campaign this exists to run.
///
/// What it cannot do is as important as what it can:
///
/// * It **cannot act on the bar it was shown.** The intent goes into the same
///   `pending` slot every rule uses and fills at the NEXT bar's open.
/// * It **cannot arrive late and still trade.** `bar_time` names the bar the
///   decision was made on; if that bar is no longer the run's last, the run
///   has moved on and the intent is refused. Slowness costs a trade, never a
///   bad fill.
/// * It **cannot set an unbounded trade.** The run's strategy is
///   `Exits::Engine`, so the stop, the target and the maximum hold belong to
///   the desk.
/// * It **cannot reach a broker.** This is a paper book. The only code in this
///   repository that can send an order is `py/live/mt5_executor.py`, and it
///   refuses any account that is not a demo.
#[derive(Debug, Deserialize)]
pub struct IntentRequest {
    pub run: String,
    /// The bar the decision was made on, epoch ms — the run's last bar.
    pub bar_time: i64,
    /// `LONG` or `SHORT`. There is no third value and no way to express a size.
    pub side: String,
    pub stop: Option<f64>,
    pub target: Option<f64>,
    #[serde(default)]
    pub reason: String,
    /// What the poster calls itself — a model id (`gpt-5`) or `coin`. Recorded
    /// on the run only when the intent is accepted, and shown on the desk so a
    /// book driven by a model is never mistaken for a rule. Absent posts still
    /// trade; they just leave the badge unclaimed.
    #[serde(default)]
    pub decider: Option<String>,
    /// The H4 bar whose facts this decision READ, epoch ms — the
    /// `h4.computed_at_bar_ms` the decider fetched from `/api/paper/htf`.
    ///
    /// Recorded so the book's own record says which higher-timeframe state
    /// each decision saw, instead of leaving it to be reconstructed later from
    /// two logs and a guess about latency. A decider that read no HTF context
    /// omits it, and that absence is the honest reading: an experiment
    /// comparing context-present rows against context-absent ones cannot tell
    /// them apart if a missing fetch looks the same as a fetch that returned
    /// nothing.
    #[serde(default)]
    pub htf_bar_time: Option<i64>,
    /// How to enter. Absent, or `market`, is today's path: the intent fills at
    /// the next bar's open (or at the open the poller posts). `limit` and
    /// `stop` rest an order at `price` and fill from the tick feed - see
    /// [`PendingOrder`] for the fill rule. Stage 1 of
    /// `docs/plans/2026-09-18-staged-ai-entry.md`.
    #[serde(default)]
    pub entry: Option<EntryIn>,
    /// `[lo, hi]`: where the decider says the entry is still valid.
    /// Informational; stored on the row and shown, never acted on.
    #[serde(default)]
    pub zone: Option<(f64, f64)>,
    /// Orders only. Cancelled when this many CLOSED bars have arrived after
    /// the decision bar without a fill. Default 2; must be at least 1.
    #[serde(default)]
    pub valid_bars: Option<u32>,
    /// Orders only. A tick beyond either cancels the order before it fills:
    /// the ask above the first, the bid below the second.
    #[serde(default)]
    pub invalidate_above: Option<f64>,
    #[serde(default)]
    pub invalidate_below: Option<f64>,
}

/// The `entry` object of an intent: `{"type": "market"|"limit"|"stop",
/// "price": <quote>|null}`. `price` is on the bar's axis, before the entry
/// cost, and is required for anything but `market`.
#[derive(Debug, Deserialize)]
pub struct EntryIn {
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub price: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct IntentResponse {
    pub accepted: bool,
    /// Why not, when not: the run's last bar against the one named.
    pub reason: String,
}

/// The order an intent describes, checked - or the sentence saying why it
/// cannot be one. Separated from the handler so the rules are readable as a
/// list and testable without a run.
///
/// The stop must be on the LOSING side of the ENTRY PRICE and the target on
/// the winning side of it - not of the last close, which is where a market
/// intent's are judged and which is not where an order fills.
fn order_of(body: &IntentRequest, side: Side, entry_type: EntryType, decided_bar_time: i64) -> Result<PendingOrder, String> {
    let type_name = entry_type.as_str();
    let price = body
        .entry
        .as_ref()
        .and_then(|e| e.price)
        .filter(|p| p.is_finite() && *p > 0.0)
        .ok_or_else(|| format!("a {type_name} entry needs a price"))?;
    let stop = body.stop.filter(|v| v.is_finite()).ok_or_else(|| format!("a {type_name} entry needs a stop"))?;
    let losing_side = if side.is_long() { stop < price } else { stop > price };
    if !losing_side {
        return Err(format!("stop {stop} is not on the losing side of the {type_name} price {price} for a {}", side.as_str()));
    }
    let target = body.target.filter(|v| v.is_finite());
    if let Some(t) = target {
        let winning_side = if side.is_long() { t > price } else { t < price };
        if !winning_side {
            return Err(format!("target {t} is not on the winning side of the {type_name} price {price} for a {}", side.as_str()));
        }
    }
    let valid_bars = body.valid_bars.unwrap_or(2);
    if valid_bars == 0 {
        return Err("valid_bars must be at least 1".to_string());
    }
    Ok(PendingOrder {
        entry_type,
        price,
        side,
        stop: Some(stop),
        target,
        reason: if body.reason.is_empty() { "external".to_string() } else { body.reason.clone() },
        zone: body.zone.filter(|(lo, hi)| lo.is_finite() && hi.is_finite()),
        valid_bars,
        bars_waited: 0,
        decided_bar_time,
        invalidate_above: body.invalidate_above.filter(|v| v.is_finite()),
        invalidate_below: body.invalidate_below.filter(|v| v.is_finite()),
    })
}

pub async fn intent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IntentRequest>,
) -> Result<Json<IntentResponse>, ApiError> {
    // NONE is a real answer and is deliberately accepted here. A decider that
    // looked and wanted nothing has still driven this bar, and recording it is
    // the only way the desk can tell "the model is standing aside" from "the
    // model is not running". It sets no pending intent — the book is left
    // exactly as it was.
    let side = match body.side.to_ascii_uppercase().as_str() {
        "LONG" => Some(Side::Long),
        "SHORT" => Some(Side::Short),
        "NONE" => None,
        other => {
            return Err(ApiError::BadRequest(format!("side must be LONG, SHORT or NONE, got `{other}`")));
        }
    };
    // Absent is `market`, so a body written before orders existed means
    // exactly what it meant then.
    let entry_type = match body.entry.as_ref().map(|e| e.entry_type.trim().to_ascii_lowercase()) {
        None => EntryType::Market,
        Some(t) => match t.as_str() {
            "market" | "" => EntryType::Market,
            "limit" => EntryType::Limit,
            "stop" => EntryType::Stop,
            other => return Err(ApiError::BadRequest(format!("entry.type must be market, limit or stop, got `{other}`"))),
        },
    };
    let mut runs = state.paper.lock().expect("paper runs");
    let run = runs
        .get_mut(&body.run)
        .ok_or_else(|| ApiError::NotFound(format!("no paper run `{}`", body.run)))?;

    // A paused book refuses entries from outside as well as from its own
    // strategy. Without this the switch would only hold for rule-driven runs,
    // and an AI book would go on trading while the desk showed it off - the
    // worst of the two states, because the display would be wrong rather than
    // merely unhelpful.
    //
    // NONE is still accepted: a decider that looked and wanted nothing has
    // driven this bar, and recording that is how the desk tells standing aside
    // from not running. Pausing stops trades, not bookkeeping.
    if run.paused && side.is_some() {
        return Ok(Json(IntentResponse {
            accepted: false,
            reason: "run is paused".to_string(),
        }));
    }

    // Only a run that has declared itself externally driven. A rule-based book
    // must never be steerable from outside: its trades are its own or the
    // comparison between books means nothing.
    if run.config.strategy != "external" {
        return Err(ApiError::BadRequest(format!(
            "run `{}` uses strategy `{}`; only an `external` run takes posted intents",
            body.run, run.config.strategy
        )));
    }

    let last = run.bars.last().map(|b| b.time).unwrap_or(0);
    if last != body.bar_time {
        return Ok(Json(IntentResponse {
            accepted: false,
            reason: format!("decided on bar {} but the run is on {last}", body.bar_time),
        }));
    }

    // An order is checked whole before anything is touched, so a refusal
    // leaves the book - and whatever already rests on it - exactly as it was.
    let order = match (side, entry_type) {
        (Some(side), EntryType::Limit | EntryType::Stop) => {
            if run.book.position.is_some() {
                return Ok(Json(IntentResponse {
                    accepted: false,
                    reason: "run holds a position; an order cannot add to it".to_string(),
                }));
            }
            match order_of(&body, side, entry_type, last) {
                Ok(order) => Some(order),
                Err(why) => return Ok(Json(IntentResponse { accepted: false, reason: why })),
            }
        }
        _ => None,
    };

    // Stamped before the badge and before any log line, so that whatever else
    // happens the number belongs to the intent that was just stored.
    let decided_at = now_ms();
    // A new entry while an order rests REPLACES it, and the replacement is a
    // row: the order that waited and was withdrawn is a decision that did not
    // become a trade, and the fill rate needs it in the denominator.
    let mut replaced = None;
    if side.is_some() {
        if let Some(old) = run.book.cancel_order() {
            replaced = Some((old, run.pending_decided_at.take()));
        }
        run.pending_decided_at = Some(decided_at);
        run.pending_htf_bar_time = body.htf_bar_time;
    }
    match (side, order) {
        (Some(_), Some(order)) => {
            run.book.place_order(order);
        }
        (Some(side), None) => {
            run.book.decide(Intent::Enter {
                side,
                stop: body.stop.filter(|v| v.is_finite()),
                target: body.target.filter(|v| v.is_finite()),
                reason: if body.reason.is_empty() { "external".to_string() } else { body.reason.clone() },
            });
        }
        (None, _) => {}
    }
    // Only now, past every refusal above: the badge names a decision the book
    // actually took, never one it was merely offered.
    if let Some(name) = body.decider.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        let d = run.decider.get_or_insert_with(Decider::default);
        d.last = name.to_string();
        d.last_at = now_ms();
        if side.is_some() {
            *d.decisions.entry(name.to_string()).or_insert(0) += 1;
        } else {
            d.stood_aside += 1;
        }
    }
    // A stand-aside is not written to `fills.jsonl`: that file is the book's
    // record, and a bar on which nothing happened is not an event in it. The
    // decider's own log (`decisions.jsonl`) already holds every reply whole.
    if side.is_none() {
        // Persisted for the badge's sake: `last_at` is how the desk tells a
        // model that is standing aside from a process that died, and a counter
        // that resets on every API restart cannot carry that.
        persist(&state.data, run)?;
        return Ok(Json(IntentResponse {
            accepted: true,
            reason: "stood aside; the book is unchanged".to_string(),
        }));
    }
    if let Some((old, old_decided_at)) = &replaced {
        record_cancelled(&state.data, &body.run, old, body.bar_time, "replaced", *old_decided_at)?;
    }
    let resting = run.book.pending_order();
    record(
        &state.data,
        &body.run,
        &json!({
            "kind": "intent",
            "time": body.bar_time,
            // When the decider actually spoke, as against the bar it read.
            // The fill this intent receives is priced at the open of the bar
            // stamped `time`, which on a slow model is a price that existed
            // before the decision did.
            "decided_at": decided_at,
            // And which H4 state it saw, when it read one. Null means the
            // decider did not fetch the higher-timeframe facts at all - a
            // different thing from fetching them and finding nothing, and the
            // two must stay distinguishable or an experiment comparing the
            // variants is measuring a mixture.
            "htf_bar_time": body.htf_bar_time,
            "side": body.side.to_ascii_uppercase(),
            "stop": body.stop,
            "target": body.target,
            "reason": body.reason,
            "decider": body.decider,
            // How it is to be entered. `market` with no price on every row
            // written before orders existed and on every market intent since.
            "entry": { "type": entry_type.as_str(), "price": resting.map(|o| o.price) },
            "zone": resting.and_then(|o| o.zone),
            "valid_bars": resting.map(|o| o.valid_bars),
            "invalidate_above": resting.and_then(|o| o.invalidate_above),
            "invalidate_below": resting.and_then(|o| o.invalidate_below),
        }),
    )?;
    let reason = match resting {
        Some(o) => format!(
            "rests as a {} at {}; fills from the tick feed, cancelled after {} closed bar{} unfilled",
            o.entry_type.as_str(),
            o.price,
            o.valid_bars,
            if o.valid_bars == 1 { "" } else { "s" }
        ),
        None => "fills at the next bar's open".to_string(),
    };
    // **Before answering, not after.** This route tells the caller "accepted,
    // fills at the next bar's open", and that promise has to survive the
    // fifteen minutes until that bar arrives. The pending intent lived only in
    // memory until now, so a restart inside that window — a deploy, a crash —
    // silently cancelled a trade the model had been told was on. It was
    // observed doing exactly that: two stand-asides recorded at 16:15 were
    // gone from both badges after a 16:17 restart, and an entry would have
    // vanished the same way, without a line anywhere saying so. An order
    // rests longer still, and the same write is what carries it.
    persist(&state.data, run)?;
    Ok(Json(IntentResponse { accepted: true, reason }))
}

/* ------------------------------------------------ acting on a resting order */

/// `POST /api/paper/pending/act` — trigger or cancel the resting order.
#[derive(Debug, Deserialize)]
pub struct ActRequest {
    pub run: String,
    /// `trigger`: fill NOW at the latest tick's touched side (the ask for a
    /// LONG, the bid for a SHORT). `cancel`: withdraw it.
    pub action: String,
    /// The caller's sentence; on a cancel it becomes the row's reason,
    /// `cancelled:<reason>`.
    #[serde(default)]
    pub reason: String,
}

/// The same two fields as [`IntentResponse`], by the Python side's request:
/// `accepted` false with `reason` saying why nothing was done (no order
/// resting, no usable quote), true with `reason` saying what was done - on a
/// trigger, whether the fill opened the position or a guard or the advisor
/// refused it (the order is consumed either way, as a market fill would be;
/// the `refused` row says which). After a true reply `pending_order` reads
/// `null` on the very next status read: the slot is taken under the lock.
#[derive(Debug, Serialize)]
pub struct ActResponse {
    pub accepted: bool,
    pub reason: String,
}

/// How old the latest tick may be for a trigger to price off it. A fill
/// forced against a quote older than this is a fill at a price nobody has
/// seen; the poller cycles every few seconds, so ten is a dead feed.
pub const MAX_TRIGGER_TICK_AGE_MS: i64 = 10_000;

/// The stage-2 fast loop's one action. A trigger goes through
/// [`feed_order_fill`] exactly as a tick that reached the level would, so
/// the fill is sized, guarded and recorded the same way; the only thing the
/// caller chooses is WHEN, and the row says `filled_from: act`.
pub async fn act(State(state): State<Arc<AppState>>, Json(body): Json<ActRequest>) -> Result<Json<ActResponse>, ApiError> {
    let action = body.action.trim().to_ascii_lowercase();
    if action != "trigger" && action != "cancel" {
        return Err(ApiError::BadRequest(format!("action must be trigger or cancel, got `{}`", body.action)));
    }
    let refuse = |reason: String| Ok(Json(ActResponse { accepted: false, reason }));
    // The quote is read before the paper lock and never under it, the same
    // order every other reader of the two mutexes keeps.
    let live = state.live_bars.lock().expect("live bars").get(&stream_key_of(&state, &body.run)?).cloned();

    let mut runs = state.paper.lock().expect("paper runs");
    let run = runs.get_mut(&body.run).ok_or_else(|| ApiError::NotFound(format!("no paper run `{}`", body.run)))?;
    let Some(order) = run.book.pending_order().cloned() else {
        return refuse("no order is resting on this run".to_string());
    };

    if action == "cancel" {
        let order = run.book.cancel_order().expect("checked");
        let decided_at = run.pending_decided_at.take();
        run.pending_htf_bar_time = None;
        persist(&state.data, run)?;
        let why = body.reason.trim();
        let reason = format!("cancelled:{}", if why.is_empty() { "act" } else { why });
        record_cancelled(&state.data, &body.run, &order, run.bars.last().map_or(0, |b| b.time), &reason, decided_at)?;
        return Ok(Json(ActResponse { accepted: true, reason: format!("{reason}; the {} at {} is withdrawn", order.entry_type.as_str(), order.price) }));
    }

    let Some(live) = live else {
        return refuse("no tick has arrived on this stream".to_string());
    };
    let age = now_ms() - live.at;
    if age > MAX_TRIGGER_TICK_AGE_MS {
        return refuse(format!("the last tick is {age} ms old; a trigger needs one within {MAX_TRIGGER_TICK_AGE_MS} ms"));
    }
    if run.bars.last().is_none_or(|last| live.time <= last.time) {
        return refuse(format!("the last tick belongs to bar {}, which the run has already closed over", live.time));
    }
    let Some(price) = order.trigger_side(live.bid, live.ask) else {
        return refuse(format!("the last tick carries no {} to fill a {} at", if order.side.is_long() { "ask" } else { "bid" }, order.side.as_str()));
    };
    if run.book.position.is_some() {
        return refuse("run holds a position; the order cannot add to it".to_string());
    }
    let done = feed_order_fill(&state, run, &live, price, "act")?
        .ok_or_else(|| ApiError::Internal("the order was checked and then was not there".to_string()))?;
    let side = if order.side.is_long() { "ask" } else { "bid" };
    let reason = match done.reason {
        None => format!("filled at {price} on the {side} of the tick at {}", live.at),
        Some(why) => format!("consumed at {price} on the {side} of the tick at {} and refused: {why}", live.at),
    };
    Ok(Json(ActResponse { accepted: true, reason }))
}

/// The `market:tf` a run is on, read under the paper lock and released
/// before anything else is taken.
fn stream_key_of(state: &AppState, run: &str) -> Result<String, ApiError> {
    let runs = state.paper.lock().expect("paper runs");
    runs.get(run).map(|r| r.config.stream()).ok_or_else(|| ApiError::NotFound(format!("no paper run `{run}`")))
}

/* --------------------------------------------- reading the models back */

/// One decision an outside decider made, as its own log recorded it.
///
/// The prompt is **not** carried. It is ~3.7 KB per bar and ninety-six bars a
/// day, so shipping it to a browser that renders two hundred characters of it
/// would move megabytes to show a sentence. `prompt_chars` is sent instead, so
/// the desk can say the prompt was kept whole without carrying it; the file
/// beside the book remains the replayable record.
#[derive(Debug, Serialize)]
pub struct DecisionDto {
    /// When the decider answered, epoch ms.
    pub at: i64,
    /// The bar it decided on. The fill, if any, was the NEXT bar's open.
    pub bar_time: i64,
    pub model: String,
    pub side: String,
    pub reason: String,
    /// The reply as received, whole — it is short, and it is the thing a
    /// reader is actually checking the summary against.
    pub response: String,
    pub latency_ms: i64,
    pub posted: bool,
    /// Set when this desk refused the decision before it reached a book.
    pub refused_locally: String,
    pub dry_run: bool,
    pub prompt_chars: usize,
    /// What the call spent, exactly as the provider reported it.
    ///
    /// `total` alone for a provider that gives only one number (Codex prints a
    /// banner total and no split); `input`/`output` where the split exists.
    /// Absent where the provider reported nothing, rather than a zero nobody
    /// measured.
    pub tokens_in: Option<i64>,
    pub tokens_cached: Option<i64>,
    pub tokens_out: Option<i64>,
    pub tokens_total: Option<i64>,
    /// Dollars, for a model billed per token. **Null for a subscription** — a
    /// plan call is not free, it draws on a quota, and printing $0.00 beside it
    /// would claim something untrue.
    pub cost_usd: Option<f64>,
}

/// One agent's turn in an advisor consultation.
#[derive(Debug, Serialize)]
pub struct TurnDto {
    pub agent: String,
    pub model: String,
    pub reason: String,
    pub response: String,
    pub latency_ms: i64,
    pub size_factor: Option<f64>,
}

/// One consultation of the advisor panel over a pending intent.
#[derive(Debug, Serialize)]
pub struct ConsultationDto {
    pub at: i64,
    pub intent_id: String,
    /// Whether the verdict actually reached the book, or was a dry run.
    pub applied: bool,
    pub dry_run: bool,
    pub size_factor: Option<f64>,
    pub reason: String,
    pub turns: Vec<TurnDto>,
}

#[derive(Debug, Serialize)]
pub struct ReasoningResponse {
    pub run: String,
    /// Newest first, both of them.
    pub decisions: Vec<DecisionDto>,
    pub consultations: Vec<ConsultationDto>,
}

#[derive(Debug, Deserialize)]
pub struct ReasoningQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct BrokerEventsQuery {
    pub account: String,
    pub limit: Option<usize>,
}

/// `GET /api/paper/broker-events/{id}?account=<id>` — what ONE account did
/// with one book.
///
/// The desk has always been able to show a book's own events, and in account
/// mode it showed the same ones with a label saying they were the book's. The
/// owner read that as the two sides not being separate, which is fair: the
/// account has its own record and nothing served it.
///
/// It is a different log answering a different question. The book's events are
/// about the rule - a gap in the feed, a guard firing, the run starting. These
/// are about the execution: a position not adopted because the price had run,
/// a size clipped to the broker's minimum, an order refused, AutoTrading off,
/// and the line that says this account is real. None of that exists on the
/// paper side, because none of it can happen there.
pub async fn broker_events(
    State(state): State<Arc<AppState>>,
    PathParam(id): PathParam<String>,
    Query(query): Query<BrokerEventsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_id(&id)?;
    // The account id becomes a directory name, so it is checked the same way a
    // run id is rather than trusted from a query string.
    check_id(&query.account)?;
    let limit = query.limit.unwrap_or(DEFAULT_REASONING).clamp(1, MAX_REASONING);
    let path = state
        .data
        .join("live")
        .join(&query.account)
        .join(&id)
        .join("executor.jsonl");
    // An absent file is not an error. A book this account does not mirror, or
    // one whose executor has never run, has nothing to say - and an empty list
    // says that more usefully than a 404, which the client would have to
    // special-case on every switch of the account picker.
    Ok(Json(json!({
        "run": id,
        "account": query.account,
        "events": tail_jsonl(&path, limit),
    })))
}

/// Default and ceiling for how many entries come back.
pub const DEFAULT_REASONING: usize = 50;
pub const MAX_REASONING: usize = 500;

fn s_of(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn i_of(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(serde_json::Value::as_i64).unwrap_or(0)
}
fn b_of(v: &serde_json::Value, key: &str) -> bool {
    v.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// How far back the first read of a tail reaches. It doubles each round, so a
/// file whose lines are large is reached in a few seeks rather than a hundred.
///
/// Measured 2026-09-17: `data/paper/ai-xau-opus-ctx/decisions.jsonl` is
/// 632,258 bytes over 99 lines, 6,386 a line — the whole prompt is in the
/// line, which is the point of the file — so 64 KiB is ten of them.
/// `data/live/vantage-demo/ai-xau-ds-ctx/executor.jsonl` is 73,862 over 334,
/// 221 a line, so 64 KiB is nearly three hundred. One constant cannot suit
/// both; doubling is what makes the difference not matter.
const TAIL_CHUNK: u64 = 64 * 1024;

/// Bytes from the end of a file, enough of them to answer `enough`.
///
/// Reads backwards in a window that doubles each round until `enough` is
/// satisfied by the whole lines inside it, or the window reaches byte 0 and
/// there is no more file to have. Returns the window and whether it starts at
/// byte 0, which is what [`complete_lines`] needs to know whether its first
/// line is a line or the tail of one.
///
/// `enough` is given the lines rather than a count because the two callers ask
/// different questions of them: [`tail_jsonl`] wants `limit` lines, and
/// [`events_of`] wants `limit` lines that are not trades, out of a file where
/// a third of them are.
///
/// Re-splitting the whole window each round rather than counting the new
/// chunk's newlines and adding them up. The counting version was written first
/// and measured slower — `bytes.iter().filter(|b| **b == b'\n').count()` walks
/// a byte at a time, where the `lines()` inside `complete_lines` uses the same
/// memchr the standard library uses everywhere else. Doubling keeps the number
/// of rounds at three or four, so the re-splitting is bounded and the faster
/// scan wins.
fn tail_window(path: &Path, enough: impl Fn(&[&str]) -> bool) -> Option<(Vec<u8>, bool)> {
    let mut file = std::fs::File::open(path).ok()?;
    // The length is taken once. Whatever is appended after this point is not
    // read at all, which is the cheapest way to be sure a line is never read
    // half-written.
    let end = file.seek(SeekFrom::End(0)).ok()?;

    let mut pos = end;
    let mut window: Vec<u8> = Vec::new();
    let mut step = TAIL_CHUNK;
    loop {
        let back = step.min(pos);
        pos -= back;
        let mut chunk = vec![0u8; back as usize];
        file.seek(SeekFrom::Start(pos)).ok()?;
        // Short of the bytes the length promised: the file was replaced under
        // us. An empty panel for one poll, not a wrong one.
        file.read_exact(&mut chunk).ok()?;
        chunk.extend_from_slice(&window);
        window = chunk;
        if pos == 0 || enough(&complete_lines(&window, pos == 0)) {
            break;
        }
        step *= 2;
    }
    Some((window, pos == 0))
}

/// The last `limit` lines of a JSONL file, oldest first, skipping unreadable
/// ones rather than failing: a half-written last line (the writer was killed
/// mid-append) must not take the whole panel down.
///
/// Read backwards from the end. Until 2026-09-17 this read the whole file with
/// `read_to_string` and kept the last fifty lines, on every poll of the Desk,
/// for every book. That was affordable while the desk was a demo on a
/// workstation that got switched off at night. It is not now: the desk runs 24
/// hours on a 2-vCPU VPS, `decisions.jsonl` gains a 6 KB line every fifteen
/// minutes and nothing has ever truncated it, so the cost of one poll grew
/// with the age of the campaign and would go on growing.
///
/// Be clear about when the change pays, because today it does not. Measured
/// 2026-09-17 on this machine, release build, fifty lines out of a file shaped
/// like the real one:
///
/// | file | whole | tail |
/// |---|---|---|
/// | `decisions.jsonl`, 638 KB — today | 253 us | 575 us |
/// | `decisions.jsonl`, 32 MiB — at the roll | 17.4 ms | 660 us |
/// | `executor.jsonl`, 74 KB — today | 95 us | 104 us |
/// | `executor.jsonl`, 32 MiB — at the roll | 20.0 ms | 180 us |
///
/// So this is a 2.3x LOSS on `decisions.jsonl` as it stands, and that is the
/// honest cost of the change: fifty of its lines are 320 KB, half the file, so
/// there is not much tail to save yet, and the window costs three reads and a
/// re-split where one `read_to_string` would do. It is taken because the
/// column that matters is the second row and not the first. The tail's cost
/// barely moves between 638 KB and 32 MiB - it is set by `limit` and the
/// length of a line, not by the age of the campaign - while the whole-file
/// read grows with the file and never stops. Half a millisecond now, against
/// seventeen milliseconds a book in 55 days (`ROTATE_BYTES` in
/// `py/live/ai_trader.py`) and more after that.
///
/// Four things the file can do to a reader, and what is done about each:
///
/// - The window opens in the middle of a line. That head fragment is not a
///   line and is discarded, unless the window reached byte 0, where the first
///   byte does begin a line.
/// - The file does not end in a newline. The tail fragment is discarded, and
///   this is the one that matters: a Python process is appending to this file
///   while the handler reads it, and a torn line that happened to parse would
///   put a number nobody wrote into a trading record. Both writers terminate
///   every line, so the only cost is that a line being written right now shows
///   up on the next poll instead of this one.
/// - A line is longer than the window. The window keeps doubling to byte 0, so
///   a long line is found rather than lost; a file with no newline in it at
///   all therefore still costs its whole length, exactly as before.
/// - UTF-8. Both ends of the window are cut at a newline, which is ASCII and
///   so always a character boundary; the middle is untouched. Nothing is ever
///   sliced through a multi-byte character. Invalid UTF-8 inside the window
///   still empties the response, as reading the file whole did — but invalid
///   UTF-8 in a line old enough to be outside the window no longer can.
///
/// A rename under the reader is safe on both platforms: the open handle
/// follows the file it was opened on, so a rotation mid-read returns the tail
/// of the rolled file and the next poll opens the new one.
fn tail_jsonl(path: &Path, limit: usize) -> Vec<serde_json::Value> {
    let Some((window, from_start)) = tail_window(path, |lines| lines.len() >= limit) else {
        return Vec::new();
    };
    let lines = complete_lines(&window, from_start);
    lines
        .iter()
        .skip(lines.len().saturating_sub(limit))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The whole, non-empty lines inside a window that ends at the end of a file.
/// `from_start` says the window begins at byte 0, where the first line is not
/// a fragment.
///
/// Blank lines are dropped before `limit` is applied, and lines that are not
/// JSON are dropped after it, which is what reading the file whole did. The
/// order is deliberate and is not an oversight to tidy up: a torn line must
/// cost the response one entry, never the response.
fn complete_lines(window: &[u8], from_start: bool) -> Vec<&str> {
    let mut bytes = window;
    if !from_start {
        match bytes.iter().position(|b| *b == b'\n') {
            Some(i) => bytes = &bytes[i + 1..],
            None => return Vec::new(),
        }
    }
    match bytes.iter().rposition(|b| *b == b'\n') {
        Some(i) => bytes = &bytes[..=i],
        None => return Vec::new(),
    }
    let Ok(text) = std::str::from_utf8(bytes) else { return Vec::new() };
    text.lines().filter(|l| !l.trim().is_empty()).collect()
}

/// `GET /api/paper/reasoning/{id}` — what the models said about this book.
///
/// Two different logs, kept apart because they are two different powers:
/// `decisions.jsonl` is a decider choosing a side on its own book, and
/// `advice.jsonl` is the advisor panel refusing or shrinking somebody else's
/// trade. A rule-based run has only the second; an `external` run usually has
/// only the first.
pub async fn reasoning(
    State(state): State<Arc<AppState>>,
    PathParam(id): PathParam<String>,
    Query(query): Query<ReasoningQuery>,
) -> Result<Json<ReasoningResponse>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_REASONING).clamp(1, MAX_REASONING);
    let dir = run_dir(&state.data, &id);
    if !dir.is_dir() {
        return Err(ApiError::NotFound(format!("no paper run `{id}`")));
    }

    let mut decisions: Vec<DecisionDto> = tail_jsonl(&dir.join("decisions.jsonl"), limit)
        .iter()
        .map(|v| {
            let d = v.get("decision").cloned().unwrap_or(serde_json::Value::Null);
            DecisionDto {
                at: i_of(v, "at"),
                bar_time: i_of(v, "bar_time"),
                model: s_of(v, "model"),
                side: s_of(&d, "side"),
                reason: s_of(&d, "reason"),
                response: s_of(v, "response"),
                latency_ms: i_of(v, "latency_ms"),
                posted: b_of(v, "posted"),
                refused_locally: s_of(v, "refused_locally"),
                dry_run: b_of(v, "dry_run"),
                prompt_chars: v.get("prompt").and_then(|p| p.as_str()).map_or(0, str::len),
                tokens_in: v.pointer("/usage/input").and_then(serde_json::Value::as_i64),
                tokens_cached: v.pointer("/usage/cached_input").and_then(serde_json::Value::as_i64),
                tokens_out: v.pointer("/usage/output").and_then(serde_json::Value::as_i64),
                tokens_total: v.pointer("/usage/total").and_then(serde_json::Value::as_i64),
                cost_usd: v.get("cost_usd").and_then(serde_json::Value::as_f64),
            }
        })
        .collect();
    decisions.reverse();

    let mut consultations: Vec<ConsultationDto> = tail_jsonl(&dir.join("advice.jsonl"), limit)
        .iter()
        .map(|v| ConsultationDto {
            at: i_of(v, "at"),
            intent_id: s_of(v, "intent_id"),
            applied: b_of(v, "applied"),
            dry_run: b_of(v, "dry_run"),
            size_factor: v.get("size_factor").and_then(serde_json::Value::as_f64),
            reason: s_of(v, "reason"),
            turns: v
                .get("transcript")
                .and_then(|t| t.as_array())
                .map(|rows| {
                    rows.iter()
                        .map(|t| TurnDto {
                            agent: s_of(t, "agent"),
                            model: s_of(t, "model"),
                            reason: s_of(t, "reason"),
                            response: s_of(t, "response"),
                            latency_ms: i_of(t, "latency_ms"),
                            size_factor: t.get("size_factor").and_then(serde_json::Value::as_f64),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    consultations.reverse();

    Ok(Json(ReasoningResponse { run: id, decisions, consultations }))
}

/// How large a log grows before it is rolled aside — the same 32 MiB the
/// Python writers use (`ROTATE_BYTES` in `py/live/ai_trader.py`).
///
/// The arithmetic for THIS file, measured 2026-09-17: the busiest advisor book
/// wrote 105,119 bytes of `advice.jsonl` in 9.7 hours — 10.8 KB an hour, 253
/// KB a day, because a panel transcript is about 12 KB and a busy book takes
/// twenty a day. That is 130 days to 32 MiB.
///
/// Worth saying because it was nearly skipped: this file was reported, by me,
/// as being years from mattering. That came from counting lines on a quiet
/// book — six a day — without measuring a busy one. It is the same disease as
/// `decisions.jsonl` at two and a half times the doubling time, not a
/// different kind of file.
const ROTATE_BYTES: u64 = 32 * 1024 * 1024;

/// Move a log that has reached `limit` aside, under a name that sorts by when.
///
/// Renaming, and nothing else. Nothing here or anywhere else in this repo
/// removes a rolled file, and there is deliberately no keep-the-last-N: the
/// owner's standing requirement is that the record reads as one unbroken thing
/// from the first day, and a rotation that is able to delete is one that
/// eventually will. If disk is ever short the answer is to move old files by
/// hand, which is a decision a person makes once.
///
/// Named with `now_ms()` rather than a calendar stamp, unlike the Python side
/// which writes `decisions-20260917T143005Z.jsonl`. Nothing in this workspace
/// depends on a date library and every time in it is epoch milliseconds, so
/// pulling one in to format a filename would be the more expensive of the two
/// inconsistencies. Thirteen digits sort chronologically until the year 2286.
///
/// The name is RESERVED with `create_new` before the rename, and that is the
/// load-bearing line. `std::fs::rename` REPLACES an existing destination on
/// both platforms — unlike Python's `os.rename`, which refuses on Windows —
/// and this is an axum handler, so two consultations on one run can be in here
/// at the same time. Exactly one can create the name; the loser leaves the
/// file alone instead of renaming over the winner's record. A failed rename
/// leaves an empty rolled file behind, which is untidy and harmless, and the
/// next millisecond gets a different name.
///
/// Failing to roll is acceptable; losing a line is not. It runs before the
/// append, never touches content, and every failure is discarded: a
/// consultation must not fail because a log could not be tidied.
fn roll_aside(path: &Path, limit: u64) {
    let Ok(meta) = std::fs::metadata(path) else { return };
    if meta.len() < limit {
        return;
    }
    let Some(stem) = path.file_stem().and_then(std::ffi::OsStr::to_str) else { return };
    let ext = path.extension().and_then(std::ffi::OsStr::to_str).unwrap_or("jsonl");
    let rolled = path.with_file_name(format!("{stem}-{}.{ext}", now_ms()));
    if std::fs::OpenOptions::new().create_new(true).write(true).open(&rolled).is_err() {
        return;
    }
    let _ = std::fs::rename(path, &rolled);
}

/// The conversation log: its own file, never `fills.jsonl`.
///
/// Kept apart because the two have different lifetimes and different readers.
/// `fills.jsonl` is what the book did; this is what was said about it, and it
/// is orders of magnitude larger now that whole prompts are in it — 12 KB a
/// line against 250 bytes. Mixing them would make the book's own reload scan
/// megabytes of transcript to find its trades.
///
/// That last sentence used to say `fills.jsonl` was the file "replayed on
/// restart", and it has not been since the history moved out:
/// [`restore_history`] reads `trades.jsonl`, which carries the engine's own
/// `Trade` where `fills.jsonl` drops `exit_kind` and `swap_usd`. The argument
/// for keeping the two files apart survives the correction - it is about size
/// and readers, not about the reload - but the sentence was telling the next
/// reader the wrong thing about which file is the record.
fn log_consultation(data: &Path, id: &str, event: &serde_json::Value) -> Result<(), ApiError> {
    let dir = run_dir(data, id);
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", dir.display())))?;
    let path = dir.join("advice.jsonl");
    roll_aside(&path, ROTATE_BYTES);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))?;
    use std::io::Write as _;
    writeln!(file, "{event}").map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))?;
    Ok(())
}

/// [`tail_jsonl`] against the shapes a JSONL file on this desk actually takes:
/// being appended to, killed mid-line, and old enough that reading it whole is
/// the thing being avoided.
#[cfg(test)]
mod tail_tests {
    use super::*;

    /// A file with `body` in it, in a directory the caller keeps alive.
    fn file_of(dir: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("decisions.jsonl");
        std::fs::write(&path, body).expect("write");
        path
    }

    fn ats(rows: &[serde_json::Value]) -> Vec<i64> {
        rows.iter().map(|v| i_of(v, "at")).collect()
    }

    #[test]
    fn a_file_that_is_not_there_is_no_lines() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(tail_jsonl(&dir.path().join("never-written.jsonl"), 50).is_empty());
    }

    #[test]
    fn an_empty_file_is_no_lines() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The book that started this minute and has not decided yet. It must
        // read as nothing to say, not as a failure.
        assert!(tail_jsonl(&file_of(&dir, ""), 50).is_empty());
    }

    #[test]
    fn fewer_lines_than_the_limit_gives_what_there_is() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body = "{\"at\":1}\n{\"at\":2}\n{\"at\":3}\n";
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 50)), vec![1, 2, 3]);
    }

    #[test]
    fn the_limit_takes_the_newest_and_keeps_them_oldest_first() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body: String = (1..=200).map(|i| format!("{{\"at\":{i}}}\n")).collect();
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, &body), 3)), vec![198, 199, 200]);
    }

    #[test]
    fn an_unterminated_last_line_is_dropped() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The Python writer is mid-append, or was killed there. `{"at":9` is
        // not JSON and would have been dropped anyway; the point of the test
        // is that the decision is made by the missing newline and not by
        // whether the fragment happens to parse.
        let body = "{\"at\":1}\n{\"at\":2}\n{\"at\":9";
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 50)), vec![1, 2]);
    }

    #[test]
    fn a_torn_line_that_would_parse_is_still_dropped() {
        let dir = tempfile::tempdir().expect("temp dir");
        // `12345` is valid JSON and `at` then reads as 0, so a reader that
        // trusted the last line would put a record nobody wrote into the
        // panel. This is the case the newline rule exists for.
        let body = "{\"at\":1}\n12345";
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 50)), vec![1]);
    }

    #[test]
    fn blank_lines_are_not_lines() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body = "{\"at\":1}\n\n{\"at\":2}\n   \n{\"at\":3}\n\n";
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 50)), vec![1, 2, 3]);
        // And they do not eat into the limit: two lines back is 2, not a gap.
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 2)), vec![2, 3]);
    }

    #[test]
    fn a_line_that_is_not_json_costs_one_entry_not_the_response() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body = "{\"at\":1}\n{\"at\":2\n{\"at\":3}\n";
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 50)), vec![1, 3]);
        // The limit counts lines, then the unreadable one drops: asking for
        // two here gives one. That is what reading the file whole did, and the
        // alternative — search backwards until `limit` PARSE — would make a
        // run of torn lines walk the whole file, which is what this change is
        // for.
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, body), 2)), vec![3]);
    }

    #[test]
    fn a_line_longer_than_the_window_is_found() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Two lines of 100 KB against a 64 KiB first read, so the window has
        // to grow twice to reach them. `advice.jsonl` averages 12 KB a line
        // (measured 2026-09-17) and a panel transcript is not bounded, so this
        // is a real shape and not a contrived one.
        let fat = "x".repeat(100_000);
        let body = format!("{{\"at\":1,\"pad\":\"{fat}\"}}\n{{\"at\":2,\"pad\":\"{fat}\"}}\n");
        assert_eq!(ats(&tail_jsonl(&file_of(&dir, &body), 2)), vec![1, 2]);
    }

    #[test]
    fn one_line_with_no_newline_at_all_is_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The window grows to byte 0 and still finds no line terminator, so
        // there is no line that is known to be whole. Deliberate, and it is
        // not free: a writer that did not terminate its lines would read as an
        // empty log here. Both writers on this desk terminate every line.
        assert!(tail_jsonl(&file_of(&dir, "{\"at\":1}"), 50).is_empty());
    }

    #[test]
    fn the_head_of_a_large_file_is_never_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        // 2 MB of bytes that are not UTF-8, then the lines. Decoding the file
        // whole fails on them, so a pass means the read genuinely stayed in
        // the tail — a claim about cost that a stopwatch cannot make on a
        // shared machine.
        let mut body: Vec<u8> = vec![0xFF; 2 * 1024 * 1024];
        body.push(b'\n');
        for i in 1..=60 {
            body.extend_from_slice(format!("{{\"at\":{i}}}\n").as_bytes());
        }
        let path = dir.path().join("decisions.jsonl");
        std::fs::write(&path, &body).expect("write");
        assert!(std::fs::read_to_string(&path).is_err(), "the whole file does not decode");
        assert_eq!(ats(&tail_jsonl(&path, 50)).len(), 50);
        assert_eq!(*ats(&tail_jsonl(&path, 50)).last().expect("last"), 60);
    }

    #[test]
    fn a_growing_file_is_read_at_the_length_it_had() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = file_of(&dir, "{\"at\":1}\n{\"at\":2}\n");
        // Appending between two calls is the normal case, not an error: the
        // second call sees the new line and neither call sees a half of one.
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).expect("append");
        f.write_all(b"{\"at\":3}\n").expect("write");
        drop(f);
        assert_eq!(ats(&tail_jsonl(&path, 50)), vec![1, 2, 3]);
    }
}

/// [`events_of`] over a `fills.jsonl` with trades
/// interleaved through it, which is the only shape it ever has.
#[cfg(test)]
mod event_tests {
    use super::*;

    /// `<data>/paper/<id>/fills.jsonl` holding `body`, and the data root.
    fn run_with(dir: &tempfile::TempDir, id: &str, body: &str) -> PathBuf {
        let data = dir.path().to_path_buf();
        let run = run_dir(&data, id);
        std::fs::create_dir_all(&run).expect("mkdir");
        std::fs::write(run.join("fills.jsonl"), body).expect("write");
        data
    }

    /// `n` lines, every third one a trade, each numbered so the newest are
    /// identifiable. Deliberately more trades than a real book takes: the
    /// point is that the window has to look past them.
    fn mixed(n: usize) -> String {
        (0..n)
            .map(|i| {
                if i % 3 == 0 {
                    format!("{{\"kind\":\"trade\",\"at\":{i}}}\n")
                } else {
                    format!("{{\"kind\":\"gap\",\"at\":{i}}}\n")
                }
            })
            .collect()
    }

    /// What the whole-file read did, kept here to compare against rather than
    /// alive in the crate where it could be called by accident.
    fn every_line(data: &Path, id: &str) -> Vec<serde_json::Value> {
        let Ok(text) = std::fs::read_to_string(run_dir(data, id).join("fills.jsonl")) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|e| e.is_object() && e["kind"] != "trade")
            .collect()
    }

    #[test]
    fn the_tail_returns_events_and_never_trades() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = run_with(&dir, "book", &mixed(300));
        let got = events_of(&data, "book", 20);
        assert_eq!(got.len(), 20, "twenty events, not twenty lines");
        assert!(got.iter().all(|e| e["kind"] != "trade"), "{got:?}");
        // The newest twenty non-trade lines of 300, oldest first. 299 and 298
        // are events, 297 is a trade.
        assert_eq!(got.last().expect("last")["at"], 299);
        assert!(got[0]["at"].as_i64() < got[19]["at"].as_i64(), "oldest first");
    }

    #[test]
    fn the_tail_agrees_with_reading_every_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = run_with(&dir, "book", &mixed(300));
        let whole = every_line(&data, "book");
        let tail = events_of(&data, "book", MAX_DETAIL_EVENTS);
        let cut = whole.len().saturating_sub(MAX_DETAIL_EVENTS);
        assert_eq!(tail, whole[cut..], "the tail is the end of what the full read returned");
    }

    #[test]
    fn fewer_events_than_asked_for_is_all_of_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = run_with(&dir, "book", &mixed(10));
        // Six of ten are not trades (0, 3, 6 and 9 are), and the window walks
        // to byte 0 to find that out rather than stopping when it runs out of
        // chunks.
        assert_eq!(events_of(&data, "book", MAX_DETAIL_EVENTS).len(), 6);
    }

    #[test]
    fn a_run_with_no_file_has_no_events_and_counts_zero() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = dir.path().to_path_buf();
        assert!(events_of(&data, "never-ran", 50).is_empty());
    }

    #[test]
    fn the_odd_lines_are_classified_exactly_as_the_full_read_classified_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Each of these was decided by `event.is_object() && event["kind"] !=
        // "trade"` before, and `is_event` has to keep deciding them the same
        // way or a number on the Desk changes for no reason anybody can see.
        let body = concat!(
            "{\"kind\":\"gap\",\"at\":1}\n",       // an event
            "{\"kind\":\"trade\",\"at\":2}\n",     // not
            "{\"at\":3}\n",                        // no kind at all: an event
            "{\"kind\":7,\"at\":4}\n",             // kind that is not a string: an event
            "12345\n",                             // valid JSON, not an object: not
            "{\"kind\":\"gap\",\"at\":6\n",        // torn: not
            "\n",                                  // blank: not
            "{\"kind\":\"stopped\",\"at\":8}\n",   // an event
        );
        let data = run_with(&dir, "book", body);
        let whole = every_line(&data, "book");
        assert_eq!(whole.iter().map(|e| i_of(e, "at")).collect::<Vec<_>>(), vec![1, 3, 4, 8]);
        assert_eq!(events_of(&data, "book", 50), whole);
    }

    #[test]
    fn a_file_of_nothing_but_trades_costs_the_whole_file_and_returns_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body: String = (0..500).map(|i| format!("{{\"kind\":\"trade\",\"at\":{i}}}\n")).collect();
        let data = run_with(&dir, "book", &body);
        // The window can never satisfy the count, so it grows to byte 0. That
        // is the honest worst case of growing on events rather than lines, and
        // it is the same cost the full read always paid.
        assert!(events_of(&data, "book", 50).is_empty());
    }
}

/// [`roll_aside`], which is the Rust half of the rotation the Python writers
/// got in `bee91a1`. The threshold is a parameter rather than the constant so
/// that a test does not have to write 32 MiB to reach it.
#[cfg(test)]
mod roll_tests {
    use super::*;

    fn file_of(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, body).expect("write");
        path
    }

    fn rolled_beside(path: &Path) -> Vec<PathBuf> {
        let stem = path.file_stem().and_then(std::ffi::OsStr::to_str).expect("stem");
        let mut out: Vec<PathBuf> = std::fs::read_dir(path.parent().expect("parent"))
            .expect("read_dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|n| n.starts_with(&format!("{stem}-")))
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_log_under_the_threshold_is_left_alone() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = file_of(&dir, "advice.jsonl", "{\"at\":1}\n");
        roll_aside(&path, 1024);
        assert!(rolled_beside(&path).is_empty(), "nothing to roll yet");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "{\"at\":1}\n");
    }

    #[test]
    fn a_full_log_is_moved_aside_whole_and_the_name_is_free_again() {
        let dir = tempfile::tempdir().expect("temp dir");
        let body = "{\"at\":1}\n{\"at\":2}\n";
        let path = file_of(&dir, "advice.jsonl", body);
        roll_aside(&path, 8);

        let rolled = rolled_beside(&path);
        assert_eq!(rolled.len(), 1, "exactly one roll");
        assert_eq!(std::fs::read_to_string(&rolled[0]).expect("read"), body, "content is untouched");
        assert!(!path.exists(), "the live name is free for the next append");
        assert_eq!(rolled[0].extension().and_then(std::ffi::OsStr::to_str), Some("jsonl"));
    }

    #[test]
    fn a_second_roll_never_overwrites_the_first() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = file_of(&dir, "advice.jsonl", "the first day\n");
        roll_aside(&path, 1);
        let first = rolled_beside(&path);
        assert_eq!(first.len(), 1);

        // Same millisecond, near enough: the reserved name is taken, so the
        // second roll declines rather than renaming over a day of record.
        // `std::fs::rename` would REPLACE it, which is what the reservation is
        // there to prevent.
        std::fs::write(&path, "the second day\n").expect("write");
        roll_aside(&path, 1);
        assert_eq!(
            std::fs::read_to_string(&first[0]).expect("read"),
            "the first day\n",
            "the first rolled file still holds the first day"
        );
        let now = rolled_beside(&path);
        assert!(now.len() <= 2, "at most one more name was taken");
        // Whichever way the clock fell, no day was destroyed: the second day
        // is either still live or in a rolled file of its own.
        let mut seen: Vec<String> =
            now.iter().map(|p| std::fs::read_to_string(p).expect("read")).collect();
        if path.exists() {
            seen.push(std::fs::read_to_string(&path).expect("read"));
        }
        assert!(seen.iter().any(|s| s == "the first day\n"), "{seen:?}");
        assert!(seen.iter().any(|s| s == "the second day\n"), "{seen:?}");
    }

    #[test]
    fn a_log_that_does_not_exist_is_not_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The first consultation on a new run: the file is created by the
        // append that follows, and asking to roll it first must be silent.
        roll_aside(&dir.path().join("advice.jsonl"), 1);
    }

    #[test]
    fn the_consultation_after_a_roll_starts_a_fresh_file_and_loses_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = dir.path().to_path_buf();
        let run = run_dir(&data, "book");
        std::fs::create_dir_all(&run).expect("mkdir");
        let path = run.join("advice.jsonl");

        for at in 1..=3 {
            log_consultation(&data, "book", &json!({ "kind": "consultation", "at": at }))
                .expect("logged");
        }
        assert_eq!(tail_jsonl(&path, 50).len(), 3);

        // Past the threshold now, so the next one rolls it first. Exercised
        // through log_consultation rather than roll_aside, because the order -
        // roll, THEN append - is the part that must not regress: the other way
        // round moves the new line into the rolled file.
        roll_aside(&path, 8);
        log_consultation(&data, "book", &json!({ "kind": "consultation", "at": 4 }))
            .expect("logged");

        let rolled = rolled_beside(&path);
        assert_eq!(rolled.len(), 1);
        let kept: Vec<i64> = tail_jsonl(&rolled[0], 50).iter().map(|v| i_of(v, "at")).collect();
        let live: Vec<i64> = tail_jsonl(&path, 50).iter().map(|v| i_of(v, "at")).collect();
        assert_eq!(kept, vec![1, 2, 3], "the history is in the rolled file");
        assert_eq!(live, vec![4], "and the new line is in the live one");
    }
}

/// The receipt for the table in [`tail_jsonl`]'s comment.
///
/// `#[ignore]`d: it writes 64 MB of temporary files and a timing is not a
/// thing to assert on a machine that is also running sixteen books and a
/// terminal. Run it by hand when the shape of a log changes, or when someone
/// wants to know whether reading the tail still pays:
///
/// ```text
/// cargo test -p fd-api --lib --release tail_bench -- --ignored --nocapture
/// ```
///
/// The comparison is against the code this replaced - `read_to_string`, split,
/// keep the last `limit` - spelled out again here rather than kept alive in
/// the crate, so the old shape cannot be called by accident.
#[cfg(test)]
mod tail_bench {
    use super::*;

    /// A file of `line_bytes`-long records, up to `target` bytes, timed both
    /// ways at both limits the handlers use. Best of five: the worst of five
    /// on a busy Windows box measures the scheduler, not the code.
    fn bench(name: &str, line_bytes: usize, target: usize) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("x.jsonl");
        let pad = "y".repeat(line_bytes.saturating_sub(24));
        let mut body = String::new();
        let mut rows = 0;
        while body.len() < target {
            rows += 1;
            body.push_str(&format!("{{\"at\":{rows},\"p\":\"{pad}\"}}\n"));
        }
        std::fs::write(&path, &body).expect("write");
        for limit in [DEFAULT_REASONING, MAX_REASONING] {
            let mut tail = std::time::Duration::MAX;
            let mut whole = std::time::Duration::MAX;
            for _ in 0..5 {
                let t = std::time::Instant::now();
                let n = tail_jsonl(&path, limit).len();
                tail = tail.min(t.elapsed());

                let t = std::time::Instant::now();
                let text = std::fs::read_to_string(&path).expect("read");
                let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                let m: Vec<serde_json::Value> = lines
                    .iter()
                    .skip(lines.len().saturating_sub(limit))
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                whole = whole.min(t.elapsed());

                // The point of running both is that they agree. A tail that is
                // fast and returns something else is not an improvement.
                assert_eq!(n, m.len(), "{name}: the tail and the whole file disagree");
            }
            println!("{name} {} bytes {rows} lines | limit {limit}: tail {tail:?} whole {whole:?}", body.len());
        }
    }

    /// What happens to a `broker.json` carrying fields this binary has never heard
/// of.
///
/// Written 2026-09-17, the day the executor started writing `server_offset_ms`
/// and `drift` into every snapshot. The API and the executor are separate
/// programs that deploy separately, so one of them is always ahead: the
/// interesting question is not what happens when they agree, it is what
/// happens in the hours when they do not.
#[cfg(test)]
mod broker_snapshot_tests {
    use super::*;

    /// A snapshot as the executor writes it TODAY, including the two fields
    /// added in 786a2e5 that this DTO does not declare.
    fn snapshot_from_a_newer_executor() -> String {
        json!({
            "account": "vantage-cent",
            "at": 1_789_650_000_000_i64,
            "login": 33_705_331,
            "server": "VantageMarkets-Live 21",
            "demo": false,
            "currency": "USC",
            "balance": 10_000.0,
            "equity": 9_980.0,
            "symbol": "XAUUSD.sc",
            "book_side": "LONG",
            "book_lots": 0.05,
            "realised": -20.0,
            "closed": 3,
            // The IB credit, as the executor writes it from 2026-09-21.
            // `realised` above is unchanged by it and this sits beside it,
            // which is the whole shape of the thing.
            "rebate": {
                "amount": 1.76,
                "realised_with_rebate": -18.24,
                "currency": "USC",
                "share_of_spread": 0.45,
                "configured_spread": 0.28,
                "exact": 1,
                "estimated": 2,
                "unpriced": 0,
            },
            "fills": [{
                "direction": "LONG",
                "entryTime": 1_789_640_000_000_i64,
                "entryPrice": 4378.59,
                "exitTime": 1_789_641_000_000_i64,
                "exitPrice": 4380.0,
                "lots": 0.07,
                "exitReason": "tp",
                "pnl": 9.87,
                "rebate": 0.88,
                "rebateBasis": "ESTIMATED",
            }],
            "position": null,
            "blocked": null,
            "standing_out": null,
            // Declared here since the desk gained readers for them. 10800000
            // is the broker on +3h, which is what all five live books
            // reported once the clock fix deployed on 2026-09-17.
            "server_offset_ms": 10_800_000_i64,
            "drift": "1 position holding 0.05 where the book wants 0.10",
            // And one this binary still has never heard of, standing in for
            // the next thing the executor learns to report. The boundary has
            // to stay tested after the two above crossed it.
            "something_the_executor_learned_later": 42,
        })
        .to_string()
    }

    fn account_with(dir: &tempfile::TempDir, account: &str, run: &str, body: &str) -> PathBuf {
        let data = dir.path().to_path_buf();
        let here = data.join("live").join(account).join(run);
        std::fs::create_dir_all(&here).expect("mkdir");
        std::fs::write(here.join("broker.json"), body).expect("write");
        data
    }

    #[test]
    fn a_snapshot_from_a_newer_executor_still_parses() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = account_with(&dir, "vantage-cent", "xau-ema", &snapshot_from_a_newer_executor());

        // The failure this guards against is the whole account view going
        // blank on a desk that is mirroring real money, because the executor
        // was deployed before the API. `serde` ignores unknown fields unless
        // a container asks it not to, and nothing in this workspace uses
        // `deny_unknown_fields` - this test is what makes that a decision
        // rather than a default nobody checked.
        let brokers = brokers_of(&data, "xau-ema");
        assert_eq!(brokers.len(), 1, "the account must still appear");
        assert_eq!(brokers[0].account, "vantage-cent");
        assert_eq!(brokers[0].login, Some(33_705_331));
        assert_eq!(brokers[0].balance, Some(10_000.0));
        assert_eq!(brokers[0].book_lots, Some(0.05));
        assert_eq!(brokers[0].closed, Some(3));
        assert_eq!(brokers[0].server_offset_ms, Some(10_800_000));
        assert!(brokers[0].drift.as_deref().is_some_and(|d| d.contains("0.10")));

        // The credit crosses the boundary WITH its three counts and its
        // currency. A reader given the amount alone could not tell a
        // measured figure from a guessed one, which is the one thing this
        // field must never let happen.
        let rebate = brokers[0].rebate.as_ref().expect("the credit is declared");
        assert_eq!(rebate.amount, Some(1.76));
        assert_eq!(rebate.currency.as_deref(), Some("USC"), "USC, not dollars");
        assert_eq!((rebate.exact, rebate.estimated, rebate.unpriced), (Some(1), Some(2), Some(0)));
        // And `realised` is untouched by it. This is the assertion that
        // fails if the credit is ever folded into the book.
        assert_eq!(brokers[0].realised, Some(-20.0), "the credit is beside the money, not in it");
        assert_eq!(rebate.realised_with_rebate, Some(-18.24));
        assert_eq!(brokers[0].fills[0].rebate, Some(0.88));
        assert_eq!(brokers[0].fills[0].rebate_basis.as_deref(), Some("ESTIMATED"));
        assert_eq!(brokers[0].fills[0].pnl, Some(9.87), "a fill's own P&L is untouched too");
    }

    #[test]
    fn what_is_declared_is_served_and_what_is_not_stops_here() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = account_with(&dir, "vantage-cent", "xau-ema", &snapshot_from_a_newer_executor());

        // `brokers_of` parses into `BrokerDto` and the status route serialises
        // the DTO, so what reaches the browser is the struct and not the file
        // - whatever the module comment's old "serves the file back unchanged"
        // suggested. The consequence is a rule with two halves, and this test
        // is both: a field this struct declares crosses, a field it does not
        // never leaves the process however faithfully the executor writes it.
        //
        // `server_offset_ms` and `drift` spent an afternoon on the wrong side
        // of that line - tolerated, parsed, and then dropped - which is why
        // the undeclared one below is kept in the fixture. The next field the
        // executor invents will be in the same position, and adding it to
        // `PaperBroker` in `ui/src/lib/api.ts` alone would declare something
        // that never arrives.
        let served = serde_json::to_value(&brokers_of(&data, "xau-ema")[0]).expect("serialise");
        assert_eq!(served["server_offset_ms"], 10_800_000, "declared, so carried: {served}");
        assert_eq!(served["drift"], "1 position holding 0.05 where the book wants 0.10");
        assert!(
            served.get("something_the_executor_learned_later").is_none(),
            "undeclared, so it stops here: {served}"
        );
        assert_eq!(served["login"], 33_705_331);
    }

    #[test]
    fn a_book_with_nothing_wrong_reports_neither_drift_nor_a_refusal() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The ordinary case, and the one an alert must stay silent on. All
        // three of these are `null` far more often than not, and a reader that
        // cannot tell absent from empty would fire on every healthy book.
        let quiet = json!({
            "at": 1_789_650_000_000_i64,
            "login": 33_705_331,
            "server_offset_ms": 10_800_000_i64,
        })
        .to_string();
        let data = account_with(&dir, "vantage-cent", "xau-ema", &quiet);
        let brokers = brokers_of(&data, "xau-ema");
        assert_eq!(brokers[0].drift, None);
        assert_eq!(brokers[0].blocked, None);
        assert_eq!(brokers[0].standing_out, None);
        assert_eq!(brokers[0].server_offset_ms, Some(10_800_000));
    }

    #[test]
    fn a_snapshot_from_an_older_executor_still_parses_too() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The other direction, which is the state the desk is in right now:
        // the API deployed ahead of the executors, so the fields the DTO
        // declares are simply absent. `#[serde(default)]` is what makes this
        // a missing reading rather than a refused file.
        let thin = json!({ "at": 1_789_650_000_000_i64, "login": 26_108_386 }).to_string();
        let data = account_with(&dir, "vantage-demo", "xau-ema", &thin);
        let brokers = brokers_of(&data, "xau-ema");
        assert_eq!(brokers.len(), 1);
        assert_eq!(brokers[0].balance, None, "absent is not zero");
        assert_eq!(brokers[0].closed, None);
        assert_eq!(brokers[0].server_offset_ms, None, "an older executor measured no clock");
        assert_eq!(brokers[0].drift, None);
        // `null`, not a credit of zero. An executor that predates the rebate
        // has not calculated one, and showing 0.00 for that would be the
        // desk asserting something nobody measured.
        assert!(brokers[0].rebate.is_none(), "absent is not a rebate of nothing");
    }

    #[test]
    fn a_snapshot_that_is_not_json_costs_its_own_account_and_no_other() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = account_with(&dir, "vantage-cent", "xau-ema", &snapshot_from_a_newer_executor());
        let torn = data.join("live").join("vantage-demo").join("xau-ema");
        std::fs::create_dir_all(&torn).expect("mkdir");
        std::fs::write(torn.join("broker.json"), "{\"at\": 178965000").expect("write");

        // A half-written snapshot is one account missing, not an empty desk.
        let brokers = brokers_of(&data, "xau-ema");
        assert_eq!(brokers.len(), 1);
        assert_eq!(brokers[0].account, "vantage-cent");
    }
}

/// The receipt for the numbers in [`events_of`].
    ///
    /// `fills.jsonl` shaped as the real ones are - a third trades, the rest
    /// events, about 210 bytes a line - at 2 MB, which is six months of the
    /// busiest book on the desk or four years of a typical one.
    #[test]
    #[ignore]
    fn tailing_events_against_reading_every_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = dir.path().to_path_buf();
        let run = run_dir(&data, "book");
        std::fs::create_dir_all(&run).expect("mkdir");
        let pad = "y".repeat(180);
        let mut body = String::new();
        let mut rows = 0;
        while body.len() < 2 * 1024 * 1024 {
            let kind = if rows % 3 == 0 { "trade" } else { "gap" };
            body.push_str(&format!("{{\"kind\":\"{kind}\",\"at\":{rows},\"p\":\"{pad}\"}}
"));
            rows += 1;
        }
        std::fs::write(run.join("fills.jsonl"), &body).expect("write");

        // What both did before: every line into a Value, then filter.
        let whole = || -> Vec<serde_json::Value> {
            std::fs::read_to_string(run.join("fills.jsonl"))
                .expect("read")
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .filter(|e| e.is_object() && e["kind"] != "trade")
                .collect()
        };

        let mut old = std::time::Duration::MAX;
        let mut tail = std::time::Duration::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let n = whole().len();
            old = old.min(t.elapsed());

            let t = std::time::Instant::now();
            let k = events_of(&data, "book", MAX_DETAIL_EVENTS).len();
            tail = tail.min(t.elapsed());

            assert!(n >= k, "the tail cannot hold more than the file does");
            assert_eq!(k, MAX_DETAIL_EVENTS);
        }
        println!("fills.jsonl {} bytes {rows} lines", body.len());
        println!("  detail: whole file {old:?} -> tail of {MAX_DETAIL_EVENTS} events {tail:?}");
    }

    #[test]
    #[ignore]
    fn reading_the_tail_against_reading_the_file() {
        // Both shapes, at the size they are today and at the size the writers
        // roll them at. Line lengths measured 2026-09-17 from the real files.
        bench("decisions-today", 6386, 632_258);
        bench("decisions-at-roll", 6386, 32 * 1024 * 1024);
        bench("executor-today", 221, 73_862);
        bench("executor-at-roll", 221, 32 * 1024 * 1024);
    }
}
