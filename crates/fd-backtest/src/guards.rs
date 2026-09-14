//! Position-level risk guards.
//!
//! Caps on how often the system may trade, how much it may lose in a day,
//! how large a position may be, and — since 2026-09-14 — what can close a
//! position the strategy would otherwise hold: an unrealised-loss cap, a
//! Friday cut-off and a scheduled-news window. They exist so that a run of
//! bad signals, or one hold through the wrong weekend, is bounded by a rule
//! rather than by someone's attention.
//!
//! Two things about where these live:
//!
//! * **They are opt-in for a backtest.** The prototype's backtest engine never
//!   applied them — only its live decision path did — and the parity gate
//!   proves this port reproduces that engine. So `run_backtest` takes `None`
//!   and reproduces the oracle; the API passes the configured guards when a
//!   request asks for them (`guards: true`, off by default), and `search`
//!   passes them when run with `--guards`. A guarded run's receipt says so
//!   in its header; every receipt under `docs/research/runs/` before
//!   2026-09-14 is an unguarded number. A backtest with guards and one
//!   without are different questions, and both are worth asking.
//! * **They were configuration with no reader.** `GuardsConfig` existed in
//!   `config/default.toml` and nothing in the engine read it. A limit that is
//!   read by no code is not a limit, it is an intention — and the day that
//!   matters is the day nobody has the attention to notice.
//!
//! Semantics of the entry guards follow the prototype's `riskGuard` in
//! `src/ai/decide.js`: the "day" is a rolling 24 hours ending now, not a
//! calendar day, and the daily figures count trades **closed** in that window.
//!
//! # The position guards
//!
//! Each is off at zero, and with every one at zero a guarded run is
//! bit-identical to an unguarded one (`tests/guards_positions.rs`). They
//! apply to **every** open position, self-managed ones included — that is
//! their purpose: until they existed, no stop, clock or guard could close an
//! `Exits::Strategy` hold (close-reopen-drift, 2026-09-13: *"nothing can
//! close a position"*).
//!
//! * **Open-loss cap** (`max_open_loss_r`): a level `entry ∓ r × risk` where
//!   `risk` is the position's sizing unit. Checked like a stop against the
//!   bar's low (long) or high (short); the fill is the level, or the bar's
//!   open when the bar gapped through it, less exit costs. Exit reason
//!   `OPEN_LOSS_CAP`.
//! * **Notional cap** (`max_notional_pct_equity`): at entry, lots are cut so
//!   `lots × contract × price ≤ pct/100 × equity`, rounded down to the lot
//!   step; when even the minimum lot exceeds the cap the entry is refused.
//! * **Weekend flat** (`flat_before_weekend_hhmm`): a bar is *past the
//!   cut-off* when the instant it **closes** (open time plus the feed's bar
//!   interval) is a Friday at or after HHMM New York, or any Saturday. The
//!   first such bar closes the position at its close, and no entry fills on
//!   a bar past the cut-off or from a signal produced on one. The close
//!   instant, not the open, so that `1655` on a 15-minute feed flattens at
//!   the 16:45 bar's close (17:00, the last Friday print) instead of never
//!   — no Friday bar of that feed *opens* at or after 16:55.
//! * **News flat** (`news_flat_before_min` / `_after_min` / `news_min_impact`):
//!   inside `[event − before, event + after)` of any installed event with
//!   impact ≥ min ([`fd_strategy::news::in_blackout`] on the bar's open
//!   time), the first bar inside closes the position at its close and no
//!   entry fills inside or from a signal inside. Inert when no calendar is
//!   installed.
//!
//! Ordering against the engine's own exits: the engine's stop, target and
//! clock are consulted first, then the open-loss cap, then the weekend and
//! news windows. A stop is at exactly 1R by construction (`risk = |entry −
//! stop|`), so on an engine-managed position a cap of 1R or more can never
//! precede the stop; with a cap under 1R and a bar that covers both, the
//! stop is taken — the same pessimistic reading the engine already applies
//! when one bar covers both stop and target.

use std::collections::VecDeque;

use fd_core::clock::new_york_local;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_strategy::registry::Side;

use crate::engine::{ExitKind, TradingRules, apply_costs};

