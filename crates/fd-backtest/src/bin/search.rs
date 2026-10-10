//! Search the hypothesis space on stored data.
//!
//! The whole point of the port, finally pointed at something. Reads bars (and,
//! where there is one, a tape) from the Parquet store, then reports three
//! different things that are easy to confuse:
//!
//! * **compare** — every method at its default parameters. A leaderboard, not
//!   a result.
//! * **sweep** — one method across its grid. Sensitivity, not a winner. The
//!   best cell of a sweep is in-sample and means nothing on its own; what the
//!   distribution says about the *median* cell is the honest reading.
//! * **walk-forward** — parameters chosen on a training window and measured on
//!   the next one, rolled forward. The only number worth quoting.
//!
//! The promising gate is applied to the walk-forward result and reported as it
//! falls. A failing gate is the answer, not a prompt to widen the grid.
//!
//! ```text
//! cargo run --release -p fd-backtest --bin search -- --market=btc --mode=wf
//! ```
//!
//! `--guards` applies `[trading.guards]` to every run of every mode —
//! hypotheses, nulls, sweeps, cost curves, the leaderboard — and says so in
//! the header. Without it every number is unguarded, as every receipt before
//! 2026-09-14 was.

use std::collections::HashMap;
use std::path::PathBuf;

use fd_backtest::engine::{Range, TradingRules, run_backtest_guarded};
use fd_strategy::registry::Strategy as _;
use fd_backtest::sweep::{SelectBy, compare_strategies_guarded, sweep_strategy_guarded, verdict, walk_forward_guarded};
use fd_backtest::hypotheses::{
    NullSides, batch as hypothesis_batch, batch_from_file, run_hypothesis_sides,
};
use fd_backtest::timeline::{TimelineOptions, build_timeline};
use fd_backtest::{Guards, OptionsTimeline, PromisingGate};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::{TapeStore, read_bars};
use fd_strategy::registry::Registry;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// The flags every mode reads: the market, the window, the cost model and the
/// run header. Passing one of these changes the numbers whatever `--mode=` is.
const ALWAYS: &[&str] =
    &["market", "mode", "data", "config", "interval", "from", "to", "spread", "trail", "companion", "guards"];

/// Every other flag, with the `--mode=` values that actually read it.
///
/// **Five flags have been found being ignored in silence** — `--exit-mix` and
/// the `cost % of R` line in `rescore`, `--samples=` and
/// `--direction-samples=` under `hypotheses`, `--rebate-share=` outside
/// `rescore` — so a receipt that passed one of them was a receipt whose header
/// claimed a setting the run never applied. This table is what
/// [`flag_audit`] reads to say so in the header, and what the test below it
/// holds against the binary's own source so that the table cannot go stale
/// without the test suite failing. `docs/decisions/2026-10-07-instrument-repair.md`.
///
/// `--mode=all` runs compare, sweep and walk-forward and therefore reads NONE
/// of these; that is a fact about `all`, not an omission here.
const BY_MODE: &[(&str, &[&str])] = &[
    // The matched null's side rule reaches `run_hypotheses` and nothing else.
    // `rescore` keeps the coin-flip null it published on purpose
    // (`hypotheses.rs`, `rescore_hypothesis`) — so on that mode this flag is
    // read by the HEADER and by no run, which is the sixth case of the class
    // and is now stated instead of implied.
    ("null-sides", &["hypotheses"]),
    ("seeds", &["hypotheses", "null", "rescore"]),
    ("samples", &["null-dir"]),
    ("direction-samples", &["rescore"]),
    ("strategy", &["null-dir"]),
    ("params", &["null-dir"]),
    ("filters", &["null-dir"]),
    ("batch", &["hypotheses"]),
    ("batch-file", &["hypotheses", "rescore"]),
    ("rebate-share", &["rescore"]),
    ("fixed", &["hypotheses"]),
    ("exit-mix", &["hypotheses", "rescore"]),
    ("null-registered-stop", &["hypotheses"]),
    // The GC->spot basis correction. It shifts the OPTIONS TIMELINE, so it is
    // read by exactly the modes that load one. `rescore` is NOT one of them:
    // every `run_backtest_guarded` in `rescore_hypothesis` still passes `None`
    // for the timeline, so a basis offset there would be a setting applied to
    // nothing — which is the class of defect this table exists to denounce.
    ("basis-offset", &["all", "compare", "sweep", "wf", "hypotheses", "null", "null-dir", "volume", "costs"]),
    // The ROLLING basis, same modes for the same reason. It is the answer to
    // the constant above having decided a verdict by itself: see
    // `fd_backtest::basis` and docs/decisions/2026-10-10-options-rolling-basis.md.
    ("basis-roll", &["all", "compare", "sweep", "wf", "hypotheses", "null", "null-dir", "volume", "costs"]),
    ("basis-roll-min", &["all", "compare", "sweep", "wf", "hypotheses", "null", "null-dir", "volume", "costs"]),
    ("basis-ref", &["all", "compare", "sweep", "wf", "hypotheses", "null", "null-dir", "volume", "costs"]),
];

