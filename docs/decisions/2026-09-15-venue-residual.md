# 2026-09-15-venue-residual — the residual is 0.8 seconds of tape, and the files are stamped to the minute

**Registered:** `dc7d7e5` (2026-09-15), before a skew was computed
**Ran:** `076e9c3` (in-sample + control), verify below
**Status:** closed. The declared direction failed; its mirror does not survive the
traded leg, and the mechanism claim cannot be made at this resolution at all.
**Hypothesis:** `docs/hypotheses/2026-09-15-venue-residual.md`
**Receipts:** `docs/research/runs/2026-09-15-venue-residual/`

## What was asked

The owner's instruction on 2026-09-15: the published families are exhausted —
twenty-nine registrations, no survivor out of sample — so stop testing what is
on the internet and work the data this repository owns and nobody else does.

Two feeds of the same metal, recorded simultaneously, is such a holding: it
needs an account at a dealer and a feed from an ECN at once. The claim was
**inventory**. A dealer marks its quote away from consensus to discourage the
flow it already holds too much of, so the skew, once the constant markup is
removed causally, should predict the next move — and it must predict it on the
**ECN leg**, which the dealer's own quote cannot manufacture by converging back.

Direction was declared: a positive skew precedes a **fall**. The registration
also said in writing that the mirror is a second look and the falsifier is
therefore the two-sided 97.5th, "stated here so that a result in the unexpected
direction cannot later be called a discovery at 95."

## The one thing that passed

**The alignment gate, and it is the best artefact here.** It uses no return at
all: gold stops for an hour a day, so each feed's daily hole must open and close
on the same UTC minute and shift on the same New York DST weeks.

```
weeks compared: 52
reopen minute disagrees in 0 week(s); close minute in 0 week(s)
XAUUSD  moved its reopen in weeks: ['2025-11-03/2025-11-09', '2026-03-09/2026-03-15']
XAUDUKA moved its reopen in weeks: ['2025-11-03/2025-11-09', '2026-03-09/2026-03-15']
```
(`align-5m.txt`.) `data-integrity` re-proved it two further ways the gate does
not use — a sub-bar minute scan (`argmin` at j=+0 in all three DST regimes) and
a per-day rather than per-week comparison (199 days, 0 reopen disagreements, 1
close disagreement of 5 minutes) — and confirmed the exporter *cannot* introduce
a sub-bar offset, since `server_offset_seconds` returns only 3h or 2h.

This repository has been bitten by a timezone twice. It was not bitten here.

## The registered claim failed

`in-sample-5m.txt`, signed by the declared direction, gate 97.5:

```
   thr   k  fires   ECN bp    up%  pctile
   2.0   1   1526   -1.539  45.1%   1.90
   2.0   6   1142   -1.626  45.5%  12.75
   2.0  12    985   -3.298  45.3%   0.95
```

The market went the other way: a positive skew precedes a **rise**.

## The mirror survived two controls that should have killed it

`control-5m.txt`, read in the direction the data took:

```
    signal   k  fires   ECN bp  Vantage bp      net    up%  pctile
     resid   1   1526   +1.539      +1.000   +0.328  54.8%  98.10
     resid  12    985   +3.298      +2.771   +2.099  54.7%  99.05
       mom   1   2851   +0.187      +0.247   -0.425  48.4%  75.76
       mom  12   1468   -1.388      -1.336   -2.007  46.3%  13.69
     stale   1   2818   +0.304      +0.340   -0.332  48.9%  82.35
     stale  12   1455   -0.440      -0.422   -1.094  47.4%  34.89
correlation between the registered residual and the dealer's own last-bar return: -0.031
```

If Dukascopy's close were simply an older tick, the residual would be the
Vantage return already printed, and a **deliberately staler** ECN would
strengthen it. It weakens it. Plain momentum does not reproduce it. That is why
the study went to review instead of closing here.

## Why it closed anyway — three vetoes, each on different evidence

### data-integrity: the residual is a sub-second quantity and the files are minutes

The decisive number. From the ECN's own 1-minute return dispersion (4.538 bp
over 348,852 contiguous minutes), the residual converts to a time:

