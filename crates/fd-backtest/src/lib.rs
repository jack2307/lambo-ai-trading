//! Backtesting.
//!
//! One engine, one fill model, every strategy. The point of the shared path is
//! that a leaderboard then compares the methods rather than comparing the
//! assumptions each of them happened to be measured under.
//!
//! Sweeps and walk-forward run on rayon, which is the reason this crate exists
//! in Rust at all: finding an edge means searching a hypothesis space, and the
//! JavaScript prototype searched it one cell at a time.

pub mod context;
pub mod control;
pub mod control_hold;
pub mod cost_table;
pub mod direction;
pub mod engine;
pub mod guards;
pub mod hypotheses;
pub mod paper;
pub mod rebate;
pub mod sweep;
pub mod timeline;

pub use context::{Frame, OptionsTimeline};
pub use control::RandomEntry;
pub use control_hold::RandomHold;
pub use timeline::{TimelineOptions, build_timeline, frame_from_snapshot};
pub use engine::{
    BacktestResult, ExitKind, Metrics, Range, Trade, TradingRules, metrics_of, run_backtest,
    run_backtest_guarded, trading_rules_for,
};
pub use direction::{DirectionFlipped, permuted_sides_pnls, profit_factor_of};
pub use rebate::{Rebate, usd_per_r};
pub use guards::{Exposure, GuardKind, GuardState, Guards, Refusal, guard_exit};
pub use paper::{PaperBook, StepReport};
pub use sweep::{
    Fold, LeaderboardRow, PromisingGate, SelectBy, SweepCell, SweepResult, SweepSummary, Verdict,
    WalkForwardResult, compare_strategies, compare_strategies_guarded, run_by_id, score_of, sweep_grid,
    sweep_grid_guarded, sweep_strategy, sweep_strategy_guarded, verdict, walk_forward, walk_forward_guarded,
};
