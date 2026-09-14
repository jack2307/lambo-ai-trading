# The claim did not replicate, and the test that would have confirmed it was worth a third of a bit

**Date:** 2026-09-15 (morning)
**Question:** Gold's 07:30 → 08:30 New York fall before the US Employment
Situation is the only claim in twenty-eight registrations to survive its
falsifier. Does it appear on instruments that had never been read for any news
question? Registered at `81cdee5` with the three conditions, before a bar of
`xagduka` or `eurduka` was read.
`docs/hypotheses/2026-09-15-nfp-cross-asset.md`.
**Outcome:** **Neither instrument confirms. The claim did not replicate.** The
euro refuses on two of three conditions and explains itself. Silver passes the
two conditions with power and **fails condition 1** — its permutation
percentile is **6.02 ± 0.04** measured over 400,000 draws against a declared
gate of 5.0, and the 4.8 the declared command printed is a one-in-four seed
draw. The registration's own sentence is *"any one failing is that instrument
refusing"*, so silver refused. And the adversary showed the test was thinner
than it looked even as a pass: in a world where silver is nothing but gold
times its ordinary beta plus noise, all three conditions pass **79.4%** of the
time. Two instruments were one.

## What was read

```
market     n     mean          t      up     sign p     percentile (400k draws)
xagduka   184   -0.038592   -4.41   29.9%   4.9e-08     6.023  +- 0.038   FAIL
eurduka   189   -0.000120   -1.28   46.0%   0.309      55.172  +- 0.079   FAIL
xauduka   186   -1.204973   -3.66   33.9%   1.3e-05     1.370  +- 0.018   (the claim)
```

| condition, declared before reading | silver | euro |
|---|---|---|
| 1. mean at or below the 5th percentile of the date permutation | **6.02 — fail** | 55.2 — fail |
| 2. share of hours up ≤ 42% | 29.9% — pass | 46.0% — fail |
| 3. mean negative | −0.0386 — pass | −1.20 pips — pass |

data-integrity reproduced every filed figure to the digit from a reader with
different mechanics, and so did the adversary, independently of each other and
of the instrument. Both of the parent's instrument faults are provably absent:
every one of the 184, 189 and 186 windows resolves to 07:30 and 08:30 New York
wall clock in both seasons at a gap of **exactly zero minutes**, with no
exact-zero moves, across all 31 daylight-saving transitions in the span.

## The seed, which is the whole decision

The declared command prints one number from 1,000 draws of a Monte-Carlo
estimator whose standard error at that size is 0.74 percentage points. The
quantity it estimates — the 5th percentile of the permutation distribution —
does not depend on a seed. The estimate does.

| | five declared seeds | 500 seeds, mean ± sd | 400,000 draws | seeds clearing ≤ 5.0 |
|---|---|---|---|---|
| silver | 4.8 – 6.3 | 5.56 ± 0.74 | **6.02** (this record) / 5.59 (adversary) | **26%** |
| gold | 0.9 – 1.5 | 1.23 ± 0.36 | 1.37 | 100% |
| euro | 52.0 – 57.8 | 55.20 ± 1.54 | 55.17 | 0% |

The adversary's big run and this record's differ by 0.4 of a percentage point
on how the pool's missing candidates are dropped, and it does not matter: both
sit above the gate with a standard error under 0.04, and three quarters of
1,000-draw seeds print a failure. Gold's 1.37 and the euro's 55.2 are
seed-proof. Only silver's sits on the line, and on the wrong side of it.

**The number was reported as seed-dependent before either review ran and the
pass was never re-rolled.** That is the only thing to be said for how it was
handled. Reporting a fragile gate is not the same as having a gate.

## Silver's fall is real and silver is not a second instrument

Both of these are true and the record needs both.

**Real.** Silver's hour fell on 129 of 184 releases. The sign test against a
fair coin is p = 4.9 × 10⁻⁸. It survives every cut either review applied:
normalising to basis points makes it **stronger** (−14.66 bp at t −4.81 against
gold's −6.33 at t −3.46) and so does normalising to the trailing range (−0.0630 R,
t −4.97); leave-one-year-out keeps t in [−5.01, −3.82]; a 10% trimmed mean is
−0.0344; dropping the three largest falls leaves −0.0273 at t −4.08; the
2010–2023 sub-span alone is 34.0% up at sign p 0.0001; both halves are
negative separately. Sixty-three percent of the fall lands in the final quarter
hour, which is where the stated mechanism puts it. The hour before it is flat.

