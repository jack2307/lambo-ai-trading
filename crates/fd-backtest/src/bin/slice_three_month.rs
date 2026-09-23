//! The 2026-09-23 three-month search, one slice of the mechanism list.
//!
//! Registered in `docs/hypotheses/2026-09-23-three-month-search.md`. The
//! protocol is not this file's to choose:
//!
//! 1. **In sample.** Every cell of every named mechanism's *registry grid* on
//!    `xauusd:15m` over the three-month window, each cell run through the
//!    existing walk-forward at the configured fold count with the cell's
//!    parameters **pinned** — a `Preset` naming every grid axis, so no fold
//!    can re-fit it. Ranked by return over the window.
//! 2. **Out of sample, unchanged.** The top five cells replayed at those same
//!    parameters over the nine months *before* the search window, with no
//!    selection of any kind (`run_hypothesis_fixed_guarded`, the replay path),
//!    each against its count-matched null.
//!
//! Nothing here sets a threshold. The gate figures come from
//! `[backtest.promising]`, the spread and the guards from the market's own
//! config, and the survival test the registration declared —
//! `expectancy > 0`, `profit factor >= 1.2`, `>= 95th percentile of the
//! matched null` — is evaluated, not chosen, below. Under 30 trades is
//! reported as "not a result" rather than scored, which is the registry's own
//! `min_trades`.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin slice_three_month -- \
//!     --data=E:/rust/flowdesk/data --guards
//! ```

use std::path::PathBuf;

use fd_backtest::hypotheses::{Hypothesis, median_count, run_hypothesis_fixed_guarded, run_hypothesis_guarded};
use fd_backtest::sweep::SelectBy;
use fd_backtest::{Guards, PromisingGate};
use fd_core::types::Bar;
use fd_core::config::Config;
use fd_store::read_bars;
use fd_strategy::registry::{Params, Registry, parameter_combinations};

/// The five mechanisms this slice owns.
const SLICE: &str = "ema-cross,rsi-reversion,donchian-breakout,bb-fade,keltner-break";

/// The search window. End dates are INCLUSIVE here and turned into the
/// exclusive bound the cutter wants, so the two windows named in the
/// registration do not overlap by a day and do not skip one either.
const IN_FROM: (i64, u32, u32) = (2026, 6, 23);
const IN_TO_INCL: (i64, u32, u32) = (2026, 9, 23);
const OOS_FROM: (i64, u32, u32) = (2025, 9, 23);
const OOS_TO_INCL: (i64, u32, u32) = (2026, 6, 22);

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn ms(date: (i64, u32, u32)) -> i64 {
    fd_core::clock::days_from_civil(date.0, date.1, date.2) * 86_400_000
}

fn iso(ms: i64) -> String {
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(86_400_000));
    let rest = ms.rem_euclid(86_400_000);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60)
}

/// The bars inside `[from, to_incl]`, the end day included.
fn cut(bars: &[Bar], from: (i64, u32, u32), to_incl: (i64, u32, u32)) -> Vec<Bar> {
    let (lo, hi) = (ms(from), ms(to_incl) + 86_400_000);
    bars.iter().filter(|b| b.time >= lo && b.time < hi).cloned().collect()
}

/// One grid cell, as a hypothesis whose every grid axis is pinned.
fn cell(base: &str, overrides: &[(String, f64)], n: usize) -> Hypothesis {
    Hypothesis {
        label: format!("{base}#{n}"),
        base: base.to_string(),
        filters: Vec::new(),
        overrides: overrides.to_vec(),
        why: "a cell of the registry grid, swept because the registration says to sweep them all".to_string(),
    }
}

fn params_text(overrides: &[(String, f64)]) -> String {
    overrides.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
}

