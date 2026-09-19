# 2026-09-19 — BOS and CHoCH, measured: what the two new markers cost

**Status:** decision note, read-only study. Nothing on the desk changed, no
registered book was touched, and no route was edited by this session. The
levels route is adding BOS and CHoCH markers in a parallel worktree; this note
is the number those markers ship with, in the same shape as the whipsaw and
latency table the three method indicators carry.

**Why it exists:** the twelve raw indicators on this desk carry the verdict of
the registration that killed them, and the three method indicators carry the
2026-09-18 bias study's latency and whipsaw table. **BOS and CHoCH would have
been the first lines drawn on this chart with nothing attached.** A marker a
reader cannot price is worse than no marker, because it looks like it was
checked.

**Answer in one line:** a CHoCH is **as fast as the fastest label on the card
at the median — and it sleeps through half the turns.** Five bars of median
lag at a 4×ATR turn, the same as `zigzag 3×ATR` on these bars, but it never
prints at all before the next turn on **50% of them** where the zigzag misses
13% and `fractal(2)` misses 18%. It is not a turn detector. It is a marker
that, when it fires near a turn, fires early — and that is a different and
much smaller claim.

---

## The data, and what is wrong with it

**`data/bars` is empty in this worktree.** It is git-ignored — fetched from
the VPS export, never committed — so a worktree checkout has no bars at all.
Rather than invent a series, this ran on the largest real gold bars that **are**
in the repository: the fixture slice the zigzag and method ports are pinned
against.

| file | bars | first | last | bar sha256 (first 16) | file sha256 (first 16) |
|---|---|---|---|---|---|
| `crates/fd-api/tests/fixtures/zigzag-h1-xauusd.csv` | 2,000 | 2026-05-19T19:00:00Z | 2026-09-18T06:00:00Z | `9eb6c013d312ade8` | `ba2d6d5b4552b134` |

Lineage, from the fixture's own header: these are **the last 2,000 closed bars
of the 25,708-bar H1 file the 2026-09-18 study measured on**, whose bar
fingerprint is `db5258a49b2807d1`. The bar digest above uses
`bars_fingerprint.py`'s recipe — big-endian bit patterns, the same absent-volume
sentinel imported from that module — so it is comparable with the study's table
rather than merely similar to it. The file digest is a plain sha256 of the
committed text, which for a versioned file is stable in a way a re-exported
parquet is not.

**State the cost of this plainly, because it bounds everything below.**

- **Four months, not four years.** 2026-05-19 to 2026-09-18 is one regime on
  one instrument. The bias study's per-year table exists precisely because
  four of its five years disagreed with each other.
- **n is small.** 59 BOS and 58 CHoCH. 62 reference turns at 4×ATR, 15 at
  8×ATR. Every percentage in section 2 moves by about 6 points if two events
  land the other way, and the 8×ATR row is 15 turns and should be read as an
  indication of direction, not as a measurement.
- **H1 only.** The study measured H1 and H4; there is no H4 file here and
  resampling 2,000 H1 bars would give 500 H4 bars, which is not a sample.
  **The H4 row of this table does not exist and must not be guessed from the
  H1 one** — the study's own tables differ between the two timeframes.

The instrument reads `FD_BARS` first, exactly like the rest of the study, so
on a machine that has the export **the same one command reproduces all of this
on 25,708 bars**. That re-run should happen before any of these numbers is
quoted at a reader, and this note should be superseded when it does.

---

## The definitions, pinned

Prose about "a break of structure" is not a definition. The zigzag port proved
that on 2026-09-18: three of d1's four independent choices differed from the
Python and **all three reproduced the owner's morning**, so agreement on a
handful of bars discriminates between nothing. These are pinned in
`py/research/smc_structure_measure.py` and each one moves the numbers.

1. **Swings are `fractal(2,2)`**, walked by the same loop as
   `bias_defs.fractal_structure(bars, 2)`. The instrument re-derives the label
   from its own swing lists and **asserts equality with that function on every
   bar** — 0 mismatches on 2,000 — so the levels behind the events are provably
   the levels behind the label the card already shows.
2. **A swing is usable only once confirmed**, two bars after it printed. The
   level a break is measured against at bar *t* is never one a live reader
   could not already see.
3. **The prevailing structure is that same `fractal(2)` label at the bar** —
   what `htf.rs` serves and what the study measured.
4. **A break is a bar CLOSING beyond the level. Never a wick.** A wick rule
   fires on every spike and turns the frequency column into a measure of the
   tape's noise.
