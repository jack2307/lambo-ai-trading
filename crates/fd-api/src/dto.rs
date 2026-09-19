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

/// Which file a chart series was read from.
#[derive(Debug, Serialize)]
pub struct BarSourceDto {
    /// The parquet read, relative to the data root.
    pub file: String,
    /// Its stored timeframe, which is not always the one asked for: `1h` may
    /// be built from `15m`.
    pub timeframe: String,
    /// True when the bars were rebucketed from a finer series. Only ever true
    /// for steps that divide an hour, where the broker's whole-hour offset
    /// makes the anchor irrelevant; `4h` and `1d` are refused instead.
    pub resampled: bool,
    /// Last write time of that file, UTC epoch ms. A stamp and not a duration,
    /// so a client ages it against its own clock.
    pub exported_at_ms: Option<i64>,
}

/// The bar still forming, aggregated from a finer stored series.
#[derive(Debug, Serialize)]
pub struct FormingDto {
    /// Where this bar starts: the last CLOSED bar's time plus the period, so
    /// it inherits the broker's anchor rather than assuming one.
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// The stored timeframe it was aggregated from.
    pub from_timeframe: String,
    /// The END of the last finer bar included: how far into this bar the high
    /// and low are actually KNOWN. Built from 15m bars they can be fifteen
    /// minutes stale while the price moves on, and a wick drawn as "the high
    /// so far" when the high is that old is the same lie a client-built
    /// forming candle tells, only smaller.
    pub complete_to_ms: i64,
}

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
    /// What this desk MEASURED this definition doing, one row per timeframe
    /// and parameter cell, or absent for anything nobody measured.
    ///
    /// ABSENT AND AN EMPTY LIST ARE DIFFERENT ANSWERS and the route never
    /// serves the second. `[]` reads as "measured, and there was nothing to
    /// report"; nothing at all is "unmeasured", which is the honest answer
    /// for eleven of the fourteen definitions here. The client then draws
    /// nothing in that slot — a grey "untested" where evidence goes is
    /// itself read as evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<Vec<MeasuredDto>>,
}

/// How a definition BEHAVED on a measured sample. Not a property of it.
///
/// The wire copy of `fd_indicators::Measured`, separate for the reason every
/// DTO here is: the crate's row is static data with borrowed names and a
/// slice of parameter pairs, which would serialise as an array of
/// two-element arrays. A client reading `params.anchor` should not have to
/// know that.
///
/// Same idiom as [`crate::htf::RuleMeasuredDto`], which carries the H1
/// structure row's numbers, and for the same reason: the parameters are a
/// definition and these are a measurement of one definition on one file over
/// one window. Definitions do not go stale and measurements do, so the
/// measurement travels with its timeframe, its sample size and the path of
/// the note it came from, and can be argued with.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredDto {
    /// The study's own name for the cell — `"avwap day"`, `"supertrend(10,3)"`.
    pub definition: String,
    /// The timeframe the numbers were measured ON. A number measured on
    /// 25,708 H1 bars is not a number about H4, so both rows are served
    /// rather than one being generalised.
    pub timeframe: String,
    /// The parameter cell these numbers describe. `avwap` anchored on the
    /// week is a different row from `avwap` anchored on the day — 9.2 flips
    /// per 100 bars against 19.7 — so a client showing the numbers for the
    /// viewer's own parameters matches on this.
    pub params: BTreeMap<String, f64>,
    pub flips_per_100_bars: f64,
    /// Share of this definition's own flips reversed within three bars. THE
    /// COST, and the number most likely to be left out of a summary: the
    /// favourable figures travel on their own.
    pub undone_within_3_pct: f64,
    pub median_lag_bars: f64,
    /// Share of reference turns the label never agreed with before the next
    /// one. Counted, never dropped — dropping them flatters a slow rule by
    /// deleting the turns it slept through.
    pub missed_pct: f64,
    pub sample_bars: usize,
    pub source: String,
    /// The study's own warning about this cell, where it wrote one. Present
    /// on `avwap day`, which the note names as one not to put on a card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caution: Option<String>,
}

impl From<&fd_indicators::Measured> for MeasuredDto {
    fn from(m: &fd_indicators::Measured) -> Self {
        Self {
            definition: m.definition.to_string(),
            timeframe: m.timeframe.to_string(),
            params: m.params.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            flips_per_100_bars: m.flips_per_100_bars,
            undone_within_3_pct: m.undone_within_3_pct,
            median_lag_bars: m.median_lag_bars,
            missed_pct: m.missed_pct,
            sample_bars: m.sample_bars,
            source: m.source.to_string(),
            caution: m.caution.map(str::to_string),
        }
    }
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

/// No `rename_all` here, deliberately, and it was removed rather than never
/// added.
///
/// The struct carried `rename_all = "camelCase"` and every field it had was a
/// single word - `market`, `symbol`, `timeframe`, `synthetic`, `live`, `bars`,
/// `stats` - so the attribute had never once changed a name. The first
/// multi-word fields added to it in 2026-09-18 came out as `barMs` and
/// `lastClosedBarMs` while `BarSourceDto` and `FormingDto`, which carry no
/// such attribute, stayed snake_case: one response, two conventions, and a
/// contract already handed to the client in the spelling the code did not use.
///
/// Snake_case is what the newer routes on this API emit (`htf.rs`) and what
/// the client was built against. Removing the attribute changes no existing
/// field, which is the only reason it is safe to remove rather than work
/// around.
#[derive(Debug, Serialize)]
pub struct BarsResponse {
    pub market: String,
    pub symbol: String,
    pub timeframe: String,
    /// The timeframe's period in milliseconds.
    ///
    /// NOT a way to compute where a bar starts. Bar times are the broker's own
    /// stamps and are not multiples of this from the epoch - real 4h candles
    /// open at 21:00 UTC in summer and 22:00 in winter. To find the bar
    /// containing an instant, SEARCH the series for the last bar at or before
    /// it; `floor(t / bar_ms) * bar_ms` is the epoch anchor and puts a marker
    /// three hours into the wrong candle.
    pub bar_ms: i64,
    /// The OPEN time of the last closed bar - the same stamp it carries in
    /// `bars`, so it compares directly against a bar and against an overlay
    /// time without anyone adding a period to it.
    pub last_closed_bar_ms: Option<i64>,
    /// Where these bars came from, and whether anything was inferred.
    pub source: BarSourceDto,
    /// The bar that has not closed yet, or `null` when nothing finer than this
    /// timeframe is stored to build one from. Never fabricated.
    pub forming: Option<FormingDto>,
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
    /// Positions a position guard closed, by its label. Empty unless
    /// `guards` was asked for.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub closed_by_guard: BTreeMap<String, usize>,
    /// Entries whose lots the notional cap reduced. Zero unless `guards`.
    #[serde(skip_serializing_if = "is_zero")]
    pub sized_down_by_guard: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
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
    /// `flat:1630-1815`, `vol:14/100:1.2-99`, `volabs:14:0.075-9`,
    /// `news:60-30` or `news:60-30:2` — minutes before/after scheduled news
    /// of impact ≥ 3, or the given level; a no-op when the server loaded no
    /// calendar). The same
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
