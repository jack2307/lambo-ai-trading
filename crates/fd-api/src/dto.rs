//! Wire shapes.
//!
//! Spelled out here rather than serialised straight off the engine types, for
//! one reason: the browser is a separate program with its own release cycle,
//! and letting a field rename inside `fd-backtest` silently change the API
//! would make every internal refactor a potential UI outage.
//!
//! Field names are camelCase because that is what the existing client reads.
//! The Rust side stays snake_case; the translation happens once, here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketInfo {
    pub id: String,
    pub label: String,
    pub bar_symbol: String,
    pub bar_source: String,
    pub options_source: String,
    /// False when the store holds nothing for this market yet, so the client
    /// can say "no data" rather than drawing an empty chart.
    pub has_data: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorInfo {
    pub id: String,
    pub name: String,
    /// `overlay` draws on the candles; `pane` gets its own strip.
    pub pane: &'static str,
    pub params: BTreeMap<String, f64>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub params: BTreeMap<String, f64>,
    pub grid: Option<BTreeMap<String, Vec<f64>>>,
    pub needs_options: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    pub timeframe: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub markets: Vec<MarketInfo>,
    pub active_market: String,
    pub timeframes: Vec<&'static str>,
    pub indicators: Vec<IndicatorInfo>,
    pub strategies: Vec<StrategyInfo>,
    pub defaults: Defaults,
    /// Stated on every catalog because a fill model is an assumption, and an
    /// assumption that is never restated stops being questioned.
    pub fill_model: String,
}

