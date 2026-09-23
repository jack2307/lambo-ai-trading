# 2026-09-23-designed-2 — conditional structure: the path is never more directional than chance, and the one reversion that pays inverts where the cost becomes affordable

**Angle:** 2 of 4, *conditional structure — when not to trade*
(`docs/hypotheses/2026-09-23-designed-methods.md`).
**Worktree / branch:** `E:/rust/fd-wt-dm2`, `agent/designed-2`.
**Data root, every run:** `E:/rust/flowdesk/data-sealed`. Every receipt in
`docs/research/runs/2026-09-23-designed-2/` prints that root and its first and
last bar on its first two lines. The last bar of `XAUUSD-15m` in every one of
them is **2025-09-22 23:45**. `E:/rust/flowdesk/data` was not opened.

## Methods proposed: none

**Zero, and the registration calls that a valid and preferred outcome over
filler.** This note is the submission. It is not "the angle looks hard"; it is
four measurements that between them say a method on this angle cannot be paid at
this cost, with the counts, and with the one candidate carried far enough to be
killed by a number rather than by an opinion.

`2026-09-23-designed-2-frozen.toml` declares the empty slate explicitly. There
is nothing for the lead to run on the withheld year. That is **no result, not a
result of zero** — no parameters were frozen because no method was proposed.

## The mechanism, stated before anything was fitted

Two things were true about this market before this window was opened, and the
whole angle follows from them.

1. **A trade with a stop and a target is a claim about the shape of a path, not
   about a mean.** Whether a breakout is paid depends on whether price, having
   travelled, tends to end up far from where it started; whether a fade is paid
   depends on the opposite. So the statistic that decides which of the two
   families can exist at all is **path efficiency** — the net move over a window
   divided by the total travel inside it — and not a conditional mean return.
   This matters because a second moment of the path is estimated far more
   precisely per observation than a first moment, which is how an angle with a
   thin slice of time can still be measured.

2. **Gold's liquidity is supplied by different desks at different hours, and the
   spread is fixed.** The mean absolute fifteen-minute return runs 0.59 points at
   04:00 UTC and 1.81 points at 13:00 UTC on fifteen years of Dukascopy gold
   (`01-hours-xauduka.txt`, n ≈ 15,750 per hour) — a factor of three — against a
   configured round trip of 0.28 points that does not move at all. Wherever the
   book is thin, a liquidity event moves price further than the information in it
   warrants, and the inventory model of the bid-ask bounce says part of that move
   comes back. That predicts reversion concentrated in the thinnest hours, and
   predicts it for moderate moves and not for the largest ones, which carry
   information.

Both predictions were then measured, and both hold. The method still does not
exist, for reasons the measurements had to produce rather than assume.

## How this differs from the clock findings that vanished

`docs/decisions/2026-09-14-night-synthesis.md` closed five mechanisms on three
assets with one shape: real at the 95th percentile or better on a first window,
worth about the spread, gone on the second. Its members were
`close-reopen-drift` (gold long 16:30 → 18:30 New York), `friday-weekend-hold`,
`fx-local-hours` (the euro short through European hours) and two trend signs.
`2026-09-13-volcond-breakout` separately closed the breakout family under an
absolute-volatility condition, and `2026-09-23-three-month-search` closed 25
out-of-sample cells across 21 mechanisms.

Four differences were designed in, and the honest accounting is that **three of
them held and the fourth did not.**

- **Held — no fixed direction anywhere.** Every closed clock finding was a
  fixed-sign drift: gold *up* across the close, the euro *down* in European
  hours. One number, estimated from the window, decaying when the flow behind it
  turns. Nothing here has a side of its own: the path-efficiency measurement is
  symmetric under reversal by construction, and the candidate's side is whatever
  the market just did, reversed.
