//! The argv sidecar: a reproduction spec written **beside** a receipt.
//!
//! `agent/receipt-reconcile` measured why 34 published decision-record rows no
//! longer re-run to their published number. None of the 34 was a unit mistake
//! and none was unreadable; **25 of them** (classes W1 and W2) were
//! unreproducible for one reason — an argv recorded with an empty `--from` or
//! `--to` over a store that later grew at one end. `XAUDUKA-5m` went 198,716 →
//! 1,135,389 bars (×5.31) and the trade counts moved by that factor (one row
//! 578 → 3,078 = ×5.33). The record is infected at the **specification**
//! layer, not at the measurement layer and not at the reporting layer.
//!
//! A receipt prints its resolved window; it does not print the argv that asked
//! for it. Printing one into the receipt is exactly what the bit-for-bit
//! standard forbids, and those two requirements conflict directly — which is
//! what `agent/hold-cap` escalated. The owner decided it on 2026-10-11: **the
//! argv goes in a separate file beside the receipt.** This module writes that
//! file. Nothing here reaches stdout, so a receipt is byte-for-byte what it was
//! before this file existed.
//!
//! Raw argv is not enough, and the four defect classes say exactly what else is
//! needed. Each section of the rendered file names the class it closes:
//!
//! | class | rows | what the sidecar carries |
//! |---|---|---|
//! | W1 / W2 / E1 | 26 | the **resolved** window — first bar, last bar, bar count |
//! | W1+C1 | 3 | a digest of the bars handed to the engine (bars rewritten *inside* a pinned window) |
//! | P1 | 4 | the batch file's path and content hash |
//! | G1 | 1 | the config — file digests **and** the resolved effective values |
//!
//! Registration: `docs/decisions/2026-10-11-argv-sidecar.md`, which also
//! records why the config is hashed whole rather than by a list of the keys the
//! engine reads (a key list is the same shape as the defect `search.rs`'s
//! source-scanning flag test exists to prevent: a set the code READS and a set
//! it DECLARES, free to drift apart in silence), and why the bar digest covers
//! the bars fed to the engine rather than the parquet file (a re-write with
//! different compression leaves the bars identical, and `XAUDUKA-1m.parquet` is
//! 100 MB of which a run uses a slice).

use std::fmt::Write as _;
use std::path::Path;

use fd_core::types::Bar;

use crate::engine::TradingRules;
use crate::guards::Guards;

/// Everything the sidecar states about one run, collected at the call site
/// after the window is resolved and before a single number is measured.
pub struct RunSpec<'a> {
    /// `std::env::args()`, verbatim, including argv[0].
    pub argv: &'a [String],
    /// The bars the engine will be handed, after `--from`/`--to`.
    pub bars: &'a [Bar],
    /// How many bars the file held before the bounds cut it.
    pub bars_before_bounds: usize,
    /// The parquet the bars were read from.
    pub bars_path: &'a Path,
    /// `--from=` / `--to=` as passed, empty string when omitted.
    pub from_arg: &'a str,
    pub to_arg: &'a str,
    /// `--config=` as passed.
    pub config_dir: &'a Path,
    /// `--batch-file=` as passed, empty when omitted.
    pub batch_file: &'a str,
    /// `--batch=` — the built-in batch, used when `--batch-file=` is empty.
    pub batch_name: &'a str,
    /// Whether `--mode=` reads a batch at all.
    pub mode_reads_batch: bool,
    /// The costs and sizing the engine is about to use, after `--spread=`,
    /// `--trail=` and any per-market override.
    pub rules: &'a TradingRules,
    /// The installed guards, or `None` when `--guards` was not passed. `None`
    /// is **not** "every guard at zero" and is not rendered as one.
    pub guards: Option<&'a Guards>,
}

/// `sha256`, hex, lowercase.
fn sha256_hex(bytes: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, bytes);
    d.as_ref().iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `sha256` over the bars **the engine was handed**, in order, each field
