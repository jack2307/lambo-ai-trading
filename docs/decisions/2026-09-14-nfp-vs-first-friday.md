# The control cell does not exist — and one clean release cell came out of the wreck

**Date:** 2026-09-15 (small hours)
**Question:** Is gold's fall over the 07:30 → 08:30 New York hour on US
Employment Situation days the **release**, or the **first Friday of the
month**? The calendar separates them because the release sometimes lands on a
month's second Friday. Registered at `297eee4` with the instrument and before
it was run. `docs/hypotheses/2026-09-14-nfp-vs-first-friday.md`.
**Outcome:** **Closed. Both declared conditions passed and neither measured
the thing they were declared for.** The falsifier compared cell B to cell C
while the claim's own sentence compared B to the baseline D; and a day-by-day
pass over cell B found that **one** of its 32 "quiet first Fridays" is an
ordinary print-free trading day. There is no control cell, so the ordering
test has nothing to order. What survives is narrower and better than the
registration asked for: **a release on a later Friday falls at the 0.2nd
percentile of a day-of-month-matched control**, which is the cleanest reading
this loop has produced. It is 25 days, it is not a strategy, and the
registration's own claim is not supported.

## What was declared, and what happened to it

Two conditions, committed before the run:

| declared | required | read |
|---|---|---|
| C's mean ≤ 0 | negative | **−1.734** ✓ |
| B's mean above C's | B > C | **+0.922 > −1.734** ✓ |

Both hold, on every centre and every robustness cut the reviews applied
(data-integrity: basis points, range normalisation, a 10% trimmed mean, a
strictly-pre-release window, the pooled cell as registered, five permutation
seeds, early and late halves). I could not make them fail and neither could
the adversary. **That is not the result.**

## The four cells

```
cell                          n    mean $    median    up      sign p
A  first Friday, release    153   -1.212    -0.873   35.3%     0.000
B  first Friday, "quiet"     32   +0.922    -0.979   40.6%     0.377
C  later Friday, release     25   -1.734    -1.395   20.0%     0.004
D  later Friday, quiet      565   +0.010    +0.130   52.6%     0.239
C' release, not a Friday      8   +0.590    -0.383   50.0%     1.000
```

data-integrity rebuilt this from the parquet and the CSV without touching the
instrument — a different target-minute construction, a different bar lookup —
and got **every digit**, including all five sign p-values under two
conventions. The window resolves to 07:30 and 08:30 New York wall clock on all
783 observations in both seasons, at a gap of exactly zero minutes, with no
exact-zero moves: **both of the parent's instrument faults are provably
absent here.** Coverage is 97.51% and the drops are Good Fridays, New Year's
Days and three feed holes, none of them in C.

## Why the pass is worthless

### 1. The gate tested a comparison the claim never made

The claim reads *"cell B is not materially more negative than the quiet
baseline **D**"*. The falsifier reads *"B's mean is above **C**"*. Those are
different sentences and only the second was tested. Against D, where the claim
pointed, the adversary's finding reproduces exactly here over five seeds:

| statistic on cell B | value | percentile of draws from D |
|---|---|---|
| mean — the declared one | +0.922 | 82nd |
| median | −0.979 | **1.4th–1.8th** |

The mean is the **only** statistic in the set that puts B above the baseline.
Split B where the adversary split it: its 26 days on days 1–3 of a month read
mean −0.322, median −1.241, **34.6% up** — which is cell A's 35.3% to within
noise. The six days on days 4–7 carry the entire positive mean. Cell B falls.
The run landed in the branch the registration pre-declared as **"both fall"**
and its summary line printed the other branch.

### 2. There is no cell B

news-desk went through all 32 days against an independent calendar. The
result ends the question on its own:

- **Eleven carry an 08:30 macro print the five-name calendar cannot see** —
  Canada's Labour Force Survey on 2026-02-06, 2025-12-05, 2025-11-07,
  2023-12-01 and 2017-12-01; US Personal Income/PCE on 2021-10-01, 2019-03-01,
  2013-03-01 and 2010-10-01; Canada's monthly GDP on 2018-03-02 and 2012-03-02.
- **Four are US market holidays** with no session at all (2014-07-04,
  2015-07-03, 2020-07-03, 2025-07-04).
- **Six are "quiet" only because a shutdown removed the release** that was
  scheduled for that morning.
- **One** — 2024-03-01 — is an ordinary, holiday-free, print-free first Friday.

A control cell of one is not a control cell. And the biggest single
observation in it, 2026-02-06 at +31.07, is a day when Canada's jobs report
printed at 08:30 *and* the US jobs report had been pulled by a shutdown four
days earlier. That day is the difference between B's mean being positive and
negative.

### 3. Both small cells are locked to a third of the calendar

news-desk derived the schedule from the BLS rule and reproduced 175 of 191
release dates exactly. The consequence it drew is the one that matters: a
release can land on a second Friday **only** when the reference month is
February or a 30-day month whose 12th is a Sunday. So cell C is 56% January
and March and contains no April, June, August or September Friday ever, while
the null is drawn from a pool that is flat across the year. Re-drawn holding
the month fixed:

