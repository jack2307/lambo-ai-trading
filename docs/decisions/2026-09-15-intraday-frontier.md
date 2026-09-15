# 2026-09-15-intraday-frontier — what this account can and cannot be asked, before anything is registered

**Written:** 2026-09-15, after `2026-09-15-venue-residual` and
`2026-09-15-quote-asymmetry` closed on the same day
**Status:** a feasibility record, not a hypothesis. **No outcome was read to
produce it** — every number below is a property of the cost, the price
dispersion, or the count of signals, and none of them says which way a trade
went. That is what makes it usable before a registration rather than after one.
**Receipts:** `docs/research/runs/2026-09-15-quote-asymmetry/` (the spread and
dispersion measurements), `docs/research/runs/2026-09-15-venue-residual/spread-probe-30d.txt`

## Why this exists

Thirty-two registrations have closed with no survivor. Two of them were
mechanisms invented rather than read, at the owner's instruction, and both
closed the same day. The useful question after that is not "what is the
thirty-third idea" but **"what size of effect can this account detect at all,
and what size must it be to pay?"** — because an idea that fails that test is
not worth a registration, and three failed it today before a line of hypothesis
was written.

## The three numbers

**Cost, measured, not assumed.** 11,576,449 ticks of read-only history
(`mt5_spread_probe.py --days=30`): p50 spread **$0.19–0.20** through Asia, London
and New York, **$0.25** at the daily stop, **$0.26–0.27** at the reopen. On a
$4,300 gold that is a round trip of **0.455 bp**, and `2026-09-15-quote-asymmetry`
paid exactly that at the quotes that filled it (mean r_long + mean r_short =
−0.9068 bp = 2 × 0.4534).

**Noise.** Unconditional dispersion of the mid return: **13.70 bp** over five
minutes, **33.01 bp** over thirty.

**Count.** How many non-overlapping trades a |z| threshold yields over five
months of this feed (104 sessions, 142,991 minutes).

## The frontier

Minimum detectable gross edge = 2 × (dispersion / √n) + the round trip. A
mechanism whose plausible edge is below this line cannot be tested here, however
true it is.

```
     k  thr      n   sd bp     SE  MDE gross
     5  2.0   4062   13.70  0.215      0.885
     5  2.5   2410   13.70  0.279      1.013
     5  3.0   1492   13.70  0.355      1.164
     5  3.5    962   13.70  0.442      1.338
     5  4.0    648   13.70  0.538      1.531
    30  2.0   1798   33.01  0.778      2.012
    30  2.5   1226   33.01  0.943      2.340
    30  3.0    826   33.01  1.148      2.752
    30  3.5    574   33.01  1.378      3.210
    30  4.0    418   33.01  1.614      3.684
```

**Raising the threshold makes the test harder, not easier.** The sample falls
faster than any plausible scaling of the effect can raise it: going from 2σ to
3σ multiplies the trigger by 1.5 and the bar by 2.8.

Two corollaries, and they are the operating rules this record exists to set:

1. **A five-minute mechanism on this account must produce ≈0.9 bp of gross edge
   per trade.** That is 0.065 of a five-minute standard deviation. At thirty
   minutes it is 2.0 bp, 0.061 sd. The fractional requirement barely moves with
   horizon, because n falls as the dispersion rises.
2. **The binding constraint is the effect size, not the cost.** The round trip
   is 3% of the five-minute dispersion. Nothing here is killed by the spread;
   things are killed by having no information.

## What this already decided, on the day it was written

**The short-horizon reversal family: closed by arithmetic, not by a test.**
`2026-09-15-quote-asymmetry`'s control arm is the one measurement in that study
whose day-block interval excluded zero — a large two-sided quote move reverts
over the next five minutes, `[-1.4303, -0.3207]` bp read as momentum, so **+0.41
bp read as reversion**. Against a round trip of **0.455 bp** the effect is
smaller than the cost. More data cannot fix that: it sharpens the measurement of
a number that is already on the wrong side of the line. And the thresholds where
reversion might exceed 0.455 are exactly the thresholds where five months cannot
measure it — 4σ needs 1.53 bp to be detectable and would have to be 3.7× its 2σ
size to get there.

**The dealer-positioning-before-a-release idea: never written, because it cannot
be powered.** Eight months of tick history hold roughly 50 high-impact releases.
At a per-event dispersion of ~30 bp the standard error is ~4–6 bp, so the design
would need an edge near 10 bp — a third of the whole move's size — to clear.
No dealer-positioning story is worth that.

**Retail order flow: not in the data.** The one variable that plausibly carries
0.06 sd of direction — the dealer's own client flow, where a payer story is not
speculative — does not exist in this feed. `copy_ticks_range` on `XAUUSD.sc` and
`EURUSD.sc` returns `last`, `volume` and `volume_real` all zero, and the
`TICK_FLAG_BUY / SELL / LAST / VOLUME` bits are never set on any tick: the only
observed flag values are 1028, 1154 and 1158, all quote flags. This feed carries
quotes and nothing else. Probed 2026-09-15, read-only.

## What it does not say

- It does not say no intraday edge exists. It says one below ~0.9 bp per
  five-minute trade cannot be **demonstrated** here on five months, and one
  below 0.455 bp cannot be **traded** here at all.
- It does not close longer horizons. It notes that they buy dispersion and give
  back sample at almost exactly the rate that keeps the fractional requirement
  near 0.06 sd, so "go slower" is not by itself an answer.
- It is not a substitute for a registration's own power line. It is the number
  that line should be computed against.

## Where the frontier is favourable

Only where the conditional move is large **and** the count stays up, or where a
data holding nobody else has raises the effect rather than the sample:

- **The options tape.** The founding thesis, and the only holding here that is
  genuinely unobtainable elsewhere: dealer hedging after a large print is
  mechanical, price-insensitive flow, which is the one payer story on the shelf
  that does not depend on someone being wrong. Gold has 9 days of tape and BTC 5,
  both live and growing; the question becomes askable at roughly three months —
  about December 2026. Until then the honest work is the measurement instrument,
  not a hypothesis.
- **More tick history as it accrues.** The broker holds about eight months and
  the loop has ingested five. Each further month lowers the five-minute bar from
  0.885 toward 0.455 asymptotically — it can never go below the round trip.

## The rule this sets

**Every registration from here states its minimum detectable effect before its
falsifier, computed from this table or its own equivalent, and says what prior
makes that plausible.** Three ideas died today against this arithmetic without
costing a research cycle; that is the record's entire value.
