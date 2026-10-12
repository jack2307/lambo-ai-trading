# 2026-10-11 — RESULT: the argv sidecar. 14 of 14 rows reproduce from the file alone, and the receipt did not move one byte

**Branch:** `agent/argv-sidecar`, cut from `29c8635`.
**Registration:** `docs/decisions/2026-10-11-argv-sidecar.md`, committed alone
before the first line of code.
**Receipts:** `receipts/2026-10-11-argv-sidecar/` — `ARGV.md` (every argv),
`F1-POSITIVE-CONTROL.txt`, `F2-byte-identity.txt`, `F3-BLIND-PROBE.txt`,
`F4-INERT-PATH.txt`, `PAST-COVERAGE.txt`, four example sidecars.
**Gate: 0 cells.** This axis measures an instrument. No row below is a
candidate, no percentile and no verdict word is read from any run in it.

## 0. What was built

`--argv-sidecar=<path>` on `search.exe` writes **one file beside the receipt**
carrying the run's reproduction spec. The owner's decision (addendum 11 §0.1)
was a separate file rather than a header line, and that decision is the whole
design constraint: **on success the flag writes zero bytes to stdout and zero to
stderr.**

The file carries six sections, and each closes a measured defect class of
`2026-10-11-receipt-reconcile-result.md`:

| § | content | class it closes |
|---|---|---|
| 1 | argv, verbatim, one token per line | — |
| 2 | the **resolved** window: first bar, last bar, bar count, and an empty `--from`/`--to` **denounced in words** | W1 / W2 / E1, 26 rows |
| 2 | `sha256` over the bars handed to the engine, `(t,o,h,l,c,v)` LE, with a tag byte so a `null` volume cannot hash as a literal `0.0` | W1+C1 |
| 3 | **`replay`** — the same argv with both ends pinned to the window it resolved | the mechanism |
| 4 | the batch file's path, `sha256`, byte size, mtime | P1, 4 rows |
| 5 | `sha256` of each file `Config::load` opened, **plus** the resolved `TradingRules` / `Guards` serialised from the live structs | G1, 1 row |
| 6 | the commit `build.rs` captured, and the run date | — |

Also wired: **`scripts/research-run.py` now passes `--argv-sidecar` for every
receipt it writes**, as `<receipt>.argv.md`. That runner is not a bystander —
its `bounds()` emits nothing at all when a stage has no `*_from` in its `[run]`
table, which is precisely how all four of the published commands in §2 below
came to carry `--to=` and no `--from=`.

## 1. F1 — the falsifier the task set. IT DOES NOT FIRE. 14 of 14.

Four published commands behind **14** published decision-record rows, all from
the W1 class, all `xauduka 5m`. Every one of the four, read out of its own
`docs/research/runs/<id>/in-sample.txt` `$ …` echo, is
`--seeds=200 --to=2025-04-10` with **no `--from=`**.

| phase | what it is | returns the published trade count |
|---|---|---|
| **A** | the published argv **verbatim** | **0 / 14** |
| **B** | both window ends pinned to `2022-06-16 … 2025-04-10` | **14 / 14** |
| **C** | argv rebuilt **from phase B's sidecar alone** | **14 / 14** |

and phase C also returns the published **PF to printed width on 14 of 14**, and
its receipt is **byte-identical to phase B's** on all four commands (md5 in the
receipt).

The size of the defect, in the open:

    published receipt:   kept  198,716 of   278,169 bars   from 2022-06-16
    the same argv today: kept 1,055,936 of 1,135,389 bars   from 2010-06-01

    fix/pm-after      578 -> 3,078      pdhl/break-nyam   172 ->   815
    fix/am-after      578 -> 3,078      pdhl/fade-allday  555 -> 2,672
    hivol/orb60       457 -> 2,115      london/nyam       525 -> 2,607
    lowvol/orb60       73 ->   478      london/expansion  338 -> 2,046

Phase C is the test. `replay.py` reads **section 3 of the sidecar and nothing
else** in the file, knows nothing about this record, and produces the published
number on every row. The answer was known in advance — `receipt-reconcile`
measured all 16 of its W1 rows returning when pinned — which is what makes this
a **positive control** rather than a hope.

