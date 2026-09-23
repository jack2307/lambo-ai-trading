//! The three-month search of `docs/hypotheses/2026-09-23-three-month-search.md`,
//! one row per **cell**, where a cell is one point of a mechanism's registry
//! grid with every swept axis pinned to it.
//!
//! Why this exists rather than an existing mode. `--mode=sweep` prints a
//! summary per strategy and never a cell; `--mode=wf` prints one row per
//! strategy and chooses the parameters itself, so it has no single parameter
//! set to carry to another window. The registration's falsifier needs both: a
//! ranking over cells in sample, and the *same parameters, no re-fit* out of
//! sample. So the cell is pinned with [`Preset`] — exactly the mechanism a
//! hypothesis preset uses, which is why the grid loses the pinned axes — and
//! the two stages are:
//!
//! * `--stage=in` — every cell of every named mechanism, walk-forward with the
//!   configured fold count, ranked by the walk-forward return over the window.
//!   The whole-window replay at the same cell is printed beside it, because the
//!   owner's question is about a return over three months and the walk-forward
//!   book covers only the folds' test windows.
//! * `--stage=oos` — a batch file of cells (the same TOML `--batch-file`
//!   format the hypotheses mode reads), each **replayed** at its parameters
//!   over the window with no folds and no selection, against the matched null.
//!   This calls `run_hypothesis_fixed_guarded`, the audited path, and adds only
//!   the return column and the hold-null flag; `search --mode=hypotheses
//!   --fixed --batch-file=<same file>` reproduces every column it shares.
//!
//! **The hold null is flagged, not quoted.** A mechanism with a self-managed
//! exit whose grid is empty — which is every self-managed mechanism once its
//! cell is pinned — is measured against `RandomHold`, and that control is not
//! count-matched (`docs/decisions/2026-09-23-matched-null-repair.md` names this
//! as the second defect, untouched by that repair). Such a row prints
//! `UNMEASURED` for the percentile and the achieved count ratio beside it. It
//! is not dropped and the number is not quoted.
//!
//! No threshold is read from anywhere but the config, and nothing here can move
//! one.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin three_month -- \
//!   --market=xauusd --interval=15m --data=E:/rust/flowdesk/data --guards \
//!   --stage=in --from=2026-06-23 --to=2026-09-23 \
//!   --strategies=trend-pullback,volume-thrust,volman-box,tsmom,intraday-momentum,session-hold
//! ```

use std::path::PathBuf;

use fd_backtest::engine::{Range, TradingRules, run_backtest_guarded, trading_rules_for};
use fd_backtest::hypotheses::{
    COUNT_MATCH_BAND, Preset, batch_from_file, median_count, run_hypothesis_fixed_guarded,
};
use fd_backtest::sweep::{SelectBy, walk_forward_guarded};
use fd_backtest::{Guards, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;
use fd_strategy::registry::{Exits, Params, Registry, Strategy, parameter_combinations};

/// Where the scheduled-news calendar lives under the data directory, spelled
/// the way `fd_strategy::news::summary` wants it for the header line.
const NEWS_FILE: &str = "data/news/events.parquet";

/// Install the news calendar, as `search` does, and say what happened.
///
/// This is not optional decoration. `[trading.guards]` carries a news flat
/// (60/30 minutes, impact ≥ 3, USD) and that guard reads the globally installed
/// calendar; without this call the guard is **inert** while the header still
/// says it is on. The first slice-4 run was made without it and the receipt
/// `slice-4-in-sample-without-news-calendar.txt` is kept to show the size of
/// the difference: 917 trades against 910 on one cell, and a profit factor in
/// the second decimal.
fn load_news(data: &std::path::Path) -> String {
    let path = data.join("news").join("events.parquet");
    if path.is_file() {
        match fd_store::read_news(&path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            Ok(_) => {}
            Err(e) => println!("news: could not load {}: {e}", path.display()),
        }
    }
    fd_strategy::news::summary(NEWS_FILE)
}

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// `--from=YYYY-MM-DD` as epoch milliseconds, as `search` reads it.
fn bound(name: &str) -> Option<i64> {
    let text = arg(name, "");
    if text.is_empty() {
        return None;
    }
    let mut parts = text.split('-').map(|p| p.parse::<i64>());
    let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000)
}

/// `2026-06-23 00:00Z`, enough for a receipt header to say which bars ran.
fn iso(ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(DAY_MS));
    let rest = ms.rem_euclid(DAY_MS);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}Z", rest / 3_600_000, (rest % 3_600_000) / 60_000)
}

