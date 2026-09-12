//! Bar-driven backtest engine.
//!
//! Every strategy runs through this one code path, so a leaderboard compares
//! methods rather than accidental differences in fill assumptions.
//!
//! The fill model, stated plainly because it decides more than most strategies
//! do:
//!
//! * A signal produced on bar `i` fills at the **open of bar `i+1`**, never at
//!   the close that produced it. Same-bar fills are the most common way a
//!   backtest invents money that was never available.
//! * Half the spread is charged on entry and half on exit; commission is per lot
//!   per side.
//! * Intrabar order is unknowable from OHLC, so when one bar's range covers both
//!   stop and target, the **stop** is taken.
//! * A gap through a level fills at the **open**, not at the level, so gapping
//!   through a stop loses more than 1R — as it does in life.
//! * With no structural stop and no ATR to derive one from, the trade is
//!   **refused** and counted, rather than sized on a guess.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_core::config::{Config, ConfigError};
use fd_indicators::{IndicatorSet, IndicatorSpec, Series, compute_indicators};
use fd_strategy::registry::{BarContext, Exits, Intent, OpenPosition, OptionsView, Params, Side, Strategy};
use serde::{Deserialize, Serialize};

use crate::context::OptionsTimeline;

/// Costs and sizing, shared by every strategy in a run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TradingRules {
    pub contract_size: f64,
    pub spread: f64,
    pub commission_per_lot: f64,
    pub starting_equity_usd: f64,
    pub risk_per_trade_pct: f64,
    /// ATR multiple used when a strategy gives no structural stop.
    pub stop_atr: f64,
    /// Target distance as a multiple of risk. Zero disables the default target.
    pub reward_risk: f64,
    pub max_hold_ms: i64,
    pub lot_step: f64,
    pub min_lot: f64,
    /// ATR period used when the strategy declares none.
    pub fallback_atr_period: usize,
}

impl Default for TradingRules {
    fn default() -> Self {
        Self {
            contract_size: 100.0,
            spread: 0.3,
            commission_per_lot: 0.0,
            starting_equity_usd: 10_000.0,
            risk_per_trade_pct: 0.01,
            stop_atr: 1.2,
            reward_risk: 1.8,
            max_hold_ms: 14_400_000,
            lot_step: 0.01,
            min_lot: 0.01,
            fallback_atr_period: 14,
        }
    }
}

/// A completed trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub direction: Side,
    pub entry_time: i64,
    pub entry_price: f64,
    pub exit_time: i64,
    pub exit_price: f64,
    /// Why the position closed, in words.
    ///
    /// For an engine exit this is a fixed label (`STOP`, `TARGET`, …). For a
    /// strategy exit it is the strategy's own sentence — "middle band reached",
    /// "opposite cross" — because that is what a reader needs when scanning a
    /// trade list, and a single `SIGNAL` label throws it away.
    pub exit_reason: String,
    /// The same thing as a value, for code that needs to branch on it.
    pub exit_kind: ExitKind,
    pub stop: f64,
    pub target: Option<f64>,
    pub lots: f64,
    pub pnl_usd: f64,
    /// Result in units of the risk taken.
    pub r: f64,
    /// Worst and best excursion, also in R.
    pub mae: f64,
    pub mfe: f64,
    pub hold_ms: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExitKind {
    Stop,
    Target,
    Timeout,
    /// The strategy asked to close; the wording lives in `Trade::exit_reason`.
    Signal,
    EndOfData,
}

impl ExitKind {
    /// The label an engine exit is recorded under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stop => "STOP",
            Self::Target => "TARGET",
            Self::Timeout => "TIMEOUT",
            Self::Signal => "SIGNAL",
            Self::EndOfData => "END_OF_DATA",
        }
    }
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestResult {
    pub strategy: String,
    pub params: Params,
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<(i64, f64)>,
    pub metrics: Metrics,
    pub bars: usize,
    pub warmup: usize,
    /// Entries refused because no risk unit could be established.
    pub skipped_no_atr: usize,
}

/// Restrict trading to a window. Indicators still warm up on earlier bars.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Range {
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl Range {
    #[must_use]
    pub fn contains(&self, time: i64) -> bool {
        self.from.is_none_or(|f| time >= f) && self.to.is_none_or(|t| time <= t)
    }
}

/// Position while it is open.
struct Live {
    side: Side,
    entry_time: i64,
    entry_price: f64,
    stop: Option<f64>,
    target: Option<f64>,
    lots: f64,
    risk: f64,
    reason: String,
    mae: f64,
    mfe: f64,
    self_managed: bool,
}

