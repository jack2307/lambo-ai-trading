//! The chart API.
//!
//! Same six endpoints and the same wire shapes the prototype served, because
//! Phase 4's gate is that the browser cannot tell which backend answered. What
//! changed is underneath: the numbers come from the Parquet store and the Rust
//! engines rather than from a JSON cache and the JavaScript ones.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use fd_backtest::engine::{Range, run_backtest_guarded};
use fd_backtest::Guards;
use fd_backtest::sweep::{compare_strategies, verdict};
use fd_core::types::Bar;
use fd_strategy::filter::{Filter, Filtered};
use fd_indicators::{INDICATORS, IndicatorSpec, Pane, compute_indicators};
use serde::Deserialize;

use crate::dto::*;
use crate::error::ApiError;
use crate::state::{AppState, TIMEFRAMES};

/// A day, for the bar-span summary.
const DAY_MS: f64 = 86_400_000.0;

#[derive(Debug, Deserialize)]
pub struct MarketQuery {
    pub market: Option<String>,
    pub tf: Option<String>,
}

impl MarketQuery {
    fn market(&self, state: &AppState) -> String {
        self.market.clone().unwrap_or_else(|| state.config.market.clone())
    }

    fn timeframe(&self, state: &AppState) -> String {
        self.tf.clone().unwrap_or_else(|| state.config.backtest.timeframe.clone())
    }
}

pub async fn catalog(State(state): State<Arc<AppState>>) -> Result<Json<Catalog>, ApiError> {
    let markets = state
        .config
        .markets
        .keys()
        .filter_map(|id| {
            let spec = state.config.market(id).ok()?;
            Some(MarketInfo {
                id: id.clone(),
                label: spec.label.clone(),
                bar_symbol: spec.bar_symbol.clone(),
                bar_source: format!("{:?}", spec.bar_source).to_lowercase(),
                options_source: format!("{:?}", spec.options_source).to_lowercase(),
                // Asked of the store, not assumed from the config: a market
                // that is configured but never ingested must not look ready.
                has_data: state.bars(id, &state.config.backtest.timeframe).is_ok(),
            })
        })
        .collect();

    let indicators = INDICATORS
        .iter()
        .map(|def| IndicatorInfo {
            id: def.id.to_string(),
            name: def.name.to_string(),
            pane: if def.pane == Pane::Overlay { "overlay" } else { "pane" },
            params: def.params.iter().map(|(name, value)| ((*name).to_string(), *value)).collect(),
            outputs: def.outputs.iter().map(|o| (*o).to_string()).collect(),
        })
        .collect();

    let strategies = state
        .registry
        .all()
        .iter()
        .map(|s| {
            let grid = s.grid();
            StrategyInfo {
                id: s.id().to_string(),
                name: s.name().to_string(),
                description: s.description().to_string(),
                params: s.default_params().0.clone(),
                grid: (!grid.is_empty()).then_some(grid),
                needs_options: s.needs_options(),
            }
        })
        .collect();

    Ok(Json(Catalog {
        markets,
        active_market: state.config.market.clone(),
        timeframes: TIMEFRAMES.to_vec(),
        indicators,
        strategies,
        defaults: Defaults { timeframe: state.config.backtest.timeframe.clone() },
        fill_model: "Signals fill at the NEXT bar's open; a stop wins when a bar covers both stop and target."
            .to_string(),
    }))
}

pub async fn bars(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MarketQuery>,
) -> Result<Json<BarsResponse>, ApiError> {
    let market = query.market(&state);
    let timeframe = query.timeframe(&state);
    let series = state.bars(&market, &timeframe)?;
    let step = fd_store::timeframe_ms(&timeframe).unwrap_or(900_000);

    let (from, to) = match (series.bars.first(), series.bars.last()) {
        (Some(first), Some(last)) => (first.time, last.time),
        _ => (0, 0),
    };

    Ok(Json(BarsResponse {
        market,
        symbol: series.symbol.clone(),
        timeframe,
        synthetic: series.synthetic,
        live: state.is_live(&query.market(&state)),
        bars: series.bars.iter().map(to_bar_dto).collect(),
        stats: BarStats {
            bars: series.bars.len(),
            from,
            to,
            days: if to > from { (to - from) as f64 / DAY_MS } else { 0.0 },
            gaps: fd_ingest::gaps(&series.bars, step).len(),
        },
    }))
}