**`2022-06-16` was never missing information.** The published receipt prints it,
in prose, one line under the argv: `bars: 198716 from 2022-06-16 00:00 to
2025-04-09 23:55`. The record was never unreadable; it was **unspecified**. That
is the whole finding of the axis, and the sidecar is the mechanical form of it.

**`--seeds=20`, declared:** seeds drive only `null p50` / `null p95`, the
percentile and the verdict word. The method row's trade count and PF do not read
them, and the trade count is what F1 compares. Nothing percentile-shaped is read
anywhere in this result.

## 2. F2 — the bit-for-bit standard. IT DOES NOT FIRE. 20 runs, one md5 each.

Five argv — `hypotheses`, `hypotheses --guards`, `rescore`, `costs`, and a
second market on `config-swap` — against four binaries:

    a  the round's shared binary                              (= 29c8635)
    b  an INDEPENDENT --release build of 29c8635 in this worktree
    c  the post-change binary, NO --argv-sidecar
    d  the post-change binary, WITH --argv-sidecar on every run

`diff -r` is **byte-clean, 5 of 5, for b, c and d against a**. All twenty
receipts carry one md5 per fixture.

`a == b` reproduces `agent/hold-cap`'s control and is worth borrowing again:
**the reproduction base is the commit, not a particular `search.exe` on a
particular disk.** That is why the sidecar records a commit and not a path.

### 2.1 And it DID move one byte first — measured, then fixed at the instrument

Wired the obvious way, with `--argv-sidecar=` still in the argv the header
counts, `--mode=costs` printed:

    < flags:    7 passed, every one of them read by --mode=costs
    > flags:    8 passed, every one of them read by --mode=costs

**One line. One byte. The exact byte the owner's decision was made to protect.**
The fix is `flags_counted()` in `search.rs`, which excludes this one flag and
only this one from the count — and that is not a flag going quiet: the sidecar
asks for a *file*, not a measurement, nothing downstream reads anything it set,
a failed write **aborts the process** before a number is measured, and the
sidecar records the run's whole argv *including itself*. The standard was not
relaxed to "approximately identical". Set `d` above is the proof it holds.

This is also the honest answer to a question the task raised: a bit-identity
test can fail for a reason that is not causal. Here it failed for a reason that
*was* causal, it was found in one `diff` line, and no threshold was touched.

## 3. F3 — the blind probe. IT FIRES, twice, which is the point.

`probe.py on` inserts one line into the sidecar block — the single most
plausible "helpful" regression:

    println!("argv-sidecar: wrote {}", arg("argv-sidecar", "<none>"));

Both guards catch it:

1. **The in-tree source test fires before anything is built.**
   `sidecar_tests::the_sidecar_block_writes_to_no_stream` FAILED with
   *"`println!` inside the sidecar block would change the receipt beside it"*.
2. **`diff -r` of the five fixtures fires** against the same five from set `a`.

Without this step an F2 pass could have been a blind harness. The probe is
reverted; the committed source contains no copy of it (`grep -c` = 0).

## 4. F4 — both paths tried, and one of them is inert

| path | result |
|---|---|
| `--argv-sidecar=<path>` on the command line | **works**, on every mode |
| `[run] argv_sidecar = "…"` inside a batch file | **INERT** — accepted in silence, writes nothing |

Cause, from the code rather than from the behaviour: `BatchFile` in
`hypotheses.rs` carries one field, `#[serde(default)] hypothesis:
Vec<HypothesisFile>`, so every other table in the file is dropped by the
deserialiser. **Same mechanism as `[run] null_sides` and `[run] max_hold_ms`.**

It is **not changed here**: making an unknown `[run]` key an error changes how
every existing batch file parses, and that is the owner's call (addendum 11 §X).
What this job owes the reader is the statement that **there is exactly one way
to ask for a sidecar**, plus the in-tree test that keeps the statement true.

And the flag itself cannot go quiet the other way: it is in `ALWAYS`, and
`sidecar_tests::no_mode_accepts_the_sidecar_flag_and_ignores_it` holds that
against `flag_audit` for all ten modes.

### 4.1 The source-scanning test fired first, as the order requires