/// What this mode will IGNORE out of what was passed, as lines for the receipt
/// header.
///
/// A receipt must denounce itself: the reader of
/// `2026-09-23-rebate-rescore.md` cannot tell whether `--exit-mix` was passed
/// and dropped or never passed at all, and that ambiguity is what made a row
/// whose own rule fired zero times unfalsifiable from its receipt.
fn flag_audit(mode: &str, args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in args {
        let Some(body) = raw.strip_prefix("--") else {
            out.push(format!("** `{raw}` is not a flag — this binary reads `--name=value` and `--name` only, and ignored it **"));
            continue;
        };
        let name = body.split('=').next().unwrap_or_default();
        if ALWAYS.contains(&name) {
            continue;
        }
        match BY_MODE.iter().find(|(flag, _)| *flag == name) {
            None => out.push(format!("** --{name} is not a flag this binary has; it changed nothing in this run **")),
            Some((_, modes)) if modes.contains(&mode) => {}
            Some((_, modes)) => out.push(format!(
                "** --{name} IS NOT READ BY --mode={mode} — only by --mode={} — so nothing below was changed by it **",
                modes.join(" / --mode=")
            )),
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "btc");
    let mode = arg("mode", "all");
    let data = PathBuf::from(arg("data", "data"));
    let config = Config::load(arg("config", "config"))?;
    let spec = config.market(&market)?;

    let interval = arg("interval", &config.backtest.timeframe);
    let bars_path = data.join("bars").join(format!("{}-{interval}.parquet", spec.bar_symbol));
    // WHICH STORE THIS RUN READ, in the receipt, before anything is measured.
    // Every other line here named a market or a symbol and left the directory
    // implicit, so a receipt could not be checked against the store it was
    // meant to come from - which is exactly what the sealed-store programme
    // asks a receipt to prove (`docs/hypotheses/2026-09-23-designed-methods.md`).
    //
    // Two agents added this line independently on the same day, one naming the
    // market and one naming the root. Both are kept: the root is what the audit
    // reads, the market is what a human looks for first, and dropping either to
    // settle a merge would lose something one of them was added for.
    println!("market:   {market} {interval}");
    println!("data:     {} (bars from {})", data.display(), bars_path.display());
    let mut bars = read_bars(&bars_path)?;
    // `--from=YYYY-MM-DD --to=YYYY-MM-DD` (UTC, `to` exclusive) cut the series
    // before anything runs: an out-of-sample feed that overlaps the in-sample
    // one is only out of sample on the dates the in-sample feed never saw.
    let bound = |name: &str| -> Option<i64> {
        let text = arg(name, "");
        if text.is_empty() {
            return None;
        }
        let mut parts = text.split('-').map(|p| p.parse::<i64>());
        let (y, m, d) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
        Some(fd_core::clock::days_from_civil(y, m as u32, d as u32) * 86_400_000)
    };
    let (from, to) = (bound("from"), bound("to"));
    if from.is_some() || to.is_some() {
        let before = bars.len();
        bars.retain(|b| from.is_none_or(|f| b.time >= f) && to.is_none_or(|t| b.time < t));
        println!(
            "bounds:   --from={} --to={} kept {} of {before} bars",
            arg("from", "…"),
            arg("to", "…"),
            bars.len()
        );
    }
    if bars.is_empty() {
        println!("no bars at {} (after bounds) — run the backfill first", bars_path.display());
        return Ok(());
    }

    let mut rules = fd_backtest::engine::trading_rules_for(&config, &market)?;
    // `--trail=<distance_r>,<activate_r>` overrides `[trading.trail]` for this
    // run only, so a sweep over trail settings does not need a config edit per
    // cell — and cannot leave one behind. `--trail=off` forces it off whatever
    // the config says, which is what the control arm of such a sweep needs.
    if let Some(spec) = std::env::args().find_map(|a| a.strip_prefix("--trail=").map(str::to_string)) {
        if spec == "off" {
            rules.trail.enabled = false;
        } else {
            let mut parts = spec.split(',');
            let d: f64 = parts.next().unwrap_or("").trim().parse()
                .map_err(|_| format!("--trail wants <distance_r>,<activate_r> or `off`, got `{spec}`"))?;
            let a: f64 = parts.next().unwrap_or("").trim().parse()
                .map_err(|_| format!("--trail wants <distance_r>,<activate_r> or `off`, got `{spec}`"))?;
            rules.trail.enabled = true;
            rules.trail.distance_r = d;
            rules.trail.activate_r = a;
        }
    }
    // `--spread=<price units>` reprices the round trip for this run only. The
    // configured 0.28 for gold was a SINGLE read of the terminal; the logger
    // has since sampled thousands and the p50 is 0.220 with a maximum of 0.260,
    // so every receipt in docs/ is charged a cost above anything ever observed.
    // Repricing is a recorded amendment, never a config edit, so this flag
    // exists to measure what the amendment would be worth before anyone makes
    // one.
    if let Some(v) = std::env::args().find_map(|a| a.strip_prefix("--spread=").map(str::to_string)) {
        let s: f64 = v.trim().parse().map_err(|_| format!("--spread wants a number, got `{v}`"))?;
        rules.spread = s;
    }
    println!("spread:   {} per round trip", rules.spread);
    println!(
        "trail:    {}",
        if rules.trail.enabled {
            format!("ON distance {}R activate {}R", rules.trail.distance_r, rules.trail.activate_r)
        } else {
            "off".to_string()
        }
    );
    // WHAT HORIZON THIS RUN MEASURED, in the receipt, beside the spread.
    // `max_hold_ms` force-closes an `Exits::Engine` position whatever its
    // signal intended, and until 2026-09-24 there was no way to state it per
    // market, so no receipt ever said which horizon it was read under — every
    // one of them was four hours. It is stated now, together with where the
    // number came from, because a receipt at 168 h and a receipt at 4 h are not
    // comparable and must not look alike.
    println!("{}", max_hold_line(&rules, spec.trading.max_hold_ms));
    let rules = rules;
    let gate = PromisingGate {
        min_trades: config.backtest.promising.min_trades,
        min_profit_factor: config.backtest.promising.min_profit_factor,
        min_expectancy_r: config.backtest.promising.min_expectancy_r,
    };
    let select_by =
        if config.backtest.select_by == "profitFactor" { SelectBy::ProfitFactor } else { SelectBy::Expectancy };

    let timeline = load_timeline(&data, &market, &config, &bars);
    // `--basis-offset=<usd>`: the GC->spot correction, applied to the LEVELS
    // and never to a bar.
    //
    // The gold option tape is COMEX GC; the only tradable gold bars here are
    // Vantage spot, which trades tens of dollars below it. Measured twice on
    // 6,718 overlapping minutes: mean +43.70, sd 1.91, p10 41.26, p90 45.78,
    // quartile means drifting monotonically 45.66 -> 41.35. A CONSTANT offset
    // is good enough to COUNT entries and is NOT good enough to read a gate —
    // at `entryAtr = 0.35` the entry tolerance is smaller than that residual
    // sd — so the honest use of this flag is three runs at p10 / mean / p90
    // with all three reported, and "not decided" if they disagree.
    //
    // Levels are shifted DOWN rather than bars UP because the bars are the
    // account: shifting them would move every notional, every margin check
    // and every price a receipt quotes, while the distances the rules trigger
    // on are identical either way.
    let basis = basis_offset()?;
    let roll = basis_roll()?;
    if basis.is_some() && roll.is_some() {
        return Err("--basis-offset= and --basis-roll= are two different corrections; pass one. The constant is the control, the rolling one is the measurement, and a run claiming both would be neither.".into());
    }
    let timeline = match roll {
        // ROLLING: the estimate is re-taken at every frame from observations
        // knowable strictly before it, and a frame with too little past has
        // its LEVELS REFUSED rather than shifted by a fabricated number. See
        // `fd_backtest::basis` for the causality argument and its test.
        Some(cfg) => {
            let (reference, ref_line) = load_basis_reference(&data);
            println!("{ref_line}");
            let (shifted, report) = match timeline {
                Some(t) => {
                    let (t, r) = fd_backtest::basis::apply(t, &reference, &bars, &cfg);
                    (Some(t), Some(r))
                }
                None => (None, None),
            };
            println!("{}", roll_line(&cfg, report.as_ref()));
            shifted
        }
        None => {
            let timeline = shift_basis(timeline, basis);
            println!("{}", basis_line(basis, timeline.is_some()));
            timeline
        }
    };
    describe(&bars, timeline.as_ref());
    let prints = spec
        .tape_id()
        .and_then(|tape| fd_store::TapeStore::open(&data, tape).and_then(|s| s.all()).ok())
        .map_or(0, |t| t.len());
    describe_tape(timeline.as_ref(), prints);
    // The calendar behind every `news:` filter, installed once for the
    // process. Printed here and again beside the swap/spread line of a
    // hypotheses receipt so a record can quote which calendar it ran on.
    println!("{}", load_news(&data));
    println!("{}", news_scope_line(&rules));
    // `--companion=<SYMBOL>`: the SECOND instrument, for the one method that
    // reads two. Installed once for the process, at the same interval and out
    // of the same store as the primary, and printed here so a receipt says
    // which series it was — a companion-reading method with none installed
    // takes no trades, and the two cases must not look alike.
    println!("{}", load_companion(&data, &interval));
    // `--guards`: the configured risk guards on every run of every mode.
    // Printed right under the calendar so a receipt's header says whether
    // its numbers were bounded, and by what.
    let guards = std::env::args().any(|a| a == "--guards").then(|| Guards::for_market(&config, &market)).transpose()?;
    let guards = guards.as_ref();
    println!("{}", guards_line(guards));
    // `--null-sides=coin|ratio|exposure`: which sides the MATCHED NULL takes.
    // `coin` is the default and is the null every percentile in
    // `docs/decisions/` was read against; `ratio` gives the control the
    // method's own measured long share; `exposure` also matches the hold
    // control's trade count, so the control's time in the market follows the
    // method's and not the gate's. Printed in the header of every run, not
    // only a hypotheses one, because a receipt that does not say which null it
    // used cannot be compared with one that does.
    // `docs/hypotheses/2026-10-02-drift-null.md`.
    let null_sides =
        NullSides::parse(&arg("null-sides", "coin")).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    println!("null sides: {} - {}", null_sides.as_str(), null_sides.describe());
    // WHICH OF THE FLAGS PASSED THIS MODE ACTUALLY READS, in the header,
    // before a single number. A flag a mode ignores has been found five times
    // and was silent every time; from here a receipt says it itself.
    let passed: Vec<String> = std::env::args().skip(1).collect();
    let ignored = flag_audit(&mode, &passed);
    if ignored.is_empty() {
        println!("flags:    {} passed, every one of them read by --mode={mode}", passed.len());
    } else {
        for line in &ignored {
            println!("flags:    {line}");
        }
    }
    println!();

    let registry = Registry::with_builtins();
    if mode == "all" || mode == "compare" {
        run_compare(&registry, &bars, &rules, timeline.as_ref(), &gate, guards);
    }
    if mode == "all" || mode == "sweep" {
        run_sweeps(&registry, &bars, &rules, timeline.as_ref(), config.backtest.min_trades_per_cell, guards);
    }
    if mode == "null-dir" {
        run_direction_null(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            arg("strategy", "maxpain-magnet"),
            arg("samples", "2000").parse().unwrap_or(2000),
            guards,
        );
    }
    if mode == "volume" {
        run_volume(&registry, &bars, &rules, timeline.as_ref(), guards);
    }
    if mode == "hypotheses" {
        run_hypotheses(
            &registry,
            &bars,
            &rules,
            // THE TIMELINE, which this mode never had. See
            // `hypotheses::run_hypothesis_fixed_options`: every backtest call
            // in that module passed `None`, with no `needs_options()` check to
            // say so, so an options-reading method in a pre-registered batch
            // took zero trades in silence.
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            &arg("batch", "gold-intraday"),
            arg("batch-file", "").as_str(),
            std::env::args().any(|a| a == "--fixed"),
            guards,
            // `--null-registered-stop` also reads every row against the
            // control the record used before 2026-09-24 - `RandomEntry`'s own
            // 1.5 ATR whatever the method stopped at - and prints the two
            // percentiles beside each other. It is how a corrected receipt
            // carries its original
            // (`docs/hypotheses/2026-09-24-cost-matched-null.md`), not a way
            // to measure anything new: it doubles the null runs and the
            // second column is a reproduction, not a reading.
            std::env::args().any(|a| a == "--null-registered-stop"),
            // `--exit-mix`: one extra line per row naming HOW each position
            // left - STOP, TARGET, TIMEOUT, a guard label - and the mean hold.
            // Off by default so the receipt format every earlier run printed is
            // byte-for-byte unchanged; on, it is the only direct evidence of
            // whether `max_hold_ms` was binding, which is a count no receipt
            // before 2026-09-24 carried. It is what measured that the cap closed
            // 87% of `struct-80`'s holds at four hours against 3-4% of the ATR
            // arm's, which is the finding of docs/research/runs/2026-09-24-repair-c.
            std::env::args().any(|a| a == "--exit-mix"),
            null_sides,
        );
    }
    if mode == "null" {
        run_null_control(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            guards,
        );
    }
    if mode == "rescore" {
        // The rate the desk actually has, from the same file the live side
        // reads. `--rebate-share=` overrides it for a sensitivity run and
        // says so in the header; it is never a config edit.
        let share = arg("rebate-share", "");
        let rebate = if share.is_empty() {
            fd_backtest::Rebate::from_config_dir(std::path::Path::new(&arg("config", "config")))
        } else {
            share.trim().parse::<f64>().ok().and_then(fd_backtest::Rebate::new)
        };
        run_rescore(
            &registry,
            &bars,
            &rules,
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            arg("seeds", "200").parse().unwrap_or(200),
            arg("direction-samples", "1000").parse().unwrap_or(1000),
            arg("batch-file", "").as_str(),
            rebate,
            guards,
            // `--exit-mix` HERE TOO. Until 2026-10-07 this mode took no such
            // parameter, so every receipt of
            // `docs/decisions/2026-09-23-rebate-rescore.md` and of `agent/n6`
            // was printed without an exit mix whether or not the flag was
            // passed — and a row whose own rule fires zero times
            // (`tsmom/120d`: PF 2.236, `SURVIVES`, exits all `NEWS_FLAT` /
            // `WEEKEND_FLAT`) cannot be ruled out of such a receipt by anyone.
            std::env::args().any(|a| a == "--exit-mix"),
        );
    }
    if mode == "costs" {
        run_cost_sensitivity(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            guards,
        );
    }
    if mode == "all" || mode == "wf" {
        run_walk_forward(
            &registry,
            &bars,
            &rules,
            timeline.as_ref(),
            config.backtest.walk_forward_folds,
            select_by,
            config.backtest.min_trades_per_cell,
            &gate,
            guards,
        );
    }
    Ok(())
}

/// The header line that says which calendar currencies this market's
/// `news:` filters and news guard read — `[markets.<id>.trading]
/// news_currencies`, empty meaning every currency.
fn news_scope_line(rules: &TradingRules) -> String {
    if rules.news_currencies.is_empty() {
        "news scope: every currency (news_currencies is empty)".to_string()
    } else {
        format!("news scope: {} (news_currencies)", rules.news_currencies.join("|"))
    }
}

/// The header line that says how long a position could be held, and where that
/// number came from.
///
/// `0` is not four hours and is not absent: `engine::check_exit` treats a
/// non-positive `max_hold_ms` as no timeout at all, so it is spelled out.
fn max_hold_line(rules: &TradingRules, override_ms: Option<i64>) -> String {
    let source = match override_ms {
        Some(ms) => format!("[markets.<id>.trading] max_hold_ms = {ms}"),
        None => "[trading] max_hold_ms, no per-market override".to_string(),
    };
    if rules.max_hold_ms <= 0 {
        return format!("max hold: none — positions run to a stop, a target or the end of data ({source})");
    }
    let hours = rules.max_hold_ms as f64 / 3_600_000.0;
    let shown = if (hours.fract()).abs() < 1e-9 { format!("{hours:.0}") } else { format!("{hours:.3}") };
    format!("max hold: {shown} h ({} ms) from {source}", rules.max_hold_ms)
}

/// The header line that says whether the run was bounded, and by what.
fn guards_line(guards: Option<&Guards>) -> String {
    match guards {
        Some(g) => format!("guards: on — {}", g.describe()),
        None => "guards: off (every number unguarded)".to_string(),
    }
}

/// Build the options timeline from whatever tape the store holds.
///
/// Returns `None` when there is no tape, which is not an error: the technical
/// strategies do not need one, and saying so beats inventing empty frames that
/// would make every options guard read as "no levels" rather than "no data".
fn load_timeline(
    data: &std::path::Path,
    market: &str,
    config: &Config,
    bars: &[Bar],
) -> Option<OptionsTimeline> {
    let tape = config.market(market).ok()?.tape_id()?.to_string();
    let store = TapeStore::open(data, &tape).ok()?;
    let trades = store.all().ok()?;
    if trades.is_empty() {
        return None;
    }
    let settings = fd_engine::engine::EngineSettings::from_config(config, market).ok()?;
    let timeline = build_timeline(
        &trades,
        &settings,
        &HashMap::new(),
        &TimelineOptions {
            step_ms: config.backtest.options_step_ms,
            big_trade_window_ms: config.ai.big_trade_window_ms,
        },
    );
    let _ = bars;
    (!timeline.is_empty()).then_some(timeline)
}

/// `--basis-offset=<usd>`, parsed. `None` means the flag was not passed, which
/// is NOT the same as `0.0`: zero is a declared decision to run the levels on
/// the tape's own price axis, and the header says which of the two happened.
fn basis_offset() -> Result<Option<f64>, String> {
    match std::env::args().find_map(|a| a.strip_prefix("--basis-offset=").map(str::to_string)) {
        None => Ok(None),
        Some(v) => {
            let n: f64 =
                v.trim().parse().map_err(|_| format!("--basis-offset wants a number in price units, got `{v}`"))?;
            if !n.is_finite() {
                return Err(format!("--basis-offset wants a finite number, got `{v}`"));
            }
            Ok(Some(n))
        }
    }
}

/// `--basis-ref=<SYMBOL-INTERVAL>`: the GC-axis bar series the rolling basis is
/// estimated from, read from `<data>/bars/<name>.parquet`. Default `GC-1m`.
///
/// It is a BAR SERIES, deliberately, and not the tape's own `Frame::spot`.
/// `Frame::spot` is the `underlying_price` of the last print, whichever of the
/// store's **26 expiry symbols** that print belonged to; those symbols are
/// options on four different GC contract months and their mean underlying runs
/// 4,146.00 to 4,385.32, so within one minute the dispersion is mean +10.66
/// USD, p90 +35.10, max +58.10, non-zero in 81.1% of minutes. Using it as the
/// axis gave a "basis" of sd 16.40 over a 115 USD range — the contract-month
/// dispersion, not a basis. `GC-1m` is one series (18,707 of 18,709 bars have
/// `open == high == low == close`), and against `XAUUSD-15m` it reads mean
/// +42.16 sd 2.31.
///
/// A named series that cannot be read is NOT an error and NOT a silent zero:
/// the loader says so, and with no observations every frame is refused, which
/// the basis line then counts. The failure mode this avoids is a run that reads
/// GC levels as spot levels 42 dollars out and prints like a measurement.
fn load_basis_reference(data: &std::path::Path) -> (Vec<Bar>, String) {
    let name = arg("basis-ref", "GC-1m");
    let path = data.join("bars").join(format!("{name}.parquet"));
    match read_bars(&path) {
        Err(e) => (
            Vec::new(),
            format!(
                "basis ref: ** could not read {} ({e}) — there is no GC axis, so EVERY frame will be refused; that is `null`, not an offset of 0.00 **",
                path.display()
            ),
        ),
        Ok(bars) if bars.is_empty() => (
            Vec::new(),
            format!("basis ref: ** {} holds 0 bars — every frame will be refused **", path.display()),
        ),
        Ok(bars) => {
            let flat = bars.iter().filter(|b| b.open == b.high && b.high == b.low && b.low == b.close).count();
            let line = format!(
                "basis ref: {name} {} bars from {} to {} ({}) — {flat} of {} are rangeless (o==h==l==c), so the close is a reference price and not a traded range; NOT Frame::spot, which is 26 expiry symbols on four GC contract months interleaved (same-minute dispersion p90 +35.10 USD)",
                bars.len(),
                iso(bars[0].time),
                iso(bars[bars.len() - 1].time),
                path.display(),
                bars.len()
            );
            (bars, line)
        }
    }
}

/// `--basis-roll=<minutes>` and `--basis-roll-min=<n>`, parsed together.
///
/// `None` means the flag was not passed, which is NOT the same as a window of
/// zero: a window of zero is refused outright, because a rolling estimate with
/// no window is a constant with no number.
///
/// `--basis-roll-min=` defaults to **5**, the value the registration fixed and
/// holds constant across the three declared windows so that the only thing
/// changing between those cells is the window length. Passing it is a declared
/// amendment, and the receipt prints whichever was used.
fn basis_roll() -> Result<Option<fd_backtest::RollingBasis>, String> {
    let Some(v) = std::env::args().find_map(|a| a.strip_prefix("--basis-roll=").map(str::to_string)) else {
        return Ok(None);
    };
    let minutes: i64 =
        v.trim().parse().map_err(|_| format!("--basis-roll wants a trailing window in MINUTES, got `{v}`"))?;
    if minutes <= 0 {
        return Err(format!("--basis-roll wants a window of at least one minute, got `{v}`"));
    }
    let min_obs: usize = match std::env::args().find_map(|a| a.strip_prefix("--basis-roll-min=").map(str::to_string)) {
        None => 5,
        Some(m) => {
            let n: usize =
                m.trim().parse().map_err(|_| format!("--basis-roll-min wants a count of observations, got `{m}`"))?;
            if n == 0 {
                return Err("--basis-roll-min=0 would make a median of nothing an estimate; `null` is not `0`".into());
            }
            n
        }
    };
    Ok(Some(fd_backtest::RollingBasis { window_ms: minutes * 60_000, min_obs }))
}

/// The receipt line for the ROLLING basis.
///
/// It prints the window and the statistic, the counts of shifted and REFUSED
/// frames separately (a refusal is not a shift of zero), how many raw
/// observations existed at all, and the distribution of both the raw basis and
/// the applied estimates — including the four quartile means in time order,
/// because the monotone drift in this series is the reason a constant was not
/// enough and a reader has to be able to see whether the rolling estimate
/// tracked it.
fn roll_line(cfg: &fd_backtest::RollingBasis, report: Option<&fd_backtest::basis::Report>) -> String {
    let head = format!(
        "basis:    ROLLING median over a trailing {} min window, >= {} observations, bound (t-W, t) OPEN so a bar never contributes to the basis it is traded on; levels moved, bars untouched",
        cfg.window_ms / 60_000,
        cfg.min_obs
    );
    let Some(r) = report else {
        return format!(
            "{head}\nbasis:    ** --basis-roll CHANGED NOTHING: there is no options timeline on this run **"
        );
    };
    let spread = |label: &str, s: Option<&fd_backtest::basis::Spread>| match s {
        None => format!("basis:    {label}: null — no value was produced (not 0.00)"),
        Some(s) => format!(
            "basis:    {label}: n {} mean {:+.2} sd {:.2} min {:+.2} p10 {:+.2} p50 {:+.2} p90 {:+.2} max {:+.2} | quartile means in TIME order {:+.2} -> {:+.2} -> {:+.2} -> {:+.2}",
            s.n, s.mean, s.sd, s.min, s.p10, s.p50, s.p90, s.max,
            s.quartile_means[0], s.quartile_means[1], s.quartile_means[2], s.quartile_means[3]
        ),
    };
    format!(
        "{head}\nbasis:    BARS (the engine's own denominator): {} of the bar series carry a frame, and {} of those still carry LEVELS after the basis — {} bars REFUSED, i.e. the rule could not place a level there and took nothing (that is `null`, not an offset of 0.00)\nbasis:    frames {} = shifted {} + refused {}, of which {} are DUPLICATE timestamps (build_timeline stamps every step with the last print's time, so a tape gap repeats one instant) — read the BAR line above, not these\nbasis:    raw observations {}\n{}\n{}\n{}",
        r.bars_covered,
        r.bars_with_levels,
        r.bars_covered - r.bars_with_levels,
        r.frames,
        r.shifted,
        r.refused,
        r.duplicate_frames,
        r.observations,
        spread("raw GC-spot basis, every bar that had a frame", r.raw.as_ref()),
        spread("the basis AS EACH TRADABLE BAR SAW IT (BAR-weighted, one value per tradable bar: THIS is the series a constant offset must be compared against, because a constant is also one number per bar)", r.at_bars.as_ref()),
        spread("APPLIED per frame (frame-weighted, so DISTORTED by the duplicate timestamps above - do not read as coverage)", r.applied.as_ref()),
    )
}

/// Move every price in the timeline DOWN by `offset`, onto the bars' axis.
///
/// Everything price-valued in a frame comes off the option tape's strike grid
/// and is therefore on the tape's axis: the cluster bounds and centre, and
/// `max_pain` / `poc` / `w_sup` / `w_res` / `call_be` / `put_be` / `spot` in
/// every expiration context. `score`, `dte`, the flow ratios and the velocity
/// are not prices and are left alone — which is also the check on this
/// function: `flow-momentum` reads only those, so its trade set and its
/// metrics must be identical at every offset.
fn shift_basis(timeline: Option<OptionsTimeline>, offset: Option<f64>) -> Option<OptionsTimeline> {
    // No flag, or an offset of zero, is the identity — and must return the
    // timeline it was handed, not drop it. Written as two `?`s first, which
    // silently turned every un-offset run into `timeline: none` over 53,396
    // prints: exactly the failure this whole job exists to undo.
    let timeline = timeline?;
    let Some(offset) = offset else { return Some(timeline) };
    if offset == 0.0 {
        return Some(timeline);
    }
    // The field list lives in ONE place, `basis::shift_frame_down`, so the
    // constant control and the rolling measurement shift exactly the same
    // things and the comparison between them is a comparison of the ESTIMATE
    // rather than of two field lists that drifted apart.
    let frames = timeline
        .frames()
        .iter()
        .map(|f| {
            let mut f = f.clone();
            fd_backtest::basis::shift_frame_down(&mut f, offset);
            f
        })
        .collect();
    Some(OptionsTimeline::new(frames))
}

/// The receipt line for the basis correction.
///
/// It says the offset, that it was applied to the levels and not to the bars,
/// and — when there is no timeline — that the flag changed nothing, because a
/// header that prints a correction over `timeline: none` is exactly the kind
/// of claim this binary's flag audit exists to stop.
fn basis_line(offset: Option<f64>, has_timeline: bool) -> String {
    match (offset, has_timeline) {
        (None, _) => "basis:    none — option levels are read on the tape's own price axis (not 0.00: the flag was not passed)".to_string(),
        (Some(v), true) => format!(
            "basis:    option levels shifted {v:+.2} price units onto the bars' axis (levels moved, bars untouched); GC-XAUUSD measured mean +43.70 sd 1.91 p10 41.26 p90 45.78, drifting 45.66 -> 41.35, so ONE offset is not a measurement — read p10/mean/p90 together"
        ),
        (Some(v), false) => format!(
            "basis:    ** --basis-offset={v:+.2} CHANGED NOTHING: there is no options timeline on this run **"
        ),
    }
}

/// The calendar path `load_news` actually read, for the receipt lines printed
/// deeper in, which do not have `--data` in scope.
static NEWS_SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The `news:` receipt line, naming the file this process read.
fn news_line() -> String {
    fd_strategy::news::summary(NEWS_SOURCE.get().map_or("data/news/events.parquet", String::as_str))
}

/// Install `<data>/news/events.parquet` for the `news:` filters, if it is
/// there, and say what happened in one line. A missing file is not an error
/// — the filters are then no-ops, and the line says so; an unreadable one is
/// reported, not fatal, for the same reason.
///
/// The line names the path actually read. It used to name a `const` reading
/// `data/news/events.parquet` whatever `--data` said, so a receipt from
/// `--data=data-sealed` claimed a calendar out of `data/` — a claim about the
/// store that was not true, in the one line a reader would check.
fn load_news(data: &std::path::Path) -> String {
    let path = data.join("news").join("events.parquet");
    if path.is_file() {
        match fd_store::read_news(&path).map_err(|e| e.to_string()).and_then(fd_strategy::news::install) {
            Ok(_) => {}
            Err(e) => println!("news: could not load {}: {e}", path.display()),
        }
    }
    let _ = NEWS_SOURCE.set(path.display().to_string());
    news_line()
}

/// Install `--companion=<SYMBOL>` from `<data>/bars/<SYMBOL>-<interval>.parquet`
/// and say what happened in one line.
///
/// No flag is not an error — every `cmp` series is then NaN and the line says
/// so. A named symbol that cannot be read IS reported, and loudly: a method
/// that reads two series and silently gets one would produce a run of zero
/// trades that looks like a run that found no signals.
fn load_companion(data: &std::path::Path, interval: &str) -> String {
    let symbol = arg("companion", "");
    if symbol.is_empty() {
        return fd_indicators::companion::summary();
    }
    let path = data.join("bars").join(format!("{symbol}-{interval}.parquet"));
    match read_bars(&path) {
        Err(e) => format!("companion: could not read {}: {e} — every cmp series is NaN", path.display()),
        Ok(bars) => {
            let companion = fd_indicators::companion::Companion::new(&symbol, interval, bars);
            match fd_indicators::companion::install(companion) {
                Ok(_) => format!("{} from {}", fd_indicators::companion::summary(), path.display()),
                Err(e) => format!("companion: {e}"),
            }
        }
    }
}

fn describe(bars: &[Bar], timeline: Option<&OptionsTimeline>) {
    println!("bars:     {} from {} to {}", bars.len(), iso(bars[0].time), iso(bars[bars.len() - 1].time));
    match timeline {
        Some(t) if !t.is_empty() => {
            let frames = t.frames();
            println!(
                "timeline: {} frames from {} to {}",
                frames.len(),
                iso(frames[0].t),
                iso(frames[frames.len() - 1].t)
            );
            // The mismatch that matters: options strategies can only trade where
            // the tape reaches, however many bars there are.
            let covered = bars.iter().filter(|b| b.time >= frames[0].t && b.time <= frames[frames.len() - 1].t).count();
            println!(
                "          the tape covers {covered} of {} bars ({:.1}%)",
                bars.len(),
                100.0 * covered as f64 / bars.len() as f64
            );
        }
        _ => println!("timeline: none — options strategies will be skipped"),
    }
    println!();
}

/// State of the tape behind a run.
///
/// Printed because the collector writes into the same store this reads from, so
/// two runs an hour apart measure different samples and report different
/// numbers for the same strategy. That happened — a profit factor moved from
/// 2.949 to 3.298 on the same nine trades — and without this line there is
/// nothing in the output to attribute the difference to.
fn describe_tape(timeline: Option<&OptionsTimeline>, prints: usize) {
    let frames = timeline.map_or(0, |t| t.len());
    println!("tape:     {prints} prints, {frames} frames  <- a result is only comparable to another run over the same tape");
    println!();
}

fn run_compare(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) {
    println!("== every method at its default parameters ==");
    println!("{:<20} {:>7} {:>8} {:>9} {:>8} {:>11}", "strategy", "trades", "win%", "profit", "expect", "maxDD$");
    for row in compare_strategies_guarded(registry, bars, rules, timeline, gate, guards) {
        match (row.skipped, row.metrics) {
            (Some(reason), _) => println!("{:<20} {reason}", row.id),
            (None, Some(m)) => println!(
                "{:<20} {:>7} {:>7.1}% {:>9.3} {:>8.3} {:>11.0}",
                row.id, m.trades, m.win_rate * 100.0, m.profit_factor, m.expectancy, m.max_drawdown_usd
            ),
            _ => println!("{:<20} no result", row.id),
        }
    }
    println!();
}

fn run_sweeps(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) {
    println!("== parameter sweeps (in-sample; read the median, not the best) ==");
    println!("{:<20} {:>6} {:>7} {:>12} {:>12} {:>11}", "strategy", "cells", "usable", "medianPF", "medianExp", "profitable");
    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let result = sweep_strategy_guarded(strategy.as_ref(), bars, rules, timeline, min_trades_per_cell, guards);
        let s = &result.summary;
        println!(
            "{:<20} {:>6} {:>7} {:>12.3} {:>12.3} {:>10.0}%",
            result.strategy,
            s.cells,
            s.usable_cells,
            s.median_profit_factor,
            s.median_expectancy,
            s.profitable_share * 100.0
        );
    }
    println!();
}

