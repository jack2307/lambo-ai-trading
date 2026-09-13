//! Diagnostic: the `2026-09-13-close-reopen-drift` rows through the fixed
//! replay, trade by trade, with the numbers a risk review needs and the
//! receipt tables do not carry — net dollars, the top-five share, the worst
//! excursions, the per-year split. Not a receipt: the record quotes
//! `search`'s files under `docs/research/runs/`.
//!
//!     cargo run --release -p fd-backtest --example diag_close -- 1615 1815 MoTuWeTh [xauduka 2018-06-16 2025-04-10]

use fd_backtest::engine::{Range, run_backtest, trading_rules_for};
use fd_backtest::hypotheses::Preset;
use fd_core::config::Config;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::Registry;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from_hhmm: f64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(1615.0);
    let to_hhmm: f64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(1815.0);
    let days = args.get(2).cloned().unwrap_or_else(|| "MoTuWeTh".into());
    let market = args.get(3).cloned().unwrap_or_else(|| "xauduka".into());
    let parse_day = |s: &str| -> i64 {
        let mut it = s.split('-').map(|p| p.parse::<i64>().unwrap());
        let (y, m, d) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000
    };
    let from = args.get(4).map(|s| parse_day(s)).unwrap_or_else(|| parse_day("2018-06-16"));
    let to = args.get(5).map(|s| parse_day(s)).unwrap_or_else(|| parse_day("2025-04-10"));

    let config = Config::load("config")?;
    let spec = config.market(&market)?;
    let path = std::path::PathBuf::from("data/bars").join(format!("{}-15m.parquet", spec.bar_symbol));
    let mut bars = fd_store::read_bars(&path)?;
    bars.retain(|b| b.time >= from && b.time < to);
    let rules = trading_rules_for(&config, &market)?;
    let registry = Registry::with_builtins();
    let base = registry.get("session-hold")?;
    let preset = Preset::new(
        base,
        &[
            ("from".into(), from_hhmm),
            ("to".into(), to_hhmm),
            ("side".into(), 1.0),
            ("riskDailyRanges".into(), 1.0),
            ("rangeDays".into(), 20.0),
        ],
    )?;
    let gate_from = from_hhmm as u32;
    let gate = format!("hours:{:04}-{:04}", gate_from, gate_from + 5);
    let filtered = Filtered {
        inner: &preset,
        filters: vec![Filter::parse(&format!("weekdays:{days}"))?, Filter::parse(&gate)?],
    };
    let fmt = |t: i64| {
        let d = t.div_euclid(86_400_000);
        let (y, m, dd) = fd_core::clock::civil_from_days(d);
        let mins = t.rem_euclid(86_400_000) / 60_000;
        format!("{y}-{m:02}-{dd:02} {:02}:{:02}Z", mins / 60, mins % 60)
    };
    let r = run_backtest(&bars, &filtered, &preset.defaults, &rules, None, Range::default());
    let m = &r.metrics;
    println!(
        "{market} {}-{} {days}: {} trades  PF {:.3}  exp {:.4}R  win {:.1}%  net ${:.0}  return {:.2}%  maxDD ${:.0} ({:.2}%)  avgMAE {:.3}R  avgMFE {:.3}R  hold {:.0} min",
        gate_from, to_hhmm as u32, m.trades, m.profit_factor, m.expectancy, m.win_rate * 100.0, m.net_pnl_usd, m.return_pct, m.max_drawdown_usd, m.max_drawdown_pct, m.avg_mae, m.avg_mfe, m.avg_hold_min
    );
    let mut pnl: Vec<f64> = r.trades.iter().map(|t| t.pnl_usd).collect();
    pnl.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let top5: f64 = pnl.iter().take(5).sum();
    let bottom5: f64 = pnl.iter().rev().take(5).sum();
    let net: f64 = pnl.iter().sum();
    println!(
        "top-5 ${:.0} = {:.1}% of net; PF without top-5 {:.3}; bottom-5 ${:.0}; median |pnl| ${:.2}; mean lots {:.3}",
        top5,
        100.0 * top5 / net,
        {
            let (mut w, mut l) = (0.0, 0.0);
            for p in pnl.iter().skip(5) {
                if *p > 0.0 { w += p } else { l -= p }
            }
            w / l
        },
        bottom5,
        {
            let mut a: Vec<f64> = pnl.iter().map(|p| p.abs()).collect();
            a.sort_by(|x, y| x.partial_cmp(y).unwrap());
            a[a.len() / 2]
        },
        r.trades.iter().map(|t| t.lots).sum::<f64>() / r.trades.len().max(1) as f64
    );
    let mut worst: Vec<&fd_backtest::engine::Trade> = r.trades.iter().collect();
    worst.sort_by(|a, b| a.r.partial_cmp(&b.r).unwrap());
    println!("worst 5 by R:");
    for t in worst.iter().take(5) {
        println!("  {} -> {}  lots {:.3}  R {:+.3}  ${:+.2}  entry {:.2} exit {:.2}", fmt(t.entry_time), fmt(t.exit_time), t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price);
    }
    println!("best 5 by R:");
    for t in worst.iter().rev().take(5) {
        println!("  {} -> {}  lots {:.3}  R {:+.3}  ${:+.2}  entry {:.2} exit {:.2}", fmt(t.entry_time), fmt(t.exit_time), t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price);
    }
    // Per-year split.
    let mut years: std::collections::BTreeMap<i64, (usize, f64, f64, f64)> = Default::default();
    for t in &r.trades {
        let (y, _, _) = fd_core::clock::civil_from_days(t.entry_time.div_euclid(86_400_000));
        let e = years.entry(y).or_default();
        e.0 += 1;
        e.1 += t.pnl_usd;
        if t.pnl_usd > 0.0 { e.2 += t.pnl_usd } else { e.3 -= t.pnl_usd }
    }
    println!("per year: year trades net PF");
    for (y, (n, net, w, l)) in &years {
        println!("  {y} {n:4} ${net:8.0} {:.3}", w / l);
    }
    // Sessions per calendar year and lots per year, for the spread/rebate arithmetic.
    let span_years = (to - from) as f64 / (365.25 * 86_400_000.0);
    let lots_year = r.trades.iter().map(|t| t.lots).sum::<f64>() / span_years;
    let spread_year = lots_year * rules.spread * rules.contract_size;
    println!("{:.1} round-turn lots/year on $10k; spread ${:.0}/year at {:.2}; net ${:.0}/year", lots_year, spread_year, rules.spread, net / span_years);
    if std::env::var("FD_TRADES").is_ok() {
        for t in &r.trades {
            println!("T {} {} {:.3} {:+.4} {:+.2} {:.2} {:.2}", fmt(t.entry_time), fmt(t.exit_time), t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price);
        }
    }
    Ok(())
}