The code that reads the flag was written **before** the flag was declared, the
suite was run, and the existing test fired:

    these flags are READ by search.rs and not DECLARED in ALWAYS/BY_MODE, so no
    receipt can say whether a mode ignored them: ["argv-sidecar"]

Only then was `ALWAYS` filled in. The declaration is a consequence of a failure
that was read, not a line typed from memory.

## 5. How the config is recorded, and why — the decision §1 of the task asked for

**Rejected: a list of the keys the engine reads.** That is the same shape as the
defect the source-scanning flag test exists to prevent — a set the code READS and
a set it DECLARES, free to drift apart in silence. A key list goes stale the
first time a field is added, and the sidecar would then be *confidently wrong*
rather than noisy.

**Chosen: both halves of a different pair.**

1. **`sha256` of every file `Config::load` actually opens** — `default.toml`,
   and `local.toml` when it exists, each with its own digest, byte size and
   mtime. This is **over-sensitive**: a comment edit moves it. Over-sensitive is
   the safe direction — it can raise a false alarm, it cannot miss G1.
   `local.toml` is fingerprinted and **never quoted**, so a secrets file is
   covered without being copied. `accounts.toml` and `exclusions.toml` are
   **not** hashed and the file says so: this run never opens them.
2. **The resolved effective values, serialised from the live structs.** This
   **cannot drift** — a field added to `TradingRules` or `Guards` appears
   without anyone remembering to add it — and it is what a reader actually
   wants. Measured on the guarded fixture, the sidecar prints
   **`flat_before_weekend_hhmm = 1640`**: the exact value whose move from 1655
   on 2026-09-19 took `close/fri-1630-1815` from 336 trades to 0. The file hash
   alone would only have said "something changed".

Same reasoning for the tape. **Rejected: `sha256` of the parquet file** — a
re-write with different compression leaves the bars identical (false alarm) and
`XAUDUKA-1m.parquet` is 100 MB of which a run uses a slice. **Chosen: a digest
of the bars the engine was handed**, which is the fingerprint of the exact
input and the only thing that can see class W1+C1, bars rewritten *inside* a
pinned window. The four F1 commands all read the same window and all print the
same digest `f09dc35a…`, which is the digest doing its job.

## 6. Can it be applied backwards? NO — and here is how much of the past it does not fix

**It covers runs made with it, from here on. Nothing before.** A sidecar states
what a run resolved; it cannot be written for a run that happened in September.

The ceiling, in counted numbers rather than an opinion:

**Row level, from `agent/pfr-audit` (the authority, not re-measured here):**

    band rows carrying a PF                            4,845
    of them with NO echoed command in any blob         4,342   = 89.6%
                                                       ^^^^^ THE CEILING

Those 4,342 are mostly agent-written summary tables, which print a row and not
the argv that made it. They are **unmeasured**, not shown to be fine, and the
sidecar does nothing for them: recovering them needs each agent's own driver
script, which is not on disk.

**File level, my own grep over this branch** (`PAST-COVERAGE.txt`):

    tree                     files   has argv   both ends   one end   no bound
    receipts/                   42          1           1         0          0
    docs/research/runs/        715        563         455        58         50
    docs/decisions/             67          2           0         0          2
    TOTAL                      824        566         456        58         52

    files with no argv at all             258 / 824 = 31.3%
    of the files that DO carry an argv,
       both window ends pinned            456 / 566 = 80.6%
       one end or neither                 110 / 566 = 19.4%  <- not a reproducible spec

**And one hole that is nobody's measurement yet:** of `pfr-audit`'s 51 vanished
band keys, **34 sit in `docs/decisions/`** and are what `receipt-reconcile`
resolved; **17 sit only in receipts** and were out of its scope. Those 17 are
**`null` — unmeasured**, not "fine", and this job did not measure them either.

So the honest shape of the answer is: **the sidecar closes the specification
layer going forward and closes none of the past.** What it does for the past is
narrower and worth naming: a row re-run *today* with a sidecar now has a spec,
so the next time the store moves, the comparison exists. That is how 4,342
becomes a shrinking number — one run at a time, not retroactively.