/// little-endian.
///
/// This is the fingerprint of the exact input. A date pin cannot repair bars
/// rewritten inside the window it pins — `orb/60m-expansion` is the one row of
/// the 34 whose trade set genuinely moved that way, 153 → 137, after the
/// Dukascopy flat-bar cleaning — and nothing can repair it after the fact. This
/// makes it **detectable and named**, which is the honest ceiling.
///
/// `volume` is `Option<f64>`: a missing volume hashes as a distinct tag byte
/// rather than as `0.0`, because `GC-1m` publishes `None` on all 16,903 rows
/// while `XAUDUKA-15m` publishes a literal `0.0` manufactured by a `null` → `0`
/// step in the pipeline, and a digest that could not tell those apart would
/// erase the distinction addendum 9 §III was written to preserve.
#[must_use]
pub fn bar_digest(bars: &[Bar]) -> String {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    for b in bars {
        ctx.update(&b.time.to_le_bytes());
        ctx.update(&b.open.to_le_bytes());
        ctx.update(&b.high.to_le_bytes());
        ctx.update(&b.low.to_le_bytes());
        ctx.update(&b.close.to_le_bytes());
        match b.volume {
            Some(v) => {
                ctx.update(&[1u8]);
                ctx.update(&v.to_le_bytes());
            }
            None => ctx.update(&[0u8]),
        }
    }
    ctx.finish().as_ref().iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// `YYYY-MM-DD HH:MM` from epoch milliseconds, UTC.
fn stamp(ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(DAY_MS));
    let rest = ms.rem_euclid(DAY_MS);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 3_600_000, (rest / 60_000) % 60)
}

/// `YYYY-MM-DD` from epoch milliseconds, UTC.
fn day(ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(DAY_MS));
    format!("{y:04}-{m:02}-{d:02}")
}

/// A file's digest, byte size and mtime, or why there is none.
fn file_line(path: &Path) -> String {
    match std::fs::read(path) {
        Err(e) => format!("MISSING — {e}"),
        Ok(bytes) => {
            let mtime = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or_else(|| "mtime null".to_string(), |d| format!("mtime {} UTC", stamp(d.as_millis() as i64)));
            format!("sha256 {}  ({} bytes, {mtime})", sha256_hex(&bytes), bytes.len())
        }
    }
}

/// The commit this binary was built from, captured by `build.rs`.
///
/// `agent/hold-cap` measured **two independent `--release` builds of one commit
/// producing byte-identical output**, so the reproduction base is the *commit*,
/// not a particular `search.exe`. A sidecar naming a binary path would name the
/// wrong thing. When `build.rs` could not reach `git` this is `null` — never a
/// placeholder hash.
fn commit_line() -> String {
    match (option_env!("FD_GIT_COMMIT"), option_env!("FD_GIT_DIRTY")) {
        (Some(c), Some(state)) => format!("{c}  (working tree {state} at build time)"),
        (Some(c), None) => format!("{c}  (dirty state null)"),
        _ => "null — build.rs could not reach git, so this binary cannot name its commit".to_string(),
    }
}

/// The replay argv: the run's own argv with both window ends pinned to the
/// window it **resolved**.
///
/// `--to` is exclusive and day-granular (`search.rs`'s `bound()` multiplies a
/// civil date by 86,400,000), so the pin is the day **after** the last bar.
fn replay_argv(spec: &RunSpec<'_>, first: i64, last: i64) -> Vec<String> {
    const DAY_MS: i64 = 86_400_000;
    let mut out: Vec<String> = spec
        .argv
        .iter()
        .filter(|a| !a.starts_with("--from=") && !a.starts_with("--to=") && !a.starts_with("--argv-sidecar="))
        .cloned()
        .collect();
    out.push(format!("--from={}", day(first)));
    out.push(format!("--to={}", day(last + DAY_MS)));
    out
}