/// Milliseconds in the rolling window.
const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guards {
    pub max_concurrent_positions: usize,
    pub max_trades_per_day: usize,
    pub daily_loss_limit_usd: f64,
    pub cooldown_ms: i64,
    /// Unrealised-loss cap on the open position, in R. 0 = off.
    pub max_open_loss_r: f64,
    /// Cap on `lots × contract × price` as a percentage of equity. 0 = off.
    pub max_notional_pct_equity: f64,
    /// Friday New York HHMM from which the book is flat. 0 = off.
    pub flat_before_weekend_hhmm: u32,
    /// Minutes before a scheduled event the book is flat. 0/0 = off.
    pub news_flat_before_min: u32,
    /// Minutes after a scheduled event the book is flat.
    pub news_flat_after_min: u32,
    /// Lowest impact the news guard reacts to.
    pub news_min_impact: u8,
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
            max_open_loss_r: g.max_open_loss_r,
            max_notional_pct_equity: g.max_notional_pct_equity,
            flat_before_weekend_hhmm: g.flat_before_weekend_hhmm,
            news_flat_before_min: g.news_flat_before_min,
            news_flat_after_min: g.news_flat_after_min,
            news_min_impact: g.news_min_impact,
        }
    }

    /// Every guard off: no cap bites, no window closes anything. A guarded
    /// run with these is the unguarded run, which is what a test that turns
    /// one guard on at a time starts from.
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            max_concurrent_positions: usize::MAX,
            max_trades_per_day: usize::MAX,
            daily_loss_limit_usd: f64::INFINITY,
            cooldown_ms: 0,
            max_open_loss_r: 0.0,
            max_notional_pct_equity: 0.0,
            flat_before_weekend_hhmm: 0,
            news_flat_before_min: 0,
            news_flat_after_min: 0,
            news_min_impact: 3,
        }
    }

    /// One line for a receipt header, so a record can quote what bounded it.
    #[must_use]
    pub fn describe(&self) -> String {
        let weekend = if self.flat_before_weekend_hhmm == 0 {
            "off".to_string()
        } else {
            format!("{:02}:{:02} NY", self.flat_before_weekend_hhmm / 100, self.flat_before_weekend_hhmm % 100)
        };
        let news = if self.news_guard_on() {
            format!("{}/{} (impact≥{})", self.news_flat_before_min, self.news_flat_after_min, self.news_min_impact)
        } else {
            "off".to_string()
        };
        let off_or = |on: bool, text: String| if on { text } else { "off".to_string() };
        // `2.0` reads as a multiple of R where `2` reads as a count.
        let r = if self.max_open_loss_r.fract() == 0.0 { format!("{:.1}", self.max_open_loss_r) } else { format!("{}", self.max_open_loss_r) };
        format!(
            "max_open_loss_r {}, notional {}, weekend flat {weekend}, news flat {news}, daily cap {}, loss limit ${}, cooldown {} min",
            off_or(self.max_open_loss_r > 0.0, r),
            off_or(self.max_notional_pct_equity > 0.0, format!("{}%", self.max_notional_pct_equity)),
            self.max_trades_per_day,
            self.daily_loss_limit_usd,
            self.cooldown_ms / MINUTE_MS,
        )
    }

    fn news_guard_on(&self) -> bool {
        self.news_flat_before_min > 0 || self.news_flat_after_min > 0
    }

    /// True when the weekend guard is on and a bar spanning `open_ms` to
    /// `close_ms` (UTC) ends past the Friday cut-off — or on a Saturday,
    /// which no feed should print and is inside the weekend if one does.
    #[must_use]
    pub fn past_weekend_cutoff(&self, close_ms: i64) -> bool {
        if self.flat_before_weekend_hhmm == 0 {
            return false;
        }
        let cutoff = (self.flat_before_weekend_hhmm / 100) * 60 + self.flat_before_weekend_hhmm % 100;
        let (weekday, minute) = new_york_local(close_ms);
        (weekday == 5 && minute >= cutoff) || weekday == 6
    }

    /// True when the news guard is on and `t_ms` is inside the window of an
    /// installed event. False with no calendar installed.
    #[must_use]
    pub fn in_news_window(&self, t_ms: i64) -> bool {
        self.news_guard_on()
            && fd_strategy::news::in_blackout(
                t_ms,
                i64::from(self.news_flat_before_min) * MINUTE_MS,
                i64::from(self.news_flat_after_min) * MINUTE_MS,
                self.news_min_impact,
            )
    }

    /// Why an entry filling on the bar `fill`, from a signal on the bar
    /// `signal`, is refused by a calendar guard — the weekend cut-off or a
    /// news window. Both bars are checked: a signal produced inside a window
    /// is not acted on even when its fill would land outside (the Sunday
    /// reopen fill of a Friday-close signal), and a fill inside a window is
    /// refused whatever produced it.
    #[must_use]
    pub fn calendar_refusal(&self, signal: &Bar, fill: &Bar, bar_ms: i64) -> Option<Refusal> {
        if self.past_weekend_cutoff(signal.time + bar_ms) || self.past_weekend_cutoff(fill.time + bar_ms) {
            return Some(Refusal::Weekend);
        }
        if self.in_news_window(signal.time) || self.in_news_window(fill.time) {
            return Some(Refusal::News);
        }
        None
    }

    /// The lots the notional cap allows for `lots` at `entry_price` on
    /// `equity`: `Ok((lots, true))` when it cut them, `Ok((lots, false))`
    /// when it did not, `Err(Refusal::Notional)` when even `min_lot` is over
    /// the cap. Untouched when the cap is off.
    pub fn cap_lots(&self, lots: f64, entry_price: f64, equity: f64, rules: &TradingRules) -> Result<(f64, bool), Refusal> {
        if self.max_notional_pct_equity <= 0.0 {
            return Ok((lots, false));
        }
        let notional = lots * rules.contract_size * entry_price;
        let allowed_usd = self.max_notional_pct_equity / 100.0 * equity;
        if notional <= allowed_usd {
            return Ok((lots, false));
        }
        let raw = allowed_usd / (rules.contract_size * entry_price);
        let capped = (raw / rules.lot_step).floor() * rules.lot_step;
        if capped < rules.min_lot {
            return Err(Refusal::Notional);
        }
        Ok((capped, true))
    }
}

