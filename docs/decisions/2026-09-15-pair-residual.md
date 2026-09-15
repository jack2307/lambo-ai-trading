# I registered the maximum and called it the middle, and the four hours were not four hours

**Date:** 2026-09-15 (afternoon)
**Question:** Does silver's move against gold come back? Beta fitted causally
on a trailing 20 trading days, the deviation accumulated over a trailing four
hours, the pair on at the next open when |z| ≥ 2.0 and off four hours later,
trades non-overlapping. Registered at `0a9e4a3` with the instrument, six
seconds before the test window was read.
`docs/hypotheses/2026-09-15-pair-residual.md`.
**Outcome:** **Closed on the registered claim.** All three declared conditions
passed on the window as written, and two independent reviews then showed that
what passed is not what was registered. The lookback and the hold are counted
in **bars**, and the tape has a daily break — so on the 1,257 of 1,706 trades
where four hours really means four hours, the cell reads **+0.287 bp at 52.51%
won with an exact sign test of 0.0803**, failing two of the three conditions.
And the cell itself was the **argmax of the win rate across all twenty-seven
exploratory cells**, which is the exact statistic both surviving gates are
functions of, while the registration told the reader it was the middle of the
grid. Something real is left over, it is overnight, and it is not this.

## What passed, and the two sentences that undo it

```
declared cell, test window 2018-06-16 -> 2026-05-31
   1,706 non-overlapping trades   gross +3.108 bp   median +4.824   win 54.6%
   exact two-sided sign test p = 1.733e-04     mean's own t = +1.675
```

| condition, declared before reading | as run | on honest four-hour trades |
|---|---|---|
| 1. gross positive | +3.108 — pass | +0.287 — pass |
| 2. win rate ≥ 53.5% | 54.57% — pass | **52.51% — fail** |
| 3. exact sign test p ≤ 0.01 | 1.73e-04 — pass | **0.0803 — fail** |

data-integrity reproduced every printed figure to the last digit from a reader
sharing no code with the instrument, confirmed the sign test by integer
arithmetic in `Decimal` and `Fraction`, and proved causality bitwise by
rebuilding the whole pipeline on three truncated series — every value,
**including the one standing at the truncation bar**, identical. The adversary
reimplemented the instrument from the registration's prose rather than its code
and also got every digit, then ran the check that matters most: entering at
`open[i]` instead of `open[i+1]`, i.e. actually peeking, gives **−21.264 bp at
36.10% won**. A leak flips that positive. It is negative. **The code is clean.**

## Fault 12: a horizon counted in rows, on a tape with a hole in it

`dev_bars` and the horizon are bar counts, so both the lookback and the hold
stretch silently across Dukascopy's daily one-hour break, a weekend, or a day
on which one leg has no feed at all.

```
test window, the declared cell                 n     gross    win     sign p   share of gross
hold exactly 240 minutes                    1,452   +1.710  53.51%   0.0080         47%
hold stretched by a break, weekend or hole    254  +11.103  60.63%   0.0008         53%
both the window AND the hold clean          1,257   +0.287  52.51%   0.0803          7%
either one stretched                          449  +11.008  60.36%   1.3e-05        93%
```

Fifteen percent of the holds are not four hours of market and they carry more
than half the gross; require the deviation window to be clean as well and
**93% of the gross sits in 26% of the trades**. Seven trades hold straight
across a day on which one of the two instruments had no feed at all, and five
of those seven average **+91.2 bp**.

The rule this writes: **a window measured in bars is a window measured in a
quantity the market does not have.** Every horizon in this repository is
currently a bar count, and on a tape with a daily break and a weekend that is
not the same thing as a duration.

## Fault 13: the registration checked every column except the one it gated

The registration says of its cell: *"It is the middle of the grid on both axes.
It is not the maximum of any column: the best gross is at z = 4.0 and the best
t-statistic at z = 1.5."* Both of those are true. Both are also beside the
point, because conditions 2 and 3 are **functions of the win rate alone**, and
the win rate is the column I did not check:

```
exploratory win rate, all 27 cells, ranked
   dev  4h  z 2.0  k 16   57.59%   <-- the registered cell, rank 1 of 27
   dev  4h  z 1.5  k 32   57.03%
   dev  4h  z 2.0  k 32   56.98%
   dev  4h  z 1.5  k 16   56.75%
```

So the cell was the argmax of exactly the statistic the falsifier gates on, and
the registration told the reader the opposite in the sentence that was supposed
to prove it had not been cherry-picked. I chose the gate for power and to avoid
repeating fault 11's seed problem, chose the cell to look principled, and never
crossed the two. **A cell is "the middle" only of the statistic it is judged
by.**

