//! Every instrument in the store, ranked by `cost/R` at matched horizons —
//! BEFORE any mechanism is measured on any of them.
//!
//! Registered as `docs/decisions/2026-10-11-instr-cost-screen.md`. This runs no
//! strategy, opens no trade and consumes no gate cell. Its whole output is an
//! inventory, a cost table, a volume census and a provider-print census.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin instr_screen -- --data=E:/rust/flowdesk/data
//! ```
//!
//! It is a SECOND binary beside `bin/cost_table.rs`, not an edit of it: that one
//! has already produced receipts and its default `--to=2025-09-23` clamp is the
//! seal's. This one reads the live store over the record's two real windows.
//!
//! ## The one horizon, declared once
//!
//! * **P (primary):** `stop = 1.5 x ATR14` on **15-minute** bars. Chosen because
//!   it is the only ruler the record's published figures stand on (gold
//!   12.58%/8.35%, AUDNZD 38.16%, BTC 2.35%->7.79%).
//! * **S1:** `stop = 1.5 x ATR20` on **session-daily** bars, day boundary at
//!   21:00Z = 17:00 New York.
//! * **S2:** `stop = 1.5 x ATR14` on **4h** bars = `[trading] max_hold_ms`.
//!
//! `k = 1.5` reorders nothing: the ratio is `spread/(k x ATR)`, so `k` rescales
//! every row by the same factor. What reorders instruments is the ATR's own
//! timeframe and period, which is why there are three rulers and not one.

use std::collections::BTreeMap;
use std::path::PathBuf;

use fd_backtest::cost_table::{Dist, Era, bucket, daily_bars, date_ms, per_bar, summarise, ymd};
use fd_backtest::engine::trading_rules_for;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::read_bars;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

const STOP_ATR: f64 = 1.5;
const ATR_15M: usize = 14;
const ATR_4H: usize = 14;
const ATR_DAILY: usize = 20;
const FOURHOUR_MS: i64 = 14_400_000;
const SESSION_OFFSET_MS: i64 = 3 * 3_600_000;
const DAY_MS: i64 = 86_400_000;

/// The mould's requirement, from `agent/mould-target`: gross/spread >= 8.7x per
/// exposed session. The measured ceiling on gold is 1.901x. The gap a different
/// market would have to close, IF the same signal (same gross in R) transplants:
const MOULD_REQUIRED: f64 = 8.7;
const GOLD_CEILING: f64 = 1.901;

fn needed_factor() -> f64 {
    MOULD_REQUIRED / GOLD_CEILING
}

/// Which keys a market block states in the config TEXT, because provenance for
/// `swap` and `starting_equity_usd` lives only there: `#[serde(default)]` makes
/// an absent key indistinguishable from a stated zero in the parsed struct.
#[derive(Debug, Default, Clone)]
struct Declared {
    swap_long: bool,
    swap_short: bool,
    equity: bool,
    max_hold: bool,
}

fn declared_keys(config_text: &str) -> BTreeMap<String, Declared> {
    let mut out: BTreeMap<String, Declared> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in config_text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("[markets.") {
            let name = rest.trim_end_matches(']');
            // `[markets.x.trading]` keeps naming market x.
            current = Some(name.split('.').next().unwrap_or(name).to_string());
            out.entry(current.clone().unwrap()).or_default();
            continue;
        }
        if t.starts_with('[') {
            current = None;
            continue;
        }
        if t.starts_with('#') {
            continue;
        }
        let Some(market) = current.as_ref() else { continue };
        let entry = out.entry(market.clone()).or_default();
        let key = t.split('=').next().unwrap_or("").trim();
        match key {
            "swap_long_per_lot" => entry.swap_long = true,
            "swap_short_per_lot" => entry.swap_short = true,
            "starting_equity_usd" => entry.equity = true,
            "max_hold_ms" => entry.max_hold = true,
            _ => {}
        }
    }
    out
}

/// Spread provenance, HAND-TRANSCRIBED from `config/default.toml`'s comments.
/// There is no key for it, so nothing in the codebase can tell a measured
/// spread from an assumed one. That is a defect, reported as one.
fn spread_provenance(market: &str) -> (&'static str, &'static str) {
    match market {
        "gold" => ("NOT STATED", "no provenance comment; 0.3 is the top-level [trading] figure for COMEX"),
        "xauusd" => ("MEASURED", "logger sampled XAUUSD.sc 2,315x/12.8h: p50 0.220; charged 0.28 is ABOVE every observed"),
        "xauduka" => ("MEASURED", "same XAUUSD.sc spread applied to Dukascopy bars; p50 0.220 measured, 0.28 charged"),
        "xagduka" => ("MEASURED", "205k ticks over 3 days: p50/p90 $0.021 an ounce; charged = measured"),
        "eurduka" => ("MEASURED", "119k ticks over 3 days: p50/p90 0.00014; charged = measured"),
        "eurusd" => ("MEASURED", "same EURUSD.sc reading: 119k ticks/3 days, p50/p90 0.00014"),
        "audnzd" => ("ASSUMED", "config states `# NOT MEASURED`: a QUOTE for a retail cross, no AUDNZD tick log exists"),
        "btcusd" => ("ONE READ", "config: ONE read of the terminal, `17.05 has never been checked over time at all`"),
        "btc" => ("ASSUMED", "config states 17.05 is `3.4x the 5.00 assumed`; 5.00 is the assumption, and Binance spot is not tradable here"),
        _ => ("NOT STATED", "no provenance established"),
    }
}

/// What the measured spread is, where one exists, so the table can be rescaled
/// to it exactly (the ratio is linear in the spread).
fn measured_spread(market: &str) -> Option<f64> {
    match market {
        "xauusd" | "xauduka" => Some(0.220),
        "xagduka" => Some(0.021),
        "eurduka" | "eurusd" => Some(0.00014),
        _ => None,
    }
}

