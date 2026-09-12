//! Sweep and walk-forward throughput.
//!
//! This benchmark exists to answer one question the migration plan asks in
//! writing: is searching the hypothesis space actually faster in Rust, and by
//! how much? The JavaScript prototype's side of the comparison is measured by
//! `research/bench-sweep.js` in the Node repo, which reads the **same golden
//! files** this bench reads — same bars, same options timeline, same config —
//! so the ratio is about the two implementations and not about their inputs.
//!
//! Two grids are measured on purpose:
//!
//!   `oracle-grid`  the 82 cells per market the prototype actually runs. This
//!                  is the honest apples-to-apples number, and it is small
//!                  enough that thread start-up is a visible share of it.
//!   `dense-grid`   every parameter axis refined. This is the workload that is
//!                  blocked today, and the one the port was for.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use fd_backtest::context::OptionsTimeline;
use fd_backtest::engine::TradingRules;
use fd_backtest::sweep::{SelectBy, sweep_grid, sweep_strategy, walk_forward};
use fd_core::types::Bar;
use fd_strategy::registry::{Registry, Strategy, parameter_combinations};

#[path = "../tests/support/golden.rs"]
mod golden;

const MARKETS: [&str; 2] = ["gold", "btc"];
const FOLDS: usize = 4;
const MIN_TRADES_PER_CELL: usize = 5;

struct Tape {
    bars: Vec<Bar>,
    rules: TradingRules,
    timeline: Option<OptionsTimeline>,
}

fn tape(market: &str) -> Option<Tape> {
    Some(Tape {
        bars: golden::load_bars(market)?,
        rules: golden::rules_from_manifest(market)?,
        timeline: golden::load_timeline(market),
    })
}

type Grid = BTreeMap<String, Vec<f64>>;

/// Refine every axis of a strategy's grid by interpolating between the values
/// it already declares. The point is to grow the cell count without inventing
/// parameter ranges the strategy never claimed were sensible.
fn densify(grid: &Grid, target_per_axis: usize) -> Grid {
    let mut dense = Grid::new();
    for (name, values) in grid.iter() {
        if values.len() < 2 {
            dense.insert(name.clone(), values.clone());
            continue;
        }
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // Integer-valued axes (periods, lookbacks) must stay integers or the
        // indicator cache keys stop matching and the sweep measures something
        // else entirely.
        let integral = values.iter().all(|v| v.fract() == 0.0);
        let steps = target_per_axis.max(2);
        let mut refined: Vec<f64> = (0..steps)
            .map(|i| {
                let t = i as f64 / (steps - 1) as f64;
                let v = lo + (hi - lo) * t;
                if integral { v.round() } else { v }
            })
            .collect();
        refined.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
        dense.insert(name.clone(), refined);
    }
    dense
}

fn cells_of(strategy: &dyn Strategy) -> usize {
    parameter_combinations(&strategy.default_params(), &strategy.grid()).len()
}

fn bench_oracle_grid(c: &mut Criterion) {
    let registry = Registry::with_builtins();
    let mut group = c.benchmark_group("sweep/oracle-grid");
    group.measurement_time(Duration::from_secs(10));

    for market in MARKETS {
        let Some(tape) = tape(market) else {
            eprintln!("skipping {market}: golden files missing — run research/export-golden.js");
            continue;
        };
        let runnable: Vec<&dyn Strategy> = registry
            .all()
            .iter()
            .map(std::convert::AsRef::as_ref)
            .filter(|s| !s.needs_options() || tape.timeline.is_some())
            .collect();
        let total_cells: usize = runnable.iter().map(|s| cells_of(*s)).sum();

        // One iteration = every strategy swept once, which is exactly what the
        // JavaScript harness reports as `totalSweepMs`.
        group.bench_function(BenchmarkId::new("all-strategies", market), |b| {
            b.iter(|| {
                for strategy in &runnable {
                    black_box(sweep_strategy(
                        *strategy,
                        &tape.bars,
                        &tape.rules,
                        tape.timeline.as_ref(),
                        MIN_TRADES_PER_CELL,
                    ));
                }
            });
        });

        group.bench_function(BenchmarkId::new("walk-forward-all", market), |b| {
            b.iter(|| {
                for strategy in &runnable {
                    black_box(walk_forward(
                        *strategy,
                        &tape.bars,
                        &tape.rules,
                        tape.timeline.as_ref(),
                        FOLDS,
                        SelectBy::Expectancy,
                        MIN_TRADES_PER_CELL,
                    ));
                }
            });
        });

        eprintln!("{market}: {} bars, {} strategies, {total_cells} cells", tape.bars.len(), runnable.len());
    }
    group.finish();
}

fn bench_dense_grid(c: &mut Criterion) {
    let registry = Registry::with_builtins();
    let mut group = c.benchmark_group("sweep/dense-grid");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    let market = "btc";
    let Some(tape) = tape(market) else { return };
    let strategy = registry.get("ema-cross").expect("ema-cross is a built-in");

    for per_axis in [4usize, 8, 12, 20, 28] {
        let grid = densify(&strategy.grid(), per_axis);
        let cells = parameter_combinations(&strategy.default_params(), &grid).len();
        group.bench_with_input(BenchmarkId::new("ema-cross", cells), &grid, |b, grid| {
            b.iter(|| {
                black_box(sweep_grid(
                    strategy,
                    &tape.bars,
                    &tape.rules,
                    tape.timeline.as_ref(),
                    grid,
                    MIN_TRADES_PER_CELL,
                ));
            });
        });
    }
    group.finish();
}

/// The two pieces a sweep is built from, measured on their own.
///
/// A sweep is `cells x (indicators + bar loop)`. Timing only the whole thing
/// leaves it ambiguous which half to work on, and guessing wrong costs a day.
fn bench_primitives(c: &mut Criterion) {
    let registry = Registry::with_builtins();
    let Some(tape) = tape("btc") else { return };
    let strategy = registry.get("ema-cross").expect("ema-cross is a built-in");
    let params = strategy.default_params();

    let mut group = c.benchmark_group("primitives");
    group.bench_function("compute_indicators/ema-only", |b| {
        let specs = vec![fd_indicators::IndicatorSpec::new("ema").with("period", 21.0)];
        b.iter(|| black_box(fd_indicators::compute_indicators(&tape.bars, &specs).unwrap()));
    });
    group.bench_function("compute_indicators/ema-cross-set", |b| {
        let specs = strategy.indicators(&params);
        b.iter(|| black_box(fd_indicators::compute_indicators(&tape.bars, &specs).unwrap()));
    });
    group.bench_function("run_backtest/one-cell", |b| {
        b.iter(|| {
            black_box(fd_backtest::engine::run_backtest(
                &tape.bars,
                strategy,
                &params,
                &tape.rules,
                tape.timeline.as_ref(),
                fd_backtest::engine::Range::default(),
            ))
        });
    });
    group.finish();
}

criterion_group!(benches, bench_primitives, bench_oracle_grid, bench_dense_grid);
criterion_main!(benches);