fn to_bar_dto(bar: &Bar) -> BarDto {
    BarDto { time: bar.time, open: bar.open, high: bar.high, low: bar.low, close: bar.close, volume: bar.volume }
}

pub async fn levels(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MarketQuery>,
) -> Result<Json<LevelsResponse>, ApiError> {
    let market = query.market(&state);
    let Some(timeline) = state.timeline(&market) else {
        // Not an error: a market with no tape has no levels, and saying so is
        // different from failing.
        return Ok(Json(LevelsResponse { frame: None, frames: 0, live: false }));
    };
    let frames = timeline.frames();
    Ok(Json(LevelsResponse {
        frame: frames.last().map(frame_dto),
        frames: frames.len(),
        live: state.is_live(&market),
    }))
}

pub fn frame_dto(frame: &fd_backtest::Frame) -> FrameDto {
    FrameDto {
        t: frame.t,
        spot: frame.spot,
        bull_ratio: frame.bull_ratio,
        bull_ratio_15m: frame.bull_ratio_15m,
        net_flow_velocity_norm: frame.net_flow_velocity_norm,
        big_trade_imbalance: frame.big_trade_imbalance,
        clusters: frame
            .clusters
            .iter()
            .map(|c| ClusterDto {
                low: c.low,
                high: c.high,
                center: c.center,
                score: c.score,
                // The frame keeps only what a strategy may read; the cluster's
                // composition counts are not part of that, so they are reported
                // as zero rather than invented.
                types: 0,
                expirations: 0,
            })
            .collect(),
        contexts: frame
            .contexts
            .iter()
            .map(|c| ContextDto {
                symbol: c.symbol.clone(),
                dte: c.dte,
                max_pain: c.max_pain,
                poc: c.poc,
                w_sup: c.w_sup,
                w_res: c.w_res,
                call_be: c.call_be,
                put_be: c.put_be,
                bull_ratio: c.bull_ratio,
            })
            .collect(),
    }
}

pub async fn leaderboard(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MarketQuery>,
) -> Result<Json<LeaderboardResponse>, ApiError> {
    let market = query.market(&state);
    let timeframe = query.timeframe(&state);
    let series = state.bars(&market, &timeframe)?;
    let timeline = state.timeline(&market);
    let rules = state.trading_rules(&market)?;
    let gate = state.gate();

    let rows = compare_strategies(&state.registry, &series.bars, &rules, timeline.as_deref(), &gate)
        .into_iter()
        .map(|row| LeaderboardRowDto {
            id: row.id,
            name: row.name,
            skipped: row.skipped,
            params: row.params.map(|p| p.0),
            trades: row.metrics.as_ref().map(|m| m.trades),
            metrics: row.metrics.as_ref().map(MetricsDto::from),
        })
        .collect();

    Ok(Json(LeaderboardResponse {
        market,
        timeframe,
        rows,
        note: "Default parameters, in sample. A leaderboard ranks methods; it does not measure any of them."
            .to_string(),
    }))
}

pub async fn indicators(
    State(state): State<Arc<AppState>>,
    Json(request): Json<IndicatorsRequest>,
) -> Result<Json<IndicatorsResponse>, ApiError> {
    let market = request.market.unwrap_or_else(|| state.config.market.clone());
    let timeframe = request.tf.unwrap_or_else(|| state.config.backtest.timeframe.clone());
    let series = state.bars(&market, &timeframe)?;

    let specs: Vec<IndicatorSpec> = request
        .specs
        .iter()
        .map(|spec| {
            let mut built = IndicatorSpec::new(&spec.id);
            for (name, value) in &spec.params {
                built = built.with(name, *value);
            }
            built
        })
        .collect();

    let computed = compute_indicators(&series.bars, &specs)
        .map_err(|error| ApiError::BadRequest(error.to_string()))?;

    let mut out: BTreeMap<String, Vec<Point>> = BTreeMap::new();
    for (key, values) in computed {
        // A bare alias and its qualified key hold the same series; sending both
        // would double the payload for nothing. The client addresses series by
        // the qualified name.
        if !key.contains('.') {
            continue;
        }
        let points = series
            .bars
            .iter()
            .zip(values.iter())
            .filter(|(_, value)| value.is_finite())
            .map(|(bar, value)| Point { time: bar.time / 1000, value: *value })
            .collect();
        out.insert(key, points);
    }

    Ok(Json(IndicatorsResponse { timeframe, series: out }))
}