5. **A level is spent once broken.** Without this, price holding above an old
   swing high for forty bars prints forty BOS, and "events per 100 bars"
   silently becomes "how long trends last". One swing point, one break.
6. **BOS** = break in the direction of the prevailing structure. **CHoCH** =
   break against it.
7. **A FLAT structure yields no event**, because a rule with no direction has
   no "with" and no "against". This is expensive — `fractal(2)` is FLAT on
   **43%** of these bars — so the carry-forward variant (FLAT inherits the
   last non-FLAT label) is measured beside it rather than argued about.

---

## 1. Frequency

| rule | BOS | /100 bars | CHoCH | /100 bars | all breaks | /100 bars |
|---|---|---|---|---|---|---|
| **prevailing = `fractal(2)`, FLAT yields nothing** | 59 | **3.0** | 58 | **2.9** | 204 | 10.2 |
| sensitivity: FLAT carries the last direction | 96 | 4.8 | 106 | 5.3 | 204 | 10.2 |

`fractal(2)` is FLAT on 854 of 2,000 bars.

**204 swing levels were broken; only 117 of them became an event.** The other
87 — **43% of all breaks** — happened while the structure rule was declining to
name a direction, and under the primary rule they are neither a BOS nor a
CHoCH. A chart that draws only the classified ones is drawing **just over half
of the breaks that occurred**, and a reader who sees no marker at a visible
break is not looking at a bug.

On H1 gold, one CHoCH every 34 bars is roughly **one every day and a half**.

---

## 2. Reversal within N: the CHoCH that changed nothing

The share of CHoCHs where the **old** direction breaks again within N bars.
This is the SMC claim's own failure mode written as a number — a change of
character that changes nothing. The look-forward uses the raw break stream, so
a break back through the old side counts whether or not the slow fractal label
had caught up by then.

| rule | CHoCHs | old direction breaks again ≤3 bars | ≤5 | ≤10 |
|---|---|---|---|---|
| **primary** | 58 | **5%** (3) | 10% (6) | **38%** (22) |
| carry-forward | 106 | 6% (6) | 11% (12) | 34% (36) |

**The CHoCH is not undone quickly. It is undone slowly.** At three bars its 5%
sits with the well-behaved half of the study's `undone ≤3 bars` column —
`fractal(2)` 1%, `supertrend(10,3)` 2%, `zigzag 3×ATR` 16%, and far from
`avwap day`'s 57%. But by ten bars **more than a third of them have had the old
direction break straight back through**, and the study's column stops at three
bars, so there is no published number to compare that against. The honest
reading: on the horizon a marker is actually looked at, better than a third of
CHoCHs are contradicted by the market resuming what it was doing.

---

## 3. Lag at a turn, and 4. the turns with no CHoCH at all

Reference turns are the study's own: `bias_ref.reference_turns`, a **hindsight**
ATR-zigzag at `k × ATR at the bar`, acausal on purpose so nothing causal is
graded against its own output. Lag and missed come from `bias_ref.latency`,
unmodified, so these are the same numbers as the study's columns and not two
things with the same name.

The two state definitions are **re-measured on this same 2,000-bar slice**
rather than quoted from the study's table, which was computed on 25,708 bars. A
lag of 5 against a lag of 12 read off two different samples is not a comparison.

| k | definition | turns | already there | lag med | lag p90 | **missed** |
|---|---|---|---|---|---|---|
| **4×ATR** | **CHoCH** | 62 | 3% | **5** | 44 | **50%** |
| 4×ATR | `fractal(2)` label | 62 | 13% | 13 | 25 | 18% |
| 4×ATR | `zigzag 3×ATR` label | 62 | 6% | 5 | 17 | 13% |
| **8×ATR** | **CHoCH** | 15 | 0% | 20 | 59 | 7% |
| 8×ATR | `fractal(2)` label | 15 | 27% | 13 | 18 | 0% |
| 8×ATR | `zigzag 3×ATR` label | 15 | 0% | 4 | 9 | 0% |

*`already there` means the definition read the new direction on the pivot bar
itself — for a state label a lean it already had, for an instantaneous event
the event firing exactly on the pivot. `missed` is never agreeing before the
**next** turn; those are counted, never dropped, because dropping them flatters
a slow definition by deleting the turns it slept through.*

**This is the finding.** At the median the CHoCH matches the fastest row the
study recommended — five bars, against `fractal(2)`'s thirteen. And then:

- **It misses half the turns.** 31 of 62. The zigzag misses 13% of the same
  turns on the same bars, and `fractal(2)` 18%.
