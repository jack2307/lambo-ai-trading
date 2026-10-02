# 2026-10-02-drift-null: what happens to the record when the control is given the method's own exposure to the instrument's unconditional drift

**Registered:** (commit time is authoritative) — written before any line of
this branch's code and before any number on it exists.
**Branch:** `agent/drift-null`, worktree `E:/rust/fd-wt-dn`, cut from `main`
(`90e8d30`), which already carries the count-match pin
(`2026-09-23-matched-null-repair.md`) and the cost match
(`2026-09-24-cost-matched-null.md`).
**Finishes:** Task A of `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`,
registered on `agent/repair-a` at `fc070cf` and cut off mid-implementation at
`d5e08d5`.
**Status:** registered.

This is not a hypothesis about the market. It is a pre-commitment about numbers
that are about to move, for the third time in this programme and for the same
reason: a corrected percentile that flatters a rule this desk likes is exactly
the number nobody will re-check.

## The defect

Gold's **unconditional** drift over the window this desk measures is
**+0.3946 ATR20 per 5 sessions, t = +6.74**, and the price on file went 1,200
to 3,700 USD per ounce. Not one control in this repository is exposed to it:

- `control.rs:RandomEntry::on_bar` — `if draw(seed, bar, 1) < 0.5 { Long } else { Short }`
- `control_hold.rs:RandomHold::on_bar` — the same line
- `direction.rs:DirectionFlipped::on_bar` and `permuted_sides_pnls` — flip each
  entry on `hash >> 63`, which is also a half

All three therefore carry an expected long share of **0.50** and an expected
drift term of about **zero**, whatever the method they are a control for did.
A method that is 100% long collected the drift for free and was then compared
against a control that was never exposed to it. **A method that merely tends
to be long beats both nulls without predicting anything**, so every percentile
in `docs/decisions/` is a figure whose long-share term is unaccounted for.

Two further defects are already on the record and are respected here rather
than re-discovered:

- the matched null was not cost-matched (`2026-09-24-cost-matched-null.md`,
  fixed on `main`);
- **the drift (hold) null is not count-matched**: `RandomHold` enters at
  `entryRate = 1.0` and re-enters as soon as it is flat, producing 3.1-3.7x the
  trade count of the method it is a control for. That one is NOT fixed on
  `main` and is in scope here, because it is the same quantity as the drift
  exposure: a control in the market three times as long collects three times
  the drift, so matching the side *ratio* while leaving the *time in market*
  unmatched does not match the exposure.

## What the control does

**Exposure-matched random entry.** `longShare` on both controls, drawn
independently per entry, set from the method's own **measured** long share the
way `hold_distribution` already takes the method's realised hold; plus a
count-matched entry rate on the hold branch, so that the control's signed time
in the market equals the method's rather than only its ratio.

- `longShare` default **0.5**, which is the coin the whole record was read
  against. At 0.5 the side decision is the old one bit for bit, on the same
  stream, at the same bar.
- The value is **measured, never declared**: `side_distribution` on the same
  trades the row's percentile is read from. A share that is not in `[0, 1]` is
  refused, not clamped, and the receipt prints the achieved share so a refusal
  is visible.
- On the hold branch, `entryRate` is calibrated by the same one-probe method
  `matched_rate` already uses for the entry branch, so the control takes about
  as many trades as the method and is in the market about as long.
- Timing stays random; the stop, target, sizing, hold distribution and cost
  model stay what they were; the control stays wrapped in the method's own
  `Filtered` gates.
- Matched on the ratio and the exposure and **nothing else**. The side
  *sequence* is destroyed, which is the thing under test.

This control is `agent/repair-a`'s choice, which stands: that registration's
two rejected alternatives — a long-only control (a subset of this one at
`longShare = 1.0`) and a block bootstrap (which destroys the wall-clock
calendar every gated row depends on and cannot reproduce an unaffected row
unchanged) — are rejected here for the same reasons, unchanged.

**I build on that branch's design and re-implement rather than merging it.**
`agent/repair-a` was cut from a tree that predates the cost match and the hold
cap, and its `control_for` signature conflicts with the one `main` now carries;
rebasing would mean resolving the same functions by hand anyway, and the
commit I would be rebasing says in its own message that nothing in it has been
reviewed and no number in it may be quoted. So the design is adopted, the code
is written against `main`, and every property is re-measured here.

## Variants

**Two, and no more.** There is no search in this task and nothing to sweep.

1. `--null-sides=ratio` — the measured long share on both controls, hold branch
   unchanged. This is `agent/repair-a`'s control exactly, run first so its
   reported collapse is reproduced before anything is changed.
2. `--null-sides=exposure` — variant 1 plus the count-matched entry rate on the
   hold branch.

`--null-sides=coin` stays the default, so the binary on this branch reproduces
every published number at its old setting.

## The three properties, and the named test that proves each

| property | test |
|---|---|
| count-matched | `the_drift_null_takes_the_methods_own_trade_count` |
| cost-matched | `the_drift_null_pays_the_methods_own_cost_share` |
| drift-controlled | `the_drift_null_carries_the_methods_own_signed_exposure` |