The damage is bounded and worth stating: running all twenty-seven cells on the
test window, **nine pass all three conditions** (eight of the twenty-five
unique), so this was not a lottery ticket. But every dev-24h cell fails, five
of those six with negative gross, and every k=68 cell fails.

## What is actually there, and it is not what was registered

Both reviews converge on the same thing from different directions, and it
replicates on the exploratory half, which had been read before the
registration was committed, so the split is checkable rather than merely
fitted.

```
entry hour (UTC)            exploratory                test
12:00-16:59 London/NY    n 812  -3.276 bp  50.37%   n 789  -1.474 bp  51.33%
17:00-01:59 evening      n 547  +9.582 bp  65.45%   n 438 +10.957 bp  60.27%
```

Ninety-five trades — 5.6% of them — entered between 20:00 and 23:45 UTC and
carry **63.3% of the entire gross sum** at +35.34 bp each, against +1.208 bp
for the other 1,611. Those are the hours bracketing the daily break and the
session rollover: the thinnest bars in the tape, on a bid-only feed whose
volume column is identically zero, where the real spread is widest and
unmeasurable from this data.

Three things say the underlying effect is not nothing:

- **It is not either leg.** Same machinery, single instrument: silver alone
  +0.487 bp at 51.34% (p 0.294); gold alone −1.474 bp at 49.87% (p 0.94).
  Neither reverts on its own. The pair does real work, and that attack failed.
- **It clears the repository's own nulls.** Against a direction null with the
  sign of z permuted, 2,000 draws: mean at the 95.6th percentile, win rate at
  the 100th. Against a matched random non-overlapping entry null: 99.6th and
  100th.
- **It replicates across halves on the hour axis**, which the clean/gapped cut
  alone would not establish.

And three that say it is not the registered mechanism:

- **The stated story is wrong.** The registration says what is left when
  gold's move is removed is silver's own and should come back. On the test
  window the +3.108 splits into **gold leg +3.495, silver leg −0.387** — silver
  does not come back, gold catches up. On the exploratory window it is the
  reverse. The leg carrying the money flipped between the two halves.
- **The four-hour hold contains a half-hour bounce and three and a half hours
  of nothing**: on the clean subset, the first 30 minutes give +1.875 bp at
  55.93% (p 2.9e-05) and the remaining 3.5 hours give −1.588 bp at 50.28%
  (p 0.87).
- **It belongs to a family already closed.** An overnight, break-crossing
  effect on gold is `2026-09-13-close-reopen-drift.md`, which failed its gate
  as a unit. This is that shape again, in a pair.

## The decay, and the owner's own criterion

| | n | gross | win | sign p |
|---|---|---|---|---|
| 2018-06 → 2021-12 | 778 | +5.903 | 58.10% | 7.1e-06 |
| 2022-01 → 2026-05 | 928 | +0.766 | 51.62% | 0.341 |
| 2023-01 → 2026-05 | 711 | −0.224 | 51.48% | 0.453 |
| **2025-06 → 2026-05**, the recent Vantage year | **220** | **−1.706** | 52.73% | 0.458 |

The sup two-proportion z over all split points is 2.97 at 2022-03-14, which is
suggestive for a maximum over 1,300 splits and not conclusive. Under the
criterion the owner set on 2026-09-13 — the recent Vantage year is primary —
this reads **−1.706 bp**.

## The cost was understated, twice

1. The registration's table printed 0.49 bp for gold and a 3.27 bp round trip.
   That came from the live terminal at today's $4,296 while the instrument uses
   $0.28 over the span's last close of $4,539. **Corrected in place in the
   registration**, not in a commit message, because that is fault 10's whole
   lesson.
2. The larger one, from the adversary: the P&L is per unit of **silver**
   notional, and beta at the 1,706 entries averages **1.514**, so the gold leg
   is 1.5× the silver notional and was charged at 1×. The honest bracket is
   **3.72 bp proportional and 11.09 bp dollar-constant**, giving net **−0.610
   bp** and **−7.984 bp** rather than −0.292 and −7.152.

Either way the registration's pre-declared economics stand: the family caps out
near 0.4% a year on deployed capital at the optimistic bound, and negative at
the other. **No outcome of this registration was ever a strategy or a paper
run, and this outcome is not one.**

## What each role said

