//! Configuration.
//!
//! Every threshold in the system is loaded from `config/default.toml`, layered
//! with an optional `config/local.toml`. No engine may carry a magic number: if
//! a value can change the result of a backtest, it belongs here where it can be
//! seen, versioned and swept.
//!
//! Market overrides are resolved once, at load time, into a [`Market`] — so the
//! engines receive conventions rather than a market identifier to branch on.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::market::{BarSource, Market, MarketId, OptionsSource, TradingSpec};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("parsing {path}: {source}")]
    Parse { path: String, source: toml::de::Error },
    #[error("unknown market `{0}` (configured: {1})")]
    UnknownMarket(String, String),
}

/// Model selections. Only two of these have been calibrated against the
/// reference feed; the rest stay configurable precisely because they are not
/// settled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelsConfig {
    /// UNVERIFIED — no candidate reproduced the reference across contracts.
    pub break_even: String,
    /// UNVERIFIED — partially reproduced.
    pub whale: String,
    /// CALIBRATED — volume-weighted positioning beat net positioning.
    pub max_pain_absolute: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub mode: String,
    pub value_area_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BigTradesConfig {
    pub min_premium_usd: f64,
    #[serde(default)]
    pub min_contracts: Option<f64>,
    pub percentile: f64,
    pub percentile_window: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowThresholds {
    pub strong_bull: f64,
    pub moderate_bull: f64,
    pub moderate_bear: f64,
    pub strong_bear: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowConfig {
    pub velocity_window_ms: i64,
    pub thresholds: FlowThresholds,
    /// Named rolling windows, e.g. `"15m" -> 900000`.
    pub windows: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub floor: f64,
    pub atr_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelsConfig {
    pub max_clusters: usize,
    pub cluster: ClusterConfig,
    pub type_weights: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromisingGate {
    pub min_trades: usize,
    pub min_profit_factor: f64,
    pub min_expectancy_r: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub timeframe: String,
    pub options_step_ms: i64,
    pub fallback_atr_period: usize,
    pub min_trades_per_cell: usize,
    pub walk_forward_folds: usize,
    pub select_by: String,
    pub promising: PromisingGate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardsConfig {
    pub max_concurrent_positions: usize,
    pub max_trades_per_day: usize,
    pub daily_loss_limit_usd: f64,
    pub cooldown_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub contract_size: f64,
    pub spread: f64,
    pub commission_per_lot: f64,
    pub starting_equity_usd: f64,
    pub risk_per_trade_pct: f64,
    pub stop_atr: f64,
    pub max_stop_atr: f64,
    pub cluster_pad_atr: f64,
    pub reward_risk: f64,
    pub min_reward_risk: f64,
    pub max_hold_ms: i64,
    pub lot_step: f64,
    pub min_lot: f64,
    pub require_basis: bool,
    pub guards: GuardsConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainConfig {
    pub epochs: usize,
    pub lr: f64,
    pub l2: f64,
    pub class_weight: bool,
}

/// The go/no-go gate. A live signal is suppressed unless a walk-forward run
/// clears every number here — and on the data collected so far it does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalGate {
    pub min_auc: f64,
    pub min_trades: usize,
    pub min_profit_factor: f64,
    pub min_expectancy_r: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalystConfig {
    pub enabled: bool,
    pub model: String,
    pub effort: String,
    pub max_tokens: u32,
    /// The analyst may block a plan…
    pub can_veto: bool,
    pub required_for_signal: bool,
    pub min_confidence: f64,
    /// …and may shrink it, but never enlarge it.
    pub scales_size: bool,
    pub full_size_confidence: f64,
    pub min_size_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    /// ATR bar width must match the trade horizon; a one-minute ATR against a
    /// sixty-minute horizon produced stops that were swept within a bar.
    pub atr_bar_ms: i64,
    pub atr_period: usize,
    pub dataset_step_ms: i64,
    pub warmup_ms: i64,
    pub horizon: String,
    pub label_band_atr: f64,
    pub folds: usize,
    pub big_trade_window_ms: i64,
    pub long_threshold: f64,
    pub short_threshold: f64,
    pub cycle_ms: i64,
    pub live_contracts: usize,
    pub horizons_ms: BTreeMap<String, i64>,
    pub train: TrainConfig,
    pub gate: SignalGate,
    pub analyst: AnalystConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
}

impl TelegramConfig {
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.bot_token.is_empty() && !self.chat_id.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default)]
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub base_url: String,
    #[serde(default)]
    pub ws_url: Option<String>,
    pub min_delay_ms: u64,
}

/// Per-market override block from `[markets.<id>]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketConfig {
    pub label: String,
    pub bar_symbol: String,
    pub bar_source: BarSource,
    pub options_source: OptionsSource,
    pub premium_in_underlying: bool,
    pub multiplier: f64,
    pub underlying: String,
    pub trading: MarketTradingOverride,
    pub big_trades: MarketBigTradesOverride,
    pub levels: MarketLevelsOverride,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketTradingOverride {
    pub symbol: String,
    pub contract_size: f64,
    pub spread: f64,
    pub lot_step: f64,
    pub min_lot: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketBigTradesOverride {
    pub min_premium_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketLevelsOverride {
    pub cluster: ClusterConfig,
}

/// The whole configuration tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub multiplier: f64,
    pub underlying: String,
    /// Default market when none is passed on the command line.
    pub market: String,
    pub models: ModelsConfig,
    pub profile: ProfileConfig,
    pub big_trades: BigTradesConfig,
    pub flow: FlowConfig,
    pub levels: LevelsConfig,
    pub backtest: BacktestConfig,
    pub trading: TradingConfig,
    pub ai: AiConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
    pub server: ServerConfig,
    pub markets: BTreeMap<String, MarketConfig>,
    pub sources: BTreeMap<String, SourceConfig>,
}

impl Config {
    /// Parse configuration from a TOML string.
    pub fn from_toml(text: &str, label: &str) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|source| ConfigError::Parse { path: label.to_string(), source })
    }

    /// Load `config/default.toml`, then layer `config/local.toml` if present.
    ///
    /// Layering is done on the parsed TOML tables rather than on the typed
    /// struct, so a local file may override a single key without restating its
    /// whole section.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let dir = dir.as_ref();
        let default_path = dir.join("default.toml");
        let text = std::fs::read_to_string(&default_path)
            .map_err(|source| ConfigError::Io { path: default_path.display().to_string(), source })?;
        let mut root: toml::Value = toml::from_str(&text)
            .map_err(|source| ConfigError::Parse { path: default_path.display().to_string(), source })?;

        let local_path = dir.join("local.toml");
        if local_path.exists() {
            let local_text = std::fs::read_to_string(&local_path)
                .map_err(|source| ConfigError::Io { path: local_path.display().to_string(), source })?;
            let local: toml::Value = toml::from_str(&local_text)
                .map_err(|source| ConfigError::Parse { path: local_path.display().to_string(), source })?;
            merge(&mut root, local);
        }

        root.try_into().map_err(|source| ConfigError::Parse { path: default_path.display().to_string(), source })
    }

    /// Resolve one market's conventions into a [`Market`].
    pub fn market(&self, id: &str) -> Result<Market, ConfigError> {
        let m = self.markets.get(id).ok_or_else(|| {
            ConfigError::UnknownMarket(id.to_string(), self.markets.keys().cloned().collect::<Vec<_>>().join(", "))
        })?;
        Ok(Market {
            id: MarketId(id.to_string()),
            label: m.label.clone(),
            bar_symbol: m.bar_symbol.clone(),
            bar_source: m.bar_source,
            options_source: m.options_source,
            premium_in_underlying: m.premium_in_underlying,
            multiplier: m.multiplier,
            underlying: m.underlying.clone(),
            trading: TradingSpec {
                symbol: m.trading.symbol.clone(),
                contract_size: m.trading.contract_size,
                spread: m.trading.spread,
                lot_step: m.trading.lot_step,
                min_lot: m.trading.min_lot,
            },
            big_trade_min_premium_usd: m.big_trades.min_premium_usd,
            cluster_floor: m.levels.cluster.floor,
            cluster_atr_fraction: m.levels.cluster.atr_fraction,
        })
    }

    /// Trading settings with the market's overrides applied.
    #[must_use]
    pub fn trading_for(&self, market: &Market) -> TradingConfig {
        let mut trading = self.trading.clone();
        trading.symbol = market.trading.symbol.clone();
        trading.contract_size = market.trading.contract_size;
        trading.spread = market.trading.spread;
        trading.lot_step = market.trading.lot_step;
        trading.min_lot = market.trading.min_lot;
        trading
    }

    /// Named rolling window in milliseconds, e.g. `"15m"`.
    #[must_use]
    pub fn window_ms(&self, name: &str) -> Option<i64> {
        self.flow.windows.get(name).copied()
    }
}

/// Recursive table merge: tables merge key by key, everything else is replaced.
fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (key, value) in overlay_table {
                match base_table.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base_table.insert(key, value);
                    }
                }
            }
        }
        (base_slot, overlay_value) => *base_slot = overlay_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
