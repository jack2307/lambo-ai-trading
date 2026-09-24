//! Search the hypothesis space on stored data.
//!
//! The whole point of the port, finally pointed at something. Reads bars (and,
//! where there is one, a tape) from the Parquet store, then reports three
//! different things that are easy to confuse:
//!
//! * **compare** — every method at its default parameters. A leaderboard, not
//!   a result.
//! * **sweep** — one method across its grid. Sensitivity, not a winner. The
//!   best cell of a sweep is in-sample and means nothing on its own; what the
//!   distribution says about the *median* cell is the honest reading.
//! * **walk-forward** — parameters chosen on a training window and measured on
//!   the next one, rolled forward. The only number worth quoting.
//!
//! The promising gate is applied to the walk-forward result and reported as it
//! falls. A failing gate is the answer, not a prompt to widen the grid.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin search -- --market=btc --mode=wf
//! ```
//!
//! `--guards` applies `[trading.guards]` to every run of every mode —
//! hypotheses, nulls, sweeps, cost curves, the leaderboard — and says so in
//! the header. Without it every number is unguarded, as every receipt before
//! 2026-09-14 was.

use std::collections::HashMap;
use std::path::PathBuf;

use fd_backtest::engine::{Range, TradingRules, run_backtest_guarded};
use fd_strategy::registry::Strategy as _;
use fd_backtest::sweep::{SelectBy, compare_strategies_guarded, sweep_strategy_guarded, verdict, walk_forward_guarded};
use fd_backtest::hypotheses::{batch as hypothesis_batch, batch_from_file, run_hypothesis_fixed_guarded, run_hypothesis_guarded};
use fd_backtest::timeline::{TimelineOptions, build_timeline};
use fd_backtest::{Guards, OptionsTimeline, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::{TapeStore, read_bars};
use fd_strategy::registry::Registry;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "btc");
    let mode = arg("mode", "all");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;

    let interval = arg("interval", &config.backtest.timeframe);
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    // WHICH STORE THIS RUN READ, in the receipt, before anything is measured.
    // Every other line here named a market or a symbol and left the directory
    // implicit, so a receipt could not be checked against the store it was
    // meant to come from - which is exactly what the sealed-store programme
    // asks a receipt to prove (`docs/hypotheses/2026-09-23-designed-methods.md`).
    //
    // Two agents added this line independently on the same day, one naming the
    // market and one naming the root. Both are kept: the root is what the audit
    // reads, the market is what a human looks for first, and dropping either to
    // settle a merge would lose something one of them was added for.
    println!("market:   {market} {interval}");
    println!("data:     {} (bars from {})", data.display(), bars_path.display());
    let mut bars = read_bars(&bars_path)?;
    // `--from=YYYY-MM-DD --to=YYYY-MM-DD` (UTC, `to` exclusive) cut the series
    // before anything runs: an out-of-sample feed that overlaps the in-sample
    // one is only out of sample on the dates the in-sample feed never saw.
    let bound = |name: &str| -> Option<i64> {
        let text = arg(name, "");
        if text.is_empty() {
            return None;
        }
        let mut parts = text.split('-').map(|p| p.parse::<i64>());
        let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
        Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000)
    };
    let (from, to) = (bound("from"), bound("to"));
    if from.is_some() || to.is_some() {
        let before = bars.len();
        bars.retain(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t));
        println!(
            "bounds:   --from={} --to={} kept {} of {before} bars",
            arg("from", "…"),
            arg("to", "…"),
            bars.len()
        );
    }
    if bars.is_empty() {
        println!("no bars at {} (after bounds) — run the backfill first", bars_path.display());
        return Ok(());
    }

    let mut rules = fd_backtest::engine::trading_rules_for(&config, &market)?;
    // `--trail=<distance_r>,<activate_r>` overrides `[trading.trail]` for this
    // run only, so a sweep over trail settings does not need a config edit per
    // cell — and cannot leave one behind. `--trail=off` forces it off whatever
    // the config says, which is what the control arm of such a sweep needs.
    if let Some(spec) = std::env::args().find_map(|a| a.strip_prefix("--trail=").map(str::to_string)) {
        if spec == "off" {
            rules.trail.enabled = false;
        } else {
            let mut parts = spec.split(',');
            let d: f64 = parts.next().unwrap_or("").trim().parse()
                .map_err(|_| format!("--trail wants <distance_r>,<activate_r> or `off`, got `{spec}`"))?;
            let a: f64 = parts.next().unwrap_or("").trim().parse()
                .map_err(|_| format!("--trail wants <distance_r>,<activate_r> or `off`, got `{spec}`"))?;
            rules.trail.enabled = true;
            rules.trail.distance_r = d;
            rules.trail.activate_r = a;
        }
    }
    // `--spread=<price units>` reprices the round trip for this run only. The
    // configured 0.28 for gold was a SINGLE read of the terminal; the logger
    // has since sampled thousands and the p50 is 0.220 with a maximum of 0.260,
    // so every receipt in docs/ is charged a cost above anything ever observed.
    // Repricing is a recorded amendment, never a config edit, so this flag
    // exists to measure what the amendment would be worth before anyone makes
    // one.
    if let Some(v) = std::env::args().find_map(|a| a.strip_prefix("--spread=").map(str::to_string)) {
        let s: f64 = v.trim().parse().map_err(|_| format!("--spread wants a number, got `{v}`"))?;
        rules.spread = s;
    }
    println!("spread:   {} per round trip", rules.spread);
    println!(
        "trail:    {}",
        if rules.trail.enabled {
            format!("ON distance {}R activate {}R", rules.trail.distance_r, rules.trail.activate_r)
        } else {
            "off".to_string()
        }
    );
    let rules = rules;
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by =
        if config.backtest.select_by == "profitFactor" { SelectBy::ProfitFactor } else { SelectBy::Expectancy };

    let timeline = load_timeline(&data, &market, &config, &bars);
    describe(&bars, timeline.as_ref());
    let prints = spec
        .tape_id()
        .and_then(|tape| fd_store::TapeStore::open(&data, tape).and_then(|s| s.all()).ok())
        .map_or(0, |t| t.len());
    describe_tape(timeline.as_ref(), prints);
    // The calendar behind every `news:` filter, installed once for the
    // process. Printed here and again beside the swap/spread line of a
    // hypotheses receipt so a record can quote which calendar it ran on.
    println!("{}", load_news(&data));
    println!("{}", news_scope_line(&rules));
    // `--companion=<SYMBOL>`: the SECOND instrument, for the one method that
    // reads two. Installed once for the process, at the same interval and out
    // of the same store as the primary, and printed here so a receipt says
    // which series it was — a companion-reading method with none installed
    // takes no trades, and the two cases must not look alike.
    println!("{}", load_companion(&data, &interval));
    // `--guards`: the configured risk guards on every run of every mode.
    // Printed right under the calendar so a receipt's header says whether
    // its numbers were bounded, and by what.
    let guards = std::env::args().any(|a| a == "--guards").then(|| Guards::for_market(&config, &market)).transpose()?;
    let guards = guards.as_ref();
    println!("{}", guards_line(guards));
    println!();

    let registry = Registry::with_builtins();
    if mode == "all" || mode == "compare" {
        run_compare(&registry, &bars, &rules, timeline.as_ref(), &gate, guards);
    }
    if mode == "all" || mode == "sweep" {
        run_sweeps(&registry, &bars, &rules, timeline.as_ref(), config.backtest.min_trades_per_cell, guards);
    }
    if mode == "null-dir" {
        run_direction_null(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            arg("strategy", "maxpain-magnet"),
            arg("samples", "2000").parse().unwrap_or(2000),
            guards,
        );
    }
    if mode == "volume" {
        run_volume(&registry, &bars, &rules, timeline.as_ref(), guards);
    }
    if mode == "hypotheses" {
        run_hypotheses(
            &registry,
            &bars,
            &rules,
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            &arg("batch", "gold-intraday"),
            arg("batch-file", "").as_str(),
            std::env::args().any(|a| a == "--fixed"),
            guards,
            // `--null-registered-stop` also reads every row against the
            // control the record used before 2026-09-24 — `RandomEntry`'s own
            // 1.5 ATR whatever the method stopped at — and prints the two
            // percentiles beside each other. It is how a corrected receipt
            // carries its original
            // (`docs/hypotheses/2026-09-24-cost-matched-null.md`), not a way
            // to measure anything new: it doubles the null runs and the
            // second column is a reproduction, not a reading.
            std::env::args().any(|a| a == "--null-registered-stop"),
        );
    }
    if mode == "null" {
        run_null_control(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            guards,
        );
    }
    if mode == "rescore" {
        // The rate the desk actually has, from the same file the live side
        // reads. `--rebate-share=` overrides it for a sensitivity run and
        // says so in the header; it is never a config edit.
        let share = arg("rebate-share", "");
        let rebate = if share.is_empty() {
            fd_backtest::Rebate::from_config_dir(std::path::Path::new(&arg("config", "config")))
        } else {
            share.trim().parse::<f64>().ok().and_then(fd_backtest::Rebate::new)
        };
        run_rescore(
            &registry,
            &bars,
            &rules,
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            arg("direction-samples", "1000").parse().unwrap_or(1000),
            arg("batch-file", "").as_str(),
            rebate,
            guards,
        );
    }
    if mode == "costs" {
        run_cost_sensitivity(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            guards,
        );
    }
    if mode == "all" || mode == "wf" {
        run_walk_forward(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            guards,
        );
    }
    Ok(())
}