/// Render the sidecar. Pure: no I/O beyond reading the files it hashes.
#[must_use]
pub fn render(spec: &RunSpec<'_>, receipt_stem: &str) -> String {
    let mut s = String::with_capacity(4096);
    let _ = writeln!(s, "# argv sidecar — {receipt_stem}");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "A receipt prints the window it resolved but not the argv that asked for it, and
25 of the record's 34 unreproducible rows come from an argv recorded with an
empty `--from`/`--to`. This file is that argv, plus the four things an argv
alone does not pin. **Nothing here is printed into the receipt**: the receipt
beside this file is byte-for-byte what it was before this file existed, which is
why the owner chose a sidecar over a header line.

Written by `search.exe`. Registration: `docs/decisions/2026-10-11-argv-sidecar.md`.
Classes referenced below are those of `docs/decisions/2026-10-11-receipt-reconcile-result.md`."
    );
    let _ = writeln!(s);

    // -------------------------------------------------- 1. argv, verbatim
    let _ = writeln!(s, "## 1. argv, verbatim");
    let _ = writeln!(s);
    for (i, a) in spec.argv.iter().enumerate() {
        let _ = writeln!(s, "    argv[{i}] = {a}");
    }
    let _ = writeln!(s);

    // -------------------------------------------- 2. the resolved window
    let _ = writeln!(s, "## 2. The window this run RESOLVED  (classes W1 / W2 / E1 — 26 rows)");
    let _ = writeln!(s);
    match (spec.bars.first(), spec.bars.last()) {
        (Some(f), Some(l)) => {
            let _ = writeln!(s, "    bars:         {} of {} in the file", spec.bars.len(), spec.bars_before_bounds);
            let _ = writeln!(s, "    first bar:    {} UTC   ({} ms)", stamp(f.time), f.time);
            let _ = writeln!(s, "    last bar:     {} UTC   ({} ms)", stamp(l.time), l.time);
            let _ = writeln!(
                s,
                "    passed:       --from={} --to={}",
                if spec.from_arg.is_empty() { "<EMPTY — THE DEFECT THIS FILE EXISTS FOR>" } else { spec.from_arg },
                if spec.to_arg.is_empty() { "<EMPTY — THE DEFECT THIS FILE EXISTS FOR>" } else { spec.to_arg },
            );
            let _ = writeln!(s, "    bar file:     {}", spec.bars_path.display());
            let _ = writeln!(s, "                  {}", file_line(spec.bars_path));
            let _ = writeln!(s, "    bar digest:   sha256 {}", bar_digest(spec.bars));
            let _ = writeln!(
                s,
                "                  over the {} bars handed to the engine, in order,
                  (time, open, high, low, close, volume) little-endian, a tag
                  byte distinguishing a null volume from a literal 0.0.
                  This is the fingerprint of the exact input (class W1+C1: bars
                  rewritten INSIDE a pinned window). The parquet file is not
                  hashed — see the module doc.",
                spec.bars.len()
            );
            let _ = writeln!(s);
            let _ = writeln!(s, "## 3. replay — reproduces section 2 from THIS FILE ALONE");
            let _ = writeln!(s);
            for a in replay_argv(spec, f.time, l.time) {
                let _ = writeln!(s, "      {a}");
            }
            let _ = writeln!(s);
            let _ = writeln!(
                s,
                "    `--to` is exclusive and day-granular, so it is the day AFTER the last
    bar. If a replay prints a different `bars:` or a different bar digest, the
    store moved under a pinned window — this file makes that DETECTABLE and
    cannot make it reproducible."
            );
        }
        _ => {
            let _ = writeln!(s, "    bars:         0 of {} — the run took no bars", spec.bars_before_bounds);
            let _ = writeln!(s, "    first bar:    null");
            let _ = writeln!(s, "    last bar:     null");
            let _ = writeln!(s, "    bar digest:   null");
            let _ = writeln!(s);
            let _ = writeln!(s, "## 3. replay");
            let _ = writeln!(s);
            let _ = writeln!(s, "    null — a window with no bars cannot be pinned to its own contents.");
        }
    }
    let _ = writeln!(s);

    // ------------------------------------------------------ 4. batch file
    let _ = writeln!(s, "## 4. Batch file  (class P1 — 4 rows)");
    let _ = writeln!(s);
    if !spec.mode_reads_batch {
        let _ = writeln!(s, "    null — --mode= does not read a batch, so none entered this run.");
    } else if spec.batch_file.is_empty() {
        let _ = writeln!(s, "    path:    null — no --batch-file= was passed");
        let _ = writeln!(s, "    builtin: --batch={}  (compiled in; its text is this commit)", spec.batch_name);
    } else {
        let p = Path::new(spec.batch_file);
        let _ = writeln!(s, "    path:    {}", p.display());
        let _ = writeln!(s, "    {}", file_line(p));
        let _ = writeln!(
            s,
            "
    The hash pins WHICH TEXT produced the rows. It does not pin which axis of a
    sweep won: `vwap/allday` declared stretchAtr 1.5 and `vwap/wide` declared
    2.5 and both printed 5,828 trades / PF 0.951 / -0.029R digit for digit,
    because the grid overwrote the preset. The engine denounces that itself and
    names the file — `hypotheses.rs:1347-1351` — so the receipt beside this
    file carries it."
        );
    }
    let _ = writeln!(s);

    // ---------------------------------------------------------- 5. config
    let _ = writeln!(s, "## 5. Config  (class G1 — 1 row)");
    let _ = writeln!(s);
    let _ = writeln!(s, "    --config={}", spec.config_dir.display());
    let default_toml = spec.config_dir.join("default.toml");
    let _ = writeln!(s, "    default.toml   {}", file_line(&default_toml));
    let local_toml = spec.config_dir.join("local.toml");
    if local_toml.exists() {
        let _ = writeln!(s, "    local.toml     {}", file_line(&local_toml));
        let _ = writeln!(s, "                   (digest only; its contents are never copied here)");
    } else {
        let _ = writeln!(s, "    local.toml     absent");
    }
    let _ = writeln!(
        s,
        "
    Only the files `Config::load` actually opens are hashed. `accounts.toml`
    and `exclusions.toml` sit in the same directory and this run never opened
    them, so they are NOT hashed and this file does not pretend otherwise.

    A whole-file digest is OVER-sensitive: a comment edit moves it. That is the
    safe direction — it can raise a false alarm, it cannot miss G1
    (`flat_before_weekend_hhmm` 1655 -> 1640 on 2026-09-19 took one row from
    336 trades to 0). A list of the keys the engine reads was rejected: it is
    the same shape as the defect the source-scanning flag test in `search.rs`
    exists to prevent."
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "### 5b. The resolved effective values, serialised from the live structs");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "    Printed from the objects the engine is about to use, after --spread=,
    --trail= and any per-market override. This CANNOT drift: a field added to
    either struct appears here without anyone remembering to add it."
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "    [trading_rules]");
    for line in dotted(spec.rules) {
        let _ = writeln!(s, "    {line}");
    }
    let _ = writeln!(s);
    match spec.guards {
        None => {
            let _ = writeln!(
                s,
                "    [guards] = null — --guards was not passed, so no guard was installed.
    That is NOT every guard at zero: an unguarded run has no guard object, and
    `sized_down_by_guard` on such a row is null, not 0."
            );
        }
        Some(g) => {
            let _ = writeln!(s, "    [guards]");
            for line in dotted(g) {
                let _ = writeln!(s, "    {line}");
            }
        }
    }
    let _ = writeln!(s);

    // ---------------------------------------------------------- 6. binary
    let _ = writeln!(s, "## 6. Binary and run");
    let _ = writeln!(s);
    let _ = writeln!(s, "    commit:    {}", commit_line());
    let _ = writeln!(
        s,
        "               Captured by build.rs at BUILD time, which is what it is: if
               the only edit since the build was outside this crate, the
               `dirty` word can be stale. The commit is the reproduction base
               because two independent --release builds of one commit were
               measured producing byte-identical output (agent/hold-cap)."
    );
    let run_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(None, |d| Some(d.as_millis() as i64));
    match run_ms {
        Some(ms) => {
            let _ = writeln!(s, "    run date:  {} UTC", stamp(ms));
        }
        None => {
            let _ = writeln!(s, "    run date:  null — the system clock is before the epoch");
        }
    }
    s
}

