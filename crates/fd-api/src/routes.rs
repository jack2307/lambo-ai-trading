//! The chart API.
//!
//! Same six endpoints and the same wire shapes the prototype served, because
//! Phase 4's gate is that the browser cannot tell which backend answered. What
//! changed is underneath: the numbers come from the Parquet store and the Rust
//! engines rather than from a JSON cache and the JavaScript ones.

use fd_store::{read_bars, resample};
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
    /// How many of the NEWEST closed bars to return. Absent means all of them.
    ///
    /// The chart needs about four hundred to fill a viewport and the store
    /// holds a hundred thousand: measured 2026-09-18, `XAUUSD-5m.parquet` is
    /// 101,242 bars and `XAUUSD-1m.parquet` 100,000. Without this, selecting
    /// 5m on the new timeframe selector serialises the entire corpus into one
    /// response, on every poll, to draw two hundred candles. Asked for by -48,
    /// who had already written the client to send it.
    ///
    /// The NEWEST, never the oldest: a chart that silently answered with 2022
    /// would look like a feed that had stopped rather than a truncation.
    pub n: Option<usize>,
}

impl MarketQuery {
    fn market(&self, state: &AppState) -> String {
        self.market.clone().unwrap_or_else(|| state.config.market.clone())
    }

    fn timeframe(&self, state: &AppState) -> String {
        self.tf.clone().unwrap_or_else(|| state.config.backtest.timeframe.clone())
    }