multiplier = 100
underlying = "GC"
market = "gold"

[models]
break_even = "premium-weighted"
whale = "net-long-whale"
max_pain_absolute = true

[profile]
mode = "PREMIUM"
value_area_pct = 0.7

[big_trades]
min_premium_usd = 100000
percentile = 0.95
percentile_window = 2000
limit = 200

[flow]
velocity_window_ms = 900000
[flow.thresholds]
strong_bull = 0.6
moderate_bull = 0.55
moderate_bear = 0.45
strong_bear = 0.4
[flow.windows]
"15m" = 900000

[levels]
max_clusters = 12
[levels.cluster]
floor = 5.0
atr_fraction = 0.15
[levels.type_weights]
MAX_PAIN = 1.0

[backtest]
timeframe = "15m"
options_step_ms = 300000
fallback_atr_period = 14
min_trades_per_cell = 5
walk_forward_folds = 4
select_by = "expectancy"
[backtest.promising]
min_trades = 30
min_profit_factor = 1.2
min_expectancy_r = 0.05

[trading]
symbol = "XAUUSD"
contract_size = 100.0
spread = 0.3
commission_per_lot = 0.0
starting_equity_usd = 10000.0
risk_per_trade_pct = 0.01
stop_atr = 1.2
max_stop_atr = 3.0
cluster_pad_atr = 0.25
reward_risk = 1.8
min_reward_risk = 1.2
max_hold_ms = 14400000
lot_step = 0.01
min_lot = 0.01
require_basis = true
[trading.guards]
max_concurrent_positions = 1
max_trades_per_day = 4
daily_loss_limit_usd = 300.0
cooldown_ms = 1800000