#[allow(clippy::too_many_arguments)]
fn run_walk_forward(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    guards: Option<&Guards>,
) {
    println!("== walk-forward, out of sample ({folds} folds) ==");
    println!("{:<20} {:>7} {:>8} {:>9} {:>8} {:>9}  verdict", "strategy", "trades", "win%", "profit", "expect", "maxDD");

    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let Some(result) =
            walk_forward_guarded(strategy.as_ref(), bars, rules, timeline, folds, select_by, min_trades_per_cell, guards)
        else {
            println!("{:<20} not enough bars", strategy.id());
            continue;
        };
        let m = &result.oos;
        let v = verdict(m, gate);
        println!(
            "{:<20} {:>7} {:>7.1}% {:>9.3} {:>8.3} {:>11.0}  {}",
            strategy.id(),
            m.trades,
            m.win_rate * 100.0,
            m.profit_factor,
            m.expectancy,
            m.max_drawdown_usd,
            if v.promising { "PASS".to_string() } else { format!("fail: {}", v.reasons.join("; ")) }
        );
    }
    println!();
    println!("A failing gate is the result. Widening the grid until something passes");
    println!("is how a backtest stops measuring the market and starts measuring the search.");
}

/// A null matched to the result it is judging.
///
/// The random-entry control answers "what does this pipeline produce from
/// noise", and its runs average hundreds of trades. That makes it the wrong
/// yardstick for a strategy with nine: a profit factor over nine trades is
/// enormously more dispersed than one over eight hundred, so comparing them
/// makes a lucky handful look extraordinary.
///
/// This control holds everything fixed except the one thing under test. Same
/// entry bars, same stop and target *distances*, same costs, same exit rules —
/// only the direction is a coin flip, mirrored around the entry price so the
/// geometry is preserved. The resulting distribution has exactly the same trade
/// count as the result, which is what makes the percentile mean something.
#[allow(clippy::too_many_arguments)]
fn run_direction_null(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    id: String,
    samples: usize,
    guards: Option<&Guards>,
) {
    use rayon::prelude::*;

    let Ok(strategy) = registry.get(&id) else {
        println!("unknown strategy: {id}");
        return;
    };
    if strategy.needs_options() && timeline.is_none() {
        println!("{id} needs an options timeline and the store has no tape");
        return;
    }
    // `--params=key=value,key=value` applies a preset, so the control can be
    // run on the hypothesis actually tested rather than on the method's
    // defaults.
    let mut params = strategy.default_params();
    for pair in arg("params", "").split(',').filter(|s| !s.is_empty()) {
        match pair.split_once('=').and_then(|(k, v)| v.parse::<f64>().ok().map(|v| (k, v))) {
            Some((key, value)) if params.contains(key) => params.set(key, value),
            _ => {
                println!("bad or unknown --params entry `{pair}` for {id}");
                return;
            }
        }
    }
    // `--filters=weekdays;hours:0800-1200;vol:14/100:1.2-99` — the batch's own
    // gates, so a filtered variant's control is its own and not its sibling's.
    let filters: Vec<fd_strategy::filter::Filter> = match arg("filters", "")
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| fd_strategy::filter::Filter::parse_for_market(s, &rules.news_currencies))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(f) => f,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    let gated = fd_strategy::filter::Filtered { inner: strategy, filters: filters.clone() };
    let actual = run_backtest_guarded(bars, &gated, &params, rules, guards, timeline, Range::default(), None);
    if actual.trades.is_empty() {
        println!("{id} took no trades; there is nothing to compare");
        return;
    }
    if !filters.is_empty() {
        println!("  filters: {}", gated.describe());
    }

    // A strategy that manages its own exits decides them from the side it
    // holds — a momentum rule flips when the position disagrees with the
    // sign — so replaying it with a coin-flip side keeps the method's exit
    // rule inside the null (the tsmom-2 pass read a null median of 1.56).
    // For those, the null is the method's own trades with their sides
    // permuted: same intervals, same lots, same costs, only the side.
    let self_managed = strategy.exits() == fd_strategy::registry::Exits::Strategy;
    let mut curve: Vec<f64> = (0..samples)
        .into_par_iter()
        .map(|seed| {
            if self_managed {
                return permuted_sides_pf(&actual.trades, rules, seed as u64 + 1);
            }
            let flipped = fd_backtest::DirectionFlipped { inner: strategy, seed: seed as u64 + 1 };
            let gated = fd_strategy::filter::Filtered { inner: &flipped, filters: filters.clone() };
            run_backtest_guarded(bars, &gated, &params, rules, guards, timeline, Range::default(), None)
                .metrics
                .profit_factor
        })
        .filter(|pf| pf.is_finite())
        .collect();
    curve.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let quantile = |q: f64| curve[(((curve.len() - 1) as f64) * q).round() as usize];
    let pf = actual.metrics.profit_factor;
    let below = curve.iter().filter(|value| **value < pf).count();
    let percentile = 100.0 * below as f64 / curve.len() as f64;

    println!("== direction control: {id}, {} trades, {samples} coin-flip assignments ==", actual.trades.len());
    if !arg("params", "").is_empty() {
        println!("  preset: {}", arg("params", ""));
    }
    if self_managed {
        println!("  the method's own trades — entry, exit, lots, costs — with each side a coin flip");
    } else {
        println!("  entries, stops and targets held fixed; only the side is randomised");
    }
    println!("  profit factor of the same trades with random direction:");
    println!("    p05 {:.3}   p25 {:.3}   p50 {:.3}   p75 {:.3}   p95 {:.3}   max {:.3}",
        quantile(0.05), quantile(0.25), quantile(0.50), quantile(0.75), quantile(0.95), curve[curve.len() - 1]);
    println!();
    println!("  actual: {pf:.3}  ->  {percentile:.0}th percentile  (p = {:.3})", 1.0 - percentile / 100.0);
    println!();
    if percentile >= 95.0 {
        println!("  Outside its own null. That is a finding worth taking further.");
    } else {
        println!("  Inside its own null. On this sample the direction rule has not been");
        println!("  distinguished from a coin flip, however good the profit factor looks.");
    }
    println!();
}