```
mean |residual|, all bars   0.2153 bp  <=>  the two feeds 0.135 s apart
sd of the residual          0.5197 bp  <=>                0.787 s apart
mean |residual| at a fire   1.4620 bp  <=>                6.227 s apart
one MINUTE of misalignment  4.54 bp    =    21x the typical residual
```

> *"The gate certifies agreement to 60 s. The signal is a 0.14 s quantity on the
> average bar and a 6.2 s quantity at a fire. A steady 0.8-second lead of the
> dealer's feed over Dukascopy's — an ordinary amount of market-data latency
> between two unrelated vendors — reproduces the residual's entire standard
> deviation. Neither file carries anything finer than a minute stamp, so this
> cannot be checked, tightened, or excluded with the data on disk."*

It also **disproved the study's own suspicion** about the thin hours: the ECN 5m
bars are 99.91% full (mean 4.999 minutes per bar, mean close staleness 0.001
min), so the 20:00–23:00 skew is not an artefact of the converter dropping flat
minutes. And dropping the contaminated hours does not remove the effect
(`drop <65m post-reopen` reads 99.40; `drop 20:00–23:59` reads 97.98) — which is
precisely why the timing argument, not the thin-hour argument, is the one that
blocks.

One further defect it named: **the null's volatility deciles are a liquidity
proxy, not the confounder they claim to control.** `sd(resid)` varies **6.43x**
across UTC hours while the control variable, the ECN bar's range, varies only
**1.46x**. Decile 0 is 73.6% evening bars at mean tick volume 485; decile 9 is
20.7% at 2,621. The permutation shuffles signs *inside* the thin-hour bucket, so
it cannot break a thin-hour mechanism.

### adversary: the mirror inverts the reason the ECN leg was chosen

Admissible — *"a two-sided test at 97.5 rejects in either tail… anyone who
blocks here is re-reading the registration after the fact"* — and then fatal:

> *"`venue_residual.py:18-19` says the ECN leg was chosen because it is 'a number
> the dealer's own quote cannot manufacture by converging back.' Under the
> declared direction that is true. Under the mirror it is exactly backwards:
> convergence now flatters the ECN leg and penalises Vantage."*

The artefact proves it on its own page — ECN minus Vantage, from `control-5m.txt`:

| signal | k=1 | k=6 | k=12 |
|---|---|---|---|
| **resid** | **+0.539** | **+0.576** | **+0.527** |
| mom | −0.060 | −0.035 | −0.052 |
| stale | −0.036 | −0.020 | −0.018 |

A flat +0.55 bp wedge at every horizon, in all three `resid` cells and in none of
the six control cells. A forecast grows with k; a constant identical at k=1 and
k=12 is paid in the first bar — it is the residual closing, the exact quantity
the registration excluded by construction and the mirror smuggled back into the
headline.

### risk: the sample is not 985 bets, and half the net is unrepresentable

94% of k=12 trades (98% at k=1, one day holding 39) share a calendar day with
another trade. Re-run with the **day** as the unit:

```
k=12  day-level sign permutation 98.35th; day-block bootstrap 95% CI [-0.4774, +4.6325], P(mean<=0) = 5.5%
k=1   day-level sign permutation 99.57th; day-block bootstrap 95% CI [-0.4082, +1.1589], P(mean<=0) = 20.8%
```

Both intervals contain zero. Concentration: at k=1 the best 5% of trades are
**674% of the net** and March 2026 alone is **98%** of it, one day (2026-01-30)
**75%**. At k=12 the ten largest winners are **88%**. Under the repository's own
configured guards (`max_trades_per_day=4`, 30-minute cooldown) k=1 turns
**negative**, −0.24 bp.

### execution-realist: the cost fear was wrong, in the study's favour

The one place the study got a discount. 11,576,449 ticks of read-only history
(`spread-probe-30d.txt`), p50 by window:

```
close  20:00-20:30 UTC   0.19    reopen 22:00-22:15 UTC   0.26
into   20:30-21:00 UTC   0.25    asia/london/ny           0.20
```

