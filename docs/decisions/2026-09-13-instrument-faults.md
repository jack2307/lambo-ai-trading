# Three engine faults found by reading trade lists, and what they touched

**Date:** 2026-09-13 (late morning)
**Question:** When a walk-forward and a fixed replay of the same pinned row
disagreed (tsmom-2: PF 0.78 vs 2.03 on nearly the same 75 trades), was the
market or the instrument responsible?
**Outcome:** The instrument, three times over. Each fault is fixed in its own
commit with a pinning test, and every earlier record that could have been
touched was re-run and either unchanged in outcome or re-checked on its
window. No outcome in `docs/decisions/` changes.

## The faults

1. **A sizing stop on a strategy-managed hold was enforced** (`f66c7bf`).
   `check_exit` skipped the engine's stop/target/clock for `Exits::Strategy`
   only when the entry carried no stop. tsmom-2's entries carry a stop for
   sizing, so the first run tested a stopped, four-hour TSMOM: 1,237 trades
   where the claim has 75 (`runs/2026-09-13-tsmom-2/stop-enforced/`). Now a
   self-managed position closes only on the strategy's signal or the end of
   its window. Test: `tests/self_managed.rs`.
2. **A preset naming a parameter at its default was not pinned** (`f66c7bf`).
   `Preset::grid` read "pinned" as "differs from the default". `gap/1atr`
   (1.0, the default), `tsmom2/60d` (60, the default) and every `*/fixed` row
   were re-selected by the walk-forward and equalled their grid rows. Pinned
   now means named in the overrides. Fixed receipts were never affected.
3. **A position open at a fold's end ran to the end of the data** (`9fde7c1`).
   Past `Range.to` the strategy was not consulted but the book was not
   flattened; a self-managed hold opened in fold 1 (window to 2021-03-08)
   closed 2025-04-09 at −27R, one such trade per fold. Engine-exit positions
   leaked less (closed by their stop or clock after the boundary). Now the
   book is flat at the first bar past the range. Test: `tests/self_managed.rs`.

And one null that was not measuring what its label said: for a method that
decides exits from the side it holds, `DirectionFlipped` kept the exit rule
inside the direction null (median PF 1.56 on tsmom-2). Replaced for
`Exits::Strategy` methods by a permutation of the method's own trades'
sides (`9ae323f`, `runs/2026-09-13-tsmom-2/flip-replayed/` keeps the old
receipts).

## What was re-run

- **btc-us-hours** (`session-hold`, walk-forward): `hold/us-hours` PF 0.781 →
  0.881 (37th → 26th percentile), `hold/off-hours` 0.946 → 0.946, `hold/asia`
  0.892 → 0.892; the null's 95th percentile fell from 2.98 to 1.24 — the
  runaway trades had been in the null as well. Outcome unchanged: every row
  fails the gate. Prior receipts under `runs/2026-09-13-btc-us-hours/before-fold-close/`.
- **london-fix** (`session-hold`): the batch file has no lower bound and the
  Dukascopy file has since been extended to 2018, so the re-run is on
  2018–25 (1,401 holds per row), not the record's 2022–25 (578). On the
  longer window: `fix/pm-into` 0.935 (92nd), `fix/pm-after` 0.932 (96th, gate
  fail on profit factor), `fix/am-into` 0.536, `fix/am-after` 0.683. Outcome
  unchanged. Prior receipts under `…/before-fold-close/`.
- **tsmom** (the first registration) was parked on sizing before any of
  this; not re-run — `tsmom-2` is its re-run.
- Every `Exits::Engine` record: a fold boundary can now cut at most one
  four-hour trade per fold; not re-run, and the records' percentiles should
  be read as ±1 trade per fold.

## What this does not say

- It does not say the earlier closed verdicts were wrong; each was checked
  and stands.
- It does not say the engine is now correct; it says three more things it
  did wrong are now things it is tested not to do. The skill's standing rule
  is the residue: read the trades before the tables.

## Addendum, 2026-09-14 (small hours): four more, found by the reviews

All found by reading receipts against code during the night's reviews, all
fixed with tests before the next registration ran, none changing a verdict:

1. **The sizing range counted Sunday evenings as days** (`average_day_range`,
   data-integrity on `2026-09-13-close-reopen-drift`): a six-hour Sunday
   session entered the 20-day mean as a full day, shrinking R by 6–9%,
   uniformly. Fixed at `f65a061`: a day whose bars span less than half the
   longest day in the window is not a day; half-days and one-bar-a-day feeds
   still count.
2. **The hold null guessed half the lookback** for a rebalanced hold
   (adversary on `2026-09-13-tsmom-silver`): 27% short of the 20-day row's
   realised hold, 2.4× the 120-day row's. Fixed at `f65a061` to the method's
   mean realised hold, then — because *"a fixed 23-day random hold cannot
   produce a 356-day, +23R trade"* (adversary on `2026-09-14-tsmom-eurusd`)
   — at `e0d46c7` to a log-normal draw per trade around the geometric mean
   of the realised holds, with the realised log-sd.