- **Held — the right null, which the closed record did not have for this
  statistic.** Path efficiency against the textbook 1/√h random-walk line is
  wrong in a way that manufactures exactly the finding an agent on this angle
  would want. See the next section: the line understates the null by about 9%
  and would have handed me five hours that look 6–9% *more* directional than
  chance. Replacing it with a sign-flip control — every fifteen-minute magnitude
  kept where it is, every sign redrawn — removes that and reverses the reading.
- **Held — replication on four series and two vendors before any method was
  written**, not after a pass. The central negative below is at the 0th or 1st
  percentile of a 200-seed control on Dukascopy gold, Vantage gold, Dukascopy
  silver and Dukascopy EUR independently.
- **Did not hold — the condition is a wall clock, and I could not reduce it to a
  measured state.** This is the one that matters and it is why there is no
  method. See "the condition is a clock" below.

## Measurement 1: the 1/√h line is the wrong null, and it flatters this angle

Gaussian increments give E|net over h bars| / E[travel] = 1/√h exactly. Read
against that line, four hours of the day look persistent
(`03-effratio-xauduka.txt`, h = 16 bars = 4 hours, n ≈ 15,750 each):

| hour UTC | efficiency | ÷ 1/√16 |
|---|---:|---:|
| 10 | 0.2737 | 1.0949 |
| 11 | 0.2725 | 1.0899 |
| 12 | 0.2706 | 1.0825 |
| 13 | 0.2692 | 1.0769 |

The line is wrong. By Cauchy–Schwarz, √(Σσⱼ²)/Σσⱼ ≥ 1/√h with equality only when
every bar in the window has the same volatility — so **volatility dispersion
inside the window pushes the efficiency ratio above 1/√h with no serial
dependence whatsoever**, and gold's volatility rises steeply through the
London–New York handover, which is where those four hours are.

The control without that defect is the **sign flip**: give every fifteen-minute
return a fresh random sign and leave its magnitude in place. Travel is a function
of magnitudes alone, so travel, volatility clustering, fat tails and the entire
hour-of-day profile are preserved **exactly**, and only direction moves. On 200
seeds (`05-flipnull-h16-xauduka.txt`), the flip median for the whole window is
**0.2731**, not 0.2500 — the walk line understates the null by **9.2%**, which is
the whole of the apparent persistence above.

## Measurement 2: no hour, and no volatility regime, is more directional than chance

XAUDUKA-15m, h = 16 bars, 200 sign-flip seeds, whole window
(`05-flipnull-h16-xauduka.txt`):

| slice | n | efficiency | flip p50 | flip p95 | percentile | vs flip |
|---|---:|---:|---:|---:|---:|---:|
| all bars | 362,787 | 0.2607 | 0.2731 | 0.2753 | 0th | −4.5% |
| 09–13 UTC | 78,775 | 0.2706 | 0.2751 | 0.2792 | 3rd | −1.6% |
| 17–23 UTC | 94,971 | 0.2508 | 0.2798 | 0.2839 | 0th | −10.4% |

Every one of the 24 hours is at or below its own flip median. The same on four
series (h = 16, all bars, `05-…` and `16-flipnull-h16-*.txt`):

| series | n | efficiency | flip p50 | percentile |
|---|---:|---:|---:|---:|
| XAUDUKA-15m | 362,787 | 0.2607 | 0.2731 | 0th |
| XAUUSD-15m | 77,262 | 0.2646 | 0.2703 | 1st |
| XAGDUKA-15m | 358,049 | 0.2442 | 0.2725 | 0th |
| EURDUKA-15m | 381,183 | 0.2578 | 0.2706 | 0th |

And on every one of them the 17:00–23:00 UTC block carries the largest deficit
against its own flip control of any block of the day: −10.4% (XAUDUKA), −4.1%
(XAUUSD), −17.7% (XAGDUKA), −11.4% (EURDUKA). Split by trailing volatility
instead of by the clock
(`15-shape-by-regime-xauduka.txt`), the deficit is −2.1% to −5.7% in all six
bands and **never positive**, including the band above 4 points of ATR where the
withheld year sits (−3.1%, n = 20,683, 2nd percentile).