/// The profit factor of `trades` with each side re-drawn by coin flip.
///
/// Moved into `fd_backtest::direction` on 2026-09-23 so the rebate rescore
/// reads the same control this mode does rather than a second copy of it. The
/// arithmetic did not change.
fn permuted_sides_pf(trades: &[fd_backtest::Trade], rules: &TradingRules, seed: u64) -> f64 {
    fd_backtest::profit_factor_of(&fd_backtest::permuted_sides_pnls(trades, rules, seed))
}

/// What this pipeline produces when it is fed no signal.
///
/// Runs the random-entry control through the identical walk-forward — the same
/// grid size, the same parameter selection on the training window, the same
/// costs — once per seed, and reports the distribution of out-of-sample profit
/// factors.
///
/// That distribution is the thing to read a real result against. If a coin flip
/// clears 1.1 a quarter of the time on this tape, a method that scores 1.1 has
/// shown nothing, and the gate needs to sit above the noise rather than at a
/// number that sounded sensible.
#[allow(clippy::too_many_arguments)]
fn run_null_control(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
) {
    use rayon::prelude::*;

    println!("== control: {seeds} runs of random entry through the same pipeline ==");

    let mut results: Vec<(f64, f64, usize)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let control = fd_backtest::RandomEntry;
            // The seed is fixed per run and the grid varies the rest, so each
            // run is one complete pass of the machinery over a different world.
            let mut defaults = control.default_params();
            defaults.set("seed", seed as f64 + 1.0);
            let result = walk_forward_seeded(
                &control,
                &defaults,
                bars,
                rules,
                folds,
                select_by,
                min_trades_per_cell,
                guards,
            )?;
            Some((result.oos.profit_factor, result.oos.expectancy, result.oos.trades))
        })
        .collect();

    if results.is_empty() {
        println!("no control run produced a trade; nothing to compare against");
        return;
    }
    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let pf = |q: f64| {
        let index = ((results.len() - 1) as f64 * q).round() as usize;
        results[index].0
    };
    let median_trades = results[results.len() / 2].2;
    let passing = results.iter().filter(|(p, e, t)| *t >= gate.min_trades && *p >= gate.min_profit_factor && *e >= gate.min_expectancy_r).count();

    println!("  runs that produced trades: {} of {seeds}", results.len());
    println!("  median trades per run:     {median_trades}");
    println!();
    println!("  out-of-sample profit factor of pure noise:");
    println!("    p05  {:.3}", pf(0.05));
    println!("    p25  {:.3}", pf(0.25));
    println!("    p50  {:.3}", pf(0.50));
    println!("    p75  {:.3}", pf(0.75));
    println!("    p95  {:.3}", pf(0.95));
    println!("    max  {:.3}", results[results.len() - 1].0);
    println!();
    println!(
        "  {passing} of {} control runs ({:.1}%) clear the gate (PF >= {:.2}, exp >= {:.2}R, >= {} trades)",
        results.len(),
        100.0 * passing as f64 / results.len() as f64,
        gate.min_profit_factor,
        gate.min_expectancy_r,
        gate.min_trades
    );
    println!();
    println!("  That percentage is the gate's false-positive rate on this tape. A method");
    println!("  scoring inside this distribution has not been distinguished from chance.");
    println!();

    // Where each real method falls inside the noise. This is the comparison the
    // whole control exists for; a profit factor on its own does not say whether
    // a method beat the market or merely won a search.
    let curve: Vec<f64> = results.iter().map(|(pf, _, _)| *pf).collect();
    println!("  where each method falls inside that distribution:");
    println!("  {:<20} {:>10} {:>12}", "strategy", "OOS PF", "percentile");
    // Options strategies are placed inside the same noise: the control never
    // reads the tape, so its distribution is the one they have to beat too.
    // They are skipped only when there is no tape to run them on. Bound the
    // bars to the tape's span with --from/--to for a fair count.
    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        let Some(result) =
            walk_forward_guarded(strategy.as_ref(), bars, rules, timeline, folds, select_by, min_trades_per_cell, guards)
        else {
            continue;
        };
        let pf = result.oos.profit_factor;
        if !pf.is_finite() {
            continue;
        }
        let below = curve.iter().filter(|value| **value < pf).count();
        let percentile = 100.0 * below as f64 / curve.len() as f64;
        let verdict = if percentile >= 95.0 { "outside the noise" } else { "inside the noise" };
        println!("  {:<20} {pf:>10.3} {percentile:>11.0}%  {verdict}", strategy.id());
    }
    println!();
    println!("  A method at the 80th percentile of a coin flip is what picking the best");
    println!("  of several coin flips looks like. Only the ones outside have said anything.");
    println!();
}

/// The rebate question, asked of the machine that can answer it.
///
/// An introducing broker is paid per lot, so a bot that trades often is worth
/// something to the broker even with no edge — *as long as the client's
/// account survives*. The number that decides that is what the client loses
/// per lot after costs. This prints, for every method at its defaults, for its
/// intraday version (flat over the break, so no swap), and for the random-entry
/// control, the trades it makes per year and what one lot of it costs the
/// client on average — both lot-weighted (what the venue sees) and per trade at
/// a fixed lot (what a client running fixed size feels). The last column is
/// the rebate per lot at which that client breaks even. Compare it with the
/// actual rebate; nothing else here is a judgement.
fn run_volume(registry: &Registry, bars: &[Bar], rules: &TradingRules, timeline: Option<&OptionsTimeline>, guards: Option<&Guards>) {
    use fd_strategy::filter::{Filter, Filtered};
    let run = |s: &dyn fd_strategy::registry::Strategy, p: &fd_strategy::registry::Params| {
        run_backtest_guarded(bars, s, p, rules, guards, timeline, Range::default(), None)
    };

    let span_years = (bars[bars.len() - 1].time - bars[0].time) as f64 / (365.25 * 86_400_000.0);
    let round_trip = rules.spread * rules.contract_size + 2.0 * rules.commission_per_lot;
    println!("== volume economics: what one lot costs the client, and the rebate that would cover it ==");
    println!(
        "contract {} | spread {} + commission {:.2}/side = {:.2} USD per lot round trip | swap {:.2}/{:.2} per lot-night | {span_years:.2} years",
        rules.contract_size, rules.spread, rules.commission_per_lot, round_trip, rules.swap_long_per_lot, rules.swap_short_per_lot
    );
    println!("all figures per lot of this contract; on a 100 oz standard lot multiply gold by 100");
    println!();
    println!(
        "{:<28} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>11}",
        "strategy", "trades", "trades/yr", "swap/lot", "net/lot", "ex-swap", "fixed-lot", "breakeven$"
    );

    let intraday = || vec![Filter::weekdays(), Filter::flat(1630, 1815)];
    let mut rows: Vec<(String, fd_backtest::BacktestResult)> = Vec::new();
    for s in registry.all() {
        if s.needs_options() && timeline.is_none() || s.id() == "buy-and-hold" {
            continue;
        }
        let p = s.default_params();
        rows.push((s.id().to_string(), run(s.as_ref(), &p)));
        let flat = Filtered { inner: s.as_ref(), filters: intraday() };
        rows.push((format!("{}/intraday", s.id()), run(&flat, &p)));
    }
    let control = fd_backtest::RandomEntry;
    let mut p = control.default_params();
    p.set("entryRate", 0.1);
    p.set("seed", 7.0);
    rows.push(("null-random".to_string(), run(&control, &p)));
    let flat = Filtered { inner: &control, filters: intraday() };
    rows.push(("null-random/intraday".to_string(), run(&flat, &p)));

    for (id, result) in rows {
        let trades = result.trades.len();
        if trades == 0 {
            println!("{id:<28} no trades");
            continue;
        }
        let lots: f64 = result.trades.iter().map(|t| t.lots).sum();
        let swap: f64 = result.trades.iter().map(|t| t.swap_usd).sum();
        let net: f64 = result.trades.iter().map(|t| t.pnl_usd).sum();
        // Lot-weighted: the venue's view of a dollar per lot turned over.
        let (swap_per_lot, net_per_lot) = (swap / lots, net / lots);
        // Fixed-lot: the same trades at one lot each, which is what a client
        // running a fixed size experiences. Sizing by risk weights tight-stop
        // trades more heavily and the two can disagree.
        let fixed: f64 = result.trades.iter().map(|t| t.pnl_usd / t.lots).sum::<f64>() / trades as f64;
        println!(
            "{id:<28} {trades:>7} {:>9.0} {:>9.2} {:>9.2} {:>9.2} {:>10.2} {:>11.2}",
            trades as f64 / span_years,
            swap_per_lot,
            net_per_lot,
            net_per_lot - swap_per_lot,
            fixed,
            -fixed
        );
    }
    println!();
    println!("net/lot is what a lot of this bot cost the client, all in; ex-swap is the same with the");
    println!("financing removed, i.e. what the intraday version could at best reach; fixed-lot is per");
    println!("trade at one lot. breakeven$ is the rebate per lot that leaves a fixed-lot client flat.");
    println!();
}

