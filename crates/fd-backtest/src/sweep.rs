//! Comparison, parameter sweeps and walk-forward.
//!
//! Three deliberately separate things, in increasing order of how much the
//! number is worth:
//!
//! * [`compare_strategies`] — every method, default parameters, same bars.
//! * [`sweep_strategy`] — one method across a grid. A **distribution**, not a
//!   winner: with enough cells something always looks profitable.
//! * [`walk_forward`] — parameters chosen on a training slice, measured on the
//!   slice after it, rolled forward. The only number worth quoting.
//!
//! Sweeps are embarrassingly parallel and run on rayon. That is the whole point
//! of this crate existing in Rust: searching the hypothesis space is what finds
//! an edge, and the prototype searched it one cell at a time.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_strategy::registry::{Params, Registry, Strategy, parameter_combinations};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::context::OptionsTimeline;
use crate::engine::{BacktestResult, Metrics, Range, TradingRules, Trade, metrics_of, run_backtest, run_backtest_guarded};
use crate::guards::Guards;

/// Thresholds a result must clear to be worth a walk-forward.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PromisingGate {
    pub min_trades: usize,
    pub min_profit_factor: f64,
    pub min_expectancy_r: f64,
}

impl Default for PromisingGate {
    fn default() -> Self {
        Self { min_trades: 30, min_profit_factor: 1.2, min_expectancy_r: 0.05 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub promising: bool,
    pub reasons: Vec<String>,
}

#[must_use]
pub fn verdict(metrics: &Metrics, gate: &PromisingGate) -> Verdict {
    let mut reasons = Vec::new();
    if metrics.trades < gate.min_trades {
        reasons.push(format!("only {} trades (need {})", metrics.trades, gate.min_trades));
    }
    // Negated on purpose: a NaN profit factor or expectancy must fail the gate,
    // and `pf < min` is false for NaN. See `open_position` for the same shape.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(metrics.profit_factor >= gate.min_profit_factor) {
        reasons.push(format!("profit factor {:.3} < {}", metrics.profit_factor, gate.min_profit_factor));
    }
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(metrics.expectancy >= gate.min_expectancy_r) {
        reasons.push(format!("expectancy {:.3}R < {}R", metrics.expectancy, gate.min_expectancy_r));
    }
    Verdict { promising: reasons.is_empty(), reasons }
}

/// One row of the leaderboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardRow {
    pub id: String,
    pub name: String,
    /// Why this method could not run here, if it could not.
    pub skipped: Option<String>,
    pub params: Option<Params>,
    pub metrics: Option<Metrics>,
    pub score: f64,
}

/// Ranking score: profit factor discounted by sample size.
///
/// A strategy with three lucky trades must not top one with thirty. This sorts
/// the table; it settles nothing.
#[must_use]
pub fn score_of(metrics: Option<&Metrics>) -> f64 {
    let Some(m) = metrics else { return f64::NEG_INFINITY };
    if m.trades == 0 {
        return f64::NEG_INFINITY;
    }
    if m.trades < 5 {
        return -1e6 + m.trades as f64;
    }
    let pf = if m.profit_factor.is_finite() { m.profit_factor } else { 3.0 };
    pf * (m.trades as f64 / 30.0).min(1.0)
}

/// Run every registered strategy on the same bars.
pub fn compare_strategies(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    gate: &PromisingGate,
) -> Vec<LeaderboardRow> {
    compare_strategies_guarded(registry, bars, rules, timeline, gate, None)
}

/// [`compare_strategies`] with the risk guards applied to every run; `None`
/// is the unguarded leaderboard.
pub fn compare_strategies_guarded(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) -> Vec<LeaderboardRow> {
    let _ = gate;
    let mut rows: Vec<LeaderboardRow> = registry
        .all()
        .par_iter()
        .map(|strategy| {
            let strategy = strategy.as_ref();
            if strategy.needs_options() && timeline.is_none_or(OptionsTimeline::is_empty) {
                return LeaderboardRow {
                    id: strategy.id().to_string(),
                    name: strategy.name().to_string(),
                    skipped: Some("needs an options timeline".into()),
                    params: None,
                    metrics: None,
                    score: f64::NEG_INFINITY,
                };
            }
            let params = strategy.default_params();
            let result = run_backtest_guarded(bars, strategy, &params, rules, guards, timeline, Range::default(), None);
            let score = score_of(Some(&result.metrics));
            LeaderboardRow {
                id: strategy.id().to_string(),
                name: strategy.name().to_string(),
                skipped: None,
                params: Some(result.params),
                metrics: Some(result.metrics),
                score,
            }
        })
        .collect();

    rows.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    rows
}

