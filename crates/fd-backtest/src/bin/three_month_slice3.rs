//! Slice 3 of the 2026-09-23 three-month search: `pdhl`,
//! `ict-sweep-mss-fvg`, `vwap-fade`, `doji-reversal`, `gap-fade`.
//!
//! Registration: `docs/hypotheses/2026-09-23-three-month-search.md`. Nothing
//! here chooses a threshold; the gate, the 95th percentile and the spread come
//! from the config and from that registration, and this binary exists only
//! because no existing mode ranks *cells* by return and then replays the named
//! cells on another window without re-fitting them.
//!
//! Two stages, run separately so the second cannot see the first's window:
//!
//! ```text
//! three_month_slice3 --stage=in  --from=2026-06-23 --to=2026-09-24 --cells-out=<file>
//! three_month_slice3 --stage=oos --from=2025-09-23 --to=2026-06-22 --cells=<file>
//! ```
//!
//! `--stage=in` sweeps every cell of every grid over the window at the
//! configured spread with the guards on, ranks the cells by `return_pct`, and
//! writes the top five as `strategy key=value,key=value` lines. It also reports
//! the mechanism's own walk-forward over the same window at the configured fold
//! count and `select_by`, because the registration's "walk-forward" is a
//! *mechanism* number: a fixed cell has nothing to select, so a per-cell
//! ranking is in-sample by construction and is published as such.
//!
//! `--stage=oos` replays exactly those cells — same strategy, same parameters,
//! no fold, no selection — through `run_hypothesis_fixed_guarded`, which is the
//! desk's existing replay-against-a-matched-null path, and prints the three
//! legs the registration declared.

use std::path::PathBuf;

use fd_backtest::engine::{Range, run_backtest_guarded};
use fd_backtest::hypotheses::{Hypothesis, run_hypothesis_fixed_guarded};
use fd_backtest::sweep::{SelectBy, walk_forward_guarded};
use fd_backtest::{Guards, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;
use fd_strategy::registry::{Params, Registry, parameter_combinations};

/// The five mechanisms this slice owns, in the order the assignment lists them.
const SLICE: [&str; 5] = ["pdhl", "ict-sweep-mss-fvg", "vwap-fade", "doji-reversal", "gap-fade"];

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// `--from=YYYY-MM-DD` as a millisecond bound, `None` when absent.
fn bound(name: &str) -> Option<i64> {
    let text = arg(name, "");
    if text.is_empty() {
        return None;
    }
    let mut parts = text.split('-').map(|p| p.parse::<i64>());
    let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000)
}

/// The same calendar arithmetic `search.rs` prints its window with, so the two
/// receipts name the same window the same way.
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

/// A cell: the strategy it belongs to and the grid values that name it.
#[derive(Clone)]
struct Cell {
    strategy: String,
    /// Only the grid axes, so the line is the preset and nothing else.
    overrides: Vec<(String, f64)>,
    params: Params,
}

impl Cell {
    /// `riskReward=1.5,bufferPips=3` — the form the two stages exchange.
    fn preset(&self) -> String {
        self.overrides.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "xauusd");
    let stage = arg("stage", "in");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;
    let interval = arg("interval", &config.backtest.timeframe);

    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let mut bars = read_bars(&bars_path)?;
    let before = bars.len();
    let (from, to) = (bound("from"), bound("to"));
    bars.retain(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t));
    if bars.is_empty() {
        println!("no bars at {} after bounds", bars_path.display());
        return Ok(());
    }

    let mut rules = fd_backtest::engine::trading_rules_for(&config, &market)?;
    if let Some(v) = std::env::args().find_map(|a| a.strip_prefix("--spread=").map(str::to_string)) {
        rules.spread = v.trim().parse().map_err(|_| format!("--spread wants a number, got `{v}`"))?;
    }
    let rules = rules;
    // Guards are always on here: the registration says so, and the warning
    // that `gap-fade` may take no trade at all is a consequence of one of them
    // (WEEKEND_FLAT against a Sunday-reopen entry), which is a finding and not
    // something to switch off.
    let guards = Guards::for_market(&config, &market)?;
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by =
        if config.backtest.select_by == "profitFactor" { SelectBy::ProfitFactor } else { SelectBy::Expectancy };

    let news = {
        let path = data.join("news").join("events.parquet");
        if path.is_file() {
            match fd_store::read_news(&path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
                Ok(_) => {}
                Err(e) => println!("news: could not load {}: {e}", path.display()),
            }
        }
        fd_strategy::news::summary("data/news/events.parquet")
    };

    println!("slice:    3 — {}", SLICE.join(", "));
    println!("stage:    {stage}");
    println!("market:   {market}:{interval}  bars {} of {before} kept", bars.len());
    println!("window:   {} -> {}", iso(bars[0].time), iso(bars[bars.len() - 1].time));
    println!("bounds:   --from={} --to={} (to exclusive)", arg("from", "…"), arg("to", "…"));
    println!("spread:   {} per round trip", rules.spread);
    println!("trail:    {}", if rules.trail.enabled { "ON" } else { "off" });
    println!("guards:   on — {}", guards.describe());
    println!("gate:     min_trades {} / PF {} / expectancy {}R", gate.min_trades, gate.min_profit_factor, gate.min_expectancy_r);
    println!("{news}");
    println!("news scope: {}", if rules.news_currencies.is_empty() { "every currency".to_string() } else { rules.news_currencies.join("|") });
    println!();

    let registry = Registry::with_builtins();
    match stage.as_str() {
        "in" => stage_in(&registry, &bars, &rules, Some(&guards), &gate, select_by, &config),
        "oos" => stage_oos(&registry, &bars, &rules, Some(&guards), &gate),
        other => println!("unknown --stage={other}; expected `in` or `oos`"),
    }
    Ok(())
}