/// The column header of the `--mode=hypotheses` table, as every published
/// receipt carries it.
///
/// Lifted out of `run_hypotheses` for one reason: so a test can assert it is
/// **byte-identical** to the header line in a receipt already in the record.
/// A new column added by widening this table would shift every figure on every
/// row, and the proof that the 2026-10-07 drawdown line did not do that is
/// that this string still matches the record character for character. The
/// drawdown is printed on its OWN line below the row, exactly as
/// `expectancy_net` was added on 2026-10-07, for the same reason.
fn gate_header() -> String {
    format!(
        "{:<12} {:<18} {:>6} {:>7} {:>7} {:>8} {:>8} {:>8} {:>5}  verdict",
        "hypothesis", "base", "trades", "OOS PF", "expect", "null p50", "null p95", "swap$", "pct"
    )
}

/// One row of that table, same reason: a test re-formats a row that is already
/// in the record from the row's own figures and asserts byte equality, which
/// is what "added beside, nothing moved" means when it is checked rather than
/// asserted.
#[allow(clippy::too_many_arguments)]
fn gate_row(
    label: &str,
    base: &str,
    trades: usize,
    profit_factor: f64,
    expectancy: f64,
    null_p50: f64,
    null_p95: f64,
    swap_usd: f64,
    pct: &str,
    verdict: &str,
) -> String {
    format!(
        "{label:<12} {base:<18} {trades:>6} {profit_factor:>7.3} {expectancy:>7.3} {null_p50:>8.3} {null_p95:>8.3} {swap_usd:>8.0} {pct:>5}  {verdict}"
    )
}

/// THE RISK FIGURE, in the words it has to be read in.
///
/// `max_drawdown_usd` is the worst peak-to-trough fall of the **closed-trade**
/// equity curve: `metrics_of` walks the trades in order, adds `pnl_usd`, and
/// tracks `peak - equity`. It therefore sees nothing that happened while a
/// position was open — a trade that went 3 R against the book and closed green
/// contributes **zero** to it, and `avg_mae` is the only field that sees that.
/// The line says so, every time, because a reader who assumes the other
/// definition assumes the worse number.
///
/// `max_drawdown_pct` divides that fall by the curve's **highest** equity, not
/// by the peak the fall started from, so it is a FLOOR on the conventional
/// max-drawdown-percent and never an overstatement. Both facts are pinned in
/// `crates/fd-backtest/tests/drawdown.rs`.
///
/// A cell with no trades gets no drawdown: `Metrics::empty` carries
/// `max_drawdown_usd = 0.0`, which is not a measurement of anything, so the
/// line refuses rather than printing a 0 that reads as "never fell".
fn drawdown_line(m: &fd_backtest::engine::Metrics) -> String {
    if m.trades == 0 {
        return "max drawdown: NOT MEASURED — this cell took no position, so there is no equity curve to fall (this is not 0.00 USD)".to_string();
    }
    let pct = if m.max_drawdown_pct.is_finite() {
        format!("{:.2}% of the book's peak equity", m.max_drawdown_pct)
    } else {
        "percent NOT MEASURED (no peak to divide by)".to_string()
    };
    format!(
        "max drawdown {:.2} USD = {pct}, on the CLOSED-TRADE curve only (an open position's excursion is not in it; avg_mae {:+.3} R is); net {:+.2} USD over {} trades",
        m.max_drawdown_usd, m.avg_mae, m.net_pnl_usd, m.trades,
    )
}

/// THE SIZE OF A TRADE, printed so the gate's own identity can be checked on
/// the row rather than assumed.
///
/// `agent/gate-legs` derived `E = Lbar x (PF - 1)` with
/// `Lbar = |sum(-R)| / n` — the gross loss per trade in R, averaged over ALL
/// trades — and found a perfect separation: 87 of 87 rows rejected by one leg
/// alone have `Lbar < 0.250R`. The record could only ever reach `Lbar` by
/// INVERTING the printed PF and expectancy, which makes any test of the
/// identity circular. Both parts are already in `Metrics` and were never
/// printed, so this line measures them directly:
///
/// * `metrics_of` splits wins from losses by `pnl_usd` sign and takes
///   `avg_loss_r` as the mean `t.r` over that loss set, so
///   `Lbar = |avg_loss_r| x (1 - win_rate)` and
///   `PF_r = avg_win_r x win_rate / (|avg_loss_r| x (1 - win_rate))`.
/// * `profit_factor` on the same struct is a ratio of **`pnl_usd`**, so it is
///   a different number in a different unit. Both are printed, because the
///   distance between them IS the unit mismatch that `AGENT-BRIEF-ADDENDUM-6`
///   section I counts at 166 rows of the record printing PF and expectancy
///   with opposite signs.
///
/// The identity is restated against `PF_r` and the residual is printed, so a
/// row where it does not hold says so on its own line instead of being
/// discovered later. A row whose loss set is empty has no `Lbar` — that is
/// NOT 0, and the line says which.
fn lbar_line(m: &fd_backtest::engine::Metrics) -> String {
    if m.trades == 0 {
        return "Lbar: NOT MEASURED — this cell took no position (this is not 0.000 R)".to_string();
    }
    if !m.avg_loss_r.is_finite() {
        return format!(
            "Lbar: NOT MEASURED — no losing trade in {} (win rate {:.1}%), so the gross loss per trade has no value; this is not 0.000 R and PF_usd {:.3} has no finite denominator in R either",
            m.trades,
            m.win_rate * 100.0,
            m.profit_factor
        );
    }
    let loss_share = 1.0 - m.win_rate;
    let lbar = m.avg_loss_r.abs() * loss_share;
    let gross_win_r = if m.avg_win_r.is_finite() { m.avg_win_r * m.win_rate } else { 0.0 };
    let pf_r = if lbar > 0.0 { gross_win_r / lbar } else { f64::INFINITY };
    let implied = lbar * (pf_r - 1.0);
    let residual = m.expectancy - implied;
    let redundant = if lbar >= 0.250 { "expectancy leg REDUNDANT (Lbar >= 0.250R)" } else { "expectancy leg BINDS (Lbar < 0.250R)" };
    format!(
        "Lbar {lbar:.4} R = |avg_loss_r| {:.4} x loss share {loss_share:.4} over {} trades; PF_r {pf_r:.4} (R) vs PF_usd {:.4} (USD, gap {:+.4}); identity E = Lbar(PF_r-1) = {implied:+.4} R vs expectancy {:+.4} R, residual {residual:+.5} R; {redundant}",
        m.avg_loss_r.abs(),
        m.trades,
        m.profit_factor,
        pf_r - m.profit_factor,
        m.expectancy,
    )
}

