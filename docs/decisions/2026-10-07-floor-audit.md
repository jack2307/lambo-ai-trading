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

## RESULT 2026-10-07 — F1 did NOT fire. Both defects are already in the record.

Receipts: `receipts/floor-audit-table.txt` (the one table, every positive slot
row named, and the named list of unreadable receipts),
`receipts/floor-audit-slot-rows.json` (the 1,053 rows behind it). Code:
`py/research/floor_audit_harvest.py`, `floor_audit_census.py`,
`floor_audit_table.py`. No Rust file and no receipt was changed; the diff
against `agent/instr-repair` contains no Rust and no config.

### 0. The gate: there is none

11 reported bands, 0 gate cells, 0 windows, 0 `search.exe` invocations, 0
candidates. Nothing here passes or fails anything.

### 1. What was read

110 refs (81 local heads including `main`, 29 remote-tracking), 81,182 (ref, blob, path)
tuples under `docs/research/runs/`, `docs/decisions/` and `*/receipts/`,
de-duplicated to **1,476 distinct text blobs**, of which **1,266 are engine
receipts** and 210 are agent-written tables or decision records. **14,876 table
rows** parsed, **9,858 distinct printed rows** after de-duplicating on printed
text and trade count.

Parse coverage, stated so it can be attacked: of every line in the record
carrying `SURVIVES`, `PASSES ALL THREE` or `gate pass`, **73 were not parsed as
a table row**; 65 of those are prose or Markdown commentary quoting a verdict,
and **8 are genuine table rows whose table prints no trade count** (two copies
of a four-row `hold/<window>` table). Those 8 are `null` for defect A and are
not folded into any band.

### 2. Defect A — F1's first leg does not fire: 69 rows, not 0

| trades | rows | positive | SURVIVES | gate pass | other |
|---|---|---|---|---|---|
| 0-4 | 588 | 0 | 0 | 0 | 0 |
| 5-9 | 381 | 0 | 0 | 0 | 0 |
| 10-19 | 558 | 4 | 0 | 0 | 4 |
| 20-29 | 376 | 1 | 0 | 0 | 1 |
| **30-39** | **447** | **69** | **26** | **41** | **2** |
| 40-99 | 2,909 | 382 | 109 | 265 | 8 |
| >=100 | 4,587 | 219 | 106 | 107 | 6 |
| **total** | **9,858** | **675** | | | |

**447 distinct published rows sit in the 30..39 slot, and 69 of them were
printed with a positive verdict** — 26 `SURVIVES`, 41 `gate pass, inside the
noise/null`, 2 `PASS` from one agent table that declares its own trade-count
column ignored. That is **10.2% of every positive verdict in the record**
(69 of 675), across **28 distinct mechanisms**. Every one of the 69 is named in
`receipts/floor-audit-table.txt` with its trade count, PF, expectancy,
percentile, guards arm and receipt path.

The heaviest concentrations, by receipt family: **`ratio-reversion` at h4 on
gold and silver — 15 rows, 30-34 trades, 13 of them `SURVIVES`** (the same
cells reprinted under the three `--null-sides` arms and both guard arms);
**`orb` in the `stopentry-*` and `n3-limit-*` sweeps — 13 rows, 31-39 trades,
2 `SURVIVES`**; **`ema-cross/asia` — 7 rows at 36-37 trades, 6 `SURVIVES`**;
`pdhl` 3 rows at 38-39; `ny-morning/donchian-breakout` 4 rows at 37-39, two of
them `SURVIVES` at the 100th percentile.

**The single most-republished positive slot row is `ema-cross/asia` at 37
trades, PF 1.632, expectancy +0.294R, 99th percentile, `SURVIVES`** — it is
carried on **110 refs** in both
`docs/research/runs/2026-09-13-recent-year-sessions/in-sample.txt` and the
decision record `docs/decisions/2026-09-13-recent-year-screen.md`, and it is
reprinted four more times (76, 76, 54 refs) by the matched-null repair and the
drift-null re-read. Three trades short of the desk floor, on every branch in
the repository.

**These 69 rows are not wrong.** Each one is a row where the engine's printed
verdict and the desk's gate disagree on the trade-count leg and nowhere else:
the engine's floor of 30 passed, the desk's floor of 40 did not. Whether any
of them would still read positive at 40 trades is **not measured here** and
needs a re-run this axis did not do.

