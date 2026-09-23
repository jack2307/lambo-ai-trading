//! The 2026-09-23 three-month search, one slice of it, both legs in one run.
//!
//! Registered in `docs/hypotheses/2026-09-23-three-month-search.md` before any
//! number was seen. The owner asked for the best method on the last three
//! months at 50% or better; the registration says the search will produce such
//! a number by arithmetic and pre-declares what would make it mean anything.
//! This binary is what runs it, and it runs **both legs in one process** so
//! that no cell is copied by hand between the window it was chosen on and the
//! window that tests it.
//!
//! * **Leg one, in sample.** Every cell of every named strategy's own grid,
//!   walked forward with the configured fold count on the in-sample window,
//!   ranked by the walk-forward return over that window. A cell is the grid
//!   point *pinned* — `Preset::new` names every grid axis, so the walk-forward
//!   has nothing left to select and measures that one parameter set on each
//!   fold's test block. The plain whole-window in-sample return of the same
//!   cell is printed beside it, because that is the number the question was
//!   really about and the registration commits to publishing it whatever it is.
//!
//! * **Leg two, out of sample.** The top `--top` cells by that ranking, with
//!   the same strategy and the same parameters and **no re-fit**, replayed over
//!   the out-of-sample window by `run_hypothesis_fixed_guarded` — fixed
//!   parameters, no folds, no selection — each against `--seeds` runs of its
//!   count-matched random-entry null (the null repaired earlier the same day,
//!   `docs/decisions/2026-09-23-matched-null-repair.md`).
//!
//! The three survival conditions are the registration's and are not settable
//! from the command line: expectancy above zero, profit factor at or above the
//! registry's standing 1.2, and at or above the 95th percentile of the matched
//! null. Trade counts are printed on every row because under 30 trades is not
//! a result whatever the other three say.
//!
//! ```text
//! three_month_2 --market=xauusd --interval=15m --data=E:/rust/flowdesk/data \
//!   --strategies=macd-cross,rsi2-pullback,squeeze-break,stoch-reversal,orb \
//!   --in-from=2026-06-23 --in-to=2026-09-23 \
//!   --oos-from=2025-09-23 --oos-to=2026-06-22 --top=5 --seeds=200 --guards
//! ```

use std::path::PathBuf;

use fd_backtest::engine::{Range, TradingRules, run_backtest_guarded};
use fd_backtest::hypotheses::{Hypothesis, Preset, run_hypothesis_fixed_guarded};
use fd_backtest::sweep::{SelectBy, walk_forward_guarded};
use fd_backtest::{Guards, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;
use fd_strategy::registry::{Registry, parameter_combinations};

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// `--from=YYYY-MM-DD`, as milliseconds. `to` is exclusive wherever it is used.
fn bound(name: &str) -> Option<i64> {
    let text = arg(name, "");
    if text.is_empty() {
        return None;
    }
    let mut parts = text.split('-').map(|p| p.parse::<i64>());
    let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000)
}

/// UTC `YYYY-MM-DD HH:MM`, the same rendering the search receipts use.
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

/// The bars inside `[from, to)`, said out loud — what was asked for and what
/// the store actually holds, because a window the feed does not reach is the
/// first thing a reader of the receipt needs to know.
fn window(bars: &[Bar], prefix: &str, label: &str) -> Vec<Bar> {
    let (from, to) = (bound(&format!("{prefix}-from")), bound(&format!("{prefix}-to")));
    let kept: Vec<Bar> = bars
        .iter()
        .filter(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t))
        .cloned()
        .collect();
    if kept.is_empty() {
        println!("{label}: no bars in {} → {}", arg(&format!("{prefix}-from"), "…"), arg(&format!("{prefix}-to"), "…"));
    } else {
        println!(
            "{label}: {} bars, {} → {} (asked {} → {}, `to` exclusive)",
            kept.len(),
            iso(kept[0].time),
            iso(kept[kept.len() - 1].time),
            arg(&format!("{prefix}-from"), "…"),
            arg(&format!("{prefix}-to"), "…"),
        );
    }
    kept
}

/// One grid point of one strategy, measured on the in-sample window.
struct Cell {
    strategy: String,
    /// Every grid axis of that strategy, at this cell's value.
    overrides: Vec<(String, f64)>,
    /// Walk-forward over the in-sample window with this cell pinned.
    wf_return_pct: f64,
    wf_trades: usize,
    wf_profit_factor: f64,
    wf_expectancy: f64,
    wf_folds_measured: usize,
    /// The same cell over the whole in-sample window, no folds: the number the
    /// owner's question asks for.
    full_return_pct: f64,
    full_trades: usize,
    full_profit_factor: f64,
    full_expectancy: f64,
}

