# 2026-10-07 — reading the desk's highest percentile: `intraday-momentum`, 98th gross / 99th net, count match 14.79

**Registered:** committed ALONE, before the first run on this branch.
**Branch:** `agent/top-percentile`, worktree `E:/rust/fd-top-percentile`, cut
from `agent/instr-repair`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` §4 and §6(a);
`E:/rust/AGENT-BRIEF-ADDENDUM-5.md` A, B, D, E(2).
**Binary:** `/e/rust/fd-instr-repair/target/release/search.exe` (no build on
this branch).
**Status:** registered.

## 0. The one row

`docs/research/runs/2026-09-23-rebate-rescore/primary-xauusd-15m.txt`, row
`intraday-momentum`:

    197 trades, PF 1.088 gross / 1.121 net, matched null 98% gross / 99% net
    matched null p50 0.952 / p95 1.067 gross, p50 0.973 / p95 1.090 net

That 98/99 is the highest matched-null percentile on any receipt in the
record. `agent/reread-null` measured the control behind it at **count match
14.79** — the null's median trade count is 2914 against the method's 197 —
and the 2026-09-23 engine printed **no count-match line at all**, so the
figure was published with no warning on it.

## 1. Hypothesis, one sentence

The 98th/99th percentile of `intraday-momentum` is a property of an
**uncalibrated control**, not of the method: the control takes ~14.8x the
method's trades, which tightens the control's profit-factor distribution and
lowers the 95th percentile the method is being compared with.

## 2. Falsifier, specific and fireable

**F1.** Re-run the row on the patched binary, same batch, same window, same
seeds (200 matched / 1000 direction). If the printed `count match` lands
**inside 1 ± 0.25** (i.e. in `[0.75, 1.25]`), the hypothesis is wrong and the
percentile was matched all along. Fires on one printed number.

**F2.** Run the same row under a null that *can* be calibrated to the method's
exposure (`--mode=hypotheses --null-sides=exposure`, which is the only mode
that reads that flag — ADDENDUM-5 E(2)). If the calibrated control's 95th
percentile still sits below the method's profit factor **on both windows**,
the percentile survives calibration and the row is a candidate, not an
artefact. Fires on the calibrated `p95` vs the method's PF.

**F3 (pre-check, spends no gate cell).** If the control for this row is built
by `matched_rate` at all, then the brief's framing ("one probe at
`entryRate = 0.02`, scaled linearly") is the mechanism and I measure the
probe. If it is built by the **hold** branch instead
(`exits() == Exits::Strategy && grid().is_empty()`), `matched_rate` is never
called on this row and the brief's framing is the wrong suspect — I say so and
measure the branch that is actually taken.

## 3. Multiplicity, counted BEFORE the runs

Declared **8 cells**, all on one construct:

| # | mode | window | arm |
|---|---|---|---|
| 1 | `rescore` | `xauusd:15m` 2025-09-13 → 2026-09-12 | no guards (as published) |
| 2 | `rescore` | same | `--guards` |
| 3 | `rescore` | `xauduka:15m` 2022-06-16 → 2025-04-10 | no guards |
| 4 | `rescore` | same | `--guards` |
| 5 | `hypotheses --null-sides=coin` | primary | no guards |
| 6 | `hypotheses --null-sides=exposure` | primary | no guards |
| 7 | `hypotheses --null-sides=exposure` | second window | no guards |
| 8 | `hypotheses --null-sides=exposure` | primary | `--guards` |

`--exit-mix` is passed on every cell: `intraday_momentum` declares
`Exits::Strategy`, so §6(a) says its own rule must be shown to fire and not
be replaced by `NEWS_FLAT` / `WEEKEND_FLAT`.

Cells 1–4 are a **re-reading of a published figure**, not a new claim: the
coin-flip control is the one the record was measured against. Cells 5–8 are a
**different measurement** and will be reported as such, never as "the same
number, corrected".

## 4. How it is read

- A percentile whose printed `count match` is outside `[0.75, 1.25]` is
  **NOT READ**. No new percentile is published for it. This follows the
  brief's §4 rule and `COUNT_MATCH_BAND = 0.25` in the code.
- Every percentile is written with its `null p50` and `null p95` beside it.
  `null p50 = 0.000` means the control did not calibrate, never "the null
  does not make money".
- The gate is reported as **one** condition with the **binding leg** named
  (ADDENDUM-5 A), at this construct's own reward/risk — not as three
  independent legs.
- A result that lives only in the no-guards arm is reported as **not
  tradeable** (ADDENDUM-5 D), not as a candidate.
- One window is nothing (ADDENDUM-5 B). No candidate is named off cell 1
  alone.
- `cost/R` on this row: the method manages its own exits and has no enforced
  stop, so `cost/R` has **no denominator** here. That is "not measurable",
  printed as such, not 0%.

## 5. What this axis does NOT promise

No drawdown figure (ADDENDUM-5 G: `max_drawdown_*` is not printed by
`--mode=hypotheses`). No change to `matched_rate` — frozen by
`docs/hypotheses/2026-09-23-matched-null-repair.md`. No change to the hold
null's arithmetic either; if the mechanism is in the hold branch it is
**reported, not repaired**, on this branch.

---

## Note added 2026-10-07 (the registration above is not rewritten)

**F3 fired at pre-check, before any gate cell was spent.** `matched_rate` is
**never called on this row**, so the brief's suspect — "one probe at
`entryRate = 0.02`, scaled linearly" — is the wrong mechanism and no probe
needs explaining. `hypotheses.rs` sets

    let drift = base.exits() == Exits::Strategy && preset.grid().is_empty();
    let rate  = (!drift).then(|| matched_rate(...));

and `IntradayMomentum::exits()` is `Exits::Strategy` with `grid()` empty, so
`drift = true`, `rate = None`, and the control is the **hold** branch. The
printed proof is on the row itself: `rescore` prints
`cost/R: NOT MEASURABLE on this row` exactly when `control_stop` is `None`,
and `control_stop` is also `(!drift).then(...)`.

**Two diagnostics added, declared here before they were read, neither a gate
cell:**

- `diag-exitlabel-noflat` — the registered row exits 197/197 under the label
  `flat window`, which `filter.rs:356` emits and which "overrides the
  strategy", while `intraday_momentum.rs:75`'s own `window closed` appears
  **0 times**. The two coincide in time (the hold ends at 16:30 New York, the
  batch's `flat:1630-1815` starts there), so this cell drops the flat filter
  to see which it is. Its gate figures are **not** this construct's figures.
- The exposure cells (6, 7, 8) are reported as a **different measurement**,
  never as a correction of cells 1-4. `--null-sides=` is inert in `rescore`
  (ADDENDUM-5 E(2)), so there is no way to run the published mode against the
  calibrated control, and the two live side by side.
