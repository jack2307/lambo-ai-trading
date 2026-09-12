//! The options engine: a tape in, derived levels out.
//!
//! Prints are kept per expiry contract and fed forward only. The engine never
//! reorders across calls, which is what makes a replay honest: a snapshot taken
//! at time `T` has seen exactly the prints that existed at `T`, and no others.

use std::collections::BTreeMap;
use std::collections::HashMap;

use fd_core::MS_PER_DAY;
use fd_core::types::{ExpirationType, OptionTrade};
use serde::{Deserialize, Serialize};

use crate::bigtrades::{BigTradeConfig, select_big_trades};
use crate::breakeven;
use crate::flow::{self, Bias, BiasThresholds, FlowSummary, FlowVelocity};
use crate::levels::{ContextLevels, DerivedLevel, LevelCluster, NearestCluster, cluster_distance, cluster_levels, levels_from_context, nearest_cluster};
use crate::maxpain::{PositioningMode, flow_max_pain};
use crate::profile::{ProfileMode, ProfileOptions, StrikeProfile, build_profile};
use fd_core::config::{Config, ConfigError};

use crate::whale::{self, StickyWhaleTracker};

/// Everything the engine needs to know that is not a print.
#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub multiplier: f64,
    pub profile_mode: ProfileMode,
    pub value_area_pct: f64,
    pub break_even: breakeven::Model,
    pub whale: whale::Model,
    pub positioning: PositioningMode,
    pub big_trades: BigTradeConfig,
    pub bias: BiasThresholds,
    /// Named rolling windows reported in every snapshot.
    pub flow_windows: BTreeMap<String, i64>,
    pub velocity_window_ms: i64,
    pub cluster_floor: f64,
    pub cluster_atr_fraction: f64,
    pub max_clusters: usize,
    pub type_weights: BTreeMap<String, f64>,
}

impl EngineSettings {
    /// Build settings from a loaded configuration, overlaid with one market.
    ///
    /// Four things here are per market, and the top-level table holds gold's
    /// values for all of them:
    ///
    /// * the **multiplier**, which turns a quoted price into USD — gold's
    ///   contract is 100 ounces, a Deribit contract is 1 BTC;
    /// * the **cluster floor**, `$5` for gold and `$100` for BTC, because five
    ///   dollars is a meaningful distance at 4,300 and noise at 77,000;
    /// * the **cluster ATR fraction**, for the same reason;
    /// * the **big-print floor**, since BTC premiums run about three orders of
    ///   magnitude below gold's.
    ///
    /// Reading the top-level values for BTC does not fail loudly. It builds
    /// narrow clusters out of unrelated levels, and every cluster feature the
    /// model sees is then wrong in a way that looks like data.
    pub fn from_config(config: &Config, market: &str) -> Result<Self, ConfigError> {
        let spec = config.market(market)?;
        Ok(Self {
            multiplier: spec.multiplier,
            profile_mode: ProfileMode::from_id(&config.profile.mode).unwrap_or(ProfileMode::Premium),
            value_area_pct: config.profile.value_area_pct,
            break_even: breakeven::Model::from_id(&config.models.break_even)
                .ok_or_else(|| ConfigError::UnknownMarket(config.models.break_even.clone(), "break-even model".into()))?,
            whale: whale::Model::from_id(&config.models.whale)
                .ok_or_else(|| ConfigError::UnknownMarket(config.models.whale.clone(), "whale model".into()))?,
            positioning: if config.models.max_pain_absolute {
                PositioningMode::Volume
            } else {
                PositioningMode::Net
            },
            big_trades: BigTradeConfig {
                min_premium_usd: spec.big_trade_min_premium_usd,
                min_contracts: config.big_trades.min_contracts,
                // Zero means "no percentile floor", not "the 0th percentile".
                percentile: (config.big_trades.percentile > 0.0).then_some(config.big_trades.percentile),
                percentile_window: config.big_trades.percentile_window,
                limit: config.big_trades.limit,
            },
            bias: BiasThresholds {
                strong_bull: config.flow.thresholds.strong_bull,
                moderate_bull: config.flow.thresholds.moderate_bull,
                moderate_bear: config.flow.thresholds.moderate_bear,
                strong_bear: config.flow.thresholds.strong_bear,
            },
            flow_windows: config.flow.windows.clone(),
            velocity_window_ms: config.flow.velocity_window_ms,
            cluster_floor: spec.cluster_floor,
            cluster_atr_fraction: spec.cluster_atr_fraction,
            max_clusters: config.levels.max_clusters,
            type_weights: config.levels.type_weights.clone(),
        })
    }
}