- **Its p90 is 44 bars** against the zigzag's 17. When it is late it is very
  late — nearly two days on H1.
- **Nearly half the CHoCHs are not near a turn at all.** The 62 turns can
  account for at most 31 of the 58 CHoCHs, so **at least 27** printed with no
  4×ATR turn to be early or late for.

**A state label and an instantaneous event are not the same kind of object and
the 50% should be read with that in mind.** A label reads *some* direction on
57% of bars and has every bar between two turns to come into agreement; a CHoCH
has to actually fire, and it fires on 2.9% of bars. The event will structurally
miss more. That is an explanation of the number, **not a defence of it** — the
reader looking at the chart does not care why the marker was absent.

---

## 5. What follows

The distribution of the next move after the event, in ATR units at the event
bar, **signed in the event's direction** so positive means the market continued
the way the break pointed. Horizons are **wall-clock hours, not a bar count**,
using `bias_measures.forward_map`: this tape stops an hour a day and all
weekend, so "10 bars ahead" on H1 crosses the break and silently becomes
thirteen hours. That is fault 11 on this desk's own list.

**Median and quartiles. Not a win rate and not a profit factor.** A win rate
needs a stop and a target, and those are a strategy.

| horizon | after BOS (n) | p25 | med | p75 | after CHoCH (n) | p25 | med | p75 | any bar (n) | p25 | med | p75 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 3h | 57 | −0.54 | **+0.31** | +0.92 | 52 | −0.45 | −0.12 | +0.32 | 1,859 | −0.68 | −0.07 | +0.63 |
| 5h | 59 | −0.68 | +0.09 | +1.10 | 54 | −0.72 | −0.24 | +0.77 | 1,823 | −0.92 | −0.02 | +0.89 |
| 10h | 47 | −1.47 | +0.17 | +1.25 | 51 | −1.57 | −0.17 | +1.03 | 1,734 | −1.48 | −0.10 | +1.35 |
| 24h | 43 | −1.98 | **−0.72** | +1.40 | 47 | −2.94 | −0.94 | +2.32 | 1,573 | −2.65 | −0.11 | +2.04 |

`any bar` is the unqualified forward move on every bar, always long-signed: the
drift this tape had over this window, which is what the event columns have to
beat to mean anything. **A figure never appears on this desk without what
qualifies it**, and for this table that is both the baseline column and this
one:

| horizon | SE of the BOS median | SE of the CHoCH median | BOS med − any-bar med |
|---|---|---|---|
| 3h | 0.27 | 0.21 | +0.38 (**1.4 SE**) |
| 5h | 0.30 | 0.29 | +0.12 (0.4 SE) |
| 10h | 0.46 | 0.35 | +0.27 (0.6 SE) |
| 24h | 0.63 | 0.59 | −0.61 (1.0 SE) |

*Closed form, 1.2533 × sd / √n, rather than a bootstrap, so the instrument
needs no seed and stays deterministic.*

**Nothing here separates from the baseline.** The largest gap in the table is
**1.4 standard errors**, in a table of eight medians, and it does not survive
to the next horizon: the BOS median goes +0.31, +0.09, +0.17, −0.72 as the
horizon lengthens, which is the shape of a sample of 57, not of a tendency.
Every median in the table is **under 1 ATR in absolute value while the
interquartile span around it is 1.5 to 3.4 ATR** — at the 5h BOS cell the span
is twenty times the median. The one thing the table does say cleanly is about
*size*, not direction: after
either event, the middle half of outcomes spans roughly **1.5 ATR at three
hours and 3.4 ATR at a day**, which is the same spread an arbitrary bar has.
**The event does not mark a moment when the market moves more than usual.**

The CHoCH median is mildly negative at every horizon — the market leaning back
the way it came after a "change of character". It is under two standard errors
at every horizon and is reported because leaving it out would be selective, not
because it is a result.

---

## 6. A divergence found on the way: the two fractal tie rules

Not a sensitivity anyone asked for. This repository contains **two different
fractal rules** and they were about to be treated as one:

- `bias_defs.fractal_structure` — not beaten by any neighbour **and** strictly
  beating at least one, which **admits a flat top** as a swing.
- `htf.rs::fractal_swings` — strictly greater than every neighbour, and its
  own doc comment says "a flat top is not a swing".