3. **The hold null exited one bar late** (adversary on
   `2026-09-14-intraday-momentum`): it signalled on the bar where the
   minutes had elapsed, and the engine fills at the next open, so every
   window null held one bar longer than the method — 15 minutes on a
   two-hour hold, and across the daily halt or the weekend when that bar
   was the day's last (the 75-minute null on gold held to 18:00, and to
   Sunday on 399 Fridays; sized-null p50 0.677 against the direction null's
   0.619 on the same window). Fixed at `4eaef94`: signal when
   `held + bar interval ≥ hold`. The session-hold records before it
   (`2026-09-13-close-reopen-drift`, `2026-09-13-friday-weekend-hold`, the
   `recent-year-hours` context) carry sized-null percentiles from the late
   control; on all of them the direction null — the method's own trades,
   unaffected — decided the row the same way, and the records say so.
4. **Filters gate entries only** (data-integrity on `2026-09-13-tsmom-silver`):
   `weekdays` does not stop a self-managed method from acting on a signal at
   the Sunday reopen bar as an exit. Not changed — a registration that wants
   no Sunday exits has to say so in the method — and stated in every tsmom
   record since.

The pattern of the first record holds: none of these was visible in a
table. Two came from an adversary comparing the two null columns to each
other, one from a data-integrity replay of the method's own rule, one from
reading a trade list's exit stamps.

5. **The receipts' profit factor is a compounded-dollar number** (adversary
   on `2026-09-14-volman-box`, afternoon): lots are 1% of compounding
   equity, so on a row losing 0.14R a trade the equity curve falls from
   $10,000 to $84 and the profit factor — gross wins over gross losses in
   dollars — is 47% the first seven months. The direction percentile of
   that row reads 1st in dollars and 38th in R. Not yet fixed: the
   receipts and the permuted-sides null should print an R-weighted (or
   fixed-lot) profit factor beside the dollar one, and the loop's
   thousands-of-trades negatives should be re-read against it. No verdict
   moves — a loser is a loser in both units — but the percentiles quoted
   for deep losers are not what they look like. Backlog.

## Addendum, 2026-09-14 (night): two more, and the first on a winner

Both in `scripts/news_drift.py`, both found by the reviews of
`2026-09-14-pre-nfp-drift`, both fixed at `436582c` with the uncorrected
receipts kept beside the corrected ones. They are the first faults this loop
has logged on a result that *flattered* a claim: every earlier one made a
number worse, which is the direction a reader forgives.

6. **A permutation null clocked in UTC against a New York event**
   (data-integrity and the adversary). The null took its minute from the
   first event's UTC stamp, so every fake Friday sat at one fixed UTC minute
   — 08:30 New York in summer and 07:30 in winter. A third of the null
   measured an hour earlier and quieter than the hour it was controlling,
   which raised the null's mean and narrowed its spread, and pushed the
   actual into a tail it does not occupy. The test window reads 0.0th
   percentile before and 3.2nd after; the exploratory window reads 0.4th
   before and **7.8th** after, which is a failing number under the gate that
   window was used to justify. The rule the fix encodes: a control for an
   event announced on a wall clock is built on that wall clock, and a
   daylight-saving boundary inside the window is a fault, not a rounding.

7. **A shut market scored as a move of exactly zero** (data-integrity).
   `open_at` returned the first bar at or after the minute asked for,
   however far away that was. On a Good Friday both ends of an hour-long
   window resolved to the same bar days later and the move was recorded as
   0.0 — which `np.isfinite` keeps, so a missing observation entered the
   mean as a real one and dragged it toward zero. Four of 94 test releases
   and fourteen of 415 null candidates were zeros of this kind. Fixed with a
   one-bar tolerance: a bar further from the minute than one interval is no
   observation. The rule: a lookup that cannot fail silently tells you it
   failed.

## Addendum, 2026-09-15 (small hours): two design faults, not coding ones

Both from `2026-09-14-nfp-vs-first-friday`, and both different in kind from
the seven above. Faults 1–7 were things the code did that its author did not
intend. These two are things the code did exactly as written, where the
writing was wrong — which is why no test would have caught either and why
both needed a role reading the design rather than the output.

8. **A null that holds one clock fixed and lets another float** (news-desk,
   and the same class as fault 6). The four-cell run drew its control from
   quiet later Fridays across all twelve months, while the cells it was
   controlling can only exist in seven or eight: the US employment report
   lands on a second Friday only when the reference month is February or a
   30-day month whose 12th is a Sunday, so the release cell is 56% January
   and March and contains no April, June, August or September day ever.
   Re-drawn holding the month of the year fixed, the cell's mean moves from
   the 6.1st percentile to the **16.2nd**. The rule both this and fault 6
   encode: **name every clock the treatment is locked to, and hold all of
   them in the null.** Weekday and minute were held; month of year was not,
   and nobody noticed because the cell was defined by a rule about weeks.

