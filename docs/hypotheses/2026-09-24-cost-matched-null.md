# 2026-09-24-cost-matched-null: what happens to the record when the control is given the method's own stop

**Registered:** (commit time is authoritative) — before the fix is written and
before any corrected percentile is seen
**Status:** open

This is not a hypothesis about the market. It is a pre-commitment about numbers
that are about to move, written before they are known, for the same reason the
count-match repair needed one the day before: a corrected percentile that
flatters a rule this desk likes is exactly the number nobody will re-check.

## The defect

`hypotheses::control_for` builds the random-entry control from
`RandomEntry.default_params()` and overrides **only** `seed` — and `entryRate`,
which `matched_rate` calibrates. Read directly:

```rust
let mut p = RandomEntry.default_params();
p.set("seed", seed);
(&RandomEntry, p)
```

`RandomEntry::default_params()` is
`[("entryRate", 0.02), ("atrPeriod", 14.0), ("stopAtr", 1.5), ("seed", 1.0)]`.

So **the control's stop is 1.5 ATR whatever stop the method under test uses.**
Cost as a fraction of risk is `spread / stop`, so the control's cost share is
pinned near 7.5% of R and its profit-factor distribution sits where that puts
it. A method that merely widens its stop pays less per trade and clears the
control's 95th percentile without predicting anything.

Found on 2026-09-24 by the agent working angle 1 of the designed-methods
programme, which was deliberately building wide-stop methods and noticed that
its losing rows were landing at the top of their nulls.

**The evidence is not subtle.** From that agent's own runs on XAUUSD 15m,
2022-06-16 → 2025-09-23, through the audited `run_hypothesis_fixed_guarded`
path:

| row | invalidation | trades | profit factor | percentile |
|---|---|---:|---:|---:|
| `struct-80` | 80-bar far edge | 1,437 | **0.973 — losing money** | **98th** |
| `struct-80-f14` over 15 years | far edge, ≥14 points | 4,441 | **0.916** | **100th** |
| the same signal | 1.2 × ATR | 2,536 | 0.833 | 2nd |

A method that loses money at the 98th percentile of its own count-matched null,
count match 0.95 and inside the band, is the defect stated as plainly as it can
be.

## Scope, measured rather than assumed

The registry's own stops run **1.0 to 3.0 ATR** against a control pinned at 1.5,
so the distortion across the record is bounded by a factor of two, not the
three-to-nine-fold mismatch the far-stop method produced:

- `stopAtr = 1.5` and therefore **unaffected**: `ema-cross`, `macd-cross`
  (`screen.rs:122`), `stoch-reversal` (`screen.rs:311`), the builtins at
  `builtin.rs:91/316/394/547`.
- `stopAtr = 2.0` and therefore **flattered**: `donchian-breakout`
  (`builtin.rs:237`), `builtin.rs:482`, `screen.rs:179`.
- `stopAtr = 1.0` and therefore **punished** — its percentile is too low:
  `vwap-fade` (`vwap_fade.rs:36`).
- **One grid sweeps the stop**: `donchian-breakout`'s
  `("stopAtr", &[1.5, 2.0, 3.0])`, so its 3.0 cells were compared to a control
  at 1.5 while its 1.5 cells were matched.

**Both funded books are at 1.5 and are therefore cost-matched by coincidence.**
`xau-macd-asia` and `xau-stoch` both run `macd-cross` and `stoch-reversal` at
the default stop, so the percentiles reported to the owner on 2026-09-23 — the
76th and the 14th — do not move on this axis, and neither does the record's
claim of the 100th for `xau-macd-asia`. That is established before the fix so
that it cannot become a conclusion afterwards.

## The fix

Give the control the method's own stop, so that cost is matched as well as
count. Two cases, and the second is the one that needs care:

1. **The method names `stopAtr`.** Copy the value. Exact.
2. **The method's stop is structural** — a channel edge, a swing, a level —
   and has no ATR multiple to copy. Then match the **realised** stop distance:
   measure the method's own median stop in ATRs on this window and set the
   control there. This is not a new idea in this codebase; it is exactly what
   `hold_distribution` already does for the hold null, which takes the method's
   realised hold rather than a guess, and the comments there record two
   separate occasions when a guessed value was the null and was wrong.

## What is pre-committed, before any corrected percentile is seen

1. **Every affected receipt is re-run and the corrected percentile published
   beside the original**, whichever direction it moves. Nothing is quietly
   replaced and nothing is dropped for being inconvenient.

2. **The direction the defect biased is stated before the re-run, so a wrong
   prediction is visible.** A wider-than-1.5 stop pays a smaller cost share
   than the control, so those methods were measured against too *weak* a
   control and **their percentiles were too high**. A tighter-than-1.5 stop is
   the reverse and its percentile was too low. So: `donchian-breakout`'s 2.0
   and 3.0 cells should **fall**, `vwap-fade` should **rise**, and everything at
   1.5 should not move at all. **If a row at exactly 1.5 moves, the fix touched
   something other than the stop and the work stops** until that is explained.

3. **A construct whose status changes, changes** — and is still not promoted to
   a funded account on this evidence, because a percentile that moved when its
   control was repaired is a percentile measured once.

4. **No threshold moves.** The gate stays `min_profit_factor` 1.2 and both
   nulls stay at the 95th. A repair that arrives with a relaxed bar is not a
   repair.

5. **The count match must not be broken by the cost match.** Changing the
   control's stop changes how long its trades last and therefore how many it
   takes, so `matched_rate` has to be recalibrated *after* the stop is set, not
   before. The re-run reports the achieved count match per row, and a row that
   leaves the 0.25 band is reported as unmatched rather than quoted.

6. **The gate figures must not move.** PF, expectancy and trade counts belong
   to the method and not to its control. If any of them changes, something
   other than the null was touched.

## What this does not fix

- **The hold null is still not count-matched.** `RandomHold` enters at
  `entryRate = 1.0` and re-enters as soon as it is flat, so a selective
  self-managed method is read against a control taking several times its
  trades — ratios of 6.75 and 15.20 measured on 2026-09-23. That is a separate
  recorded defect and it is not in scope here.
- **The percentile legs were never the binding constraint.** Across the
  designed-methods programme the profit-factor leg failed first in every case,
  and the three agents that reported found gross profit factors of 1.01–1.16
  against the 1.21–1.32 they needed. Repairing the null makes the record
  honest; it does not make anything pass.

## What would make this work wrong

- A corrected percentile that cannot be reproduced from its own receipt.
- A row at `stopAtr = 1.5` whose percentile moves.
- Any change to a gate figure.
- A control that is cost-matched on paper while its trade count has drifted out
  of the band, trading one defect for the other.
