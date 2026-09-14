# Gold falls in the hour before the US employment report — on two windows, two feeds, and by about what it costs to touch

**Date:** 2026-09-14 (night)
**Question:** Does gold's move over the hour before the US Employment
Situation release (the open of the first bar at or after 07:30 New York to
the open of the first bar at or after 08:30) fall by more than the same
hour on other Fridays, on the eight years no run had read for this
question? Registered at `99c0830` with the instrument, the exploratory
figures and three numbers, before the test window was opened.
`docs/hypotheses/2026-09-14-pre-nfp-drift.md`.
**Outcome:** **The claim survives its falsifier — the first registration in
this loop to do so — and one of the numbers that justified registering it
was wrong.** On the corrected instrument the test window sits at the 3.2nd
percentile of a clock-matched null with 35.6% of hours up on 90 releases,
passing all three conditions declared in advance. The exploratory window,
re-measured the same way, sits at the **7.8th** percentile: it would have
failed the very gate it was registered under. A second feed from another
venue agrees on the direction and the up-rate. The effect is worth about
**$46 a year on a $10,000 account**, and the repository's own news guard
already forbids trading the hour it names. Nothing is promoted; no paper
book changes.

## Where it came from

The owner sent a survey of day-trading strategies (sarwa.co). Eight of its
nine are families this loop had closed on multi-year windows. The ninth,
news trading, had never been testable here until the scheduled-release
calendar landed the same day (`data/news/events.csv`, 747 releases from
2010, `docs/news/README.md`).

So the first step was a measurement and not a registration:
`scripts/news_drift.py` on `xauduka` 2010-06 → 2018-06 — four event classes
across five windows, twenty cells, each against the same clock seven days
earlier. Nineteen said what everything else in this loop has said. One did
not, and it was registered with its multiplicity stated: the most extreme
of twenty looks, family-wise near 8%.

## What was measured

- The window: the open of the first 15-minute bar at or after **07:30 New
  York** to the open of the first at or after **08:30**, the release minute.
  Open to open, the way the engine fills.
- The null: 1,000 draws of the same number of days from every Friday in the
  window at the same **New York** minute — the real release days included,
  because removing them would make the null a sample of "days that were not
  announcements" and bias it by the effect being measured. It holds the day
  of the week and the hour fixed and varies only which Fridays carried a
  release.
- The instrument: `scripts/news_drift.py`, committed with the registration
  and **amended once after the reviews** (`436582c`, see below). Receipts in
  `docs/research/runs/2026-09-14-pre-nfp-drift/`, both the filed and the
  corrected readings.

## Evidence

```
window                         n      mean $/oz    t      up      percentile of the null
primary  2010-06 → 2018-06    96      -0.895    -2.23   32.3%    7.8th   (null -0.433)
test     2018-06 → 2026-05    90      -1.536    -2.90   35.6%    3.2nd   (null -0.109)
Vantage  2022-06 → 2026-09    48      -1.660    -1.76   33.3%   16.3rd   (null -0.148)
```

The three conditions, declared before the test window was read, judged on
the corrected instrument:

| declared | required | test window |
|---|---|---|
| percentile of the date permutation | ≤ 5th | **3.2nd** |
| mean move | ≤ −0.40 $/oz | **−1.536** |
| share of hours up | ≤ 42% | **35.6%** |

As filed, before the instrument was corrected, the same run read 0.0th
percentile, −1.470 and 34.0% on 94 releases. Both readings are on file.

The fall is concentrated where the mechanism would put it (news-desk, on
the test window): the half hour before the print carries −0.918 of the
hour's −1.536 — 62% of the fall in 50% of the time — while the hour before
*that* is positive and ordinary at the 66th percentile. The selling starts
at 07:30 and steepens into 08:30.

Leave-one-year-out on the test mean stays within [−1.22, −1.72]; no year
carries it (adversary).

## The instrument was wrong twice, and both reviews found it