/// Trading rules for one market.
///
/// Contract size, spread and lot granularity come from the market, not from
/// the top-level `[trading]` table — that table describes gold. Reading it for
/// BTC prices a 1-BTC contract as 100 ounces and charges a $0.30 spread where
/// the real one is $5.00, which understates the cost of every trade by about
/// seventeen times and makes a backtest look better than the market allowed.
pub fn trading_rules_for(config: &Config, market: &str) -> Result<TradingRules, ConfigError> {
    let spec = config.market(market)?;
    Ok(TradingRules {
        contract_size: spec.trading.contract_size,
        spread: spec.trading.spread,
        lot_step: spec.trading.lot_step,
        min_lot: spec.trading.min_lot,
        // The rest is policy rather than venue convention, and is shared.
        commission_per_lot: config.trading.commission_per_lot,
        starting_equity_usd: config.trading.starting_equity_usd,
        risk_per_trade_pct: config.trading.risk_per_trade_pct,
        stop_atr: config.trading.stop_atr,
        reward_risk: config.trading.reward_risk,
        max_hold_ms: config.trading.max_hold_ms,
        fallback_atr_period: config.backtest.fallback_atr_period,
    })
}

/// Key of the series the engine sizes trades with.
///
/// The strategy's own ATR when it declared one, otherwise the configured
/// fallback — the engine still has to size a trade the strategy stopped by
/// some other means.
#[must_use]
pub fn sizing_atr_key(params: &Params, rules: &TradingRules) -> String {
    let declared = params.get("atrPeriod");
    let period = if declared.is_finite() && declared > 0.0 {
        declared.round() as usize
    } else {
        rules.fallback_atr_period
    };
    format!("atr_{period}.atr")
}

/// Add the sizing ATR to `set` if the strategy's own specs did not produce it.
pub fn ensure_fallback_atr(bars: &[Bar], params: &Params, rules: &TradingRules, set: &mut IndicatorSet) {
    let key = sizing_atr_key(params, rules);
    if set.contains_key(&key) {
        return;
    }
    let period = key.trim_start_matches("atr_").trim_end_matches(".atr").parse::<f64>().unwrap_or(0.0);
    let fallback = compute_indicators(bars, &[IndicatorSpec::new("atr").with("period", period)]).unwrap_or_default();
    for (k, series) in fallback {
        set.entry(k).or_insert(series);
    }
}

/// Run one strategy over one bar series.
pub fn run_backtest(
    bars: &[Bar],
    strategy: &dyn Strategy,
    params: &Params,
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    range: Range,
) -> BacktestResult {
    run_backtest_with(bars, strategy, params, rules, timeline, range, None)
}