**This is the measured conditional difference the angle asked for, and its first
consequence is a closure, not a method.** There is no slice of this market — no
hour, no volatility regime, no instrument of the four — in which the
fifteen-minute path over four hours is more directional than a copy of itself
with the directions shuffled. A breakout or momentum method on 15m gold is
therefore pushing against a measured headwind everywhere, which gives the PF ≈
1.0 that `keltner-break`, `donchian-breakout`, `squeeze-break` and
`volcond-breakout` all produced a mechanism instead of only a number. Angle 2's
answer to "when not to trade a breakout here" is **always**.

Per year, the deficit decays: −7.9% (2010) and −7.6% (2020) to **−1.7% (2024) and
−1.8% (2025)**, the two most recent years of the design window being the two
weakest of sixteen. Whatever supplies this structure has been supplying less of
it.

## Measurement 3: the only first-moment consequence, and the two things that kill it

The one direction the path-shape measurement licenses is reversion, and only in
the evening block. The candidate condition, fixed in this order from
measurements 1 and 2 and then tested:

- entry hour **19:00–23:00 UTC**;
- **|z| = |close_t − close_{t−1}| / ATR96 through t−1, in 0.5 to 2** — a moderate
  move. Above 2 the sign turns to continuation, as the mechanism predicts
  (`11-cell-m4-xauduka.txt`: |z| 1.5–2 gives +0.293 ATR, |z| 2–3 gives −0.051);
- **thin_t = travel over t−4…t−1 ÷ (travel over t−96…t−1 ÷ 24) below 1** — a
  quiet book. Causal, scale-free;
- the forward window crosses **no halt and no weekend** (measurement 5);
- side **against the last bar**; hold 8 bars; entry at `open[t+1]` and exit at
  `open[t+1+8]`, which is the engine's own fill convention.

Measured that way (`13-fill-m8-xauusd.txt`, `14-fill-m8-xauduka.txt`), in ATR
units, gross of cost, with the round trip converted to ATR units beside it:

| series | window | n | gross | NW t | spread/ATR | net |
|---|---|---:|---:|---:|---:|---:|
| XAUDUKA-15m | 2010-06 → 2025-09 | 4,321 | +0.15306 | 6.22 | 0.13608 | **+0.01698** |
| XAUUSD-15m | 2022-06 → 2025-09 | 586 | +0.15440 | 2.02 | 0.09549 | **+0.05890** |

Two vendors agree on the gross effect to three decimal places in ATR units, which
is more replication than most rows in this record have. Every other hour block on
the broker feed is net-negative: 00–06 −0.0758, 07–11 −0.0387, 12–16 −0.0880,
17–18 −0.0613 ATU. The fill convention costs almost nothing — the
`close_t → open[t+1]` gap is worth **+0.0016 ATR**, so the effect does not live
in the gap the engine hands to the market.

**Kill 1 — the effect inverts exactly where the cost becomes affordable.**
The whole case for this method is that the effect is a fraction of the move, so
it scales with volatility while 0.28 points does not, and therefore becomes
affordable in the high-volatility regime the withheld year is in. That is a
pre-statable question — does the effect scale with ATR? — and it was asked of
both feeds (`17-reversion-by-vol-band.txt`):

| ATR band, points | XAUDUKA n | gross | net | XAUUSD n | gross | net |
|---|---:|---:|---:|---:|---:|---:|
| 0–1.25 | 787 | +0.1727 | −0.1019 | 5 | — | — |
| 1.25–1.75 | 1,329 | +0.1523 | −0.0342 | 73 | +0.3166 | +0.1381 |
| 1.75–2.25 | 1,010 | +0.1798 | +0.0371 | 137 | +0.3316 | +0.1897 |
| 2.25–3 | 616 | +0.1801 | +0.0703 | 158 | +0.1204 | +0.0118 |
| 3–4 | 331 | +0.1660 | +0.0838 | 113 | +0.2145 | +0.1325 |
| **> 4** | **248** | **−0.0988** | **−0.1528** | **100** | **−0.2450** | **−0.3002** |

