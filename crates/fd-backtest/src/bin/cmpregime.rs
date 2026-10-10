//! TIEN KIEM: does a companion instrument's **simultaneous** state separate
//! the outcomes of a gold mechanism's trades better than a random filter with
//! the same cut ratio?
//!
//! Registered in `docs/decisions/2026-10-10-companion-regime.md` before this
//! file existed. This is NOT the lead-lag question (`agent/lead-lag`, closed):
//! nothing here reads the companion at a lag. It reads the companion bar
//! stamped with the **signal bar's own timestamp**, which closes at the same
//! instant the signal bar closes, one bar before the engine fills — the lag-0
//! reading `agent/lead-lag` measured at `corr +0.6900` / `+0.7446` and
//! explicitly left open.
//!
//! # Why this partitions instead of filtering
//!
//! A filter CHANGES the trade set: the engine holds one position, so removing
//! an entry lets a later one happen that the unfiltered run never saw
//! (`month-clock` measured +17…+22 trades, `m8` +36). That makes the two arms
//! of a filter incomparable as halves of one book, and it costs gate cells.
//! The pre-check question is narrower and cheaper: run the mechanism ONCE,
//! unfiltered, then label each of its real trades by the companion state at
//! its signal bar. If the labels carry no information about the outcome, no
//! filter built from them can help, and no gate cell needs to be spent.
//!
//! # The three controls
//!
//! A filter that cuts half the trades moves PF even when it is meaningless, so
//! the effect is read against:
//!
//! 1. **time shuffle** — the per-bar companion value is permuted across the
//!    bars of the window (Fisher-Yates, seeded), so the marginal frequency of
//!    every state is preserved exactly and the trades are relabelled from the
//!    shuffled series. This is the "random filter with the same cut ratio".
//! 2. **label shuffle** — the labels are permuted among the trades, keeping
//!    the two group sizes exactly. This is the exact permutation test of
//!    "does this label know anything about this outcome".
//! 3. **day-block shuffle** — the time shuffle in contiguous blocks of 96 bars
//!    (one 15m trading day), which preserves intraday structure, so an effect
//!    that is really a time-of-day effect cannot borrow the narrow null of a
//!    full shuffle.
//!
//! Run:
//! ```text
//! cmpregime --data=/e/rust/flowdesk/data --config=config --market=xauduka \
//!           --interval=15m --draws=1000 --seed=12345
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use fd_backtest::engine::{Range, TradingRules, run_backtest_guarded, trading_rules_for};
use fd_backtest::Guards;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_indicators::companion::{Companion, aligned_change};
use fd_store::read_bars;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{Params, Registry};

/* ------------------------------------------------------------------ flags */

fn arg(name: &str, default: &str) -> String {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).map(str::to_string)).unwrap_or_else(|| default.to_string())
}

fn day_ms(text: &str) -> i64 {
    let mut parts = text.split('-').map(|p| p.parse::<i64>().expect("YYYY-MM-DD"));
    let (y, m, d) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
    fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000
}

/* ------------------------------------------------------------------- RNG */

/// xoshiro-style splitmix64. Written out rather than pulled in so the draw
/// sequence is a property of this file and a receipt can be reproduced from
/// the seed alone.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xDEAD_BEEF_CAFE_BABE)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `0..n`, rejection-free enough for this purpose (n is small
    /// against 2^64 and the modulo bias is below 1e-14).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % (n as u64)) as usize
    }
}

/* --------------------------------------------------------- state families */

/// The per-bar companion quantity a state definition reads, and the rule that
/// turns it (plus the trade's side) into "in state" or "out of state".
#[derive(Clone, Copy, PartialEq, Eq)]
enum StateKind {
    /// `change / atr` of the companion over its own last closed bar. In state
    /// when it agrees with the gold trade's side and |k| >= 0.25.
    Dir,
    /// The same k. In state when |k| >= 0.75, whatever the side.
    Move,
    /// `atr/close` of the companion minus its own trailing 500-bar median.
    /// In state when positive.
    Vol,
}

impl StateKind {
    fn id(self) -> &'static str {
        match self {
            Self::Dir => "dir",
            Self::Move => "move",
            Self::Vol => "vol",
        }
    }
    fn describe(self) -> &'static str {
        match self {
            Self::Dir => "companion 1-bar move AGREES with the trade side, |k| >= 0.25 comp-ATR",
            Self::Move => "|companion 1-bar move| >= 0.75 comp-ATR, either direction",
            Self::Vol => "companion ATR/close ABOVE its own trailing 500-bar median",
        }
    }
    /// Is this trade in state, given the per-bar value at its signal bar?
    fn in_state(self, v: f64, long: bool) -> bool {
        match self {
            Self::Dir => v.abs() >= 0.25 && (v > 0.0) == long,
            Self::Move => v.abs() >= 0.75,
            Self::Vol => v > 0.0,
        }
    }
}

/// Per-bar values for one companion, indexed by PRIMARY bar index. `NaN` is
/// "the companion carries no bar at this primary bar's timestamp, or its
/// lookback crosses a break" — missing, never zero and never the bar before.
struct CompanionSeries {
    symbol: String,
    bars: usize,
    /// `change/atr` of the companion, at each primary bar.
    k: Vec<f64>,
    /// `atr/close` in percent minus its own trailing median, at each primary bar.
    vol_excess: Vec<f64>,
    matched: usize,
}

impl CompanionSeries {
    fn value(&self, kind: StateKind, i: usize) -> f64 {
        match kind {
            StateKind::Dir | StateKind::Move => self.k[i],
            StateKind::Vol => self.vol_excess[i],
        }
    }
}

