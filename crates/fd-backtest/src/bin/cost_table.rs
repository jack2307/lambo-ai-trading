//! `spread / stop` across every instrument in the config, at two declared stop
//! rules held identical between instruments.
//!
//! Task D of `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`. This is
//! a **measurement, not a search**: it runs no strategy, opens no trade, ranks
//! nothing by profitability and proposes no method. Its entire output is a cost
//! table.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin cost_table -- --data=E:/rust/flowdesk/data-sealed
//! ```
//!
//! ## The two stop rules, declared once and applied unchanged
//!
//! One rule cannot answer the question. Gold's configured 0.28 costs 0.67% of R
//! at a 41.57-point multi-day stop and 14.0% at a 2.00-point intraday one — the
//! same spread, a 21x difference — so the stop rule matters as much as the
//! instrument and must be held fixed to compare instruments at all.
//!
//! * **Rule I, intraday:** `stop = 1.5 x ATR14` on **15-minute** bars.
//! * **Rule H, the cap's horizon:** `stop = 1.5 x ATR14` on **four-hour** bars.
//!   Four hours is `[trading] max_hold_ms`, which `trading_rules_for` applies to
//!   every market, so it is the horizon every `Exits::Engine` method in this
//!   record was actually force-closed at.
//! * **Rule M, multi-day:** `stop = 1.5 x ATR20` on **session-daily** bars,
//!   aggregated from the 15m series with the day boundary on the 17:00 New York
//!   metals/FX break (UTC+3, the broker's clock).
//!
//! 1.5 is not chosen here: it is the multiple the registry's nulls run at and
//! the one both funded books sit at. ATR14 is `[backtest] fallback_atr_period`;
//! ATR20 is the period the multi-day figure in the record was taken at. Nothing
//! is tuned, because nothing is being scored.
//!
//! **The multiple cannot change any comparison between instruments, and that is
//! worth saying before the table rather than after.** The ratio is
//! `spread / (k x ATR)`, so `k` is a pure rescaling: every instrument's figure
//! moves by the same factor and the ordering is identical at k = 0.8, 1.5 or
//! 3.0. What DOES reorder instruments is the ATR's own **timeframe and period**,
//! because that is a different series per instrument - which is exactly why
//! three horizons are reported and one would not have been enough.
//!
//! ## Eras, because coverage differs enormously
//!
//! XAUUSD-15m starts 2022-06-16 with 77k bars; the Dukascopy series start
//! 2010-06-01 with 358k-381k. `spread / ATR` moves with the price level — gold
//! was 1,200 in 2010 and 3,700 in 2025 — so a pooled figure is partly an
//! artefact of which years an instrument happens to cover. Every row is reported
//! per era as well as pooled, and a **common era** every surviving instrument
//! covers is printed last; that is the only row on which the claim can be
//! decided.
//!
//! ## The seal
//!
//! Default `--to=2025-09-23`, exclusive, matching `seal-store`'s cutoff. The
//! receipt prints how many bars the clamp dropped; on the sealed store it is
//! zero, and on any other store a non-zero count is the reader's warning that
//! the withheld year was in range.

use std::path::PathBuf;

use fd_backtest::cost_table::{
    Dist, Era, bucket, cost_fraction_of_r, daily_bars, date_ms, per_bar, summarise, ymd,
};
use fd_backtest::engine::trading_rules_for;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// Intraday rule.
const ATR_INTRADAY: usize = 14;
/// The four-hour rule: the horizon `max_hold_ms` has silently imposed on every
/// engine-exit method ever measured here.
const ATR_FOURHOUR: usize = 14;
const FOURHOUR_MS: i64 = 14_400_000;
/// Multi-day rule.
const ATR_DAILY: usize = 20;
/// Held identical across both rules and every instrument.
const STOP_ATR: f64 = 1.5;
/// The session boundary: 17:00 New York in summer is 21:00 UTC, which is
/// midnight on the broker's UTC+3 clock.
const SESSION_OFFSET_MS: i64 = 3 * 3_600_000;

struct Row {
    market: String,
    symbol: String,
    spread: Option<f64>,
    contract_size: f64,
    bars: usize,
    first: String,
    last: String,
    synthetic_share: f64,
    intraday: Option<Dist>,
    fourhour: Option<Dist>,
    daily: Option<Dist>,
    daily_n_sessions: usize,
    /// Per-era medians, in the order of [`ERAS`], for every rule.
    era_intraday: Vec<Option<Dist>>,
    era_fourhour: Vec<Option<Dist>>,
    era_daily: Vec<Option<Dist>>,
    common_intraday: Option<Dist>,
    common_fourhour: Option<Dist>,
    common_daily: Option<Dist>,
}