/// The same run, optionally reading its indicator series from a set computed
/// elsewhere.
///
/// A sweep's cells share most of their series — every cell with `fast = 9`
/// wants the same `ema_9`, and every cell of every strategy wants the same
/// ATR — so a sweep computes the union once and hands it to each cell. The set
/// may be a superset of what this cell needs; a strategy only ever reads the
/// series it declared, so the extra entries change nothing.
pub fn run_backtest_with(
    bars: &[Bar],
    strategy: &dyn Strategy,
    params: &Params,
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    range: Range,
    shared: Option<&IndicatorSet>,
) -> BacktestResult {
    let mut owned;
    let ind: &IndicatorSet = match shared {
        Some(set) => set,
        None => {
            owned = compute_indicators(bars, &strategy.indicators(params)).unwrap_or_default();
            ensure_fallback_atr(bars, params, rules, &mut owned);
            &owned
        }
    };

    let atr_key = sizing_atr_key(params, rules);
    // Shared, not copied: this is the same series the strategy may also read.
    let atr_series: Series =
        ind.get(&atr_key).map_or_else(|| Series::from(vec![f64::NAN; bars.len()]), Series::clone);

    // Resolve the strategy's own series once. Doing it per bar means formatting
    // an indicator key and walking a string-keyed tree for every bar of every
    // parameter cell, which is most of what a sweep would spend its time on.
    let series_keys = strategy.series(params);
    let resolved: Vec<&[f64]> =
        series_keys.iter().map(|k| ind.get(k).map_or(&[][..], |s| &s[..])).collect();

    let warmup = strategy.warmup(params);
    let self_managed = strategy.exits() == Exits::Strategy;

    let mut trades: Vec<Trade> = Vec::new();
    // Grown lazily: a point is appended per trade, not per bar, so reserving a
    // bar's worth up front allocates more than the run will ever use.
    let mut equity_curve: Vec<(i64, f64)> = Vec::new();
    let mut equity = rules.starting_equity_usd;
    let mut position: Option<Live> = None;
    let mut pending: Option<Intent> = None;
    let mut skipped_no_atr = 0usize;
    // Forward cursor into the options timeline; see `OptionsTimeline::view_from`.
    let mut frame_cursor = 0usize;
    // A strategy that never reads the options frame should not pay to have one
    // built for every bar it sees.
    let wants_options = strategy.needs_options();

    for (i, bar) in bars.iter().enumerate() {
        let tradable = range.contains(bar.time);

        // 1. Fill whatever the previous bar decided, at this bar's open.
        if tradable && let Some(intent) = pending.take() {
            match intent {
                Intent::Enter { side, stop, target, reason } if position.is_none() => {
                    // The risk unit comes from the bar that produced the signal,
                    // not from the bar being filled on — the strategy could not
                    // have seen this bar's ATR when it decided.
                    //
                    // Only the very first bar has no predecessor. A NaN at
                    // `i - 1` means ATR is not warm yet and the entry is
                    // refused; it must NOT quietly fall through to this bar's
                    // value, which is how the port first drifted one bar ahead
                    // of the oracle.
                    let atr = match i.checked_sub(1) {
                        Some(previous) => atr_series.get(previous).copied(),
                        None => atr_series.get(i).copied(),
                    }
                    .filter(|v| v.is_finite());
                    match open_position(side, stop, target, reason, bar, atr, equity, rules, self_managed) {
                        Some(opened) => position = Some(opened),
                        None => skipped_no_atr += 1,
                    }
                }
                Intent::Exit { reason } if position.is_some() => {
                    let open = position.take().expect("checked");
                    let exit = apply_costs(bar.open, open.side, false, rules);
                    let entry_reason = open.reason.clone();
                    let trade =
                        close_position(open, exit, bar.time, ExitKind::Signal, &reason, rules, &entry_reason);
                    equity += trade.pnl_usd;
                    equity_curve.push((bar.time, round2(equity)));
                    trades.push(trade);
                }
                _ => {}
            }
        }

        // 2. Manage an open position against this bar's range.
        if let Some(open) = position.as_mut() {
            if let Some((price, kind)) = check_exit(open, bar, rules) {
                let open = position.take().expect("checked");
                let entry_reason = open.reason.clone();
                let trade = close_position(open, price, bar.time, kind, kind.label(), rules, &entry_reason);
                equity += trade.pnl_usd;
                equity_curve.push((bar.time, round2(equity)));
                trades.push(trade);
            } else {
                track_excursion(open, bar);
            }
        }

        // 3. Ask the strategy, using only what is known at this bar.
        if i >= warmup && tradable {
            let view =
                if wants_options { timeline.and_then(|t| t.view_from(&mut frame_cursor, bar.time)) } else { None };
            let ctx = BarContext {
                bar,
                i,
                bars,
                ind,
                series: &resolved,
                options: view.as_ref(),
                position: position.as_ref().map(|p| OpenPosition {
                    side: p.side,
                    entry_price: p.entry_price,
                    entry_time: p.entry_time,
                    stop: p.stop,
                    target: p.target,
                }),
                params,
            };
            match strategy.on_bar(&ctx) {
                Intent::None => {}
                intent => pending = Some(intent),
            }
        }
    }

    // Close anything still open, so the record carries no ghost trade.
    if let Some(open) = position.take()
        && let Some(last) = bars.last()
    {
        let exit = apply_costs(last.close, open.side, false, rules);
        let entry_reason = open.reason.clone();
        let trade = close_position(
            open,
            exit,
            last.time,
            ExitKind::EndOfData,
            ExitKind::EndOfData.label(),
            rules,
            &entry_reason,
        );
        equity += trade.pnl_usd;
        equity_curve.push((last.time, round2(equity)));
        trades.push(trade);
    }

    let metrics = metrics_of(&trades, rules.starting_equity_usd);
    BacktestResult {
        strategy: strategy.id().to_string(),
        params: params.clone(),
        trades,
        equity_curve,
        metrics,
        bars: bars.len(),
        warmup,
        skipped_no_atr,
    }
}