/// A cell's in-sample row.
struct Row {
    base: String,
    overrides: Vec<(String, f64)>,
    return_pct: f64,
    total_r: f64,
    trades: usize,
    profit_factor: f64,
    expectancy: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "xauusd");
    let interval = arg("interval", "15m");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;

    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let all = read_bars(&bars_path)?;
    if all.is_empty() {
        println!("no bars at {}", bars_path.display());
        return Ok(());
    }

    let rules = fd_backtest::engine::trading_rules_for(&config, &market)?;
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by =
        if config.backtest.select_by == "profitFactor" { SelectBy::ProfitFactor } else { SelectBy::Expectancy };
    let folds = config.backtest.walk_forward_folds;
    let min_trades_per_cell = config.backtest.min_trades_per_cell;
    let guards = std::env::args().any(|a| a == "--guards").then(|| Guards::for_market(&config, &market)).transpose()?;
    let guards = guards.as_ref();

    // The same calendar every `news:` filter and the news guard read.
    let news_path = data.join("news").join("events.parquet");
    if news_path.is_file()
        && let Err(e) = fd_store::read_news(&news_path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install)
    {
        println!("news: could not load {}: {e}", news_path.display());
    }

    let seeds_in: usize = arg("seeds-in", "0").parse().unwrap_or(0);
    let seeds_oos: usize = arg("seeds", "200").parse().unwrap_or(200);
    let carry: usize = arg("carry", "5").parse().unwrap_or(5);

    println!("== 2026-09-23 three-month search, slice 1 ==");
    println!("market:   {market}:{interval} ({})", spec.bar_symbol);
    println!("bars:     {} on file, {} to {}", all.len(), iso(all[0].time), iso(all[all.len() - 1].time));
    println!("spread:   {} per round trip (configured, not overridden)", rules.spread);
    println!(
        "trail:    {}",
        if rules.trail.enabled {
            format!("ON distance {}R activate {}R", rules.trail.distance_r, rules.trail.activate_r)
        } else {
            "off".to_string()
        }
    );
    println!("folds:    {folds} (config walk_forward_folds), select_by {}", config.backtest.select_by);
    println!(
        "gate:     min_trades {} / min_profit_factor {} / min_expectancy_r {}",
        gate.min_trades, gate.min_profit_factor, gate.min_expectancy_r
    );
    println!("{}", fd_strategy::news::summary("data/news/events.parquet"));
    println!(
        "guards:   {}",
        match guards {
            Some(g) => format!("on — {}", g.describe()),
            None => "off (every number unguarded)".to_string(),
        }
    );
    println!();

    let registry = Registry::with_builtins();
    let names: Vec<String> = arg("strategies", SLICE).split(',').map(|s| s.trim().to_string()).collect();

    let in_bars = cut(&all, IN_FROM, IN_TO_INCL);
    let oos_bars = cut(&all, OOS_FROM, OOS_TO_INCL);
    if in_bars.is_empty() || oos_bars.is_empty() {
        println!("one of the windows is empty on this feed — nothing to run");
        return Ok(());
    }
    println!(
        "in sample:  {} bars, {} to {} (asked {:?}..={:?})",
        in_bars.len(),
        iso(in_bars[0].time),
        iso(in_bars[in_bars.len() - 1].time),
        IN_FROM,
        IN_TO_INCL
    );
    println!(
        "out of s.:  {} bars, {} to {} (asked {:?}..={:?})",
        oos_bars.len(),
        iso(oos_bars[0].time),
        iso(oos_bars[oos_bars.len() - 1].time),
        OOS_FROM,
        OOS_TO_INCL
    );
    println!();

    /* ------------------------------------------------ 1. the in-sample sweep */

    println!("-- in sample: every cell of every grid, walk-forward ({folds} folds), parameters pinned per cell --");
    println!("{:<20} {:<28} {:>9} {:>9} {:>7} {:>7} {:>8}", "base", "cell", "return%", "total R", "trades", "PF", "expect");
    let mut rows: Vec<Row> = Vec::new();
    let mut swept = 0usize;
    let mut unrunnable: Vec<String> = Vec::new();
    for name in &names {
        let strategy = match registry.get(name) {
            Ok(s) => s,
            Err(e) => {
                println!("{name:<20} refused: {e}");
                continue;
            }
        };
        let grid = strategy.grid();
        let axes: Vec<String> = grid.keys().cloned().collect();
        let combos: Vec<Params> = parameter_combinations(&strategy.default_params(), &grid);
        for (n, combo) in combos.iter().enumerate() {
            let overrides: Vec<(String, f64)> = axes.iter().map(|k| (k.clone(), combo.get(k))).collect();
            let h = cell(name, &overrides, n + 1);
            swept += 1;
            match run_hypothesis_guarded(
                &registry, &h, &in_bars, &rules, folds, select_by, min_trades_per_cell, &gate, seeds_in, guards,
            ) {
                Ok(Some(report)) => {
                    let m = &report.oos;
                    println!(
                        "{:<20} {:<28} {:>9.2} {:>9.3} {:>7} {:>7.3} {:>8.4}",
                        name,
                        params_text(&overrides),
                        m.return_pct,
                        m.total_r,
                        m.trades,
                        m.profit_factor,
                        m.expectancy
                    );
                    rows.push(Row {
                        base: name.clone(),
                        overrides,
                        return_pct: m.return_pct,
                        total_r: m.total_r,
                        trades: m.trades,
                        profit_factor: m.profit_factor,
                        expectancy: m.expectancy,
                    });
                }
                Ok(None) => {
                    println!("{:<20} {:<28} not enough bars for {folds} folds", name, params_text(&overrides));
                    unrunnable.push(format!("{name} {}: not enough bars", params_text(&overrides)));
                }
                Err(e) => {
                    println!("{:<20} {:<28} refused: {e}", name, params_text(&overrides));
                    unrunnable.push(format!("{name} {}: {e}", params_text(&overrides)));
                }
            }
        }
    }
    println!();
    println!("cells swept: {swept} across {} mechanisms; {} produced a walk-forward number", names.len(), rows.len());

    rows.sort_by(|a, b| b.return_pct.partial_cmp(&a.return_pct).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<&Row> = rows.iter().take(carry).collect();
    println!();
    println!("-- the top {} by in-sample return over the three months --", top.len());
    for (i, r) in top.iter().enumerate() {
        println!(
            "{}. {:<20} {:<28} return {:.2}%  total {:.3} R over {} trades, PF {:.3}, expectancy {:.4} R",
            i + 1,
            r.base,
            params_text(&r.overrides),
            r.return_pct,
            r.total_r,
            r.trades,
            r.profit_factor,
            r.expectancy
        );
    }

    /* --------------------------------------- 2. the same cells, nine months */

    println!();
    println!("-- out of sample: the same {} cells, same parameters, NO re-fit, on the nine months before the search --", top.len());
    println!("null: {seeds_oos} count-matched control runs per cell, the 2026-09-23 repair in force");
    println!();
    println!(
        "{:<20} {:<28} {:>7} {:>9} {:>7} {:>8} {:>8} {:>8} {:>5}",
        "base", "cell", "trades", "return%", "PF", "expect", "null p50", "null p95", "pct"
    );
    let mut survivors: Vec<String> = Vec::new();
    let mut verdict_lines: Vec<String> = Vec::new();
    for r in &top {
        let h = cell(&r.base, &r.overrides, 0);
        match run_hypothesis_fixed_guarded(&registry, &h, &oos_bars, &rules, &gate, seeds_oos, guards) {
            Ok(report) => {
                let m = &report.oos;
                println!(
                    "{:<20} {:<28} {:>7} {:>9.2} {:>7.3} {:>8.4} {:>8.3} {:>8.3} {:>4.0}%",
                    r.base,
                    params_text(&r.overrides),
                    m.trades,
                    m.return_pct,
                    m.profit_factor,
                    m.expectancy,
                    report.null_quantile(0.5),
                    report.null_quantile(0.95),
                    report.percentile
                );
                println!(
                    "{:<20} {:<28} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
                    "",
                    "",
                    median_count(&report.null_trades),
                    m.trades,
                    report.count_match(),
                    if report.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" }
                );
                if guards.is_some() {
                    println!("{:<20} {:<28} {}", "", "", report.guard_activity());
                }
                // The registration's three legs, each said out loud.
                let thin = m.trades < gate.min_trades;
                let leg_e = m.expectancy > 0.0;
                let leg_pf = m.profit_factor >= gate.min_profit_factor;
                let leg_null = report.percentile >= 95.0;
                let line = if thin {
                    format!(
                        "{} {}: {} trades on the nine months — under {}, so it is not a result and is not scored",
                        r.base,
                        params_text(&r.overrides),
                        m.trades,
                        gate.min_trades
                    )
                } else {
                    format!(
                        "{} {}: expectancy {} ({:.4} R), profit factor {} ({:.3} vs {}), null {} ({:.0}th vs 95th) — {}",
                        r.base,
                        params_text(&r.overrides),
                        if leg_e { "PASS" } else { "FAIL" },
                        m.expectancy,
                        if leg_pf { "PASS" } else { "FAIL" },
                        m.profit_factor,
                        gate.min_profit_factor,
                        if leg_null { "PASS" } else { "FAIL" },
                        report.percentile,
                        if leg_e && leg_pf && leg_null { "SURVIVES all three" } else { "refuted" }
                    )
                };
                if !thin && leg_e && leg_pf && leg_null {
                    survivors.push(format!("{} {}", r.base, params_text(&r.overrides)));
                }
                verdict_lines.push(line);
            }
            Err(e) => {
                println!("{:<20} {:<28} refused: {e}", r.base, params_text(&r.overrides));
                verdict_lines.push(format!("{} {}: could not run — {e}", r.base, params_text(&r.overrides)));
            }
        }
    }

    println!();
    println!("-- the three legs, cell by cell --");
    for line in &verdict_lines {
        println!("{line}");
    }
    println!();
    if survivors.is_empty() {
        println!(
            "Survivors: none of {}. Every cell carried forward from the three-month search is refuted on the nine months it was not selected on.",
            top.len()
        );
    } else {
        println!("Survivors: {}", survivors.join(", "));
    }
    if !unrunnable.is_empty() {
        println!();
        println!("could not be scored in sample:");
        for u in &unrunnable {
            println!("  {u}");
        }
    }
    println!();
    println!("cells swept: {swept}. Nothing above went near the funded account, and the registration forbids it doing so.");
    Ok(())
}