    /// Clamped rather than trusted: a caller asking for zero gets the default
    /// rather than an empty chart, and one asking for a million gets the file.
    fn window(&self) -> Option<usize> {
        self.n.filter(|n| *n > 0)
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
        .map(|def| {
            // THE MEASUREMENT TRAVELS WITH THE DEFINITION, so a viewer reads
            // what a line costs in the menu where the line is chosen rather
            // than in a note under a chart that already has it drawn. Empty
            // becomes absent, never `[]` — see `IndicatorInfo::measured`.
            let rows = fd_indicators::measured(def.id);
            let measured = (!rows.is_empty()).then(|| rows.iter().map(MeasuredDto::from).collect::<Vec<_>>());
            IndicatorInfo {
                id: def.id.to_string(),
                name: def.name.to_string(),
                pane: if def.pane == Pane::Overlay { "overlay" } else { "pane" },
                params: def.params.iter().map(|(name, value)| ((*name).to_string(), *value)).collect(),
                param_order: def.params.iter().map(|(name, _)| (*name).to_string()).collect(),
                outputs: def.outputs.iter().map(|o| (*o).to_string()).collect(),
                measured,
            }
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
    let step = fd_store::timeframe_ms(&timeframe).unwrap_or(900_000);
    let mut got = anchored_bars(&state, &market, &timeframe)?;
    // Trimmed AFTER the resample, so an hour built from fifteens is built from
    // every fifteen it needs and only then cut - trimming first would drop the
    // finer bars that make up the oldest returned hour and leave it short.
    if let Some(n) = query.window() {
        let skip = got.bars.len().saturating_sub(n);
        if skip > 0 {
            got.bars.drain(..skip);
        }
    }
    let got = got;

    let (from, to) = match (got.bars.first(), got.bars.last()) {
        (Some(first), Some(last)) => (first.time, last.time),
        _ => (0, 0),
    };
    let forming = forming_bar(&got.bars, step, got.finer.as_ref());

    Ok(Json(BarsResponse {
        market,
        symbol: got.symbol.clone(),
        timeframe,
        bar_ms: step,
        last_closed_bar_ms: got.bars.last().map(|b| b.time),
        source: BarSourceDto {
            file: got.file.clone(),
            timeframe: got.source_timeframe.clone(),
            resampled: got.resampled,
            exported_at_ms: got.exported_at_ms,
        },
        forming,
        synthetic: got.synthetic,
        live: state.is_live(&query.market(&state)),
        bars: got.bars.iter().map(to_bar_dto).collect(),
        stats: BarStats {
            bars: got.bars.len(),
            from,
            to,
            days: if to > from { (to - from) as f64 / DAY_MS } else { 0.0 },
            gaps: fd_ingest::gaps(&got.bars, step).len(),
        },
    }))
}

/* ------------------------------------------- the anchor rule for the chart */

/// Is `step` a bucket whose boundaries are the same wherever the trading day
/// starts?
///
/// The broker's clock is a WHOLE number of hours from UTC — UTC+3 in New York
/// summer and UTC+2 outside it — so any bucket that divides an hour lands on
/// the same instants whether it is anchored to the Unix epoch or to the
/// server's own day. Fifteen minutes is fifteen minutes past every hour on
/// both. Verified 2026-09-18 against every stored XAUUSD series: bar times
/// carry seconds 0 and minutes in {0}, {0,5,..}, {0,15,30,45} with no
/// exceptions, and the 4h file's hours are whole.
///
/// Four hours is not such a bucket, and neither is a day. Those must come from
/// a file the terminal exported, because the broker's own 4h candles start at
/// 21:00 UTC in summer and 22:00 in winter — hours an epoch-anchored resample
/// never produces. See `crates/fd-api/src/htf.rs`, which refuses for the same
/// reason.
const fn anchor_free(step: i64) -> bool {
    step > 0 && 3_600_000 % step == 0
}

/// What a chart series came from, and whether anything was inferred.
#[derive(Debug)]
pub struct Anchored {
    pub bars: Vec<Bar>,
    pub symbol: String,
    pub synthetic: bool,
    /// The parquet actually read, relative to the data root.
    pub file: String,
    /// The stored timeframe of that file, which is NOT always the one asked
    /// for: `1h` may be built from `15m`.
    pub source_timeframe: String,
    pub resampled: bool,
    /// Last write time of that file, UTC epoch ms — a STAMP, so a client ages
    /// it against its own clock rather than against however long the response
    /// spent in flight.
    pub exported_at_ms: Option<i64>,
    /// The finest stored series below the requested step, for building the
    /// bar that has not closed yet.
    pub finer: Option<(Vec<Bar>, String, i64)>,
}

fn file_stamp_ms(path: &std::path::Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

/// Bars for a chart at `timeframe`, refusing where the anchor could differ.
///
/// Reads the files directly rather than through `AppState::bars`, which
/// resamples on a miss and caches the result under `market/timeframe` — so one
/// request for an epoch-anchored 4h would poison every later one. Naming the
/// file in the response is the other half of that: a reader can check what was
/// served instead of trusting that it was right.
fn anchored_bars(state: &AppState, market: &str, timeframe: &str) -> Result<Anchored, ApiError> {
    let step = fd_store::timeframe_ms(timeframe)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown timeframe: {timeframe}")))?;
    let spec = state.config.market(market).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let symbol = spec.bar_symbol.clone();
    let dir = state.data.join("bars");

    let mut exact: Option<(Vec<Bar>, std::path::PathBuf)> = None;
    // The finest stored series strictly below the request, kept for the
    // forming bar whether or not it is used to resample.
    let mut finest: Option<(Vec<Bar>, String, i64)> = None;
    let mut coarsest_below: Option<(Vec<Bar>, String, i64, std::path::PathBuf)> = None;

    for tf in TIMEFRAMES {
        let Some(candidate) = fd_store::timeframe_ms(tf) else { continue };
        if candidate > step {
            continue;
        }
        let path = dir.join(format!("{symbol}-{tf}.parquet"));
        if !path.exists() {
            continue;
        }
        let bars = read_bars(&path)?;
        if bars.is_empty() {
            continue;
        }
        if candidate == step {
            exact = Some((bars, path));
            continue;
        }
        if finest.as_ref().is_none_or(|(_, _, s)| candidate < *s) {
            finest = Some((bars.clone(), tf.to_string(), candidate));
        }
        if coarsest_below.as_ref().is_none_or(|(_, _, s, _)| candidate > *s) {
            coarsest_below = Some((bars, tf.to_string(), candidate, path));
        }
    }

    if let Some((bars, path)) = exact {
        let synthetic = !bars.is_empty() && bars.iter().all(Bar::is_synthetic);
        return Ok(Anchored {
            symbol,
            synthetic,
            file: format!("bars/{}", path.file_name().unwrap_or_default().to_string_lossy()),
            source_timeframe: timeframe.to_string(),
            resampled: false,
            exported_at_ms: file_stamp_ms(&path),
            finer: finest,
            bars,
        });
    }

    // No file at this timeframe. Resampling is allowed only where the answer
    // cannot depend on where the trading day starts.
    if !anchor_free(step) {
        return Err(ApiError::NoData(format!(
            "no {timeframe} bars for {market}: bars/{symbol}-{timeframe}.parquet has not been exported, \
             and {timeframe} cannot be resampled from a finer series because this broker's day starts at \
             21:00 UTC (22:00 outside US summer time) - an epoch-anchored {timeframe} candle is not the \
             broker's. Export it: py/ingest/mt5_export.py --symbols {symbol} --timeframes {}",
            match timeframe {
                "4h" => "H4",
                "1d" => "D1",
                other => other,
            }
        )));
    }

    let (bars, source_tf, _src_step, path) = coarsest_below
        .ok_or_else(|| ApiError::NoData(format!("no stored bars for {symbol} at or below {timeframe}")))?;
    let resampled = resample(&bars, step);
    let synthetic = !resampled.is_empty() && resampled.iter().all(Bar::is_synthetic);
    Ok(Anchored {
        symbol,
        synthetic,
        file: format!("bars/{}", path.file_name().unwrap_or_default().to_string_lossy()),
        source_timeframe: source_tf,
        resampled: true,
        exported_at_ms: file_stamp_ms(&path),
        finer: finest,
        bars: resampled,
    })
}

/// The bar that has not closed yet, aggregated from a finer stored series.
///
/// NOT guessed and not bucketed. The start comes from the last CLOSED bar of
/// the series being drawn, plus its period — so on a 4h chart the forming bar
/// begins where the broker's own last 4h candle ended, whatever the anchor is.
/// Everything in it is then the true open, high, low and close of the finer
/// bars that have closed inside it.
///
/// `complete_to_ms` is the end of the last finer bar included, and it is the
/// field that keeps this honest: built from 15m bars, the high and low can be
/// up to fifteen minutes stale while the price moves on. A wick drawn as "the
/// high so far" when the high is fifteen minutes old is a smaller version of
/// the lie a client-side forming candle tells, and the client renders the two
/// parts differently because this field lets it.
fn forming_bar(closed: &[Bar], step: i64, finer: Option<&(Vec<Bar>, String, i64)>) -> Option<FormingDto> {
    let last = closed.last()?;
    let start = last.time + step;
    let (fine, tf, fine_step) = finer?;
    let inside: Vec<&Bar> = fine.iter().filter(|b| b.time >= start && b.time < start + step).collect();
    let first = inside.first()?;
    let high = inside.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    let low = inside.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let last_fine = inside.last()?;
    Some(FormingDto {
        time: start,
        open: first.open,
        high,
        low,
        close: last_fine.close,
        from_timeframe: tf.clone(),
        complete_to_ms: last_fine.time + fine_step,
    })
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

    // The rules first: a `news:` filter is scoped to the market's
    // `news_currencies`, which the rules carry.
    let rules = state.trading_rules(&market)?;
    // Filters wrap the method exactly as `Filtered` does for the loop; a
    // misspelt filter is refused, not ignored.
    let filters = request
        .filters
        .iter()
        .filter(|f| !f.trim().is_empty())
        .map(|f| Filter::parse_for_market(f, &rules.news_currencies))
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

    let guards = request
        .guards
        .then(|| Guards::for_market(&state.config, &market))
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
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
        closed_by_guard: result.closed_by_guard,
        sized_down_by_guard: result.sized_down_by_guard,
    }))
}

#[cfg(test)]
mod anchor_tests {
    use super::*;

