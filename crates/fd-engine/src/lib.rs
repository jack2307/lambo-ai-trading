//! Options-derived levels, and — since 2026-09-19 — price-bar ones beside
//! them.
//!
//! Most of this crate turns a tape of option prints into prices a trader can
//! point at: where premium is concentrated, where a settlement would hurt the
//! most option holders, where large prints have anchored themselves.
//! [`price_levels`] is the one module whose input is BARS rather than prints.
//! It lives here because what this crate IS is "a price a trader can point at,
//! derived" rather than "the option tape", and its own doc comment says which
//! of its neighbours it deliberately does not share code with and why.
//!
//! Two standing cautions, enforced by naming rather than convention:
//!
//! * **There is no open interest here.** Neither feed publishes it. What this
//!   crate calls max pain is derived from traded positioning, and the type is
//!   named [`maxpain::PositioningTable`] rather than an OI table so the
//!   difference cannot be quietly forgotten downstream.
//! * **Some formulas are unverified.** Break-even and whale levels remain
//!   candidate models behind a registry, exactly as in the prototype. A level
//!   whose formula has never been reproduced is marked experimental all the way
//!   to the screen.

pub mod bigtrades;
pub mod breakeven;
pub mod engine;
pub mod flow;
pub mod levels;
pub mod maxpain;
pub mod price_levels;
pub mod profile;
pub mod whale;

pub use bigtrades::{BigTradeConfig, WhaleFootprint, WhaleStrike, is_big_trade, quantile, select_big_trades};
pub use flow::{
    Bias, BiasThresholds, FlowSummary, FlowVelocity, bias_label, net_flow_velocity, summarize, summarize_window,
};
pub use maxpain::{
    DataState, PositioningMode, PositioningTable, StrikePositioning, flow_max_pain, max_pain, payout_curve,
    positioning_table,
};
pub use price_levels::{
    BarProfile, ConfirmedSwing, Direction, FairValueGap, LevelKind, LevelState, LiquidityPool, LiquiditySide,
    OrderBlock, PeriodExtremes, PriceLevel, activity_profile, bucket_size_price, confirmed_swings,
    liquidity_pools, order_blocks, period_extremes, period_pool, runs_split_by_gap, swing_id,
    unfilled_fair_value_gaps,
};
pub use profile::{ProfileMode, ProfileOptions, StrikeProfile, ValueArea, build_profile, value_area};
