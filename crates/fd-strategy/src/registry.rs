//! The strategy contract.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use serde::{Deserialize, Serialize};

/// Numeric parameters, keyed by name.
///
/// A map rather than a per-strategy struct because the sweep, the UI and the
/// config all address parameters by name at runtime; a typed struct would force
/// every one of those to know every strategy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Params(pub BTreeMap<String, f64>);

impl Params {
    #[must_use]
    pub fn new(entries: &[(&str, f64)]) -> Self {
        Self(entries.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
    }

    /// Value for `name`, or `NaN` when the strategy never declared it.
    ///
    /// NaN rather than a default: a strategy reading a parameter it did not
    /// declare is a bug, and NaN makes the resulting comparison fail closed
    /// instead of silently trading on a zero.
    #[must_use]
    pub fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(f64::NAN)
    }

    /// Value rounded to a whole number of periods.
    #[must_use]
    pub fn period(&self, name: &str) -> usize {
        let v = self.get(name);
        if v.is_finite() && v > 0.0 { v.round() as usize } else { 0 }
    }

    pub fn set(&mut self, name: &str, value: f64) {
        self.0.insert(name.to_string(), value);
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// Overlay `overrides`, rejecting names the strategy does not declare.
    pub fn with_overrides(&self, overrides: &Params) -> Result<Self, StrategyError> {
        let mut merged = self.clone();
        for (key, value) in &overrides.0 {
            if !merged.0.contains_key(key) {
                return Err(StrategyError::UnknownParameter {
                    name: key.clone(),
                    known: merged.0.keys().cloned().collect::<Vec<_>>().join(", "),
                });
            }
            merged.0.insert(key.clone(), *value);
        }
        Ok(merged)
    }
}

/// Direction of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Long,
    Short,
}

impl Side {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Long => "LONG",
            Self::Short => "SHORT",
        }
    }

    #[must_use]
    pub const fn is_long(self) -> bool {
        matches!(self, Self::Long)
    }
}

/// What a strategy wants to do on this bar.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Intent {
    #[default]
    None,
    Enter {
        side: Side,
        /// Absolute stop price. `None` lets the engine derive one from ATR.
        stop: Option<f64>,
        target: Option<f64>,
        reason: String,
    },
    Exit {
        reason: String,
    },
}

/// Who closes a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Exits {
    /// The engine imposes a stop, a target and a maximum hold.
    #[default]
    Engine,
    /// The strategy issues its own exit; the engine imposes nothing.
    ///
    /// A hold-forever control needs this. With an ATR stop imposed on it, the
    /// buy-and-hold baseline turned into dozens of stopped-out trades and
    /// stopped being a control at all.
    Strategy,
}

/// A position the engine currently holds, as the strategy sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenPosition {
    pub side: Side,
    pub entry_price: f64,
    pub entry_time: i64,
    pub stop: Option<f64>,
    pub target: Option<f64>,
}

/// Everything a strategy may read on one bar.
pub struct BarContext<'a> {
    pub bar: &'a Bar,
    pub i: usize,
    pub bars: &'a [Bar],
    pub ind: &'a fd_indicators::IndicatorSet,
    /// The series named by [`Strategy::series`], already looked up.
    ///
    /// Resolving them by name costs a string format and a tree walk; doing it
    /// here once per run instead of once per bar is the difference between a
    /// sweep that searches a hypothesis space and one that crawls it.
    pub series: &'a [&'a [f64]],
    /// The most recent options frame at or before this bar, if any.
    pub options: Option<&'a OptionsView<'a>>,
    pub position: Option<OpenPosition>,
    pub params: &'a Params,
}