/// Every field of a `Serialize` struct as one `dotted.key = value` line.
///
/// Fully flattened on purpose: a nested table in the middle of a struct's
/// fields makes a TOML document serialiser refuse (`trail` sits between
/// `max_hold_ms` and `lot_step` in `TradingRules`), and dotted keys read the
/// same whatever order the fields arrive in. The point of going through
/// `Serialize` rather than naming fields is that it **cannot drift**: add a
/// field to either struct and the sidecar carries it.
fn dotted<T: serde::Serialize>(value: &T) -> Vec<String> {
    let mut out = Vec::new();
    match toml::Value::try_from(value) {
        Err(e) => out.push(format!("# this struct could not be serialised: {e}")),
        Ok(v) => walk("", &v, &mut out),
    }
    out
}

fn walk(prefix: &str, v: &toml::Value, out: &mut Vec<String>) {
    match v {
        toml::Value::Table(t) => {
            for (k, vv) in t {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                walk(&key, vv, out);
            }
        }
        other => out.push(format!("{prefix} = {}", scalar(other))),
    }
}

fn scalar(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => format!("{s:?}"),
        toml::Value::Integer(i) => i.to_string(),
        // `{:?}` keeps a whole float's `.0`, so `1` and `1.0` do not read alike.
        toml::Value::Float(f) => format!("{f:?}"),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => d.to_string(),
        toml::Value::Array(a) => format!("[{}]", a.iter().map(scalar).collect::<Vec<_>>().join(", ")),
        // A table inside an array is the one shape this renderer does not
        // reach; neither struct has one, and saying so beats inventing a line.
        toml::Value::Table(_) => "{ a nested table — not rendered }".to_string(),
    }
}