#[allow(clippy::too_many_arguments)]
fn open_position(
    side: Side,
    stop: Option<f64>,
    target: Option<f64>,
    reason: String,
    bar: &Bar,
    atr: Option<f64>,
    equity: f64,
    rules: &TradingRules,
    self_managed: bool,
) -> Option<Live> {
    let entry = apply_costs(bar.open, side, true, rules);

    let stop = match stop.filter(|s| s.is_finite()) {
        Some(explicit) => Some(explicit),
        None => {
            let atr = atr?; // no structural stop and no ATR: refuse the trade
            if self_managed {
                None
            } else if side.is_long() {
                Some(entry - atr * rules.stop_atr)
            } else {
                Some(entry + atr * rules.stop_atr)
            }
        }
    };

    // A self-managed position still needs a risk unit for sizing and for R.
    let risk = match stop {
        Some(s) => (entry - s).abs(),
        None => atr? * rules.stop_atr,
    };
    // Negated on purpose, and clippy is wrong to want `risk <= 0.0` here: a
    // NaN risk must refuse the trade, and every comparison with NaN is false.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(risk > 0.0) {
        return None;
    }

    let risk_usd = equity * rules.risk_per_trade_pct;
    let raw_lots = risk_usd / (risk * rules.contract_size);
    let lots = ((raw_lots / rules.lot_step).floor() * rules.lot_step).max(rules.min_lot);

    let target = match target.filter(|t| t.is_finite()) {
        Some(explicit) => Some(explicit),
        None if self_managed || stop.is_none() || rules.reward_risk == 0.0 => None,
        None => {
            let distance = risk * rules.reward_risk;
            Some(if side.is_long() { entry + distance } else { entry - distance })
        }
    };

    Some(Live {
        side,
        entry_time: bar.time,
        entry_price: entry,
        stop,
        target,
        lots,
        risk,
        reason,
        mae: 0.0,
        mfe: 0.0,
        self_managed,
    })
}

/// Stop first, then target — the pessimistic reading when a bar's range covers
/// both and OHLC cannot say which came first.
fn check_exit(position: &Live, bar: &Bar, rules: &TradingRules) -> Option<(f64, ExitKind)> {
    // A self-managed position has no engine stop, target or clock: the strategy
    // must issue its own exit, and the final bar closes whatever is left.
    if position.self_managed && position.stop.is_none() {
        return None;
    }
    let long = position.side.is_long();

    if let Some(stop) = position.stop {
        let hit = if long { bar.low <= stop } else { bar.high >= stop };
        if hit {
            let gapped = if long { bar.open <= stop } else { bar.open >= stop };
            let raw = if gapped { bar.open } else { stop };
            return Some((apply_costs(raw, position.side, false, rules), ExitKind::Stop));
        }
    }

    if let Some(target) = position.target.filter(|t| t.is_finite()) {
        let hit = if long { bar.high >= target } else { bar.low <= target };
        if hit {
            let gapped = if long { bar.open >= target } else { bar.open <= target };
            let raw = if gapped { bar.open } else { target };
            return Some((apply_costs(raw, position.side, false, rules), ExitKind::Target));
        }
    }

    if rules.max_hold_ms > 0 && bar.time - position.entry_time > rules.max_hold_ms {
        return Some((apply_costs(bar.close, position.side, false, rules), ExitKind::Timeout));
    }
    None
}

fn track_excursion(position: &mut Live, bar: &Bar) {
    let long = position.side.is_long();
    let best = if long { bar.high - position.entry_price } else { position.entry_price - bar.low };
    let worst = if long { bar.low - position.entry_price } else { position.entry_price - bar.high };
    position.mfe = position.mfe.max(best);
    position.mae = position.mae.min(worst);
}

/// Half the spread against the trader on every side.
fn apply_costs(price: f64, side: Side, entering: bool, rules: &TradingRules) -> f64 {
    let half = rules.spread / 2.0;
    let long = side.is_long();
    if entering {
        if long { price + half } else { price - half }
    } else if long {
        price - half
    } else {
        price + half
    }
}

fn close_position(
    position: Live,
    exit_price: f64,
    exit_time: i64,
    kind: ExitKind,
    exit_reason: &str,
    rules: &TradingRules,
    entry_reason: &str,
) -> Trade {
    let points =
        if position.side.is_long() { exit_price - position.entry_price } else { position.entry_price - exit_price };
    let commission = rules.commission_per_lot * position.lots * 2.0;
    let pnl = points * position.lots * rules.contract_size - commission;

    Trade {
        direction: position.side,
        entry_time: position.entry_time,
        entry_price: round2(position.entry_price),
        exit_time,
        exit_price: round2(exit_price),
        exit_reason: exit_reason.to_string(),
        exit_kind: kind,
        stop: round2(position.stop.unwrap_or(f64::NAN)),
        target: position.target.map(round2),
        lots: position.lots,
        pnl_usd: round2(pnl),
        r: round4(points / position.risk),
        mae: round4(position.mae / position.risk),
        mfe: round4(position.mfe / position.risk),
        hold_ms: exit_time - position.entry_time,
        reason: entry_reason.to_string(),
    }
}