impl Cell {
    fn params_label(&self) -> String {
        self.overrides.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "xauusd");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;
    let interval = arg("interval", &config.backtest.timeframe);
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let bars = read_bars(&bars_path)?;
    if bars.is_empty() {
        println!("no bars at {}", bars_path.display());
        return Ok(());
    }

    let rules: TradingRules = fd_backtest::engine::trading_rules_for(&config, &market)?;
    let guards = std::env::args().any(|a| a == "--guards").then(|| Guards::for_market(&config, &market)).transpose()?;
    let guards = guards.as_ref();
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by = match config.backtest.select_by.as_str() {
        "profitFactor" => SelectBy::ProfitFactor,
        "totalR" => SelectBy::TotalR,
        _ => SelectBy::Expectancy,
    };
    let folds = config.backtest.walk_forward_folds;
    let min_trades_per_cell = config.backtest.min_trades_per_cell;
    let seeds: usize = arg("seeds", "200").parse().unwrap_or(200);
    let top: usize = arg("top", "5").parse().unwrap_or(5);

    println!("market:   {market} {interval} from {}", bars_path.display());
    println!("bars:     {} from {} to {}", bars.len(), iso(bars[0].time), iso(bars[bars.len() - 1].time));
    println!("spread:   {} per round trip (configured)", rules.spread);
    println!("equity:   {} starting", rules.starting_equity_usd);
    println!(
        "trail:    {}",
        if rules.trail.enabled { format!("ON {}R/{}R", rules.trail.distance_r, rules.trail.activate_r) } else { "off".into() }
    );
    let news_path = data.join("news").join("events.parquet");
    if news_path.is_file() {
        if let Err(e) = fd_store::read_news(&news_path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            println!("news: could not load {}: {e}", news_path.display());
        }
    }
    println!("{}", fd_strategy::news::summary("data/news/events.parquet"));
    println!(
        "guards:   {}",
        match guards {
            Some(g) => format!("on — {}", g.describe()),
            None => "off (every number unguarded)".to_string(),
        }
    );
    println!("gate:     min_trades {} / min_profit_factor {} / min_expectancy_r {}", gate.min_trades, gate.min_profit_factor, gate.min_expectancy_r);
    println!("walk:     {folds} folds, select_by {:?}, min {min_trades_per_cell} trades per training cell", select_by);
    println!();

    let in_bars = window(&bars, "in", "in-sample");
    let oos_bars = window(&bars, "oos", "oos      ");
    println!();
    if in_bars.is_empty() {
        return Ok(());
    }

    let registry = Registry::with_builtins();
    let ids: Vec<String> = arg("strategies", "").split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    if ids.is_empty() {
        println!("--strategies=<id,id,…> is required");
        return Ok(());
    }

    /* ------------------------------------------------ leg one: in sample */

    println!("== leg one: every cell of every grid, walked forward on the in-sample window ==");
    let mut cells: Vec<Cell> = Vec::new();
    for id in &ids {
        let base = registry.get(id)?;
        let grid = base.grid();
        if grid.is_empty() {
            println!("{id}: no grid — nothing to sweep");
            continue;
        }
        let axes: Vec<String> = grid.keys().cloned().collect();
        let combos = parameter_combinations(&base.default_params(), &grid);
        println!("{id}: {} cells over {}", combos.len(), axes.join(", "));
        for combo in &combos {
            let overrides: Vec<(String, f64)> = axes.iter().map(|k| (k.clone(), combo.get(k))).collect();
            let preset = Preset::new(base, &overrides)?;
            let wf = walk_forward_guarded(&preset, &in_bars, &rules, None, folds, select_by, min_trades_per_cell, guards);
            let full = run_backtest_guarded(&in_bars, &preset, &preset.defaults, &rules, guards, None, Range::default(), None);
            let (wf_return_pct, wf_trades, wf_pf, wf_exp, measured) = match &wf {
                Some(r) => (
                    r.oos.return_pct,
                    r.oos.trades,
                    r.oos.profit_factor,
                    r.oos.expectancy,
                    r.folds.iter().filter(|f| f.test.is_some()).count(),
                ),
                None => (f64::NAN, 0, f64::NAN, f64::NAN, 0),
            };
            cells.push(Cell {
                strategy: id.clone(),
                overrides,
                wf_return_pct,
                wf_trades,
                wf_profit_factor: wf_pf,
                wf_expectancy: wf_exp,
                wf_folds_measured: measured,
                full_return_pct: full.metrics.return_pct,
                full_trades: full.metrics.trades,
                full_profit_factor: full.metrics.profit_factor,
                full_expectancy: full.metrics.expectancy,
            });
        }
    }
    println!();
    println!("cells swept: {}", cells.len());
    println!();