/// The header line that says which calendar currencies this market's
/// `news:` filters and news guard read — `[markets.<id>.trading]
/// news_currencies`, empty meaning every currency.
fn news_scope_line(rules: &TradingRules) -> String {
    if rules.news_currencies.is_empty() {
        "news scope: every currency (news_currencies is empty)".to_string()
    } else {
        format!("news scope: {} (news_currencies)", rules.news_currencies.join("|"))
    }
}

/// The header line that says whether the run was bounded, and by what.
fn guards_line(guards: Option<&Guards>) -> String {
    match guards {
        Some(g) => format!("guards: on — {}", g.describe()),
        None => "guards: off (every number unguarded)".to_string(),
    }
}

/// Build the options timeline from whatever tape the store holds.
///
/// Returns `None` when there is no tape, which is not an error: the technical
/// strategies do not need one, and saying so beats inventing empty frames that
/// would make every options guard read as "no levels" rather than "no data".
fn load_timeline(
    data: &std::path::Path,
    market: &str,
    config: &Config,
    bars: &[Bar],
) -> Option<OptionsTimeline> {
    let tape = config.market(market).ok()?.tape_id()?.to_string();
    let store = TapeStore::open(data, &tape).ok()?;
    let trades = store.all().ok()?;
    if trades.is_empty() {
        return None;
    }
    let settings = fd_engine::engine::EngineSettings::from_config(config, market).ok()?;
    let timeline = build_timeline(
        &trades,
        &settings,
        &HashMap::new(),
        &TimelineOptions {
            step_ms: config.backtest.options_step_ms,
            big_trade_window_ms: config.ai.big_trade_window_ms,
        },
    );
    let _ = bars;
    (!timeline.is_empty()).then_some(timeline)
}

/// The calendar path `load_news` actually read, for the receipt lines printed
/// deeper in, which do not have `--data` in scope.
static NEWS_SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The `news:` receipt line, naming the file this process read.
fn news_line() -> String {
    fd_strategy::news::summary(NEWS_SOURCE.get().map_or("data/news/events.parquet", String::as_str))
}

/// Install `<data>/news/events.parquet` for the `news:` filters, if it is
/// there, and say what happened in one line. A missing file is not an error
/// — the filters are then no-ops, and the line says so; an unreadable one is
/// reported, not fatal, for the same reason.
///
/// The line names the path actually read. It used to name a `const` reading
/// `data/news/events.parquet` whatever `--data` said, so a receipt from
/// `--data=data-sealed` claimed a calendar out of `data/` — a claim about the
/// store that was not true, in the one line a reader would check.
fn load_news(data: &std::path::Path) -> String {
    let path = data.join("news").join("events.parquet");
    if path.is_file() {
        match fd_store::read_news(&path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            Ok(_) => {}
            Err(e) => println!("news: could not load {}: {e}", path.display()),
        }
    }
    let _ = NEWS_SOURCE.set(path.display().to_string());
    news_line()
}