pub async fn backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRequest>,
) -> Result<Json<BacktestResponse>, ApiError> {
    let market = request.market.unwrap_or_else(|| state.config.market.clone());
    let timeframe = request.tf.unwrap_or_else(|| state.config.backtest.timeframe.clone());
    let series = state.bars(&market, &timeframe)?;

    let strategy = state
        .registry
        .get(&request.strategy)
        .map_err(|error| ApiError::BadRequest(error.to_string()))?;

    // Overrides are checked against what the strategy declared rather than
    // merged blindly: a typo in a parameter name would otherwise be accepted
    // and silently ignored, and the run would look like it honoured the ask.
    let mut params = strategy.default_params();
    for (name, value) in &request.params {
        if !params.contains(name) {
            return Err(ApiError::BadRequest(format!(
                "{} has no parameter `{name}`",
                request.strategy
            )));
        }
        params.set(name, *value);
    }

    // Filters wrap the method exactly as `Filtered` does for the loop; a
    // misspelt filter is refused, not ignored.
    let filters = request
        .filters
        .iter()
        .filter(|f| !f.trim().is_empty())
        .map(|f| Filter::parse(f))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::BadRequest)?;
    let gated = Filtered { inner: strategy, filters };
    let strategy: &dyn fd_strategy::registry::Strategy = if gated.filters.is_empty() { strategy } else { &gated };

    let timeline = state.timeline(&market);
    if strategy.needs_options() && timeline.is_none() {
        return Err(ApiError::NoData(format!(
            "{} needs an options timeline and the store has no tape for {market}",
            request.strategy
        )));
    }

    let rules = state.trading_rules(&market)?;
    let guards = request.guards.then(|| Guards::from_config(&state.config));
    // A date bound is a UTC day; `to` runs to the end of its day.
    let day = |text: &Option<String>, end: bool| -> Result<Option<i64>, ApiError> {
        let Some(text) = text.as_deref().map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
        let mut parts = text.split('-').map(|p| p.parse::<i64>());
        let (y, m, d) = match (parts.next(), parts.next(), parts.next()) {
            (Some(Ok(y)), Some(Ok(m)), Some(Ok(d))) if (1..=12).contains(&m) && (1..=31).contains(&d) => (y, m, d),
            _ => return Err(ApiError::BadRequest(format!("bad date `{text}`: expected YYYY-MM-DD"))),
        };
        let ms = fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000;
        Ok(Some(if end { ms + 86_399_999 } else { ms }))
    };
    let range = Range { from: day(&request.from, false)?, to: day(&request.to, true)? };
    if let (Some(a), Some(b)) = (range.from, range.to)
        && a > b
    {
        return Err(ApiError::BadRequest("`from` is after `to`".into()));
    }
    let result = run_backtest_guarded(
        &series.bars,
        strategy,
        &params,
        &rules,
        guards.as_ref(),
        timeline.as_deref(),
        range,
        None,
    );
    let v = verdict(&result.metrics, &state.gate());

    Ok(Json(BacktestResponse {
        market,
        strategy: strategy.id().to_string(),
        name: strategy.name().to_string(),
        timeframe,
        params: params.0.clone(),
        metrics: MetricsDto::from(&result.metrics),
        verdict: VerdictDto { promising: v.promising, reasons: v.reasons },
        trades: result.trades.iter().map(TradeDto::from).collect(),
        equity_curve: result
            .equity_curve
            .iter()
            .map(|(time, equity)| EquityPoint { time: *time, equity: *equity })
            .collect(),
        indicator_specs: strategy
            .indicators(&params)
            .into_iter()
            .map(|spec| IndicatorSpecDto {
                id: spec.id,
                params: (!spec.params.is_empty()).then_some(spec.params),
            })
            .collect(),
        skipped_by_guard: result.skipped_by_guard,
    }))
}