**Not a second instrument.** The adversary ran the test the registration should
have declared and did not. Fit silver's beta on quiet Fridays only, where no
release information exists, then simulate silver as beta × gold's actual
release-day moves plus a resampled quiet-Friday residual, and judge those
simulated worlds by the three declared conditions:

| | passes |
|---|---|
| condition 1 | 85.4% |
| condition 2 | 89.3% |
| condition 3 | 100% |
| **all three** | **79.4%** |

Given that gold had already fallen, "silver confirms" was four-to-one on before
any bar was read — about a third of a bit. And silver's excess over gold's beta
is not there: the mean residual is **+0.0176 $ at t +1.67** in raw units, where
a positive residual means silver fell *less* than gold implied, and **−2.06 bp
at t −0.61** in basis points. The sign of the excess flips with the unit, which
is what "no signal" looks like. Silver's proportional fall is 2.03× gold's
against an ordinary quiet-Friday beta of 1.66: a high-beta gold, not a second
witness.

## The euro refuses, and says why

The euro's hour is −1.198 pips on release Fridays and −1.382 on quiet ones. The
release **adds +0.312 pips at a Welch t of +0.28**. The drift is not even
Friday-specific: the same hour reads +0.10, −0.91, −0.51, −1.44 and −1.31 pips
from Monday to Friday. This is the European-hours drift this loop closed on
2026-09-14 (`2026-09-14-fx-local-hours.md`), and the employment report adds
nothing to it. The adversary chased the one sub-window that could have looked
like a confirmation — 07:45 → 08:00, −1.634 pips at t −3.56 on release days —
and found quiet Fridays read −1.776 there, so the release makes the euro fall
*less*.

One thing the euro did show, and it is a question and not a finding: split on
whether Canada's Labour Force Survey also printed at 08:30, the euro reads
−4.28 pips with 33.9% of hours up on the 62 mornings with only the US report,
and +1.25 pips with 53.3% up on the 105 with both. It is not a time split —
restricting to releases after Canada moved the LFS to 08:30 in April 2012
leaves it intact. It is a post-hoc two-way cut found after the aggregate
failed, there is no unread data left to test it on, and it is recorded as a
loose end rather than a rescue.

## The overlap, which data-integrity was right to raise

111 of the 191 measured hours carry a second 08:30 New York print, 107 of them
Canada's jobs report. So "NFP days" are mostly "both jobs reports on one
morning", and the test's calendar cannot separate them. The decomposition
answers it, and on the metals it answers the strong way — restricted to
releases from 2012-04-05, where both groups can exist:

```
                          silver                    gold
NFP with Canada's LFS   n 104  -0.0289  33.7%    n 106  -1.3247  33.0%
NFP alone               n  58  -0.0583  20.7%    n  58  -1.1227  34.5%
Canada's LFS alone      n  64  +0.0288  48.4%    n  64  +0.5127  54.7%
```

A morning with only the US report is if anything stronger; Canada's report
alone does nothing on either metal, which is the same answer gold gave when the
question was asked of it directly. The overlap is not what moves the metals.

## The money, which would have ended it anyway

The short, sized the way every paper book is sized — 1% of equity against the
trailing 20-day range — with one measured Vantage spread per round trip:

| | R | gross | spread | net | per year |
|---|---|---|---|---|---|
| gold | +0.0523 | $5.23 | $1.61 | $3.62 | **$43** |
| silver | +0.0630 | $6.30 | $4.92 | $1.37 | **$16** |

Silver's hour is 2.3× gold's in proportional terms and worth a third as much,
because silver's spread is 78% of its own gross effect where gold's is 31%.
The instrument that was supposed to confirm the claim would have made it
smaller.

## What each role said

- **adversary — BROKEN.** *"Silver's permutation percentile is 5.59 ± 0.04 over
  400,000 draws — the declared command's 4.8 is a one-in-four seed draw from a
  quantity that fails the 5th-percentile gate on 74% of seeds — and in a null
  where silver is nothing but gold times its ordinary quiet-Friday beta plus
  its own noise, all three declared conditions pass 79.4% of the time, so the
  registration's own sentence ('any one failing is that instrument refusing')
  says silver refused, and its own arithmetic says a pass would have carried
  almost no information even if it had cleared."* It verified provenance
  itself: `81cdee5` precedes every receipt by seconds, the instrument is
  byte-identical to `436582c`, both parquet files are untouched since
  2026-09-13, and the pre-declared sample counts of 184 and 189 are exactly
  right. It also found the thing this record owes most to it — see fault 10.