/// Install `--companion=<SYMBOL>` from `<data>/bars/<SYMBOL>-<interval>.parquet`
/// and say what happened in one line.
///
/// No flag is not an error — every `cmp` series is then NaN and the line says
/// so. A named symbol that cannot be read IS reported, and loudly: a method
/// that reads two series and silently gets one would produce a run of zero
/// trades that looks like a run that found no signals.
fn load_companion(data: &std::path::Path, interval: &str) -> String {
    let symbol = arg("companion", "");
    if symbol.is_empty() {
        return fd_indicators::companion::summary();
    }
    let path = data.join("bars").join(format!("{symbol}-{interval}.parquet"));
    match read_bars(&path) {
        Err(e) => format!("companion: could not read {}: {e} — every cmp series is NaN", path.display()),
        Ok(bars) => {
            let companion = fd_indicators::companion::Companion::new(&symbol, interval, bars);
            match fd_indicators::companion::install(companion) {
                Ok(_) => format!("{} from {}", fd_indicators::companion::summary(), path.display()),
                Err(e) => format!("companion: {e}"),
            }
        }
    }
}

fn describe(bars: &[Bar], timeline: Option<&OptionsTimeline>) {
    println!("bars:     {} from {} to {}", bars.len(), iso(bars[0].time), iso(bars[bars.len() - 1].time));
    match timeline {
        Some(t) if !t.is_empty() => {
            let frames = t.frames();
            println!(
                "timeline: {} frames from {} to {}",
                frames.len(),
                iso(frames[0].t),
                iso(frames[frames.len() - 1].t)
            );
            // The mismatch that matters: options strategies can only trade where
            // the tape reaches, however many bars there are.
            let covered = bars.iter().filter(|b| b.time >= frames[0].t && b.time <= frames[frames.len() - 1].t).count();
            println!(
                "          the tape covers {covered} of {} bars ({:.1}%)",
                bars.len(),
                100.0 * covered as f64 / bars.len() as f64
            );
        }
        _ => println!("timeline: none — options strategies will be skipped"),
    }
    println!();
}

/// State of the tape behind a run.
///
/// Printed because the collector writes into the same store this reads from, so
/// two runs an hour apart measure different samples and report different
/// numbers for the same strategy. That happened — a profit factor moved from
/// 2.949 to 3.298 on the same nine trades — and without this line there is
/// nothing in the output to attribute the difference to.
fn describe_tape(timeline: Option<&OptionsTimeline>, prints: usize) {
    let frames = timeline.map_or(0, |t| t.len());
    println!("tape:     {prints} prints, {frames} frames  <- a result is only comparable to another run over the same tape");
    println!();
}

fn run_compare(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) {
    println!("== every method at its default parameters ==");
    println!("{:<20} {:>7} {:>8} {:>9} {:>8} {:>11}", "strategy", "trades", "win%", "profit", "expect", "maxDD$");
    for row in compare_strategies_guarded(registry, bars, rules, timeline, gate, guards) {
        match (row.skipped, row.metrics) {
            (Some(reason), _) => println!("{:<20} {reason}", row.id),
            (None, Some(m)) => println!(
                "{:<20} {:>7} {:>7.1}% {:>9.3} {:>8.3} {:>11.0}",
                row.id, m.trades, m.win_rate * 100.0, m.profit_factor, m.expectancy, m.max_drawdown_usd
            ),
            _ => println!("{:<20} no result", row.id),
        }
    }
    println!();
}

fn run_sweeps(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) {
    println!("== parameter sweeps (in-sample; read the median, not the best) ==");
    println!("{:<20} {:>6} {:>7} {:>12} {:>12} {:>11}", "strategy", "cells", "usable", "medianPF", "medianExp", "profitable");
    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let result = sweep_strategy_guarded(strategy.as_ref(), bars, rules, timeline, min_trades_per_cell, guards);
        let s = &result.summary;
        println!(
            "{:<20} {:>6} {:>7} {:>12.3} {:>12.3} {:>10.0}%",
            result.strategy,
            s.cells,
            s.usable_cells,
            s.median_profit_factor,
            s.median_expectancy,
            s.profitable_share * 100.0
        );
    }
    println!();
}

#[allow(clippy::too_many_arguments)]
fn run_walk_forward(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) {
    println!("== walk-forward, out of sample ({folds} folds) ==");
    println!("{:<20} {:>7} {:>8} {:>9} {:>8} {:>9}  verdict", "strategy", "trades", "win%", "profit", "expect", "maxDD");

    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let Some(result) =
            walk_forward_guarded(strategy.as_ref(), bars, rules, timeline, folds, select_by, min_trades_per_cell, guards)
        else {
            println!("{:<20} not enough bars", strategy.id());
            continue;
        };
        let m = &result.oos;
        let v = verdict(m, gate);
        println!(
            "{:<20} {:>7} {:>7.1}% {:>9.3} {:>8.3} {:>11.0}  {}",
            strategy.id(),
            m.trades,
            m.win_rate * 100.0,
            m.profit_factor,
            m.expectancy,
            m.max_drawdown_usd,
            if v.promising { "PASS".to_string() } else { format!("fail: {}", v.reasons.join("; ")) }
        );
    }
    println!();
    println!("A failing gate is the result. Widening the grid until something passes");
    println!("is how a backtest stops measuring the market and starts measuring the search.");
}