#[derive(Debug, Serialize)]
pub struct BarDto {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarStats {
    pub bars: usize,
    pub from: i64,
    pub to: i64,
    pub days: f64,
    /// Holes in the series. Reported rather than filled: an invented bar is
    /// worse than a visible gap.
    pub gaps: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarsResponse {
    pub market: String,
    pub symbol: String,
    pub timeframe: String,
    /// True when the source publishes closes only, so the bars are flat.
    pub synthetic: bool,
    /// True when this market has an upstream websocket. Markets without one
    /// are snapshots and must not be shown with a live badge.
    pub live: bool,
    pub bars: Vec<BarDto>,
    pub stats: BarStats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDto {
    pub trades: usize,
    pub win_rate: f64,
    pub avg_r: f64,
    pub avg_win_r: f64,
    pub avg_loss_r: f64,
    pub profit_factor: f64,
    pub expectancy: f64,
    pub total_r: f64,
    pub net_pnl_usd: f64,
    pub return_pct: f64,
    pub max_drawdown_usd: f64,
    pub max_drawdown_pct: f64,
    pub sharpe: f64,
    pub avg_mae: f64,
    pub avg_mfe: f64,
    pub avg_hold_min: f64,
    pub exits: BTreeMap<String, usize>,
}

impl From<&fd_backtest::Metrics> for MetricsDto {
    fn from(m: &fd_backtest::Metrics) -> Self {
        Self {
            trades: m.trades,
            win_rate: m.win_rate,
            avg_r: m.avg_r,
            avg_win_r: m.avg_win_r,
            avg_loss_r: m.avg_loss_r,
            profit_factor: m.profit_factor,
            expectancy: m.expectancy,
            total_r: m.total_r,
            net_pnl_usd: m.net_pnl_usd,
            return_pct: m.return_pct,
            max_drawdown_usd: m.max_drawdown_usd,
            max_drawdown_pct: m.max_drawdown_pct,
            sharpe: m.sharpe,
            avg_mae: m.avg_mae,
            avg_mfe: m.avg_mfe,
            avg_hold_min: m.avg_hold_min,
            exits: m.exits.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeDto {
    pub direction: &'static str,
    pub entry_time: i64,
    pub entry_price: f64,
    pub exit_time: i64,
    pub exit_price: f64,
    pub exit_reason: String,
    pub stop: f64,
    /// `null` for a strategy that manages its own exit. The client draws no
    /// target band for those rather than inventing one.
    pub target: Option<f64>,
    pub lots: f64,
    pub pnl_usd: f64,
    pub r: f64,
    pub mae: f64,
    pub mfe: f64,
    pub hold_ms: i64,
    pub reason: String,
}

impl From<&fd_backtest::Trade> for TradeDto {
    fn from(t: &fd_backtest::Trade) -> Self {
        Self {
            direction: if t.direction.is_long() { "LONG" } else { "SHORT" },
            entry_time: t.entry_time,
            entry_price: t.entry_price,
            exit_time: t.exit_time,
            exit_price: t.exit_price,
            exit_reason: t.exit_reason.clone(),
            stop: t.stop,
            target: t.target,
            lots: t.lots,
            pnl_usd: t.pnl_usd,
            r: t.r,
            mae: t.mae,
            mfe: t.mfe,
            hold_ms: t.hold_ms,
            reason: t.reason.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VerdictDto {
    pub promising: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquityPoint {
    pub time: i64,
    pub equity: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorSpecDto {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, f64>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResponse {
    pub market: String,
    pub strategy: String,
    pub name: String,
    pub timeframe: String,
    pub params: BTreeMap<String, f64>,
    pub metrics: MetricsDto,
    pub verdict: VerdictDto,
    pub trades: Vec<TradeDto>,
    pub equity_curve: Vec<EquityPoint>,
    pub indicator_specs: Vec<IndicatorSpecDto>,
    /// Entries a risk guard refused, by reason. Empty unless `guards` was
    /// asked for — the default reproduces the oracle, which had none.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub skipped_by_guard: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardRowDto {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trades: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardResponse {
    pub market: String,
    pub timeframe: String,
    pub rows: Vec<LeaderboardRowDto>,
    pub note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterDto {
    pub low: f64,
    pub high: f64,
    pub center: f64,
    pub score: f64,
    pub types: usize,
    pub expirations: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDto {
    pub symbol: String,
    pub dte: f64,
    pub max_pain: Option<f64>,
    pub poc: Option<f64>,
    #[serde(rename = "wSup")]
    pub w_sup: Option<f64>,
    #[serde(rename = "wRes")]
    pub w_res: Option<f64>,
    #[serde(rename = "callBE")]
    pub call_be: Option<f64>,
    #[serde(rename = "putBE")]
    pub put_be: Option<f64>,
    pub bull_ratio: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameDto {
    pub t: i64,
    pub spot: f64,
    pub bull_ratio: f64,
    pub bull_ratio_15m: f64,
    pub net_flow_velocity_norm: f64,
    pub big_trade_imbalance: f64,
    pub clusters: Vec<ClusterDto>,
    pub contexts: Vec<ContextDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsResponse {
    pub frame: Option<FrameDto>,
    pub frames: usize,
    pub live: bool,
}

#[derive(Debug, Serialize)]
pub struct Point {
    pub time: i64,
    pub value: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorsResponse {
    pub timeframe: String,
    pub series: BTreeMap<String, Vec<Point>>,
}

/* ---------------- request bodies ---------------- */

#[derive(Debug, Deserialize)]
pub struct SpecRequest {
    pub id: String,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
pub struct IndicatorsRequest {
    pub market: Option<String>,
    pub tf: Option<String>,
    #[serde(default)]
    pub specs: Vec<SpecRequest>,
}

#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub market: Option<String>,
    pub tf: Option<String>,
    pub strategy: String,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    /// Enforce `[trading.guards]` at every entry. Off by default so the
    /// browser sees the same numbers the oracle produced.
    #[serde(default)]
    pub guards: bool,
    /// Gates in front of the strategy's entries, in the batch-file spelling
    /// (`weekdays`, `hours:0800-1200`, `sessions:0100-0500|0600-1000`,
    /// `flat:1630-1815`, `vol:14/100:1.2-99`, `volabs:14:0.075-9`). The same
    /// wrapper the research loop uses, so a Workbench run and a hypothesis
    /// row are the same computation.
    #[serde(default)]
    pub filters: Vec<String>,
    /// UTC dates, `YYYY-MM-DD`, both inclusive; absent means the whole
    /// stored series. The same bounds `search --from/--to` takes, so a
    /// Workbench run over a month is the batch's computation over a month.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}
