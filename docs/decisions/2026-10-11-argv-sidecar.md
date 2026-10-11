# 2026-10-11 — the argv sidecar: a reproduction spec beside the receipt, not inside it

**Registered:** commit time is authoritative. Written and committed ALONE,
before the first line of code on this branch.
**Branch:** `agent/argv-sidecar`, worktree `E:/rust/fd-argv-sidecar`, cut from
`29c8635`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + addenda 5, 6, 7, 8, 9,
10, 11. The owner's decision this implements is addendum 11 §0.1: **argv goes
in a FILE BESIDE the receipt, not printed into it**, so the byte-identity
standard is untouched.
**Status:** registered.

## 0. What is already known, so this job does not re-measure it

`agent/receipt-reconcile` (`adbf0c1`) measured why 34 published decision-record
rows no longer re-run to their published number. Its own table
(`receipts/receipt-reconcile/TABLE-34-rows.txt`) is the input here, not an
opinion to re-derive:

| class | what moved | rows | exact return once pinned |
|---|---|---|---|
| W1 | store BACKFILLED under an omitted `--from` | 16 | 16 |
| W1+C1 | W1 **plus** Dukascopy flat-bar cleaning *inside* the window | 3 | 2 |
| W2 | store GREW at the live end under an omitted `--to` | 9 | 5 |
| P1 | a preset that did not pin a swept grid axis | 4 | 4 |
| G1 | `flat_before_weekend_hhmm` 1655 → 1640 (config, 2026-09-19) | 1 | 1 |
| E1 | one trade at a pinned window edge | 1 | 0 |

**The 25 the brief names are `W1 + W2 = 16 + 9`** — the rows whose *only*
defect is an empty `--from`/`--to` over a store that grew at one end. Their
return is **21 exact (16 + 5) and 4 off by exactly one trade**, not 25 exact;
the four are `close/*` on a window that matched **bar for bar**. `0/34` are a
unit mistake and `0/34` are unreadable. The infection is at the
**specification** layer.

**One number in that record disagrees with itself, and the measurement wins
(§8).** `TABLE-34-rows.txt`'s header line reads *"23 of 34 return their
PUBLISHED TRADE COUNT exactly … 15 of those also return the published PF"*,
while the same file's own class table sums to **28** (16 + 2 + 5 + 4 + 1 + 0)
and the result record states 28 with **8 of the 28** drifting on PF, i.e. 20
returning it. The header is stale against its own body. **28/34 and 20/28 are
the numbers used here.**

So the sidecar must carry **more than raw argv**, and the four classes say
exactly what:

- **W1 / W2 / E1 (26 rows)** → the **resolved** window: first bar, last bar,
  **bar count**. A date pin is not enough on its own; the count is what lets a
  reader *detect* that the tape under a pinned window changed. The 4 off-by-one
  `close/*` rows are the proof that a date pin alone is insufficient: their
  window matched bar for bar and a trade still moved.
- **W1+C1 (1 row of the 3 whose trade set genuinely moved)** → a **fingerprint
  of the bars actually fed to the engine**. A date pin cannot fix bars rewritten
  *inside* the window; nothing can, after the fact. A fingerprint can make it
  **detectable and named**, which is the honest ceiling.
- **P1 (4 rows)** → the batch file by **path and content hash**. The evidence is
  in the published receipt itself: `vwap/allday` declares `stretchAtr = 1.5` and
  `vwap/wide` declares `2.5` and both print **5,828 trades / PF 0.951 /
  −0.029R, digit for digit**, because the grid overwrote the preset. The engine
  writes this defect down and names the file itself at
  `crates/fd-backtest/src/hypotheses.rs:1347-1351`. A hash of the batch file
  pins which text produced the row; it does not pin which axis won, so the
  resolved parameters per row are recorded too where the engine already has
  them.
- **G1 (1 row)** → the **config**. One value moved and the row went from 336
  trades to 0.

## 1. Hypothesis — one sentence

A file written beside a receipt, carrying verbatim argv + the resolved window
(first bar / last bar / bar count) + the batch file's path and hash + a
fingerprint of the bars fed to the engine + the resolved effective config +
the binary's commit, is a **sufficient reproduction spec** for a row — meaning
a run rebuilt from that file alone returns the row's trade count — and writing
it changes **no byte** of the receipt.

## 2. How config is recorded, and why (the decision §1 of the task asks for)

Two candidates were on the table: hash the whole config file, or hash only the
keys the engine reads.

**Rejected: a list of the keys the engine reads.** That is the same shape as the
defect the flag-scan test in `search.rs` exists to prevent — a set the code
READS and a set it DECLARES, free to drift apart. A key list would go stale
silently the first time a new field is added, and the sidecar would then be
*confidently wrong* rather than noisy.

**Chosen: both halves of a different pair.**

1. **`sha256` of every file `Config::load` actually opens** — `default.toml`,
   and `local.toml` when it exists — each named with its own digest. This is
   **over-sensitive**: a comment edit changes it. Over-sensitive is the safe
   direction. It can raise a false alarm; it cannot miss G1.
