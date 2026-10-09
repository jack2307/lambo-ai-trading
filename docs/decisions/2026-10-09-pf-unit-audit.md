# Registration — the unit of the PF leg, read across the whole record

**Date:** 2026-10-09 (registered before the first engine run of this axis)
**Branch:** `agent/pfr-audit`, cut from `agent/stop-width` (which carries the
`lbar_line` patch).
**Binary:** `/e/rust/fd-stop-width/target-sw/release/search.exe` (built
2026-10-08 20:57 from `agent/stop-width`). Nothing is built on this axis.

## Hypothesis (one sentence)

Reading the desk's `PF >= 1.200` leg in R (`PF_r`) instead of in USD
(`PF_usd`, which is what every published receipt prints) moves a countable
number of published verdicts across the 1.200 line, in both directions, and
the rows it moves can be named.

## Why this axis exists, and why it has NO gate

`agent/stop-width` measured, on the SAME trade set (trade count invariant,
only the trade-size denominator changes):

    PF_usd shifted up to 0.3640   |   PF_r shifted 0.0040   => 91x

with the sign of the shift following the sign of the book, and found **5 of
200 cells with the two numbers on OPPOSITE SIDES of 1.200**, and reading the
gate in `PF_r` moving the surviving-cell count in the tradable arm from 1 to 3
of 40.