Flat at +0.15 to +0.33 ATR in every band up to 4 points and **negative above it,
on both feeds independently**. The mean ATR96 of the design window's last year is
4.657 points on Dukascopy and 4.575 on the broker feed; gold was above 3,700 at
the seal date. **The withheld year is in the band where the sign is wrong.** The
cost advantage and the effect are anti-correlated, so there is no band in which
this method is paid, and the 2025 readings that first looked like a bad year
(−0.116 ATR on XAUUSD, −0.157 on XAUDUKA) are that regime, not that year.

**Kill 2 — the candidate's own evidence on the instrument that decides is
t = 2.02, chosen from about a thousand inspected cells.** The 15-year Dukascopy
t = 6.22 is a different feed, and the thing it measures is worth +0.017 ATR net —
one eighth of a spread. On `XAUUSD-15m`, the feed the withheld year is on, the
whole design window gives n = 586 at t = 2.02, selected after looking at 5 hour
groupings × 7 |z| bands × 5 thinness ceilings × 6 holding periods. That is not
evidence and no honest reading of it clears a 95th percentile twice.

## Measurement 4: the condition is a clock, and the state would not take its place

This is the difference from the closed findings that I could not deliver, and it
is reported as a failure rather than dressed up.

If the reversion is thin liquidity, the bars can see thinness directly and a
rule could read the state instead of the hour — following it when the hour moves,
which is exactly what `close-reopen-drift` and `fx-local-hours` could not do.
Holding `thin` fixed and splitting on the clock instead
(`09-thinness-vs-clock-xauduka.txt`, |z| 0.5–2, 4 bars forward, ATR units):

| thin band | 17–20 UTC | n | all other hours | n |
|---|---:|---:|---:|---:|
| 0–0.5 | +0.0775 (t 2.76) | 2,293 | +0.0179 (t 1.45) | 13,143 |
| 0.5–0.75 | +0.0986 (t 4.66) | 3,946 | +0.0209 (t 1.95) | 19,105 |
| 0.75–1 | +0.0911 (t 3.77) | 3,987 | +0.0118 (t 1.02) | 19,085 |
| 1–1.5 | +0.0810 (t 4.08) | 5,001 | +0.0169 (t 1.57) | 25,778 |

**Four to eight times larger in the evening block at every thinness band.** The
measured state does not substitute for the hour; the hour carries it. By hour at
`thin < 0.75`, the effect is in 19:00 (+0.086, t 3.52), 20:00 (+0.217, t 5.99),
21:00 (+0.193, t 2.92) and 22:00 (+0.109, t 3.24) and nowhere else. So this
candidate is a wall clock with a story attached, which is the closed shape, and
the one designed defence against that shape is the one it does not have.

## Measurement 5: the daily halt manufactures half of hour 20 — size put on a fault already named

A four-bar forward window opened at 20:45 UTC does not measure four bars of
trading. It measures one bar, the CFD's hour-long evening halt, and a fresh mark.
Filtering to windows made only of consecutive fifteen-minute bars
(`10-halt-artifact-xauduka.txt`, |z| 0.5–2, ATR units):

| hour UTC | all bars | n | consecutive only | n | dropped |
|---|---:|---:|---:|---:|---:|
| 19 | +0.0523 (t 2.75) | 4,625 | +0.0538 (t 2.83) | 4,619 | 0.1% |
| **20** | **+0.1927 (t 7.84)** | 3,301 | **+0.0844 (t 3.78)** | 1,759 | **46.7%** |
| 21 | +0.1381 (t 2.67) | 981 | +0.3561 (t 6.34) | 254 | 74.1% |
| 22 | +0.0995 (t 4.07) | 2,322 | +0.1442 (t 5.24) | 1,440 | 38.0% |