/// Contract metadata the feed supplies alongside the tape.
#[derive(Debug, Clone, Default)]
pub struct ContractMeta {
    pub expiration: Option<String>,
    pub expiration_type: Option<ExpirationType>,
}

/// One expiry's derived state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpirationContext {
    pub symbol: String,
    pub expiration: String,
    pub expiration_type: ExpirationType,
    pub dte: f64,
    pub max_pain: Option<f64>,
    pub poc: Option<f64>,
    pub above_poc: Option<f64>,
    pub under_poc: Option<f64>,
    pub vah: Option<f64>,
    pub val: Option<f64>,
    /// ⚠ unverified formula.
    pub call_be: Option<f64>,
    /// ⚠ unverified formula.
    pub put_be: Option<f64>,
    /// ⚠ unverified formula.
    pub w_sup: Option<f64>,
    /// ⚠ unverified formula.
    pub w_res: Option<f64>,
    /// Requires a Greeks pipeline; not computed yet.
    pub gamma_wall: Option<f64>,
    pub flow: FlowSummary,
    pub profile: StrikeProfile,
    pub trades: usize,
}

/// A full read of the board at one instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Epoch milliseconds of the newest print consumed.
    pub as_of: i64,
    pub spot: f64,
    pub contexts: Vec<ExpirationContext>,
    pub clusters: Vec<LevelCluster>,
    pub cluster_distance: f64,
    pub nearest: Option<NearestCluster>,
    pub flow_overall: FlowSummary,
    pub bias: Bias,
    pub flow_windows: BTreeMap<String, FlowSummary>,
    pub velocity: FlowVelocity,
    pub big_trades: Vec<OptionTrade>,
}

/// Streaming options engine.
pub struct OptionsEngine {
    settings: EngineSettings,
    /// Contract symbols in first-seen order.
    order: Vec<String>,
    by_symbol: HashMap<String, Vec<OptionTrade>>,
    all: Vec<OptionTrade>,
    whale_trackers: HashMap<String, StickyWhaleTracker>,
    /// How many prints of each contract the tracker has already seen.
    whale_cursor: HashMap<String, usize>,
    spot: f64,
    last_ts: i64,
}

impl OptionsEngine {
    #[must_use]
    pub fn new(settings: EngineSettings) -> Self {
        Self {
            settings,
            order: Vec::new(),
            by_symbol: HashMap::new(),
            all: Vec::new(),
            whale_trackers: HashMap::new(),
            whale_cursor: HashMap::new(),
            spot: f64::NAN,
            last_ts: 0,
        }
    }

    /// Feed prints. They must be ascending in time within a call; the engine
    /// never sorts across calls, which is what keeps replay free of lookahead.
    pub fn ingest(&mut self, trades: &[OptionTrade]) {
        for trade in trades {
            self.all.push(trade.clone());
            if !self.by_symbol.contains_key(&trade.symbol) {
                self.order.push(trade.symbol.clone());
            }
            self.by_symbol.entry(trade.symbol.clone()).or_default().push(trade.clone());

            if trade.timestamp > self.last_ts {
                self.last_ts = trade.timestamp;
                if trade.underlying_price.is_finite() {
                    self.spot = trade.underlying_price;
                }
            }
        }
    }

    pub fn set_spot(&mut self, price: f64) {
        if price.is_finite() {
            self.spot = price;
        }
    }

    #[must_use]
    pub fn spot(&self) -> f64 {
        self.spot
    }

    #[must_use]
    pub fn as_of(&self) -> i64 {
        self.last_ts
    }

    #[must_use]
    pub fn trade_count(&self) -> usize {
        self.all.len()
    }