/// What the position guards need to know about an open position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    pub side: Side,
    pub entry_price: f64,
    /// The sizing unit: one R in price.
    pub risk: f64,
}

/// Which position guard closed a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GuardKind {
    OpenLoss,
    Weekend,
    News,
}

impl GuardKind {
    /// The `exit_reason` a guard exit is recorded under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenLoss => "OPEN_LOSS_CAP",
            Self::Weekend => "WEEKEND_FLAT",
            Self::News => "NEWS_FLAT",
        }
    }
}

/// The exit a position guard forces on `bar`, if any: `(fill price after
/// exit costs, kind)`. Open-loss cap first (a level inside the bar), then
/// the weekend cut-off and the news window (both at the bar's close).
///
/// `bar_ms` is the feed's bar interval, so the bar's close instant is
/// `bar.time + bar_ms`; see the weekend guard in the module doc.
#[must_use]
pub fn guard_exit(position: &Exposure, bar: &Bar, bar_ms: i64, rules: &TradingRules, guards: &Guards) -> Option<(f64, ExitKind)> {
    let long = position.side.is_long();

    if guards.max_open_loss_r > 0.0 && position.risk.is_finite() && position.risk > 0.0 {
        let distance = guards.max_open_loss_r * position.risk;
        let level = if long { position.entry_price - distance } else { position.entry_price + distance };
        let hit = if long { bar.low <= level } else { bar.high >= level };
        if hit {
            let gapped = if long { bar.open <= level } else { bar.open >= level };
            let raw = if gapped { bar.open } else { level };
            return Some((apply_costs(raw, position.side, false, rules), ExitKind::Guard(GuardKind::OpenLoss)));
        }
    }

    if guards.past_weekend_cutoff(bar.time + bar_ms) {
        return Some((apply_costs(bar.close, position.side, false, rules), ExitKind::Guard(GuardKind::Weekend)));
    }

    if guards.in_news_window(bar.time) {
        return Some((apply_costs(bar.close, position.side, false, rules), ExitKind::Guard(GuardKind::News)));
    }

    None
}

/// The feed's bar interval: the smallest positive gap between consecutive
/// bars. A weekend or a holiday is a larger gap; the smallest is the bar.
/// Zero for a series of fewer than two bars.
#[must_use]
pub fn bar_interval_ms(bars: &[Bar]) -> i64 {
    bars.windows(2).map(|w| w[1].time - w[0].time).filter(|d| *d > 0).min().unwrap_or(0)
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
    /// Past the Friday cut-off, on the signal bar or the fill bar.
    Weekend,
    /// Inside a scheduled-news window, on the signal bar or the fill bar.
    News,
    /// Even the minimum lot is over the notional cap.
    Notional,
}

impl Refusal {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Concurrent => "MAX_CONCURRENT",
            Self::DailyLoss => "DAILY_LOSS_LIMIT",
            Self::DailyCap => "DAILY_TRADE_CAP",
            Self::Cooldown => "COOLDOWN",
            Self::Weekend => "WEEKEND_FLAT",
            Self::News => "NEWS_FLAT",
            Self::Notional => "NOTIONAL_CAP",
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
    use fd_core::clock::days_from_civil;

    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;