/// The options picture a strategy can consult.
///
/// A narrow view rather than the whole engine snapshot: a strategy that could
/// reach into raw prints could also reach into the future, and this keeps the
/// surface small enough to reason about.
pub struct OptionsView<'a> {
    pub spot: f64,
    pub bull_ratio: f64,
    pub bull_ratio_15m: f64,
    pub net_flow_velocity_norm: f64,
    pub big_trade_imbalance: f64,
    pub clusters: &'a [ClusterView],
    pub contexts: &'a [ContextView],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClusterView {
    pub low: f64,
    pub high: f64,
    pub center: f64,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextView {
    pub symbol: String,
    pub dte: f64,
    pub max_pain: Option<f64>,
    pub poc: Option<f64>,
    pub w_sup: Option<f64>,
    pub w_res: Option<f64>,
    pub call_be: Option<f64>,
    pub put_be: Option<f64>,
    pub bull_ratio: f64,
}

impl BarContext<'_> {
    /// Indicator value at this bar, or `NaN` when it is not ready.
    #[must_use]
    pub fn v(&self, key: &str) -> f64 {
        self.back(key, 0)
    }

    /// Value of the slot-th series this strategy declared, at this bar.
    #[must_use]
    pub fn s(&self, slot: usize) -> f64 {
        self.s_back(slot, 0)
    }

    /// Value of the slot-th declared series, `n` bars ago.
    #[must_use]
    pub fn s_back(&self, slot: usize, n: usize) -> f64 {
        let Some(series) = self.series.get(slot) else {
            return f64::NAN;
        };
        match self.i.checked_sub(n) {
            Some(index) => series.get(index).copied().unwrap_or(f64::NAN),
            None => f64::NAN,
        }
    }

    /// Indicator value `n` bars ago.
    #[must_use]
    pub fn back(&self, key: &str, n: usize) -> f64 {
        let Some(series) = self.ind.get(key) else {
            return f64::NAN;
        };
        match self.i.checked_sub(n) {
            Some(index) => series.get(index).copied().unwrap_or(f64::NAN),
            None => f64::NAN,
        }
    }

    /// The previous bar, if there is one.
    #[must_use]
    pub fn prev(&self) -> Option<&Bar> {
        self.i.checked_sub(1).and_then(|i| self.bars.get(i))
    }
}

/// A trading method.
pub trait Strategy: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_params(&self) -> Params;

    /// Parameter values the sweep explores.
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::new()
    }

    /// Indicator series this strategy reads, in the order `on_bar` indexes them.
    ///
    /// Named once per run rather than once per bar. `on_bar` is called for
    /// every bar of every parameter cell, so building an indicator key inside
    /// it was measured at 128ns a call — enough on its own to dominate a
    /// parameter sweep.
    fn series(&self, _p: &Params) -> Vec<String> {
        Vec::new()
    }

    /// True when the strategy reads the options frame.
    fn needs_options(&self) -> bool {
        false
    }

    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn indicators(&self, params: &Params) -> Vec<IndicatorSpec>;

    /// Bars that must pass before the strategy may trade.
    fn warmup(&self, params: &Params) -> usize;

    fn on_bar(&self, ctx: &BarContext) -> Intent;
}

#[derive(Debug, thiserror::Error)]
pub enum StrategyError {
    #[error("unknown strategy `{id}` (have: {known})")]
    Unknown { id: String, known: String },
    #[error("strategy has no parameter `{name}` (has: {known})")]
    UnknownParameter { name: String, known: String },
}