[ai]
atr_bar_ms = 900000
atr_period = 14
dataset_step_ms = 300000
warmup_ms = 7200000
horizon = "60m"
label_band_atr = 0.5
folds = 4
big_trade_window_ms = 1800000
long_threshold = 0.58
short_threshold = 0.42
cycle_ms = 300000
live_contracts = 4
[ai.horizons_ms]
"60m" = 3600000
[ai.train]
epochs = 800
lr = 0.1
l2 = 2.0
class_weight = true
[ai.gate]
min_auc = 0.55
min_trades = 30
min_profit_factor = 1.2
min_expectancy_r = 0.05
[ai.analyst]
enabled = true
model = "claude-opus-5"
effort = "high"
max_tokens = 4000
can_veto = true
required_for_signal = false
min_confidence = 0.4
scales_size = true
full_size_confidence = 0.8
min_size_scale = 0.4

[server]
host = "127.0.0.1"
port = 8137

[markets.gold]
label = "COMEX gold"
bar_symbol = "GC"
bar_source = "reference"
options_source = "reference"
premium_in_underlying = false
multiplier = 100
underlying = "GC"
[markets.gold.trading]
symbol = "XAUUSD"
contract_size = 100.0
spread = 0.3
lot_step = 0.01
min_lot = 0.01
[markets.gold.big_trades]
min_premium_usd = 100000
[markets.gold.levels.cluster]
floor = 5.0
atr_fraction = 0.15

