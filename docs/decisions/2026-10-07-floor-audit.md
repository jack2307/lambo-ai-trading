# 2026-10-07-floor-audit: counting the published record against two printed-receipt defects

**Registered:** 2026-10-07. Branch `agent/floor-audit`, worktree
`/e/rust/fd-floor-audit` (cut from `agent/instr-repair`). Brief:
`AGENT-BRIEF-2026-10-07-AUDIT.md`. `df -h /e` before: **23 GB free**
(floor 3 GB, brief §2).

## 0. THIS AXIS HAS NO GATE

Nothing here is a candidate. No row of this work can pass or fail
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`, no
window is spent, no `search.exe` is invoked, and **no method is proposed**. It
is a count of what the already-printed receipts say. Every cell is descriptive
and is counted as descriptive in the ledger below.

**Count, do not fix.** No engine file, no config file and no receipt is
changed. Raising `[backtest.promising] min_trades` from 30 to 40 would restate
the verdict column of every receipt in the record, and adding the `guards:`
header line retroactively is not possible at all; both are the owner's to
decide, for the same reason `wrong_side_stop` and the `check_exit`-at-`bar.open`
defect are counted and left alone (brief §7).

**Honesty note, 2026-10-07:** the census script
(`py/research/floor_audit_census.py`) was written **before** this registration
file, which is a departure from brief §5. It is recorded rather than hidden.
The axis has no gate, so no gate cell was consumed by the ordering; the
thresholds below are the brief's own and were not chosen after seeing a result.

## 1. The two questions, one sentence each

**A — the trade-count floor.** `fd-backtest::sweep::verdict` compares
`metrics.trades` with `PromisingGate::min_trades`, which `config/default.toml`
sets to **30** and which the binary prints as `only <n> trades (need 30)`. The
desk floor is **40**. So: **how many published rows sit in the 30..39 slot,
and how many of those were printed with a positive verdict?**

**B — the arm the guards ran on.** `search.rs::guards_line` prints
`guards: on — …` / `guards: off (every number unguarded)` in the header, but
only since that line was added to the binary; earlier receipts carry no such
line. So: **how many published receipts do not record the arm in their
header?**

## 2. What counts as a row, and what counts as positive

A **row** is a line of a printed table whose header carries a `trades` (or
`wfTrades`) column, parsed by the index of that column in its own header. A
row is **positive** when its printed text carries `SURVIVES`,
`PASSES ALL THREE`, `gate pass`, `PASS`, `passes` or `promising`. `fail:` and
`not promising` are negative. Rows are de-duplicated on **(printed text, trade
count)**, so one receipt copied onto 110 branches counts once; the branches and
paths it was found on are carried on the row.

A row in the 30..39 slot is **not automatically wrong.** It is a row where the
engine's printed verdict and the desk's gate **say different things**: the
engine's trade-count leg passed, the desk's did not. Both readings are written
beside each other and neither is corrected.

For B, three populations are kept apart and never merged:

* **records the arm** — a `guards: on` or `guards: off` line in the header.
  Readable, **for that arm only**.
* **arm recoverable from the argv** — no `guards:` line, but the receipt echoes
  its own full command line, and guards are enabled by nothing but
  `--guards` (`std::env::args().any(|a| a == "--guards")`), so its absence in
  the echo places the run on the unguarded arm. Readable, **not from the
  header**.
* **arm not readable** — an engine receipt with neither. This is the population
  the question asks for; it is `null`, not `off` (brief §8).

## 3. Scope — declared before the count

`docs/research/runs/`, `docs/decisions/` and every `receipts/` directory, on
**every ref in the repository** (110 local heads + 30 remote heads), read with
`git ls-tree` / `git cat-file`. No branch is merged and no branch is checked
out. Files are de-duplicated by **blob SHA** first, so content shared between
branches is parsed once.

## 4. Multiple testing

| axis | values | n |
|---|---|---|
| defect | A (trade floor), B (guards arm) | 2 |
| trade-count bands for A | 0-4, 5-9, 10-19, 20-29, **30-39**, 40-99, >=100 | 7 |
| guards classes for B | header / argv-only / neither / not an engine receipt | 4 |

**Declared: 2 descriptive questions, 11 reported bands, 0 gate cells, 0
`search.exe` invocations, 0 candidates.**

## 5. Falsifier — one, and it covers both defects

**F1 FIRES (both defects are latent, not blown, and the desk records that and
stops) if BOTH of the following hold:**

* **0** published rows in the 30..39 slot carry a positive verdict, **and**
* **at or above 95%** of engine receipts record the arm in their header.

Anything else means at least one of the two is already in the published
record, and then the named list is the deliverable, not the total.

## 6. How it is read

For A: the slot count, the positive count inside it, the share of **all**
positive verdicts in the record that the slot holds, and **every positive slot
row named** with its trade count, PF, expectancy, percentile, guards arm and
receipt path. A total alone is not a result.

For B: the four-way split by count and by percentage, the rows living under
each, and the receipts in the "arm not readable" class **named in full**.

## 7. What this will NOT answer, declared now

* **Whether any slot row would survive at 40 trades.** That needs a re-run and
  this axis runs nothing. The slot is a disagreement between two floors, not a
  verdict on the mechanism.
* **Whether a receipt that records `guards: off` is the right arm.** It records
  one arm. The 23/398 paired-row figure the brief carries says the arm changes
  the answer, so a one-arm receipt is readable **for that arm** and silent
  about the other, and it is counted that way.
* **Rows whose table prints no trade count.** They cannot be binned and are
  reported as `null`, never folded into a band.
* **Anything about the sealed tape.** `data-sealed/` is not opened (brief §9).

## 8. Signed

Written by the `floor-audit` agent (Claude Opus 5, 1M context) on
`agent/floor-audit`, 2026-10-07.