This axis checks the ruler, not a mechanism. **Nothing here passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`.** No
gate cell is spent; the multiple-testing ledger below counts RE-RUNS, not
candidates.

## The trap this registration is built around

`PF_r` **cannot be inferred from an old receipt.** `Lbar = E/(PF-1)` on an old
receipt uses `PF_usd`, so it yields a `Lbar_usd` that is not the real `Lbar` —
circular. `agent/gate-legs` computed `Lbar` that way over 6,113 rows and its
conclusion about the 0.250R split stands (it compares internally
consistently), but **it is not used here to infer `PF_r`.** Every `PF_r` in
this axis comes from a re-run with the `lbar_line` patch, which prints
`Lbar`, `PF_r`, `PF_usd`, the gap and the identity residual on the same line
for the same cell.

## THE BAND — declared here, before any run

    Re-run every published row whose PF_usd lies in  [0.700 , 1.700]
    that is  1.200 +/- 0.500

**Chosen by measurement, not by feel.** `agent/stop-width` measured
`|PF_usd - PF_r|` up to **0.3640** on one trade set. A band of +/-0.3640 would
be exactly the largest gap ever measured and would therefore miss any gap even
slightly larger. **0.500 = 1.37x the measured maximum**: a row outside
[0.700, 1.700] would need a USD-vs-R gap 37% wider than anything measured to
reach 1.200. Rows outside the band are declared UNMEASURED on this axis, not
declared unflippable.

## Multiple-testing ledger — declared BEFORE running

Harvest, with `py/research/floor_audit_{harvest,census}.py` **reused
byte-identical from `agent/floor-audit`** (md5 `141d375d2ecde4c0f47ab3caa50e9ae1`,
`dac30be534231686b48ae5930fe54642`) and the row reader
`py/research/gate_legs_check.py` **reused byte-identical from
`agent/gate-legs`** (md5 `255e2cebfeceffd9269edbecc76b50bc`). No harvester is
written on this axis. The record is a moving target — 9,858 rows / 110 refs
when `floor-audit` ran, 10,076 / 118 when `gate-legs` re-ran the same
harvester; **my own numbers**:

    refs scanned                                   131
    distinct text blobs                          1,571
    table rows parsed                           16,788
    distinct printed rows                       10,802
    rows carrying BOTH a PF and an expectancy    6,589

    IN THE BAND [0.700, 1.700]                   4,845 rows
      distinct (label, trades, PF, E) keys       3,381
      keys reproducible from an echoed command     503   <- the re-run set
      distinct commands needed for those            88   <- the runs declared

    OF THE BAND, IN THE DECISION RECORD            153 rows, 91 labels, 27 docs
      reproducible from an echoed command           98
      commands needed for those                     47  (a subset of the 88)

**Declared run count: 88 engine commands.** Every one is
`--mode=hypotheses --batch-file=<docs/hypotheses/*.toml>`; all 32 TOMLs exist
in this worktree; markets `xauusd`, `xauduka`, `xagduka`, `eurduka`, `btc`.

## How a flip is read

For each re-run cell the patched binary prints, on one line, `Lbar` (R),
`PF_r` (R), `PF_usd` (USD), their gap, and the residual of
`E = Lbar x (PF_r - 1)`.

    FLIP, record reads TOO HIGH :  PF_usd >= 1.200  AND  PF_r <  1.200
    FLIP, record reads TOO LOW  :  PF_usd <  1.200  AND  PF_r >= 1.200

**Both directions are counted and every flipping row is named** — the usable
part of `floor-audit` was that it named all 69 of its rows.

**Reproduction check, declared in advance:** a re-run cell counts only if its
`trades`, `PF_usd` and `expectancy` match the published row to printed width.
A cell that does not match is reported as NOT REPRODUCED and is **not** folded
into any flip count — it is null for this axis, not a flip and not a non-flip.

**Decision record vs receipt-only:** every flip is labelled with whether it
sits in `docs/decisions/`. A flip in the decision record weighs more than a
flip that only ever appeared in a receipt. Precedent: `floor-audit` found
`ema-cross/asia` (37 trades, 99th percentile, `SURVIVES`) sitting in
`docs/decisions/2026-09-13-recent-year-screen.md` on all 110 refs.

**Defect 17 split, declared in advance:** every `xauusd` row runs at MINIMUM
LOT (`starting_equity_usd = 100.0`, a cent book, so `raw_lots < min_lot = 0.01`
and is clamped), so its `PF_usd` is the PF of a 0.01-lot book, not of a 1%-risk
book. The flip table is therefore **split by instrument/book**, because
`PF_usd` on `xauusd` and `PF_usd` on a 10,000 USD book do not mean the same
thing.

**Identity check on my own data:** `E = Lbar x (PF_r - 1)` must hold to ~1e-4
on every cell I run. `stop-width` measured 200/200 cells with a largest
residual of 0.00008 R. A cell of mine that fails it is a SECOND place the
identity breaks (the first is known: `engine.rs:1070-1076` mixing USD with R)
and **that is worth more than the result** — it gets reported as the finding.

## FALSIFIER — specific and fireable

**If 0 published verdicts flip when the PF leg is read in `PF_r`**, then the
record's failure to declare the unit of its PF leg is **harmless in practice**,
and the desk writes that down and closes the question — the way
`agent/reread-null` closed the null-sides question (0 receipts misdeclared,
0 of 264 verdicts changed).

If any row flips, it is named and its direction is stated.

## Not promised

- No drawdown criterion (addendum 5 G: this is not the drawdown job).
- Nothing is deployed, `config/accounts.toml` and `config/local.toml` outside
  this worktree are untouched, `main` is untouched, `--data=` is read-only,
  and the two `collect.exe` processes are not touched.
- `data-sealed/` is NOT opened.
- Rows outside the band, rows with no echoed command, and rows that do not
  reproduce are counted and named as unmeasured, never as 0.

---

## Note added 2026-10-09, after the first trial run (appended, nothing above rewritten)

Three things the registration above did not anticipate, each settled by
measurement before the run set was launched:

1. **`--seeds=1` is used instead of each command's published seed count.**
   MEASURED on `2026-09-13-recent-year-sessions.toml` (xauusd 15m,
   2025-09-13 -> 2026-09-12): `--seeds=1` and `--seeds=200` print the method
   rows AND every `Lbar ... PF_r ... PF_usd ... residual` line
   **byte-identical**. Only `null p50`/`p95`, the percentile and the verdict
   WORD move, and this axis reads none of those — the verdict side is read off
   the PUBLISHED row. This buys the whole 88-command set at a cost the machine
   can pay while three other agents hold the same binary.

2. **The record does not re-run to its own `PF_usd`.** First trial:
   `ema-cross/asia` reproduced its trade count (37) and its expectancy
   (+0.294 R) exactly, but `PF_usd` printed 1.622 against a published 1.632 —
   the binary has moved since publication. The registration's reproduction
   check ("trades, PF_usd and expectancy match to printed width") is therefore
   **split into two readings that are reported separately and never mixed**:

       (A) PUBLISHED PF_usd vs re-run PF_r  — the published verdict's own side
       (B) re-run  PF_usd   vs re-run PF_r  — the pure unit effect in one run

   Pairing requires the **same label and the same trade count** (the same trade
   set); the `PF_usd` drift is reported as its own table and is never folded
   into a flip count. A published row with no cell at its trade count is
   counted and named as NOT REPRODUCED.

3. **`--config=` is normalised to this worktree's `config/`.** Any published
   run that used a copied config dir would not be reproduced faithfully; all 88
   band commands echoed the default `config`, so none is affected. Flagged
   because it is a fidelity limit, not because it bit.
