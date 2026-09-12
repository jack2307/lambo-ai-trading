//! Position-level risk guards.
//!
//! Caps on how often the system may trade and how much it may lose in a day,
//! enforced at the moment an entry is about to be taken. They exist so that a
//! run of bad signals is bounded by a rule rather than by someone's attention.
//!
//! Two things about where these live:
//!
//! * **They are opt-in for a backtest.** The prototype's backtest engine never
//!   applied them — only its live decision path did — and the parity gate
//!   proves this port reproduces that engine. So `run_backtest` takes `None`
//!   and reproduces the oracle; `search`, the API and the live loop pass the
//!   configured guards and get the bounded behaviour. A backtest with guards
//!   and one without are different questions, and both are worth asking.
//! * **They were configuration with no reader.** `GuardsConfig` existed in
//!   `config/default.toml` and nothing in the engine read it. A limit that is
//!   read by no code is not a limit, it is an intention — and the day that
//!   matters is the day nobody has the attention to notice.
//!
//! Semantics follow the prototype's `riskGuard` in `src/ai/decide.js`: the
//! "day" is a rolling 24 hours ending now, not a calendar day, and the daily
//! figures count trades **closed** in that window.

use std::collections::VecDeque;

use fd_core::config::Config;

/// Milliseconds in the rolling window.
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guards {
    pub max_concurrent_positions: usize,
    pub max_trades_per_day: usize,
    pub daily_loss_limit_usd: f64,
    pub cooldown_ms: i64,
}

impl Guards {
    /// The configured guards, from `[trading.guards]`.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        let g = &config.trading.guards;
        Self {
            max_concurrent_positions: g.max_concurrent_positions,
            max_trades_per_day: g.max_trades_per_day,
            daily_loss_limit_usd: g.daily_loss_limit_usd,
            cooldown_ms: g.cooldown_ms,
        }
    }
}

/// Why an entry was refused. The order is the order the checks run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// As many positions open as allowed.
    Concurrent,
    /// The rolling day's realised loss has reached the limit.
    DailyLoss,
    /// The rolling day's trade count has reached the cap.
    DailyCap,
    /// Too soon after the previous entry.
    Cooldown,
}

impl Refusal {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Concurrent => "MAX_CONCURRENT",
            Self::DailyLoss => "DAILY_LOSS_LIMIT",
            Self::DailyCap => "DAILY_TRADE_CAP",
            Self::Cooldown => "COOLDOWN",
        }
    }
}

/// What the guard has to remember, fed by the engine as trades open and close.
///
/// Bars arrive in time order, so closed trades older than the window are
/// dropped from the front as it slides; the deque never holds more than a
/// day's trades.
#[derive(Debug, Default)]
pub struct GuardState {
    /// `(exit_time, pnl_usd)` of closed trades, oldest first.
    closed: VecDeque<(i64, f64)>,
    /// Most recent entry, open or closed.
    ///
    /// The prototype reads the last *closed* trade's entry first and only
    /// falls back to an open one, which lets a fresh open position escape the
    /// cooldown check. Using the most recent entry of either kind is stricter,
    /// never looser, and is the reading a cooldown was written to mean.
    last_entry_time: Option<i64>,
}

impl GuardState {
    pub fn opened(&mut self, entry_time: i64) {
        self.last_entry_time = Some(self.last_entry_time.map_or(entry_time, |t| t.max(entry_time)));
    }

    pub fn closed(&mut self, exit_time: i64, pnl_usd: f64) {
        self.closed.push_back((exit_time, pnl_usd));
    }

