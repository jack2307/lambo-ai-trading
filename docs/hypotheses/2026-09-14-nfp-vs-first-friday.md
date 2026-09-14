# 2026-09-14-nfp-vs-first-friday: is the pre-NFP hour the release, or the first Friday?

**Registered:** (commit time is authoritative) — the script is committed with
this file and has not been run
**Status:** decided — `docs/decisions/2026-09-14-nfp-vs-first-friday.md`. Both
declared conditions passed and neither measured what it was declared for: the
falsifier compares cell B to cell C while the claim compares B to D, and a
day-by-day pass found that **one** of cell B's 32 days is an ordinary
print-free trading day. **The claim is not supported and the cell table below
is wrong** — it sums to 868 against 835 Fridays in the span, because D was
written as all later Fridays including C's releases and the busy days. Neither
`D = 643` nor `C = 33 releases` may be quoted. What survived is the one clean
cell: 25 releases on a later Friday, at the 0.2nd percentile of a
day-of-month-matched control.
**Instrument:** `scripts/friday_cells.py`
**Parent:** `docs/decisions/2026-09-14-pre-nfp-drift.md`

## What this is, and what it can never be

`2026-09-14-pre-nfp-drift` closed as the loop's first surviving registration:
gold's 07:30 → 08:30 New York hour falls on US Employment Situation days at
the 3.2nd percentile of a Friday null, on a window declared before it was
read. Its own record named the thing that would close it, and this is that
thing.

**Both windows on this feed have now been read for the parent question.** So
this registration is declared, in advance, to be **explanatory and not
confirmatory**:

- No outcome of it can strengthen the parent claim, raise its percentile, or
  turn it into a strategy. It was already worth about $46 a year and the
  loop's news guard already forbids the hour.
- The only outcomes that change anything are the ones that **weaken** the
  parent: if the fall belongs to the calendar rather than the release, the
  parent's record gets an amendment saying so, and the hour stops being about
  employment.
- The multiplicity is stated here rather than discovered later: this is a
  second look at data already read once, and a percentile computed on it is
  not the same object as one computed on an unread window.

## Where the question comes from

The adversary's review of the parent, quoted in its record: early-month
Fridays average −0.368 $/oz over this hour against +0.082 on late-month
Fridays and +0.270 on non-release Fridays, which puts roughly a quarter to a
third of the −1.536 on the calendar and leaves about −1.10 unexplained. That
decomposition was a side-calculation inside a review. It deserves its own
instrument and its own declared reading.

## The identification

The Employment Situation is released on the third Friday after the reference
week, which is **usually** the month's first Friday and **sometimes** its
second. The calendar therefore separates the two explanations by itself, a
few times a year, and the separation is a fact about the schedule that was
counted before this file was written and does not depend on any price:

|  | release | no release |
|---|---|---|
| **first Friday** | **A** — 158 days | **B** — 34 days |
| **later Friday** | **C** — 33 days | **D** — 643 days |

(2010-06 → 2026-05, both windows; `B` and `D` exclude any day carrying another
scheduled high-impact release, so a "quiet Friday" is quiet.)

- If the fall is **the release**, A and C fall and B and D do not.
- If the fall is **the first Friday of the month**, A and B fall and C and D
  do not.
- If both, all three of A, B, C fall and A falls hardest.
- If neither, the parent was a coincidence of 90 observations and says so.

## Claim

The hour's fall follows the **release**, not the position in the month: cell C
(a release on a later Friday) is negative, and cell B (a first Friday with no
release) is not materially more negative than the quiet baseline D.

## Falsifier

Computed by
`python scripts/friday_cells.py xauduka 2010-06-01 2026-05-31 --draws=1000`:

1. **C is negative** — mean ≤ 0 on its 33 releases.
2. **B is not the story** — B's mean is above (less negative than) C's.

Both must hold. Either failing means the calendar explains at least as much
as the release does, and the parent record is amended to say the hour is not
cleanly about employment.

**The power is declared unreachable, in advance, and this is the important
sentence in the file.** The parent's own numbers give a standard deviation of
about 5.0 $/oz on this hour. On 33 observations the standard error is 0.88,
so an effect of the parent's size (−1.5) reaches t ≈ −1.7 at best and the 5th
percentile of a permutation null is **not attainable**. The two conditions
above are therefore **an ordering test, not a significance test**: they ask
which of two explanations the sign and the size point at, and they cannot
settle it. A percentile is printed for C and for B because it is informative
about direction, and it is **not** a gate here — quoting it as one would be
the mistake this loop exists to avoid.

## Data

- `xauduka:15m` 2010-06-01 → 2026-05-31. Both windows, deliberately pooled:
  the identifying cells are 16–18 days per window and splitting them would
  leave nothing to read. Pooling is legitimate **only** because this is
  explanatory; the parent's confirmatory test kept them apart and stays that
  way.
- `data/news/events.csv`, scheduled layer only. The README's known gap
  (October 2025, the shutdown) removes one release from A.
- The calendar holds five event names and two currencies, so "no release"
  means none this calendar can see — the same UNKNOWN news-desk raised on the
  parent, and Canada's Labour Force Survey shares the 08:30 slot.

## Sample needed

33 in C and 34 in B, both fixed by the schedule and both too small, as above.
No further data exists on this feed to grow them.

## What each outcome means

- **C falls, B does not** → the parent's hour is the release. The record says
  so and the claim keeps the description it already has, no stronger.
- **B falls, C does not** → the parent measured the first Friday of the month
  and named it after an announcement. The parent record is amended and the
  claim is downgraded to a calendar effect.
- **Both fall** → the honest reading is a calendar tilt plus something on the
  release, and neither cell can say how much is which. The parent record
  carries that sentence and nothing more.
- **Neither falls** → the parent's 90 observations were the only place this
  lives, which for a claim already worth $46 a year is the end of it.