[markets.btc]
label = "BTC"
bar_symbol = "BTCUSDT"
bar_source = "binance"
options_source = "deribit"
premium_in_underlying = true
multiplier = 1
underlying = "BTC"
[markets.btc.trading]
symbol = "BTCUSD"
contract_size = 1.0
spread = 5.0
lot_step = 0.001
min_lot = 0.001
[markets.btc.big_trades]
min_premium_usd = 25000
[markets.btc.levels.cluster]
floor = 100.0
atr_fraction = 0.15

[sources.deribit]
base_url = "https://www.deribit.com/api/v2"
ws_url = "wss://www.deribit.com/ws/api/v2"
min_delay_ms = 400
"#;

    fn config() -> Config {
        Config::from_toml(SAMPLE, "sample").expect("sample config must parse")
    }

    #[test]
    fn the_sample_config_parses_into_typed_settings() {
        let cfg = config();
        assert_eq!(cfg.market, "gold");
        assert!(cfg.models.max_pain_absolute);
        assert_eq!(cfg.window_ms("15m"), Some(900_000));
        assert_eq!(cfg.window_ms("nope"), None);
        assert!(!cfg.alerts.telegram.is_configured());
    }

    #[test]
    fn each_market_resolves_to_its_own_conventions() {
        let cfg = config();
        let gold = cfg.market("gold").unwrap();
        let btc = cfg.market("btc").unwrap();

        assert!(!gold.premium_in_underlying);
        assert!(btc.premium_in_underlying);
        assert_eq!(gold.trading.contract_size, 100.0);
        assert_eq!(btc.trading.contract_size, 1.0);
        // BTC premiums are far smaller, so its big-print floor must be too.
        assert!(btc.big_trade_min_premium_usd < gold.big_trade_min_premium_usd);
        // Clusters are measured in price units, so BTC's floor must be wider.
        assert!(btc.cluster_floor > gold.cluster_floor);
    }

    #[test]
    fn trading_settings_pick_up_the_market_override() {
        let cfg = config();
        let btc = cfg.market("btc").unwrap();
        let trading = cfg.trading_for(&btc);
        assert_eq!(trading.symbol, "BTCUSD");
        assert_eq!(trading.spread, 5.0);
        // Untouched values survive the override.
        assert_eq!(trading.risk_per_trade_pct, cfg.trading.risk_per_trade_pct);
        assert_eq!(trading.guards.max_trades_per_day, 4);
    }

    #[test]
    fn an_unknown_market_names_the_configured_ones() {
        let err = config().market("doge").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("doge"), "{message}");
        assert!(message.contains("btc") && message.contains("gold"), "{message}");
    }

    #[test]
    fn local_overrides_merge_key_by_key() {
        let mut base: toml::Value = toml::from_str("[trading]\nspread = 0.3\nstop_atr = 1.2\n").unwrap();
        let overlay: toml::Value = toml::from_str("[trading]\nspread = 0.9\n").unwrap();
        merge(&mut base, overlay);
        let trading = base.get("trading").unwrap();
        assert_eq!(trading.get("spread").unwrap().as_float(), Some(0.9));
        // A key the overlay did not mention must survive.
        assert_eq!(trading.get("stop_atr").unwrap().as_float(), Some(1.2));
    }
}