Amended at `436582c`, before the record was written, and both readings kept:

1. **The null was clocked in UTC.** It took its minute from the first
   event's UTC stamp, so every fake Friday sat at one fixed UTC minute —
   08:30 New York in summer, 07:30 in winter. A third of the null measured
   an hour earlier and quieter than the hour it was controlling, which
   raised the null's mean and narrowed its spread. That is the whole of the
   difference between "0.0th percentile" and 3.2nd, and between the
   primary's 0.4th and 7.8th.
2. **A shut market was scored as a move of zero.** `open_at` walked to the
   next available bar however far away it was, so on a Good Friday both ends
   of the window resolved to the same bar and the move was recorded as
   exactly 0.0 — which `np.isfinite` was happy to keep. Four of the 94 test
   releases and fourteen of the 415 candidates were zeros of this kind. A
   bar more than one interval past the minute asked for is now no
   observation.

These are the sixth and seventh instrument faults this loop has logged
(`2026-09-13-instrument-faults.md`), and the first found on a result that
looked like a success rather than a failure. Every earlier fault made a
number worse; these made one better, which is the direction nobody checks.

## What each role said

- **adversary:** SURVIVED, with one number retracted. Provenance clean —
  the registration, the instrument and the exploratory receipts are one
  commit, and the test receipts were created after it. Its strongest
  objection is the one that landed: *"The registration's headline '0.0th
  percentile of 1000 permutations' is an artifact of a null clocked in UTC
  instead of New York… the primary would have failed the very falsifier it
  was registered under. All three declared gates still pass on the test, so
  the claim stands; the number quoted in the record must not."* Five seeds
  put the corrected test at 2.7–4.4th. On the confounds: a month-start tilt
  is real and eats roughly a quarter to a third of the gap — early-month
  Fridays average −0.368 against +0.082 late and +0.270 on all non-release
  Fridays — leaving **−1.10 $/oz unexplained**, and the same hour on the
  first Monday through Thursday of a month has no consistent sign. On
  multiplicity: *"Twenty cells were the primary's search; the test was one
  cell, three thresholds, committed first."* On the outliers it could not
  break the claim: *"the median and the up-fraction are outlier-proof and
  are the strongest statistics, not the weakest."* Its named falsification
  — a second feed putting the up-fraction above 44% — was run and came back
  at 33.3%.
- **data-integrity:** BLOCK on the receipts as filed, and the correction
  above is what it asked for. It reproduced the effect independently from
  the parquet to the digit (test −1.4702, t −2.899, 34.0%) and found both
  faults. On the calendar's side everything is clean: every one of the 203
  release stamps converts to exactly 08:30 New York in both seasons, the
  median gap between the minute asked for and the bar used is 0.0 minutes,
  and no release day but the four shut ones spans a hole. Two caveats the
  record carries: five of the 94 test releases are not Fridays at all
  (shutdown reschedules) and so have no counterpart in a Friday null; and
  its corrected figures — 3.64th and 7.72nd — are within seed noise of the
  instrument's own 3.2nd and 7.8th. Its sentence for the record: *"gold's
  08:30-NY hour before NFP fell about −1.5 $/oz over 90 usable releases at
  the 3.6th percentile of a correctly clocked Friday null — a real but
  thinner result than the receipts state, on a primary that does not
  survive its own control."*
- **news-desk:** CLEAN on the calendar, with one UNKNOWN. The dates are
  complete for both windows (the one missing month is October 2025, the
  shutdown, as the README records), the time is 08:30 New York in every
  year with nothing defaulted, and **no other scheduled release sits in the
  hour** on any of the 94 days. The UNKNOWN is the calendar's own scope:
  it holds five event names and two currencies, so Canada's Labour Force
  Survey — routinely released into the same 08:30 slot — is invisible to
  it. "No second event" means none this calendar can see. And the point
  only this role could make: the guards in `config/default.toml` flatten
  and refuse entries from 60 minutes before a high-impact USD release to 30
  after, so **the blackout is 07:30–09:00 New York and the claim's hour is
  its first half**. A bot that traded this hour would be trading inside its
  own news blackout.

