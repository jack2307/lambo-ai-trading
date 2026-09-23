# 2026-09-23-matched-null-repair: what happens to the record when the matched null is count-matched again

**Registered:** (commit time is authoritative) — before the fix is written and
before any corrected number is seen
**Status:** decided → docs/decisions/2026-09-23-matched-null-repair.md (the pin is in; no gate figure moved on any of the 85 rows re-run and no direction percentile moved; the achieved count match went from 0 of 20 inside the band to 14 of 20 on the rescore; 13 of 22 rescore percentiles moved and the 0-of-22 conclusion did not; one status changed — `keltner-break/asia` 89th → 98th, still two legs of three — and pre-commitment 4's stated direction was itself falsified by one row, which is recorded rather than smoothed over)

This is not a hypothesis about the market. It is a pre-commitment about what
will be done with numbers that are about to change, written before they are
known, because the temptation on the other side is obvious: a corrected
percentile that flatters a rule this desk likes is exactly the number nobody
will re-check.

## The defect

`hypotheses::matched_rate` calibrates the random-entry control's `entryRate`
so the control takes about as many trades as the method it is a control for.
Its own comment says the rate is *"pinned, so the control's grid loses its
rate axis too."*

It is not pinned. The walk-forward paths wrap the control in `Preset::bare`,
which sets `pinned: Vec::new()`, and `walk_forward_guarded` then sweeps
`RandomEntry`'s grid — which carries `entryRate` over `{0.01, 0.02, 0.04}`
(`crates/fd-backtest/src/control.rs:73`). Every cell the sweep selects
carries a grid rate, so the calibration is discarded. Count-matching survives
only on the non-walk-forward path, which runs the control with explicit
params and never touches the grid.

Found on 2026-09-23 while re-scoring the record with the rebate, by an
adversary who noticed that 19 of 22 rows — trade counts **2 to 1,297** —
were being measured against an identical null distribution, p50 0.962 and
p95 1.198.

This is the exact failure `matched_rate` was written to prevent. Its comment
records the original case: "the adversary caught a 61-trade row measured
against a 300-trade null."

## Scope

Every walk-forward `--mode=hypotheses` matched-null percentile this desk has
published. 42 files under `docs/decisions/` mention a percentile; how many
carry an affected number is part of the work to establish, not assumed here.

The **direction null is unaffected** — it permutes sides on the method's own
trades, so it is count-matched by construction. Gate figures (PF, trades,
expectancy) are unaffected: the defect is only in the control the percentile
is read against.

## The fix

Pin `entryRate` on the control in the walk-forward paths, so the calibrated
rate survives the sweep and the control takes about as many trades as the
method. That is what the comment already claims happens.

## What is pre-committed, before any corrected number is seen

1. **Every affected receipt is re-run and the corrected percentile published
   beside the original**, whichever direction it moves. No receipt is
   quietly replaced and none is dropped for being inconvenient.

2. **A construct whose status changes, changes.** If a row that was closed
   now clears its gate and both nulls, it becomes a candidate and is
   recorded as one — and it is *still* not promoted to a funded account on
   this evidence, because a percentile that improved when its control was
   repaired is a percentile measured once. If a row that passed now fails,
   it is closed, and anything running on it is reported to the owner the
   same day.

3. **`xau-stoch` and `xau-macd-asia` are on the funded account right now.**
   Both are closed constructs the owner promoted against the record. If the
   repair moves either one's number, the owner is told the new number and
   the direction it moved, unprompted, whether it helps or hurts his
   decision.

4. **The direction the defect biased is stated before the re-run.** An
   uncalibrated control takes a rate from `{0.01, 0.02, 0.04}` chosen by the
   sweep to maximise the control's own walk-forward score. A thin method was
   therefore compared against a null built from far more trades — a
   tighter, higher distribution — so **thin rows were measured against too
   strong a control and their percentiles were too low**. The expectation is
   that thin rows' percentiles RISE on repair. If they fall instead, that is
   a second defect and it is investigated rather than published.

5. **No threshold moves.** The gate stays `min_profit_factor` 1.2 and the
   nulls stay at the 95th. A repair that comes with a relaxed bar is not a
   repair.

## What would make this work wrong

- A corrected percentile that cannot be reproduced from its own receipt.
- A control that is count-matched on paper but whose trade count still
  diverges from the method's by more than about a quarter. The re-run must
  report the achieved match, not assume it.
- Any change to gate figures. If PF or trade counts move, something other
  than the null was touched and the work stops.