| null for cell C | mean | median |
|---|---|---|
| all quiet later Fridays, as filed | 6.1st | 1.6th |
| month-of-year matched | **16.2nd** | 1.4th |
| **day-of-month matched (days 8–14)** | **0.2nd** | **0.5th** |

That is the same class of fault the parent had to retract — a null that holds
one clock and lets another float.

## What survives, and it is the best thing here

The last row of that table is not a correction, it is the finding. Cell C's
days are all second Fridays, so they sit on days 8–14, **away from the turn of
the month entirely**. Matched there against 143 quiet Fridays on the same days
of the month, a release hour sits at the **0.2nd percentile on the mean and
the 0.5th on the median**, stable across five seeds. The contrast in plain
numbers: quiet Fridays on days 8–14 read +0.384 mean, +0.190 median, 53.1% up;
release Fridays on the same days read −1.734, −1.395, **20.0% up**.

It holds the cuts:

- Without 2013-11-08, which news-desk identified as a shutdown reschedule
  wearing a second Friday's clothes and which is C's largest fall: n=24, mean
  **−1.161**, median −1.196, 20.8% up.
- In basis points, removing the 3.7× drift in the gold price across the span
  (data-integrity): C **−9.71 bp**, A −6.16, B −0.68, D −0.31.
- Strictly before the print, 07:30 → 08:29 (data-integrity): C **−1.309**.
- On each window separately (adversary): −1.499 on 13 days and −1.989 on 12.

This is a release effect measured on days the first-Friday and turn-of-month
confounds cannot reach. It is 25 observations.

## What each role said

- **adversary — WEAKENED.** It reproduced the receipts from the parquet at
  every digit and then went after the gate rather than the numbers. Its
  sentence, which decided this record: *"The declared gate compares B to C
  when the claim compares B to D, and on every statistic except the single one
  that was declared, cell B falls as hard as cell A does — B's median sits at
  the 1.65th percentile of quiet-later-Friday draws while B's mean, on the
  same 32 days, sits at the 82.99th, so the run landed in the registration's
  'both fall' branch and reported its 'C falls, B does not' branch."* It also
  measured what I failed to declare: bootstrapping each cell's residuals, the
  two-condition gate passes **32.1% of the time in a world where nothing
  falls anywhere** — I declared the power unreachable and never declared the
  size, and a gate that passes a third of the time by accident is a coin
  flipped twice. On the sign test it allowed the quote with three caveats: it
  was not declared, it is the smallest of about ten statistics printed
  (Bonferroni ×10 puts it at 0.041), and C's 25 days are a subset of release
  days the parent had already found negative. And it refused the shutdown
  reading of C′ as a rescue: dropping the three largest positives after their
  signs were visible moves the cell by 5.9 $/oz, so C′ says nothing in either
  direction and must be quoted in neither.
- **data-integrity — CAVEAT, not blocked.** Its reproduction is quoted above.
  Four caveats reach this record. **(a)** The registration's cell table is
  arithmetically impossible: it declares 158+34+33+643 = 868 against 835
  Fridays in the span, because D was written as *all* later Fridays including
  C's releases and the 40 busy days the same paragraph promises to exclude,
  and because C = 33 counted the 8 non-Friday releases the instrument then
  splits off. **Neither 643 nor "C's 33 releases" may ever be quoted.** The
  pooled cell as registered is n=33, mean −1.171, so C's headline is 48% more
  negative than the cell the falsifier named. **(b)** The cells are dollars
  pooled across a 3.7× price level, so a 2026 day carries four times a 2012
  day's weight; in basis points the ordering survives but **B's mean changes
  sign to −0.68 bp**, and "B is +0.922" must never be read as "B is positive".
  **(c)** *"the mean says 'release', the median and up-rate say 'release, plus
  a first-Friday tilt that this test cannot size'."* **(d)** A quarter of the
  fall is the single minute containing the print — 27.6% of A's and 24.5% of
  C's, against 6% for D — so the hour is not purely pre-release drift and the
  exit price used is the least fillable price of the day.
- **news-desk — CAVEAT (heavy).** The calendar itself is clean: 191 releases,
  October 2025 the only gap and correctly so, five Good Fridays legitimately
  dropped, every one of C's 25 days independently confirmed as a real 08:30
  release with nothing inside the window. Everything built on top of it is
  not. Its sentence: *"only one of the 32 'quiet first Fridays' (2024-03-01)
  is an ordinary trading day — the rest carry an 08:30 macro print the
  five-name calendar cannot see, or are a US market holiday, or exist only
  because a shutdown removed the release; cell C is 24 ordinary releases plus
  one shutdown reschedule; cell C′ is four pre-Independence-Day Thursdays and
  four shutdown reschedules, one of which (2014-07-03) has an ECB rate
  decision at 07:45 New York inside the measured hour; and because the
  schedule rule confines B and C to 7–8 months of the year while the null is
  drawn from an all-month pool, the ordering test cannot separate 'the
  release' from 'the month' either."*

## Reading