/// Every cell of every grid in the slice, over the whole window.
fn stage_in(
    registry: &Registry,
    bars: &[Bar],
    rules: &fd_backtest::engine::TradingRules,
    guards: Option<&Guards>,
    gate: &PromisingGate,
    select_by: SelectBy,
    config: &Config,
) {
    let folds = config.backtest.walk_forward_folds;
    let min_trades_per_cell = config.backtest.min_trades_per_cell;

    let mut rows: Vec<(Cell, fd_backtest::engine::Metrics)> = Vec::new();
    for id in SLICE {
        let Ok(strategy) = registry.get(id) else {
            println!("unknown strategy: {id}");
            continue;
        };
        let grid = strategy.grid();
        let axes: Vec<String> = grid.keys().cloned().collect();
        for params in parameter_combinations(&strategy.default_params(), &grid) {
            let result = run_backtest_guarded(bars, strategy, &params, rules, guards, None, Range::default(), None);
            let overrides = axes.iter().map(|k| (k.clone(), params.get(k))).collect();
            rows.push((Cell { strategy: id.to_string(), overrides, params: params.clone() }, result.metrics));
        }
    }

    println!("== in-sample: every cell of the slice's grids, whole window ==");
    println!("cells swept: {}", rows.len());
    println!();
    println!(
        "{:<20} {:<34} {:>7} {:>7} {:>8} {:>8} {:>9} {:>10}",
        "strategy", "cell", "trades", "win%", "profit", "expect", "totalR", "return%"
    );
    let mut ordered: Vec<usize> = (0..rows.len()).collect();
    ordered.sort_by(|a, b| {
        rows[*b].1.return_pct.partial_cmp(&rows[*a].1.return_pct).unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in &ordered {
        let (cell, m) = &rows[*i];
        println!(
            "{:<20} {:<34} {:>7} {:>6.1}% {:>8.3} {:>8.3} {:>9.2} {:>9.2}%",
            cell.strategy,
            cell.preset(),
            m.trades,
            m.win_rate * 100.0,
            m.profit_factor,
            m.expectancy,
            m.total_r,
            m.return_pct
        );
    }
    println!();

    println!("== trade counts by mechanism (a cell under 30 trades is not a result) ==");
    for id in SLICE {
        let counts: Vec<usize> = rows.iter().filter(|(c, _)| c.strategy == id).map(|(_, m)| m.trades).collect();
        let (lo, hi) = (counts.iter().copied().min().unwrap_or(0), counts.iter().copied().max().unwrap_or(0));
        let total: usize = counts.iter().sum();
        println!("{id:<20} {} cells, trades {lo}..{hi} (sum {total})", counts.len());
    }
    println!();

    println!("== the mechanism's own walk-forward over the same window ({folds} folds, select_by {select_by:?}) ==");
    println!("{:<20} {:>7} {:>7} {:>8} {:>8} {:>9}  verdict", "strategy", "trades", "win%", "profit", "expect", "return%");
    for id in SLICE {
        let Ok(strategy) = registry.get(id) else { continue };
        match walk_forward_guarded(strategy, bars, rules, None, folds, select_by, min_trades_per_cell, guards) {
            Some(result) => {
                let m = &result.oos;
                let v = fd_backtest::sweep::verdict(m, gate);
                println!(
                    "{id:<20} {:>7} {:>6.1}% {:>8.3} {:>8.3} {:>8.2}%  {}",
                    m.trades,
                    m.win_rate * 100.0,
                    m.profit_factor,
                    m.expectancy,
                    m.return_pct,
                    if v.promising { "PASS".to_string() } else { format!("fail: {}", v.reasons.join("; ")) }
                );
                // Fold by fold, because "zero trades" has two different
                // meanings — nothing was selectable on the training window, or
                // something was selected and found no setup on the test one —
                // and only the second is the mechanism's own thinness.
                for fold in &result.folds {
                    match (&fold.selected, &fold.test) {
                        (Some(params), Some(t)) => println!(
                            "   fold {}: selected {} on {} training trades -> {} test trades, PF {:.3}, expectancy {:.3}R",
                            fold.fold,
                            params.0.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(","),
                            fold.train_trades,
                            t.trades,
                            t.profit_factor,
                            t.expectancy
                        ),
                        _ => println!(
                            "   fold {}: {}",
                            fold.fold,
                            fold.reason.clone().unwrap_or_else(|| "no measurement".to_string())
                        ),
                    }
                }
            }
            None => println!("{id:<20} not enough bars"),
        }
    }
    println!();

    let top: Vec<&(Cell, fd_backtest::engine::Metrics)> = ordered.iter().take(5).map(|i| &rows[*i]).collect();
    println!("== top five of the slice by in-sample return, carried forward unchanged ==");
    for (rank, (cell, m)) in top.iter().enumerate() {
        println!(
            "{}. {} {}  return {:.2}%  trades {}  PF {:.3}  expectancy {:.3}R",
            rank + 1,
            cell.strategy,
            cell.preset(),
            m.return_pct,
            m.trades,
            m.profit_factor,
            m.expectancy
        );
    }
    println!();

    let out = arg("cells-out", "");
    if !out.is_empty() {
        let text: String = top.iter().map(|(c, _)| format!("{} {}\n", c.strategy, c.preset())).collect();
        match std::fs::write(&out, text) {
            Ok(()) => println!("carried-forward cells written to {out}"),
            Err(e) => println!("could not write {out}: {e}"),
        }
    }
    let _ = top.iter().map(|(c, _)| &c.params).count();
}

/// The named cells replayed on this window at their registered parameters.
fn stage_oos(
    registry: &Registry,
    bars: &[Bar],
    rules: &fd_backtest::engine::TradingRules,
    guards: Option<&Guards>,
    gate: &PromisingGate,
) {
    let path = arg("cells", "");
    let seeds: usize = arg("seeds", "200").parse().unwrap_or(200);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            println!("--cells={path} could not be read: {e}");
            return;
        }
    };

    println!("== out of sample: the same cells, no re-fit, {seeds} matched-null seeds ==");
    println!("legs: expectancy > 0, profit factor >= 1.2, percentile >= 95 of the matched null");
    println!();

    let mut survivors = 0usize;
    let mut considered = 0usize;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (base, preset) = line.split_once(' ').unwrap_or((line, ""));
        let overrides: Vec<(String, f64)> = preset
            .split(',')
            .filter(|s| !s.is_empty())
            .filter_map(|pair| pair.split_once('=').and_then(|(k, v)| v.parse().ok().map(|v| (k.to_string(), v))))
            .collect();
        let hypothesis = Hypothesis {
            label: format!("{base} {preset}"),
            base: base.to_string(),
            filters: Vec::new(),
            overrides,
            why: "top-five cell of the three-month in-sample search, slice 3".to_string(),
        };
        considered += 1;
        match run_hypothesis_fixed_guarded(registry, &hypothesis, bars, rules, gate, seeds, guards) {
            Err(e) => println!("{:<44} could not run: {e}", hypothesis.label),
            Ok(report) => {
                let m = &report.oos;
                let leg_exp = m.expectancy.is_finite() && m.expectancy > 0.0;
                let leg_pf = m.profit_factor.is_finite() && m.profit_factor >= 1.2;
                let leg_null = report.percentile.is_finite() && report.percentile >= 95.0;
                let all = leg_exp && leg_pf && leg_null;
                if all {
                    survivors += 1;
                }
                println!("-- {}", hypothesis.label);
                println!(
                    "   trades {}  win {:.1}%  PF {:.3}  expectancy {:.3}R  totalR {:.2}  return {:.2}%  maxDD ${:.0}",
                    m.trades,
                    m.win_rate * 100.0,
                    m.profit_factor,
                    m.expectancy,
                    m.total_r,
                    m.return_pct,
                    m.max_drawdown_usd
                );
                println!(
                    "   matched null: {} runs, 95th {:.3}, median trades {:.0}, count match {:.2} ({}), method at the {:.0}th",
                    report.null_pf.len(),
                    report.null_quantile(0.95),
                    fd_backtest::hypotheses::median_count(&report.null_trades),
                    report.count_match(),
                    if report.count_matched() { "inside the band" } else { "OUTSIDE the band" },
                    report.percentile
                );
                println!(
                    "   legs: expectancy>0 {}  PF>=1.2 {}  >=95th {}  => {}",
                    if leg_exp { "yes" } else { "no" },
                    if leg_pf { "yes" } else { "no" },
                    if leg_null { "yes" } else { "no" },
                    if all { "SURVIVES" } else { "refuted" }
                );
                println!(
                    "   standing gate: {}",
                    if report.verdict.promising { "PASS".to_string() } else { format!("fail: {}", report.verdict.reasons.join("; ")) }
                );
                if m.trades < gate.min_trades {
                    println!("   UNSCOREABLE: {} trades is under the {}-trade floor", m.trades, gate.min_trades);
                }
                println!("   {}", report.guard_activity());
                println!();
            }
        }
    }
    println!("{survivors} of {considered} cells cleared all three legs.");
}