/// A declared batch of hypotheses, each read against its matched null.
#[allow(clippy::too_many_arguments)]
fn run_hypotheses(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    batch_name: &str,
    batch_file: &str,
    fixed: bool,
    guards: Option<&Guards>,
    also_registered_stop: bool,
    exit_mix: bool,
    null_sides: NullSides,
) {
    let (batch, shown) = if batch_file.is_empty() {
        match hypothesis_batch(batch_name) {
            Some(b) => (b, batch_name.to_string()),
            None => {
                println!("unknown batch `{batch_name}` (have: gold-intraday, ict-m1, ict-m5, ict-oos; or --batch-file=<toml>)");
                return;
            }
        }
    } else {
        match batch_from_file(std::path::Path::new(batch_file)) {
            Ok(b) => (b, batch_file.to_string()),
            Err(e) => {
                println!("{e}");
                return;
            }
        }
    };
    if fixed {
        println!("== hypotheses `{shown}`: {} declared, FIXED parameters over the whole window (no selection), each against {seeds} matched null runs ==", batch.len());
    } else {
        println!("== hypotheses `{shown}`: {} declared, walk-forward ({folds} folds), each against {seeds} matched null runs ==", batch.len());
    }
    // In the receipt's own header, not only the run header: a percentile is
    // meaningless without the null it was read against, and these files are
    // quoted on their own.
    println!("null sides: {} ({})", null_sides.as_str(), null_sides.describe());
    println!("swap: long {:.2} / short {:.2} USD per lot per night; spread {}", rules.swap_long_per_lot, rules.swap_short_per_lot, rules.spread);
    println!("{}", news_line());
    println!("{}", news_scope_line(rules));
    println!("{}", guards_line(guards));
    println!();
    println!("{}", gate_header());
    let mut survivors = Vec::new();
    for hypothesis in &batch {
        // WHETHER THIS ROW CAN SEE THE TAPE, said out loud before its numbers.
        // A method that `needs_options()` and is handed no timeline does not
        // fail — it takes no position at all, which prints as a gate miss and
        // reads like a measurement. Until 2026-10-09 the hypotheses path
        // passed `None` unconditionally, so every such row in this mode was
        // that, and the walk-forward path still is.
        let wants_options = registry.get(&hypothesis.base).map(|s| s.needs_options()).unwrap_or(false);
        if wants_options {
            match (timeline, fixed) {
                (Some(t), true) => println!(
                    "{:<12} {:<18} reads the tape: {} frames reach this row",
                    hypothesis.label,
                    hypothesis.base,
                    t.len()
                ),
                (_, false) => {
                    println!(
                        "{:<12} {:<18} refused: an options-reading method cannot be measured on the walk-forward hypotheses path — it still passes no timeline. Re-run with --fixed.",
                        hypothesis.label, hypothesis.base
                    );
                    continue;
                }
                (None, true) => {
                    println!(
                        "{:<12} {:<18} refused: this method reads options and this run has no timeline — that is `options_source` in the config or no tape in the store, NOT a result",
                        hypothesis.label, hypothesis.base
                    );
                    continue;
                }
            }
        }
        let outcome = if fixed {
            fd_backtest::hypotheses::run_hypothesis_fixed_options(
                registry,
                hypothesis,
                bars,
                rules,
                gate,
                seeds,
                guards,
                fd_backtest::hypotheses::CostMatch::Method,
                null_sides,
                timeline,
            )
            .map(Some)
        } else {
            run_hypothesis_sides(
                registry, hypothesis, bars, rules, folds, select_by, min_trades_per_cell, gate, seeds, guards, null_sides,
            )
        };
        let report = match outcome {
            Ok(Some(report)) => report,
            Ok(None) => {
                println!("{:<12} {:<18} not enough bars", hypothesis.label, hypothesis.base);
                continue;
            }
            Err(e) => {
                println!("{:<12} {:<18} refused: {e}", hypothesis.label, hypothesis.base);
                continue;
            }
        };
        let m = &report.oos;
        let verdict = if report.survives() {
            "SURVIVES".to_string()
        } else if report.verdict.promising {
            "gate pass, inside the noise".to_string()
        } else {
            format!("fail: {}", report.verdict.reasons.join("; "))
        };
        println!(
            "{}",
            gate_row(
                &report.label,
                &report.base,
                m.trades,
                m.profit_factor,
                m.expectancy,
                report.null_quantile(0.5),
                report.null_quantile(0.95),
                report.swap_usd,
                // A percentile read against a null whose seeds all agree is one
                // comparison dressed as a quantile, so the column says `null`
                // rather than a number it cannot support. It is not 0 and it is
                // not 100; the line below says why.
                &match report.percentile_or_null() {
                    Some(p) => format!("{p:.0}%"),
                    None => "null".to_string(),
                },
                &verdict,
            )
        );
        println!("{:<12} {:<18} {}  — {}", "", "", report.filters, report.why);
        // THE RISK FIGURE BESIDE THE PROFIT FIGURE. Until 2026-10-07 this mode
        // printed neither: `max_drawdown_usd` and `max_drawdown_pct` have been
        // in `Metrics` the whole time and no printer ever read them, so every
        // profit factor this record holds was published with no risk number
        // next to it. PF 1.200 at an 8% drawdown and PF 1.200 at a 60% one are
        // not the same row for a real account, and the desk could not tell
        // them apart. `docs/decisions/2026-10-07-drawdown-printed.md`.
        println!("{:<12} {:<18} {}", "", "", drawdown_line(m));
        println!("{:<12} {:<18} {}", "", "", lbar_line(m));
        // EXPECTANCY WITH THE FINANCING IN IT, on every row that paid any.
        //
        // `expectancy` above is the mean of `r = points / risk`, which is
        // PRICE over price: the spread is inside it because `apply_costs`
        // moved both fills, but commission and swap are not. `profit_factor`
        // reads `pnl_usd` and sees all three. Measured 2026-10-06 by
        // `agent/n5`: `qs-h15` printed expectancy +0.130R identically at swap
        // 0.00 and at -0.83 per lot-night while its profit factor fell
        // 1.262 -> 0.594 against -$6,166 of financing. Half the gate could not
        // see the largest cost of an overnight hold.
        //
        // Printed ONLY where it can differ, so a zero-swap, zero-commission
        // receipt — every receipt this record holds — is unchanged, and
        // `r_net == r` there by construction. The old column is never
        // overwritten; this is a second column beside it.
        // `docs/decisions/2026-10-07-instrument-repair.md`.
        if report.swap_usd != 0.0 || rules.commission_per_lot != 0.0 {
            if m.expectancy_net.is_finite() {
                println!(
                    "{:<12} {:<18} expectancy_net {:+.3} R and total_r_net {:+.2} R (pnl_usd / risk_usd: commission and swap IN) \
                     vs expectancy {:+.3} R and total_r {:+.2} R (points / risk: spread only) — the gap is {:+.3} R per trade of financing and commission",
                    "",
                    "",
                    m.expectancy_net,
                    m.total_r_net,
                    m.expectancy,
                    m.total_r,
                    m.expectancy_net - m.expectancy,
                );
            } else {
                println!(
                    "{:<12} {:<18} expectancy_net: NOT MEASURED — at least one trade on this row carries no risk unit in USD, so its result cannot be put in R with the costs in it (this is not 0.000 R)",
                    "", "",
                );
            }
        }
        // What the count-matching ACHIEVED, printed on every row rather than
        // assumed: the control's median out-of-sample trade count against the
        // method's. The registration of 2026-09-23 calls anything outside a
        // quarter either way a null that is not this method's size, and a
        // percentile read against such a null is not this method's percentile.
        println!(
            "{:<12} {:<18} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
            "",
            "",
            fd_backtest::hypotheses::median_count(&report.null_trades),
            m.trades,
            report.count_match(),
            if report.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" },
        );
        // The SIDE ratio, method against control, on every row and whichever
        // null ran. On a coin-flip run this line is what shows the defect: a
        // long-only method read against a 50%-long control, on an instrument
        // whose unconditional drift is +0.3946 ATR20 per 5 sessions at
        // t = +6.74. On a matched run it is the achieved match, measured rather
        // than assumed, for the same reason the count match above is.
        println!(
            "{:<12} {:<18} long share: method {:.3} vs the null's median {:.3}{}",
            "",
            "",
            report.long_share,
            report.null_long_share_median(),
            match (
                null_sides.matches_ratio(),
                fd_backtest::hypotheses::side_ratio_is_one_sided(report.long_share),
                report.side_matched(),
            ) {
                (false, true, _) =>
                    "  ** outside 40-60% long against a coin: this percentile carries the instrument's drift **",
                (true, _, false) => "  ** the control's side ratio is not the method's **",
                _ => "",
            },
        );
        // The COST, measured in what was paid rather than inferred from the
        // stop — which is the only form the hold branch has, since it has no
        // stop at all and its cost is its trade count times the spread.
        println!(
            "{:<12} {:<18} spread paid: method {:.2} USD vs the null's median {:.2} USD — cost match {:.2}{}",
            "",
            "",
            report.cost_usd,
            fd_backtest::hypotheses::median_f64(&report.null_cost_usd),
            report.cost_match(),
            if report.cost_match().is_nan() {
                "  (nothing to compare)"
            } else if report.cost_matched() {
                ""
            } else {
                "  ** outside the band: the control did not pay what the method paid **"
            },
        );
        // And the EXPOSURE, which is the quantity the drift is actually
        // collected in. TWO figures, because it has two factors and either one
        // alone is an instrument that lies: the SIGNED SHARE of time (+1.000 is
        // long the whole time) says which way the exposure pointed, and the
        // GROSS TIME IN MARKET says how much of it there was. A control with
        // the method's side ratio exactly, in the market 1.35x as long,
        // collects 35% more drift and has a signed share difference of zero.
        println!(
            "{:<12} {:<18} exposure: signed share of time method {:+.3} vs the null's {:+.3}; time in market method {:.0} min vs the null's median {:.0} min — ratio {:.2}{}",
            "",
            "",
            report.signed_share(),
            report.null_signed_share_median(),
            report.gross_minutes,
            fd_backtest::hypotheses::median_f64(&report.null_gross_minutes),
            report.time_in_market_match(),
            if report.exposure_matched() {
                ""
            } else {
                "  ** the control did not collect the drift the method did **"
            },
        );
        // WHAT A PERCENTILE AGAINST THIS NULL DOES AND DOES NOT MEAN. Measured
        // 2026-10-02 by `agent/new-method-2`: the existing nulls' own median
        // profit factor is 0.867 and is below 1.000 in 240 of 247 published
        // cells, and nine of those cells sat at or above the 95th percentile
        // while LOSING money. Beating a losing null is beating a loss. The
        // absolute gate (PF 1.2, expectancy 0.05R, 30 trades) is what carries
        // profitability and `survives()` requires it as well as the 95th - but
        // the percentile is the number a reader takes, so the row says in
        // words what its own percentile is a comparison against.
        if report.null_quantile(0.5).is_finite() && report.null_quantile(0.5) < 1.0 {
            println!(
                "{:<12} {:<18} the null's own median LOSES money (p50 {:.3} < 1.000): a high percentile here means \"loses less than random entry at the same cost, count and side ratio\", NOT \"makes money\" - the gate leg is what says that",
                "",
                "",
                report.null_quantile(0.5),
            );
        }
        // A null whose seeds all produce the same profit factor is not a
        // distribution and its percentile is one comparison, not a quantile.
        // A window hold at `entryRate = 1.0` with a fixed hold has the side
        // coin as its ONLY random input, so matching a 0% or 100% long share
        // leaves nothing random — which also means the coin-flip null's whole
        // spread on such a row WAS the side lottery, a lottery the method
        // never faced.
        if !report.null_has_spread() && !report.null_pf.is_empty() {
            println!(
                "{:<12} {:<18} ** the null has no spread across {} seeds (p50 = p95 = {:.3}): this is one comparison, not a quantile, and the percentile is reported as null **",
                "",
                "",
                report.null_pf.len(),
                report.null_quantile(0.5),
            );
        }
        // And what the COST matching achieved, printed the same way and for
        // the same reason: cost as a fraction of risk is `spread / stop`, so
        // a control at a different stop is a control at a different cost, and
        // a method that merely widens its stop clears such a control without
        // predicting anything (`docs/hypotheses/2026-09-24-cost-matched-null.md`).
        if let Some(stop) = &report.control_stop {
            println!(
                "{:<12} {:<18} cost-matched null: control stop {:.3} ATR = {:.2} points, cost {:.2}% of R ({}){}",
                "",
                "",
                stop.atr,
                stop.points(),
                100.0 * stop.cost_fraction_of_r(rules),
                stop.source.as_str(),
                match stop.named {
                    Some(v) if stop.source == fd_backtest::hypotheses::StopSource::Realised =>
                        format!("; the method DECLARES stopAtr {v:.2} and does not use it"),
                    _ => String::new(),
                },
            );
            println!(
                "{:<12} {:<18} the method's own realised stop: median {:.3} ATR = {:.2} points over {} trades",
                "", "", stop.realised_atr, stop.realised_points, stop.measured_trades,
            );
        }
        // The same row against the control the record used before
        // 2026-09-24, printed beside the corrected one rather than replaced
        // by it. The method's run is the same call with the same parameters,
        // so the gate figures are checked for having held still rather than
        // assumed to have.
        if also_registered_stop {
            if !fixed {
                println!("{:<12} {:<18} --null-registered-stop needs --fixed; no original printed", "", "");
            } else {
                match fd_backtest::hypotheses::run_hypothesis_fixed_as(
                    registry,
                    hypothesis,
                    bars,
                    rules,
                    gate,
                    seeds,
                    guards,
                    fd_backtest::hypotheses::CostMatch::RegisteredStop,
                    null_sides,
                ) {
                    Ok(before) => {
                        let moved = if before.percentile.is_nan() && report.percentile.is_nan() {
                            "both unmeasured".to_string()
                        } else {
                            format!("{:+.0} points of percentile", report.percentile - before.percentile)
                        };
                        println!(
                            "{:<12} {:<18} at the control's registered 1.5 ATR (the record's reading): pct {:.0}%, \
                             null p50 {:.3} p95 {:.3}, count match {:.2} — corrected {:.0}%, {moved}",
                            "",
                            "",
                            before.percentile,
                            before.null_quantile(0.5),
                            before.null_quantile(0.95),
                            before.count_match(),
                            report.percentile,
                        );
                        for (name, a, b) in [
                            ("trades", before.oos.trades as f64, m.trades as f64),
                            ("profit factor", before.oos.profit_factor, m.profit_factor),
                            ("expectancy", before.oos.expectancy, m.expectancy),
                        ] {
                            if a.to_bits() != b.to_bits() {
                                println!(
                                    "{:<12} {:<18} ** {name} MOVED between the two readings: {a} vs {b} — \
                                     the null is not the only thing that changed **",
                                    "", "",
                                );
                            }
                        }
                    }
                    Err(e) => println!("{:<12} {:<18} original refused: {e}", "", ""),
                }
            }
        }
        // How often a guard acted on this row's own out-of-sample runs, so a
        // receipt shows whether a bounded number was bounded in practice.
        if guards.is_some() {
            println!("{:<12} {:<18} {}", "", "", report.guard_activity());
        }
        // How each position left, and the mean hold. `TIMEOUT` is the hold cap
        // acting; a zero there on a row whose mean hold is well under the cap
        // means the cap was not binding on that row, which is a statement the
        // percentile cannot make.
        if exit_mix {
            let mix = if m.exits.is_empty() {
                "no exits recorded".to_string()
            } else {
                m.exits.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")
            };
            println!("{:<12} {:<18} exits: {mix}; mean hold {:.1} min", "", "", m.avg_hold_min);
        }
        if report.survives() {
            survivors.push(format!("{}/{}", report.label, report.base));
        }
    }
    println!();
    if survivors.is_empty() {
        println!("Nothing survived: no hypothesis is both past the gate and outside its own null.");
        println!("That is the result. The list is closed; add a hypothesis only with a new reason.");
    } else {
        println!("Survivors: {} — worth a decision record and a direction null before anything else.", survivors.join(", "));
    }
    println!();
}