## Reading

Twenty-six registrations closed before this one, and the shape was always
the same: an effect that is real on the window it was found on and gone on
the next. This one is different in the only way that counts — it was
declared with three numbers before the window was opened, and the window
agreed. It is also the first time a review has caught a fault in a result
that *flattered* the claim rather than buried it, which is the harder
direction and the reason the null is worth its cost.

But the corrected numbers say something quieter than the filed ones did.
The exploratory cell that justified the registration sits at the 7.8th
percentile once its own control is clocked properly — it would not have
been registered under its own rule. So the honest description is: a search
over twenty mis-controlled cells nominated a claim whose out-of-sample test,
correctly controlled, passes at the 3.2nd percentile on 90 releases, with
about a third of the gap explained by a month-start tilt and the rest not.
A second venue's feed agrees on the direction and the up-rate and cannot
reach significance on 48 releases.

And the size is the sentence that governs everything else. On the test
window gold's mean 20-day range is $30.45; one percent of $10,000 against
it is 3.3 ounces, so the effect is **$4.83 gross a release, $3.91 after the
quoted spread, about $46 a year** — and at the wider spread a real book
pays into an employment print, roughly half that. Twelve trades a year for
the price of a dinner, on an account that would have to sit through the
release to collect it. The record ends where the registration said it
would: this is a fact about the market, not a trade, and the loop's own
guards forbid the hour anyway.

## What would reopen this

- **A third window does not exist on this feed**, and the claim does not get
  one. What would strengthen it is different data, not more of the same: the
  same measurement on another instrument whose reaction to US employment is
  independent of gold's (the dollar index, a Treasury future) with the
  window and the null declared first, and the Canadian Labour Force Survey
  added to the calendar so "no second event" stops being a statement about
  the calendar's scope.
- **What would close it**: the adversary's month-start decomposition run as
  its own registration — if the unexplained residue falls below about
  −0.5 $/oz once the first-Friday tilt is removed properly, the claim is a
  calendar effect wearing a release's name.

## What this does not say

- It does not say there is a trade here. Three conditions passed and the
  fourth thing anyone would want — that it pays — was declared unreachable
  before the test and is: about $46 a year, less at a realistic spread.
- It does not say the exploratory window supports it. Corrected, that
  window is at the 7.8th percentile and fails the registration's own bar.
  The claim rests on the out-of-sample window and on the second feed's
  agreement about the direction, not on the cell that nominated it.
- It does not say the drift is the release rather than the day. About a
  quarter to a third of it is a month-start tilt on Fridays; the rest is
  unexplained, and "unexplained" is not "caused by the announcement".
- It does not say gold falls before every release. Nineteen other cells on
  the exploratory window said nothing, the session after NFP is a Friday
  afternoon and not a release, and the hour before FOMC and the ECB were
  suggestive and were not registered.
- It changes nothing about the ten paper books, and it is not a reason to
  lift the news guard that makes the hour untradeable.

## Amendment, 2026-09-15 (small hours): the null let the day of the month float

`2026-09-14-nfp-vs-first-friday` was registered to answer the question this
record left open, and it closed on its own instrument rather than on its
numbers — but two of its findings are about **this** record and are filed here
as well.

**1. The null holds the weekday and the minute fixed and lets the day of the
month float.** Nearly every release in this record's cells is the first Friday
of a month; the Fridays the null draws against are spread across all of it.
Gold's 07:30 → 08:30 hour is not flat at the turn of the month. Quiet Fridays
on days 1–3 read a median of −1.241 $/oz with 34.6% of hours up over 26 days,
against +0.321 and roughly 57% on quiet Fridays in the middle of a month. So
part of the gap this record measured is the position in the month and not the
announcement, and the 3.2nd percentile overstates what the release itself
contributes. Matched on the day of the month — release days against quiet days
on days 1–3 — the release adds **−0.742 $/oz at a Welch t of −0.45**, which is
not distinguishable from nothing. The adversary's review of this record put a
quarter to a third of the fall on a month-start tilt; measured properly with a
matched control, it cannot separate the two at all.