Hour 20's apparent reversion is **less than half** what an unfiltered
measurement reports.

**This is not a new defect and the record should not read it as one.** This desk
already names it: `py/research/bias_measures.py:forward_map` refuses a forward
window whose wall-clock gap is wrong, and its own comment calls a bar-counted
horizon on a tape that stops an hour a day *"fault 11 in this desk's own list"*;
`py/research/smc_structure_measure.py` says that study "already refused to
pay it". What is new here is the **size** of it on the 15-minute gold tape at the
specific hour where an evening effect would be looked for: 46.7% of that hour's
bars, and half the effect. The engine is not exposed — `max_hold_ms` is
wall-clock milliseconds, not a bar count — but **a `Strategy` that counts bars
is**, and every exploratory table in this angle would have been wrong by a factor
of two without the filter.

Anything measured on the 20:00–22:00 UTC bars of a gold
CFD without that filter is partly measuring the re-mark, and it is not tradable:
the position is held through a period with no quotes and reopened at the widest
spread of the day, 0.26–0.27 points against 0.19–0.20 either side by
`2026-09-15-intraday-frontier`'s own tick measurement. Every table in this note
carries the filter.

## Variants tried — my own multiplicity

Candidate specifications considered, in the order they were reached and killed:

1. a fixed-sign drift at an hour of day — killed on magnitude, |cont| ≤ 0.17
   points over 8 bars at the largest hour against 0.28 of spread
   (`01-hours-xauduka.txt`);
2. continuation after a very large bar, |z| > 4 — +0.429 points over 4 bars but
   n = 941 in fifteen years, 62 a year, t = 1.84 (`02-zsize-xauduka.txt`);
3. a compression breakout — killed by measurement 2 before it was written: no
   hour is above the flip control;
4. a breakout gated to 09:00–13:00 UTC, the only hours *inside* the flip control
   — killed because inside a control is the absence of an effect, not one;
5. reversion of a displacement accumulated over 8 bars — +0.174 points at its
   best cell, 0.62 of a spread (`08-giveback-k8-m8-xauduka.txt`);
6. **the evening single-bar reversion cell** — the one carried through the fill
   convention, the thinness contrast, the halt filter, two vendors and six
   volatility bands, and killed by kills 1 and 2 above;
7. the inversion of 6 in the high-volatility band — **refused rather than
   tested**, because its whole sample is the part of the design window adjacent
   to the hold-out and fitting there is what the seal exists to prevent.

Inside candidate 6, the tables inspect 5 hour groupings × 7 move-size bands × 5
thinness ceilings × 6 holding periods, plus 2 displacement lookbacks, 6
volatility bands, 3 eras, 16 years and 4 instruments. **Call it of the order of
a thousand cells looked at.** That number is why a t of 2.02 on the deciding feed
is reported as nothing, and it is stated here because a design-window figure
cannot be read without it.

## What this says for the programme

The registration sets up two explanations for 25 closed registrations and 161
cells: the mechanisms were too well known, or a 15-minute single-instrument edge
net of 0.28 is thin to nonexistent. Angle 2's measurements point at the second,
and add something the record did not have — **a reason**. The path over four
hours is less directional than chance in every hour, every volatility band and
all four instruments, so the breakout family is structurally disadvantaged rather
than merely unlucky; the reversion that structure implies is worth about one
spread at the fifteen-year cost ratio; and it turns negative in the only
volatility regime where the fixed spread would have been cheap enough for it.

Designing from measured structure was a real widening and it was worth doing. On
this angle it produced a mechanism for the record's failures rather than an
escape from them.

---

*Written on branch `agent/designed-2` from receipts in
`docs/research/runs/2026-09-23-designed-2/`. Every figure is quoted from the
file named beside it; nothing here is recomputed from memory. Units: points are
US dollars an ounce; ATR units are multiples of the trailing 96-bar mean true
range; the configured round trip is 0.28 points.*