    /// The first reason an entry at `now` would be refused, if any.
    pub fn refusal(&mut self, guards: &Guards, now: i64, open_positions: usize) -> Option<Refusal> {
        if open_positions >= guards.max_concurrent_positions {
            return Some(Refusal::Concurrent);
        }

        let day_start = now - DAY_MS;
        while self.closed.front().is_some_and(|(exit, _)| *exit < day_start) {
            self.closed.pop_front();
        }
        let day_pnl: f64 = self.closed.iter().map(|(_, pnl)| pnl).sum();
        if day_pnl <= -guards.daily_loss_limit_usd.abs() {
            return Some(Refusal::DailyLoss);
        }
        if self.closed.len() >= guards.max_trades_per_day {
            return Some(Refusal::DailyCap);
        }

        if let Some(last) = self.last_entry_time
            && now - last < guards.cooldown_ms
        {
            return Some(Refusal::Cooldown);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;

    fn guards() -> Guards {
        Guards { max_concurrent_positions: 1, max_trades_per_day: 2, daily_loss_limit_usd: 300.0, cooldown_ms: 30 * MINUTE }
    }

    #[test]
    fn nothing_is_refused_on_a_clean_slate() {
        let mut state = GuardState::default();
        assert_eq!(state.refusal(&guards(), 10 * HOUR, 0), None);
    }

    #[test]
    fn an_open_position_blocks_another_when_the_cap_is_one() {
        // The engine holds one position structurally; the guard says so in its
        // own words so a wider engine cannot forget.
        let mut state = GuardState::default();
        assert_eq!(state.refusal(&guards(), 10 * HOUR, 1), Some(Refusal::Concurrent));
    }

    #[test]
    fn the_daily_cap_counts_trades_closed_in_the_last_24_hours() {
        let mut state = GuardState::default();
        state.closed(20 * HOUR, 10.0);
        state.closed(21 * HOUR, 10.0);
        assert_eq!(state.refusal(&guards(), 22 * HOUR, 0), Some(Refusal::DailyCap));

        // Twenty-five hours later the first has left the window and the cap
        // has room again.
        assert_eq!(state.refusal(&guards(), 45 * HOUR + MINUTE, 0), None);
    }

    #[test]
    fn a_days_loss_at_the_limit_stops_new_entries() {
        let mut state = GuardState::default();
        state.closed(20 * HOUR, -150.0);
        state.closed(21 * HOUR, -150.0);
        // Exactly the limit counts: the prototype uses `<=`, and so does this.
        assert_eq!(state.refusal(&guards(), 21 * HOUR + MINUTE, 0), Some(Refusal::DailyLoss));
    }

    #[test]
    fn the_loss_limit_is_read_as_a_magnitude() {
        // A configured `-300` and `300` must mean the same thing; a sign typo
        // in a risk limit must not turn it off.
        let mut state = GuardState::default();
        state.closed(20 * HOUR, -300.0);
        let mut negative = guards();
        negative.daily_loss_limit_usd = -300.0;
        assert_eq!(state.refusal(&negative, 21 * HOUR, 0), Some(Refusal::DailyLoss));
    }

    #[test]
    fn the_loss_check_runs_before_the_cap_check() {
        // Both fire; the loss is the more important reason and is the one
        // reported, matching the prototype's order.
        let mut state = GuardState::default();
        state.closed(20 * HOUR, -200.0);
        state.closed(21 * HOUR, -200.0);
        assert_eq!(state.refusal(&guards(), 22 * HOUR, 0), Some(Refusal::DailyLoss));
    }

    #[test]
    fn the_cooldown_runs_from_the_previous_entry() {
        let mut state = GuardState::default();
        state.opened(10 * HOUR);
        state.closed(10 * HOUR + 5 * MINUTE, 40.0);
        assert_eq!(state.refusal(&guards(), 10 * HOUR + 20 * MINUTE, 0), Some(Refusal::Cooldown));
        assert_eq!(state.refusal(&guards(), 10 * HOUR + 31 * MINUTE, 0), None);
    }

    #[test]
    fn the_cooldown_also_sees_a_position_that_is_still_open() {
        let mut state = GuardState::default();
        state.opened(10 * HOUR);
        // No close yet. With the cap at one this is refused as Concurrent
        // first; with a wider cap the cooldown must still catch it.
        let mut wide = guards();
        wide.max_concurrent_positions = 3;
        assert_eq!(state.refusal(&wide, 10 * HOUR + 10 * MINUTE, 1), Some(Refusal::Cooldown));
    }

    #[test]
    fn the_configured_guards_are_the_ones_in_the_toml() {
        let config = Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"),
        )
        .expect("the workspace config");
        let guards = Guards::from_config(&config);
        assert_eq!(guards.max_concurrent_positions, config.trading.guards.max_concurrent_positions);
        assert_eq!(guards.max_trades_per_day, config.trading.guards.max_trades_per_day);
        assert_eq!(guards.daily_loss_limit_usd, config.trading.guards.daily_loss_limit_usd);
        assert_eq!(guards.cooldown_ms, config.trading.guards.cooldown_ms);
        assert!(guards.max_trades_per_day > 0, "a cap of zero would refuse every trade silently");
    }
}