## 7. Three things measured that nobody asked for

1. **`TABLE-34-rows.txt` disagrees with its own body.** Its header reads *"23 of
   34 return their PUBLISHED TRADE COUNT exactly … 15 of those also return the
   published PF"*, while the same file's class table sums to **28** (16+2+5+4+1+0)
   and the result record states **28** with 8 of the 28 drifting on PF, i.e. 20
   returning it. The header is stale against the body; **28/34 and 20/28** are
   the numbers. **The correction is recorded here and nowhere else**, because
   `TABLE-34-rows.txt` and `2026-10-11-receipt-reconcile-result.md` live on
   `agent/receipt-reconcile` and this branch does not carry either file — so
   there was no line of theirs to append to, and none was rewritten. Every
   quotation of them in this document was read with `git show
   agent/receipt-reconcile:<path>`.
2. **"25 of 34" is `W1 + W2 = 16 + 9`, and their return is 21 exact plus 4 off
   by exactly one trade** — not 25 exact. The four are `close/*` on a window
   that matched **bar for bar**, which is itself the argument that a date pin
   alone is not enough and the bar count and digest have to be in the file.
3. **The byte-identity standard is sensitive to the line endings of its own
   source.** Editing `search.rs` through a tool that wrote CRLF made
   `flag_tests` fail with *"this test module's own header must be findable"* —
   not a code fault and not a flaky test, but `include_str!` no longer matching
   a marker containing `\n`. Anyone editing a source-scanning test's file
   should check `file <path>` says no CRLF before reading a failure.

## 8. Multi-test ledger — declared 38, launched 44, read 42

| block | declared | ran |
|---|---|---|
| F2 byte identity, 5 argv × 4 binaries | 20 | 20 |
| F1 positive control, 4 commands × 3 phases | 12 | 12 |
| F3 blind probe, 5 argv × 1 looking build | 5 | 5 |
| F4 inert-path probe | 1 | 1 |
| **declared total** | **38** | **38** |
| *over*: `--mode=costs` 1-month runs that found and then confirmed the one-byte defect (§2.1) | 0 | 4 |
| *discarded*: two set-`c` runs launched against the pre-fix binary and killed when §2.1 was found; **not read** | 0 | 2 |

**Gate cells: 0.** Nothing was spent against
`PF_r ≥ 1.200 ∧ E ≥ +0.050R ∧ n ≥ 40`.

**Suite, at `--release` and `-p fd-backtest` only:** 112 lib + 18 `search` bin +
74 integration = **204 tests, 0 failures**, and `cargo check --release
--workspace` clean. Six of the lib tests and five of the bin tests are new and
belong to this change; the ones worth naming are
`a_null_volume_does_not_hash_as_zero`,
`the_replay_line_pins_a_window_the_argv_left_empty`,
`guards_off_renders_null_and_not_eleven_zeroes`,
`the_sidecar_block_writes_to_no_stream`,
`asking_for_a_sidecar_does_not_change_the_flags_line` and
`the_batch_file_run_table_cannot_ask_for_a_sidecar`.

## 9. Rules kept

- **`main` untouched.** No receipt rewritten: the only change to an existing
  record is a dated note **appended**, 0 lines deleted.
- **Not touched:** `config/local.toml`, `config/accounts.toml`, `data-sealed/`,
  `data/gold/`, `data/btc/`, `/e/rust/flowdesk/target/release/`, and
  `collect.exe` pid 5044 / 38720 — both still running. `--data=` read-only.
- **Build:** `--release`, `-p fd-backtest`, this worktree's own `target-as/`.
  Never `cargo test --workspace`.
- **One new dependency on `fd-backtest`, and it is not a new crate in the
  lock file:** `ring`, already the tree's crypto through `fd-api`'s auth. SHA-256
  only.
- **A cosmetic note, so the record does not look tidier than it is:** the
  `the_sidecar_block_writes_to_no_stream` panic message carries extra interior
  spaces from a `\` string continuation. It was left alone deliberately —
  changing the source after the 20 byte-identity receipts were taken would
  break the chain between them and the committed tree, for a whitespace.

## 10. Amendments

Amendments go here, dated, appended. No line above is rewritten.
