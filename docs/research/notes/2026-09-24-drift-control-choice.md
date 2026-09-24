# Task A: which drift-carrying control, and why — written before any corrected number

**Registered:** committed before the control was built, before the reproduction was
run, and before any percentile on this branch was seen. Task A of
`docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`.
**Branch:** `agent/repair-a`, worktree `E:\rust\fd-wt-ra`.
**Status:** choice fixed. Nothing below is adjustable once a number exists.

## The defect this control answers

Both existing nulls draw sides from a coin. `control.rs:RandomEntry::on_bar` and
`control_hold.rs:RandomHold::on_bar` both read
`if draw(seed, bar, 1) < 0.5 { Long } else { Short }`, and
`direction.rs:permuted_sides_pnls` flips each trade on `hash >> 63`, which is also a
half. So all three carry an expected long share of 0.50 and an expected drift exposure
of about zero, whatever the method they are a control for did. Gold's unconditional
drift is +0.3946 ATR20 per five sessions (t = +6.74) on the recent window. A long-biased
multi-day gold method is therefore read against a control that was never exposed to the
drift the method collected.

## The control I am building

**A side-ratio-matched random entry**, as one new parameter on the two existing
controls rather than a new control:

- `RandomEntry` and `RandomHold` gain `longShare`, **default 0.5**. The draw becomes
  `if draw(seed, bar, 1) < longShare { Long } else { Short }`. At 0.5 the comparison is
  the one that is there now, on the same stream, at the same bar, so a row whose method
  is 50% long reproduces **bit for bit** rather than approximately.
- `longShare` is set from the method's **measured** long share on its own trades for
  this window — `hypotheses::side_distribution`, the sibling of `hold_distribution` —
  and nothing else. No guess, no registered value, no default that stands in for a
  measurement.
- Timing stays random, the stop/target/sizing/cost model stays what it was, and the
  control stays wrapped in the method's own `Filtered` gates.
- Each entry's side is an **independent Bernoulli(longShare)** draw. The control is
  matched on the *ratio* and on nothing else.

### Why this one

1. **It matches the drift exposure, which is the whole point.** A method that took 100%
   longs over an average hold of *h* collected *h* × drift for free; a control at the
   same long share over the same hold collects the same free term, so what is left in
   the percentile is timing.
2. **It follows the precedent this codebase already set.** `hold_distribution` takes the
   method's *realised* hold rather than a guessed one, and the comments there record two
   occasions when the guess was the null and was wrong — `tsmom` on silver (2026-09-13:
   the guess held 27% less than the 20-day row and 2.4× more than the 120-day row) and
   `tsmom` on EURUSD (2026-09-14: a fixed mean hold could not produce the 356-day trade
   that carried the row, so the null's profit factor was bounded where the method's was
   not). A side ratio is the same kind of quantity, and guessing it is the same mistake.
3. **It degenerates to the existing control exactly, not approximately.** That is what
   makes "an unaffected row must reproduce unchanged" checkable rather than a matter of
   opinion, and it means the change cannot quietly move a row nobody was looking at.
4. **It generalises.** A 72%-long method gets a 72%-long control. A long-only method
   gets 1.0 and needs no second mechanism.

### The two failure modes, and why this is not either of them

- **A control that removes the drift the method was exposed to is not a control.** That
  is precisely today's coin flip, and it is what this replaces. The new control's
  expected drift term equals the method's by construction.
- **A control handed the method's own side *sequence* is not a control either.** That
  would be `DirectionFlipped` at flip probability zero, or replaying the method's sides
  at the method's entry times: it reproduces the method and reports a percentile of
  itself. Here the sides are drawn per entry from the ratio, at random times the method
  never chose, so every bit of *which trade got which side* is destroyed and only the
  budget survives.

## Alternatives rejected, with the reason

**1. A long-only random entry (rejected as the general control).** It is the special
case of this control at `longShare = 1.0`, so it adds nothing for a long-only method and
cannot express the 60–90% band that most affected rows will sit in. Building it
separately would mean two mechanisms where one does, and the one it does not cover is
the common case. It is not wrong — it is a subset, and it is reachable here by
measurement rather than by declaration.