- **adversary — WEAKENED, one step from BROKEN.** Its sentence: *"On the 1,257
  of 1,706 trades where the registered treatment actually occurred — a
  deviation accumulated over four clock hours and a hold of four clock hours —
  the cell reads +0.287 bp at 52.51% won with an exact sign-test p of 0.0803,
  failing two of the three declared conditions, because the windows are counted
  in rows on a tape with a daily break, and the entire pass is carried by the
  26% of trades that straddle that break, where the effect is 38× larger and
  replicates on both halves as an overnight phenomenon in the family
  `2026-09-13-close-reopen-drift` already closed."* It also verified the
  provenance to the second, found fault 13, measured the latency fragility (one
  bar's delay gives +2.298 bp at 54.55%; two bars gives +1.461 at 51.92% and
  fails condition 3), and was explicit about why it stopped short of BROKEN:
  *"A reader who thinks a registration should be judged only by the words it
  wrote would call this BROKEN."*
- **data-integrity — CAVEAT**, with no objection to the arithmetic and a
  standing objection to what it is called. Its sentence: *"the tape supports
  the arithmetic exactly as printed … but it cannot support the claim as a fact
  about a four-hour gold–silver residual, because 14.9% of the holds are not
  four hours of market and carry 53% of the gross, and the 6.0% entered in the
  hour around Dukascopy's daily break carry 70% of it."* It confirmed both
  previously declared faults are genuinely gone and measured them — the level
  form of the deviation moves **760.94 bp per 0.01 of beta** against the
  committed form's 0.29, and the overlapping variant inflates t from +1.675 to
  +7.01, the factor of four the registration predicted. It also caught a
  misattribution of mine: I told both reviewers that
  `2026-09-15-nfp-cross-asset.md` records "206 intra-week holes totalling 4,287
  hours" for the silver feed. That figure was in that run's data-integrity
  report, not in the record, and I cited the record. **A number quoted from
  memory is a number that has not been checked.**

## Reading

Thirty registrations. The one before this failed because I gated a percentile
at a precision its estimator could not deliver. This one failed because I
gated a statistic and then selected the cell on it, and because I measured four
hours in a unit that is not time. Both are mistakes about the *instrument of
judgement* rather than about the market, and both were invisible from inside
the numbers: the code is causally clean, every figure reproduces to the last
digit in two independent implementations, and the result clears the
repository's own direction and entry nulls at the 100th percentile.

What that pattern says is worth more than the claim it destroyed. The loop has
stopped finding bugs in its arithmetic and started finding them in its
epistemics — and the reviews are now catching things a test suite never could,
because they are failures of what a number was allowed to mean rather than of
what it was.

The honest residue is a real, overnight, break-crossing dislocation between
gold and silver that is not reducible to either leg, replicates on both halves
of the data, sits at the 100th percentile of two nulls, has decayed to nothing
since 2022, and lives entirely in the hours where this repository cannot price
what it would cost to touch.

## What would reopen this

The adversary's experiment, and it is cheap, concrete and decisive:

**Measure the Vantage spread on `XAUUSD.sc` and `XAGUSD.sc` read-only for one
week, bucketed by UTC hour, then re-price the 438 evening trades at their own
hour's median and 90th percentile instead of an all-hours blend.** The one
spread number this repository owns is a three-day median across every hour, and
the entire surviving effect sits between 17:00 and 02:00 UTC where the
15-minute range is a third of its 13:00 value — silver 11.8–17.0 bp against
43.2 bp — and where retail metals spreads are widest.

- If the evening spread is more than about twice the midday spread, +10.96 bp
  at 60.27% won is inside the cost, and the family closes as an artefact of
  pricing thin hours at liquid-hour costs.
- If it is not, the repository has a real overnight gold–silver dislocation,
  and it is re-registered as a **break-crossing** hypothesis with **clock-based
  windows**, a **one-bar entry latency**, and **the hour declared in advance**.

**One blocker to name rather than work around:** `XAGUSD.sc` is not in the
terminal's Market Watch, and `symbol_select` is not on the approved read-only
list for this machine's live account. Reading silver's spread needs either the
symbol added by hand in the terminal, or explicit permission to call that one
function. Gold can be logged today; silver cannot.

## What this does not say

- It does not say the code was wrong. Two independent reimplementations
  reproduce every digit, causality is proven bitwise, and a deliberate
  look-ahead flips the sign. The instrument is clean; the *design* had two
  faults and the *registration* had one.
- It does not say the pair is nothing. It clears both of the repository's nulls
  at the 100th percentile and neither leg produces it alone.
- It does not say a count-based gate was the wrong choice. It was the right
  choice under fault 11 and the registration said so in advance. The error was
  selecting the cell on the same statistic, not gating on it.
- **It changes nothing about the ten paper books.** Nothing here is a strategy
  and nothing here was ever going to be one.