| tie rule | source | swing highs | swing lows | labels differing | BOS | CHoCH |
|---|---|---|---|---|---|---|
| not beaten, beats one | `bias_defs` (every number above) | 272 | 278 | — | 59 | 58 |
| strictly beats all | `htf.rs::fractal_swings` | 272 | 278 | **0%** (0) | 59 | 58 |

**On this slice the two are the same object.** An exact tie between a bar's
extreme and a neighbour's never occurs in 2,000 H1 bars of gold at two
decimals, so every count matches.

That is a fact about these bars, not a proof that the rules are equivalent. A
tie is possible, the full file has twelve times as many chances to contain one,
and lower-priced instruments or coarser quotes have far more. **Every number in
this note is the `bias_defs` rule.** If the route ships the `htf.rs` rule it is
shipping this table about a slightly different object, and the two files should
be reconciled deliberately rather than left to agree by luck.

---

## What this means

**For the marker on the chart.** A CHoCH is worth drawing as *early notice that
the last swing against the trend has given way*, and nothing more. It is early
when it is right — five bars at the median, against thirteen for the structure
label beside it — and it is absent for half the turns a chart reader would
point at. A reader who treats its absence as "no turn here" will be wrong half
the time, so **the absence of the marker must not be presented as information.**

**For the marker's neighbours.** The route already serves a `fractal(2)`
structure label and the card is moving to `zigzag 3×ATR` on H1. On these bars
the zigzag matches the CHoCH's median lag, beats its p90 by a factor of two and
a half, and misses 13% of turns against 50%. **A CHoCH adds punctuation to a
structure row; it does not replace one**, and a UI that lets it look like a
faster version of the structure label will mislead.

**For the FLAT question.** Under the primary rule 43% of breaks produce no
marker. That is the correct behaviour for a rule that declines to name a
direction, but it should be visible: if the route draws markers, it should also
be able to say the structure is RANGE, or a reader will read the silence as
"nothing happened".

## What this does not mean

**This is not a rebuttal of the ICT result, and it must not be read as one.**
The full chain — higher-timeframe fair value gap, liquidity sweep, market
structure shift with displacement, retrace into the gap — was ported from the
expert's manual, passed in-sample on the only three months of broker M1
available, and then lost on four years of out-of-sample minutes from a second
feed: **1,428 trades, PF 0.746, expectancy −0.190R**
(`docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md`, decided in
`docs/decisions/2026-09-13-ict-sweep-mss-fvg.md`). The direction null already
put the direction rule at the 79th percentile of a coin flip in-sample. **That
verdict stands, it was not tested here, and nothing above reopens it.** This
note measures the timing and whipsaw of an *event*, the way the whipsaw table
does for a trend line. Section 5 in particular is a description of what
follows a marker, not evidence that anything follows it — the largest cell in
it is 1.4 standard errors.

**This is descriptive, not a hypothesis test, and the multiple-comparisons
position is that it does not need one and does not get one.** No claim was
pre-registered, no falsifier was named, and nothing was corrected for the
number of cells printed — there are eight medians, two reversal rules, three
horizons and two structure variants in here, and **if any of them is treated as
a result, the arithmetic that closed the bias study applies immediately**: the
expected largest of thirty standard normals under pure noise is **+1.89**, and
the largest thing in this note is 1.4.
**No number in this note may be promoted to a claim, a filter, a gate or a
size, without its own registration with its own falsifier written before it is
run.** The moment a BOS or a CHoCH acquires an entry and a stop it is a
strategy and goes through `/research` like everything else.

**And it is four months of one instrument in one regime.** The bias study's
per-year table exists because four of its five years disagreed. This note has
one season. Re-run it with `FD_BARS` set to the export before quoting it at
anyone, and supersede it when you do.

---

## Reproduce

Every number in this note, including the fingerprints and the mismatch count,
is printed by one command run from the repository root:

```
python py/research/smc_structure_measure.py
```

Python 3.9.13, pyarrow 21.0.0. Deterministic: no sampling, no seeds, no
permutations — every figure is a count or a quantile over a fixed bar series,
and two consecutive runs are byte-identical. With `FD_BARS` pointing at the
exported bars it reads `XAUUSD-1h.parquet` instead of the fixture and prints
the same tables on the full 25,708-bar file; the fingerprint row is how you
tell which one you are reading.

The instrument reuses `bias_defs.fractal_structure` (asserted, not assumed),
`bias_ref.reference_turns`, `bias_ref.latency`, `bias_measures.forward_map`
and `bars_fingerprint.NO_VOLUME`, rather than reimplementing any of them —
which is the only reason the lag and missed columns here are comparable with
the 2026-09-18 table at all.
