//! Diagnostic: one `tsmom` preset through the fixed replay and the walk-forward,
//! trade by trade, with the numbers a risk review needs — net dollars, the
//! top-five share, worst holds, per-year split. Not a receipt — the record
//! quotes `search`'s files — but the thing to run when the two disagree
//! (three engine faults were found this way,
//! `docs/decisions/2026-09-13-instrument-faults.md`).
//!
//!     cargo run --release -p fd-backtest --example diag_tsmom -- 60 [xauduka 2018-06-16 2025-04-10 1800 1800-1900]
//!
//! Arguments: lookback days, market, from, to (exclusive), rebalance HHMM,
//! hours gate. Set `FD_TRADES=1` to print every trade.

use fd_backtest::engine::{Range, run_backtest, trading_rules_for};
use fd_backtest::hypotheses::Preset;
use fd_backtest::sweep::{SelectBy, walk_forward};
use fd_core::config::Config;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{Registry, Side};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lookback: f64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(60.0);
    let market = args.get(1).cloned().unwrap_or_else(|| "xauduka".into());
    let parse_day = |s: &str| -> i64 {
        let mut it = s.split('-').map(|p| p.parse::<i64>().unwrap());
        let (y, m, d) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000
    };
    let from = args.get(2).map(|s| parse_day(s)).unwrap_or_else(|| parse_day("2018-06-16"));
    let to = args.get(3).map(|s| parse_day(s)).unwrap_or_else(|| parse_day("2025-04-10"));
    let rebalance: f64 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(1800.0);
    let gate = args.get(5).cloned().unwrap_or_else(|| "1800-1900".into());

    let config = Config::load("config")?;
    let spec = config.market(&market)?;
    let path = std::path::PathBuf::from("data/bars").join(format!("{}-15m.parquet", spec.bar_symbol));
    let mut bars = fd_store::read_bars(&path)?;
    bars.retain(|b| b.time >= from && b.time < to);
    let rules = trading_rules_for(&config, &market)?;
    let registry = Registry::with_builtins();
    let base = registry.get("tsmom")?;
    let preset = Preset::new(
        base,
        &[
            ("lookbackDays".into(), lookback),
            ("rebalanceHHMM".into(), rebalance),
            ("riskDailyRanges".into(), 2.0),
            ("rangeDays".into(), 20.0),
        ],
    )?;
    let filtered = Filtered { inner: &preset, filters: vec![Filter::parse("weekdays")?, Filter::parse(&format!("hours:{gate}"))?] };
    let fmt = |t: i64| {
        let d = t.div_euclid(86_400_000);
        let (y, m, dd) = fd_core::clock::civil_from_days(d);
        format!("{y}-{m:02}-{dd:02}")
    };
    let fixed = run_backtest(&bars, &filtered, &preset.defaults, &rules, None, Range::default());
    let m = &fixed.metrics;
    println!(
        "{market} tsmom {lookback}d @{rebalance} {gate}: {} trades  PF {:.3}  exp {:.3}R  win {:.1}%  net ${:.0}  return {:.2}%  maxDD ${:.0} ({:.2}%)  avgMAE {:.3}R  avgMFE {:.3}R  hold {:.0} min",
        m.trades, m.profit_factor, m.expectancy, m.win_rate * 100.0, m.net_pnl_usd, m.return_pct, m.max_drawdown_usd, m.max_drawdown_pct, m.avg_mae, m.avg_mfe, m.avg_hold_min
    );
    let mut pnl: Vec<f64> = fixed.trades.iter().map(|t| t.pnl_usd).collect();
    pnl.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let net: f64 = pnl.iter().sum();
    let top5: f64 = pnl.iter().take(5).sum();
    let (mut w, mut l) = (0.0, 0.0);
    for p in pnl.iter().skip(5) {
        if *p > 0.0 { w += p } else { l -= p }
    }
    let longs = fixed.trades.iter().filter(|t| matches!(t.direction, Side::Long)).count();
    println!(
        "top-5 ${:.0} = {:.1}% of net; PF without top-5 {:.3}; bottom-5 ${:.0}; longs {} shorts {}",
        top5,
        100.0 * top5 / net,
        w / l,
        pnl.iter().rev().take(5).sum::<f64>(),
        longs,
        fixed.trades.len() - longs
    );
    let mut by_r: Vec<&fd_backtest::engine::Trade> = fixed.trades.iter().collect();
    by_r.sort_by(|a, b| a.r.partial_cmp(&b.r).unwrap());
    println!("worst 5 by R:");
    for t in by_r.iter().take(5) {
        println!("  {} -> {}  {:<5} lots {:.3}  R {:+.3}  ${:+.2}  {} -> {}", fmt(t.entry_time), fmt(t.exit_time), format!("{:?}", t.direction), t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price);
    }
    println!("best 5 by R:");
    for t in by_r.iter().rev().take(5) {
        println!("  {} -> {}  {:<5} lots {:.3}  R {:+.3}  ${:+.2}  {} -> {}", fmt(t.entry_time), fmt(t.exit_time), format!("{:?}", t.direction), t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price);
    }
    let mut years: std::collections::BTreeMap<i64, (usize, f64, f64, f64)> = Default::default();
    for t in &fixed.trades {
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
    if std::env::var("FD_TRADES").is_ok() {
        for t in &fixed.trades {
            println!("T {} {} {:?} {:.3} {:+.4} {:+.2} {:.3} {:.3} {}", fmt(t.entry_time), fmt(t.exit_time), t.direction, t.lots, t.r, t.pnl_usd, t.entry_price, t.exit_price, t.exit_reason);
        }
    }
    let wf = walk_forward(&filtered, &bars, &rules, None, 4, SelectBy::ProfitFactor, 1).expect("wf");
    println!("wf: PF {:.3} exp {:.3} net {:.0}", wf.oos.profit_factor, wf.oos.expectancy, wf.oos.net_pnl_usd);
    for f in &wf.folds {
        let m = f.test.as_ref();
        println!(
            "fold {} {} -> {}: trades {:?} PF {:?} net {:?}",
            f.fold,
            fmt(f.window.0),
            fmt(f.window.1),
            m.map(|m| m.trades),
            m.map(|m| m.profit_factor),
            m.map(|m| m.net_pnl_usd)
        );
    }
    Ok(())
}