/// The trailing median of the previous `window` finite values of `x`, as of
/// each index, computed on a sorted window so nothing later than `i-1` enters.
/// `NaN` until the window is full.
fn trailing_median(x: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; x.len()];
    // Sorted window of the last `window` FINITE values, with the order they
    // arrived in so the oldest can be removed.
    let mut sorted: Vec<f64> = Vec::with_capacity(window + 1);
    let mut queue: std::collections::VecDeque<f64> = std::collections::VecDeque::with_capacity(window + 1);
    for i in 0..x.len() {
        if sorted.len() == window {
            out[i] = sorted[window / 2];
        }
        let v = x[i];
        if v.is_finite() {
            let at = sorted.partition_point(|p| *p < v);
            sorted.insert(at, v);
            queue.push_back(v);
            if queue.len() > window {
                let old = queue.pop_front().expect("len > window");
                let at = sorted.partition_point(|p| *p < old);
                    sorted.remove(at);
            }
        }
    }
    out
}

fn companion_series(primary: &[Bar], symbol: &str, interval: &str, data: &std::path::Path) -> Result<CompanionSeries, String> {
    let path = data.join("bars").join(format!("{symbol}-{interval}.parquet"));
    let bars = read_bars(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let n = bars.len();
    let comp = Companion::new(symbol, interval, bars);
    // period 1: the companion's own last closed bar. atrPeriod 14, the same
    // ATR period every mechanism here sizes on.
    let (change, atr, close) = aligned_change(primary, &comp, 1, 14);
    let k: Vec<f64> = change.iter().zip(&atr).map(|(c, a)| if a.is_finite() && *a > 0.0 { c / a } else { f64::NAN }).collect();
    let vol_pct: Vec<f64> =
        atr.iter().zip(&close).map(|(a, c)| if a.is_finite() && c.is_finite() && *c != 0.0 { 100.0 * a / c } else { f64::NAN }).collect();
    let median = trailing_median(&vol_pct, 500);
    let vol_excess: Vec<f64> =
        vol_pct.iter().zip(&median).map(|(v, m)| if v.is_finite() && m.is_finite() { v - m } else { f64::NAN }).collect();
    let matched = close.iter().filter(|v| v.is_finite()).count();
    Ok(CompanionSeries { symbol: symbol.to_string(), bars: n, k, vol_excess, matched })
}

/* ----------------------------------------------------------------- groups */

#[derive(Default, Clone)]
struct Book {
    n: usize,
    total_r: f64,
    gross_win_r: f64,
    gross_loss_r: f64,
    wins: usize,
    exits: BTreeMap<String, usize>,
}

impl Book {
    fn add(&mut self, r: f64, label: &str) {
        self.n += 1;
        self.total_r += r;
        if r > 0.0 {
            self.gross_win_r += r;
            self.wins += 1;
        } else {
            self.gross_loss_r += -r;
        }
        *self.exits.entry(label.to_string()).or_default() += 1;
    }
    /// `E = total_r / n` — the brief's rule, not the engine's 3-digit print.
    fn e(&self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.total_r / self.n as f64 }
    }
    /// `PF_r`: a property of the METHOD. Never `PF_usd`, which is a property
    /// of the leverage path and which these sub-books do not have one of.
    fn pf_r(&self) -> f64 {
        if self.gross_loss_r <= 0.0 { f64::NAN } else { self.gross_win_r / self.gross_loss_r }
    }
    /// `Lbar`: gross loss per trade, in R. The gate's expectancy leg is
    /// redundant exactly when this is >= 0.250 R.
    fn lbar(&self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.gross_loss_r / self.n as f64 }
    }
    fn win_pct(&self) -> f64 {
        if self.n == 0 { f64::NAN } else { 100.0 * self.wins as f64 / self.n as f64 }
    }
}

fn fmt(v: f64, dp: usize) -> String {
    if v.is_finite() { format!("{v:.dp$}") } else { "null".to_string() }
}

/// One labelled trade: everything the pre-check needs and nothing else.
struct Labelled {
    r: f64,
    long: bool,
    signal_bar: usize,
    exit: String,
}

