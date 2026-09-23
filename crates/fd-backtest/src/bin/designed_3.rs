//! The design-window measurement for angle 3 of
//! `docs/hypotheses/2026-09-23-designed-methods.md` (lower frequency).
//!
//! It runs ONE method at ONE frozen parameter set through the ordinary guarded
//! backtest path — `run_hypothesis_fixed_guarded`, the same function the
//! three-month search's out-of-sample leg calls — so the numbers are
//! comparable to the record rather than to a private harness. It adds nothing
//! to the gate and moves no threshold.
//!
//! Three runs per invocation, because the brief asks what the weekend flat
//! costs and that cannot be read off a single number:
//!
//! 1. **guards on**, exactly as configured (`Guards::for_market`);
//! 2. **guards on with the weekend flat off** — the *same* Guards struct with
//!    `flat_before_weekend_hhmm = 0` and nothing else touched, which isolates
//!    the Friday cut-off from the news windows and the open-loss cap;
//! 3. **guards off** entirely, for the record's older, unguarded convention.
//!
//! The header prints the data root and the window bounds. The registration for
//! this program withholds a year of bars from the designers, and a receipt that
//! does not say which store it read proves nothing about that.
//!
//! ```text
//! designed_3 --market=xauusd --interval=15m --data=E:/rust/flowdesk/data-sealed \
//!   --from=2022-06-16 --to=2025-09-23 --seeds=200
//! ```

use std::path::PathBuf;

use fd_backtest::direction::{permuted_sides_pnls, profit_factor_of};
use fd_backtest::engine::{TradingRules, trading_rules_for};
use fd_backtest::hypotheses::{Hypothesis, median_count, run_hypothesis_fixed_guarded};
use fd_backtest::{Guards, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;
use fd_strategy::registry::{Registry, Strategy};

const DAY_MS: i64 = 86_400_000;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn bound(name: &str) -> Option<i64> {
    let text = arg(name, "");
    if text.is_empty() {
        return None;
    }
    let mut parts = text.split('-').map(|p| p.parse::<i64>());
    let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * DAY_MS)
}

/// UTC `YYYY-MM-DD HH:MM`, the rendering every receipt in `docs/research/runs/` uses.
fn iso(ms: i64) -> String {
    let days = ms.div_euclid(DAY_MS);
    let rest = ms.rem_euclid(DAY_MS);
    let (y, m, d) = fd_core::clock::civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60)
}

/// One measured run, printed as a block.
#[allow(clippy::too_many_arguments)]
fn measure(
    label: &str,
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
) -> Result<(), String> {
    let report = run_hypothesis_fixed_guarded(registry, hypothesis, bars, rules, gate, seeds, guards)?;
    let m = &report.oos;
    println!("-- {label} --");
    println!(
        "   trades {}  expectancy {:+.4} R  profit factor {:.3}  return {:+.2} %  win rate {:.1} %",
        m.trades, m.expectancy, m.profit_factor, m.return_pct, m.win_rate * 100.0
    );
    if m.trades == 0 {
        println!("   NO TRADES: this row has no return, not a return of zero, and sorts below every row that traded.");
        println!();
        return Ok(());
    }
    println!(
        "   mean hold {:.2} days  max drawdown {:.2} %  total R {:+.2}",
        m.avg_hold_min / (24.0 * 60.0),
        m.max_drawdown_pct,
        m.total_r
    );
    println!(
        "   drift null (RandomHold, {seeds} seeds): 95th percentile PF {:.3}, method at the {:.0}th; \
         null median {:.0} trades vs the method's {} (count match {:.2}{})",
        report.null_quantile(0.95),
        report.percentile,
        median_count(&report.null_trades),
        m.trades,
        report.count_match(),
        if report.count_matched() { "" } else { ", OUTSIDE the band: this percentile is unmatched" },
    );
    let logs: Vec<f64> = run_holds(bars, hypothesis, rules, guards, registry)
        .into_iter()
        .filter(|m| *m > 0.0)
        .map(f64::ln)
        .collect();
    if !logs.is_empty() {
        let mean = logs.iter().sum::<f64>() / logs.len() as f64;
        let sd = (logs.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / logs.len() as f64).sqrt();
        println!(
            "   hold distribution the null is built from: geometric-mean {:.1} min ({:.2} days), sd of ln {:.2};              shortest {:.0} min, longest {:.1} days",
            mean.exp(),
            mean.exp() / 1440.0,
            sd,
            logs.iter().copied().fold(f64::INFINITY, f64::min).exp(),
            logs.iter().copied().fold(f64::NEG_INFINITY, f64::max).exp() / 1440.0,
        );
    }
    println!("   {}", report.guard_activity());
    println!(
        "   gate: trades>={} {} | PF>={} {} | expectancy>={} {} | 95th percentile {}",
        gate.min_trades,
        pass(m.trades >= gate.min_trades),
        gate.min_profit_factor,
        pass(m.profit_factor >= gate.min_profit_factor),
        gate.min_expectancy_r,
        pass(m.expectancy >= gate.min_expectancy_r),
        pass(report.percentile >= 95.0),
    );
    println!();
    Ok(())
}