A side count, not a defect: **5 further positive rows sit below 30 trades**
(11, 13, 17, 18, 22), all five in `receipts/Z-summary.txt`, whose own header
declares `gate PASS counts rows with PF >= 1.200 AND expectancy >= +0.050R,
trade count ignored`. Declared by its author, so it is readable; it is reported
because a reader skimming that column would take it for an engine verdict.

### 3. Defect B — F1's second leg does not fire either: 43.0%, not under 5%

| class | receipts | % of engine receipts | rows |
|---|---|---|---|
| records the arm in its header (`guards: on` / `guards: off`) | 722 | 57.0% | 12,965 |
| no `guards:` line, but echoes its own full argv | 514 | 40.6% | 670 |
| engine receipt, no `guards:` line **and** no argv echo | 30 | 2.4% | 16 |
| not an engine receipt (agent table / decision record) | 210 | — | 1,225 |

**544 of 1,266 engine receipts (43.0%) do not record arm guards in their
header.** The two halves of that number are not the same thing and must not be
added by a reader:

* **514 of them echo their own full command line**, and in **not one** of the
  514 does `--guards` appear. All 514 were written by `search.exe`, whose only
  guards switch is `std::env::args().any(|a| a == "--guards")`
  (`search.rs:245`), so **those 514 are the unguarded arm** — stated by the
  argv, not by the header. Readable, with one more step.
* **30 are engine receipts with neither**, and of those **10 book trades** —
  these are the only receipts in the whole record whose arm the desk cannot
  recover at all. They are `null`, not `off`. Named in the receipt; they are
  the 2026-09-13 `ict-sweep-mss-fvg`, `orb-ny`, `london-range`, `doji`,
  `doji-2018` and `close-reopen-drift` runs, carried on **110 refs** each. The
  other 20 book no trade at all (19 `2026-09-23-designed-2` path-efficiency
  measurements plus one pre-check), so no arm applies to them.

**And the sharper finding, which the question did not ask for: of the 722
receipts that DO record the arm, 367 record `off`, 355 record `on`, and ZERO
print both arms side by side.** Every recording receipt pins exactly one arm.
Given the 23/398 paired-row figure — 5.8% of paired rows crossing PF 1.200 on
the flag alone, worst PF 2.328 unguarded against 0.894 guarded on exactly 64
trades — a receipt that records one arm is readable **for that arm** and says
nothing about the other, and the record contains no single receipt a reader can
compare the two arms inside.

Crossing the two defects: of the 69 positive slot rows, **63 sit in receipts
that record the arm** (36 `off`, 27 `on`), 1 in a receipt where only the argv
says, and 5 in agent-written tables. So the slot is almost entirely readable
for guards — the two defects are largely independent populations rather than a
compounding one.

### 4. The figure in the brief that does not match the print

Brief §9 says six receipts in the repo print that they ran on `data-sealed/`.
Counted from the printed `data:` / `data root:` header: **48 blobs** —
`2026-09-23-designed-2` (19), `2026-09-24-repair-c` (16),
`2026-09-23-designed-4` (6), `2026-09-24-cost-matched-null` (5),
`2026-09-24-repair-d` (2). Per brief §8 the measured number is reported. The
sealed store was **not opened**; this is a count of what the receipts say about
themselves, and the reason not to mix the two stores is unchanged.

### 5. Ledger, declared vs looked at

| | declared | looked at |
|---|---|---|
| descriptive questions | 2 | 2 |
| reported bands | 11 | 11 |
| gate cells | 0 | 0 |
| `search.exe` invocations | 0 | 0 |
| refs scanned | every ref | 110 (81 local + 29 remote) |

### 6. What is still not measured

* Whether any of the 69 slot rows survives at 40 trades. Needs a re-run.
* Which arm the 10 unreadable receipts ran. Unrecoverable from the print; only
  the owner's shell history or a re-run could say, and the re-run would not be
  the published number.
* The 8 positive rows whose table prints no trade count.
* Whether the `off` arm each of the 367 recording receipts pins was the right
  arm for the claim it supports. That is a reading question, not a count.