I designed a control cell without checking whether it could exist, and then
declared a falsifier that compared the treatment to another treatment instead
of to the control. Both mistakes are mine and both were invisible from inside
the numbers: the gate passed, every robustness cut preserved it, and the
summary line the instrument printed was a sentence I would have believed. It
took one role reading the claim against its own falsifier and another reading
32 dates against an outside calendar.

The useful thing is what the wreck left standing. Strip out everything the
reviews broke and one cell is untouched: 25 releases that landed on a second
Friday, verified one by one, measured against Fridays on the same days of the
same months, falling at the 0.2nd percentile with a fifth of hours up against
a control's half. That cell is small, it is month-locked, a quarter of its
fall is the print minute itself, and it is still the cleanest directional
reading in twenty-eight registrations.

And the three cells together now point somewhere the registration never
looked. The turn-of-month fall is a **Friday** fall — quiet Monday-to-Thursday
days 1–3 are flat at a median of −0.005 over 274 days, while quiet Fridays on
the same dates read −1.241 over 26. A calendar cannot tell a Friday from a
Tuesday. An 08:30 release can, and news-desk found one on eleven of those
Fridays. The hypothesis that fits all of it is not *the US employment report*
and not *the first Friday*: it is **the hour before any 08:30 New York macro
release**. That is a mechanism, it has five to ten times the sample, and it
needs the calendar extended first. It goes to the backlog as the next
registration and it is not claimed here.

## What would reopen this

- Nothing reopens *this* question. The control cell cannot be built from this
  calendar, and no amount of data on this feed creates quiet first Fridays
  that were never quiet.
- The successor was going to be *any* 08:30 release. **The first look killed
  that and narrowed it**, and the look is filed with this record
  (`pce-exploratory.txt`, exploratory on data already read, one cut, declared).
  BEA's Personal Income and Outlays schedule is now collected — 199 releases,
  each date and time taken from that release's own embargo header, 187 at
  08:30 New York and 12 at 10:00. Ninety of them land on a Friday with no
  employment report, which makes them the natural second release to test. They
  do almost nothing:

  ```
  quiet Fridays, gold, 2010-06 -> 2026-05      n     mean   median     up
  a PCE release at 08:30                      90   -0.198   -0.214   44.4%
  no PCE release                             503   +0.090   +0.155   53.1%
  the employment report, same feed            178   -1.286   -0.903   33.1%
  ```

  and matched on the day of the month — 161 of the 199 releases sit on days
  22–31, where Fridays already tilt down — the release adds **−0.409 $/oz at a
  Welch t of −0.50**, with medians of −0.24 against −0.26. Nothing.

  So the hour is not about macro releases in general. The narrower question —
  **is it employment news specifically?** — then died the same way, and the
  look is filed beside it (`lfs-exploratory.txt`).

  Statistics Canada's Labour Force Survey schedule is now collected too, 201
  releases, and it carries a control inside a single release that no design
  could have arranged: StatCan published the LFS at **07:00** New York until
  2012-03-09 and at **08:30** from 2012-04-05, a move announced in *The Daily*
  of 2011-12-02 for administrative reasons with nothing to do with gold. The
  same report, the same country, the same weekday, shifted from before the
  measured hour to its closing minute. On Fridays carrying no US employment
  report:

  ```
  gold, quiet Fridays, 2010-06 -> 2026-05       n     mean   median     up
  Canada LFS at 07:00, before the window         6   +0.678   +1.61   50.0%
  Canada LFS at 08:30, the closing minute       56   +0.622   +0.11   53.6%
  no jobs report at all, same era              478   +0.023   +0.07   50.8%
  the US employment report, for scale          178   -1.286   -0.90   33.1%
  ```

  A jobs report at the same minute, in the same hour, does **nothing** — if
  anything the sign is up (+0.599 against its own baseline, Welch t +0.68).

  Two candidate mechanisms, two negatives, and between them they leave exactly
  one thing standing: it is not macro news, it is not employment news, it is
  **the US Employment Situation and nothing else**. That is a narrower claim
  than the parent made and a better-supported one, and it is still worth about
  $46 a year inside a blackout the bot already enforces.

  What is registrable after this is not another mechanism. It is the only
  genuine out-of-sample test left for the loop's one surviving claim: **does
  the same hour fall on instruments never read for any news question?** Silver
  and the euro sit on disk 2010–2026 and have never been looked at for this
  family. That registration follows this record.

## What this does not say

- It does not say the release explains the hour. On days 1–3, matched, the
  release adds −0.742 $/oz at a Welch t of −0.45 — not distinguishable from
  nothing.
- It does not say the calendar explains the hour either. Quiet Monday-to-
  Thursday turn-of-month days do not fall.
- It does not say cell C is a trade. Twenty-five days across sixteen years,
  a quarter of the move in the print minute itself, at an exit price a book
  could not get, worth less than the parent's $46 a year.
- It does not rescue or strengthen the parent. The parent's own amendment,
  filed with this record, is that its null held the weekday and the minute
  fixed and let the day of the month float, and that the Fridays it was
  measured against include days carrying other 08:30 releases.
- **It changes nothing about the ten paper books.** The news guard still
  blacks out 07:30–09:00 New York, which forbids every hour discussed here.