/// Performance of a set of trades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub trades: usize,
    pub win_rate: f64,
    pub avg_r: f64,
    pub avg_win_r: f64,
    pub avg_loss_r: f64,
    pub profit_factor: f64,
    pub expectancy: f64,
    pub total_r: f64,
    pub net_pnl_usd: f64,
    pub return_pct: f64,
    pub max_drawdown_usd: f64,
    pub max_drawdown_pct: f64,
    pub sharpe: f64,
    pub avg_mae: f64,
    pub avg_mfe: f64,
    pub avg_hold_min: f64,
    pub exits: BTreeMap<String, usize>,
}

impl Metrics {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            trades: 0,
            win_rate: f64::NAN,
            avg_r: f64::NAN,
            avg_win_r: f64::NAN,
            avg_loss_r: f64::NAN,
            profit_factor: f64::NAN,
            expectancy: f64::NAN,
            total_r: 0.0,
            net_pnl_usd: 0.0,
            return_pct: 0.0,
            max_drawdown_usd: 0.0,
            max_drawdown_pct: f64::NAN,
            sharpe: f64::NAN,
            avg_mae: f64::NAN,
            avg_mfe: f64::NAN,
            avg_hold_min: f64::NAN,
            exits: BTreeMap::new(),
        }
    }
}

#[must_use]
pub fn metrics_of(trades: &[Trade], starting_equity: f64) -> Metrics {
    if trades.is_empty() {
        return Metrics::empty();
    }

    let rs: Vec<f64> = trades.iter().map(|t| t.r).collect();
    let wins: Vec<&Trade> = trades.iter().filter(|t| t.pnl_usd > 0.0).collect();
    let losses: Vec<&Trade> = trades.iter().filter(|t| t.pnl_usd <= 0.0).collect();
    let gross_win: f64 = wins.iter().map(|t| t.pnl_usd).sum();
    let gross_loss: f64 = losses.iter().map(|t| t.pnl_usd).sum::<f64>().abs();

    let mean = rs.iter().sum::<f64>() / rs.len() as f64;
    let variance = rs.iter().map(|r| (r - mean) * (r - mean)).sum::<f64>() / (rs.len().max(2) - 1) as f64;
    let sd = variance.sqrt();

    let mut equity = starting_equity;
    let mut peak = starting_equity;
    let mut max_dd = 0.0_f64;
    for trade in trades {
        equity += trade.pnl_usd;
        peak = peak.max(equity);
        max_dd = max_dd.max(peak - equity);
    }
    let net = equity - starting_equity;

    let mut exits: BTreeMap<String, usize> = BTreeMap::new();
    for trade in trades {
        *exits.entry(trade.exit_reason.clone()).or_insert(0) += 1;
    }

    Metrics {
        trades: trades.len(),
        win_rate: wins.len() as f64 / trades.len() as f64,
        avg_r: round4(mean),
        avg_win_r: if wins.is_empty() { f64::NAN } else { round4(wins.iter().map(|t| t.r).sum::<f64>() / wins.len() as f64) },
        avg_loss_r: if losses.is_empty() {
            f64::NAN
        } else {
            round4(losses.iter().map(|t| t.r).sum::<f64>() / losses.len() as f64)
        },
        profit_factor: if gross_loss > 0.0 { round4(gross_win / gross_loss) } else { f64::INFINITY },
        expectancy: round4(mean),
        total_r: round4(rs.iter().sum::<f64>()),
        net_pnl_usd: round2(net),
        return_pct: round4(net / starting_equity * 100.0),
        max_drawdown_usd: round2(max_dd),
        max_drawdown_pct: round4(max_dd / peak * 100.0),
        sharpe: if sd > 0.0 { round4(mean / sd * (rs.len() as f64).sqrt()) } else { f64::NAN },
        avg_mae: round4(trades.iter().map(|t| t.mae).sum::<f64>() / trades.len() as f64),
        avg_mfe: round4(trades.iter().map(|t| t.mfe).sum::<f64>() / trades.len() as f64),
        avg_hold_min: round2(trades.iter().map(|t| t.hold_ms as f64).sum::<f64>() / trades.len() as f64 / 60_000.0),
        exits,
    }
}

// The oracle serialises these with `Math.round(v * scale) / scale`, which
// rounds a half towards positive infinity. `f64::round` rounds away from zero
// and so disagrees on every negative half — and PnL, R and excursions are
// negative about as often as not.
fn round2(v: f64) -> f64 {
    fd_core::js_round_to(v, 2)
}

fn round4(v: f64) -> f64 {
    fd_core::js_round_to(v, 4)
}

/// Unused import guard: `OptionsView` is part of the public context shape.
const _: Option<fn(&OptionsView<'_>)> = None;