    // Ranked by the walk-forward return over the in-sample window, which is
    // what the registration names as the selection rule. Every cell is printed;
    // the registration forbids dropping one for being embarrassing in either
    // direction.
    let mut ranked: Vec<&Cell> = cells.iter().collect();
    ranked.sort_by(|a, b| {
        let (x, y) = (b.wf_return_pct, a.wf_return_pct);
        let x = if x.is_finite() { x } else { f64::NEG_INFINITY };
        let y = if y.is_finite() { y } else { f64::NEG_INFINITY };
        x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("{:<4} {:<16} {:<34} {:>10} {:>7} {:>7} {:>8} {:>6} {:>10} {:>7} {:>7} {:>8}", "#", "strategy", "cell", "wfRet%", "wfTrd", "wfPF", "wfExp", "folds", "fullRet%", "fullTrd", "fullPF", "fullExp");
    for (n, c) in ranked.iter().enumerate() {
        println!(
            "{:<4} {:<16} {:<34} {:>10.2} {:>7} {:>7.3} {:>8.3} {:>6} {:>10.2} {:>7} {:>7.3} {:>8.3}",
            n + 1,
            c.strategy,
            c.params_label(),
            c.wf_return_pct,
            c.wf_trades,
            c.wf_profit_factor,
            c.wf_expectancy,
            c.wf_folds_measured,
            c.full_return_pct,
            c.full_trades,
            c.full_profit_factor,
            c.full_expectancy,
        );
    }
    println!();

    /* --------------------------------------------- leg two: out of sample */

    if oos_bars.is_empty() {
        println!("no out-of-sample bars in the window — leg two did not run");
        return Ok(());
    }
    let carried: Vec<&Cell> = ranked.iter().take(top).copied().collect();
    println!(
        "== leg two: the top {} cells replayed on the out-of-sample window, same parameters, no re-fit, {seeds} matched-null runs each ==",
        carried.len()
    );
    println!("survival is all three: expectancy > 0, profit factor >= {}, and >= the 95th percentile of the matched null", gate.min_profit_factor);
    println!();
    println!("{:<16} {:<34} {:>8} {:>8} {:>8} {:>9} {:>9} {:>6} {:>7} {:>9}", "strategy", "cell", "ret%", "trades", "PF", "expect", "nullP95", "pct", "match", "survives");
    let mut survivors: Vec<String> = Vec::new();
    for c in &carried {
        let hypothesis = Hypothesis {
            label: format!("{}/{}", c.strategy, c.params_label()),
            base: c.strategy.clone(),
            filters: Vec::new(),
            overrides: c.overrides.clone(),
            why: "carried out of the three-month search by its in-sample rank, unchanged, to the nine months it was not selected on".to_string(),
        };
        let report = run_hypothesis_fixed_guarded(&registry, &hypothesis, &oos_bars, &rules, &gate, seeds, guards)?;
        let m = &report.oos;
        // The registration's three conditions, read off the report. Not the
        // `verdict` helper: that one also wants expectancy >= 0.05, which is
        // the registry's promotion gate and stricter than what was registered
        // here. Both are printed so neither reading is hidden.
        let legs = (
            m.expectancy > 0.0,
            m.profit_factor >= gate.min_profit_factor,
            report.percentile >= 95.0,
        );
        let survived = legs.0 && legs.1 && legs.2;
        if survived {
            survivors.push(hypothesis.label.clone());
        }
        println!(
            "{:<16} {:<34} {:>8.2} {:>8} {:>8.3} {:>9.3} {:>9.3} {:>5.0}% {:>7.2} {:>9}",
            c.strategy,
            c.params_label(),
            m.return_pct,
            m.trades,
            m.profit_factor,
            m.expectancy,
            report.null_quantile(0.95),
            report.percentile,
            report.count_match(),
            if survived { "YES" } else { "no" },
        );
        println!(
            "{:<16} {:<34} expectancy>0 {} | PF>={} {} | >=95th {} | registry verdict: {}{}",
            "",
            "",
            if legs.0 { "pass" } else { "FAIL" },
            gate.min_profit_factor,
            if legs.1 { "pass" } else { "FAIL" },
            if legs.2 { "pass" } else { "FAIL" },
            if report.verdict.promising { "promising".to_string() } else { format!("fail: {}", report.verdict.reasons.join("; ")) },
            if m.trades < gate.min_trades { format!("  ** {} trades: under {} is not a result **", m.trades, gate.min_trades) } else { String::new() },
        );
        println!(
            "{:<16} {:<34} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
            "",
            "",
            fd_backtest::hypotheses::median_count(&report.null_trades),
            m.trades,
            report.count_match(),
            if report.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" },
        );
        if guards.is_some() {
            println!("{:<16} {:<34} {}", "", "", report.guard_activity());
        }
    }
    println!();
    if survivors.is_empty() {
        println!("Nothing survived: not one carried cell is past both gate legs and outside its own matched null on the nine months.");
    } else {
        println!("Survived all three: {}", survivors.join(", "));
    }
    println!();
    Ok(())
}
