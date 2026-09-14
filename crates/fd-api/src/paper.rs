//! The paper loop's run: one strategy on closed bars, the book on disk.
//!
//! `docs/paper/DESIGN.md`. A run is one `market:tf`, one registered
//! strategy with its filters, the configured guards, and a
//! [`PaperBook`] — the engine's own fill model one bar at a time. Nothing
//! here can send an order; the only thing that executes is the JSON file
//! under `<data>/paper/<market>-<tf>/`.
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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
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

/// What a run was started with. `params` are the full parameters after the
/// overrides, so a reload needs no registry lookup to know what ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperConfig {
    pub market: String,
    pub tf: String,
    pub strategy: String,
    pub params: BTreeMap<String, f64>,
    pub filters: Vec<String>,
    pub guards: bool,
    pub window: usize,
}

impl PaperConfig {
    #[must_use]
    pub fn id(&self) -> String {
        run_id(&self.market, &self.tf)
    }
}

#[must_use]
pub fn run_id(market: &str, tf: &str) -> String {
    format!("{market}-{tf}")
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
        if ![bar.open, bar.high, bar.low, bar.close].iter().all(|v| v.is_finite()) || bar.high < bar.low {
            return Err(ApiError::BadRequest(format!("malformed bar at {}: prices must be finite and high >= low", bar.time)));
        }
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
fn resolve<'a>(registry: &'a Registry, config: &PaperConfig) -> Result<(Filtered<'a>, Params), ApiError> {
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
        .map(|f| Filter::parse(f))
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

/// Every `state.json` under `<data>/paper/`, keyed by run id. A file that
/// does not parse is reported on stderr and skipped rather than taking the
/// process down: the other runs are still worth keeping.
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
            Ok(run) => {
                runs.insert(run.config.id(), run);
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

#[derive(Debug, Deserialize)]
pub struct RunKey {
    pub market: String,
    pub tf: String,
}

#[derive(Debug, Serialize)]
pub struct BarResponse {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    /// Bars in the window after this one.
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
}

#[derive(Debug, Serialize)]
pub struct NewsDto {
    pub events_loaded: usize,
    pub next_blackout: Option<BlackoutDto>,
}

#[derive(Debug, Serialize)]
pub struct RunStatus {
    pub id: String,
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
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub runs: Vec<RunStatus>,
}

fn status_of(run: &PaperRun, rules: &TradingRules, guards: Option<&Guards>) -> RunStatus {
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
    // event at or after now with impact at or above the guards' threshold.
    let min_impact = guards.map_or(3, |g| g.news_min_impact);
    let now = now_ms();
    let events = fd_strategy::news::events();
    let next_blackout = events
        .iter()
        .find(|e| e.time >= now && e.impact >= min_impact)
        .map(|e| BlackoutDto { time: e.time, currency: e.currency.clone(), impact: e.impact });
    let skip = book.trades.len().saturating_sub(10);
    RunStatus {
        id: run.config.id(),
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
    }
}

/// The rules and guards a run steps with, from the config the process
/// loaded — not persisted with the run, so a config change applies at the
/// next bar, and the status reflects it.
fn rules_and_guards(state: &AppState, config: &PaperConfig) -> Result<(TradingRules, Option<Guards>), ApiError> {
    let rules = state.trading_rules(&config.market)?;
    let guards = config.guards.then(|| Guards::from_config(&state.config));
    Ok((rules, guards))
}

/* ---------------- handlers ---------------- */

/// `POST /api/paper/start`
pub async fn start(State(state): State<Arc<AppState>>, Json(request): Json<StartRequest>) -> Result<Json<RunStatus>, ApiError> {
    if !TIMEFRAMES.contains(&request.tf.as_str()) {
        return Err(ApiError::BadRequest(format!("unknown timeframe: {}", request.tf)));
    }
    state.config.market(&request.market).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let window = request.window.max(1);
    let config = PaperConfig {
        market: request.market,
        tf: request.tf,
        strategy: request.strategy,
        params: request.params,
        filters: request.filters,
        guards: request.guards,
        window,
    };
    let (strategy, params) = resolve(&state.registry, &config)?;
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
    let (rules, guards) = rules_and_guards(&state, &config)?;

    // Warm-up from the store: the last `window` bars, or none when the
    // store has nothing for this market yet — the poller will supply them.
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
        return Err(ApiError::Conflict(format!("a paper run for {id} exists; stop it first")));
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
    let status = status_of(&run, &rules, guards.as_ref());
    runs.insert(id, run);
    Ok(Json(status))
}

/// `POST /api/paper/bar`
pub async fn bar(State(state): State<Arc<AppState>>, Json(request): Json<BarRequest>) -> Result<Json<BarResponse>, ApiError> {
    let id = run_id(&request.market, &request.tf);
    let bar = Bar {
        time: request.bar.time,
        open: request.bar.open,
        high: request.bar.high,
        low: request.bar.low,
        close: request.bar.close,
        volume: request.bar.volume,
    };
    let mut runs = state.paper.lock().expect("paper runs");
    let run = runs.get_mut(&id).ok_or_else(|| ApiError::NotFound(format!("no paper run for {id}")))?;
    let (strategy, params) = resolve(&state.registry, &run.config)?;
    let (rules, guards) = rules_and_guards(&state, &run.config)?;
    let bar_ms = timeframe_ms(&run.config.tf).unwrap_or(0);

    let (report, gap) = match run.accept(bar, &strategy, &params, &rules, guards.as_ref(), bar_ms)? {
        Accepted::Seen => {
            return Ok(Json(BarResponse {
                accepted: false,
                reason: Some("seen"),
                bars: run.bars.len(),
                closed: Vec::new(),
                opened: false,
                refused: None,
                gap: None,
            }));
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

    Ok(Json(BarResponse {
        accepted: true,
        reason: None,
        bars: run.bars.len(),
        closed: report.trades.iter().map(TradeDto::from).collect(),
        opened: report.opened,
        refused: report.refused,
        gap,
    }))
}

/// `POST /api/paper/stop`
pub async fn stop(State(state): State<Arc<AppState>>, Json(request): Json<RunKey>) -> Result<Json<StopResponse>, ApiError> {
    let id = run_id(&request.market, &request.tf);
    let mut runs = state.paper.lock().expect("paper runs");
    let mut run = runs.remove(&id).ok_or_else(|| ApiError::NotFound(format!("no paper run for {id}")))?;
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
        out.push(status_of(run, &rules, guards.as_ref()));
    }
    Ok(Json(StatusResponse { runs: out }))
}