    /// Derived state for one contract.
    pub fn context_for(&mut self, symbol: &str, meta: &ContractMeta) -> Option<ExpirationContext> {
        let trades = self.by_symbol.get(symbol)?;
        if trades.is_empty() {
            return None;
        }
        let last = trades.last().expect("non-empty");
        let expiration_ms = last.expiration;
        let dte_raw = (expiration_ms - self.last_ts) as f64 / MS_PER_DAY;
        // The oracle rounds DTE to four decimals before it reaches anything that
        // consumes it, including the expiry weighting. Round here too or the
        // weights can land on opposite sides of a threshold.
        // `toFixed(4)`, not `Math.round(x * 1e4) / 1e4`: the oracle rounds the
        // value itself rather than a scaled product, and the two disagree at
        // the fourth decimal often enough to move a contract between the daily
        // and weekly expiry weights.
        let dte = fd_core::js_to_fixed(dte_raw, 4);

        let profile = build_profile(
            trades,
            ProfileOptions {
                mode: self.settings.profile_mode,
                value_area_pct: self.settings.value_area_pct,
                strike_step: None,
            },
        );

        let max_pain = flow_max_pain(trades, self.settings.multiplier, self.settings.positioning);
        let call_be = self.settings.break_even.call_break_even(trades, self.settings.multiplier);
        let put_be = self.settings.break_even.put_break_even(trades, self.settings.multiplier);

        let spot = if self.spot.is_finite() { self.spot } else { last.underlying_price };

        // Advance the sticky whale tracker in place. Written inline rather than
        // as a `&mut self` method so the borrow checker can see that
        // `by_symbol`, `whale_trackers` and `whale_cursor` are disjoint fields —
        // the alternative is cloning the whole contract tape on every snapshot,
        // which for the front-month gold board is thousands of prints per call.
        let (whale_model, whale_config) = (self.settings.whale, self.settings.big_trades);
        let cursor = self.whale_cursor.get(symbol).copied().unwrap_or(0).min(trades.len());
        let tracker = self
            .whale_trackers
            .entry(symbol.to_string())
            .or_insert_with(|| StickyWhaleTracker::new(whale_model, whale_config));
        let whale_levels = tracker.update(&trades[cursor..], spot);
        let seen = trades.len();
        self.whale_cursor.insert(symbol.to_string(), seen);

        Some(ExpirationContext {
            symbol: symbol.to_string(),
            expiration: meta.expiration.clone().unwrap_or_else(|| iso_from_ms(expiration_ms)),
            expiration_type: meta.expiration_type.unwrap_or_else(|| ExpirationType::from_dte(dte)),
            dte,
            max_pain,
            poc: profile.poc,
            above_poc: profile.above_poc,
            under_poc: profile.under_poc,
            vah: profile.vah,
            val: profile.val,
            call_be,
            put_be,
            w_sup: whale_levels.support,
            w_res: whale_levels.resistance,
            gamma_wall: None,
            flow: flow::summarize(trades),
            profile,
            trades: trades.len(),
        })
    }