    fn guards() -> Guards {
        Guards {
            max_concurrent_positions: 1,
            max_trades_per_day: 2,
            daily_loss_limit_usd: 300.0,
            cooldown_ms: 30 * MINUTE,
            ..Guards::unbounded()
        }
    }

    fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> i64 {
        days_from_civil(year, month, day) * DAY_MS + hour * HOUR + minute * MINUTE
    }

    fn bar(time: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time, open, high, low, close, volume: None }
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
    fn unbounded_refuses_nothing() {
        let mut state = GuardState::default();
        state.opened(10 * HOUR);
        for i in 0..50 {
            state.closed(10 * HOUR + i * MINUTE, -1e6);
        }
        assert_eq!(state.refusal(&Guards::unbounded(), 11 * HOUR, 1), None);
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
        assert_eq!(guards.max_open_loss_r, config.trading.guards.max_open_loss_r);
        assert_eq!(guards.max_notional_pct_equity, config.trading.guards.max_notional_pct_equity);
        assert_eq!(guards.flat_before_weekend_hhmm, config.trading.guards.flat_before_weekend_hhmm);
        assert_eq!(guards.news_flat_before_min, config.trading.guards.news_flat_before_min);
        assert_eq!(guards.news_flat_after_min, config.trading.guards.news_flat_after_min);
        assert_eq!(guards.news_min_impact, config.trading.guards.news_min_impact);
        assert!(guards.max_trades_per_day > 0, "a cap of zero would refuse every trade silently");
        // The risk role's four are on in the shipped config; a zero here
        // would be an intention again.
        assert!(guards.max_open_loss_r > 0.0 && guards.max_notional_pct_equity > 0.0);
        assert!(guards.flat_before_weekend_hhmm > 0 && guards.news_flat_before_min > 0);
    }

    #[test]
    fn a_config_without_the_new_keys_still_loads_with_them_off() {
        let text = "max_concurrent_positions = 1\nmax_trades_per_day = 4\ndaily_loss_limit_usd = 300.0\ncooldown_ms = 0\n";
        let g: fd_core::config::GuardsConfig = toml::from_str(text).expect("old guards table");
        assert_eq!(g.max_open_loss_r, 0.0);
        assert_eq!(g.max_notional_pct_equity, 0.0);
        assert_eq!(g.flat_before_weekend_hhmm, 0);
        assert_eq!((g.news_flat_before_min, g.news_flat_after_min), (0, 0));
        assert_eq!(g.news_min_impact, 3, "the impact threshold keeps the filter's default");
    }

    #[test]
    fn the_describe_line_names_every_guard() {
        let on = Guards {
            max_open_loss_r: 2.0,
            max_notional_pct_equity: 300.0,
            flat_before_weekend_hhmm: 1655,
            news_flat_before_min: 60,
            news_flat_after_min: 30,
            max_trades_per_day: 4,
            daily_loss_limit_usd: 300.0,
            cooldown_ms: 30 * MINUTE,
            ..Guards::unbounded()
        };
        assert_eq!(
            on.describe(),
            "max_open_loss_r 2.0, notional 300%, weekend flat 16:55 NY, news flat 60/30 (impact≥3), daily cap 4, loss limit $300, cooldown 30 min"
        );
        assert!(Guards::unbounded().describe().starts_with("max_open_loss_r off, notional off, weekend flat off, news flat off"));
    }

    #[test]
    fn the_weekend_cutoff_is_read_on_the_new_york_clock() {
        let g = Guards { flat_before_weekend_hhmm: 1655, ..Guards::unbounded() };
        // 2026-09-11 is a Friday. 20:54 UTC in September is 16:54 New York.
        assert!(!g.past_weekend_cutoff(utc(2026, 9, 11, 20, 54)));
        assert!(g.past_weekend_cutoff(utc(2026, 9, 11, 20, 55)));
        assert!(g.past_weekend_cutoff(utc(2026, 9, 11, 21, 0)));
        // Saturday 00:05 New York is inside the weekend.
        assert!(g.past_weekend_cutoff(utc(2026, 9, 12, 4, 5)));
        // Thursday at the same minute, and Monday, are not.
        assert!(!g.past_weekend_cutoff(utc(2026, 9, 10, 20, 55)));
        assert!(!g.past_weekend_cutoff(utc(2026, 9, 14, 20, 55)));
        // Winter: 16:55 New York is 21:55 UTC.
        assert!(!g.past_weekend_cutoff(utc(2026, 1, 9, 21, 54)));
        assert!(g.past_weekend_cutoff(utc(2026, 1, 9, 21, 55)));
        // Off at zero.
        assert!(!Guards::unbounded().past_weekend_cutoff(utc(2026, 9, 11, 22, 0)));
    }

