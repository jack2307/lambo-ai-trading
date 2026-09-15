# 2026-09-15-selftest-audit — the two scripts holding live findings, tested rather than trusted

**Written:** 2026-09-15
**Status:** an instrument audit, not a hypothesis. Nothing here changes a
verdict; two findings the loop is carrying are confirmed to be the market's and
not the harness's, and one open fault is measured and found immaterial.
**Receipts:** `docs/research/runs/2026-09-15-selftest-audit/`

## Why

`2026-09-15-quote-asymmetry` shipped three faults inside one file in one day.
One of them — a null that computed `mean(sign × sign × raw)` and therefore
permuted nothing — left every other column looking perfectly reasonable. Had it
landed the other way it would have produced a percentile out of a quantity with
no direction in it.

Every closure in this loop rests on a script with no test of that class. For a
closure a bug can only bury an effect; for a **surviving** finding it can
manufacture one, and the loop has two of those. They are audited here.

The method is the one `2026-09-15-quote-asymmetry` established: **plant
something of known size and check the instrument finds it, then feed it
nothing and check it stays quiet.**

## 1. `news_drift.py` — the loop's only survivor

`2026-09-14-pre-nfp-drift`: gold's 07:30 → 08:30 New York hour before the US
employment report, at the 3.2nd percentile of a date-permutation null on
2018-06 → 2026-05. A bug in that permutation would have created it.

**A. Recovery.** A drift of exactly δ dollars planted into the hour before each
real release — δ subtracted from every bar stamped in `[event − 60m, event)`,
and nowhere else, so no other window is touched.

```
    planted $    n  mean move  recovered    up%  pctile
        +0.00   91    -1.5404    +0.0000  35.2%    2.70
        -1.00   91    -2.5404    -1.0000  24.2%    0.00
        -0.50   91    -2.0404    -0.5000  28.6%    0.90
        +0.50   91    -1.0404    +0.5000  41.8%    8.70
        +1.00   91    -0.5404    +1.0000  51.6%   17.60
```

Recovered to the penny at every level, and the percentile is monotone. The
δ = 0 row is the real data and reproduces the published finding.

**B. Calibration — the test that matters.** Real bars, untouched; **fake**
release dates drawn from the pool the null draws its own controls from (same
weekday, same New York minute, same span). Twenty sets:

```
   min 15.70   median 50.35   max 86.20   mean 48.86
   below the 5th percentile: 0 of 20 (expected about 1.0)
   below the 50th: 10 of 20 (expected about 10)
```

Uniform. **The 3.2nd percentile is a property of the release, not of the
instrument.** The loop's single survivor stands.

## 2. `friday_cells.py` — the number the loop kept quoting

`2026-09-14-nfp-vs-first-friday` closed, and left one figure alive: 25 releases
on a later Friday, at the 0.2nd percentile of a **day-of-month-matched** control,
20% of hours up against 53%. Twenty-five observations and a percentile that
extreme is the combination a bad null invents on its own.

**A. Calibration.** Forty fake release assignments — the same number of first
Fridays and later Fridays, drawn at random — so cell C becomes a random subset
of later Fridays and its percentile against D must be uniform:

```
   min 0.40   median 40.62   max 97.90   mean 44.63
   below the 5th: 1 (expected 2.0);  below the 1st: 1 (expected 0.4)
```

Within noise at both tails. The harness does not manufacture extreme
percentiles. The mild tilt below 50 in the mean is worth one line and no more.

**B. Fault 9, measured instead of asserted.** The backlog has carried this since
2026-09-13: `friday_cells.py` builds its cells as `if d in release: A or C;
elif d not in busy: B or D`, so a Friday carrying a **second** high-impact
release is excluded from the controls and kept in the treatments — a screen
applied to one arm of a comparison.

```
   as published (screen on controls only): C n=25 mean -1.734 up 20.0% pctile 6.55
   screen on both arms:                    C n=25 mean -1.734 up 20.0% pctile 6.55
   later-Friday releases that also carry a second high-impact event: 0
```

**Zero of the twenty-five.** The fault is real in the code and stays on the
backlog, because it will bite a different cell or a different window. It does
not touch the finding the loop is carrying: the number moves by 0.00.

## 3. What the audit found that was not being looked for

**The as-filed percentile is seed-sensitive at the precision it is quoted.**
Cell C against quiet later Fridays reads **4.7** (the script's own defaults,
seed 7, 1,000 draws), **6.1** (the decision record), and **6.55** (seed
20260915, 2,000 draws). Two percentage points of wander on a figure printed to
one decimal. Nothing turns on it here — the record uses the day-of-month-matched
null for its claim and says so — but it is the same fault as backlog item
"every percentile gate states the precision it requires", now with a number.

**A suspicion of mine, raised and dismissed.** `news_drift.py` compares
`times[i] - when_ns > TOLERANCE_NS` against a **nanosecond** constant, and these
bar files are `datetime64[ms]` — the unit trap that emptied two cells in
`quote_asymmetry` the same day. It is not present: the script casts explicitly
with `.astype("datetime64[ns, UTC]")` before converting to int64, and pandas'
`.value` is nanoseconds on the other side. The fault was in the audit script,
not the audited one, and it is recorded here so the next reader does not have to
re-derive it.

**The audit scripts shipped two faults of their own** — an early return with the
wrong arity, and a symmetric screen written against `busy` (which holds every
high-impact date **including the releases themselves**, so it emptied cell C
entirely and reported "25 dropped"). Both were caught by reading the output
rather than the code. A test is an instrument and is a suspect like any other.

## What this does not cover

`pair_residual.py`, `fx_window_drift.py` and `venue_residual.py` have no
self-test. They hold closures rather than live findings, so a fault there buries
rather than invents — the lower-stakes direction, but not a harmless one: the
gold–silver overnight dislocation is still an open observation and rests on
`pair_residual.py`.

## What it buys

Two of the loop's live numbers are now backed by a receipt showing the
instrument that produced them can find what is planted in it and stays quiet on
what is not. That is the difference between believing a result and having tested
the thing that produced it.