/// A null matched to the result it is judging.
///
/// The random-entry control answers "what does this pipeline produce from
/// noise", and its runs average hundreds of trades. That makes it the wrong
/// yardstick for a strategy with nine: a profit factor over nine trades is
/// enormously more dispersed than one over eight hundred, so comparing them
/// makes a lucky handful look extraordinary.
///
/// This control holds everything fixed except the one thing under test. Same
/// entry bars, same stop and target *distances*, same costs, same exit rules —
/// only the direction is a coin flip, mirrored around the entry price so the
/// geometry is preserved. The resulting distribution has exactly the same trade
/// count as the result, which is what makes the percentile mean something.
#[allow(clippy::too_many_arguments)]
fn run_direction_null(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    id: String,
    samples: usize,
    guards: Option<&Guards>,
) {
    use rayon::prelude::*;

    let Ok(strategy) = registry.get(&id) else {
        println!("unknown strategy: {id}");
        return;
    };
    if strategy.needs_options() && timeline.is_none() {
        println!("{id} needs an options timeline and the store has no tape");
        return;
    }
    // `--params=key=value,key=value` applies a preset, so the control can be
    // run on the hypothesis actually tested rather than on the method's
    // defaults.
    let mut params = strategy.default_params();
    for pair in arg("params", "").split(',').filter(|s| !s.is_empty()) {
        match pair.split_once('=').and_then(|(k, v)| v.parse::<f64>().ok().map(|v| (k, v))) {
            Some((key, value)) if params.contains(key) => params.set(key, value),
            _ => {
                println!("bad or unknown --params entry `{pair}` for {id}");
                return;
            }
        }
    }
    // `--filters=weekdays;hours:0800-1200;vol:14/100:1.2-99` — the batch's own
    // gates, so a filtered variant's control is its own and not its sibling's.
    let filters: Vec<fd_strategy::filter::Filter> = match arg("filters", "")
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| fd_strategy::filter::Filter::parse_for_market(s, &rules.news_currencies))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(f) => f,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    let gated = fd_strategy::filter::Filtered { inner: strategy, filters: filters.clone() };
    let actual = run_backtest_guarded(bars, &gated, &params, rules, guards, timeline, Range::default(), None);
    if actual.trades.is_empty() {
        println!("{id} took no trades; there is nothing to compare");
        return;
    }
    if !filters.is_empty() {
        println!("  filters: {}", gated.describe());
    }

    // A strategy that manages its own exits decides them from the side it
    // holds — a momentum rule flips when the position disagrees with the
    // sign — so replaying it with a coin-flip side keeps the method's exit
    // rule inside the null (the tsmom-2 pass read a null median of 1.56).
    // For those, the null is the method's own trades with their sides
    // permuted: same intervals, same lots, same costs, only the side.
    let self_managed = strategy.exits() == fd_strategy::registry::Exits::Strategy;
    let mut curve: Vec<f64> = (0..samples)
        .into_par_iter()
        .map(|seed| {
            if self_managed {
                return permuted_sides_pf(&actual.trades, rules, seed as u64 + 1);
            }
            let flipped = fd_backtest::DirectionFlipped { inner: strategy, seed: seed as u64 + 1 };
            let gated = fd_strategy::filter::Filtered { inner: &flipped, filters: filters.clone() };
            run_backtest_guarded(bars, &gated, &params, rules, guards, timeline, Range::default(), None)
                .metrics
                .profit_factor
        })
        .filter(|pf| pf.is_finite())
        .collect();
    curve.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let quantile = |q: f64| curve[(((curve.len() - 1) as f64) * q).round() as usize];
    let pf = actual.metrics.profit_factor;
    let below = curve.iter().filter(|value| **value < pf).count();
    let percentile = 100.0 * below as f64 / curve.len() as f64;

    println!("== direction control: {id}, {} trades, {samples} coin-flip assignments ==", actual.trades.len());
    if !arg("params", "").is_empty() {
        println!("  preset: {}", arg("params", ""));
    }
    if self_managed {
        println!("  the method's own trades — entry, exit, lots, costs — with each side a coin flip");
    } else {
        println!("  entries, stops and targets held fixed; only the side is randomised");
    }
    println!("  profit factor of the same trades with random direction:");
    println!("    p05 {:.3}   p25 {:.3}   p50 {:.3}   p75 {:.3}   p95 {:.3}   max {:.3}",
        quantile(0.05), quantile(0.25), quantile(0.50), quantile(0.75), quantile(0.95), curve[curve.len() - 1]);
    println!();
    println!("  actual: {pf:.3}  ->  {percentile:.0}th percentile  (p = {:.3})", 1.0 - percentile / 100.0);
    println!();
    if percentile >= 95.0 {
        println!("  Outside its own null. That is a finding worth taking further.");
    } else {
        println!("  Inside its own null. On this sample the direction rule has not been");
        println!("  distinguished from a coin flip, however good the profit factor looks.");
    }
    println!();
}

/// The profit factor of `trades` with each side re-drawn by coin flip.
///
/// Moved into `fd_backtest::direction` on 2026-09-23 so the rebate rescore
/// reads the same control this mode does rather than a second copy of it. The
/// arithmetic did not change.
fn permuted_sides_pf(trades: &[fd_backtest::Trade], rules: &TradingRules, seed: u64) -> f64 {
    fd_backtest::profit_factor_of(&fd_backtest::permuted_sides_pnls(trades, rules, seed))
}