/// The realised holds, in minutes, of the same run the report measured.
fn run_holds(
    bars: &[Bar],
    hypothesis: &Hypothesis,
    rules: &TradingRules,
    guards: Option<&Guards>,
    registry: &Registry,
) -> Vec<f64> {
    use fd_backtest::engine::{Range, run_backtest_guarded};
    use fd_backtest::hypotheses::Preset;
    let Ok(base) = registry.get(&hypothesis.base) else { return Vec::new() };
    let Ok(preset) = Preset::new(base, &hypothesis.overrides) else { return Vec::new() };
    let run = run_backtest_guarded(bars, &preset, &preset.defaults, rules, guards, None, Range::default(), None);
    run.trades.iter().map(|t| (t.exit_time - t.entry_time) as f64 / 60_000.0).collect()
}

fn pass(ok: bool) -> &'static str {
    if ok { "pass" } else { "FAIL" }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "xauusd");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;
    let interval = arg("interval", &config.backtest.timeframe);
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let all = read_bars(&bars_path)?;
    if all.is_empty() {
        println!("no bars at {}", bars_path.display());
        return Ok(());
    }
    let (from, to) = (bound("from"), bound("to"));
    let bars: Vec<Bar> = all
        .iter()
        .filter(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t))
        .cloned()
        .collect();

    let rules = trading_rules_for(&config, &market)?;
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let seeds: usize = arg("seeds", "200").parse().unwrap_or(200);

    println!("== designed-3: quiet-tape multi-day continuation, frozen parameters ==");
    println!("DATA ROOT:  {}", data.display());
    println!("bars file:  {}", bars_path.display());
    println!("market:     {market} {interval}, spread {} per round trip, contract {}", rules.spread, rules.contract_size);
    println!("equity:     {} {} starting", rules.starting_equity_usd, rules.account_currency);
    println!(
        "store span: {} bars, {} -> {}",
        all.len(),
        iso(all[0].time),
        iso(all[all.len() - 1].time)
    );
    if bars.is_empty() {
        println!("window:     NO BARS in {} -> {}", arg("from", "…"), arg("to", "…"));
        return Ok(());
    }
    println!(
        "window:     {} bars, {} -> {} (asked {} -> {}, `to` exclusive)",
        bars.len(),
        iso(bars[0].time),
        iso(bars[bars.len() - 1].time),
        arg("from", "…"),
        arg("to", "…"),
    );
    let news_path = data.join("news").join("events.parquet");
    if news_path.is_file() {
        fd_store::read_news(&news_path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install)?;
    }
    println!("news:       {}", fd_strategy::news::summary(&news_path.display().to_string()));

    let registry = Registry::with_builtins();
    let base = registry.get("quiet-swing")?;
    // Every parameter named, so `Preset::new` pins each one and no sweep can
    // put a grid value back. This IS the frozen set; it is the same list as
    // `docs/research/designs/2026-09-23-designed-3-frozen.toml`.
    let overrides: Vec<(String, f64)> = vec![
        ("lookbackSessions".to_string(), 10.0),
        ("windowSessions".to_string(), 60.0),
        ("quietPct".to_string(), 0.50),
        ("holdSessions".to_string(), 5.0),
        ("riskDailyRanges".to_string(), 1.5),
        ("rangeDays".to_string(), 20.0),
        ("atrPeriod".to_string(), 14.0),
    ];
    println!(
        "frozen:     {}",
        overrides.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
    );
    println!("exits:      {:?} (the engine's 4-hour max_hold_ms does not apply)", base.exits());
    println!("gate:       {} trades / PF {} / {} R, both nulls at the 95th", gate.min_trades, gate.min_profit_factor, gate.min_expectancy_r);
    println!();

    let hypothesis = Hypothesis {
        label: "quiet-swing/frozen".to_string(),
        base: "quiet-swing".to_string(),
        filters: Vec::new(),
        overrides,
        why: "angle 3: a multi-day hold makes the stop tens of points against the same 0.28 spread, \
              and the continuation it trades is measured only where realised range is low"
            .to_string(),
    };

    let full = Guards::for_market(&config, &market)?;
    println!("guards on:  {}", full.describe());
    let mut no_weekend = full.clone();
    no_weekend.flat_before_weekend_hhmm = 0;
    println!();

    measure("guards on, as configured", &registry, &hypothesis, &bars, &rules, &gate, seeds, Some(&full))?;
    measure("guards on, WEEKEND FLAT OFF (everything else identical)", &registry, &hypothesis, &bars, &rules, &gate, seeds, Some(&no_weekend))?;
    measure("guards off (the record's pre-2026-09-14 convention)", &registry, &hypothesis, &bars, &rules, &gate, seeds, None)?;

    /* ---- the direction null, and a long-only control for the drift ---- */

    // Re-run the guarded method once to get its trades for the two extra
    // controls, which both work off the trade list rather than the bars.
    use fd_backtest::engine::{Range, run_backtest_guarded};
    use fd_backtest::hypotheses::Preset;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let run = run_backtest_guarded(&bars, &preset, &preset.defaults, &rules, Some(&full), None, Range::default(), None);
    let pf = run.metrics.profit_factor;
    let mut dir: Vec<f64> = (1..=seeds as u64)
        .map(|seed| profit_factor_of(&permuted_sides_pnls(&run.trades, &rules, seed)))
        .filter(|v| v.is_finite())
        .collect();
    dir.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if dir.is_empty() {
        println!("-- direction null -- no finite seed: nothing to report");
    } else {
        let q95 = dir[(((dir.len() - 1) as f64) * 0.95).round() as usize];
        let pct = 100.0 * dir.iter().filter(|v| **v < pf).count() as f64 / dir.len() as f64;
        println!("-- direction null (same trades, side by coin flip, {} finite seeds) --", dir.len());
        println!("   method PF {pf:.3}; null 95th percentile {q95:.3}; method at the {pct:.0}th");
    }


    /* ---- the drift null with its ENTRY RATE matched as well as its hold ---- */

    // `matched_control_for` sets the control's `entryRate` from
    // `matched_rate`, but `run_hypothesis_fixed_guarded` computes that rate
    // only on the NON-drift branch: `let rate = (!drift).then(|| ...)`. So a
    // drift null keeps `RandomHold`'s default `entryRate = 1.0` and re-enters
    // on every bar it is flat — matched in hold LENGTH and unmatched in
    // FREQUENCY. `tsmom` is always in the market so the two coincide there;
    // a method that trades in a slice of the sessions gets a null with
    // several times its trade count, and `count_matched()` says so above.
    //
    // Nothing shared is changed here. The same control is simply run again at
    // an entry rate calibrated the way `matched_rate` calibrates the other
    // one, so the percentile can be read at all. Both figures are published.
    {
        use fd_backtest::RandomHold;
        let target = run.trades.len();
        // The same statistic `hypotheses::hold_distribution` computes, spelled
        // out here rather than reached for, so nothing shared has to change:
        // the geometric mean of the realised holds and the sd of their logs.
        let logs: Vec<f64> = run
            .trades
            .iter()
            .map(|t| (t.exit_time - t.entry_time) as f64 / 60_000.0)
            .filter(|m| *m > 0.0)
            .map(f64::ln)
            .collect();
        let mean = logs.iter().sum::<f64>() / logs.len().max(1) as f64;
        let log_sd = (logs.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / logs.len().max(1) as f64).sqrt();
        let hold_min = mean.exp();
        let params_at = |rate: f64, seed: f64| {
            let mut p = RandomHold.default_params();
            p.set("holdMinutes", hold_min.round());
            p.set("holdLogSd", log_sd);
            p.set("riskDailyRanges", 1.5);
            p.set("rangeDays", 20.0);
            p.set("entryRate", rate);
            p.set("seed", seed);
            p
        };
        let count_at = |rate: f64| -> f64 {
            let mut counts: Vec<usize> = (1..=9u64)
                .map(|seed| {
                    let p = params_at(rate, seed as f64);
                    run_backtest_guarded(&bars, &RandomHold, &p, &rules, Some(&full), None, Range::default(), None)
                        .trades
                        .len()
                })
                .collect();
            counts.sort_unstable();
            counts[counts.len() / 2] as f64
        };
        // Bisect on log(rate): the count is monotone in the rate.
        let (mut lo, mut hi) = (0.0005f64, 1.0f64);
        for _ in 0..18 {
            let mid = ((lo.ln() + hi.ln()) * 0.5).exp();
            if count_at(mid) > target as f64 { hi = mid } else { lo = mid }
        }
        let rate = ((lo.ln() + hi.ln()) * 0.5).exp();
        let mut pfs: Vec<(f64, usize)> = (1..=seeds as u64)
            .map(|seed| {
                let p = params_at(rate, seed as f64);
                let r = run_backtest_guarded(&bars, &RandomHold, &p, &rules, Some(&full), None, Range::default(), None);
                (r.metrics.profit_factor, r.metrics.trades)
            })
            .filter(|(pf, _)| pf.is_finite())
            .collect();
        pfs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut counts: Vec<usize> = pfs.iter().map(|p| p.1).collect();
        counts.sort_unstable();
        let q95 = pfs[(((pfs.len() - 1) as f64) * 0.95).round() as usize].0;
        let pct = 100.0 * pfs.iter().filter(|(v, _)| *v < pf).count() as f64 / pfs.len() as f64;
        let med = median_count(&counts);
        println!();
        println!("-- drift null, entry rate calibrated to the trade count ({} finite seeds) --", pfs.len());
        println!(
            "   entryRate {rate:.5}; null median {med:.0} trades vs the method's {target} (count match {:.2});              95th percentile PF {q95:.3}; method PF {pf:.3} at the {pct:.0}th",
            med / target as f64
        );
    }

    // The stop distance, said out loud in points as well as in ATRs. The
    // count-matched `RandomEntry` null is NOT cost-matched — `control_for`
    // leaves its `stopAtr` at 1.5 whatever the method uses — so a wide stop
    // can clear a percentile leg on stop width alone. This method is read
    // against `RandomHold` instead, and that one IS sized like the method
    // (`control_for` copies `riskDailyRanges` and `rangeDays`, which is why
    // those two names were chosen). The distance is printed regardless,
    // because a percentile is not evidence and the arithmetic is.
    let mut risks: Vec<f64> = run.trades.iter().map(|t| (t.entry_price - t.stop).abs()).filter(|v| v.is_finite() && *v > 0.0).collect();
    risks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if !risks.is_empty() {
        let q = |f: f64| risks[(((risks.len() - 1) as f64) * f).round() as usize];
        println!();
        println!("-- the stop distance, which is the whole cost argument --");
        println!(
            "   sizing stop |entry - stop|: p10 {:.2}  median {:.2}  p90 {:.2} USD per ounce",
            q(0.10),
            q(0.50),
            q(0.90)
        );
        println!(
            "   spread {:.2} as a share of R: p10 {:.2} %  median {:.2} %  p90 {:.2} %              (an intraday 2.00 USD stop pays {:.1} %)",
            rules.spread,
            100.0 * rules.spread / q(0.90),
            100.0 * rules.spread / q(0.50),
            100.0 * rules.spread / q(0.10),
            100.0 * rules.spread / 2.0
        );
    }

    // How long-biased was it? A low-frequency method on an instrument with a
    // large positive drift can clear both nulls on its side ratio alone —
    // neither null controls for the instrument's drift — so the share is
    // printed beside the result rather than left to be inferred.
    let longs = run.trades.iter().filter(|t| t.direction.is_long()).count();
    let long_pnl: f64 = run.trades.iter().filter(|t| t.direction.is_long()).map(|t| t.pnl_usd).sum();
    let short_pnl: f64 = run.trades.iter().filter(|t| !t.direction.is_long()).map(|t| t.pnl_usd).sum();
    println!();
    println!("-- side split, because neither null controls for the instrument's own drift --");
    println!(
        "   {longs} long / {} short of {} trades ({:.0} % long); long P&L {:+.2} USD, short P&L {:+.2} USD",
        run.trades.len() - longs,
        run.trades.len(),
        100.0 * longs as f64 / run.trades.len().max(1) as f64,
        long_pnl,
        short_pnl
    );
    println!("   A method whose short leg also pays is not harvesting the drift. One whose short leg");
    println!("   only loses has been measured against two nulls that have no drift exposure at all.");
    Ok(())
}