/// The set of available strategies.
pub struct Registry {
    strategies: Vec<Box<dyn Strategy>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self { strategies: Vec::new() }
    }

    /// Every built-in method, in display order.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        crate::builtin::register_all(&mut registry);
        registry
    }

    pub fn register(&mut self, strategy: Box<dyn Strategy>) {
        assert!(
            !self.strategies.iter().any(|s| s.id() == strategy.id()),
            "strategy already registered: {}",
            strategy.id()
        );
        self.strategies.push(strategy);
    }

    pub fn get(&self, id: &str) -> Result<&dyn Strategy, StrategyError> {
        self.strategies
            .iter()
            .find(|s| s.id() == id)
            .map(std::convert::AsRef::as_ref)
            .ok_or_else(|| StrategyError::Unknown {
                id: id.to_string(),
                known: self.ids().join(", "),
            })
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.strategies.iter().map(|s| s.id()).collect()
    }

    #[must_use]
    pub fn all(&self) -> &[Box<dyn Strategy>] {
        &self.strategies
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.strategies.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Every combination of a grid, layered onto the defaults.
///
/// Order is deterministic — keys ascending, values in declaration order — so a
/// sweep produces the same cells in the same sequence on every run.
#[must_use]
pub fn parameter_combinations(defaults: &Params, grid: &BTreeMap<String, Vec<f64>>) -> Vec<Params> {
    if grid.is_empty() {
        return vec![defaults.clone()];
    }
    let mut combos = vec![defaults.clone()];
    for (key, values) in grid {
        let mut next = Vec::with_capacity(combos.len() * values.len());
        for combo in &combos {
            for value in values {
                let mut variant = combo.clone();
                variant.set(key, *value);
                next.push(variant);
            }
        }
        combos = next;
    }
    combos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_reject_names_the_strategy_never_declared() {
        let defaults = Params::new(&[("fast", 21.0), ("slow", 55.0)]);
        let ok = defaults.with_overrides(&Params::new(&[("fast", 9.0)])).unwrap();
        assert_eq!(ok.get("fast"), 9.0);
        assert_eq!(ok.get("slow"), 55.0);

        let err = defaults.with_overrides(&Params::new(&[("nope", 1.0)])).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("nope") && message.contains("fast"), "{message}");
    }

    #[test]
    fn an_undeclared_parameter_reads_as_nan_rather_than_zero() {
        let params = Params::new(&[("fast", 21.0)]);
        assert!(params.get("missing").is_nan());
        assert_eq!(params.period("missing"), 0);
    }

    #[test]
    fn a_grid_expands_to_every_combination_deterministically() {
        let defaults = Params::new(&[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
        let mut grid = BTreeMap::new();
        grid.insert("a".to_string(), vec![1.0, 2.0]);
        grid.insert("b".to_string(), vec![5.0, 6.0]);

        let combos = parameter_combinations(&defaults, &grid);
        assert_eq!(combos.len(), 4);
        assert!(combos.iter().all(|c| c.get("c") == 3.0), "untouched keys survive");

        let again = parameter_combinations(&defaults, &grid);
        assert_eq!(combos, again, "the order must not vary between runs");
    }

    #[test]
    fn an_empty_grid_yields_the_defaults_alone() {
        let defaults = Params::new(&[("a", 1.0)]);
        assert_eq!(parameter_combinations(&defaults, &BTreeMap::new()), vec![defaults]);
    }

    #[test]
    fn the_context_returns_nan_outside_the_series() {
        let bars = vec![Bar::flat(0, 100.0), Bar::flat(60_000, 101.0)];
        let mut ind = fd_indicators::IndicatorSet::new();
        ind.insert("ema_3".to_string(), fd_indicators::Series::from(vec![f64::NAN, 100.5]));
        let params = Params::default();
        let ctx =
            BarContext { bar: &bars[1], i: 1, bars: &bars, ind: &ind, series: &[], options: None, position: None, params: &params };

        assert_eq!(ctx.v("ema_3"), 100.5);
        assert!(ctx.back("ema_3", 1).is_nan(), "warm-up value stays NaN");
        assert!(ctx.back("ema_3", 5).is_nan(), "reaching before the series is NaN, not a panic");
        assert!(ctx.v("missing").is_nan());
        assert_eq!(ctx.prev().map(|b| b.close), Some(100.0));
    }

    #[test]
    fn the_registry_refuses_a_duplicate_and_names_what_it_has() {
        let registry = Registry::with_builtins();
        assert!(registry.len() >= 9);
        assert!(registry.get("ema-cross").is_ok());

        // `unwrap_err` would need `Debug` on `&dyn Strategy`, which a trait
        // object cannot provide; match instead.
        let err = match registry.get("nothing") {
            Err(err) => err.to_string(),
            Ok(found) => panic!("expected an error, got {}", found.id()),
        };
        assert!(err.contains("nothing") && err.contains("ema-cross"), "{err}");
    }
}