**2. The Fridays in the null are not all quiet.** news-desk checked 32
first Fridays that carried no US employment report and found **one** that was
an ordinary print-free trading day. Eleven carried another 08:30 New York
macro release the five-name calendar cannot see — Canada's Labour Force Survey
on five of them, US Personal Income and PCE on four, Canada's monthly GDP on
two. This cuts the opposite way to the first point: a null pool containing
falling days is a null whose mean is too low, which makes this record's
percentile conservative rather than generous. Both faults are real and they
push in opposite directions, and nothing here says which wins.

**What changes in the claim.** The sentence *"gold falls in the hour before
the US employment report by more than the same hour on other Fridays"* stands
— it is what was measured and it is what the test window agreed to. The
sentence this record does **not** support, and did not state carefully enough,
is the attribution: that the fall belongs to **the employment report
specifically**. The comparison Fridays are not matched on the day of the
month, and the ones that are matched carry other releases at the same minute.
The claim is now: *the hour before 08:30 on a US employment Friday falls, for
a reason this feed and this calendar cannot pin to the employment report.*

Nothing else in this record moves. The size is unchanged and still about $46 a
year, the out-of-sample window still agreed with three numbers declared before
it was read, the retraction of the 0.0th percentile still stands, and the news
guard still forbids the hour. The successor question — whether the hour falls
before **any** 08:30 release rather than this one — is on the backlog with the
calendar work it needs.

## Amendment, 2026-09-15 (morning): the claim did not replicate

`2026-09-15-nfp-cross-asset` spent the last out-of-sample data this loop owned
for this claim — silver and the euro, 2010–2026, never read for any news
question — on the three conditions declared in this record, and **neither
instrument confirms**.

The euro refuses outright: its hour is −1.198 pips on release Fridays against
−1.382 on quiet ones, so the release **adds +0.312 pips at a Welch t of +0.28**,
and the drift turns out to be the European-hours effect already closed in
`2026-09-14-fx-local-hours.md` rather than anything to do with employment.

Silver is the harder case and the one this amendment exists for. Its hour does
fall — 29.9% of 184 release hours up, an exact sign test at p = 4.9 × 10⁻⁸,
robust to units, trimming and leave-one-year-out — but it **fails condition 1**,
the percentile gate this record was judged on. Measured over 400,000 draws
rather than 1,000, silver sits at **6.02 ± 0.04** against a gate of 5.0; the
4.8 that the declared command printed is a one-in-four seed draw, and three
quarters of seeds print a failure. By the rule written before the window was
opened, that is silver refusing.

And the adversary established the thing that governs how much any of this was
ever worth: in a simulated world where silver is **only** gold times its
ordinary quiet-Friday beta plus its own noise, all three of this record's
conditions pass **79.4%** of the time. Silver was never an independent witness
— its excess over gold's beta is +0.0176 $ at t +1.67 in raw units and −2.06
basis points at t −0.61, a sign that flips with the unit. A confirmation would
have carried about a third of a bit.

**What changes here.** This record's claim stands as a measurement and loses
its last support as a general fact. The sentence it may now carry is: *gold's
07:30 → 08:30 hour falls before the US employment report on 2010–2026
Dukascopy bars, at the 1.37th percentile of a null that does not control the
day of the month, and it did not reproduce on the only out-of-sample
instrument capable of reproducing it independently.*

Two things do not change. The size is still about $43 a year at 1% risk —
recomputed here with the short's sign and the engine's own sizing, and silver's
version is worth $16 because its spread eats 78% of its own gross effect. And
the news guard still blacks out 07:30–09:00 New York, so none of it was ever
tradeable by this repository's own rules.