/* ------------------------------------------------------------------- main */

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = PathBuf::from(arg("data", "/e/rust/flowdesk/data"));
    let config = Config::load(arg("config", "config"))?;
    let market = arg("market", "xauduka");
    let interval = arg("interval", "15m");
    let draws: usize = arg("draws", "1000").parse()?;
    let seed: u64 = arg("seed", "12345").parse()?;
    // `--drop-break=<minutes>` removes every trade whose SIGNAL bar sits
    // within that many minutes of the 17:00 New York CME break.
    //
    // WHY: `agent/hour-screen` measured a PROVIDER PRINT ARTEFACT at that
    // boundary on BOTH Dukascopy feeds - the reopening bar's `open` IS its
    // `low` on 19.02% of gold reopens and 24.16% of silver's, against 3.20%
    // and 4.86% of ordinary bars, and it nearly vanishes on the broker's own
    // XAUUSD feed (6.75%). Two feeds from one provider printing the same
    // artefact on the same bars would read as "the companion is in phase",
    // which is exactly the lag-0 correlation this job builds on. So the
    // effect is reported with and without those bars, and if it needs them it
    // is the provider's pen and not a regime.
    let drop_break: u32 = arg("drop-break", "0").parse()?;
    let spec = config.market(&market)?;
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    let bars = read_bars(&bars_path)?;

    println!("== cmpregime: companion as a SIMULTANEOUS regime label on an existing gold mechanism ==");
    println!("registered: docs/decisions/2026-10-10-companion-regime.md");
    println!("market:   {market} {interval}  ({})", spec.label);
    println!("data:     {} (bars from {})", data.display(), bars_path.display());
    println!("bars:     {} from {} to {}", bars.len(), iso(bars[0].time), iso(bars[bars.len() - 1].time));

    let rules: TradingRules = trading_rules_for(&config, &market)?;
    println!("spread:   {} per round trip", rules.spread);
    println!("equity:   starting_equity_usd = {} (defect 17 applies only at 100 USD)", rules.starting_equity_usd);
    println!("min_lot:  {}   lot_step {}   contract_size {}", rules.min_lot, rules.lot_step, rules.contract_size);

    // The calendar the news guard reads. The path is NAMED because the engine
    // otherwise reads `data/news/events.parquet` whatever `--data=` says.
    let news_path = data.join("news").join("events.parquet");
    if news_path.is_file() {
        match fd_store::read_news(&news_path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            Ok(n) => println!("news:     {n} events from {}", news_path.display()),
            Err(e) => println!("news:     could not load {}: {e}", news_path.display()),
        }
    } else {
        println!("news:     none at {} — the news guard is a no-op", news_path.display());
    }
    println!("draws:    {draws} per control, seed {seed}");
    if drop_break > 0 {
        println!(
            "drop-break: trades whose SIGNAL bar is within {drop_break} min of 17:00 New York are REMOVED (provider print artefact, agent/hour-screen)"
        );
    } else {
        println!("drop-break: 0 - every bar kept, including the CME-break boundary");
    }

    let mut registry = Registry::new();
    fd_strategy::builtin::register_all(&mut registry);

    let mechanisms = ["ema-cross", "rsi-reversion", "donchian-breakout", "bb-fade"];
    // `--quarters` replaces the desk's two windows with four four-year
    // slices. It is the stability read (window artefact #12), declared in the
    // dated note of the registration before it was run; a dE that flips sign
    // between adjacent quarters is an artefact, not a regime.
    let quarters = std::env::args().any(|a| a == "--quarters");
    let windows: &[(&str, &str, &str)] = if quarters {
        &[
            ("Q1", "2010-06-01", "2014-06-01"),
            ("Q2", "2014-06-01", "2018-06-01"),
            ("Q3", "2018-06-01", "2022-06-01"),
            ("Q4", "2022-06-01", "2026-06-01"),
        ]
    } else {
        &[("A", "2010-06-01", "2018-06-01"), ("B", "2018-06-01", "2026-06-01")]
    };
    let states = [StateKind::Dir, StateKind::Move, StateKind::Vol];
    // The third entry is the PRIMARY ITSELF. It is the baseline that makes the
    // measurement a comparison rather than a reading: gold's own volatility
    // regime, read through exactly the same code path, matched at 100% of
    // timestamps by construction. `agent/lead-lag` closed its axis on the
    // equivalent baseline (silver's own past against gold's past), and the
    // trap it found is the one that matters here too - a companion figure that
    // merely reproduces the primary's own state is not information from a
    // second instrument.
    let companion_names = [
        arg("companion-a", "XAGDUKA"),
        arg("companion-b", "EURDUKA"),
        arg("baseline", spec.bar_symbol.as_str()),
    ];

    let mut companions = Vec::new();
    for name in &companion_names {
        let s = companion_series(&bars, name, &interval, &data)?;
        println!(
            "companion: {}-{interval} {} bars; exact-timestamp matches on {} of {} primary bars ({:.1}%); k defined at {}, vol_excess at {}",
            s.symbol,
            s.bars,
            s.matched,
            bars.len(),
            100.0 * s.matched as f64 / bars.len() as f64,
            s.k.iter().filter(|v| v.is_finite()).count(),
            s.vol_excess.iter().filter(|v| v.is_finite()).count(),
        );
        companions.push(s);
    }
    println!("states:");
    for st in states {
        println!("  {:<5} {}", st.id(), st.describe());
    }
    println!(
        "declared comparisons: {} companions x {} states x {} mechanisms = {} (the {} row is the BASELINE, not a hypothesis)",
        companions.len() - 1,
        states.len(),
        mechanisms.len(),
        (companions.len() - 1) * states.len() * mechanisms.len(),
        companion_names[companion_names.len() - 1],
    );
    println!();

    // One sorted copy of the bar times, so a fill time becomes a bar index.
    let times: Vec<i64> = bars.iter().map(|b| b.time).collect();

    // (arm, companion, state, mechanism, window) -> what that read said.
    let mut ledger: BTreeMap<(bool, String, &str, &str, &str), Outcome> = BTreeMap::new();

    for guards_on in [true, false] {
        let guards = if guards_on { Some(Guards::for_market(&config, &market)?) } else { None };
        println!("######## guards {} ########", if guards_on { "ON (the only arm the owner permits)" } else { "OFF (read only, not a trading arm)" });
        for (wname, from, to) in windows.iter().copied() {
            let range = Range { from: Some(day_ms(from)), to: Some(day_ms(to)) };
            for mech in mechanisms {
                let strategy = registry.get(mech)?;
                let params: Params = strategy.default_params();
                // `weekdays` + `flat:1630-1815` — the swap-free intraday arm
                // the record's `gold-intraday` batch puts on every base
                // method, so this measures the mechanism the desk would run.
                let gated = Filtered { inner: strategy, filters: vec![Filter::weekdays(), Filter::flat(1630, 1815)] };
                let result = run_backtest_guarded(&bars, &gated, &params, &rules, guards.as_ref(), None, range, None);

                // Label every trade by the state at its SIGNAL bar, which is
                // the bar BEFORE the fill (`engine.rs` fills `pending` at the
                // next bar's open). A fill time that is not a bar time, or a
                // fill on bar 0, is unlabelled rather than guessed.
                let mut labelled: Vec<Labelled> = Vec::with_capacity(result.trades.len());
                let mut unmatched_fill = 0usize;
                let mut dropped_break = 0usize;
                for t in &result.trades {
                    match times.binary_search(&t.entry_time) {
                        Ok(idx) if idx > 0 => {
                            if drop_break > 0 {
                                let (_, minute) = fd_core::clock::new_york_local(bars[idx - 1].time);
                                let break_min = 17 * 60;
                                let delta = (i64::from(minute) - i64::from(break_min)).abs().min(
                                    1440 - (i64::from(minute) - i64::from(break_min)).abs(),
                                );
                                if delta <= i64::from(drop_break) {
                                    dropped_break += 1;
                                    continue;
                                }
                            }
                            labelled.push(Labelled {
                                r: t.r,
                                long: t.direction.is_long(),
                                signal_bar: idx - 1,
                                exit: t.exit_kind.label().to_string(),
                            });
                        }
                        _ => unmatched_fill += 1,
                    }
                }
                let whole = {
                    let mut b = Book::default();
                    for t in &labelled {
                        b.add(t.r, &t.exit);
                    }
                    b
                };
                println!(
                    "-- {wname} {from}..{to}  {mech}  (weekdays + flat 16:30-18:15)  n={} trades, {} unmatched fills, {} dropped at the CME break, cap_lots hit {} of {} entries, no-risk {} entries",
                    whole.n, unmatched_fill, dropped_break, result.sized_down_by_guard, result.trades.len(), result.skipped_no_atr
                );
                println!(
                    "   WHOLE BOOK  n {:<5} E {:>9} R  PF_r {:>7}  Lbar {:>7} R  win {:>5}%  PF_usd {:>7}  maxDD {:>10} USD / {:>6}%  exits {}",
                    whole.n,
                    fmt(whole.e(), 5),
                    fmt(whole.pf_r(), 4),
                    fmt(whole.lbar(), 4),
                    fmt(whole.win_pct(), 1),
                    fmt(result.metrics.profit_factor, 4),
                    fmt(result.metrics.max_drawdown_usd, 2),
                    fmt(result.metrics.max_drawdown_pct, 2),
                    exit_mix(&whole.exits),
                );
                if whole.n == 0 {
                    println!("   no trades — every comparison on this row is null, not zero");
                    continue;
                }

                for comp in &companions {
                    for st in states {
                        let out = report(st, comp, &labelled, draws, seed);
                        ledger.insert((guards_on, comp.symbol.clone(), st.id(), mech, wname), out);
                    }
                }
                println!();
            }
        }
    }

    /* ----------------------------------------------- the decision ledger */

    println!("######## DECISION LEDGER - F1 as registered ########");
    println!(
        "F1 needs, for ONE comparison: |dE| past all three controls' 5% band on BOTH windows, SAME SIGN, both groups >= 40 trades."
    );
    for guards_on in [true, false] {
        let arm = if guards_on { "guards ON" } else { "guards OFF" };
        println!();
        println!("-- {arm} --");
        println!(
            "{:<9} {:<6} {:<19} {:>10} {:>10} {:>8} {:>8}  {}",
            "companion", "state", "mechanism", "dE A (R)", "dE B (R)", "pA min%", "pB min%", "verdict"
        );
        let mut separates_both = 0usize;
        let mut same_sign = 0usize;
        let mut not_measurable = 0usize;
        let mut total = 0usize;
        for comp in &companions {
            for st in states {
                for mech in mechanisms {
                    total += 1;
                    let a = ledger.get(&(guards_on, comp.symbol.clone(), st.id(), mech, windows[0].0));
                    let b = ledger.get(&(guards_on, comp.symbol.clone(), st.id(), mech, windows[windows.len() - 1].0));
                    let (Some(a), Some(b)) = (a, b) else {
                        println!(
                            "{:<9} {:<6} {:<19} {:>10} {:>10} {:>8} {:>8}  no trades on one window - null",
                            comp.symbol, st.id(), mech, "null", "null", "null", "null"
                        );
                        not_measurable += 1;
                        continue;
                    };
                    let pa = a.p_time.min(a.p_label).min(a.p_block);
                    let pb = b.p_time.min(b.p_label).min(b.p_block);
                    let signs_match = a.de.is_finite() && b.de.is_finite() && (a.de > 0.0) == (b.de > 0.0);
                    if signs_match {
                        same_sign += 1;
                    }
                    let verdict = if !a.measurable() || !b.measurable() {
                        not_measurable += 1;
                        "NOT MEASURABLE (a group under 40)"
                    } else if a.separates() && b.separates() && signs_match {
                        separates_both += 1;
                        "** SEPARATES on BOTH windows, same sign **"
                    } else if signs_match {
                        "same sign, inside the noise"
                    } else {
                        "sign flips between windows"
                    };
                    println!(
                        "{:<9} {:<6} {:<19} {:>10} {:>10} {:>8} {:>8}  {verdict}",
                        comp.symbol,
                        st.id(),
                        mech,
                        fmt(a.de, 5),
                        fmt(b.de, 5),
                        fmt(pa, 1),
                        fmt(pb, 1),
                    );
                }
            }
        }
        println!(
            "{arm}: {separates_both} of {total} declared comparisons separate on BOTH windows with the SAME sign (a coin gives about {:.2}); {same_sign} of {total} merely share a sign (a coin gives {:.1}); {not_measurable} not measurable",
            total as f64 * 0.0025,
            total as f64 * 0.5,
        );
        let mut best = f64::NEG_INFINITY;
        let mut best_label = String::new();
        for ((g, sym, st, mech, w), o) in &ledger {
            if *g == guards_on && o.measurable() && o.best_e > best {
                best = o.best_e;
                best_label = format!("{sym} {st} {mech} window {w}");
            }
        }
        println!(
            "{arm}: F3 ceiling - the BEST E of any group of any comparison is {} R ({}); the gate needs >= +0.050 R",
            fmt(best, 5),
            best_label
        );
        // F6: does the SAME state read on the primary itself say the same
        // thing? A companion that only reproduces the primary's own regime is
        // not a second instrument.
        let base_sym = &companion_names[companion_names.len() - 1];
        println!();
        println!(
            "{arm}: F6 baseline - the same state computed on {base_sym} (the PRIMARY itself), beside the companion"
        );
        println!(
            "{:<9} {:<6} {:<19} {:>10} {:>10} {:>10} {:>10}  {}",
            "companion", "state", "mechanism", "cmp dE A", "base dE A", "cmp dE B", "base dE B", "does the companion add anything?"
        );
        let mut adds = 0usize;
        let mut baseline_wins = 0usize;
        for comp in &companions {
            if &comp.symbol == base_sym {
                continue;
            }
            for st in states {
                for mech in mechanisms {
                    let g = |sym: &str, w: &str| ledger.get(&(guards_on, sym.to_string(), st.id(), mech, w)).map_or(f64::NAN, |o| o.de);
                    let (ca, cb) = (g(&comp.symbol, windows[0].0), g(&comp.symbol, windows[windows.len() - 1].0));
                    let (ba, bb) = (g(base_sym, windows[0].0), g(base_sym, windows[windows.len() - 1].0));
                    let verdict = if !(ca.is_finite() && cb.is_finite() && ba.is_finite() && bb.is_finite()) {
                        "null - not measurable on one window"
                    } else if ba.abs() >= ca.abs() && bb.abs() >= cb.abs() {
                        baseline_wins += 1;
                        "NO - the primary's own state separates at least as much on BOTH windows"
                    } else if ca.abs() > ba.abs() && cb.abs() > bb.abs() {
                        adds += 1;
                        "yes - companion separates MORE on both windows"
                    } else {
                        "mixed - companion more on one window, less on the other"
                    };
                    println!(
                        "{:<9} {:<6} {:<19} {:>10} {:>10} {:>10} {:>10}  {verdict}",
                        comp.symbol,
                        st.id(),
                        mech,
                        fmt(ca, 5),
                        fmt(ba, 5),
                        fmt(cb, 5),
                        fmt(bb, 5),
                    );
                }
            }
        }
        println!(
            "{arm}: companion separates MORE than the primary's own state on both windows in {adds} comparisons; the PRIMARY's own state wins on both in {baseline_wins}"
        );
    }
    Ok(())
}