Each runs on the real engine through the audited hypothesis path, not on
synthetic bars, and each reports the achieved figure rather than asserting an
intention. Two more hold the instrument itself:
`a_half_long_share_is_the_old_coin_flip_bit_for_bit` and
`a_null_whose_seeds_all_agree_is_not_a_distribution`.

## Falsifiers for MY OWN control, not for any method

Each of these says the control is broken. Each is checked on the re-run and
published whether it fires or not.

1. **An even row must not move.** A row whose method is 40-60% long must
   reproduce its coin-flip percentile. If such a row moves by more than the
   seed-to-seed noise of its own null, the change reached something other than
   the sides and the work stops.
2. **No gate figure may move.** PF, expectancy and trade count belong to the
   method, not to its control. One differing gate column on one row stops the
   work.
3. **The side match must be achieved, not claimed.** The control's median
   realised long share must be within 0.05 of the method's. A row outside that
   is printed as unmatched and is not quoted.
4. **The count match must survive the drift match.** A row inside the 0.25
   band on the coin run and outside it on the drift run means the drift match
   broke the count match, and the work stops until that is explained.
5. **A null with no spread is not a distribution.** If `null p50 == null p95`
   across seeds, the "percentile" is one comparison dressed as a quantile and
   the figure is reported as **`null`** — not as 0, not as 100, not as a
   percentile. If that fires on a row whose method is *not* fully invested on
   one side, my calibration is broken rather than the row being degenerate.
6. **The sign check, which is the strongest of these.** The same rule long and
   short must land at comparable distances from the middle of their own
   drift-matched nulls. Against a coin, long-gold sits near the 100th and
   short-gold near the 0th *by construction*. If that asymmetry survives the
   drift match on a pair that differs only in `side`, the control has not
   removed the drift and it is not a drift control.

## What is pre-committed before any corrected number is seen

1. Every re-run figure is published **beside its original**, whichever
   direction it moves. Nothing is replaced and nothing is dropped for being
   inconvenient.
2. **No threshold moves.** 30 trades / PF 1.2 / expectancy 0.05R, both nulls at
   the 95th.
3. The direction stated in advance: a method with a long share **above** 0.5 on
   an instrument with positive drift was read against too *weak* a control and
   **its percentile was too high**; below 0.5, too low. A row at 0.5 must not
   move. If a long-leaning row's percentile RISES on the drift match, that is a
   second effect and it is investigated, not published as a result.
4. **The funded books are reported unprompted.** `xau-macd-asia`
   (`macd-cross/asia`, published 100th matched / 93rd direction) and `xau-stoch`
   (`stoch-reversal`) are on real money. Their long shares are measured and
   their drift-matched figures are reported to the owner whichever way they
   move, the same day.
5. A construct whose status changes, changes — and is still not promoted on this
   evidence, because a percentile that moved when its control was repaired is a
   percentile measured once.
6. **If no verdict changes, that is the result**, and it is published as the
   refutation of Task A's claim rather than as a quiet success.
7. **`data-sealed/` is not opened.** Nothing in this task needs it.

## Scope, and the window

The **recent Vantage year** is the primary criterion (owner's decision
2026-09-13): `xauusd:15m`, 2025-09-13 to 2026-09-12, which is the window of the
`2026-09-13-recent-year-*` batches and the one that carries every row the
record still calls promising. The long Dukascopy window
(`xauduka:15m`, 2018-06-16 to 2025-04-10) is **context only**, and is where the
sign check of falsifier 6 is run, because a long-only session hold needs years
of drift to be visible at all.

Scope is settled by **measuring** every re-runnable row's long share, not by
reading names. A row inside 40-60% long is expected not to move and is the
programme's own test of the repair.

## What this does not fix, stated now

- **A one-sided method has no direction null.** `DirectionFlipped` and
  `permuted_sides_pnls` flip each entry on a coin, so they *also* carry an
  expected long share of 0.5 and they *also* remove the drift. A side-ratio-
  preserving direction null is a random permutation of the method's own side
  labels, and for a 100%-long method that permutation is the identity: there is
  nothing to permute and no test exists. This is measured and reported here;
  whether a permutation replaces the coin flip for two-sided rows is decided on
  what the measurement shows, and a published direction percentile for a
  one-sided row is reported as `null` either way.
- **A fully invested one-sided method may have no matched null with a spread at
  all**, and if so the honest output is `null` rather than a percentile. The
  control cannot manufacture a timing lottery for a method that made no timing
  decision.
- The four-hour hold cap is Task C's and is not touched. The cost table is
  Task D's and is not touched. The paper loop, `mt5_executor` and both funded
  books are not touched by anything on this branch.

## What would make this whole piece of work wrong

- A corrected figure that cannot be reproduced from its own receipt.
- A row at 40-60% long whose percentile moves.
- Any change to a gate figure.
- A control that is drift-matched on paper while its trade count or its cost
  share has drifted out of band — trading one defect for another, which is the
  mistake the last two repairs each had to be checked for.