/// Every construct in a batch, scored twice: as the record closed it, and
/// with the introducing-broker rebate credited beside the book.
///
/// **The credit never touches the cost model.** `rules.spread` is what the
/// venue charges and stays what it was; the net columns come from a credited
/// copy of the same trades. Gross is byte-identical to a `--mode=hypotheses`
/// run of the same batch, which is the property
/// `fd-backtest/src/rebate.rs` pins and `docs/decisions/2026-09-21-rebate-credit-line.md`
/// asks for.
///
/// **Both nulls carry the rebate.** The matched null's own out-of-sample
/// trades and the direction null's own trades are credited with the same
/// arithmetic, so the percentiles compare like with like. A run that could
/// not do that for one of the two says so in its header rather than printing
/// a percentile that means nothing.
#[allow(clippy::too_many_arguments)]
fn run_rescore(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &fd_backtest::PromisingGate,
    seeds: usize,
    direction_samples: usize,
    batch_file: &str,
    rebate: Option<fd_backtest::Rebate>,
    guards: Option<&Guards>,
    exit_mix: bool,
) {
    let Some(rebate) = rebate else {
        println!("no [rebate] table in the config directory's accounts.toml, and no --rebate-share=.");
        println!("That is 'no arrangement is recorded', not a rebate of zero, so nothing is scored.");
        return;
    };
    let batch = match batch_from_file(std::path::Path::new(batch_file)) {
        Ok(b) => b,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    println!("== rebate rescore `{batch_file}`: {} constructs, walk-forward ({folds} folds) ==", batch.len());
    println!("rebate:   {:.2} of the round-turn spread, credited BESIDE the book", rebate.share_of_spread);
    println!("          gross columns are unchanged; the cost model is untouched");
    println!("nulls:    matched null {seeds} runs and direction null {direction_samples} draws, BOTH carrying the same credit");
    println!("swap:     long {:.2} / short {:.2} USD per lot per night; spread {}", rules.swap_long_per_lot, rules.swap_short_per_lot, rules.spread);
    println!("{}", news_line());
    println!("{}", news_scope_line(rules));
    println!("{}", guards_line(guards));
    println!();
    println!(
        "{:<22} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}  verdict",
        "construct", "trades", "PF gross", "PF net", "rebate$", "reb/R", "mnullG", "mnullN", "dirN"
    );

    let mut passed: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let mut not_run: Vec<String> = Vec::new();
    for hypothesis in &batch {
        let outcome = fd_backtest::hypotheses::rescore_hypothesis(
            registry,
            hypothesis,
            bars,
            rules,
            folds,
            select_by,
            min_trades_per_cell,
            gate,
            seeds,
            direction_samples,
            rebate,
            guards,
        );
        let row = match outcome {
            Ok(Some(row)) => row,
            Ok(None) => {
                println!("{:<22} not run: the walk-forward could not be formed on these bars", hypothesis.label);
                not_run.push(format!("{}: no walk-forward on these bars", hypothesis.label));
                continue;
            }
            Err(e) => {
                println!("{:<22} not run: {e}", hypothesis.label);
                not_run.push(format!("{}: {e}", hypothesis.label));
                continue;
            }
        };
        ran += 1;
        let verdict = if row.passes_net() {
            "PASSES ALL THREE".to_string()
        } else if row.verdict_net.promising {
            format!("gate pass, inside a null (matched {:.0}, dir {:.0})", row.matched_pct_net, row.dir_pct_net)
        } else {
            format!("fail: {}", row.verdict_net.reasons.join("; "))
        };
        println!(
            "{:<22} {:>6} {:>8.3} {:>8.3} {:>8.2} {:>6.2}% {:>6.0}% {:>6.0}% {:>6.0}%  {verdict}",
            row.label,
            row.oos.trades,
            row.oos.profit_factor,
            row.oos_net.profit_factor,
            row.rebate_usd,
            row.rebate_frac_r * 100.0,
            row.matched_pct_gross,
            row.matched_pct_net,
            row.dir_pct_net,
        );
        println!(
            "{:<22} whole window {} trades, PF {:.3} gross / {:.3} net; direction null p95 {:.3} gross / {:.3} net{}",
            "",
            row.whole.trades,
            row.whole.profit_factor,
            row.whole_net.profit_factor,
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.dir_null_gross, 0.95),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.dir_null_net, 0.95),
            if row.dir_self_managed { " (own sides permuted)" } else { "" },
        );
        // The same risk figure this mode also never printed, on both of the
        // two curves it scores: the walk-forward out-of-sample book and the
        // whole window the direction null is a percentile of. The rebate
        // credit does not change the drawdown of the GROSS book, which is what
        // these two are — `oos_net` is a credited copy and its own drawdown is
        // the gross one minus a credit that is never negative, so the gross
        // figure is the conservative one and the one printed.
        // `docs/decisions/2026-10-07-drawdown-printed.md`.
        println!("{:<22} walk-forward {}", "", drawdown_line(&row.oos));
        println!("{:<22} whole window {}", "", drawdown_line(&row.whole));
        println!("{:<22} walk-forward {}", "", lbar_line(&row.oos));
        println!("{:<22} whole window {}", "", lbar_line(&row.whole));
        println!(
            "{:<22} matched null p50 {:.3} / p95 {:.3} gross, p50 {:.3} / p95 {:.3} net over {} runs",
            "",
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_gross, 0.50),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_gross, 0.95),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_net, 0.50),
            fd_backtest::hypotheses::RescoreRow::null_quantile(&row.matched_null_net, 0.95),
            row.matched_null_net.len(),
        );
        // The achieved count match, measured and printed rather than assumed
        // — see `docs/decisions/2026-09-23-matched-null-repair.md`.
        println!(
            "{:<22} matched null {:.0} trades median vs the method's {} — count match {:.2}{}",
            "",
            fd_backtest::hypotheses::median_count(&row.matched_null_trades),
            row.oos.trades,
            row.count_match(),
            if row.count_matched() { "" } else { "  ** outside the band: this percentile is unmatched **" },
        );
        // THE COST AS A FRACTION OF R, which this mode never printed. Cost/R
        // is `spread / stop` and follows the HORIZON, not the instrument —
        // 0.67% of R at a 41.57-point stop against 14.0% at a 2.00-point stop
        // on the same instrument at the same spread. A rebate column read
        // without it is a credit of an unknown share of an unknown cost.
        // `rescore_hypothesis` has computed this control stop all along and
        // simply did not carry it out of the function.
        match &row.control_stop {
            Some(stop) => {
                println!(
                    "{:<22} cost-matched null: control stop {:.3} ATR = {:.2} points, cost {:.2}% of R ({})",
                    "",
                    stop.atr,
                    stop.points(),
                    100.0 * stop.cost_fraction_of_r(rules),
                    stop.source.as_str(),
                );
                println!(
                    "{:<22} the method's own realised stop: median {:.3} ATR = {:.2} points over {} trades",
                    "", stop.realised_atr, stop.realised_points, stop.measured_trades,
                );
            }
            // Not 0% and not absent: a self-managed hold has no stop, so its
            // control is the drift null and `cost/R` has no denominator here.
            None => println!(
                "{:<22} cost/R: NOT MEASURABLE on this row — it manages its own exits, so it has no stop to divide the spread by; its cost is the spread column above",
                "",
            ),
        }
        // How each position left, and the mean hold — the same line the
        // hypotheses branch prints, from the same `Metrics`.
        if exit_mix {
            let mix = if row.oos.exits.is_empty() {
                "no exits recorded".to_string()
            } else {
                row.oos.exits.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")
            };
            println!(
                "{:<22} exits (walk-forward, out of sample): {mix}; mean hold {:.1} min",
                "", row.oos.avg_hold_min,
            );
            let whole_mix = if row.whole.exits.is_empty() {
                "no exits recorded".to_string()
            } else {
                row.whole.exits.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")
            };
            println!(
                "{:<22} exits (whole window, what the direction null is a percentile OF): {whole_mix}; mean hold {:.1} min",
                "", row.whole.avg_hold_min,
            );
        }
        if row.passes_net() {
            passed.push(format!("{} ({})", row.label, if row.passes_gross() { "passed gross too" } else { "the credit moved it" }));
        }
    }

    println!();
    println!("ran {ran} of {} constructs in the batch; {} not run", batch.len(), not_run.len());
    for line in &not_run {
        println!("  not run — {line}");
    }
    println!();
    // The multiplicity statement, made by the record rather than left to a
    // reader: at the 95th percentile, one in twenty passes by luck, so a
    // batch of n expects n/20 passes from noise alone.
    let expected = batch.len() as f64 / 20.0;
    println!(
        "{} of {} passed all three legs; noise at the 95th percentile yields about {expected:.1} of {} by luck.",
        passed.len(),
        batch.len(),
        batch.len()
    );
    if passed.is_empty() {
        println!("Nothing crossed. The rebate lowers the bar and the bar was not the binding constraint.");
    } else {
        println!("Passed: {}", passed.join(", "));
        println!("A pass at or below the expected-by-chance count is indistinguishable from luck at this width.");
    }
    println!();
}

/// How much of the result is the market and how much is the spread.
///
/// Runs the same walk-forward at several fractions of the configured cost and
/// prints the curve. It answers one question and only one: is there a gross
/// edge that costs consume, or is there no edge to consume?
///
/// **The zero-cost column is not a result.** It is an upper bound that no one
/// can trade, printed so the distance between it and the real column is
/// visible. A method that only works at zero cost does not work.
#[allow(clippy::too_many_arguments)]
fn run_cost_sensitivity(
    registry: &Registry,
    bars: &[Bar],
    rules: &TradingRules,
    timeline: Option<&OptionsTimeline>,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) {
    const FRACTIONS: [f64; 5] = [0.0, 0.25, 0.5, 1.0, 2.0];

    println!("== cost sensitivity: walk-forward profit factor at N x the configured cost ==");
    println!("configured spread {}, commission {:.2}/lot
", rules.spread, rules.commission_per_lot);
    print!("{:<20}", "strategy");
    for fraction in FRACTIONS {
        print!("{:>10}", format!("{fraction}x"));
    }
    println!("{:>12}", "trades");

    for strategy in registry.all() {
        if strategy.needs_options() && timeline.is_none() {
            continue;
        }
        print!("{:<20}", strategy.id());
        let mut trades = 0usize;
        for fraction in FRACTIONS {
            let scaled = TradingRules {
                spread: rules.spread * fraction,
                commission_per_lot: rules.commission_per_lot * fraction,
                ..rules.clone()
            };
            match walk_forward_guarded(strategy.as_ref(), bars, &scaled, timeline, folds, select_by, min_trades_per_cell, guards)
            {
                Some(result) => {
                    if (fraction - 1.0).abs() < f64::EPSILON {
                        trades = result.oos.trades;
                    }
                    print!("{:>10.3}", result.oos.profit_factor);
                }
                None => print!("{:>10}", "-"),
            }
        }
        println!("{trades:>12}");
    }
    println!();
    println!("Read the gap, not the left column. A method whose profit factor only");
    println!("clears the gate at a cost nobody pays has not been shown to work.");
    println!();
}

/// Walk-forward over a strategy whose defaults have been overridden.
///
/// `walk_forward` builds its combinations from the strategy's own defaults, so
/// varying the seed means varying the defaults it starts from.
#[allow(clippy::too_many_arguments)]
fn walk_forward_seeded(
    strategy: &dyn fd_strategy::registry::Strategy,
    defaults: &fd_strategy::registry::Params,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    guards: Option<&Guards>,
) -> Option<fd_backtest::WalkForwardResult> {
    struct Seeded<'a> {
        inner: &'a dyn fd_strategy::registry::Strategy,
        defaults: fd_strategy::registry::Params,
    }
    impl fd_strategy::registry::Strategy for Seeded<'_> {
        fn id(&self) -> &'static str {
            self.inner.id()
        }
        fn name(&self) -> &'static str {
            self.inner.name()
        }
        fn description(&self) -> &'static str {
            self.inner.description()
        }
        fn default_params(&self) -> fd_strategy::registry::Params {
            self.defaults.clone()
        }
        fn grid(&self) -> std::collections::BTreeMap<String, Vec<f64>> {
            self.inner.grid()
        }
        fn indicators(&self, p: &fd_strategy::registry::Params) -> Vec<fd_indicators::IndicatorSpec> {
            self.inner.indicators(p)
        }
        fn warmup(&self, p: &fd_strategy::registry::Params) -> usize {
            self.inner.warmup(p)
        }
        fn series(&self, p: &fd_strategy::registry::Params) -> Vec<String> {
            self.inner.series(p)
        }
        fn needs_options(&self) -> bool {
            self.inner.needs_options()
        }
        fn on_bar(&self, ctx: &fd_strategy::registry::BarContext) -> fd_strategy::registry::Intent {
            self.inner.on_bar(ctx)
        }
    }
    let seeded = Seeded { inner: strategy, defaults: defaults.clone() };
    walk_forward_guarded(&seeded, bars, rules, None, folds, select_by, min_trades_per_cell, guards)
}

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

#[cfg(test)]
mod flag_tests {
    use super::{ALWAYS, BY_MODE, flag_audit};

    /// THIS BINARY'S OWN SOURCE.
    ///
    /// The warning line is the symptom; this is the cure. A flag can only be
    /// ignored in silence while the set of flags the code READS and the set it
    /// DECLARES are allowed to drift apart, so the test reads the first set out
    /// of the source text itself.
    const SOURCE: &str = include_str!("search.rs");