fn exit_mix(exits: &BTreeMap<String, usize>) -> String {
    exits.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(" / ")
}

fn iso(ms: i64) -> String {
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(86_400_000));
    format!("{y:04}-{m:02}-{d:02}")
}

/// What one comparison said on one window, for the decision ledger.
#[derive(Clone, Copy)]
struct Outcome {
    /// `E(in) - E(out)` in R, or NaN when a group was under 40 trades.
    de: f64,
    /// Share of control draws whose |dE| reached the observed one, per control.
    p_time: f64,
    p_label: f64,
    p_block: f64,
    n_in: usize,
    n_out: usize,
    /// `E` of the better of the two groups, in R - the F3 ceiling.
    best_e: f64,
}

impl Outcome {
    fn measurable(&self) -> bool {
        self.n_in >= 40 && self.n_out >= 40
    }
    /// Past all three controls at the 5% two-sided band.
    fn separates(&self) -> bool {
        self.measurable() && self.p_time < 5.0 && self.p_label < 5.0 && self.p_block < 5.0
    }
}

/// Split the labelled trades by one state definition, then read the split
/// against the three controls.
fn report(st: StateKind, comp: &CompanionSeries, trades: &[Labelled], draws: usize, seed: u64) -> Outcome {
    // The bars in play: every bar the labelled trades could have been signalled
    // on, which is the window the shuffle must stay inside. Taking the span of
    // the signal bars rather than the whole series keeps the control's marginal
    // the marginal of THIS window.
    let (lo, hi) = (trades.iter().map(|t| t.signal_bar).min().unwrap(), trades.iter().map(|t| t.signal_bar).max().unwrap());
    let values: Vec<f64> = (lo..=hi).map(|i| comp.value(st, i)).collect();
    let defined: Vec<usize> = values.iter().enumerate().filter(|(_, v)| v.is_finite()).map(|(i, _)| i).collect();

    let mut inside = Book::default();
    let mut outside = Book::default();
    let mut unmeasurable = 0usize;
    for t in trades {
        let v = comp.value(st, t.signal_bar);
        if !v.is_finite() {
            unmeasurable += 1;
        } else if st.in_state(v, t.long) {
            inside.add(t.r, &t.exit);
        } else {
            outside.add(t.r, &t.exit);
        }
    }
    let measurable = inside.n + outside.n;
    let observed = inside.e() - outside.e();
    let cut = if measurable == 0 { f64::NAN } else { 100.0 * outside.n as f64 / measurable as f64 };

    println!(
        "   {:<8} {:<5} IN   n {:<5} E {:>9} R  PF_r {:>7}  Lbar {:>7} R  win {:>5}%   exits {}",
        comp.symbol,
        st.id(),
        inside.n,
        fmt(inside.e(), 5),
        fmt(inside.pf_r(), 4),
        fmt(inside.lbar(), 4),
        fmt(inside.win_pct(), 1),
        exit_mix(&inside.exits)
    );
    println!(
        "   {:<8} {:<5} OUT  n {:<5} E {:>9} R  PF_r {:>7}  Lbar {:>7} R  win {:>5}%   exits {}",
        "", "", outside.n,
        fmt(outside.e(), 5),
        fmt(outside.pf_r(), 4),
        fmt(outside.lbar(), 4),
        fmt(outside.win_pct(), 1),
        exit_mix(&outside.exits)
    );

    let best_e = if inside.e().is_finite() && outside.e().is_finite() {
        inside.e().max(outside.e())
    } else {
        f64::NAN
    };
    if inside.n < 40 || outside.n < 40 {
        println!(
            "   {:<8} {:<5} dE   null — a group under 40 trades is NOT MEASURABLE, not an absence of edge (in {} / out {}); unlabelled {}",
            "", "", inside.n, outside.n, unmeasurable
        );
        return Outcome {
            de: f64::NAN,
            p_time: f64::NAN,
            p_label: f64::NAN,
            p_block: f64::NAN,
            n_in: inside.n,
            n_out: outside.n,
            best_e,
        };
    }

    // --- control 1: time shuffle, marginal preserved exactly -------------
    let mut shuffled = values.clone();
    let mut t_hits = 0usize;
    let mut t_abs_sum = 0.0;
    let mut t_cut_sum = 0.0;
    let mut rng = Rng::new(seed ^ (st.id().len() as u64) ^ comp.symbol.len() as u64);
    for _ in 0..draws {
        // Fisher-Yates over the DEFINED positions only, so a bar the
        // companion does not cover stays uncovered.
        for i in (1..defined.len()).rev() {
            let j = rng.below(i + 1);
            let (a, b) = (defined[i], defined[j]);
            shuffled.swap(a, b);
        }
        let (d, c) = split_e(st, trades, lo, &shuffled);
        if d.is_finite() {
            if d.abs() >= observed.abs() {
                t_hits += 1;
            }
            t_abs_sum += d.abs();
            t_cut_sum += c;
        }
    }

    // --- control 2: label shuffle, group sizes preserved exactly ---------
    let mut labels: Vec<bool> = Vec::with_capacity(measurable);
    let mut rs: Vec<f64> = Vec::with_capacity(measurable);
    for t in trades {
        let v = comp.value(st, t.signal_bar);
        if v.is_finite() {
            labels.push(st.in_state(v, t.long));
            rs.push(t.r);
        }
    }
    let mut l_hits = 0usize;
    let mut l_abs_sum = 0.0;
    let mut rng = Rng::new(seed.wrapping_add(7) ^ comp.symbol.len() as u64 ^ (st.id().len() as u64) << 8);
    for _ in 0..draws {
        for i in (1..labels.len()).rev() {
            let j = rng.below(i + 1);
            labels.swap(i, j);
        }
        let (mut si, mut so, mut ni, mut no) = (0.0, 0.0, 0usize, 0usize);
        for (lab, r) in labels.iter().zip(&rs) {
            if *lab {
                si += r;
                ni += 1;
            } else {
                so += r;
                no += 1;
            }
        }
        if ni > 0 && no > 0 {
            let d = si / ni as f64 - so / no as f64;
            if d.abs() >= observed.abs() {
                l_hits += 1;
            }
            l_abs_sum += d.abs();
        }
    }

    // --- control 3: day-block time shuffle (96 bars = one 15m day) -------
    const BLOCK: usize = 96;
    let blocks: Vec<usize> = (0..values.len().div_ceil(BLOCK)).collect();
    let mut order = blocks.clone();
    let mut b_hits = 0usize;
    let mut b_abs_sum = 0.0;
    let mut rng = Rng::new(seed.wrapping_add(13) ^ (comp.symbol.len() as u64) << 16 ^ st.id().len() as u64);
    for _ in 0..draws {
        for i in (1..order.len()).rev() {
            let j = rng.below(i + 1);
            order.swap(i, j);
        }
        // Indexed, not materialised: the block at position `q` of the
        // permuted series is block `order[q]` of the original, so bar `p`
        // reads `order[p / BLOCK] * BLOCK + p % BLOCK`. Building a 1.5 MB
        // copy per draw would cost more than every other control together.
        let read = |p: usize| -> f64 {
            let src = order[p / BLOCK] * BLOCK + p % BLOCK;
            if src < values.len() { values[src] } else { f64::NAN }
        };
        let (d, _) = split_e_with(st, trades, lo, &read);
        if d.is_finite() {
            if d.abs() >= observed.abs() {
                b_hits += 1;
            }
            b_abs_sum += d.abs();
        }
    }

    let frac = |hits: usize| 100.0 * hits as f64 / draws as f64;
    let share = |sum: f64| if observed.abs() > 0.0 { 100.0 * (sum / draws as f64) / observed.abs() } else { f64::NAN };
    println!(
        "   {:<8} {:<5} dE   {:>9} R  cut {:>5}% (null mean cut {:>5}%)  unlabelled {}",
        "", "", fmt(observed, 5), fmt(cut, 1), fmt(t_cut_sum / draws as f64, 1), unmeasurable
    );
    println!(
        "   {:<8} {:<5} ctrl time-shuffle |dE|>=obs in {:>5}% of {draws}; control reproduces {:>5}% of the move",
        "", "", fmt(frac(t_hits), 1), fmt(share(t_abs_sum), 1)
    );
    println!(
        "   {:<8} {:<5} ctrl label-shuffle |dE|>=obs in {:>5}% of {draws}; control reproduces {:>5}% of the move",
        "", "", fmt(frac(l_hits), 1), fmt(share(l_abs_sum), 1)
    );
    println!(
        "   {:<8} {:<5} ctrl day-block    |dE|>=obs in {:>5}% of {draws}; control reproduces {:>5}% of the move",
        "", "", fmt(frac(b_hits), 1), fmt(share(b_abs_sum), 1)
    );
    let out = Outcome {
        de: observed,
        p_time: frac(t_hits),
        p_label: frac(l_hits),
        p_block: frac(b_hits),
        n_in: inside.n,
        n_out: outside.n,
        best_e,
    };
    println!(
        "   {:<8} {:<5} => {} on this window",
        "", "",
        if out.separates() { "SEPARATES past all three controls" } else { "inside the noise of a same-cut random filter" }
    );
    out
}