/// One cell, measured in sample.
struct Row {
    strategy: String,
    cell: String,
    /// Walk-forward, out-of-sample books concatenated. `None` when no fold
    /// could select: with the cell pinned that means the cell never reached
    /// `min_trades_per_cell` on a training window.
    wf_return: Option<f64>,
    wf_trades: usize,
    wf_pf: f64,
    wf_expectancy: f64,
    scored_folds: usize,
    /// The same cell replayed over the whole window, no folds.
    whole_return: f64,
    whole_trades: usize,
    whole_pf: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "xauusd");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;
    let interval = arg("interval", &config.backtest.timeframe);
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let mut bars = read_bars(&bars_path)?;

    let (from, to) = (bound("from"), bound("to"));
    let before = bars.len();
    bars.retain(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t));
    println!("window:   --from={} --to={} kept {} of {before} bars", arg("from", "…"), arg("to", "…"), bars.len());
    if bars.is_empty() {
        println!("no bars at {} after the bounds — nothing to run", bars_path.display());
        return Ok(());
    }
    println!("bars:     {} from {} to {}", bars.len(), iso(bars[0].time), iso(bars[bars.len() - 1].time));

    let rules: TradingRules = trading_rules_for(&config, &market)?;
    println!("spread:   {} per round trip (configured, not overridden)", rules.spread);
    println!("equity:   {} USD starting, risk {}% per trade", rules.starting_equity_usd, rules.risk_per_trade_pct * 100.0);
    println!("trail:    {}", if rules.trail.enabled { "ON" } else { "off" });
    println!("{}", load_news(&data));
    println!(
        "news scope: {}",
        if rules.news_currencies.is_empty() {
            "every currency (news_currencies is empty)".to_string()
        } else {
            format!("{} (news_currencies)", rules.news_currencies.join("|"))
        }
    );
    let guards = std::env::args().any(|a| a == "--guards").then(|| Guards::for_market(&config, &market)).transpose()?;
    let guards = guards.as_ref();
    println!(
        "guards:   {}",
        guards.map_or_else(|| "off (every number unguarded)".to_string(), |g| format!("on — {}", g.describe()))
    );
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by =
        if config.backtest.select_by == "profitFactor" { SelectBy::ProfitFactor } else { SelectBy::Expectancy };
    println!(
        "config:   folds {} · select_by {} · min_trades_per_cell {} · gate trades≥{} PF≥{} expectancy≥{}",
        config.backtest.walk_forward_folds,
        config.backtest.select_by,
        config.backtest.min_trades_per_cell,
        gate.min_trades,
        gate.min_profit_factor,
        gate.min_expectancy_r
    );
    println!("filters:  none — the bare mechanisms and the registry's own grids");
    println!();

    let registry = Registry::with_builtins();
    match arg("stage", "in").as_str() {
        "in" => in_sample(&registry, &bars, &rules, &config, select_by, guards),
        "oos" => out_of_sample(&registry, &bars, &rules, &gate, guards),
        other => {
            println!("unknown --stage={other} (want `in` or `oos`)");
            Ok(())
        }
    }
}