    /// Every contract, nearest expiry first.
    pub fn contexts(&mut self, meta: &HashMap<String, ContractMeta>) -> Vec<ExpirationContext> {
        let symbols = self.order.clone();
        let default = ContractMeta::default();
        let mut out: Vec<ExpirationContext> = symbols
            .iter()
            .filter_map(|symbol| self.context_for(symbol, meta.get(symbol).unwrap_or(&default)))
            .collect();
        // Stable sort: contracts sharing a DTE keep first-seen order.
        out.sort_by(|a, b| a.dte.partial_cmp(&b.dte).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    /// Full board state.
    pub fn snapshot(&mut self, meta: &HashMap<String, ContractMeta>, atr: Option<f64>) -> Snapshot {
        let contexts = self.contexts(meta);

        let derived: Vec<DerivedLevel> = contexts
            .iter()
            .flat_map(|ctx| {
                levels_from_context(
                    &ContextLevels {
                        symbol: ctx.symbol.clone(),
                        expiration: ctx.expiration.clone(),
                        dte: ctx.dte,
                        max_pain: ctx.max_pain,
                        poc: ctx.poc,
                        above_poc: ctx.above_poc,
                        under_poc: ctx.under_poc,
                        call_be: ctx.call_be,
                        put_be: ctx.put_be,
                        w_sup: ctx.w_sup,
                        w_res: ctx.w_res,
                        gamma_wall: ctx.gamma_wall,
                    },
                    &self.settings.type_weights,
                )
            })
            .collect();

        let distance = cluster_distance(self.settings.cluster_floor, self.settings.cluster_atr_fraction, atr);
        let all_clusters = cluster_levels(&derived, distance);
        let nearest = if self.spot.is_finite() { nearest_cluster(&all_clusters, self.spot, distance) } else { None };
        let clusters: Vec<LevelCluster> = all_clusters.into_iter().take(self.settings.max_clusters).collect();

        let mut windows = BTreeMap::new();
        for (name, ms) in &self.settings.flow_windows {
            windows.insert(name.clone(), flow::summarize_window(&self.all, self.last_ts, *ms));
        }

        let overall = flow::summarize(&self.all);
        Snapshot {
            as_of: self.last_ts,
            spot: self.spot,
            contexts,
            clusters,
            cluster_distance: distance,
            nearest,
            bias: flow::bias_label(overall.bull_ratio, self.settings.bias),
            flow_overall: overall,
            flow_windows: windows,
            velocity: flow::net_flow_velocity(&self.all, self.last_ts, self.settings.velocity_window_ms),
            big_trades: select_big_trades(&self.all, &self.settings.big_trades),
        }
    }
}

/// Epoch milliseconds to an ISO-8601 string, without pulling in a date crate.
fn iso_from_ms(ms: i64) -> String {
    // Days since the Unix epoch, then the civil-date algorithm.
    let (days, rem_ms) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    let hours = rem_ms / 3_600_000;
    let minutes = (rem_ms % 3_600_000) / 60_000;
    let seconds = (rem_ms % 60_000) / 1000;
    let millis = rem_ms % 1000;
    format!("{year:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::classify::classify_flow;
    use fd_core::types::{AggressorSide, OptionType, TradeFlags};

    fn settings() -> EngineSettings {
        let mut windows = BTreeMap::new();
        windows.insert("15m".to_string(), 900_000);
        EngineSettings {
            multiplier: 100.0,
            profile_mode: ProfileMode::Premium,
            value_area_pct: 0.7,
            break_even: breakeven::Model::PremiumWeighted,
            whale: whale::Model::NetLongWhale,
            positioning: PositioningMode::Volume,
            big_trades: BigTradeConfig { min_premium_usd: 100_000.0, percentile: None, ..Default::default() },
            bias: BiasThresholds { strong_bull: 0.6, moderate_bull: 0.55, moderate_bear: 0.45, strong_bear: 0.4 },
            flow_windows: windows,
            velocity_window_ms: 900_000,
            cluster_floor: 5.0,
            cluster_atr_fraction: 0.15,
            max_clusters: 12,
            type_weights: BTreeMap::new(),
        }
    }

    fn trade(symbol: &str, ts: i64, strike: f64, option_type: OptionType, side: AggressorSide, contracts: f64, premium: f64) -> OptionTrade {
        OptionTrade {
            id: format!("{symbol}-{ts}"),
            timestamp: ts,
            symbol: symbol.into(),
            instrument: None,
            underlying: "GC".into(),
            expiration: 1_600_000_000_000,
            dte: 0.0,
            strike,
            option_type,
            trade_price: premium / contracts / 100.0,
            contracts,
            bid: None,
            ask: None,
            aggressor_side: side,
            flow_class: classify_flow(option_type, side),
            premium_usd: premium,
            underlying_price: 4350.0,
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags::default(),
            source: "test".into(),
        }
    }

    #[test]
    fn a_snapshot_reports_levels_flow_and_clusters() {
        let mut engine = OptionsEngine::new(settings());
        engine.ingest(&[
            trade("OGV6", 1_000, 4300.0, OptionType::Put, AggressorSide::Buy, 10.0, 200_000.0),
            trade("OGV6", 2_000, 4400.0, OptionType::Call, AggressorSide::Buy, 10.0, 300_000.0),
            trade("OGV6", 3_000, 4500.0, OptionType::Call, AggressorSide::Sell, 5.0, 50_000.0),
        ]);

        let snapshot = engine.snapshot(&HashMap::new(), Some(10.0));
        assert_eq!(snapshot.as_of, 3_000);
        assert_eq!(snapshot.spot, 4350.0);
        assert_eq!(snapshot.contexts.len(), 1);

        let ctx = &snapshot.contexts[0];
        assert_eq!(ctx.trades, 3);
        assert!(ctx.max_pain.is_some() && ctx.poc.is_some());
        assert_eq!(ctx.flow.trades, 3);
        assert!(!snapshot.clusters.is_empty());
        assert!(snapshot.flow_windows.contains_key("15m"));
    }

    #[test]
    fn contracts_are_ordered_by_time_to_expiry() {
        let mut engine = OptionsEngine::new(settings());
        let mut far = trade("FAR", 1_000, 4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 1000.0);
        far.expiration = 1_600_000_000_000 + 30 * 86_400_000;
        let near = trade("NEAR", 2_000, 4400.0, OptionType::Call, AggressorSide::Buy, 1.0, 1000.0);
        engine.ingest(&[far, near]);

        let contexts = engine.contexts(&HashMap::new());
        assert_eq!(contexts[0].symbol, "NEAR", "nearest expiry first");
        assert_eq!(contexts[1].symbol, "FAR");
    }

    #[test]
    fn a_whale_level_survives_a_later_snapshot_with_no_large_prints() {
        let mut engine = OptionsEngine::new(settings());
        engine.ingest(&[trade("OGV6", 1_000, 4200.0, OptionType::Put, AggressorSide::Buy, 60.0, 240_000.0)]);
        let first = engine.snapshot(&HashMap::new(), None);
        assert_eq!(first.contexts[0].w_sup, Some(4200.0));

        engine.ingest(&[trade("OGV6", 2_000, 4360.0, OptionType::Call, AggressorSide::Buy, 1.0, 50.0)]);
        let second = engine.snapshot(&HashMap::new(), None);
        assert_eq!(second.contexts[0].w_sup, Some(4200.0), "the level is sticky across snapshots");
    }

    #[test]
    fn ingesting_the_same_tape_in_two_batches_gives_the_same_snapshot() {
        let tape: Vec<OptionTrade> = (0..40)
            .map(|i| {
                trade(
                    "OGV6",
                    i * 1_000,
                    4300.0 + (i % 5) as f64 * 25.0,
                    if i % 2 == 0 { OptionType::Call } else { OptionType::Put },
                    if i % 3 == 0 { AggressorSide::Sell } else { AggressorSide::Buy },
                    1.0 + (i % 4) as f64,
                    10_000.0 * (1.0 + (i % 7) as f64),
                )
            })
            .collect();

        let mut whole = OptionsEngine::new(settings());
        whole.ingest(&tape);

        let mut split = OptionsEngine::new(settings());
        split.ingest(&tape[..17]);
        let _ = split.snapshot(&HashMap::new(), None); // a mid-way read must not change the outcome
        split.ingest(&tape[17..]);

        let a = whole.snapshot(&HashMap::new(), None);
        let b = split.snapshot(&HashMap::new(), None);
        assert_eq!(a.contexts[0].max_pain, b.contexts[0].max_pain);
        assert_eq!(a.contexts[0].poc, b.contexts[0].poc);
        assert_eq!(a.contexts[0].w_sup, b.contexts[0].w_sup);
        assert_eq!(a.flow_overall, b.flow_overall);
    }

    #[test]
    fn iso_conversion_matches_known_instants() {
        assert_eq!(iso_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_ms(1_600_000_000_000), "2020-09-13T12:26:40.000Z");
        // Cross-checked against an independent implementation rather than
        // written from memory — the first draft of this expectation was wrong.
        assert_eq!(iso_from_ms(1_790_000_000_000), "2026-09-21T14:13:20.000Z");
        // A pre-epoch instant must not wrap: the civil-date maths is signed.
        assert_eq!(iso_from_ms(-86_400_000), "1969-12-31T00:00:00.000Z");
    }
}