2. **The resolved effective values, serialised from the live structs** —
   `TradingRules` and, when `--guards` is passed, `Guards`. These are printed
   *from the objects the engine is about to use*, after `--spread=` / `--trail=`
   / per-market overrides have been applied. **This cannot drift**: a field
   added to either struct appears in the sidecar without anyone remembering to
   add it. It is also what a reader actually wants — `flat_before_weekend_hhmm
   = 1655` is the fact that explains G1, and the file hash alone would only say
   "something changed".

A digest is recorded for `local.toml` and **its content is never read into the
sidecar**, so a secrets file is fingerprinted without being quoted.

**What this does NOT cover, stated now so the result cannot claim it:** config
files outside `Config::load`'s two (`accounts.toml`, `exclusions.toml`) are not
hashed, because this run never opens them.

## 3. The bar fingerprint, and why not the parquet file

**Rejected: `sha256` of the parquet file.** It is the wrong object twice — a
re-write with different compression or metadata changes the digest while the
bars are identical (false alarm), and `XAUDUKA-1m.parquet` is 100 MB, so every
run would pay for a hash of bars it mostly discarded.

**Chosen: `sha256` over the bars the engine was handed**, in order, each as
`(time, open, high, low, close, volume)` little-endian. That is a fingerprint of
**the exact input**, it costs the window not the file, and it is the only thing
that can catch W1+C1 — bars rewritten inside a pinned window. File path, byte
size and mtime are recorded alongside it as provenance.

## 4. Falsifier — specific, and it fires on a number

**F1 (the task's falsifier).** Take **at least 3** of the 25 window-class rows.
For each: run the published verbatim argv (empty `--from`), then run it with the
window pinned, then **rebuild the argv from the sidecar file alone** and run
that. F1 FIRES if the sidecar-rebuilt run's trade count does not equal the
published trade count on any of them. `receipt-reconcile` already measured that
all 16 W1 rows return exactly when pinned, so **this test has a known answer and
is the positive control**: if my sidecar cannot reach a number another agent
already reached by hand, the sidecar is the thing that is broken.

**F2 (the standard the owner chose the sidecar to protect).** Five argv, run
against (a) the shared `29c8635` binary, (b) an independent `--release` build of
`29c8635` in this worktree's own target dir, (c) the post-change binary with no
`--argv-sidecar=`, (d) the post-change binary **with** `--argv-sidecar=`. F2
FIRES if `diff -r` between any of the four is not byte-clean. If it fires on a
last digit, the answer is to **find the cause**, not to relax the standard to
"approximately" — `range-sizing` hit a case where BLAS GEMM changed addition
order with batch row count and fixed the instrument instead of the bar.

**F3 (the blind-probe, required by addendum 11 §VIII).** A probe that
deliberately *does* print one extra line to stdout must make F2's comparison
FIRE. Without that, an F2 pass could be a blind harness.

**F4 (the silent-flag class).** The sidecar must not be settable from a path
that accepts it and does nothing. Both paths are measured: the CLI flag, and
`[run] argv_sidecar = …` inside a batch file — the `null_sides` / `max_hold_ms`
class. The result will name which path is inert. And `search.rs`'s own
source-scanning test must FIRE before the flag is declared in `ALWAYS`/`BY_MODE`
— the code is written first, the suite is run, the failure is read, and only
then is the table filled in.

## 5. Multi-test ledger — declared BEFORE any run

| block | runs declared |
|---|---|
| F2 byte-identity: 5 argv × 4 binaries | 20 |
| F1 positive control: 4 window-class rows × 3 (verbatim / pinned / sidecar-rebuilt) | 12 |
| F3 blind-probe: 5 argv × 1 deliberately-looking build | 5 |
| F4 inert-path probe: `[run] argv_sidecar` in a batch file | 1 |
| **declared total** | **38** |

**Gate cells: 0.** This axis measures an instrument, not a method. No row here
is a candidate, no percentile is published, and nothing is spent against
`PF_r ≥ 1.200 ∧ E ≥ +0.050R ∧ n ≥ 40`.

## 6. How to read the result

- **F1 reports trade counts, nothing else.** The sidecar is a specification
  tool; whether a row passes a gate is irrelevant to whether it reproduces. PF
  and expectancy are quoted only where they also return, and never as a reading
  of the method.
- **"Sufficient" has a boundary and the result must state it.** The sidecar can
  only cover **runs made with it from here on**. It is not retroactive. The
  result will state, as a counted number, how much of the past it does **not**
  fix — the ceiling being `agent/pfr-audit`'s **4,342 band rows with no argv at
  all**, and the **17 of 51 vanished band keys that sit only in receipts and
  not in `docs/decisions/`**, which `receipt-reconcile` left out of scope and
  **nobody has measured**. Unmeasured is `null`, not "fine".
- **No existing receipt is rewritten.** Any correction to the record is a dated
  note **appended**, 0 lines deleted — the standard `receipt-reconcile` held
  across all three of its commits.

## 7. Amendments

Amendments go here, dated, appended. No line above is rewritten.