- **data-integrity — CLEAN.** Independent reproduction with different mechanics
  throughout, zero discrepancies on any figure for any instrument. Its
  sentence: *"this data can support the statement that the hour is a metals
  effect and not a dollar effect, and cannot support attributing it to the US
  employment release specifically, because 111 of the 191 measured hours also
  contain another 08:30 macro print the test's calendar cannot see."* It also
  established that the nine dropped days are four metals holidays plus five
  whole-day feed holes, that dropping them cannot change either verdict (the
  euro's four extra observations are all down, which makes its failure worse),
  and that silver's t is not a units artefact — normalisation makes it larger.

## Fault 10, which the adversary found and which is the worst kind

A committed receipt contained known-wrong numbers with the correction living
only in a commit message. `cells-and-robustness.txt` printed fifteen
percentiles computed from midnight-New-York stamps — the hour before midnight,
not the hour before the release — because the wrapper passed
`friday_cells.load_calendar`'s normalised dates into `news_drift.permutation`.
I caught the fault myself, said so in the commit, re-ran the percentiles
correctly, and left the wrong ones sitting unmarked in a file that reads as if
silver's condition 1 passed at the 0.5th percentile: an order of magnitude
better than the truth. The adversary reproduced all fifteen wrong numbers by
feeding midnight stamps, which is how the diagnosis was confirmed rather than
assumed, and then said the thing that matters: *anyone quoting that receipt in
six months quotes a bug.* The file now carries the correction at the top of
itself. **A receipt is not corrected by a commit message.**

## Reading

The registration asked the one question this loop still had data to answer, and
got the answer its own history predicted. Twenty-eight registrations, one
survivor, and the survivor does not replicate.

What makes this a clean ending rather than a grudging one is that the
registration priced the outcome in advance. It declared three conditions and
the rule for reading them, it declared condition 1 as the faulted one, it
declared the confound-free cell as direction-only with its 21% false-pass rate,
and it declared that the expected outcome was refusal. Every one of those
declarations did work here. The one thing it failed to declare — how much
information a silver confirmation would have carried given gold — is the
adversary's contribution, and it turns out to be the largest single fact in the
run: 79.4%, four-to-one on, a third of a bit. **A confirmation from a
correlated instrument is not a confirmation, and the registration should have
said so before it went looking.**

The claim that stands after all of this is narrower than the parent's and
smaller: gold's 07:30 → 08:30 hour falls before the US employment report on
2010–2026 Dukascopy bars, at the 1.37th percentile of a null that does not
control the day of the month, worth about $43 a year at 1% risk, inside a
blackout the bot already enforces, and **not reproduced on the only
out-of-sample instrument that could have reproduced it independently**.

## What would reopen this

- The adversary's experiment, and it is the only one left that could settle
  anything: **pre-register a residual gate on an instrument still unread** —
  platinum, copper, or a dollar-side pair. Fit beta on quiet Fridays only,
  where no release information exists, then gate the sign and the up-rate of
  the *residual* of (instrument_bp − β·gold_bp), not of the instrument itself.
  Run the permutation at 100,000 draws, where the standard error is 0.07
  percentage points instead of 0.74, so the percentile stops being a lottery.
  A residual that falls is a second factor. A residual at 50% is what silver
  gave, and it closes the thread for good.
- The euro's post-hoc split, if a feed ever appears that has not been read.
  There is none on this machine.

## What this does not say

- It does not say silver's hour does not fall. It does, on 184 releases at
  sign p 4.9 × 10⁻⁸, and that fact survives every robustness cut two reviews
  could apply.
- It does not say the parent was wrong. It says the parent did not replicate,
  which is a different and weaker statement, and it is the statement filed
  against the parent's record.
- It does not say a 1,000-draw permutation is useless. It says a gate placed at
  5.0 cannot be judged by an estimator with a standard error of 0.74, and that
  every future percentile gate in this repository needs its draws set by the
  precision the gate requires.
- **It changes nothing about the ten paper books.** The news guard still
  blacks out 07:30–09:00 New York, which forbids every hour in this record, and
  nothing here is a strategy or a paper run.