/// `E(in) − E(out)` and the cut ratio, for a (possibly shuffled) value series
/// whose index 0 is primary bar `lo`.
fn split_e(st: StateKind, trades: &[Labelled], lo: usize, values: &[f64]) -> (f64, f64) {
    split_e_with(st, trades, lo, &|p: usize| values[p])
}

/// The same split, over a value LOOKUP rather than a slice, so a control that
/// permutes blocks does not have to materialise the permuted series.
fn split_e_with(st: StateKind, trades: &[Labelled], lo: usize, value: &dyn Fn(usize) -> f64) -> (f64, f64) {
    let (mut si, mut so, mut ni, mut no) = (0.0, 0.0, 0usize, 0usize);
    for t in trades {
        let v = value(t.signal_bar - lo);
        if !v.is_finite() {
            continue;
        }
        if st.in_state(v, t.long) {
            si += t.r;
            ni += 1;
        } else {
            so += t.r;
            no += 1;
        }
    }
    if ni == 0 || no == 0 {
        return (f64::NAN, f64::NAN);
    }
    (si / ni as f64 - so / no as f64, 100.0 * no as f64 / (ni + no) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(t: i64, c: f64) -> Bar {
        Bar { time: t, open: c, high: c + 0.5, low: c - 0.5, close: c, volume: None }
    }
    const M15: i64 = 900_000;

    /// F2, THE CAUSALITY TEST. Truncating the companion at bar `cut` must not
    /// change the per-bar state value at ANY bar at or before it: that is what
    /// makes a lag-0 read of a second series causal rather than a read of a
    /// forming bar.
    #[test]
    fn truncating_the_companion_never_changes_an_earlier_state_value() {
        let comp_bars: Vec<Bar> = (0..600).map(|i| bar(i * M15, 20.0 + (i as f64 / 7.0).sin() * 2.0)).collect();
        let primary: Vec<Bar> = (0..600).map(|i| bar(i * M15, 1000.0 + (i as f64 / 11.0).cos() * 5.0)).collect();
        let full = Companion::new("X", "15m", comp_bars.clone());
        let series = |c: &Companion, p: &[Bar]| -> (Vec<f64>, Vec<f64>) {
            let (change, atr, close) = aligned_change(p, c, 1, 14);
            let k: Vec<f64> =
                change.iter().zip(&atr).map(|(ch, a)| if a.is_finite() && *a > 0.0 { ch / a } else { f64::NAN }).collect();
            let vol: Vec<f64> = atr
                .iter()
                .zip(&close)
                .map(|(a, cl)| if a.is_finite() && cl.is_finite() && *cl != 0.0 { 100.0 * a / cl } else { f64::NAN })
                .collect();
            let med = trailing_median(&vol, 50);
            let ex: Vec<f64> =
                vol.iter().zip(&med).map(|(v, m)| if v.is_finite() && m.is_finite() { v - m } else { f64::NAN }).collect();
            (k, ex)
        };
        let (k_full, ex_full) = series(&full, &primary);
        let cut = 400;
        let short = Companion::new("X", "15m", comp_bars[..cut].to_vec());
        let (k_cut, ex_cut) = series(&short, &primary[..cut]);
        for i in 0..cut {
            assert!(fd_core::parity_eq(k_full[i], k_cut[i]), "k at {i} moved when later companion bars arrived");
            assert!(fd_core::parity_eq(ex_full[i], ex_cut[i]), "vol_excess at {i} moved when later companion bars arrived");
        }
        // And it is not vacuous: the series actually carries values here.
        assert!(k_full[..cut].iter().filter(|v| v.is_finite()).count() > 300);
        assert!(ex_full[..cut].iter().filter(|v| v.is_finite()).count() > 300);
    }

    /// The POSITIVE CONTROL for the test above: a state that reads the NEXT
    /// bar must make it fail. Without this, "the causality test passed" could
    /// mean the test cannot fail.
    #[test]
    fn a_state_that_reads_the_next_bar_breaks_the_causality_test() {
        let comp_bars: Vec<Bar> = (0..600).map(|i| bar(i * M15, 20.0 + (i as f64 / 7.0).sin() * 2.0)).collect();
        let primary: Vec<Bar> = (0..600).map(|i| bar(i * M15, 1000.0)).collect();
        // The look-ahead: shift the companion BACK one bar, so the value at
        // primary bar i is the companion's bar i+1 — the bar still forming.
        let peeking = |c: &Companion, p: &[Bar]| -> Vec<f64> {
            let (change, atr, _) = aligned_change(p, c, 1, 14);
            let mut k: Vec<f64> =
                change.iter().zip(&atr).map(|(ch, a)| if a.is_finite() && *a > 0.0 { ch / a } else { f64::NAN }).collect();
            k.remove(0);
            k.push(f64::NAN);
            k
        };
        let full = Companion::new("X", "15m", comp_bars.clone());
        let a = peeking(&full, &primary);
        // FIVE cut points, not one: a probe that only fails at a lucky
        // boundary would still leave the test half blind.
        for cut in [120usize, 200, 300, 400, 500] {
            let short = Companion::new("X", "15m", comp_bars[..cut].to_vec());
            let b = peeking(&short, &primary[..cut]);
            let differs = (0..cut).any(|i| !fd_core::parity_eq(a[i], b[i]));
            assert!(differs, "a next-bar read must change when the series is truncated at {cut} — otherwise the causality test is vacuous");
        }
        // And the honest read passes at the same five cut points, so the two
        // assertions are about the READING and not about the cut.
        let honest = |c: &Companion, p: &[Bar]| -> Vec<f64> {
            let (change, atr, _) = aligned_change(p, c, 1, 14);
            change.iter().zip(&atr).map(|(ch, x)| if x.is_finite() && *x > 0.0 { ch / x } else { f64::NAN }).collect()
        };
        let h = honest(&full, &primary);
        for cut in [120usize, 200, 300, 400, 500] {
            let short = Companion::new("X", "15m", comp_bars[..cut].to_vec());
            let hb = honest(&short, &primary[..cut]);
            for i in 0..cut {
                assert!(fd_core::parity_eq(h[i], hb[i]), "the honest read moved at {i} when cut at {cut}");
            }
        }
    }

    /// The trailing median reads only bars strictly before the one it is
    /// stamped on, and is NaN until its window is full.
    #[test]
    fn the_trailing_median_never_reads_its_own_bar() {
        let x: Vec<f64> = (0..10).map(f64::from).collect();
        let m = trailing_median(&x, 3);
        assert!(m[0].is_nan() && m[1].is_nan() && m[2].is_nan(), "not warm until three values have passed");
        assert_eq!(m[3], 1.0, "median of 0,1,2");
        assert_eq!(m[4], 2.0, "median of 1,2,3");
        // A spike at the end cannot reach backwards.
        let mut y = x.clone();
        y[9] = 1000.0;
        let m2 = trailing_median(&y, 3);
        for i in 0..9 {
            assert!(fd_core::parity_eq(m[i], m2[i]), "median at {i} moved when a later value changed");
        }
    }

    /// The state rules, spelled out so a reader does not have to infer them
    /// from the arithmetic.
    #[test]
    fn the_state_rules_are_what_the_registration_says() {
        assert!(StateKind::Dir.in_state(0.30, true), "companion up, trade long, |k| over 0.25");
        assert!(!StateKind::Dir.in_state(0.30, false), "companion up, trade short: disagrees");
        assert!(!StateKind::Dir.in_state(0.20, true), "under the 0.25 band is not agreement");
        assert!(StateKind::Dir.in_state(-0.30, false), "companion down, trade short: agrees");
        assert!(StateKind::Move.in_state(-0.80, true) && StateKind::Move.in_state(0.80, false), "magnitude is sideless");
        assert!(!StateKind::Move.in_state(0.74, true));
        assert!(StateKind::Vol.in_state(0.001, true) && !StateKind::Vol.in_state(-0.001, false));
    }

    /// The shuffle keeps the marginal exactly: a control that cut a different
    /// share of the bars would not be the control the registration declares.
    #[test]
    fn the_time_shuffle_preserves_the_marginal_exactly() {
        let values: Vec<f64> = (0..200).map(|i| if i % 3 == 0 { f64::NAN } else { f64::from(i) * 0.01 }).collect();
        let defined: Vec<usize> = values.iter().enumerate().filter(|(_, v)| v.is_finite()).map(|(i, _)| i).collect();
        let mut shuffled = values.clone();
        let mut rng = Rng::new(1);
        for i in (1..defined.len()).rev() {
            let j = rng.below(i + 1);
            let (a, b) = (defined[i], defined[j]);
            shuffled.swap(a, b);
        }
        // NaN positions are untouched, and the multiset of finite values is
        // the same one.
        for i in 0..values.len() {
            assert_eq!(values[i].is_nan(), shuffled[i].is_nan(), "coverage moved at {i}");
        }
        let mut before: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        let mut after: Vec<f64> = shuffled.iter().copied().filter(|v| v.is_finite()).collect();
        before.sort_by(f64::total_cmp);
        after.sort_by(f64::total_cmp);
        assert_eq!(before, after);
        assert!(values != shuffled, "a shuffle that changed nothing is not a control");
    }

    /// `E = Lbar x (PF_r - 1)`, the identity the record checked on 200/200,
    /// 452/452 and 72/72 cells — held here on the sub-books this tool prints,
    /// which is what makes the two columns readable together.
    #[test]
    fn the_gate_identity_holds_on_a_sub_book() {
        let mut b = Book::default();
        for r in [1.8, -1.0, -1.0, 2.4, -1.0, 0.3, -1.0, 1.1] {
            b.add(r, "STOP");
        }
        let predicted = b.lbar() * (b.pf_r() - 1.0);
        assert!((predicted - b.e()).abs() < 1e-12, "E {} vs Lbar x (PF_r - 1) {}", b.e(), predicted);
    }
}