**2. A block bootstrap over returns that preserves drift and serial structure and
destroys timing (rejected, and this is the one I would most like to have).** It is the
better statistical object: it keeps the drift *and* the volatility clustering and
autocorrelation, where a Bernoulli side draw keeps only the first moment. Three reasons
it is the wrong instrument **for this record**:

- **It destroys the clock every affected row is gated on.** Nearly every percentile in
  `docs/decisions/` comes from a method wrapped in `Filtered` — `weekdays`,
  `flat:1630-1815`, session windows, hour bands. `CLAUDE.md` already names as a paid-for
  trap that "a session or regime gate needs a matched null (the control wrapped in the
  same `Filtered` gates)". Block-resampling bars detaches the price path from the
  wall-clock calendar, so the control can no longer be gated the same way and stops
  being matched on the axis this desk has already been burned on.
- **It cannot be count-matched by the existing machinery.** `matched_rate` calibrates an
  entry rate against real bars; on synthetic bars the count match would have to be
  rebuilt, and count matching is the thing three separate defects have already been found
  in over two days. Adding a fourth surface for it, in the same change that repairs the
  drift, would make an unattributable result.
- **It cannot reproduce an unaffected row unchanged**, so it fails the programme's own
  test for a repair (`what-the-record-cannot-see`, "what would make this whole programme
  wrong": "a repaired null that cannot reproduce an unaffected row unchanged").

A block bootstrap is the right next instrument and belongs in its own registration, with
its own answer to the calendar problem. It is named here so that the choice is on the
record as a choice.

## What this control does NOT fix, stated now

- **The direction null is untouched, and for a long-only method it is not a test.**
  Ratio-matching a *permutation* of the method's own sides at `longShare = 1.0` is the
  identity: there is nothing to permute, so a 100%-long method has no direction null at
  all. The published direction percentiles for such rows are reporting the coin flip's
  drift removal, exactly like the matched null. This is measured and reported, not fixed
  here: fixing it means deciding what a direction null *means* for a one-sided method,
  which is a separate registration.
- **The cost match** is Task B's and is not touched.
- **The four-hour hold cap** is Task C's and is not touched.

## Pre-commitments

1. The new control is a **switch**, default off (`--null-sides=coin`), so the binary on
   this branch reproduces every published number at its old setting and the corrected
   figure is `--null-sides=ratio`. Both are run and both are published.
2. **No threshold moves.** 30 trades / PF 1.2 / 0.05R; both nulls at the 95th.
3. **Gate figures belong to the method.** If trades, PF or expectancy differ between the
   two settings on any row, the change touched the method and the work stops and reports
   that instead.
4. **The count match is recalibrated after the change and reported per row as achieved,
   never assumed.** A row outside the 0.25 band is reported unmatched rather than quoted.
5. **The achieved long share of the control is printed on every row** beside the
   method's, for the same reason the count match is: a match that is claimed and not
   measured is the defect this desk keeps finding.
6. Scope is established by **measuring** every re-runnable row's long share and keeping
   those outside 40–60%, not by reading names.
7. If no verdict changes, that is the result and it is published as the refutation of the
   claim.

## The seal, and a conflict in my own brief that has to be said before it flatters me

My brief says the withheld year stays sealed — "do not read anything after 2025-09-23
from any store" — and gives as the reason that "every row you re-run is a window the
record already published". **The second half is false, and it shrinks the scope.**
`docs/hypotheses/2026-09-23-rebate-rescore.toml` and the five
`2026-09-13-recent-year-*.toml` batches run `in_sample_from = "2025-09-13"` to
`in_sample_to = "2026-09-12"`, which lies almost entirely inside the sealed year; several
others (`intraday-momentum`, `tsmom-silver`, `fx-local-hours`, `volman-box`
out-of-sample) end 2026-05-31, and the BTC batches are unbounded.

I am obeying the seal, because it is in the rules that do not move and because reading it
cannot be undone. The consequence is recorded in advance: **the corrected table covers
only rows whose published window ends on or before 2025-09-23**, every row excluded by
the seal is named as excluded rather than omitted, and the 85-row correction set of
`2026-09-23-matched-null-repair.md` is largely out of reach on this branch.