/// Every cell of every named mechanism, walk-forward, ranked by return.
fn in_sample(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    config: &Config,
    select_by: SelectBy,
    guards: Option<&Guards>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ids: Vec<String> = arg("strategies", "").split(',').filter(|s| !s.trim().is_empty()).map(str::to_string).collect();
    if ids.is_empty() {
        println!("--strategies=<id,id,…> is required for --stage=in");
        return Ok(());
    }
    let folds = config.backtest.walk_forward_folds;
    let min_trades_per_cell = config.backtest.min_trades_per_cell;

    println!("== in sample: every cell of {} mechanisms, walk-forward ({folds} folds) ==", ids.len());
    println!("ranked by the walk-forward return over the window; `whole` is the same cell replayed over every bar");
    println!("a cell that took no trade has no return, so it ranks last and is listed as unscoreable with its counts");
    println!();

    let mut rows: Vec<Row> = Vec::new();
    let mut per_strategy: Vec<(String, usize)> = Vec::new();
    for id in &ids {
        let base = registry.get(id)?;
        let grid = base.grid();
        let combos = parameter_combinations(&base.default_params(), &grid);
        per_strategy.push((id.clone(), combos.len()));
        for cell in &combos {
            // Pin exactly the swept axes at this cell's coordinates. The other
            // parameters are the method's own defaults and no sweep touches
            // them.
            let overrides: Vec<(String, f64)> = grid.keys().map(|k| (k.clone(), cell.get(k))).collect();
            let preset = Preset::new(base, &overrides)?;
            let wf = walk_forward_guarded(&preset, bars, rules, None, folds, select_by, min_trades_per_cell, guards);
            let whole =
                run_backtest_guarded(bars, &preset, &preset.defaults, rules, guards, None, Range::default(), None);
            let (wf_return, wf_trades, wf_pf, wf_expectancy, scored_folds) = match &wf {
                Some(r) => (
                    Some(r.oos.return_pct),
                    r.oos.trades,
                    r.oos.profit_factor,
                    r.oos.expectancy,
                    r.folds.iter().filter(|f| f.test.is_some()).count(),
                ),
                None => (None, 0, f64::NAN, f64::NAN, 0),
            };
            rows.push(Row {
                strategy: id.clone(),
                cell: describe_cell(&grid.keys().cloned().collect::<Vec<_>>(), cell),
                wf_return,
                wf_trades,
                wf_pf,
                wf_expectancy,
                scored_folds,
                whole_return: whole.metrics.return_pct,
                whole_trades: whole.metrics.trades,
                whole_pf: whole.metrics.profit_factor,
            });
        }
    }

    // A cell that took no trade has **no return**, and `metrics_of` reports an
    // empty book as 0.00% — which would rank every mechanism that never fired
    // above every mechanism that fired and lost. Those cells are ranked last
    // and listed below as unscoreable, with their counts. This is a statement
    // about what a return is, not a filter anyone loosened: no cell is dropped
    // and every count is printed.
    rows.sort_by(|a, b| {
        let key = |r: &Row| {
            if r.wf_trades == 0 { f64::NEG_INFINITY } else { r.wf_return.filter(|v| v.is_finite()).unwrap_or(f64::NEG_INFINITY) }
        };
        key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "{:>4} {:<19} {:<28} {:>7} {:>9} {:>7} {:>8} {:>6}  {:>7} {:>9} {:>7}",
        "rank", "strategy", "cell", "wfTrd", "wfRet%", "wfPF", "wfExp", "folds", "whlTrd", "whlRet%", "whlPF"
    );
    for (i, r) in rows.iter().enumerate() {
        println!(
            "{:>4} {:<19} {:<28} {:>7} {:>9} {:>7} {:>8} {:>6}  {:>7} {:>9.2} {:>7}",
            i + 1,
            r.strategy,
            r.cell,
            r.wf_trades,
            if r.wf_trades == 0 {
                "no book".to_string()
            } else {
                r.wf_return.map_or_else(|| "no fold".to_string(), |v| format!("{v:.2}"))
            },
            fmt(r.wf_pf),
            fmt(r.wf_expectancy),
            r.scored_folds,
            r.whole_trades,
            r.whole_return,
            fmt(r.whole_pf),
        );
    }
    println!();
    println!("cells swept, per mechanism:");
    for (id, n) in &per_strategy {
        println!("  {id:<19} {n:>4}");
    }
    println!("  {:<19} {:>4}  ← N, the number `the best of N` is meaningless without", "TOTAL", rows.len());
    println!();
    let unscoreable: Vec<&Row> = rows.iter().filter(|r| r.wf_return.is_none() || r.wf_trades == 0).collect();
    if unscoreable.is_empty() {
        println!("every cell produced a walk-forward book.");
    } else {
        println!("cells with no walk-forward book at all (a filter was NOT loosened to make one):");
        for r in &unscoreable {
            println!(
                "  {:<19} {:<28} folds scored {} · whole-window trades {}",
                r.strategy, r.cell, r.scored_folds, r.whole_trades
            );
        }
    }
    println!();
    println!("thin books, which the registration says to report rather than quote:");
    for r in rows.iter().filter(|r| r.wf_trades > 0 && r.wf_trades < 30) {
        println!("  {:<19} {:<28} {} walk-forward trades", r.strategy, r.cell, r.wf_trades);
    }
    println!();
    Ok(())
}

/// The carried-forward cells, replayed on the window they were not chosen on.
fn out_of_sample(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = arg("batch-file", "");
    if path.is_empty() {
        println!("--batch-file=<toml> is required for --stage=oos");
        return Ok(());
    }
    let seeds: usize = arg("seeds", "200").parse().unwrap_or(200);
    let batch = batch_from_file(std::path::Path::new(&path)).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    println!("== out of sample: {} cells REPLAYED at their parameters, no folds, no selection ==", batch.len());
    println!("each against {seeds} matched-null runs; the three legs are expectancy > 0, PF ≥ 1.2, percentile ≥ 95");
    println!("nothing is re-fitted here: the parameters are the ones the three-month search chose");
    println!();
    println!(
        "{:<26} {:<19} {:>7} {:>9} {:>7} {:>8} {:>9} {:>9} {:>11} {:>6}",
        "cell", "base", "trades", "return%", "PF", "expect", "null p50", "null p95", "pct", "cm"
    );

    let mut survived = Vec::new();
    for h in &batch {
        let base = registry.get(&h.base)?;
        let preset = Preset::new(base, &h.overrides)?;
        // The same test `hypotheses.rs` makes: a self-managed exit with an
        // empty grid is a drift claim and is read against random *holds*. With
        // the cell pinned every self-managed mechanism lands here, and the hold
        // null is not count-matched.
        let hold_null = base.exits() == Exits::Strategy && preset.grid().is_empty();
        let report = run_hypothesis_fixed_guarded(registry, h, bars, rules, gate, seeds, guards)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let m = &report.oos;
        let ratio = report.count_match();
        println!(
            "{:<26} {:<19} {:>7} {:>9.2} {:>7} {:>8} {:>9} {:>9} {:>11} {:>6}",
            h.label,
            h.base,
            m.trades,
            m.return_pct,
            fmt(m.profit_factor),
            fmt(m.expectancy),
            fmt(report.null_quantile(0.5)),
            fmt(report.null_quantile(0.95)),
            if hold_null { "UNMEASURED".to_string() } else { format!("{:.0}%", report.percentile) },
            if ratio.is_finite() { format!("{ratio:.2}") } else { "—".to_string() },
        );
        let leg1 = m.expectancy.is_finite() && m.expectancy > 0.0;
        let leg2 = m.profit_factor.is_finite() && m.profit_factor >= gate.min_profit_factor;
        let leg3 = if hold_null { None } else { Some(report.percentile.is_finite() && report.percentile >= 95.0) };
        println!(
            "{:<26} {:<19} expectancy > 0: {} · PF ≥ {}: {} · ≥ 95th of its matched null: {}",
            "",
            "",
            if leg1 { "pass" } else { "FAIL" },
            gate.min_profit_factor,
            if leg2 { "pass" } else { "FAIL" },
            match leg3 {
                Some(true) => "pass".to_string(),
                Some(false) => format!("FAIL (percentile {:.0}, against 95)", report.percentile),
                None => format!(
                    "UNMEASURED — hold null, not count-matched; control took a median of {:.0} trades against this cell's {} (ratio {})",
                    median_count(&report.null_trades),
                    m.trades,
                    if ratio.is_finite() { format!("{ratio:.2}") } else { "—".to_string() }
                ),
            }
        );
        if !hold_null && !report.count_matched() {
            println!(
                "{:<26} {:<19} ** the count match is {:.2}, outside 1 ± {COUNT_MATCH_BAND}: this percentile is not this cell's size **",
                "", "", ratio
            );
        }
        println!(
            "{:<26} {:<19} registry verdict (stricter than the three legs): {}",
            "",
            "",
            if report.verdict.promising { "promising".to_string() } else { report.verdict.reasons.join("; ") }
        );
        if guards.is_some() {
            println!("{:<26} {:<19} {}", "", "", report.guard_activity());
        }
        if leg1 && leg2 && leg3 == Some(true) {
            survived.push(h.label.clone());
        }
    }
    println!();
    if survived.is_empty() {
        println!("Nothing cleared all three legs. That is the result of the registration, not a prompt to widen anything.");
    } else {
        println!("Cleared all three legs: {}", survived.join(", "));
    }
    println!();
    Ok(())
}

fn describe_cell(keys: &[String], cell: &Params) -> String {
    if keys.is_empty() {
        return "(no grid — one cell)".to_string();
    }
    keys.iter().map(|k| format!("{k}={}", trim(cell.get(k)))).collect::<Vec<_>>().join(" ")
}

/// A grid value the way the grid writes it: `20` not `20.0000`.
fn trim(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") }
}

fn fmt(v: f64) -> String {
    if v.is_finite() { format!("{v:.3}") } else if v.is_infinite() { "inf".to_string() } else { "—".to_string() }
}