/// What this pipeline produces when it is fed no signal.
///
/// Runs the random-entry control through the identical walk-forward — the same
/// grid size, the same parameter selection on the training window, the same
/// costs — once per seed, and reports the distribution of out-of-sample profit
/// factors.
///
/// That distribution is the thing to read a real result against. If a coin flip
/// clears 1.1 a quarter of the time on this tape, a method that scores 1.1 has
/// shown nothing, and the gate needs to sit above the noise rather than at a
/// number that sounded sensible.
#[allow(clippy::too_many_arguments)]
fn run_null_control(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
) {
    use rayon::prelude::*;

    println!("== control: {seeds} runs of random entry through the same pipeline ==");

    let mut results: Vec<(f64, f64, usize)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let control = fd_backtest::RandomEntry;
            // The seed is fixed per run and the grid varies the rest, so each
            // run is one complete pass of the machinery over a different world.
            let mut defaults = control.default_params();
            defaults.set("seed", seed as f64 + 1.0);
            let result = walk_forward_seeded(
                &control,
                &defaults,
                bars,
                rules,
                folds,
                select_by,
                min_trades_per_cell,
                guards,
            )?;
            Some((result.oos.profit_factor, result.oos.expectancy, result.oos.trades))
        })
        .collect();

    if results.is_empty() {
        println!("no control run produced a trade; nothing to compare against");
        return;
    }
    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let pf = |q: f64| {
        let index = ((results.len() - 1) as f64 * q).round() as usize;
        results[index].0
    };
    let median_trades = results[results.len() / 2].2;
    let passing = results.iter().filter(|(p, e, t)| *t >= gate.min_trades && *p >= gate.min_profit_factor && *e >= gate.min_expectancy_r).count();

    println!("  runs that produced trades: {} of {seeds}", results.len());
    println!("  median trades per run:     {median_trades}");
    println!();
    println!("  out-of-sample profit factor of pure noise:");
    println!("    p05  {:.3}", pf(0.05));
    println!("    p25  {:.3}", pf(0.25));
    println!("    p50  {:.3}", pf(0.50));
    println!("    p75  {:.3}", pf(0.75));
    println!("    p95  {:.3}", pf(0.95));
    println!("    max  {:.3}", results[results.len() - 1].0);
    println!();
    println!(
        "  {passing} of {} control runs ({:.1}%) clear the gate (PF >= {:.2}, exp >= {:.2}R, >= {} trades)",
        results.len(),
        100.0 * passing as f64 / results.len() as f64,
        gate.min_profit_factor,
        gate.min_expectancy_r,
        gate.min_trades
    );
    println!();
    println!("  That percentage is the gate's false-positive rate on this tape. A method");
    println!("  scoring inside this distribution has not been distinguished from chance.");
    println!();

    // Where each real method falls inside the noise. This is the comparison the
    // whole control exists for; a profit factor on its own does not say whether
    // a method beat the market or merely won a search.
    let curve: Vec<f64> = results.iter().map(|(pf, _, _)| *pf).collect();
    println!("  where each method falls inside that distribution:");
    println!("  {:<20} {:>10} {:>12}", "strategy", "OOS PF", "percentile");
    // Options strategies are placed inside the same noise: the control never
    // reads the tape, so its distribution is the one they have to beat too.
    // They are skipped only when there is no tape to run them on. Bound the
    // bars to the tape's span with --from/--to for a fair count.
    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let Some(result) =
            walk_forward_guarded(strategy.as_ref(), bars, rules, timeline, folds, select_by, min_trades_per_cell, guards)
        else {
            continue;
        };
        let pf = result.oos.profit_factor;
        if !pf.is_finite() {
            continue;
        }
        let below = curve.iter().filter(|value| **value < pf).count();
        let percentile = 100.0 * below as f64 / curve.len() as f64;
        let verdict = if percentile >= 95.0 { "outside the noise" } else { "inside the noise" };
        println!("  {:<20} {pf:>10.3} {percentile:>11.0}%  {verdict}", strategy.id());
    }
    println!();
    println!("  A method at the 80th percentile of a coin flip is what picking the best");
    println!("  of several coin flips looks like. Only the ones outside have said anything.");
    println!();
}

/// The rebate question, asked of the machine that can answer it.
///
/// An introducing broker is paid per lot, so a bot that trades often is worth
/// something to the broker even with no edge — *as long as the client's
/// account survives*. The number that decides that is what the client loses
/// per lot after costs. This prints, for every method at its defaults, for its
/// intraday version (flat over the break, so no swap), and for the random-entry
/// control, the trades it makes per year and what one lot of it costs the
/// client on average — both lot-weighted (what the venue sees) and per trade at
/// a fixed lot (what a client running fixed size feels). The last column is
/// the rebate per lot at which that client breaks even. Compare it with the
/// actual rebate; nothing else here is a judgement.
fn run_volume(registry: &Registry, bars: &[Bar], rules: &TradingRules, timeline: Option<&OptionsTimeline>, guards: Option<&Guards>) {
    use fd_strategy::filter::{Filter, Filtered};
    let run = |s: &dyn fd_strategy::registry::Strategy, p: &fd_strategy::registry::Params| {
        run_backtest_guarded(bars, s, p, rules, guards, timeline, Range::default(), None)
    };

    let span_years = (bars[bars.len() - 1].time - bars[0].time) as f64 / (365.25 * 86_400_000.0);
    let round_trip = rules.spread * rules.contract_size + 2.0 * rules.commission_per_lot;
    println!("== volume economics: what one lot costs the client, and the rebate that would cover it ==");
    println!(
        "contract {} | spread {} + commission {:.2}/side = {:.2} USD per lot round trip | swap {:.2}/{:.2} per lot-night | {span_years:.2} years",
        rules.contract_size, rules.spread, rules.commission_per_lot, round_trip, rules.swap_long_per_lot, rules.swap_short_per_lot
    );
    println!("all figures per lot of this contract; on a 100 oz standard lot multiply gold by 100");
    println!();
    println!(
        "{:<28} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>11}",
        "strategy", "trades", "trades/yr", "swap/lot", "net/lot", "ex-swap", "fixed-lot", "breakeven$"
    );

    let intraday = || vec![Filter::weekdays(), Filter::flat(1630, 1815)];
    let mut rows: Vec<(String, fd_backtest::BacktestResult)> = Vec::new();
    for s in registry.all() {
        if s.needs_options() && timeline.is_none() || s.id() == "buy-and-hold" {
            continue;
        }
        let p = s.default_params();
        rows.push((s.id().to_string(), run(s.as_ref(), &p)));
        let flat = Filtered { inner: s.as_ref(), filters: intraday() };
        rows.push((format!("{}/intraday", s.id()), run(&flat, &p)));
    }
    let control = fd_backtest::RandomEntry;
    let mut p = control.default_params();
    p.set("entryRate", 0.1);
    p.set("seed", 7.0);
    rows.push(("null-random".to_string(), run(&control, &p)));
    let flat = Filtered { inner: &control, filters: intraday() };
    rows.push(("null-random/intraday".to_string(), run(&flat, &p)));

    for (id, result) in rows {
        let trades = result.trades.len();
        if trades == 0 {
            println!("{id:<28} no trades");
            continue;
        }
        let lots: f64 = result.trades.iter().map(|t| t.lots).sum();
        let swap: f64 = result.trades.iter().map(|t| t.swap_usd).sum();
        let net: f64 = result.trades.iter().map(|t| t.pnl_usd).sum();
        // Lot-weighted: the venue's view of a dollar per lot turned over.
        let (swap_per_lot, net_per_lot) = (swap / lots, net / lots);
        // Fixed-lot: the same trades at one lot each, which is what a client
        // running a fixed size experiences. Sizing by risk weights tight-stop
        // trades more heavily and the two can disagree.
        let fixed: f64 = result.trades.iter().map(|t| t.pnl_usd / t.lots).sum::<f64>() / trades as f64;
        println!(
            "{id:<28} {trades:>7} {:>9.0} {:>9.2} {:>9.2} {:>9.2} {:>10.2} {:>11.2}",
            trades as f64 / span_years,
            swap_per_lot,
            net_per_lot,
            net_per_lot - swap_per_lot,
            fixed,
            -fixed
        );
    }
    println!();
    println!("net/lot is what a lot of this bot cost the client, all in; ex-swap is the same with the");
    println!("financing removed, i.e. what the intraday version could at best reach; fixed-lot is per");
    println!("trade at one lot. breakeven$ is the rebate per lot that leaves a fixed-lot client flat.");
    println!();
}

