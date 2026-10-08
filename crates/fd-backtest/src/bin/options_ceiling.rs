//! Count the CEILING on the trade count of the options-level entry rule.
//!
//! Registered in `docs/decisions/2026-10-08-options-ceiling-precheck.md`, which
//! executes section 6 of `docs/decisions/2026-10-06-options-thesis-scope.md`.
//!
//! This is arithmetic, not a backtest. It runs no strategy, opens no position,
//! prices no exit, computes no PF, no expectancy, no percentile and no null.
//! It asks one thing: on how many bars is the `level-reversion` entry condition
//! satisfiable at all? That count is an upper bound on the trade count, because
//! a position can only ever REMOVE entries (`builtin.rs:437`,
//! `ctx.position.is_some()`).
//!
//! The clusters are the engine's own, through `build_timeline` over the real
//! tape, bound to bars by the same `view_from` cursor the backtest uses - so a
//! count here is a count of what the strategy would have seen, not of a
//! re-implementation of it.
//!
//!   options_ceiling --market=gold --data=E:/rust/flowdesk/data --bars=GC --interval=1m --offset=0

use std::collections::HashMap;
use std::path::PathBuf;

use fd_backtest::timeline::{TimelineOptions, build_timeline};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_indicators::{IndicatorSpec, compute_indicators};
use fd_store::{TapeStore, read_bars};

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// The distance function of `level-reversion` itself, `builtin.rs:479-487`.
fn cluster_distance(price: f64, low: f64, high: f64) -> f64 {
    if price < low {
        low - price
    } else if price > high {
        price - high
    } else {
        0.0
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = PathBuf::from(arg("data", "data"));
    let market = arg("market", "gold");
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;

    // The bar series is named SEPARATELY from the market, which is the whole
    // point: the gold tape levels have to be tried against a tradable series,
    // and the market that owns a tradable series has `options_source = "none"`.
    let bar_symbol = arg("bars", spec.bar_symbol.as_str());
    let interval = arg("interval", "1m");
    let offset: f64 = arg("offset", "0").parse()?;
    let bars_path = data.join("bars").join(format!("{bar_symbol}-{interval}.parquet"));

    let mut bars: Vec<Bar> = read_bars(&bars_path)?;
    // The basis is a TRANSLATION of the price axis. Shifting the bars onto the
    // GC axis is the same operation as shifting every level onto the spot axis,
    // and doing it here means the engine and its config are untouched.
    if offset != 0.0 {
        for b in &mut bars {
            b.open += offset;
            b.high += offset;
            b.low += offset;
            b.close += offset;
        }
    }

    let tape = spec.tape_id().ok_or("this market has no tape (options_source = none)")?.to_string();
    let store = TapeStore::open(&data, &tape)?;
    let trades = store.all()?;
    let settings = fd_engine::engine::EngineSettings::from_config(&config, &market)?;
    let timeline = build_timeline(
        &trades,
        &settings,
        &HashMap::new(),
        &TimelineOptions {
            step_ms: config.backtest.options_step_ms,
            big_trade_window_ms: config.ai.big_trade_window_ms,
        },
    );

    println!("market:   {market} (engine settings, cluster floor / atr_fraction)");
    println!("bars:     {} {} from {}", bars.len(), bar_symbol, bars_path.display());
    println!("offset:   {offset:+.2} USD added to every bar price (0 = none)");
    println!(
        "tape:     {} prints, {} frames, step {} ms",
        trades.len(),
        timeline.len(),
        config.backtest.options_step_ms
    );

    // Degeneracy, printed before any count, because a bar with no range cannot
    // cover a stop and a ceiling measured on such a series is a count about
    // something that does not trade. `null` is NOT printed as `0`.
    let degenerate = bars.iter().filter(|b| b.open == b.high && b.high == b.low && b.low == b.close).count();
    // `Bar::volume` is `Option<f64>`, so the TYPE already separates "no volume
    // was published" from "the volume was zero" (brief rule: `null` != `0`).
    // The parent survey reports "volume is 0 on every bar"; that is the one
    // claim of it this run can contradict with a type, not an opinion.
    let vol_zero = bars.iter().filter(|b| b.volume == Some(0.0)).count();
    let vol_null = bars.iter().filter(|b| b.volume.is_none()).count();
    println!(
        "shape:    {degenerate}/{} bars open==high==low==close ({:.2}%)   volume: {vol_zero} zero, {vol_null} null",
        bars.len(),
        100.0 * degenerate as f64 / bars.len() as f64
    );
    if degenerate == bars.len() {
        println!("          ** EVERY BAR HAS ZERO RANGE: this series cannot cover a stop, the counts below are NOT TRADABLE **");
    }

    let atr_period: usize = arg("atr-period", "14").parse()?;
    let set = compute_indicators(&bars, &[IndicatorSpec::new("atr").with("period", atr_period as f64)])?;
    let key = format!("atr_{atr_period}");
    let atr = set.get(&key).ok_or_else(|| format!("no series {key}; have {:?}", set.keys().collect::<Vec<_>>()))?;
    // `warmup = atrPeriod * 3` (`builtin.rs:425`), and the engine does not ask
    // the strategy before it (`engine.rs:613`).
    let warmup = atr_period * 3;

    let max_hold_ms = config.trading.max_hold_ms;
    println!("warmup:   {warmup} bars skipped (atrPeriod x 3)   max_hold {max_hold_ms} ms");
    println!();
    println!("minScore  entryAtr  framed  CEILING  4h-SPACED  medDist  medAtr");

    for min_score in [2.0_f64, 3.0, 5.0] {
        for entry_atr in [0.2_f64, 0.35, 0.6] {
            let mut cursor = 0usize;
            let mut framed = 0usize;
            let mut ceiling = 0usize;
            let mut spaced = 0usize;
            let mut free_at = i64::MIN;
            let mut dists: Vec<f64> = Vec::new();
            let mut atrs: Vec<f64> = Vec::new();
            for (i, bar) in bars.iter().enumerate() {
                if i < warmup {
                    continue;
                }
                let a = atr[i];
                if !a.is_finite() {
                    continue;
                }
                let Some(view) = timeline.view_from(&mut cursor, bar.time) else { continue };
                framed += 1;
                atrs.push(a);
                let mut best: Option<f64> = None;
                for c in view.clusters {
                    if c.score < min_score {
                        continue;
                    }
                    let d = cluster_distance(bar.close, c.low, c.high);
                    if best.is_none_or(|b| d < b) {
                        best = Some(d);
                    }
                }
                let Some(d) = best else { continue };
                dists.push(d);
                if d <= a * entry_atr {
                    ceiling += 1;
                    if bar.time >= free_at {
                        spaced += 1;
                        free_at = bar.time + max_hold_ms;
                    }
                }
            }
            let med = |mut v: Vec<f64>| -> String {
                if v.is_empty() {
                    return "null".to_string();
                }
                v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
                format!("{:.3}", v[v.len() / 2])
            };
            println!(
                "{min_score:>8.1}  {entry_atr:>8.2}  {framed:>6}  {ceiling:>7}  {spaced:>9}  {:>7}  {:>6}",
                med(dists),
                med(atrs)
            );
        }
    }
    Ok(())
}