    #[test]
    fn the_open_loss_cap_fills_at_the_level_or_the_open_through_it() {
        let rules = TradingRules { spread: 0.0, ..TradingRules::default() };
        let g = Guards { max_open_loss_r: 2.0, ..Guards::unbounded() };
        let long = Exposure { side: Side::Long, entry_price: 100.0, risk: 5.0 };
        // Level at 90. A bar that touches it fills there.
        assert_eq!(guard_exit(&long, &bar(0, 95.0, 96.0, 89.0, 92.0), 60_000, &rules, &g), Some((90.0, ExitKind::Guard(GuardKind::OpenLoss))));
        // A bar that opens through it fills at the open.
        assert_eq!(guard_exit(&long, &bar(0, 85.0, 88.0, 84.0, 86.0), 60_000, &rules, &g), Some((85.0, ExitKind::Guard(GuardKind::OpenLoss))));
        // A bar that stays above it does nothing.
        assert_eq!(guard_exit(&long, &bar(0, 95.0, 96.0, 90.5, 92.0), 60_000, &rules, &g), None);
        // Mirrored for a short: level at 110.
        let short = Exposure { side: Side::Short, entry_price: 100.0, risk: 5.0 };
        assert_eq!(guard_exit(&short, &bar(0, 105.0, 111.0, 104.0, 106.0), 60_000, &rules, &g), Some((110.0, ExitKind::Guard(GuardKind::OpenLoss))));
        assert_eq!(guard_exit(&short, &bar(0, 112.0, 113.0, 111.0, 112.0), 60_000, &rules, &g), Some((112.0, ExitKind::Guard(GuardKind::OpenLoss))));
        // Exit costs are the engine's: half the spread against the trader.
        let costed = TradingRules { spread: 0.4, ..TradingRules::default() };
        assert_eq!(guard_exit(&long, &bar(0, 95.0, 96.0, 89.0, 92.0), 60_000, &costed, &g), Some((89.8, ExitKind::Guard(GuardKind::OpenLoss))));
        // Off at zero.
        assert_eq!(guard_exit(&long, &bar(0, 85.0, 88.0, 84.0, 86.0), 60_000, &rules, &Guards::unbounded()), None);
    }

    #[test]
    fn the_notional_cap_rounds_down_to_the_step_and_refuses_below_the_minimum() {
        let rules = TradingRules { contract_size: 100.0, lot_step: 0.01, min_lot: 0.01, ..TradingRules::default() };
        let g = Guards { max_notional_pct_equity: 300.0, ..Guards::unbounded() };
        // $10,000 × 300% = $30,000 of notional; at $2,000 an ounce, 100 oz a
        // lot, that is 0.15 lots.
        assert_eq!(g.cap_lots(0.10, 2000.0, 10_000.0, &rules), Ok((0.10, false)));
        let (lots, cut) = g.cap_lots(0.50, 2000.0, 10_000.0, &rules).unwrap();
        assert!(cut && (lots - 0.15).abs() < 1e-12, "{lots}");
        // 0.157 raw rounds down to 0.15, never up.
        let (lots, _) = g.cap_lots(1.0, 1910.0, 10_000.0, &rules).unwrap();
        assert!((lots - 0.15).abs() < 1e-12, "{lots}");
        // A cap under the minimum lot refuses the entry.
        let tiny = Guards { max_notional_pct_equity: 1.0, ..Guards::unbounded() };
        assert_eq!(tiny.cap_lots(0.10, 2000.0, 10_000.0, &rules), Err(Refusal::Notional));
        // Off at zero.
        assert_eq!(Guards::unbounded().cap_lots(50.0, 2000.0, 10_000.0, &rules), Ok((50.0, false)));
    }

    #[test]
    fn the_bar_interval_is_the_smallest_gap() {
        let b = |t: i64| bar(t, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(bar_interval_ms(&[b(0), b(15 * MINUTE), b(30 * MINUTE), b(3 * DAY_MS)]), 15 * MINUTE);
        assert_eq!(bar_interval_ms(&[b(0)]), 0);
        assert_eq!(bar_interval_ms(&[]), 0);
    }
}
