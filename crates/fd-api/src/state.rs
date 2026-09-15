//! What every request needs, loaded once.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use fd_backtest::engine::TradingRules;
use fd_backtest::{OptionsTimeline, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::{TapeStore, read_bars, resample, timeframe_ms};
use fd_strategy::registry::Registry;

use crate::error::ApiError;

/// Timeframes the API will serve. Anything else is refused rather than
/// silently resampled to something the client did not ask for.
pub const TIMEFRAMES: [&str; 7] = ["1m", "5m", "15m", "30m", "1h", "4h", "1d"];

pub struct AppState {
    pub config: Config,
    pub data: PathBuf,
    /// Where the research loop writes: hypotheses, run receipts, decisions.
    pub docs: PathBuf,
    pub registry: Registry,
    /// Resampled series, keyed by `market/timeframe`.
    ///
    /// Reading and resampling two years of minutes on every request would make
    /// the chart feel broken. The cache is invalidated by restarting the
    /// process, which is honest for a research tool: the store is only written
    /// by a separate collector.
    bars: RwLock<HashMap<String, Arc<BarSeries>>>,
    timelines: RwLock<HashMap<String, Option<Arc<OptionsTimeline>>>>,
    /// One upstream connection per market, shared by every viewer.
    pub live: crate::live::LiveHub,
    /// The paper runs, keyed by run id; any number may share a
    /// `market:tf`, and a bar POST feeds them all under one lock, each
    /// step and its write to disk in turn. Reloaded from
    /// `<data>/paper/*/state.json` when the state is built.
    pub paper: Mutex<BTreeMap<String, crate::paper::PaperRun>>,
    /// The bar still forming on each `market:tf`, as the pollers last
    /// reported it — what the Desk draws as the current price between two
    /// closed bars.
    ///
    /// Deliberately beside `paper` and not inside it: nothing in a run may
    /// read a forming bar, and a separate mutex makes that structural rather
    /// than a rule someone has to remember. Not persisted, and not reloaded:
    /// a forming bar is worthless the moment the process stops.
    pub live_bars: Mutex<BTreeMap<String, crate::paper::LiveBar>>,
    /// Every forming bar, the instant it arrives, for anyone watching.
    ///
    /// A broadcast channel rather than a queue: a screen that was not
    /// connected when a tick arrived does not want it later, and the next one
    /// is a second away. Lagging receivers are dropped by design — a slow
    /// client gets the newest candle, never a backlog of stale ones, which is
    /// the only behaviour a price display can honestly have.
    pub ticks: tokio::sync::broadcast::Sender<crate::paper::TickEvent>,
}

pub struct BarSeries {
    pub symbol: String,
    pub bars: Vec<Bar>,
    /// True when the source publishes closes only.
    pub synthetic: bool,
}

impl AppState {
    pub fn new(config: Config, data: PathBuf) -> Self {
        let paper = crate::paper::reload(&data);
        Self {
            config,
            docs: PathBuf::from("docs"),
            data,
            registry: Registry::with_builtins(),
            bars: RwLock::new(HashMap::new()),
            timelines: RwLock::new(HashMap::new()),
            live: crate::live::LiveHub::default(),
            paper: Mutex::new(paper),
            live_bars: Mutex::new(BTreeMap::new()),
            // Sixty-four is a couple of minutes of ticks across every stream:
            // enough that a browser blocked on a repaint is not disconnected,
            // small enough that nothing accumulates when nobody is watching.
            ticks: tokio::sync::broadcast::channel(64).0,
        }
    }

    /// The forming bar for one stream, or `None` when there is none or the
    /// last one arrived more than [`crate::paper::MAX_LIVE_AGE_MS`] ago.
    ///
    /// The staleness rule lives here, at the one place the value leaves the
    /// map, so no caller can draw a dead price as current by forgetting it.
    pub fn live_bar(&self, market: &str, timeframe: &str) -> Option<crate::paper::LiveBar> {
        let key = crate::paper::stream_key(market, timeframe);
        let live = self.live_bars.lock().expect("live bars");
        let bar = live.get(&key)?;
        (crate::paper::now_ms() - bar.at <= crate::paper::MAX_LIVE_AGE_MS).then(|| bar.clone())
    }

    /// Point the research endpoint at a docs directory other than `./docs`.
    #[must_use]
    pub fn with_docs(mut self, docs: PathBuf) -> Self {
        self.docs = docs;
        self
    }

    /// Trading rules for one market.
    ///
    /// Per market, not global: the top-level `[trading]` table describes gold,
    /// and pricing a BTC contract with it charges a $0.30 spread where the real
    /// one is $5.00.
    pub fn trading_rules(&self, market: &str) -> Result<TradingRules, ApiError> {
        fd_backtest::engine::trading_rules_for(&self.config, market)
            .map_err(|error| ApiError::BadRequest(error.to_string()))
    }

    pub fn gate(&self) -> PromisingGate {
        PromisingGate {
            min_trades: self.config.backtest.promising.min_trades,
            min_profit_factor: self.config.backtest.promising.min_profit_factor,
            min_expectancy_r: self.config.backtest.promising.min_expectancy_r,
        }
    }

    /// Bars for a market at a timeframe.
    ///
    /// The file stored *at* that timeframe wins; only when there is none is a
    /// finer series resampled. Preferring the finest series instead sounds
    /// tidier and is wrong here: the stored minute series covers a week and the
    /// stored 15-minute series covers two years, so "finest wins" silently
    /// answered a two-year request with seven days of bars.
    ///
    /// One rule, applied the same way everywhere, is what keeps a chart and a
    /// backtest looking at the same series — not which rule it is.
    pub fn bars(&self, market: &str, timeframe: &str) -> Result<Arc<BarSeries>, ApiError> {
        let key = format!("{market}/{timeframe}");
        if let Some(hit) = self.bars.read().expect("bar cache").get(&key) {
            return Ok(Arc::clone(hit));
        }
        let step = timeframe_ms(timeframe).ok_or_else(|| ApiError::BadRequest(format!("unknown timeframe: {timeframe}")))?;
        let spec = self.config.market(market).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let (source, source_step) = self.source_for(&spec.bar_symbol, step)?;
        let bars = if source_step == step { source } else { resample(&source, step) };
        // Closes only: every bar flat, at every timeframe.
        let synthetic = !bars.is_empty() && bars.iter().all(Bar::is_synthetic);

        let series = Arc::new(BarSeries { symbol: spec.bar_symbol.clone(), bars, synthetic });
        self.bars.write().expect("bar cache").insert(key, Arc::clone(&series));
        Ok(series)
    }

    /// The stored series to build `step` from: an exact match, else the
    /// coarsest series that is still fine enough to resample.
    ///
    /// Coarsest-that-fits rather than finest: resampling a week of minutes into
    /// hours is not a better hour than the stored hourly series covering a
    /// year, it is a shorter one.
    fn source_for(&self, symbol: &str, step: i64) -> Result<(Vec<Bar>, i64), ApiError> {
        let mut best: Option<(Vec<Bar>, i64)> = None;
        for timeframe in TIMEFRAMES {
            let Some(candidate) = timeframe_ms(timeframe) else { continue };
            if candidate > step {
                continue;
            }
            let path = self.data.join("bars").join(format!("{symbol}-{timeframe}.parquet"));
            if !path.exists() {
                continue;
            }
            let bars = read_bars(&path)?;
            if bars.is_empty() {
                continue;
            }
            if candidate == step {
                return Ok((bars, candidate));
            }
            match &best {
                Some((_, previous)) if *previous >= candidate => {}
                _ => best = Some((bars, candidate)),
            }
        }
        best.ok_or_else(|| ApiError::NoData(format!("no stored bars for {symbol} at or below {step}ms")))
    }

    /// The options timeline for a market, or `None` when the store has no tape.
    pub fn timeline(&self, market: &str) -> Option<Arc<OptionsTimeline>> {
        if let Some(hit) = self.timelines.read().expect("timeline cache").get(market) {
            return hit.clone();
        }
        let built = self.build_timeline(market).map(Arc::new);
        self.timelines.write().expect("timeline cache").insert(market.to_string(), built.clone());
        built
    }

    fn build_timeline(&self, market: &str) -> Option<OptionsTimeline> {
        // A market with no options feed has no timeline; one sharing a feed
        // reads the store that feed is collected into.
        let tape = self.config.market(market).ok()?.tape_id()?.to_string();
        let store = TapeStore::open(&self.data, &tape).ok()?;
        let trades = store.all().ok()?;
        if trades.is_empty() {
            return None;
        }
        let settings = fd_engine::engine::EngineSettings::from_config(&self.config, market).ok()?;
        let timeline = fd_backtest::build_timeline(
            &trades,
            &settings,
            &HashMap::new(),
            &fd_backtest::TimelineOptions {
                step_ms: self.config.backtest.options_step_ms,
                big_trade_window_ms: self.config.ai.big_trade_window_ms,
            },
        );
        (!timeline.is_empty()).then_some(timeline)
    }

    /// True when the market has an upstream websocket rather than a snapshot.
    pub fn is_live(&self, market: &str) -> bool {
        self.config.market(market).is_ok_and(|spec| spec.bar_source == fd_core::market::BarSource::Binance)
    }
}
