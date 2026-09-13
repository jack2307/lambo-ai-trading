//! Strategy interface and registry.
//!
//! A strategy declares which indicators it needs, how long it must warm up, and
//! — given one bar's context — what it wants to do. It never touches positions,
//! sizing, stops or costs: the backtest engine owns those, so every method is
//! measured on identical machinery and a leaderboard compares the methods rather
//! than the measurement.
//!
//! The context carries only what existed at that bar. A strategy cannot reach
//! past it into the future, because there is nothing there to reach.

pub mod builtin;
pub mod filter;
pub mod ict;
pub mod orb;
pub mod pdhl;
pub mod registry;
pub mod session_hold;
pub mod tsmom;
pub mod vwap_fade;

pub use registry::{
    BarContext, Exits, Intent, Params, Registry, Side, Strategy, StrategyError, parameter_combinations,
};