/// One cell of a parameter sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepCell {
    pub params: Params,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepSummary {
    pub cells: usize,
    pub usable_cells: usize,
    pub median_profit_factor: f64,
    pub median_expectancy: f64,
    pub profitable_share: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepResult {
    pub strategy: String,
    pub cells: Vec<SweepCell>,
    pub summary: SweepSummary,
}

/// Run one strategy across its grid, in parallel.
pub fn sweep_strategy(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    min_trades_per_cell: usize,
) -> SweepResult {
    sweep_grid(strategy, bars, rules, timeline, &strategy.grid(), min_trades_per_cell)
}

/// [`sweep_strategy`] with the risk guards applied to every cell.
pub fn sweep_strategy_guarded(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) -> SweepResult {
    sweep_grid_guarded(strategy, bars, rules, timeline, &strategy.grid(), min_trades_per_cell, guards)
}

/// The same sweep over a caller-supplied grid, mirroring the prototype's
/// optional `grid` argument. Useful for refining a promising region without
/// editing the strategy, and for benchmarking a grid larger than the default.
pub fn sweep_grid(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    grid: &BTreeMap<String, Vec<f64>>,
    min_trades_per_cell: usize,
) -> SweepResult {
    sweep_grid_guarded(strategy, bars, rules, timeline, grid, min_trades_per_cell, None)
}

/// [`sweep_grid`] with the risk guards applied to every cell.
pub fn sweep_grid_guarded(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    grid: &BTreeMap<String, Vec<f64>>,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) -> SweepResult {
    let combos = parameter_combinations(&strategy.default_params(), grid);
    let mut cells: Vec<SweepCell> = combos
        .par_iter()
        .map(|params| {
            let result = run_backtest_guarded(bars, strategy, params, rules, guards, timeline, Range::default(), None);
            SweepCell { params: params.clone(), metrics: result.metrics }
        })
        .collect();

    let usable: Vec<&SweepCell> = cells.iter().filter(|c| c.metrics.trades >= min_trades_per_cell).collect();
    let mut pfs: Vec<f64> = usable
        .iter()
        .map(|c| if c.metrics.profit_factor.is_finite() { c.metrics.profit_factor } else { 3.0 })
        .collect();
    let mut expectancies: Vec<f64> = usable.iter().map(|c| c.metrics.expectancy).collect();
    pfs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    expectancies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let summary = SweepSummary {
        cells: cells.len(),
        usable_cells: usable.len(),
        median_profit_factor: median(&pfs),
        median_expectancy: median(&expectancies),
        profitable_share: if usable.is_empty() {
            f64::NAN
        } else {
            usable.iter().filter(|c| c.metrics.expectancy > 0.0).count() as f64 / usable.len() as f64
        },
    };

    cells.sort_by(|a, b| {
        b.metrics.profit_factor.partial_cmp(&a.metrics.profit_factor).unwrap_or(std::cmp::Ordering::Equal)
    });
    SweepResult { strategy: strategy.id().to_string(), cells, summary }
}

/// One fold of a walk-forward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fold {
    pub fold: usize,
    pub selected: Option<Params>,
    pub reason: Option<String>,
    pub train_score: f64,
    pub train_trades: usize,
    pub test: Option<Metrics>,
    pub window: (i64, i64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkForwardResult {
    pub strategy: String,
    pub folds: Vec<Fold>,
    /// Combined out-of-sample trades.
    pub oos: Metrics,
    pub oos_trades: Vec<Trade>,
    /// Whether the chosen parameters held still between folds.
    pub parameter_stability: BTreeMap<String, Vec<f64>>,
    /// Guard activity summed over the out-of-sample folds; empty and zero
    /// for an unguarded run. Training-window runs are not counted: they
    /// select, they do not measure.
    #[serde(default)]
    pub skipped_by_guard: BTreeMap<String, usize>,
    #[serde(default)]
    pub closed_by_guard: BTreeMap<String, usize>,
    #[serde(default)]
    pub sized_down_by_guard: usize,
}

/// What a fold selects on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectBy {
    #[default]
    Expectancy,
    ProfitFactor,
    TotalR,
}

impl SelectBy {
    fn score(self, metrics: &Metrics) -> f64 {
        let value = match self {
            Self::Expectancy => metrics.expectancy,
            Self::ProfitFactor => metrics.profit_factor,
            Self::TotalR => metrics.total_r,
        };
        if value.is_finite() { value } else { f64::NEG_INFINITY }
    }
}

/// Choose parameters on a training window, measure them on the next one, roll
/// forward.
pub fn walk_forward(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
) -> Option<WalkForwardResult> {
    walk_forward_guarded(strategy, bars, rules, timeline, folds, select_by, min_trades_per_cell, None)
}

/// [`walk_forward`] with the risk guards applied to every run, selection and
/// measurement alike: a guarded walk-forward selects on guarded numbers.
#[allow(clippy::too_many_arguments)]
pub fn walk_forward_guarded(
    strategy: &dyn Strategy,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) -> Option<WalkForwardResult> {
    if bars.len() < folds + 2 || folds == 0 {
        return None;
    }
    let combos = parameter_combinations(&strategy.default_params(), &strategy.grid());
    let block = bars.len() / (folds + 1);
    if block == 0 {
        return None;
    }

    let mut fold_results = Vec::with_capacity(folds);
    let mut oos_trades: Vec<Trade> = Vec::new();
    let mut skipped_by_guard: BTreeMap<String, usize> = BTreeMap::new();
    let mut closed_by_guard: BTreeMap<String, usize> = BTreeMap::new();
    let mut sized_down_by_guard = 0usize;

    for f in 1..=folds {
        let train_end = bars[block * f - 1].time;
        let test_start = bars[block * f].time;
        let test_end = bars[(block * (f + 1)).min(bars.len() - 1)].time;

        // Selection sees only the training window.
        let best = combos
            .par_iter()
            .filter_map(|params| {
                let result = run_backtest_guarded(
                    bars,
                    strategy,
                    params,
                    rules,
                    guards,
                    timeline,
                    Range { from: None, to: Some(train_end) },
                    None,
                );
                (result.metrics.trades >= min_trades_per_cell)
                    .then(|| (select_by.score(&result.metrics), result.metrics.trades, params.clone()))
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let Some((train_score, train_trades, params)) = best else {
            fold_results.push(Fold {
                fold: f,
                selected: None,
                reason: Some("no parameter set produced enough training trades".into()),
                train_score: f64::NAN,
                train_trades: 0,
                test: None,
                window: (test_start, test_end),
            });
            continue;
        };

        // Measurement sees only the window that follows it.
        let test = run_backtest_guarded(
            bars,
            strategy,
            &params,
            rules,
            guards,
            timeline,
            Range { from: Some(test_start), to: Some(test_end) },
            None,
        );
        oos_trades.extend(test.trades.iter().cloned());
        for (label, n) in &test.skipped_by_guard {
            *skipped_by_guard.entry(label.clone()).or_default() += n;
        }
        for (label, n) in &test.closed_by_guard {
            *closed_by_guard.entry(label.clone()).or_default() += n;
        }
        sized_down_by_guard += test.sized_down_by_guard;
        fold_results.push(Fold {
            fold: f,
            selected: Some(params),
            reason: None,
            train_score,
            train_trades,
            test: Some(test.metrics),
            window: (test_start, test_end),
        });
    }

    // A strategy whose best settings jump every fold has found nothing stable.
    let mut stability: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for fold in &fold_results {
        if let Some(params) = &fold.selected {
            for (key, value) in &params.0 {
                stability.entry(key.clone()).or_default().push(*value);
            }
        }
    }

    Some(WalkForwardResult {
        strategy: strategy.id().to_string(),
        folds: fold_results,
        oos: metrics_of(&oos_trades, rules.starting_equity_usd),
        oos_trades,
        parameter_stability: stability,
        skipped_by_guard,
        closed_by_guard,
        sized_down_by_guard,
    })
}

fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 { sorted[mid] } else { (sorted[mid - 1] + sorted[mid]) / 2.0 }
}

/// Convenience: run one strategy by id.
pub fn run_by_id(
    registry: &Registry,
    id: &str,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    overrides: Option<&Params>,
) -> Result<BacktestResult, fd_strategy::registry::StrategyError> {
    let strategy = registry.get(id)?;
    let params = match overrides {
        Some(o) => strategy.default_params().with_overrides(o)?,
        None => strategy.default_params(),
    };
    Ok(run_backtest(bars, strategy, &params, rules, timeline, Range::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_refuses_a_thin_sample_however_good_it_looks() {
        let gate = PromisingGate::default();
        let mut thin = Metrics::empty();
        thin.trades = 4;
        thin.profit_factor = 5.0;
        thin.expectancy = 1.0;
        let v = verdict(&thin, &gate);
        assert!(!v.promising);
        assert!(v.reasons[0].contains("only 4 trades"), "{:?}", v.reasons);
    }

    #[test]
    fn the_gate_passes_only_when_everything_clears() {
        let gate = PromisingGate::default();
        let mut solid = Metrics::empty();
        solid.trades = 50;
        solid.profit_factor = 1.4;
        solid.expectancy = 0.2;
        assert!(verdict(&solid, &gate).promising);
    }

    #[test]
    fn ranking_discounts_a_tiny_sample_instead_of_crowning_it() {
        let mut lucky = Metrics::empty();
        lucky.trades = 3;
        lucky.profit_factor = 9.0;
        let mut solid = Metrics::empty();
        solid.trades = 60;
        solid.profit_factor = 1.3;
        assert!(score_of(Some(&solid)) > score_of(Some(&lucky)));
        assert_eq!(score_of(None), f64::NEG_INFINITY);
    }

    #[test]
    fn median_handles_both_parities_and_emptiness() {
        assert!(median(&[]).is_nan());
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }
}