/// Write the sidecar. The caller treats an error as fatal: a sidecar that
/// failed in silence is the `null_sides` defect class with a new name.
pub fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(time: i64, close: f64, volume: Option<f64>) -> Bar {
        Bar { time, open: close - 1.0, high: close + 1.0, low: close - 2.0, close, volume }
    }

    fn spec_of<'a>(bars: &'a [Bar], argv: &'a [String], rules: &'a TradingRules) -> RunSpec<'a> {
        RunSpec {
            argv,
            bars,
            bars_before_bounds: bars.len() + 7,
            bars_path: Path::new("data/bars/XAUDUKA-15m.parquet"),
            from_arg: "",
            to_arg: "",
            config_dir: Path::new("config"),
            batch_file: "",
            batch_name: "gold-intraday",
            mode_reads_batch: true,
            rules,
            guards: None,
        }
    }

    /// A NULL volume and a LITERAL 0.0 volume are different data, and the
    /// digest says so.
    ///
    /// Addendum 9 §III: the provider published nothing on `XAUDUKA-1m`
    /// (`null` on 5,635,777 rows) and the pipeline manufactured a `0.0` on the
    /// resampled series. A fingerprint that could not tell those apart would
    /// erase the distinction.
    #[test]
    fn a_null_volume_does_not_hash_as_zero() {
        let none = [bar(0, 100.0, None)];
        let zero = [bar(0, 100.0, Some(0.0))];
        assert_ne!(bar_digest(&none), bar_digest(&zero));
    }

    /// The digest is of the bars, in order.
    #[test]
    fn the_digest_sees_order_and_content() {
        let a = [bar(0, 100.0, Some(1.0)), bar(900_000, 101.0, Some(1.0))];
        let b = [bar(900_000, 101.0, Some(1.0)), bar(0, 100.0, Some(1.0))];
        let c = [bar(0, 100.0, Some(1.0)), bar(900_000, 101.5, Some(1.0))];
        assert_ne!(bar_digest(&a), bar_digest(&b), "order must be visible");
        assert_ne!(bar_digest(&a), bar_digest(&c), "a price change must be visible");
        assert_eq!(bar_digest(&a), bar_digest(&a.to_vec()), "and the same bars must agree with themselves");
    }

    /// THE DEFECT, DIRECTLY: an empty `--from` resolves to a window the argv
    /// does not state, and the replay line states it.
    ///
    /// This is the 25-row class in miniature. The argv says nothing; the
    /// sidecar's replay pins both ends to the bars the run actually saw, and
    /// `--to` is the day AFTER the last bar because `bound()` is exclusive.
    #[test]
    fn the_replay_line_pins_a_window_the_argv_left_empty() {
        let bars = [bar(1_527_811_200_000, 1300.0, Some(1.0)), bar(1_780_271_100_000, 3300.0, Some(1.0))];
        let argv: Vec<String> = ["search.exe", "--market=xauduka", "--mode=hypotheses", "--argv-sidecar=x.md"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let rules = TradingRules::default();
        let spec = spec_of(&bars, &argv, &rules);
        let replay = replay_argv(&spec, bars[0].time, bars[1].time);
        assert!(replay.contains(&"--from=2018-06-01".to_string()), "{replay:?}");
        assert!(replay.contains(&"--to=2026-06-01".to_string()), "exclusive, so the day after: {replay:?}");
        assert!(
            !replay.iter().any(|a| a.starts_with("--argv-sidecar=")),
            "a replay must not re-write the sidecar it came from: {replay:?}"
        );
        let text = render(&spec, "unit-test");
        assert!(text.contains("<EMPTY — THE DEFECT THIS FILE EXISTS FOR>"), "an empty bound must be denounced");
        assert!(text.contains("--from=2018-06-01"), "and the resolved window must be in the file");
    }

    /// An unguarded run has NO guard object, and the sidecar says `null`.
    ///
    /// `null != 0 != []`. Rendering eleven zeroes would claim a measurement
    /// that was never made, and `sized_down_by_guard` on an unguarded row is
    /// null for the same reason.
    #[test]
    fn guards_off_renders_null_and_not_eleven_zeroes() {
        let bars = [bar(0, 100.0, Some(1.0))];
        let argv = vec!["search.exe".to_string()];
        let rules = TradingRules::default();
        let text = render(&spec_of(&bars, &argv, &rules), "unit-test");
        assert!(text.contains("[guards] = null"), "guards off must render null");
        assert!(!text.contains("max_concurrent_positions"), "and must not render a guard table: {text}");
    }

    /// The resolved values come from the live struct, so a `--spread=`
    /// override is in the file — class G1 is a config value that moved.
    #[test]
    fn the_resolved_spread_is_what_the_engine_will_use() {
        let bars = [bar(0, 100.0, Some(1.0))];
        let argv = vec!["search.exe".to_string(), "--spread=0.220".to_string()];
        let mut rules = TradingRules::default();
        rules.spread = 0.220;
        let text = render(&spec_of(&bars, &argv, &rules), "unit-test");
        assert!(text.contains("spread = 0.22"), "the resolved spread must be in the file: {text}");
    }

    /// A window with no bars is `null`, never a zero-length window that looks
    /// pinned.
    #[test]
    fn an_empty_window_is_null_not_a_pinned_one() {
        let argv = vec!["search.exe".to_string()];
        let rules = TradingRules::default();
        let text = render(&spec_of(&[], &argv, &rules), "unit-test");
        assert!(text.contains("bar digest:   null"), "{text}");
        assert!(text.contains("a window with no bars cannot be pinned"), "{text}");
    }
}