    const H: i64 = 3_600_000;
    const M15: i64 = 900_000;

    fn bar(time: i64, o: f64, h: f64, l: f64, c: f64) -> Bar {
        Bar { time, open: o, high: h, low: l, close: c, volume: Some(1.0) }
    }

    fn state_with(dir: &std::path::Path, files: &[(&str, Vec<Bar>)]) -> Arc<AppState> {
        for (tf, bars) in files {
            let path = dir.join("bars").join(format!("BTCUSDT-{tf}.parquet"));
            fd_store::write_bars(&path, bars).expect("write");
        }
        let config_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
        let config = fd_core::config::Config::load(config_dir).expect("config");
        Arc::new(AppState::new(config, dir.to_path_buf()))
    }

    /// 21:00 UTC — the broker's own 4h anchor in summer, which no epoch
    /// bucketing produces.
    const ANCHOR: i64 = 1_789_678_800_000;

    #[test]
    fn four_hours_is_refused_when_only_a_finer_series_is_stored() {
        // THE rule. An epoch-anchored 4h candle opens at 20:00 UTC where the
        // broker's opens at 21:00, so it is a different candle with different
        // highs and lows — and "yesterday's high" from it is a level a trader
        // would recognise the name of and not the value.
        let dir = tempfile::tempdir().expect("temp dir");
        let fifteens: Vec<Bar> = (0..40).map(|i| bar(ANCHOR + i * M15, 1.0, 2.0, 0.5, 1.5)).collect();
        let state = state_with(dir.path(), &[("15m", fifteens)]);

        let err = anchored_bars(&state, "btc", "4h").expect_err("4h must be refused");
        let text = format!("{err:?}");
        assert!(text.contains("21:00"), "the refusal explains the anchor: {text}");
        assert!(text.contains("mt5_export.py"), "and names the remedy: {text}");

        // A day is refused for the same reason.
        assert!(anchored_bars(&state, "btc", "1d").is_err());
    }

