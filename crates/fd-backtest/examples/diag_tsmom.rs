//! Diagnostic: one preset through the fixed replay and the walk-forward,
//! trade by trade. Not a receipt — the record quotes `search`'s files — but
//! the thing to run when the two disagree (three engine faults were found
//! this way, `docs/decisions/2026-09-13-instrument-faults.md`). Adapt the
//! market, preset and filters at the top for another row.
//!
//!     cargo run --release -p fd-backtest --example diag_tsmom -- 60

use fd_backtest::engine::{Range, run_backtest, trading_rules_for};
use fd_backtest::hypotheses::Preset;
use fd_backtest::sweep::{SelectBy, walk_forward};
use fd_core::config::Config;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::Registry;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load("config")?;
    let spec = config.market("xauduka")?;
    let path = std::path::PathBuf::from("data/bars").join(format!("{}-15m.parquet", spec.bar_symbol));
    let mut bars = fd_store::read_bars(&path)?;
    let day = |y, m, d| fd_core::clock::days_from_civil(y, m, d) * 86_400_000;
    let (from, to) = (day(2018, 6, 16), day(2025, 4, 10));
    bars.retain(|b| b.time >= from && b.time < to);
    let rules = trading_rules_for(&config, "xauduka")?;
    let registry = Registry::with_builtins();
    let base = registry.get("tsmom")?;
    let lookback: f64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(60.0);
    let preset = Preset::new(
        base,
        &[("lookbackDays".into(), lookback), ("riskDailyRanges".into(), 2.0), ("rangeDays".into(), 20.0)],
    )?;
    let filtered = Filtered { inner: &preset, filters: vec![Filter::parse("weekdays")?, Filter::parse("hours:1800-1900")?] };
    let fmt = |t: i64| {
        let d = t.div_euclid(86_400_000);
        let (y, m, dd) = fd_core::clock::civil_from_days(d);
        format!("{y}-{m:02}-{dd:02}")
    };
    let show = |label: &str, trades: &[fd_backtest::engine::Trade]| {
        println!("== {label}: {} trades", trades.len());
        for t in trades {
            println!(
                "{} -> {}  {:<5} lots {:>7.2}  R {:>7.2}  pnl {:>9.2}  {:?}",
                fmt(t.entry_time),
                fmt(t.exit_time),
                format!("{:?}", t.direction),
                t.lots,
                t.r,
                t.pnl_usd,
                t.exit_kind
            );
        }
    };
    let fixed = run_backtest(&bars, &filtered, &preset.defaults, &rules, None, Range::default());
    println!("fixed: PF {:.3} exp {:.3} net {:.0} maxDD {:.0}", fixed.metrics.profit_factor, fixed.metrics.expectancy, fixed.metrics.net_pnl_usd, fixed.metrics.max_drawdown_usd);
    show("fixed", &fixed.trades);
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
    show("wf", &wf.oos_trades);
    Ok(())
}