The rollover spread is **$0.26–0.27**, *below* the $0.28 the registration charged
everywhere. The advisory's own fear — that the evening would need $0.64 to
break k=1 — is not what the tape says. The study was charged too much, not too
little, and it still failed.

## The verify run: zero of six cells survive

`verify-5m.txt` — the same six registered cells, re-read under every correction
the reviews demanded: percentile on the **traded** leg, the null's centre
printed, **no trade allowed to span the daily stop or a weekend** (my fault:
`fires()` checked freshness on the signal bar only, so 27 k=12 trades crossed a
hole, eleven of them weekend holds, the longest 51 hours, carrying 16% of the
net — against `flat_before_weekend_hhmm = 1655` in the project's own config), and
the cost charged by the entry's own hour from the measured tape.

```
--- markup removed over 4h ---
     k     n     ECN    VANT  null mu    edge   cost     net  net@flat  pctile      H1      H2
     1  1341  +1.256  +0.628   +0.349  +0.279  0.520  +0.108    -0.044   77.60   75.90   55.13
     6  1018  +0.783  +0.057   -0.072  +0.128  0.512  -0.456    -0.615   56.47   82.49   31.86
    12   870  +2.512  +1.807   -0.119  +1.926  0.509  +1.298    +1.136   93.48   41.95   96.46
--- markup removed over 24h ---
     1  1363  +0.873  +0.281   +0.352  -0.071  0.517  -0.236    -0.391   42.10   50.08   32.45
     6  1019  +1.020  +0.288   +0.329  -0.041  0.511  -0.222    -0.384   49.09   71.19   30.57
    12   866  +2.125  +1.386   +0.157  +1.229  0.507  +0.879    +0.714   83.59   33.31   90.28
```

**Nothing reaches 97.5.** The best cell is 93.48, and it is one half of the
window: H1 41.95, H2 96.46. Its 99%-identical twin at the 24h markup reads 83.59.
The delay curve is not a decaying edge but noise — 77.60 / 97.78 / 19.55 at
k=1 for delays of 0, 1 and 2 bars. Šidák over the six cells turns the best
two-sided p of 0.019 into **0.109**.

## The verdict

**Closed, both ways.** The declared direction is falsified at the 1.90th, 12.75th
and 0.95th percentile. The mirror reaches no cell's gate on the leg that would be
traded once a trade is forbidden from spanning a weekend, and its best reading is
one half-year.

And even had a cell passed, **the word in the claim could not have been
defended**. What is measurable here is that the dealer's feed is *earlier* than
the ECN's by something on the order of a second. That is a fact about plumbing,
not about a book. "Inventory" requires separating a dealer's pricing decision
from two vendors' clock latency, and minute-stamped bars cannot do it.

## What this does not say

- It does not say the two feeds are misaligned. They are aligned to the minute,
  proven three ways, and that result stands on its own and is reusable.
- It does not say Dukascopy is unfit as an out-of-sample feed. Structure-based
  strategies read bar shape, and a sub-second lead does not touch that. It says
  only that a **bar-by-bar price comparison between the two venues** measures
  latency at this resolution.
- It does not say a dealer's skew carries no information. It says this data
  cannot be used to ask.
- It does not retract `2026-09-14-pre-nfp-drift`, which quoted a second venue's
  agreement on an **hour-scale** direction and up-rate. A sub-second lead is
  four orders of magnitude below that claim's resolution.

## What would reopen it

Tick data with venue timestamps on both legs — not minute bars. Then the
question is well posed: does the dealer's quote move before the consensus by
more than the feeds' own latency, and does what is left predict anything? Until
such a feed exists on disk, no venue-comparison hypothesis should be registered
at bar resolution, and this record is the reason.

## What it cost, and what it bought

One day. It bought: an alignment proof that is reusable and that closes a
standing worry about two timezone bugs; thirty days of measured spread by hour,
which answers most of the top backlog item without waiting for the logger; the
knowledge that the venue family is not askable at bar resolution; and four
instrument faults now on the backlog, one of them mine.