const ERAS: &[(&str, (i64, u32, u32), (i64, u32, u32))] = &[
    ("2010-2012", (2010, 1, 1), (2013, 1, 1)),
    ("2013-2015", (2013, 1, 1), (2016, 1, 1)),
    ("2016-2018", (2016, 1, 1), (2019, 1, 1)),
    ("2019-2021", (2019, 1, 1), (2022, 1, 1)),
    ("2022-2025", (2022, 1, 1), (2025, 9, 23)),
    // A FIXED comparison window, not an era: the whole of the XAUUSD-15m file.
    // Coverage inside it still differs (btcusd starts 2023-10, btc 2024-09), so
    // the n beside each figure is load-bearing and a short n is a partial cover.
    ("XAUUSD window", (2022, 6, 16), (2025, 9, 23)),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = PathBuf::from(arg("data", "data"));
    let config_dir = arg("config", "config");
    let config = Config::load(&config_dir)?;
    let interval = arg("interval", "15m");
    let to_text = arg("to", "2025-09-23");
    let mut parts = to_text.split('-');
    let (ty, tm, td) = (
        parts.next().ok_or("--to=YYYY-MM-DD")?.parse::<i64>()?,
        parts.next().ok_or("--to=YYYY-MM-DD")?.parse::<u32>()?,
        parts.next().ok_or("--to=YYYY-MM-DD")?.parse::<u32>()?,
    );
    let to_ms = date_ms(ty, tm, td);

    let eras: Vec<Era> = ERAS
        .iter()
        .map(|(label, (fy, fm, fd), (ty, tm, td))| Era {
            label,
            from_ms: date_ms(*fy, *fm, *fd),
            to_ms: date_ms(*ty, *tm, *td),
        })
        .collect();

    println!("cost_table — spread / stop per instrument. A MEASUREMENT, not a search.");
    println!("task D, docs/hypotheses/2026-09-24-what-the-record-cannot-see.md");
    println!("data:     {} (config: {})", data.display(), config_dir);
    println!("interval: {interval} (multi-day rule aggregates it to session days)");
    println!("clamp:    bars at or after {to_text} 00:00:00Z are dropped (the seal's cutoff, exclusive)");
    println!(
        "rule I:   stop = {STOP_ATR} x ATR{ATR_INTRADAY} on {interval} bars, in the instrument's own price units"
    );
    println!(
        "rule H:   stop = {STOP_ATR} x ATR{ATR_FOURHOUR} on 4h bars \u{2014} the horizon [trading] max_hold_ms imposes on every market"
    );
    println!(
        "rule M:   stop = {STOP_ATR} x ATR{ATR_DAILY} on session-daily bars (day boundary 21:00Z = broker UTC+3 midnight)"
    );
    println!("invariance: k is a pure rescale. spread/(k x ATR) reorders no instrument; only the ATR series does");
    println!("ratio:    spread/stop. cost = spread x lots x contract; R = stop x lots x contract; lots and contract CANCEL");
    println!("quantile: nearest-rank, index round(q x (n-1)) on the sorted sample\n");

    let mut rows: Vec<Row> = Vec::new();
    let mut excluded: Vec<(String, String)> = Vec::new();

    // The common era is fixed after the first pass, from the latest start among
    // instruments that produced any ratio at all.
    let mut loaded: Vec<(String, Vec<Bar>, Option<f64>, f64, String, usize)> = Vec::new();

    let skip = arg("skip", "");
    let skip: Vec<&str> = skip.split(',').filter(|s| !s.is_empty()).collect();
    if !skip.is_empty() {
        println!("skip:     {} (excluded on the command line, to widen the common era)\n", skip.join(", "));
    }

    for id in config.markets.keys() {
        if skip.contains(&id.as_str()) {
            excluded.push((id.clone(), "--skip on the command line; not a property of the instrument".into()));
            continue;
        }
        let spec = config.market(id)?;
        // PER-INSTRUMENT SPREAD, from the market's own block through the same
        // resolver the engine uses. Never gold's, never the top-level table's.
        let rules = trading_rules_for(&config, id)?;
        let spread = if rules.spread.is_finite() && rules.spread >= 0.0 { Some(rules.spread) } else { None };

        let path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
        let bars = match read_bars(&path) {
            Ok(b) => b,
            Err(e) => {
                excluded.push((id.clone(), format!("no {interval} bar file at {} ({e})", path.display())));
                continue;
            }
        };
        let before = bars.len();
        let bars: Vec<Bar> = bars.into_iter().filter(|b| b.time < to_ms).collect();
        let dropped = before - bars.len();
        if dropped > 0 {
            println!("!! {id}: the {to_text} clamp dropped {dropped} bars — this store is NOT the sealed one");
        }
        if bars.is_empty() {
            excluded.push((
                id.clone(),
                format!("{} holds {before} bars; empty is empty, not an instrument with no volatility", path.display()),
            ));
            continue;
        }
        if spread.is_none() {
            excluded.push((id.clone(), "configured spread could not be established — no ratio, and not gold's".into()));
            continue;
        }
        loaded.push((id.clone(), bars, spread, rules.contract_size, spec.trading.symbol.clone(), before));
    }

    // Common era: the latest first bar and the earliest last bar across every
    // instrument that survived. An instrument with 1.5 years cannot be compared
    // to one with 15 on a quantity that moves with the price level.
    let common_from = loaded.iter().map(|(_, b, ..)| b[0].time).max().unwrap_or(0);
    let common_to = loaded.iter().map(|(_, b, ..)| b[b.len() - 1].time).min().unwrap_or(0) + 1;
    let common = Era { label: "common", from_ms: common_from, to_ms: common_to };

    for (id, bars, spread, contract_size, symbol, _raw) in &loaded {
        let synthetic = bars.iter().filter(|b| b.is_synthetic()).count() as f64 / bars.len() as f64;

        let atr_i = fd_indicators::atr(bars, ATR_INTRADAY);
        let intraday_rows = per_bar(bars, &atr_i, STOP_ATR, *spread);

        // Four-hour buckets are aligned to UTC, not to the session: a 4h bar is
        // a horizon, not a trading day, and `max_hold_ms` counts wall clock.
        let fourhours = bucket(bars, FOURHOUR_MS, 0);
        let atr_h = fd_indicators::atr(&fourhours, ATR_FOURHOUR);
        let fourhour_rows = per_bar(&fourhours, &atr_h, STOP_ATR, *spread);

        let sessions = daily_bars(bars, SESSION_OFFSET_MS);
        let atr_d = fd_indicators::atr(&sessions, ATR_DAILY);
        let daily_rows = per_bar(&sessions, &atr_d, STOP_ATR, *spread);

        let dist_of = |rows: &[(i64, f64, f64)], era: Option<&Era>| -> Option<Dist> {
            let (mut r, mut s) = (Vec::new(), Vec::new());
            for (t, stop, ratio) in rows {
                if era.is_some_and(|e| !e.contains(*t)) {
                    continue;
                }
                r.push(*ratio);
                s.push(*stop);
            }
            summarise(&r, &s)
        };

        rows.push(Row {
            market: id.clone(),
            symbol: symbol.clone(),
            spread: *spread,
            contract_size: *contract_size,
            bars: bars.len(),
            first: ymd(bars[0].time),
            last: ymd(bars[bars.len() - 1].time),
            synthetic_share: synthetic,
            intraday: dist_of(&intraday_rows, None),
            fourhour: dist_of(&fourhour_rows, None),
            daily: dist_of(&daily_rows, None),
            daily_n_sessions: sessions.len(),
            era_intraday: eras.iter().map(|e| dist_of(&intraday_rows, Some(e))).collect(),
            era_fourhour: eras.iter().map(|e| dist_of(&fourhour_rows, Some(e))).collect(),
            era_daily: eras.iter().map(|e| dist_of(&daily_rows, Some(e))).collect(),
            common_intraday: dist_of(&intraday_rows, Some(&common)),
            common_fourhour: dist_of(&fourhour_rows, Some(&common)),
            common_daily: dist_of(&daily_rows, Some(&common)),
        });
    }

    println!("\n=== coverage, and the units the ratio cancels ===\n");
    println!(
        "{:<9} {:<11} {:>10} {:>9} {:>9} {:>12} {:>12} {:>9} {:>7}",
        "market", "symbol", "spread", "contract", "bars", "first", "last", "sessions", "flat%"
    );
    for r in &rows {
        println!(
            "{:<9} {:<11} {:>10} {:>9} {:>9} {:>12} {:>12} {:>9} {:>6.1}%",
            r.market,
            r.symbol,
            r.spread.map_or("null".to_string(), |s| format!("{s:.5}")),
            format!("{:.2}", r.contract_size),
            r.bars,
            r.first,
            r.last,
            r.daily_n_sessions,
            r.synthetic_share * 100.0
        );
    }

    for (title, pick) in [
        ("RULE I — intraday, stop = 1.5 x ATR14 on 15m bars", 0u8),
        ("RULE H — four hours, stop = 1.5 x ATR14 on 4h bars (the max_hold_ms horizon)", 1),
        ("RULE M — multi-day, stop = 1.5 x ATR20 on session-daily bars", 2),
    ] {
        println!("\n=== {title} ===\n");
        println!(
            "{:<9} {:>12} {:>10} {:>10} {:>10} {:>10} {:>9}",
            "market", "med stop", "med s/stop", "p10", "p90", "spread", "n"
        );
        for r in &rows {
            let d = match pick {
                0 => r.intraday,
                1 => r.fourhour,
                _ => r.daily,
            };
            match d {
                Some(d) => println!(
                    "{:<9} {:>12} {:>9.3}% {:>9.3}% {:>9.3}% {:>10} {:>9}",
                    r.market,
                    fmt_price(d.median_stop),
                    d.p50 * 100.0,
                    d.p10 * 100.0,
                    d.p90 * 100.0,
                    r.spread.map_or("null".into(), |s| fmt_price(s)),
                    d.n
                ),
                None => println!("{:<9} {:>12} {:>10} {:>10} {:>10} {:>10} {:>9}", r.market, "-", "null", "null", "null", "-", 0),
            }
        }
    }

    println!("\n=== per era — median spread/stop (n), because coverage differs ===\n");
    for (rule, pick) in [("I (intraday, 15m)", 0u8), ("H (four hours)", 1), ("M (multi-day)", 2)] {
        println!("rule {rule}:");
        print!("{:<9}", "market");
        for (label, ..) in ERAS {
            print!(" {label:>20}");
        }
        println!(" {:>20}", "pooled");
        for r in &rows {
            print!("{:<9}", r.market);
            let per_era = match pick {
                0 => &r.era_intraday,
                1 => &r.era_fourhour,
                _ => &r.era_daily,
            };
            for d in per_era {
                print!(" {:>20}", fmt_dist(*d));
            }
            let pooled = match pick {
                0 => r.intraday,
                1 => r.fourhour,
                _ => r.daily,
            };
            println!(" {:>20}", fmt_dist(pooled));
        }
        println!();
    }

    println!(
        "=== the common era every surviving instrument covers: {} to {} ===\n",
        ymd(common.from_ms),
        ymd(common.to_ms - 1)
    );
    println!("This is the only window on which the instruments are comparable without");
    println!("the price level of different decades doing the work.\n");
    println!("{:<9} {:>22} {:>22} {:>22}", "market", "rule I med (n)", "rule H med (n)", "rule M med (n)");
    for r in &rows {
        println!(
            "{:<9} {:>22} {:>22} {:>22}",
            r.market,
            fmt_dist(r.common_intraday),
            fmt_dist(r.common_fourhour),
            fmt_dist(r.common_daily)
        );
    }

    println!("\n=== excluded, and why ===\n");
    if excluded.is_empty() {
        println!("none");
    }
    for (id, why) in &excluded {
        println!("{id}: {why}");
    }

    // WHERE EACH CONFIGURED SPREAD CAME FROM, and what the table looks like at
    // the number that was actually measured rather than the number that is
    // charged. This block is HAND-TRANSCRIBED FROM THE COMMENTS in
    // config/default.toml, because provenance lives only in comments there:
    // there is no key that says whether a spread was measured, assumed, or read
    // once. No program can tell them apart, which is a defect in its own right
    // and is reported as one. Every quotation below is from that file.
    //
    // The rescale is exact, not a model: the ratio is linear in the spread, so
    // at a spread s\u{2032} every figure is the printed one times s\u{2032}/s.
    println!("\n=== where each configured spread came from, and the table at the MEASURED one ===\n");
    println!("Provenance is transcribed from config/default.toml's comments. There is NO KEY");
    println!("for it, so nothing in the codebase can distinguish a measured spread from an");
    println!("assumed one. The rescale below is exact: the ratio is linear in the spread.\n");
    println!(
        "{:<9} {:>9} {:>9} {:<44} {:>11} {:>11} {:>11}",
        "market", "charged", "measured", "provenance, as the config states it", "I at meas", "H at meas", "M at meas"
    );
    for r in &rows {
        let (measured, provenance): (Option<f64>, &str) = match r.market.as_str() {
            // "the logger ... has since sampled XAUUSD.sc 2,315 times over 12.8
            //  hours ... p10 0.210, p50 0.220, p90 0.220, max 0.260 - the
            //  configured 0.28 is above every spread ever observed, and charges
            //  roughly 27% too much."
            "xauusd" | "xauduka" => (Some(0.220), "p50 of 2,315 samples/12.8h; 0.28 charges ~27% too much"),
            // "measured 2026-09-13 from 205k ticks over three days (read-only):
            //  spread p50/p90 $0.021 an ounce."
            "xagduka" => (Some(0.021), "p50/p90 of 205k ticks over 3 days; charged = measured"),
            // "measured 2026-09-13 from 119k ticks over three days (read-only):
            //  spread p50/p90 0.00014."
            "eurduka" | "eurusd" => (Some(0.00014), "p50/p90 of 119k ticks over 3 days; charged = measured"),
            // "BTCUSD.sc ... spread ~1705 pts = $17.05" from a SINGLE read, and
            // "whose 17.05 has never been checked over time at all."
            "btcusd" => (None, "ONE read of the terminal; never checked over time"),
            // "3.4x the 5.00 assumed for the Binance market" - 5.00 is an
            // assumption, and it is the cheapest row in the table.
            "btc" => (None, "ASSUMED, never measured (and not an account this desk trades)"),
            _ => (None, "not established"),
        };
        let scale = match (measured, r.spread) {
            (Some(m), Some(c)) if c > 0.0 => Some(m / c),
            _ => None,
        };
        let at = |d: Option<Dist>| -> String {
            match (d, scale) {
                (Some(d), Some(k)) => format!("{:.3}%", d.p50 * k * 100.0),
                // A spread whose provenance is an assumption or a single read has
                // no measured figure. That is null, and null is not the charged
                // number wearing a different label.
                _ => "null".to_string(),
            }
        };
        println!(
            "{:<9} {:>9} {:>9} {:<44} {:>11} {:>11} {:>11}",
            r.market,
            r.spread.map_or("null".into(), |v| fmt_price(v)),
            measured.map_or("null".to_string(), fmt_price),
            provenance,
            at(r.intraday),
            at(r.fourhour),
            at(r.daily)
        );
    }
    println!("\nRead the two BTC rows with the third column in hand: the cheapest figures in");
    println!("this whole table rest on a spread nobody has measured over time, and on one");
    println!("(btc, Binance spot) that is not an account this desk can trade at all.");

    // The cancellation, worked once on the record's own two gold figures so a
    // reader can check the arithmetic in this binary against the record.
    println!("\n=== the cancellation, worked ===\n");
    for (label, spread, stop, lots, contract) in [
        ("gold intraday (record: 14.0% at a 2.00-point stop)", 0.28, 2.00, 0.07, 1.0),
        ("gold multi-day (record: 0.67% at a 41.57-point stop)", 0.28, 41.57, 2.563, 1.0),
        ("the same gold intraday at a 100 oz lot", 0.28, 2.00, 0.07, 100.0),
    ] {
        let f = cost_fraction_of_r(Some(spread), stop, lots, contract).unwrap();
        println!(
            "{label}: ({spread} x {lots} x {contract}) / ({stop} x {lots} x {contract}) = {:.4}% of R",
            f * 100.0
        );
    }

    Ok(())
}

fn fmt_price(v: f64) -> String {
    if v.abs() >= 1.0 {
        format!("{v:.3}")
    } else if v.abs() >= 0.001 {
        format!("{v:.5}")
    } else {
        format!("{v:.7}")
    }
}

fn fmt_dist(d: Option<Dist>) -> String {
    match d {
        Some(d) => format!("{:.3}% ({})", d.p50 * 100.0, d.n),
        None => "null".to_string(),
    }
}
