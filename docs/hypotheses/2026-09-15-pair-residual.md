# 2026-09-15-pair-residual: does silver's move against gold come back?

**Registered:** (commit time is authoritative) — before `2018-06-16 → 2026-05-31`
is read for this question
**Status:** registered
**Instrument:** `scripts/pair_residual.py`, committed with this file

## Why this family, when twenty-nine registrations have died

Every family this loop has closed asks one instrument about itself — a level, a
range, a session, a trend, a release. This is the first that needs two
instruments to exist at all, so it is a new mechanism and not a new window on a
closed one.

The idea came out of the wreck of `2026-09-15-nfp-cross-asset`. The adversary
proved silver was not independent evidence of gold's pre-release fall by
fitting silver's beta to gold on quiet Fridays and showing that beta explained
almost everything. Read the other way, that is a signal: whatever is left when
gold's move is removed is silver's own, and if it is noise rather than news it
should come back.

## What was measured, and the multiplicity

`scripts/pair_residual.py` on `xauduka` × `xagduka`, 15-minute bars,
**2010-06-01 → 2018-06-15**, which is the exploratory half and can never be the
test. Beta over a trailing 20 trading days, the deviation accumulated over a
trailing window, the z-score scaled by a trailing 20-day dispersion, all of it
shifted so that nothing standing at a bar was unknown at that bar. Trades are
non-overlapping: a signal is taken and nothing fires again until the pair is
off, because an overlapping sample counts the same four hours up to sixteen
times.

**Twenty-seven cells were looked at** — twelve in the first grid (two deviation
windows × two thresholds × three horizons) and fifteen in the second (five
thresholds × three horizons at the better deviation window). That is the
multiplicity and it is stated here rather than discovered later.

What the twenty-seven say is consistent rather than extreme: **the gross
reversion is positive in every one of the fifteen cells of the second grid and
in nine of the twelve of the first, and the win rate is above 51.8% in all
fifteen**, in a range of 54% to 58%. It also rises with the size of the
deviation the way the mechanism says it should — 3.18 bp at a z of 1.5 against
6.72 bp at a z of 4.0 on a four-hour hold — while the *win rate* does not,
which says the larger number is a larger move and not a more reliable one.

## Claim

On `xauduka` × `xagduka`, **2018-06-16 → 2026-05-31**, the deviation reverts:
after the residual return accumulated over four hours reaches a z-score of 2.0,
the next four hours give it back more often than a coin would.

## The cell, chosen on principle and not on its number

**Deviation window 4 hours, threshold z = 2.0, horizon 16 bars (four hours).**

It is the middle of the grid on both axes. It is not the maximum of any column:
the best gross is at z = 4.0 and the best t-statistic at z = 1.5. It has 1,785
non-overlapping trades on the exploratory half, which is the most power
available anywhere in the grid at a threshold that means anything. Its
exploratory reading is gross **+3.832 bp**, median +6.848, win **57.6%**,
t +2.47.

## Falsifier

From
`python scripts/pair_residual.py 2018-06-16 2026-05-31 --dev-hours 4 --threshold 2.0 --horizon 16`:

1. The mean gross reversion is **positive**.
2. The win rate is **≥ 53.5%** — the exploratory 57.6% is 7.6 points above a
   coin, and this halves that excess, the same discipline the adversary
   demanded of `2026-09-14-fx-local-hours-sign`.
3. An exact two-sided sign test against a fair coin gives **p ≤ 0.01**.

Any one failing closes the family. At roughly 1,700 expected trades a 53.5% win
rate is a sign test at about p = 0.004, so condition 2 and condition 3 are
close to the same gate and both are **powered** — which two of the last three
registrations in this repository were not.

**The draws are not a question here.** There is no permutation percentile in
this falsifier and therefore no seed. That is deliberate: fault 11 in
`docs/decisions/2026-09-13-instrument-faults.md` is a gate placed at a
precision its estimator could not deliver, and the way to not repeat it is to
gate on statistics that have closed forms.

## The economics, declared in advance and not revisable afterwards

**No outcome of this registration is a strategy, a paper run, or a reason to
change any of the ten paper books.** The exploratory window already settles
that, and it is written here so that a passing sign cannot later be read as
something it is not.

A pair pays two spreads to get in and out and the true historical cost is
unknowable, so it is bracketed:

| | gold | silver | round trip |
|---|---|---|---|
| **proportional** — today's quote over today's level | 0.62 bp | 2.78 bp ($0.021 at $75.56) | **3.40 bp** |

> **Correction, same day, before the reviews reported** (data-integrity): this
> row first read 0.49 bp and 3.27 bp. That came from the live terminal quote of
> $0.21 over the live price of $4,296 on 2026-09-15, which is a different level
> from the one the data ends at. The instrument uses $0.28 over the span's last
> close of $4,539.335 = 0.617 bp, and both receipts print 3.40. **The numbers
> the instrument actually uses are the ones above.** Nothing depends on it — the
> net is negative at either — but a committed document carrying a number the
> instrument does not use is the shape of fault 10 and is corrected in place
> rather than in a commit message.
| **dollar-constant** — today's quote over the span's mean level | 1.62 bp | 8.64 bp | **10.26 bp** |

Against that, the best cell in twenty-seven nets **+3.3 bp at the optimistic
bound and −3.5 bp at the pessimistic one**. And the product of trade count and
net edge is almost constant across the grid: 223 trades a year at +0.43 bp and
29 trades a year at +3.32 bp are both about **96 basis points a year on the
silver leg's notional**, with the gold leg requiring another 1.44× of it. Call
it **0.4% a year on deployed capital at the optimistic cost bound, and negative
at the pessimistic one**, before slippage, financing, or the fact that both
legs must fill.

The family is therefore being tested as a **fact about the two instruments**,
not as a candidate for the desk.

## Data

- `xauduka:15m` × `xagduka:15m`, inner-joined on the bar stamp so that no
  residual is ever computed against a stale leg.
- Exploratory: 2010-06-01 → 2018-06-15, 190,261 aligned bars. Read, and it can
  never be the test.
- Test: 2018-06-16 → 2026-05-31, never read for this question. Both files were
  fetched 2026-09-13 and have been used since only for
  `2026-09-15-nfp-cross-asset`, which touched 184 release hours and nothing
  else.

## Two faults this instrument already had, fixed before anything was registered

Recorded because the loop's habit is to suspect the instrument first, and twice
in an hour it was right.

1. **The spread was built on log levels.** `log(silver) − beta × log(gold)`
   carries `log(gold) ≈ 7.2`, so a beta that moved by 0.01 moved the spread by
   72 basis points on its own. The first run measured beta's drift, called it
   divergence, and printed reversions of −400 bp. The deviation is now an
   accumulated residual **return**, in which no level appears.
2. **Trades overlapped.** A signal fired on 11% of bars and was held for
   sixteen, so the same four hours were counted up to sixteen times and every
   t-statistic was inflated by roughly four. Only non-overlapping trades are
   counted now, which is also the only version a book could take.

## What each outcome means

- **All three hold** → the gold–silver residual reverts, out of sample, on a
  powered test. That is the first mechanism in this repository to survive a
  pre-registered out-of-sample test with real power, and it is worth about
  0.4% a year at the optimistic cost bound. The record says exactly that and
  proposes nothing.
- **Any fails** → the family closes with the other twenty-nine, and the
  consistency across twenty-seven exploratory cells goes in the record as
  another instance of the loop's one recurring finding: a real effect on the
  window that produced it and nothing on the window that did not.
