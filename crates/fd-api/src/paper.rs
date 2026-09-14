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
use fd_backtest::{Guards, PaperBook, StepReport, Trade, TradingRules};
use fd_core::types::Bar;
use fd_indicators::compute_indicators;
use fd_store::timeframe_ms;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Registry, Strategy};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::TradeDto;
use crate::error::ApiError;
use crate::state::{AppState, TIMEFRAMES};

/// Bars kept when a start does not say.
pub const DEFAULT_WINDOW: usize = 600;

/// The longest run id accepted.
pub const MAX_ID_LEN: usize = 40;

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
    Stepped { report: StepReport, gap: Option<usize> },
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

        let report = self.book.step(&bars[i], atr_prev, rules, guards, bar_ms, |position| {
            if i < warmup {
                return Intent::None;
            }
            let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series: &resolved, options: None, position, params };
            strategy.on_bar(&ctx)
        });
        Ok(Accepted::Stepped { report, gap })
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
                if let Some(previous) = runs.insert(id.clone(), run) {
                    eprintln!("paper: two state files claim run `{id}`; keeping {}, the earlier one was for {}", path.display(), previous.config.stream());
                }
            }
            Err(e) => eprintln!("paper: could not reload {}: {e}", path.display()),
        }
    }
    runs
}

fn now_ms() -> i64 {
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
}

#[derive(Debug, Serialize)]
pub struct RunStatus {
    pub id: String,
    pub label: Option<String>,
    pub market: String,
    pub tf: String,
    pub strategy: String,
    pub params: BTreeMap<String, f64>,
    pub filters: Vec<String>,
    pub guards: bool,
    pub started_at: i64,
    /// Bars in the window.
    pub bars: usize,
    pub bars_seen: usize,
    pub warmup_bars: usize,
    pub last_bar_time: Option<i64>,
    pub equity: f64,
    pub open: Option<OpenDto>,
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

fn status_of(data: &Path, run: &PaperRun, rules: &TradingRules, guards: Option<&Guards>) -> RunStatus {
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
    });
    // The next blackout the run's guards would act on: the first installed
    // event at or after now with impact at or above the guards' threshold
    // **of the market's currencies** (`rules.news_currencies`; `All` events
    // count). A Canadian rate decision is not a gold run's next blackout.
    let min_impact = guards.map_or(3, |g| g.news_min_impact);
    let now = now_ms();
    let events = fd_strategy::news::events();
    let next_blackout = events
        .iter()
        .find(|e| e.time >= now && e.impact >= min_impact && e.concerns(Some(&rules.news_currencies)))
        .map(|e| BlackoutDto { time: e.time, currency: e.currency.clone(), impact: e.impact, name: e.name.clone() });
    let skip = book.trades.len().saturating_sub(10);
    RunStatus {
        id: run.config.id(),
        label: run.config.label.clone(),
        market: run.config.market.clone(),
        tf: run.config.tf.clone(),
        strategy: run.config.strategy.clone(),
        params: run.config.params.clone(),
        filters: run.config.filters.clone(),
        guards: run.config.guards,
        started_at: run.started_at,
        bars: run.bars.len(),
        bars_seen: run.bars_seen,
        warmup_bars: run.warmup_bars,
        last_bar_time: run.bars.last().map(|b| b.time),
        equity: fd_core::js_round_to(book.equity, 2),
        open,
        trades: book.trades.len(),
        net_usd: metrics.net_pnl_usd,
        profit_factor: metrics.profit_factor,
        skipped_by_guard: book.skipped_by_guard.clone(),
        closed_by_guard: book.closed_by_guard.clone(),
        sized_down: book.sized_down_by_guard,
        skipped_no_atr: book.skipped_no_atr,
        gaps: run.gaps,
        news: NewsDto { events_loaded: events.len(), next_blackout },
        last_fills: book.trades[skip..].iter().map(TradeDto::from).collect(),
        equity_curve: book.equity_curve.len(),
        events: events_of(data, &run.config.id()).len(),
    }
}

/// The rules and guards a run steps with, from the config the process
/// loaded — not persisted with the run, so a config change applies at the
/// next bar, and the status reflects it.
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
    let status = status_of(&state.data, &run, &rules, guards.as_ref());
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

    let (report, gap) = match run.accept(bar, &strategy, &params, &rules, guards.as_ref(), bar_ms)? {
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
        Accepted::Stepped { report, gap } => (report, gap),
    };

    persist(&state.data, run)?;
    if let Some(missing) = gap {
        record(&state.data, &id, &json!({ "kind": "gap", "time": bar.time, "missing_bars": missing }))?;
    }
    if let Some(why) = &report.refused {
        record(&state.data, &id, &json!({ "kind": "refused", "time": bar.time, "reason": why }))?;
    }
    if report.no_risk {
        record(&state.data, &id, &json!({ "kind": "refused", "time": bar.time, "reason": "NO_RISK_UNIT" }))?;
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
        out.push(status_of(&state.data, run, &rules, guards.as_ref()));
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
    let status = status_of(&state.data, run, &rules, guards.as_ref());

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

    Ok(Json(RunDetail { run: status, equity_curve, fills, events, bars }))
}