    #[test]
    fn an_hour_is_allowed_from_fifteens_and_lands_on_the_hour() {
        // An hour divides an hour, and the broker's clock is a whole number of
        // hours from UTC, so this bucket is the same wherever the day starts.
        let dir = tempfile::tempdir().expect("temp dir");
        let fifteens: Vec<Bar> = (0..8)
            .map(|i| bar(ANCHOR + i * M15, i as f64, i as f64 + 1.0, i as f64 - 1.0, i as f64 + 0.5))
            .collect();
        let state = state_with(dir.path(), &[("15m", fifteens)]);

        let got = anchored_bars(&state, "btc", "1h").expect("1h is allowed");
        assert!(got.resampled, "built from the finer series");
        assert_eq!(got.source_timeframe, "15m");
        assert_eq!(got.bars.len(), 2);
        for b in &got.bars {
            assert_eq!(b.time % H, 0, "every hourly bar lands on an hour: {}", b.time);
        }
        // The first hour is the first four fifteens, aggregated.
        assert_eq!(got.bars[0].open, 0.0);
        assert_eq!(got.bars[0].high, 4.0);
        assert_eq!(got.bars[0].low, -1.0);
        assert_eq!(got.bars[0].close, 3.5);
    }

    #[test]
    fn an_exported_file_is_served_whole_and_never_rebucketed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let fours: Vec<Bar> = (0..5).map(|i| bar(ANCHOR + i * 4 * H, 10.0, 12.0, 9.0, 11.0)).collect();
        let state = state_with(dir.path(), &[("4h", fours)]);

        let got = anchored_bars(&state, "btc", "4h").expect("the file is there");
        assert!(!got.resampled);
        assert_eq!(got.source_timeframe, "4h");
        assert!(got.file.ends_with("-4h.parquet"), "{}", got.file);
        assert_eq!(got.bars.first().map(|b| b.time), Some(ANCHOR), "the broker's anchor, kept");
        assert!(got.exported_at_ms.is_some(), "the file's age is reported");
    }

    #[test]
    fn the_forming_bar_is_the_finer_bars_inside_it_and_says_how_far_it_knows() {
        // a5's case: three closed M15 inside an open 4h bar. The start comes
        // from the last CLOSED 4h bar plus its period, so it inherits the
        // broker's 21:00 anchor instead of assuming one.
        let dir = tempfile::tempdir().expect("temp dir");
        let fours = vec![bar(ANCHOR, 10.0, 12.0, 9.0, 11.0)];
        let open_at = ANCHOR + 4 * H;
        let fifteens = vec![
            bar(open_at, 11.0, 13.0, 10.5, 12.0),
            bar(open_at + M15, 12.0, 15.0, 11.5, 14.0),
            bar(open_at + 2 * M15, 14.0, 14.5, 13.0, 13.5),
        ];
        let state = state_with(dir.path(), &[("4h", fours), ("15m", fifteens)]);

        let got = anchored_bars(&state, "btc", "4h").expect("bars");
        let f = forming_bar(&got.bars, 4 * H, got.finer.as_ref()).expect("a forming bar");
        assert_eq!(f.time, open_at, "starts where the closed 4h bar ended, at 01:00Z not 00:00Z");
        assert_eq!(f.time % (4 * H), (ANCHOR % (4 * H)), "on the broker's anchor, not the epoch's");
        assert_eq!(f.open, 11.0, "the first finer bar's open");
        assert_eq!(f.high, 15.0);
        assert_eq!(f.low, 10.5);
        assert_eq!(f.close, 13.5, "the last finer bar's close");
        assert_eq!(f.from_timeframe, "15m");
        assert_eq!(
            f.complete_to_ms,
            open_at + 3 * M15,
            "the high and low are known only to the end of the third fifteen"
        );
    }