/// A declared batch of hypotheses, each read against its matched null.
#[allow(clippy::too_many_arguments)]
fn run_hypotheses(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    batch_name: &str,
    batch_file: &str,
    fixed: bool,
    guards: Option<&Guards>,
    also_registered_stop: bool,
) {
    let (batch, shown) = if batch_file.is_empty() {
        match hypothesis_batch(batch_name) {
            Some(b) => (b, batch_name.to_string()),
            None => {
                println!("unknown batch `{batch_name}` (have: gold-intraday, ict-m1, ict-m5, ict-oos; or --batch-file=<toml>)");
                return;
            }
        }
    } else {
        match batch_from_file(std::path::Path::new(batch_file)) {
            Ok(b) => (b, batch_file.to_string()),
            Err(e) => {
                println!("{e}");
                return;
            }
        }
    };
    if fixed {
        println!("== hypotheses `{shown}`: {} declared, FIXED parameters over the whole window (no selection), each against {seeds} matched null runs ==", batch.len());
    } else {
        println!("== hypotheses `{shown}`: {} declared, walk-forward ({folds} folds), each against {seeds} matched null runs ==", batch.len());
    }
    println!("swap: long {:.2} / short {:.2} USD per lot per night; spread {}", rules.swap_long_per_lot, rules.swap_short_per_lot, rules.spread);
    println!("{}", news_line());
    println!("{}", news_scope_line(rules));
    println!("{}", guards_line(guards));
    println!();
    println!(
        "{:<12} {:<18} {:>6} {:>7} {:>7} {:>8} {:>8} {:>8} {:>5}  verdict",
        "hypothesis", "base", "trades", "OOS PF", "expect", "null p50", "null p95", "swap$", "pct"
    );
    let mut survivors = Vec::new();
    for hypothesis in &batch {
        let outcome = if fixed {
            run_hypothesis_fixed_guarded(registry, hypothesis, bars, rules, gate, seeds, guards).map(Some)
        } else {
            run_hypothesis_guarded(registry, hypothesis, bars, rules, folds, select_by, min_trades_per_cell, gate, seeds, guards)
        };
        let report = match outcome {
            Ok(Some(report)) => report,
            Ok(None) => {
                println!("{:<12} {:<18} not enough bars", hypothesis.label, hypothesis.base);
                continue;
            }
            Err(e) => {
                println!("{:<12} {:<18} refused: {e}", hypothesis.label, hypothesis.base);
                continue;
            }
        };
        let m = &report.oos;
        let verdict = if report.survives() {
            "SURVIVES".to_string()
        } else if report.verdict.promising {
            "gate pass, inside the noise".to_string()
        } else {
            format!("fail: {}", report.verdict.reasons.join("; "))
        };
        println!(
            "{:<12} {:<18} {:>6} {:>7.3} {:>7.3} {:>8.3} {:>8.3} {:>8.0} {:>4.0}%  {verdict}",
            report.label,
            report.base,
            m.trades,
            m.profit_factor,
            m.expectancy,
            report.null_quantile(0.5),
            report.null_quantile(0.95),
            report.swap_usd,
            report.percentile
        );
        println!("{:<12} {:<18} {}  — {}", "", "", report.filters, report.why);
        // What the count-matching ACHIEVED, printed on every row rather than
        // assumed: the control's median out-of-sample trade count against the
        // method's. The registration of 2026-09-23 calls anything outside a
        // quarter either way a null that is not this method's size, and a
        // percentile read against such a null is not this method's percentile.
        println!(
            "{:<12} {:<18} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
            "",
            "",
            fd_backtest::hypotheses::median_count(&report.null_trades),
            m.trades,
            report.count_match(),
            if report.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" },
        );
        // And what the COST matching achieved, printed the same way and for
        // the same reason: cost as a fraction of risk is `spread / stop`, so
        // a control at a different stop is a control at a different cost, and
        // a method that merely widens its stop clears such a control without
        // predicting anything (`docs/hypotheses/2026-09-24-cost-matched-null.md`).
        if let Some(stop) = &report.control_stop {
            println!(
                "{:<12} {:<18} cost-matched null: control stop {:.3} ATR = {:.2} points, cost {:.2}% of R ({}){}",
                "",
                "",
                stop.atr,
                stop.points(),
                100.0 * stop.cost_fraction_of_r(rules),
                stop.source.as_str(),
                match stop.named {
                    Some(v) if stop.source == fd_backtest::hypotheses::StopSource::Realised =>
                        format!("; the method DECLARES stopAtr {v:.2} and does not use it"),
                    _ => String::new(),
                },
            );
            println!(
                "{:<12} {:<18} the method's own realised stop: median {:.3} ATR = {:.2} points over {} trades",
                "", "", stop.realised_atr, stop.realised_points, stop.measured_trades,
            );
        }
        // The same row against the control the record used before
        // 2026-09-24, printed beside the corrected one rather than replaced
        // by it. The method's run is the same call with the same parameters,
        // so the gate figures are checked for having held still rather than
        // assumed to have.
        if also_registered_stop {
            if !fixed {
                println!("{:<12} {:<18} --null-registered-stop needs --fixed; no original printed", "", "");
            } else {
                match fd_backtest::hypotheses::run_hypothesis_fixed_as(
                    registry,
                    hypothesis,
                    bars,
                    rules,
                    gate,
                    seeds,
                    guards,
                    fd_backtest::hypotheses::CostMatch::RegisteredStop,
                ) {
                    Ok(before) => {
                        let moved = if before.percentile.is_nan() && report.percentile.is_nan() {
                            "both unmeasured".to_string()
                        } else {
                            format!("{:+.0} points of percentile", report.percentile - before.percentile)
                        };
                        println!(
                            "{:<12} {:<18} at the control's registered 1.5 ATR (the record's reading): pct {:.0}%, \
                             null p50 {:.3} p95 {:.3}, count match {:.2} — corrected {:.0}%, {moved}",
                            "",
                            "",
                            before.percentile,
                            before.null_quantile(0.5),
                            before.null_quantile(0.95),
                            before.count_match(),
                            report.percentile,
                        );
                        for (name, a, b) in [
                            ("trades", before.oos.trades as f64, m.trades as f64),
                            ("profit factor", before.oos.profit_factor, m.profit_factor),
                            ("expectancy", before.oos.expectancy, m.expectancy),
                        ] {
                            if a.to_bits() != b.to_bits() {
                                println!(
                                    "{:<12} {:<18} ** {name} MOVED between the two readings: {a} vs {b} — \
                                     the null is not the only thing that changed **",
                                    "", "",
                                );
                            }
                        }
                    }
                    Err(e) => println!("{:<12} {:<18} original refused: {e}", "", ""),
                }
            }
        }
        // How often a guard acted on this row's own out-of-sample runs, so a
        // receipt shows whether a bounded number was bounded in practice.
        if guards.is_some() {
            println!("{:<12} {:<18} {}", "", "", report.guard_activity());
        }
        if report.survives() {
            survivors.push(format!("{}/{}", report.label, report.base));
        }
    }
    println!();
    if survivors.is_empty() {
        println!("Nothing survived: no hypothesis is both past the gate and outside its own null.");
        println!("That is the result. The list is closed; add a hypothesis only with a new reason.");
    } else {
        println!("Survivors: {} — worth a decision record and a direction null before anything else.", survivors.join(", "));
    }
    println!();
}