9. **A screen applied to the control cells and not to the treatment cells**
   (data-integrity and news-desk). The same run excluded any day carrying a
   second scheduled high-impact release — but only from the two cells with no
   release in them. The two release cells were never screened, so
   `2014-07-03` sat in one of them carrying an **ECB rate decision at 07:45
   New York, inside the measured hour**. Harmless in fact on the main cells
   (0 of 153 and 0 of 25 carry one) and not harmless in principle: a filter
   that runs on one arm of a comparison and not the other is a difference
   between the arms that nobody declared. The rule: **a screen is a property
   of the comparison, not of a cell.**

And one fault that is not the instrument's but belongs beside them, because
it did the same damage: **the registration's own cell table was
arithmetically impossible** — 158 + 34 + 33 + 643 = 868 against 835 Fridays
in the span, because the baseline was written as *all* later Fridays,
silently including the release days it was the control for and the busy days
the same paragraph promised to exclude. A declared count that does not add up
to the calendar is a fault findable before any data is read, and it was
committed unread.

## Addendum, 2026-09-15 (morning): two faults about reporting, not measuring

Faults 1–5 were things the code did that its author did not intend. Faults 6–7
were the same on a result that flattered a claim. Faults 8–9 were design: a
null that held one clock and let another float, and a screen applied to one arm
of a comparison. These last two are about what happens to a number **after** it
is computed, which is the part no test can reach.

10. **A receipt was corrected by a commit message** (adversary,
    `2026-09-15-nfp-cross-asset`). A wrapper passed
    `friday_cells.load_calendar`'s midnight-normalised dates into
    `news_drift.permutation`, so fifteen percentiles in
    `cells-and-robustness.txt` measured the hour before midnight instead of the
    hour before the release. I found the fault myself, said so in the commit,
    and re-ran the numbers correctly — and left the wrong ones sitting unmarked
    in the receipt, where they read as if silver's gate passed at the 0.5th
    percentile against a truth of 6.02. The adversary reproduced all fifteen
    wrong values by feeding midnight stamps, confirming the diagnosis rather
    than accepting it, and then said the sentence that made this a numbered
    fault: *anyone quoting that receipt in six months quotes a bug.* The rule:
    **a wrong number is corrected inside the file that carries it.** A commit
    message is a note to whoever is reading the history, not to whoever is
    reading the evidence.

11. **A gate was placed at a precision the estimator could not deliver**
    (adversary, same record). The permutation prints one draw of a Monte-Carlo
    estimate whose standard error at the instrument's default 1,000 draws is
    **0.74 percentage points**, and the gate was written at "the 5th
    percentile". Silver's true value is 6.02 ± 0.04 and the declared command
    printed 4.8, so the run turned on which seed the default happened to be.
    Gold at 1.37 and the euro at 55.2 were never in doubt; only a value sitting
    within a standard error of its own gate was. The rule: **set the draws from
    the precision the gate needs, not from the instrument's default.** At
    100,000 draws the standard error is 0.07, and every percentile gate in this
    repository should say what precision it requires before it is run.

## Addendum, 2026-09-15 (afternoon): a unit that is not time, and a gate chosen after the cell

From `2026-09-15-pair-residual`. Both are about the instrument of judgement
rather than the instrument of measurement, which is where this loop's faults
have been migrating since fault 10.

12. **A horizon counted in rows on a tape with a hole in it** (data-integrity
    and the adversary). The lookback and the hold were bar counts, so "four
    hours" stretched silently across Dukascopy's daily one-hour break, a
    weekend, or a day on which one of two paired instruments had no feed at
    all. Fifteen percent of the holds were not four hours of market and they
    carried **53% of the gross**; requiring the lookback to be clean as well
    put **93% of the gross into 26% of the trades**, and seven trades held
    straight across a day with one leg absent, five of them averaging +91 bp.
    On the 1,257 honest trades the cell failed two of its three declared
    conditions. The rule: **a window measured in bars is measured in a quantity
    the market does not have.** Every horizon in this repository is currently a
    bar count, and on a feed with a daily break that is not a duration. Where a
    claim is about a duration, the window must be checked against the stamps.

13. **The gate and the cell were chosen on the same statistic** (adversary).
    The registration gated the win rate — twice, since its sign test is a
    function of the same number — and then selected its cell from twenty-seven
    with the sentence *"it is not the maximum of any column."* That was checked
    against gross and against the t-statistic and was true of both. It was not
    checked against the win rate, where the registered cell ranks **1 of 27**.
    The gate was chosen well, for power and to avoid fault 11's seed problem;
    the cell was chosen to look principled; and the two were never crossed. The
    rule: **a cell is "the middle" only of the statistic it will be judged by**,
    and a registration claiming a cell was not cherry-picked must say so about
    the gated column specifically.

And one that is not numbered because it is not about an instrument at all, only
about care: in the review prompts for this run I attributed a figure — "206
intra-week holes totalling 4,287 hours" for the silver feed — to
`2026-09-15-nfp-cross-asset.md`. The figure was real but it lived in that run's
data-integrity report, not in the record, and data-integrity checked the record
and said so. **A number quoted from memory is a number that has not been
checked**, and that applies to what is put in front of a reviewer exactly as it
applies to what is put in a record.