    #[test]
    fn there_is_no_forming_bar_when_nothing_finer_is_stored() {
        // Absent, not fabricated. The chart says "closed bars only" and that
        // is honest; a candle whose open is invented is not.
        let dir = tempfile::tempdir().expect("temp dir");
        let fours: Vec<Bar> = (0..3).map(|i| bar(ANCHOR + i * 4 * H, 10.0, 12.0, 9.0, 11.0)).collect();
        let state = state_with(dir.path(), &[("4h", fours)]);
        let got = anchored_bars(&state, "btc", "4h").expect("bars");
        assert!(got.finer.is_none());
        assert!(forming_bar(&got.bars, 4 * H, got.finer.as_ref()).is_none());
    }

    #[test]
    fn a_forming_bar_with_a_hole_in_the_fine_series_carries_what_it_has() {
        // Export lag: the finer series stops part-way into the open bar. It
        // reports what closed and says so through `complete_to_ms` rather than
        // pretending the bar is current.
        let dir = tempfile::tempdir().expect("temp dir");
        let fours = vec![bar(ANCHOR, 10.0, 12.0, 9.0, 11.0)];
        let open_at = ANCHOR + 4 * H;
        let fifteens = vec![bar(open_at, 11.0, 13.0, 10.5, 12.0)];
        let state = state_with(dir.path(), &[("4h", fours), ("15m", fifteens)]);
        let got = anchored_bars(&state, "btc", "4h").expect("bars");
        let f = forming_bar(&got.bars, 4 * H, got.finer.as_ref()).expect("a forming bar");
        assert_eq!(f.complete_to_ms, open_at + M15, "one fifteen into a four-hour bar, and it says so");
        assert!(f.complete_to_ms < f.time + 4 * H);
    }

    #[test]
    fn the_window_keeps_the_newest_bars_and_not_the_oldest() {
        // A chart asking for 400 of 101,242 must get the most recent 400. The
        // other end of that mistake is a chart quietly showing 2022, which
        // reads as a feed that has stopped rather than as a truncation.
        let dir = tempfile::tempdir().expect("temp dir");
        let fifteens: Vec<Bar> =
            (0..100).map(|i| bar(ANCHOR + i * M15, i as f64, i as f64, i as f64, i as f64)).collect();
        let state = state_with(dir.path(), &[("15m", fifteens)]);
        let all = anchored_bars(&state, "btc", "15m").expect("bars");
        assert_eq!(all.bars.len(), 100);

        let mut trimmed = anchored_bars(&state, "btc", "15m").expect("bars");
        let skip = trimmed.bars.len().saturating_sub(10);
        trimmed.bars.drain(..skip);
        assert_eq!(trimmed.bars.len(), 10);
        assert_eq!(trimmed.bars.last().map(|b| b.time), all.bars.last().map(|b| b.time), "newest kept");
        assert_eq!(trimmed.bars.first().map(|b| b.close), Some(90.0), "and the oldest ten are the ones dropped");
    }

    #[test]
    fn every_series_comes_back_ascending_by_time() {
        // -48's placement searches these bars for the last one at or before a
        // marker's time. That search is correct only on an ascending series,
        // and it would fail as a misplaced fill rather than an exception.
        let dir = tempfile::tempdir().expect("temp dir");
        let fifteens: Vec<Bar> = (0..12).map(|i| bar(ANCHOR + i * M15, 1.0, 2.0, 0.5, 1.5)).collect();
        let state = state_with(dir.path(), &[("15m", fifteens)]);
        for tf in ["15m", "1h"] {
            let got = anchored_bars(&state, "btc", tf).expect(tf);
            assert!(
                got.bars.windows(2).all(|w| w[0].time < w[1].time),
                "{tf} must be strictly ascending"
            );
        }
    }
}