/// Every construct in a batch, scored twice: as the record closed it, and
/// with the introducing-broker rebate credited beside the book.
///
/// **The credit never touches the cost model.** `rules.spread` is what the
/// venue charges and stays what it was; the net columns come from a credited
/// copy of the same trades. Gross is byte-identical to a `--mode=hypotheses`
/// run of the same batch, which is the property
/// `fd-backtest/src/rebate.rs` pins and `docs/decisions/2026-09-21-rebate-credit-line.md`
/// asks for.
///
/// **Both nulls carry the rebate.** The matched null's own out-of-sample
/// trades and the direction null's own trades are credited with the same
/// arithmetic, so the percentiles compare like with like. A run that could
/// not do that for one of the two says so in its header rather than printing
/// a percentile that means nothing.
#[allow(clippy::too_many_arguments)]
fn run_rescore(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    direction_samples: usize,
    batch_file: &str,
    rebate: Option<fd_backtest::Rebate>,
    guards: Option<&Guards>,
) {
    let Some(rebate) = rebate else {
        println!("no [rebate] table in the config directory's accounts.toml, and no --rebate-share=.");
        println!("That is 'no arrangement is recorded', not a rebate of zero, so nothing is scored.");
        return;
    };
    let batch = match batch_from_file(std::path::Path::new(batch_file)) {
        Ok(b) => b,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    println!("== rebate rescore `{batch_file}`: {} constructs, walk-forward ({folds} folds) ==", batch.len());
    println!("rebate:   {:.2} of the round-turn spread, credited BESIDE the book", rebate.share_of_spread);
    println!("          gross columns are unchanged; the cost model is untouched");
    println!("nulls:    matched null {seeds} runs and direction null {direction_samples} draws, BOTH carrying the same credit");
    println!("swap:     long {:.2} / short {:.2} USD per lot per night; spread {}", rules.swap_long_per_lot, rules.swap_short_per_lot, rules.spread);
    println!("{}", news_line());
    println!("{}", news_scope_line(rules));
    println!("{}", guards_line(guards));
    println!();
    println!(
        "{:<22} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}  verdict",
        "construct", "trades", "PF gross", "PF net", "rebate$", "reb/R", "mnullG", "mnullN", "dirN"
    );

    let mut passed: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let mut not_run: Vec<String> = Vec::new();
    for hypothesis in &batch {
        let outcome = fd_backtest::hypotheses::rescore_hypothesis(
            registry,
            hypothesis,
            bars,
            rules,
            folds,
            select_by,
            min_trades_per_cell,
            gate,
            seeds,
            direction_samples,
            rebate,
            guards,
        );
        let row = match outcome {
            Ok(Some(row)) => row,
            Ok(None) => {
                println!("{:<22} not run: the walk-forward could not be formed on these bars", hypothesis.label);
                not_run.push(format!("{}: no walk-forward on these bars", hypothesis.label));
                continue;
            }
            Err(e) => {
                println!("{:<22} not run: {e}", hypothesis.label);
                not_run.push(format!("{}: {e}", hypothesis.label));
                continue;
            }
        };
        ran += 1;
        let verdict = if row.passes_net() {
            "PASSES ALL THREE".to_string()
        } else if row.verdict_net.promising {
            format!("gate pass, inside a null (matched {:.0}, dir {:.0})", row.matched_pct_net, row.dir_pct_net)
        } else {
            format!("fail: {}", row.verdict_net.reasons.join("; "))
        };
        println!(
            "{:<22} {:>6} {:>8.3} {:>8.3} {:>8.2} {:>6.2}% {:>6.0}% {:>6.0}% {:>6.0}%  {verdict}",
            row.label,
            row.oos.trades,
            row.oos.profit_factor,
            row.oos_net.profit_factor,
            row.rebate_usd,
            row.rebate_frac_r * 100.0,
            row.matched_pct_gross,
            row.matched_pct_net,
            row.dir_pct_net,
        );
        println!(
            "{:<22} whole window {} trades, PF {:.3} gross / {:.3} net; direction null p95 {:.3} gross / {:.3} net{}",
            "",
            row.whole.trades,
            row.whole.profit_factor,
            row.whole_net.profit_factor,
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.dir_null_gross, 0.95),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.dir_null_net, 0.95),
            if row.dir_self_managed { " (own sides permuted)" } else { "" },
        );
        println!(
            "{:<22} matched null p50 {:.3} / p95 {:.3} gross, p50 {:.3} / p95 {:.3} net over {} runs",
            "",
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_gross, 0.50),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_gross, 0.95),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_net, 0.50),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_net, 0.95),
            row.matched_null_net.len(),
        );
        // The achieved count match, measured and printed rather than assumed
        // — see `docs/decisions/2026-09-23-matched-null-repair.md`.
        println!(
            "{:<22} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
            "",
            fd_backtest::hypotheses::median_count(&row.matched_null_trades),
            row.oos.trades,
            row.count_match(),
            if row.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" },
        );
        if row.passes_net() {
            passed.push(format!("{} ({})", row.label, if row.passes_gross() { "passed gross too" } else { "the credit moved it" }));
        }
    }

    println!();
    println!("ran {ran} of {} constructs in the batch; {} not run", batch.len(), not_run.len());
    for line in &not_run {
        println!("  not run — {line}");
    }
    println!();
    // The multiplicity statement, made by the record rather than left to a
    // reader: at the 95th percentile, one in twenty passes by luck, so a
    // batch of n expects n/20 passes from noise alone.
    let expected = batch.len() as f64 / 20.0;
    println!(
        "{} of {} passed all three legs; noise at the 95th percentile yields about {expected:.1} of {} by luck.",
        passed.len(),
        batch.len(),
        batch.len()
    );
    if passed.is_empty() {
        println!("Nothing crossed. The rebate lowers the bar and the bar was not the binding constraint.");
    } else {
        println!("Passed: {}", passed.join(", "));
        println!("A pass at or below the expected-by-chance count is indistinguishable from luck at this width.");
    }
    println!();
}

