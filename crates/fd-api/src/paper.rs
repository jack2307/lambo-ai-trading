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
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as PathParam, Query, State};
use fd_backtest::engine::{ensure_fallback_atr, sizing_atr_key};
use fd_backtest::paper::Advice;
use fd_backtest::{Guards, PaperBook, StepReport, Trade, TradingRules};
use fd_core::types::Bar;
use fd_indicators::compute_indicators;
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

/// The run's `fills.jsonl` less its `trade` lines, oldest first: the
/// `started`, `gap`, `refused`, `guard_close` and `stopped` events. Read
/// line by line; a line that is not a JSON object (a write cut short by a
/// crash) is skipped, not fatal — the book in `state.json` is the record,
/// this file is its narration. No file (a run that never wrote one) is no
/// events.
fn events_of(data: &Path, id: &str) -> Vec<serde_json::Value> {
    let Ok(file) = std::fs::File::open(run_dir(data, id).join("fills.jsonl")) else { return Vec::new() };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
        .filter(|event| event.is_object() && event["kind"] != "trade")
        .collect()
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
    pub open: Option<OpenDto>,
    /// An entry decided at the last close and waiting for the next open. Never
    /// set at the same time as a fill on the same bar: the book takes one
    /// position, so a pending entry means the book is flat right now and will
    /// not be after the next bar opens.
    pub pending: Option<PendingDto>,
    pub trades: usize,
    pub net_usd: f64,
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
    /// Event lines in the run's `fills.jsonl` other than trades (started,
    /// gaps, refusals, guard closes, stopped). The lines themselves are on
    /// `GET /api/paper/run/{id}`.
    pub events: usize,
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

fn status_of(data: &Path, run: &PaperRun, rules: &TradingRules, guards: Option<&Guards>, live: Option<LiveBar>) -> RunStatus {
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
        open,
        pending,
        trades: book.trades.len(),
        net_usd: metrics.net_pnl_usd,
        profit_factor: metrics.profit_factor,
        skipped_by_guard: book.skipped_by_guard.clone(),
        closed_by_guard: book.closed_by_guard.clone(),
        sized_down: book.sized_down_by_guard,
        skipped_no_atr: book.skipped_no_atr,
        gaps: run.gaps,
        news: NewsDto { events_loaded: events.len(), next_blackout, horizon, horizon_days, horizon_name },
        live,
        last_fills: book.trades[skip..].iter().map(TradeDto::from).collect(),
        equity_curve: book.equity_curve.len(),
        events: events_of(data, &run.config.id()).len(),
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
    let guards = config
        .guards
        .then(|| Guards::for_market(&state.config, &config.market))
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok((rules, guards))
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
        book: PaperBook::new(&rules, strategy.exits() == Exits::Strategy),
        // Cloned from the book on its first bar; see the field's own note.
        shadow: None,
        advice: None,
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
    let status = status_of(&state.data, &run, &rules, guards.as_ref(), state.live_bar(&run.config.market, &run.config.tf));
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

    let (report, gap, applied, shadow_closed) = match run.accept(bar, &strategy, &params, &rules, guards.as_ref(), bar_ms)? {
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

    persist(&state.data, run)?;
    if let Some(missing) = gap {
        record(&state.data, &id, &json!({ "kind": "gap", "time": bar.time, "missing_bars": missing }))?;
    }
    if let Some(why) = &report.refused {
        record(&state.data, &id, &json!({ "kind": "refused", "time": bar.time, "reason": why }))?;
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
        record(&state.data, &id, &trade_event("trade", trade))?;
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
/// The bar still **forming** on a `market:tf`, for the screen only.
///
/// This handler touches **no book, no run and no file**. It validates the
/// bar exactly as [`bar`] validates a closed one — finite prices, `high >=
/// low`, a timeframe the API serves — stores it in `AppState::live_bars`
/// under `market:tf` with the server's own clock, and returns. It never
/// takes the paper mutex, so a tick cannot delay or interleave with a
/// closed bar's step, and nothing it stores can reach a decision: the only
/// thing that appends to a run's window is [`PaperRun::accept`], from
/// [`bar`].
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
    // Anyone watching hears it now rather than on their next poll. `send`
    // fails only when nobody is subscribed, which is the normal case and not
    // an error: the bar is already stored and `/status` will carry it.
    let _ = state.ticks.send(TickEvent { market: request.market, tf: request.tf, stream: key, live });
    Ok(Json(TickResponse { stored: true }))
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
    let closed = run.book.stop(&rules);
    if let Some(trade) = &closed {
        record(&state.data, &id, &trade_event("trade", trade))?;
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

/// `GET /api/paper/status`
pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<StatusResponse>, ApiError> {
    let runs = state.paper.lock().expect("paper runs");
    let mut out = Vec::with_capacity(runs.len());
    for run in runs.values() {
        let (rules, guards) = rules_and_guards(&state, &run.config)?;
        let live = state.live_bar(&run.config.market, &run.config.tf);
        out.push(status_of(&state.data, run, &rules, guards.as_ref(), live));
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
    let status = status_of(&state.data, run, &rules, guards.as_ref(), live.clone());

    let book = &run.book;
    let mut equity_curve = Vec::with_capacity(book.equity_curve.len() + 1);
    equity_curve.push((run.started_at, rules.starting_equity_usd));
    equity_curve.extend_from_slice(&book.equity_curve);

    let skip = book.trades.len().saturating_sub(MAX_DETAIL_FILLS);
    let fills = book.trades[skip..].iter().map(TradeDto::from).collect();

    let mut events = events_of(&state.data, &id);
    let skip = events.len().saturating_sub(MAX_DETAIL_EVENTS);
    events.drain(..skip);

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
        let Some(Intent::Enter { side, stop, target, reason }) = run.book.pending() else { continue };
        let (rules, _) = rules_and_guards(&state, &run.config)?;
        let Some(last) = run.bars.last() else { continue };
        let tail: Vec<_> =
            run.bars.iter().rev().take(120).rev().map(|b| (b.time, b.open, b.high, b.low, b.close)).collect();
        let intent_id = run.pending_id();
        out.push(PendingEntry {
            run: run.config.id(),
            advised: run.advice.as_ref().is_some_and(|a| a.intent_id == intent_id),
            intent_id,
            market: run.config.market.clone(),
            tf: run.config.tf.clone(),
            strategy: run.config.strategy.clone(),
            label: run.config.label.clone(),
            params: run.config.params.clone(),
            filters: run.config.filters.clone(),
            side: format!("{side:?}").to_uppercase(),
            stop: *stop,
            target: *target,
            reason: reason.clone(),
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
    let current = run.pending_id();
    let fresh = current == body.intent_id && matches!(run.book.pending(), Some(Intent::Enter { .. }));

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
}

#[derive(Debug, Serialize)]
pub struct IntentResponse {
    pub accepted: bool,
    /// Why not, when not: the run's last bar against the one named.
    pub reason: String,
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
    let mut runs = state.paper.lock().expect("paper runs");
    let run = runs
        .get_mut(&body.run)
        .ok_or_else(|| ApiError::NotFound(format!("no paper run `{}`", body.run)))?;

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

    if let Some(side) = side {
        run.book.decide(Intent::Enter {
            side,
            stop: body.stop.filter(|v| v.is_finite()),
            target: body.target.filter(|v| v.is_finite()),
            reason: if body.reason.is_empty() { "external".to_string() } else { body.reason.clone() },
        });
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
    record(
        &state.data,
        &body.run,
        &json!({
            "kind": "intent",
            "time": body.bar_time,
            "side": body.side.to_ascii_uppercase(),
            "stop": body.stop,
            "target": body.target,
            "reason": body.reason,
            "decider": body.decider,
        }),
    )?;
    // **Before answering, not after.** This route tells the caller "accepted,
    // fills at the next bar's open", and that promise has to survive the
    // fifteen minutes until that bar arrives. The pending intent lived only in
    // memory until now, so a restart inside that window — a deploy, a crash —
    // silently cancelled a trade the model had been told was on. It was
    // observed doing exactly that: two stand-asides recorded at 16:15 were
    // gone from both badges after a 16:17 restart, and an entry would have
    // vanished the same way, without a line anywhere saying so.
    persist(&state.data, run)?;
    Ok(Json(IntentResponse { accepted: true, reason: "fills at the next bar's open".to_string() }))
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

/// The last `limit` lines of a JSONL file, oldest first, skipping unreadable
/// ones rather than failing: a half-written last line (the writer was killed
/// mid-append) must not take the whole panel down.
fn tail_jsonl(path: &Path, limit: usize) -> Vec<serde_json::Value> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(limit))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
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

/// The conversation log: its own file, never `fills.jsonl`.
///
/// Kept apart because the two have different lifetimes and different readers.
/// `fills.jsonl` is what the book did and is replayed on restart; this is what
/// was said about it, is never replayed, and will be orders of magnitude
/// larger once whole prompts are in it. Mixing them would make the book's own
/// reload scan megabytes of transcript to find its trades.
fn log_consultation(data: &Path, id: &str, event: &serde_json::Value) -> Result<(), ApiError> {
    let dir = run_dir(data, id);
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(format!("paper: {}: {e}", dir.display())))?;
    let path = dir.join("advice.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))?;
    use std::io::Write as _;
    writeln!(file, "{event}").map_err(|e| ApiError::Internal(format!("paper: {}: {e}", path.display())))?;
    Ok(())
}