/// Volume-column provenance, from the ingest code, not guessed.
fn volume_provenance(bar_source: &str) -> (&'static str, &'static str) {
    match bar_source {
        "mt5" => ("tick", "py/ingest/mt5_export.py:33-34 — `CFD real volume is always zero`, so tick_volume is stored"),
        "binance" => ("real", "Binance spot/futures publish traded base volume"),
        "dukascopy" => ("null-or-manufactured", "the Dukascopy bid feed publishes no volume; any 0.0 is the pipeline's"),
        "reference" => ("unknown", "OTL tape bars; no provenance comment in config"),
        _ => ("unknown", "bar_source not recognised"),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct VolCensus {
    n: usize,
    null: usize,
    zero: usize,
    positive: usize,
    negative: usize,
    distinct_capped: usize,
}

fn volume_census(bars: &[Bar]) -> VolCensus {
    let mut c = VolCensus { n: bars.len(), ..Default::default() };
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for b in bars {
        match b.volume {
            None => c.null += 1,
            Some(v) if v == 0.0 => {
                c.zero += 1;
                if seen.len() < 64 {
                    seen.insert(v.to_bits());
                }
            }
            Some(v) if v > 0.0 => {
                c.positive += 1;
                if seen.len() < 64 {
                    seen.insert(v.to_bits());
                }
            }
            Some(v) => {
                c.negative += 1;
                if seen.len() < 64 {
                    seen.insert(v.to_bits());
                }
            }
        }
    }
    c.distinct_capped = seen.len();
    c
}

fn volume_verdict(c: &VolCensus, stated: &str) -> String {
    if c.n == 0 {
        return "null (empty file)".into();
    }
    if c.null == c.n {
        return "null (provider publishes nothing)".into();
    }
    if c.positive == 0 && c.zero > 0 {
        return format!("MANUFACTURED 0 ({} distinct value(s), none > 0)", c.distinct_capped);
    }
    if c.null > 0 {
        return format!("mixed: {} null + {} >0 — read as {stated}", c.null, c.positive);
    }
    // A column that is `stated` but carries exact zeros on a large share of its
    // rows is not the stated column on those rows. Say the share.
    let zero_share = c.zero as f64 * 100.0 / c.n as f64;
    if zero_share >= 1.0 {
        return format!("{stated}, BUT {zero_share:.1}% of rows are exactly 0");
    }
    stated.to_string()
}

/// `open == low` / `close == low` / `open == high` / `close == high`, split by
/// whether the bar is the one that REOPENS after a break.
///
/// Two definitions of "reopen", because a definition is a choice:
/// * `gap`: the first bar after a hole in the series longer than one interval
///   and shorter than a day — i.e. the session break, whatever the clock says.
/// * `ny18`: the bar whose open stamps 18:00 New York local, the CME reopen,
///   which still exists when the feed fills the break with bars.
#[derive(Debug, Clone, Copy, Default)]
struct PrintCensus {
    n: usize,
    open_eq_low: usize,
    close_eq_low: usize,
    open_eq_high: usize,
    close_eq_high: usize,
}

impl PrintCensus {
    fn add(&mut self, b: &Bar) {
        self.n += 1;
        if b.open == b.low {
            self.open_eq_low += 1;
        }
        if b.close == b.low {
            self.close_eq_low += 1;
        }
        if b.open == b.high {
            self.open_eq_high += 1;
        }
        if b.close == b.high {
            self.close_eq_high += 1;
        }
    }
    fn pct(num: usize, den: usize) -> f64 {
        if den == 0 { f64::NAN } else { num as f64 * 100.0 / den as f64 }
    }
    fn line(&self) -> String {
        if self.n == 0 {
            return "null (0 bars)".into();
        }
        format!(
            "o==l {:>6.2}%  c==l {:>6.2}%  o==h {:>6.2}%  c==h {:>6.2}%  (n {})",
            Self::pct(self.open_eq_low, self.n),
            Self::pct(self.close_eq_low, self.n),
            Self::pct(self.open_eq_high, self.n),
            Self::pct(self.close_eq_high, self.n),
            self.n
        )
    }
}

/// Split a 15m series into (gap-reopen, ny-18:00, ordinary) print censuses, and
/// return the histogram of the NY local hour at which gap-reopens happen.
fn print_census(bars: &[Bar], interval_ms: i64) -> (PrintCensus, PrintCensus, PrintCensus, BTreeMap<u32, usize>) {
    let (mut gap, mut ny18, mut ordinary) = (PrintCensus::default(), PrintCensus::default(), PrintCensus::default());
    let mut hours: BTreeMap<u32, usize> = BTreeMap::new();
    for (i, b) in bars.iter().enumerate() {
        let is_gap = if i == 0 {
            false
        } else {
            let d = b.time - bars[i - 1].time;
            d > interval_ms && d < DAY_MS
        };
        // `new_york_local` returns (WEEKDAY, MINUTE OF DAY), not (hour, minute).
        // Reading it as (hour, minute) made `ny18` structurally empty on every
        // instrument and printed a weekday histogram labelled as hours; the
        // nonsense in that histogram is how the mistake was caught.
        let (_weekday, minute_of_day) = fd_core::clock::new_york_local(b.time);
        let hour = minute_of_day / 60;
        let is_ny18 = hour == 18;
        if is_gap {
            gap.add(b);
            *hours.entry(hour).or_default() += 1;
        }
        if is_ny18 {
            ny18.add(b);
        }
        if !is_gap && !is_ny18 {
            ordinary.add(b);
        }
    }
    (gap, ny18, ordinary, hours)
}

struct Row {
    market: String,
    symbol: String,
    bar_symbol: String,
    bar_source: String,
    spread: f64,
    contract_size: f64,
    min_lot: f64,
    lot_step: f64,
    equity: f64,
    equity_declared: bool,
    swap_long: f64,
    swap_short: f64,
    swap_declared: bool,
    bars15: usize,
    first: String,
    last: String,
    flat_share: f64,
    // cost/R by ruler x window: [ruler][window]
    dist: Vec<Vec<Option<Dist>>>,
}

const RULERS: [&str; 3] = ["P  1.5xATR14(15m)", "S1 1.5xATR20(session-day)", "S2 1.5xATR14(4h)"];
const WINDOWS: [&str; 4] = ["W1 2010-06..2018-06", "W2 2018-06..2026-06", "CM common", "PL pooled"];

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = PathBuf::from(arg("data", "data"));
    let config_dir = arg("config", "config");
    let config = Config::load(&config_dir)?;
    let config_text = std::fs::read_to_string(PathBuf::from(&config_dir).join("default.toml"))?;
    let declared = declared_keys(&config_text);

    println!("instr_screen — every instrument in the store, ranked by cost/R at matched horizons.");
    println!("registration: docs/decisions/2026-10-11-instr-cost-screen.md");
    println!("A MEASUREMENT. No strategy runs, no trade opens, no gate cell is consumed.\n");
    println!("data:    {}", data.display());
    println!("config:  {config_dir}");
    println!("ruler P  (PRIMARY, printed on every table): stop = {STOP_ATR} x ATR{ATR_15M} on 15m bars, instrument's own price units");
    println!("ruler S1 (second):  stop = {STOP_ATR} x ATR{ATR_DAILY} on session-daily bars, day boundary 21:00Z = 17:00 New York");
    println!("ruler S2 (third):   stop = {STOP_ATR} x ATR{ATR_4H} on 4h bars = [trading] max_hold_ms");
    println!("k = {STOP_ATR} REORDERS NOTHING: spread/(k x ATR) rescales every row by the same factor.");
    println!("   What reorders instruments is the ATR's own timeframe and period. Hence three rulers.");
    println!("ratio:   spread/stop. cost = spread x lots x contract; R = stop x lots x contract; lots and contract CANCEL.");
    println!("stat:    median (nearest-rank, index round(q x (n-1))), with p10/p90 and n beside it.");
    println!("windows: W1 2010-06-01..2018-06-01, W2 2018-06-01..2026-06-01 (the record's two real windows),");
    println!("         CM the common window every surviving instrument covers (computed, not chosen),");
    println!("         PL pooled over the whole store — PRINTED FOR CONTRAST ONLY, it mixes decades");
    println!("         and must NOT be used to compare instruments.\n");

    let w1 = Era { label: "W1", from_ms: date_ms(2010, 6, 1), to_ms: date_ms(2018, 6, 1) };
    let w2 = Era { label: "W2", from_ms: date_ms(2018, 6, 1), to_ms: date_ms(2026, 6, 1) };

    // ---------------------------------------------------------------- load
    let mut loaded: Vec<(String, Vec<Bar>)> = Vec::new();
    let mut excluded: Vec<(String, String)> = Vec::new();
    let mut declared_symbols: Vec<String> = Vec::new();

    for id in config.markets.keys() {
        let spec = config.market(id)?;
        declared_symbols.push(spec.bar_symbol.clone());
        let path = data.join("bars").join(format!("{}-15m.parquet", spec.bar_symbol));
        match read_bars(&path) {
            Ok(b) if b.is_empty() => {
                excluded.push((id.clone(), format!("{} holds 0 bars — empty is empty, not an instrument with no volatility", path.display())));
            }
            Ok(b) => loaded.push((id.clone(), b)),
            Err(e) => excluded.push((id.clone(), format!("no 15m bar file at {} ({e})", path.display()))),
        }
    }

    let common_from = loaded.iter().map(|(_, b)| b[0].time).max().unwrap_or(0);
    let common_to = loaded.iter().map(|(_, b)| b[b.len() - 1].time).min().unwrap_or(0) + 1;
    let common = Era { label: "CM", from_ms: common_from, to_ms: common_to };

    let mut rows: Vec<Row> = Vec::new();
    for (id, bars) in &loaded {
        let spec = config.market(id)?;
        let rules = trading_rules_for(&config, id)?;
        let d = declared.get(id).cloned().unwrap_or_default();

        let atr_i = fd_indicators::atr(bars, ATR_15M);
        let p_rows = per_bar(bars, &atr_i, STOP_ATR, Some(rules.spread));

        let sessions = daily_bars(bars, SESSION_OFFSET_MS);
        let atr_d = fd_indicators::atr(&sessions, ATR_DAILY);
        let s1_rows = per_bar(&sessions, &atr_d, STOP_ATR, Some(rules.spread));

        let fourhours = bucket(bars, FOURHOUR_MS, 0);
        let atr_h = fd_indicators::atr(&fourhours, ATR_4H);
        let s2_rows = per_bar(&fourhours, &atr_h, STOP_ATR, Some(rules.spread));

        let dist_of = |r: &[(i64, f64, f64)], era: Option<&Era>| -> Option<Dist> {
            let (mut v, mut s) = (Vec::new(), Vec::new());
            for (t, stop, ratio) in r {
                if era.is_some_and(|e| !e.contains(*t)) {
                    continue;
                }
                v.push(*ratio);
                s.push(*stop);
            }
            summarise(&v, &s)
        };

        let per_ruler = |r: &[(i64, f64, f64)]| -> Vec<Option<Dist>> {
            vec![dist_of(r, Some(&w1)), dist_of(r, Some(&w2)), dist_of(r, Some(&common)), dist_of(r, None)]
        };

        rows.push(Row {
            market: id.clone(),
            symbol: spec.trading.symbol.clone(),
            bar_symbol: spec.bar_symbol.clone(),
            bar_source: format!("{:?}", spec.bar_source).to_lowercase(),
            spread: rules.spread,
            contract_size: rules.contract_size,
            min_lot: rules.min_lot,
            lot_step: rules.lot_step,
            equity: rules.starting_equity_usd,
            equity_declared: d.equity,
            swap_long: rules.swap_long_per_lot,
            swap_short: rules.swap_short_per_lot,
            swap_declared: d.swap_long || d.swap_short,
            bars15: bars.len(),
            first: ymd(bars[0].time),
            last: ymd(bars[bars.len() - 1].time),
            flat_share: bars.iter().filter(|b| b.is_synthetic()).count() as f64 * 100.0 / bars.len() as f64,
            dist: vec![per_ruler(&p_rows), per_ruler(&s1_rows), per_ruler(&s2_rows)],
        });
    }

    // ------------------------------------------------- STEP 1: the inventory
    println!("================ STEP 1 — INVENTORY: every markets.* in the config ================\n");
    println!(
        "{:<8} {:<11} {:<9} {:<11} {:>10} {:>9} {:>8} {:>8} {:>11} {:<9}",
        "market", "symbol", "bar_sym", "bar_source", "spread", "contract", "min_lot", "lot_step", "equity USD", "eq.src"
    );
    for r in &rows {
        println!(
            "{:<8} {:<11} {:<9} {:<11} {:>10} {:>9} {:>8} {:>8} {:>11} {:<9}",
            r.market,
            r.symbol,
            r.bar_symbol,
            r.bar_source,
            fmt_price(r.spread),
            format!("{:.2}", r.contract_size),
            format!("{:.3}", r.min_lot),
            format!("{:.3}", r.lot_step),
            format!("{:.1}", r.equity),
            if r.equity_declared { "STATED" } else { "inherit" }
        );
    }
    println!("\neq.src `inherit` = the market states no starting_equity_usd and takes [trading] 10,000.0");
    println!("(engine.rs:354). `STATED` = the market's own block declares it. Defect 17 is NOT universal.\n");

    println!("--- spread provenance (HAND-TRANSCRIBED from config comments; THERE IS NO KEY FOR IT) ---\n");
    println!("{:<8} {:>10} {:>10} {:<10} {}", "market", "charged", "measured", "verdict", "the config's own words");
    for r in &rows {
        let (verdict, words) = spread_provenance(&r.market);
        println!(
            "{:<8} {:>10} {:>10} {:<10} {}",
            r.market,
            fmt_price(r.spread),
            measured_spread(&r.market).map_or("null".to_string(), fmt_price),
            verdict,
            words
        );
    }
    println!("\nNothing in this codebase can distinguish a measured spread from an assumed one:");
    println!("provenance lives only in comments. That is a defect in its own right.\n");

    println!("--- swap: a stated 0.00 and an ABSENT key are the same thing to serde, and they are NOT ---\n");
    println!("{:<8} {:>10} {:>10} {:<24} {}", "market", "swap_long", "swap_short", "key in config?", "reading");
    for r in &rows {
        let reading = if !r.swap_declared {
            "ABSENT -> serde default 0.0. This is `null` wearing a 0: no overnight cost is known."
        } else if r.market == "xauusd" {
            "stated 0.0, and MEASURED on this symbol (340 closed, 87 overnight, swap 0.00, 2026-04..09)"
        } else if r.market == "xauduka" {
            "stated 0.0, measurement BORROWED from xauusd's 5 months and applied to 16 years — a FLOOR"
        } else {
            "stated 0.0 but config says the measurement is NOT on this symbol: account terms, not a reading"
        };
        println!(
            "{:<8} {:>10} {:>10} {:<24} {}",
            r.market,
            format!("{:.2}", r.swap_long),
            format!("{:.2}", r.swap_short),
            if r.swap_declared { "stated" } else { "ABSENT (serde default)" },
            reading
        );
    }
    println!("\nEvery cost/R in this report is spread/stop ONLY. It contains NO swap, NO commission,");
    println!("NO slippage. Where swap is null it is a FLOOR on the true cost, not the cost.\n");

    // ------------------------------------------------- the bar store itself
    println!("--- the bar store: what is actually on disk, per instrument and per candle size ---\n");
    println!("{:<8} {:<9} {:<5} {:>10} {:>12} {:>12} {:>7} {:>8}", "market", "bar_sym", "size", "bars", "first", "last", "flat%", "file MB");
    let mut seen_files: Vec<String> = Vec::new();
    for r in &rows {
        for iv in ["1m", "5m", "15m"] {
            let path = data.join("bars").join(format!("{}-{iv}.parquet", r.bar_symbol));
            let name = format!("{}-{iv}.parquet", r.bar_symbol);
            let mb = std::fs::metadata(&path).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(f64::NAN);
            match read_bars(&path) {
                Ok(b) if !b.is_empty() => {
                    seen_files.push(name);
                    println!(
                        "{:<8} {:<9} {:<5} {:>10} {:>12} {:>12} {:>6.1}% {:>8.1}",
                        r.market,
                        r.bar_symbol,
                        iv,
                        b.len(),
                        ymd(b[0].time),
                        ymd(b[b.len() - 1].time),
                        b.iter().filter(|x| x.is_synthetic()).count() as f64 * 100.0 / b.len() as f64,
                        mb
                    );
                }
                Ok(_) => println!("{:<8} {:<9} {:<5} {:>10} {:>12} {:>12} {:>7} {:>8}", r.market, r.bar_symbol, iv, 0, "-", "-", "null", "-"),
                Err(_) => println!("{:<8} {:<9} {:<5} {:>10} {:>12} {:>12} {:>7} {:>8}", r.market, r.bar_symbol, iv, "absent", "-", "-", "null", "-"),
            }
        }
    }

    // ------------------------------------------ undeclared stores on disk
    println!("\n--- bar stores ON DISK that the config does NOT declare ---\n");
    let mut undeclared: Vec<String> = Vec::new();
    if let Ok(dir) = std::fs::read_dir(data.join("bars")) {
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".parquet") {
                println!("{name}: not a .parquet bar file (kept out of every count below)");
                continue;
            }
            let stem = name.trim_end_matches(".parquet");
            let sym = stem.rsplit_once('-').map_or(stem, |(s, _)| s);
            if !declared_symbols.iter().any(|d| d == sym) {
                undeclared.push(name.clone());
                let mb = e.metadata().map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(f64::NAN);
                match read_bars(&e.path()) {
                    Ok(b) if !b.is_empty() => println!(
                        "{name}: {} bars, {} .. {}, {:.1} MB — NO [markets.*] declares bar_symbol `{sym}`",
                        b.len(),
                        ymd(b[0].time),
                        ymd(b[b.len() - 1].time),
                        mb
                    ),
                    Ok(_) => println!("{name}: 0 bars, {mb:.1} MB — undeclared AND empty"),
                    Err(err) => println!("{name}: unreadable ({err}), {mb:.1} MB — undeclared"),
                }
            }
        }
    }
    if undeclared.is_empty() {
        println!("none: every .parquet under data/bars is claimed by a markets.* block");
    }

    // ------------------------------------------------- volume census
    println!("\n--- STEP 1b — the volume column, per instrument and per candle size ---\n");
    println!("Column states: null (provider publishes nothing) / MANUFACTURED 0 (a null->0 step) /");
    println!("tick (mt5_export.py:33-34: `CFD real volume is always zero`) / real (an exchange's own).\n");
    println!("{:<8} {:<9} {:<5} {:>9} {:>9} {:>9} {:>9} {:>7} {:<34}", "market", "bar_sym", "size", "rows", "null", "zero", ">0", "distinct", "verdict");
    for r in &rows {
        let (stated, _) = volume_provenance(&r.bar_source);
        for iv in ["1m", "5m", "15m"] {
            let path = data.join("bars").join(format!("{}-{iv}.parquet", r.bar_symbol));
            let Ok(b) = read_bars(&path) else {
                println!("{:<8} {:<9} {:<5} {:>9} {:>9} {:>9} {:>9} {:>7} {:<34}", r.market, r.bar_symbol, iv, "absent", "-", "-", "-", "-", "null (no file)");
                continue;
            };
            let c = volume_census(&b);
            println!(
                "{:<8} {:<9} {:<5} {:>9} {:>9} {:>9} {:>9} {:>7} {:<34}",
                r.market,
                r.bar_symbol,
                iv,
                c.n,
                c.null,
                c.zero,
                c.positive,
                if c.distinct_capped >= 64 { ">=64".to_string() } else { c.distinct_capped.to_string() },
                volume_verdict(&c, stated)
            );
        }
    }
    for src in ["mt5", "binance", "dukascopy", "reference"] {
        let (s, why) = volume_provenance(src);
        println!("  bar_source {src:<10} -> stated `{s}`: {why}");
    }

    // A column can change state inside one file. A single census over the whole
    // file averages that away, so the same census is printed per calendar year.
    println!("\n--- the volume column PER YEAR: a column can change state inside one file ---\n");
    println!("{:<8} {:<5} {:>6} {:>10} {:>8} {:>8} {:>14}", "market", "size", "year", "rows", "null%", "zero%", "median of >0");
    for r in &rows {
        for iv in ["1m", "15m"] {
            let path = data.join("bars").join(format!("{}-{iv}.parquet", r.bar_symbol));
            let Ok(b) = read_bars(&path) else { continue };
            let mut by_year: BTreeMap<i64, Vec<Option<f64>>> = BTreeMap::new();
            for x in &b {
                let (y, _, _) = fd_core::clock::civil_from_days(x.time.div_euclid(DAY_MS));
                by_year.entry(y).or_default().push(x.volume);
            }
            for (y, v) in by_year {
                let n = v.len();
                let nulls = v.iter().filter(|x| x.is_none()).count();
                let zeros = v.iter().filter(|x| **x == Some(0.0)).count();
                let mut pos: Vec<f64> = v.iter().filter_map(|x| *x).filter(|x| *x > 0.0).collect();
                pos.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let med = fd_backtest::cost_table::quantile(&pos, 0.5);
                println!(
                    "{:<8} {:<5} {:>6} {:>10} {:>7.1}% {:>7.1}% {:>14}",
                    r.market,
                    iv,
                    y,
                    n,
                    nulls as f64 * 100.0 / n as f64,
                    zeros as f64 * 100.0 / n as f64,
                    med.map_or("null".to_string(), |m| format!("{m:.1}"))
                );
            }
        }
    }

    // the XAUUSD shape change, cheap to check and already claimed in the record
    for r in rows.iter().filter(|r| r.bar_symbol == "XAUUSD") {
        let path = data.join("bars").join(format!("{}-15m.parquet", r.bar_symbol));
        if let Ok(b) = read_bars(&path) {
            let cut = date_ms(2023, 12, 1);
            let mean = |it: Vec<f64>| if it.is_empty() { f64::NAN } else { it.iter().sum::<f64>() / it.len() as f64 };
            let before = mean(b.iter().filter(|x| x.time < cut).filter_map(|x| x.volume).collect());
            let after = mean(b.iter().filter(|x| x.time >= cut).filter_map(|x| x.volume).collect());
            println!("\n  XAUUSD tick-volume LEVEL across 2023-12: mean before {before:.1} / after {after:.1} = x{:.2}", after / before);
        }
    }

    // ------------------------------------------------- STEP 2: cost/R
    println!("\n\n================ STEP 2 — cost/R = spread/stop, matched horizons ================\n");
    println!("Every figure carries FOUR things: window, statistic, stop size (in the instrument's own");
    println!("price units), and whether the spread behind it is MEASURED or ASSUMED.\n");

    for (ri, rname) in RULERS.iter().enumerate() {
        println!("--- ruler {rname} ---\n");
        println!("{:<8} {:<10}", "market", "spread");
        for (wi, wname) in WINDOWS.iter().enumerate() {
            println!("  window {wname}:");
            println!(
                "    {:<8} {:>12} {:>11} {:>10} {:>10} {:>9} {:<9}",
                "market", "med stop", "med cost/R", "p10", "p90", "n", "spread?"
            );
            for r in &rows {
                let (prov, _) = spread_provenance(&r.market);
                match r.dist[ri][wi] {
                    Some(d) => println!(
                        "    {:<8} {:>12} {:>10.3}% {:>9.3}% {:>9.3}% {:>9} {:<9}",
                        r.market,
                        fmt_price(d.median_stop),
                        d.p50 * 100.0,
                        d.p10 * 100.0,
                        d.p90 * 100.0,
                        d.n,
                        prov
                    ),
                    None => println!(
                        "    {:<8} {:>12} {:>11} {:>10} {:>10} {:>9} {:<9}",
                        r.market, "-", "null", "null", "null", 0, prov
                    ),
                }
            }
            println!();
        }
    }

    println!("--- the same three rulers, one line per instrument, medians only ---\n");
    print!("{:<8}", "market");
    for rname in RULERS {
        for wname in WINDOWS {
            print!(" {:>22}", format!("{}/{}", &rname[..2], &wname[..2]));
        }
    }
    println!();
    for r in &rows {
        print!("{:<8}", r.market);
        for ri in 0..3 {
            for wi in 0..4 {
                print!(" {:>22}", fmt_dist(r.dist[ri][wi]));
            }
        }
        println!();
    }
    println!(
        "\ncommon window CM = {} .. {} (computed from the surviving instruments, not chosen)",
        ymd(common.from_ms),
        ymd(common.to_ms - 1)
    );
    println!("PL (pooled) mixes decades and must NOT be used to compare instruments.");

    // --- the same table at the MEASURED spread, where one exists -------------
    println!("\n--- ruler P at the MEASURED spread (exact rescale: the ratio is linear in the spread) ---\n");
    println!("{:<8} {:>10} {:>10} {:>18} {:>18} {:<10}", "market", "charged", "measured", "W1 at measured", "W2 at measured", "verdict");
    for r in &rows {
        let (prov, _) = spread_provenance(&r.market);
        let k = measured_spread(&r.market).map(|m| m / r.spread);
        let at = |d: Option<Dist>| -> String {
            match (d, k) {
                (Some(d), Some(k)) => format!("{:.3}%", d.p50 * k * 100.0),
                _ => "null".into(),
            }
        };
        println!(
            "{:<8} {:>10} {:>10} {:>18} {:>18} {:<10}",
            r.market,
            fmt_price(r.spread),
            measured_spread(&r.market).map_or("null".into(), fmt_price),
            at(r.dist[0][0]),
            at(r.dist[0][1]),
            prov
        );
    }
    println!("\nA spread whose provenance is an assumption or a single read has NO measured figure.");
    println!("That is null. Null is not the charged number wearing a different label.");

    // ------------------------------------------------- STEP 3: the two marks
    let gold_bench = arg("gold", "xauduka");
    let gold = rows.iter().find(|r| r.market == gold_bench);
    println!("\n\n================ STEP 3 — the two comparison marks, beside every row ================\n");
    println!("mark 1: cost/R of GOLD 15m (market `{gold_bench}`, ruler P), same window, same ruler.");
    println!("mark 2: the mould's threshold. `agent/mould-target` requires gross/spread >= {MOULD_REQUIRED}x per");
    println!("        exposed session; the MEASURED ceiling on gold is {GOLD_CEILING}x. So the factor another");
    println!("        market must beat gold's cost by, to carry the SAME signal into the mould, is");
    println!("        {MOULD_REQUIRED} / {GOLD_CEILING} = {:.3}x.\n", needed_factor());
    println!("  !! THIS CONVERSION DECLARES AN ASSUMPTION, and the assumption is NOT measured:");
    println!("     it assumes the SAME signal transplants — same gross in R units — so that gross/spread");
    println!("     scales exactly as cost_gold / cost_instrument. Nothing in this record has measured");
    println!("     a gold signal's gross on another instrument. The factor column is therefore a");
    println!("     NECESSARY condition, not a sufficient one.\n");

    match gold {
        None => println!("gold benchmark `{gold_bench}` produced no row — the marks cannot be computed. null, not 0."),
        Some(g) => {
            for (wi, wname) in WINDOWS.iter().enumerate() {
                println!("--- window {wname}, ruler P ---\n");
                let gd = g.dist[0][wi];
                match gd {
                    None => println!("  gold has no figure in this window -> every factor here is null, not 0.\n"),
                    Some(gd) => {
                        println!("  gold mark: {:.3}% of R (median, n {})", gd.p50 * 100.0, gd.n);
                        println!(
                            "  {:<8} {:>11} {:>11} {:>13} {:<9} {:>14} {:<28}",
                            "market", "cost/R", "gold cost/R", "x cheaper", "spread?", "spread needed", "screen (needs >= 4.576x)"
                        );
                        for r in &rows {
                            let (prov, _) = spread_provenance(&r.market);
                            match r.dist[0][wi] {
                                None => println!(
                                    "  {:<8} {:>11} {:>11} {:>13} {:<9} {:>14} {:<28}",
                                    r.market,
                                    "null",
                                    format!("{:.3}%", gd.p50 * 100.0),
                                    "null",
                                    prov,
                                    "null",
                                    "NOT COVERED in this window"
                                ),
                                Some(d) => {
                                    let factor = gd.p50 / d.p50;
                                    // the spread this instrument would need for cost/R = gold/4.576
                                    let target_cost = gd.p50 / needed_factor();
                                    let needed_spread = r.spread * (target_cost / d.p50);
                                    let passes = factor >= needed_factor();
                                    let verdict = if r.market == gold_bench {
                                        "-- the benchmark itself".to_string()
                                    } else if !passes {
                                        format!("NO  ({:.2}x short of 4.576x)", needed_factor() / factor)
                                    } else if prov != "MEASURED" {
                                        format!("passes BUT spread is {prov}")
                                    } else {
                                        "YES, and spread is MEASURED".to_string()
                                    };
                                    println!(
                                        "  {:<8} {:>10.3}% {:>10.3}% {:>12.2}x {:<9} {:>14} {:<28}",
                                        r.market,
                                        d.p50 * 100.0,
                                        gd.p50 * 100.0,
                                        factor,
                                        prov,
                                        fmt_price(needed_spread),
                                        verdict
                                    );
                                }
                            }
                        }
                        println!();
                    }
                }
            }
        }
    }

    // ------------------------------------------------- defect 17, per market
    println!("--- defect 17, checked PER MARKET: does the risk sizing survive min_lot? ---\n");
    println!("raw_lots = (equity x 0.01) / (median stop x contract_size); lots = max(floor(raw/lot_step)*lot_step, min_lot)");
    println!("(engine.rs:729-730). A clamped market's PF_usd is the PF of a min_lot book, not of a 1%-risk book.\n");
    println!("The CRITICAL STOP is the stop above which raw_lots falls below min_lot and the clamp bites:");
    println!("  critical stop = (equity x 0.01) / (min_lot x contract_size), in the instrument's price units.");
    println!("A market is not clamped or unclamped: it is clamped ABOVE A STOP SIZE, and that is the number.\n");
    println!(
        "{:<8} {:>11} {:>14} {:>11} {:>9} {:>11} {:>9} {:>11} {:>9}",
        "market", "equity USD", "critical stop", "med stop P", "P clamp?", "med stop S1", "S1 clamp?", "med stop S2", "S2 clamp?"
    );
    for r in &rows {
        let crit = (r.equity * 0.01) / (r.min_lot * r.contract_size);
        let state = |d: Option<Dist>| -> (String, String) {
            match d {
                None => ("null".into(), "null".into()),
                Some(d) => {
                    let raw = (r.equity * 0.01) / (d.median_stop * r.contract_size);
                    let stepped = (raw / r.lot_step).floor() * r.lot_step;
                    (fmt_price(d.median_stop), if stepped < r.min_lot { format!("CLAMPED x{:.1}", r.min_lot / raw) } else { format!("no ({raw:.3})") })
                }
            }
        };
        let (p_stop, p_cl) = state(r.dist[0][3]);
        let (s1_stop, s1_cl) = state(r.dist[1][3]);
        let (s2_stop, s2_cl) = state(r.dist[2][3]);
        println!(
            "{:<8} {:>11} {:>14} {:>11} {:>9} {:>11} {:>9} {:>11} {:>9}",
            r.market,
            format!("{:.1}", r.equity),
            fmt_price(crit),
            p_stop,
            p_cl,
            s1_stop,
            s1_cl,
            s2_stop,
            s2_cl
        );
    }
    println!("\n`no (x)` prints raw_lots. Defect 17 as stated (`every xauusd line runs at min_lot`) was");
    println!("measured at a WIDE stop: the clamp is a property of the stop size, not of the market alone.");

    // ------------------------------------------------- provider print
    println!("\n\n================ PROVIDER PRINT — open==low / close==low, EVERY instrument ================\n");
    println!("Two independent agents measured this around the 17:00 New York CME break on XAUDUKA and");
    println!("XAGDUKA. Here it runs on every instrument in the store, on 15m bars, under TWO definitions");
    println!("of `the bar that reopens`, because a definition is a choice:\n");
    println!("  gap  = the first bar after a hole in the series longer than one interval and shorter than");
    println!("         a day, i.e. the session break, whatever the clock says");
    println!("  ny18 = the bar whose open stamps 18:00 New York local — the CME reopen, which still");
    println!("         exists when the feed fills the break with bars");
    println!("  ord  = every other bar\n");
    println!("F5 fires on a row when a reopen share is >= 2x the ordinary share.\n");
    for r in &rows {
        let path = data.join("bars").join(format!("{}-15m.parquet", r.bar_symbol));
        let Ok(b) = read_bars(&path) else { continue };
        let (gap, ny18, ord, hours) = print_census(&b, 900_000);
        println!("{} ({}, {} bars 15m, {})", r.market, r.bar_symbol, b.len(), r.bar_source);
        println!("   gap  {}", gap.line());
        println!("   ny18 {}", ny18.line());
        println!("   ord  {}", ord.line());
        let ratio = |num: usize, den: usize, dn: usize, dd: usize| -> f64 {
            if den == 0 || dd == 0 || dn == 0 { f64::NAN } else { (num as f64 / den as f64) / (dn as f64 / dd as f64) }
        };
        let g_ol = ratio(gap.open_eq_low, gap.n, ord.open_eq_low, ord.n);
        let g_cl = ratio(gap.close_eq_low, gap.n, ord.close_eq_low, ord.n);
        let n_ol = ratio(ny18.open_eq_low, ny18.n, ord.open_eq_low, ord.n);
        let n_cl = ratio(ny18.close_eq_low, ny18.n, ord.close_eq_low, ord.n);
        let lifts = [g_ol, g_cl, n_ol, n_cl];
        let fires = lifts.iter().any(|x| x.is_finite() && *x >= 2.0);
        let measurable = lifts.iter().any(|x| x.is_finite());
        println!(
            "   lift vs ord: gap o==l x{g_ol:.2}  gap c==l x{g_cl:.2}  ny18 o==l x{n_ol:.2}  ny18 c==l x{n_cl:.2}   => F5 {}",
            if fires {
                "FIRES — this instrument carries a provider print".to_string()
            } else if !measurable {
                // null, not a pass: a test with no reopen bars said nothing.
                format!(
                    "NOT MEASURABLE — null, not `does not fire`: {} gap-reopen and {} NY-18:00 bars",
                    gap.n, ny18.n
                )
            } else {
                "does not fire".to_string()
            }
        );
        let top: Vec<String> = hours.iter().map(|(h, c)| format!("{h:02}h:{c}")).collect();
        println!("   NY-local hour of the gap-reopens: {}\n", if top.is_empty() { "none (no intraday gaps)".to_string() } else { top.join(" ") });
    }

    // ------------------------------------------------- positive controls
    println!("\n================ POSITIVE CONTROLS — the instrument must be shown to SEE ================\n");
    println!("Addendum 10 VII: prove the tool sees before reporting any negative. And write a probe that");
    println!("deliberately looks, then confirm the probe FIRES — without that, a non-firing test may be blind.\n");

    if let Some(g) = gold {
        let path = data.join("bars").join(format!("{}-15m.parquet", g.bar_symbol));
        if let Ok(b) = read_bars(&path) {
            let atr = fd_indicators::atr(&b, ATR_15M);
            let base = summarise(
                &per_bar(&b, &atr, STOP_ATR, Some(g.spread)).iter().map(|(_, _, r)| *r).collect::<Vec<_>>(),
                &per_bar(&b, &atr, STOP_ATR, Some(g.spread)).iter().map(|(_, s, _)| *s).collect::<Vec<_>>(),
            );
            let tenth = summarise(
                &per_bar(&b, &atr, STOP_ATR, Some(g.spread / 10.0)).iter().map(|(_, _, r)| *r).collect::<Vec<_>>(),
                &per_bar(&b, &atr, STOP_ATR, Some(g.spread / 10.0)).iter().map(|(_, s, _)| *s).collect::<Vec<_>>(),
            );
            match (base, tenth) {
                (Some(a), Some(c)) => {
                    let f = a.p50 / c.p50;
                    println!("control 1 — cost ruler, on gold's OWN bars at 1/10 of gold's spread:");
                    println!("  gold spread {:.4} -> {:.3}% of R ; spread/10 {:.4} -> {:.3}% of R ; factor x{f:.3}", g.spread, a.p50 * 100.0, g.spread / 10.0, c.p50 * 100.0);
                    println!(
                        "  the 4.576x screen on this synthetic instrument: {} (it MUST pass, or the screen is blind)\n",
                        if f >= needed_factor() { "PASSES" } else { "FAILS — THE SCREEN IS BLIND, discard every negative below" }
                    );
                }
                _ => println!("control 1 could not be computed — null. No negative in this report is readable.\n"),
            }
        }
    }

    // control 2: provider-print, three synthetic series
    let mk = |f: &dyn Fn(usize) -> Bar, n: usize| -> Vec<Bar> { (0..n).map(f).collect() };
    let all_open_is_low = mk(
        &|i| {
            let p = 100.0 + i as f64 * 0.01;
            Bar { time: i as i64 * 900_000, open: p, high: p + 0.5, low: p, close: p + 0.25, volume: None }
        },
        1_000,
    );
    let continuous = mk(
        &|i| {
            // a deterministic wobble so no bar's open coincides with its low
            let p = 100.0 + (i as f64 * 0.37).sin();
            Bar { time: i as i64 * 900_000, open: p, high: p + 0.9, low: p - 0.9, close: p + 0.1, volume: None }
        },
        1_000,
    );
    let c_all = {
        let mut c = PrintCensus::default();
        for b in &all_open_is_low {
            c.add(b);
        }
        c
    };
    let c_cont = {
        let mut c = PrintCensus::default();
        for b in &continuous {
            c.add(b);
        }
        c
    };
    println!("control 2 — the provider-print test, on two synthetic series:");
    println!("  every bar built with open = low : {}", c_all.line());
    println!("  continuous prices, open inside  : {}", c_cont.line());
    println!(
        "  reads 100% / ~0% as it must: {}\n",
        if c_all.open_eq_low == c_all.n && c_cont.open_eq_low == 0 { "YES" } else { "NO — THE PRINT TEST IS BLIND" }
    );

    // control 3: a probe that deliberately looks, which MUST fire F5
    let mut probe: Vec<Bar> = Vec::new();
    for i in 0..2_000usize {
        let p = 100.0 + (i as f64 * 0.11).sin();
        // a gap every 96 bars, i.e. one break a (24h) day on 15m bars
        let t = i as i64 * 900_000 + (i as i64 / 96) * 900_000;
        let is_reopen = i % 96 == 0 && i > 0;
        probe.push(if is_reopen {
            Bar { time: t, open: p - 0.9, high: p + 0.9, low: p - 0.9, close: p + 0.1, volume: None }
        } else {
            Bar { time: t, open: p, high: p + 0.9, low: p - 0.9, close: p + 0.1, volume: None }
        });
    }
    let (pg, _, po, _) = print_census(&probe, 900_000);
    let lift = if po.n == 0 || po.open_eq_low == 0 {
        f64::INFINITY
    } else {
        (pg.open_eq_low as f64 / pg.n as f64) / (po.open_eq_low as f64 / po.n as f64)
    };
    println!("control 3 — a probe that deliberately prints (open = low ONLY on the reopen bar):");
    println!("  reopen bars {}", pg.line());
    println!("  ordinary    {}", po.line());
    println!(
        "  lift x{lift:.2} => F5 {} (it MUST fire; if it does not, every `does not fire` above is blind)\n",
        if lift >= 2.0 { "FIRES, as it must" } else { "DOES NOT FIRE — THE F5 TEST IS BLIND" }
    );

    println!("\n--- excluded, and why ---\n");
    if excluded.is_empty() {
        println!("none");
    }
    for (id, why) in &excluded {
        println!("{id}: {why}");
    }

    Ok(())
}