/// How much of the result is the market and how much is the spread.
///
/// Runs the same walk-forward at several fractions of the configured cost and
/// prints the curve. It answers one question and only one: is there a gross
/// edge that costs consume, or is there no edge to consume?
///
/// **The zero-cost column is not a result.** It is an upper bound that no one
/// can trade, printed so the distance between it and the real column is
/// visible. A method that only works at zero cost does not work.
#[allow(clippy::too_many_arguments)]
fn run_cost_sensitivity(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) {
    const FRACTIONS: [f64; 5] = [0.0, 0.25, 0.5, 1.0, 2.0];

    println!("== cost sensitivity: walk-forward profit factor at N x the configured cost ==");
    println!("configured spread {}, commission {:.2}/lot
", rules.spread, rules.commission_per_lot);
    print!("{:<20}", "strategy");
    for fraction in FRACTIONS {
        print!("{:>10}", format!("{fraction}x"));
    }
    println!("{:>12}", "trades");

    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        print!("{:<20}", strategy.id());
        let mut trades = 0usize;
        for fraction in FRACTIONS {
            let scaled = TradingRules {
                spread: rules.spread * fraction,
                commission_per_lot: rules.commission_per_lot * fraction,
                ..rules.clone()
            };
            match walk_forward_guarded(strategy.as_ref(), bars, &scaled, timeline, folds, select_by, min_trades_per_cell, guards)
            {
                Some(result) => {
                    if (fraction - 1.0).abs() < f64::EPSILON {
                        trades = result.oos.trades;
                    }
                    print!("{:>10.3}", result.oos.profit_factor);
                }
                None => print!("{:>10}", "-"),
            }
        }
        println!("{trades:>12}");
    }
    println!();
    println!("Read the gap, not the left column. A method whose profit factor only");
    println!("clears the gate at a cost nobody pays has not been shown to work.");
    println!();
}

/// Walk-forward over a strategy whose defaults have been overridden.
///
/// `walk_forward` builds its combinations from the strategy's own defaults, so
/// varying the seed means varying the defaults it starts from.
#[allow(clippy::too_many_arguments)]
fn walk_forward_seeded(
    strategy: &dyn fd_strategy::registry::Strategy,
    defaults: &fd_strategy::registry::Params,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) -> Option<fd_backtest::WalkForwardResult> {
    struct Seeded<'a> {
        inner: &'a dyn fd_strategy::registry::Strategy,
        defaults: fd_strategy::registry::Params,
    }
    impl fd_strategy::registry::Strategy for Seeded<'_> {
        fn id(&self) -> &'static str {
            self.inner.id()
        }
        fn name(&self) -> &'static str {
            self.inner.name()
        }
        fn description(&self) -> &'static str {
            self.inner.description()
        }
        fn default_params(&self) -> fd_strategy::registry::Params {
            self.defaults.clone()
        }
        fn grid(&self) -> std::collections::BTreeMap<String, Vec<f64>> {
            self.inner.grid()
        }
        fn indicators(&self, p: &fd_strategy::registry::Params) -> Vec<fd_indicators::IndicatorSpec> {
            self.inner.indicators(p)
        }
        fn warmup(&self, p: &fd_strategy::registry::Params) -> usize {
            self.inner.warmup(p)
        }
        fn series(&self, p: &fd_strategy::registry::Params) -> Vec<String> {
            self.inner.series(p)
        }
        fn needs_options(&self) -> bool {
            self.inner.needs_options()
        }
        fn on_bar(&self, ctx: &fd_strategy::registry::BarContext) -> fd_strategy::registry::Intent {
            self.inner.on_bar(ctx)
        }
    }
    let seeded = Seeded { inner: strategy, defaults: defaults.clone() };
    walk_forward_guarded(&seeded, bars, rules, None, folds, select_by, min_trades_per_cell, guards)
}

fn iso(ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    let days = ms.div_euclid(DAY_MS);
    let rest = ms.rem_euclid(DAY_MS);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60)
}