    /// The source WITHOUT this test module.
    ///
    /// The module below deliberately contains `--exitmix`, a flag that does
    /// not exist, as the fixture proving a misspelling is not silent. Scanning
    /// it would make the table's own test demand that the typo be declared.
    fn production_source() -> &'static str {
        let marker = concat!("\n#[cfg", "(test)]\nmod flag_tests");
        let at = SOURCE
            .find(marker)
            .expect("this test module's own header must be findable, or the scan reads its fixtures as flags");
        &SOURCE[..at]
    }

    fn declared(name: &str) -> bool {
        ALWAYS.contains(&name) || BY_MODE.iter().any(|(flag, _)| *flag == name)
    }

    fn leading_flag_name(rest: &str) -> String {
        rest.chars().take_while(|c| c.is_ascii_lowercase() || *c == '-').collect()
    }

    /// Every flag literal the source reads is in the table.
    ///
    /// Add `arg("widen-grid", ...)` or `a == "--widen-grid"` without saying
    /// which modes read it and this test fails. That is the whole mechanism:
    /// the fifth instance of this defect class could be written in silence,
    /// the sixth cannot.
    #[test]
    fn every_flag_the_source_reads_is_declared_in_the_table() {
        let mut found: Vec<String> = Vec::new();
        let src = production_source();
        for (at, _) in src.match_indices("arg(") {
            let rest = &src[at + 4..];
            if !rest.starts_with('"') {
                continue;
            }
            let name: String = rest[1..].chars().take_while(|c| *c != '"').collect();
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                found.push(name);
            }
        }
        let quote_dashdash = ['"', '-', '-'].iter().collect::<String>();
        for (at, _) in src.match_indices(quote_dashdash.as_str()) {
            let name = leading_flag_name(&src[at + 3..]);
            if !name.is_empty() {
                found.push(name);
            }
        }
        found.sort();
        found.dedup();
        assert!(
            found.len() >= 20,
            "the scanner found only {} flag literals in {} bytes of source; it has stopped seeing them and would pass vacuously: {found:?}",
            found.len(),
            src.len(),
        );
        let missing: Vec<&String> = found.iter().filter(|name| !declared(name)).collect();
        assert!(
            missing.is_empty(),
            "these flags are READ by search.rs and not DECLARED in ALWAYS/BY_MODE, so no receipt can say whether a mode ignored them: {missing:?}",
        );
    }

    /// And nothing is declared that the source does not read, or the table
    /// would promise a flag that does not exist.
    #[test]
    fn nothing_in_the_table_is_a_flag_the_source_never_reads() {
        for name in ALWAYS.iter().chain(BY_MODE.iter().map(|(flag, _)| flag)) {
            let as_arg = format!("arg(\"{name}\"");
            let as_long = format!("\"--{name}");
            assert!(
                production_source().contains(&as_arg) || production_source().contains(&as_long),
                "--{name} is declared in the table but no literal in search.rs reads it",
            );
        }
    }

    #[test]
    fn a_mode_that_does_not_read_a_flag_says_so_in_the_header() {
        let one = |mode: &str, flag: &str| flag_audit(mode, &[flag.to_string()]);

        // The two cases the brief names, which were silent until today.
        let hyp = one("hypotheses", "--direction-samples=1000");
        assert_eq!(hyp.len(), 1, "a mode that ignores --direction-samples= must say so: {hyp:?}");
        assert!(hyp[0].contains("--mode=rescore"), "and must name the mode that does read it: {hyp:?}");
        assert_eq!(one("hypotheses", "--samples=2000").len(), 1, "--samples= is read by null-dir only");
        assert_eq!(one("hypotheses", "--rebate-share=0.5").len(), 1, "--rebate-share= is read by rescore only");

        // The sixth case, found while fixing the fifth: the header prints
        // `null sides:` whatever the mode, and only `hypotheses` runs a null
        // that reads it.
        assert_eq!(one("rescore", "--null-sides=exposure").len(), 1, "rescore keeps the coin-flip null it published");

        // And the repair itself: rescore now reads --exit-mix, so passing it
        // there is silent BECAUSE IT WORKS.
        assert!(one("rescore", "--exit-mix").is_empty(), "--exit-mix is read by rescore as of 2026-10-07");
        assert!(one("hypotheses", "--exit-mix").is_empty(), "--exit-mix is read by hypotheses");

        // A flag every mode reads, and one that does not exist at all.
        assert!(one("rescore", "--spread=0.22").is_empty(), "--spread= reprices every mode");
        let typo = one("hypotheses", "--exitmix");
        assert_eq!(typo.len(), 1, "a misspelt flag must not be silent: {typo:?}");
        assert!(typo[0].contains("not a flag this binary has"), "{typo:?}");
        let bare = flag_audit("hypotheses", &["exit-mix".to_string()]);
        assert_eq!(bare.len(), 1, "an argument with no leading dashes is ignored and must say so: {bare:?}");
    }

    /// `--mode=all` runs compare, sweep and walk-forward and reads none of the
    /// per-mode flags. A receipt of `--mode=all --seeds=500` was a receipt at
    /// 200 seeds, and said nothing.
    #[test]
    fn mode_all_admits_it_reads_none_of_the_scoring_flags() {
        let lines = flag_audit("all", &["--seeds=500".to_string(), "--exit-mix".to_string(), "--market=xauduka".to_string()]);
        assert_eq!(lines.len(), 2, "--seeds= and --exit-mix are both ignored by --mode=all: {lines:?}");
        assert!(lines.iter().all(|l| l.contains("IS NOT READ BY --mode=all")), "{lines:?}");
    }
}

/// THE PROOF THAT THE NEW COLUMN MOVED NOTHING.
///
/// `instr-repair` proved its additive patch with `to_bits()`: `r_net == r` on
/// a costless row, so every published receipt still reads as it did. The
/// equivalent proof for a PRINTED column is byte equality against the printed
/// record itself — a table that gained a column, or lost a space, would fail
/// here even though every `f64` in the engine was untouched.
///
/// Both strings are compared against a receipt that is in the repository and
/// was produced before this branch existed.
#[cfg(test)]
mod drawdown_printer_tests {
    use super::{drawdown_line, gate_header, gate_row, lbar_line};
    use fd_backtest::engine::Metrics;

    /// A receipt from the record: `--mode=hypotheses` over `xauusd` 15m,
    /// 2025-09-13 to 2026-09-12, 200 seeds, committed 2026-09-13.
    fn published_receipt() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/research/runs/2026-09-13-recent-year-hours/in-sample.txt");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("the receipt this test reads is part of the record: {}: {e}", path.display()))
    }

    /// THE HEADER, byte for byte against the record. A drawdown column widened
    /// into this table would shift every figure on every row of every receipt;
    /// this is the assertion that it did not happen.
    #[test]
    fn the_table_header_is_byte_identical_to_the_published_record() {
        let receipt = published_receipt();
        let published = receipt
            .lines()
            .find(|l| l.starts_with("hypothesis "))
            .expect("the receipt has a table header");
        assert_eq!(gate_header(), published, "the header this build prints is not the header the record holds");
    }

    /// AND ONE ROW, re-formatted from its own published figures. `SURVIVES` at
    /// PF 1.481 on 205 trades: the row is quoted in three decision records, so
    /// its layout is load-bearing.
    #[test]
    fn a_published_row_reformats_byte_identically() {
        let receipt = published_receipt();
        let published = receipt
            .lines()
            .find(|l| l.starts_with("hold/04-06-short session-hold"))
            .expect("the receipt has this row");
        let rebuilt = gate_row("hold/04-06-short", "session-hold", 205, 1.481, 0.173, 0.963, 1.284, 0.0, "100%", "SURVIVES");
        assert_eq!(rebuilt, published, "the row this build prints is not the row the record holds");
    }

    /// The new line itself: it names the quantity, carries a unit, and says
    /// which of the two drawdowns it is.
    #[test]
    fn the_drawdown_line_states_its_unit_and_which_drawdown_it_is() {
        let m = Metrics { trades: 40, max_drawdown_usd: 812.5, max_drawdown_pct: 7.4318, avg_mae: -0.61, net_pnl_usd: 1234.5, ..Metrics::empty() };
        let line = drawdown_line(&m);
        assert!(line.contains("max drawdown 812.50 USD"), "{line}");
        assert!(line.contains("7.43% of the book's peak equity"), "{line}");
        assert!(line.contains("CLOSED-TRADE curve only"), "{line}");
        assert!(line.contains("avg_mae -0.610 R"), "the intrabar figure is named beside it: {line}");
    }

    /// AND IT REFUSES ON AN EMPTY CELL rather than printing the 0.0 that
    /// `Metrics::empty` carries, which is not a measurement of anything.
    #[test]
    fn an_empty_cell_gets_no_drawdown_number() {
        let line = drawdown_line(&Metrics::empty());
        assert!(line.contains("NOT MEASURED"), "{line}");
        assert!(line.contains("this is not 0.00 USD"), "{line}");
        assert!(!line.contains("max drawdown 0.00"), "an empty cell must not read as a book that never fell: {line}");
    }

    /// A zero drawdown on a real curve, however, IS printed as 0.00 USD: the
    /// fall was measured and it was zero. The two cases above and here are the
    /// difference between `null` and `0`.
    #[test]
    fn a_measured_zero_is_printed_as_zero() {
        let m = Metrics { trades: 7, max_drawdown_usd: 0.0, max_drawdown_pct: 0.0, avg_mae: -0.2, net_pnl_usd: 700.0, ..Metrics::empty() };
        let line = drawdown_line(&m);
        assert!(line.contains("max drawdown 0.00 USD = 0.00% of the book's peak equity"), "{line}");
        assert!(!line.contains("NOT MEASURED"), "{line}");
    }

    /// The Lbar line reproduces the gate identity from the two fields it is
    /// built out of, and it does so on numbers chosen so every part is
    /// checkable by hand: 40 trades, 50% wins, every win +0.60 R, every loss
    /// -0.40 R.  Lbar = 0.40 x 0.50 = 0.200 R, gross win per trade
    /// = 0.60 x 0.50 = 0.300 R, PF_r = 1.500, and the identity gives
    /// 0.200 x 0.500 = +0.100 R, which is the mean of +0.60 and -0.40.
    #[test]
    fn the_lbar_line_reproduces_the_gate_identity_from_measured_parts() {
        let m = Metrics {
            trades: 40,
            win_rate: 0.5,
            avg_win_r: 0.6,
            avg_loss_r: -0.4,
            expectancy: 0.1,
            profit_factor: 1.5,
            ..Metrics::empty()
        };
        let line = lbar_line(&m);
        assert!(line.contains("Lbar 0.2000 R"), "{line}");
        assert!(line.contains("PF_r 1.5000"), "{line}");
        assert!(line.contains("residual +0.00000 R"), "the identity must close on exact inputs: {line}");
        assert!(line.contains("expectancy leg BINDS (Lbar < 0.250R)"), "{line}");
    }

    /// The same row with the PF the engine actually prints — a ratio of
    /// `pnl_usd` — different from the R ratio: the line prints the gap rather
    /// than hiding it, because that gap is the unit mismatch itself.
    #[test]
    fn the_lbar_line_prints_the_usd_versus_r_gap_instead_of_hiding_it() {
        let m = Metrics { trades: 40, win_rate: 0.5, avg_win_r: 0.6, avg_loss_r: -0.4, expectancy: 0.1, profit_factor: 2.0, ..Metrics::empty() };
        let line = lbar_line(&m);
        assert!(line.contains("PF_r 1.5000 (R) vs PF_usd 2.0000 (USD, gap -0.5000)"), "{line}");
    }

    /// `Lbar` over an empty cell, and over a cell with no losing trade, is
    /// absent — not 0.000 R.  `null != 0` (brief section 8).
    #[test]
    fn an_lbar_with_no_losing_trade_is_absent_not_zero() {
        assert!(lbar_line(&Metrics::empty()).contains("NOT MEASURED"), "empty cell");
        let m = Metrics { trades: 12, win_rate: 1.0, avg_win_r: 0.3, avg_loss_r: f64::NAN, expectancy: 0.3, profit_factor: f64::INFINITY, ..Metrics::empty() };
        let line = lbar_line(&m);
        assert!(line.contains("NOT MEASURED"), "{line}");
        assert!(line.contains("this is not 0.000 R"), "{line}");
        assert!(!line.contains("Lbar 0.0000"), "{line}");
    }

    /// A row the gate's expectancy leg cannot bind on: `Lbar >= 0.250 R` is
    /// exactly the condition `agent/gate-legs` derived, and the line names it
    /// so a reader does not have to re-derive the threshold.
    #[test]
    fn an_lbar_at_or_above_the_threshold_says_the_expectancy_leg_is_redundant() {
        let m = Metrics { trades: 60, win_rate: 0.4, avg_win_r: 1.8, avg_loss_r: -0.8333, expectancy: 0.22, profit_factor: 1.44, ..Metrics::empty() };
        let line = lbar_line(&m);
        assert!(line.contains("expectancy leg REDUNDANT (Lbar >= 0.250R)"), "{line}");
    }
}
