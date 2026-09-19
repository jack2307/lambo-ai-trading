# 2026-09-19 — BOS and CHoCH, measured: what the two new markers cost

**Status:** decision note, read-only study. Nothing on the desk changed, no
registered book was touched, and no route was edited by this session. The
levels route is adding BOS and CHoCH markers in a parallel worktree; this note
is the number those markers ship with, in the same shape as the whipsaw and
latency table the three method indicators carry.

**Revision, same day.** This note was first measured on a 2,000-bar committed
fixture, because `data/bars` is git-ignored and a worktree checkout has none.
The real export then arrived and it was re-measured on **25,722 H1 bars and
6,732 H4 bars**. Three things moved, one of them decisively, and **§0 records
what** — rather than overwriting the small-sample numbers as though they had
never been published. **Every table below is the full export.**

**Why it exists:** the twelve raw indicators on this desk carry the verdict of
the registration that killed them, and the three method indicators carry the
2026-09-18 bias study's latency and whipsaw table. **BOS and CHoCH would have
been the first lines drawn on this chart with nothing attached.** A marker a
reader cannot price is worse than no marker, because it looks like it was
checked.

**Answer in one line:** a CHoCH is **faster than the structure label beside it
and it sleeps through half the turns.** Seven bars of median lag at a 4×ATR
turn against `fractal(2)`'s twelve — but it never prints at all before the next
turn on **47% of those turns, where `zigzag 3×ATR` misses 1%**. It is not a
turn detector. It is early notice when it fires, which is a much smaller claim,
and what follows it is the tape's own drift.

---

## 0. What changed when the sample grew

The first version of this note ran on the last 2,000 H1 bars — four months of
one regime — and said so, at length, at the top. It is worth recording what
four months got right and what it got wrong, because the desk keeps paying for
small samples and this is a cheap reading of the bill.

| | 2,000-bar fixture | 25,722-bar export | verdict |
|---|---|---|---|
| CHoCH per 100 bars | 2.9 | 2.7 | held |
| CHoCH lag med, 4×ATR | 5 | 7 | held |
| CHoCH missed, 4×ATR | 50% | **47%** | held |
| **`zigzag 3×ATR` missed, 4×ATR** | 13% | **1%** | **wrong by twelve points** |
| CHoCH reversal ≤10 bars | 38% | 29% | softened |
| **BOS median − any-bar median, best cell** | +1.4 SE | **+0.3 SE** | **vanished** |
| **fractal tie rules disagree** | 0 labels | **196 labels** | **flipped** |

**The claim about the event itself survived.** Frequency, lag and the miss rate
are within a few points on twelve times the data. The event is a stable object
and four months described it about right.

**The claim about its competition did not.** On four months `zigzag 3×ATR`
missed 13% of 4×ATR turns and the CHoCH's 50% looked like a bad score on a
scale where the alternatives were merely imperfect. On four years the zigzag
misses **1%** — it sees essentially every turn. The CHoCH's miss rate is not a
worse score on the same scale; it is a different kind of object. **The contrast
is sharper, not softer, and the first version understated it.**

**The follow-through finding vanished entirely**, which it should have: it was
1.4 standard errors on n=57 and was already refused. See §5.

**And the prediction in §6 came true.** The first version found the two fractal
tie rules identical on 2,000 bars and wrote that this was "a fact about these
bars, not a proof that the rules are equivalent — a tie is possible and the
wider file has twelve times as many chances to contain one." It does, and it
contains 196 of them.

---

## The data, fingerprinted

Both files exported by the VPS task from the cent terminal — **the machine that
trades** — and copied into this worktree read-only.

| file | bars | first | last | sha256 (first 16) |
|---|---|---|---|---|
| `data/bars/XAUUSD-1h.parquet` | 25,722 | 2022-05-16T11:00:00Z | 2026-09-18T20:00:00Z | `3aef507ac395d1f3` |
| `data/bars/XAUUSD-4h.parquet` | 6,732 | 2022-05-16T09:00:00Z | 2026-09-18T17:00:00Z | `672f4e637c77a640` |

Provenance on both: `broker_symbol=XAUUSD.sc; server=VantageMarkets-Live 21;
exported_at=2026-09-19T07:46:01Z; contract_size=1.0`. Recompute with
`py -3.9 py/research/bars_fingerprint.py data/bars/XAUUSD-1h.parquet
data/bars/XAUUSD-4h.parquet`.

These are the 2026-09-18 study's files **one day on** — 25,722 bars where it
read 25,708, 6,732 where it read 6,728 — so the digests differ from the ones in
that note while the reference-turn counts do not: **754 at 4×ATR and 182 at
8×ATR on H1, the study's exact figures.** Comparisons with its table are on
effectively the same series.

**A correction, and it is the instrument's fault.** The first version of this
note printed a digest from `smc_structure_measure.py` and called it
"`bars_fingerprint.py`'s recipe". It is not. That module hashes six numbers per
row; this instrument only ever sees five, because `bias_measures.load` drops
volume, so it writes the absent-volume sentinel on every row. On the committed
fixture, which carries no volume, the two agree. **On the export, which carries
a full volume column, they do not** — `3aef507ac395d1f3` against
`08c90946d8fa36de` on the same file. The instrument now labels its column an
**OHLC digest**, prints that it is not canonical, and names the command that
is. The digests in the table above are `bars_fingerprint.py`'s.

**What is still missing.** One instrument, one broker, one anchor, and no
per-year split. The bias study's per-year table exists because four of its five
years disagreed with each other; nothing below is split by year, and a reader
should assume it would move if it were.

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
   bar** — 0 mismatches on 25,722 H1 and 6,732 H4 — so the levels behind the
   events are provably the levels behind the label the card already shows.
   **Which tie rule that is, and how it differs from the route's, is §6 and is
   no longer a footnote.**
2. **A swing is usable only once confirmed**, two bars after it printed. The
   level a break is measured against at bar *t* is never one a live reader
   could not already see.
3. **The prevailing structure is that same `fractal(2)` label at the bar** —
   what `htf.rs` serves and what the bias study measured.
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
   **43%** of H1 bars and 40% of H4 — so the carry-forward variant (FLAT
   inherits the last non-FLAT label) is measured beside it rather than argued
   about.

---

## 1. Frequency

**H1, 25,722 bars.** FLAT on 11,023 of them (43%).

| rule | BOS | /100 bars | CHoCH | /100 bars | all breaks | /100 bars |
|---|---|---|---|---|---|---|
| **prevailing = `fractal(2)`, FLAT yields nothing** | 733 | **2.8** | 684 | **2.7** | 2,444 | 9.5 |
| sensitivity: FLAT carries the last direction | 1,233 | 4.8 | 1,210 | 4.7 | 2,444 | 9.5 |

**H4, 6,732 bars.** FLAT on 2,676 (40%).

| rule | BOS | /100 bars | CHoCH | /100 bars | all breaks | /100 bars |
|---|---|---|---|---|---|---|
| **prevailing = `fractal(2)`, FLAT yields nothing** | 234 | **3.5** | 156 | **2.3** | 649 | 9.6 |
| sensitivity: FLAT carries the last direction | 347 | 5.2 | 301 | 4.5 | 649 | 9.6 |

**Of 2,444 H1 breaks only 1,417 became an event.** The other 1,027 — **42% of
all breaks** — happened while the structure rule was declining to name a
direction, and under the primary rule they are neither a BOS nor a CHoCH. A
chart drawing only the classified ones is drawing **just under six in ten of
the breaks that occurred**, and a reader who sees no marker at a visible break
is not looking at a bug.

On H1 that is one CHoCH every 38 bars, **roughly one every day and a half**.

**H1 and H4 differ, and in the one way that matters for a card.** On H1 the two
events are near-equally common (2.8 and 2.7); on H4 the BOS is **half again as
common as the CHoCH** (3.5 and 2.3). The slower timeframe holds a named
direction for longer, so more of its breaks land with the trend. A UI tuned to
H1 marker density will look sparse in CHoCHs on H4.

---

## 2. Reversal within N: the CHoCH that changed nothing

The share of CHoCHs where the **old** direction breaks again within N bars.
This is the SMC claim's own failure mode written as a number — a change of
character that changes nothing. The look-forward uses the raw break stream, so
a break back through the old side counts whether or not the slow fractal label
had caught up by then.

| tf | rule | CHoCHs | old direction breaks again ≤3 bars | ≤5 | ≤10 |
|---|---|---|---|---|---|
| **H1** | **primary** | 684 | **6%** (41) | 11% (75) | **29%** (197) |
| H1 | carry-forward | 1,210 | 7% (79) | 12% (142) | 31% (377) |
| **H4** | **primary** | 156 | **4%** (6) | 8% (12) | **28%** (44) |
| H4 | carry-forward | 301 | 5% (15) | 8% (25) | 28% (83) |

**The CHoCH is not undone quickly. It is undone slowly.** At three bars its 6%
sits with the well-behaved half of the study's `undone ≤3 bars` column —
`fractal(2)` 1%, `supertrend(10,3)` 2%, `zigzag 3×ATR` 16%, and far from
`avwap day`'s 57%. But by ten bars **more than a quarter have had the old
direction break straight back through**, and the study's column stops at three
bars, so there is no published number to compare that against. The honest
reading: on the horizon a marker is actually looked at, close to a third of
CHoCHs are contradicted by the market resuming what it was doing.

Both timeframes agree to within two points at every horizon, which is the one
place in this note where they do.

---

## 3. Lag at a turn, and 4. the turns with no CHoCH at all

Reference turns are the study's own: `bias_ref.reference_turns`, a **hindsight**
ATR-zigzag at `k × ATR at the bar`, acausal on purpose so nothing causal is
graded against its own output. Lag and missed come from `bias_ref.latency`,
unmodified, so these are the same numbers as the study's columns and not two
things with the same name.

The two state definitions are **re-measured on these same bars** rather than
quoted from the study's table. That precaution earned its keep today: on the
2,000-bar fixture `zigzag 3×ATR` missed 13% of 4×ATR turns, and on the full
file it misses 1%.

**H1** — 754 turns at 4×ATR (one per 34 bars), 182 at 8×ATR.

| k | definition | turns | already there | lag med | lag p90 | **missed** |
|---|---|---|---|---|---|---|
| **4×ATR** | **CHoCH** | 754 | 1% | **7** | 37 | **47%** |
| 4×ATR | `fractal(2)` label | 754 | 17% | 12 | 22 | 23% |
| 4×ATR | `zigzag 3×ATR` label | 754 | 8% | 6 | 15 | **1%** |
| **8×ATR** | **CHoCH** | 182 | 1% | **17** | 81 | **15%** |
| 8×ATR | `fractal(2)` label | 182 | 11% | 14 | 23 | 2% |
| 8×ATR | `zigzag 3×ATR` label | 182 | 11% | 6 | 14 | 0% |

**H4** — 168 turns at 4×ATR (one per 40 bars), 52 at 8×ATR.

| k | definition | turns | already there | lag med | lag p90 | **missed** |
|---|---|---|---|---|---|---|
| **4×ATR** | **CHoCH** | 168 | 2% | **5** | 43 | **49%** |
| 4×ATR | `fractal(2)` label | 168 | 15% | 12 | 20 | 21% |
| 4×ATR | `zigzag 3×ATR` label | 168 | 5% | 6 | 12 | 4% |
| **8×ATR** | **CHoCH** | 52 | 0% | **7** | 66 | **25%** |
| 8×ATR | `fractal(2)` label | 52 | 10% | 12 | 17 | 2% |
| 8×ATR | `zigzag 3×ATR` label | 52 | 6% | 5 | 13 | 0% |

*`already there` means the definition read the new direction on the pivot bar
itself — for a state label a lean it already had, for an instantaneous event
the event firing exactly on the pivot. `missed` is never agreeing before the
**next** turn; those are counted, never dropped, because dropping them flatters
a slow definition by deleting the turns it slept through.*

**This is the finding, and the full sample sharpened it.** At the median the
CHoCH is faster than the structure label beside it — seven bars against twelve
on H1, five against twelve on H4. And then:

- **It misses about half the turns, on both timeframes.** 47% on H1, 49% on
  H4. On the same bars `zigzag 3×ATR` misses **1%** and 4%, and `fractal(2)`
  23% and 21%.
- **Its p90 is 37 bars on H1** against the zigzag's 15, and **81 bars at
  8×ATR** against 14. When it is late it is very late — three days on H1.
- **It gets worse on the turns that matter more, alone among the three.**
  Going from 4×ATR to 8×ATR, `fractal(2)` and the zigzag both improve
  (23%→2% and 1%→0% missed) because a bigger turn gives a state label more
  room to catch up. The CHoCH's median lag instead **more than doubles**, 7→17
  on H1, and its p90 goes 37→81. A larger turn does not make the event fire
  sooner; it makes the window longer, so misses become late prints.
- **Most CHoCHs are not near a turn at all.** The 754 H1 turns can account for
  at most 400 of the 684 CHoCHs, so **at least 284 — two in five** — printed
  with no 4×ATR turn to be early or late for.

**A state label and an instantaneous event are not the same kind of object and
the 47% should be read with that in mind.** A label reads *some* direction on
57% of bars and has every bar between two turns to come into agreement; a CHoCH
has to actually fire, and it fires on 2.7% of bars. The event will structurally
miss more. That is an explanation of the number, **not a defence of it** — the
reader looking at the chart does not care why the marker was absent.

---

## 5. What follows

The distribution of the next move after the event, in ATR units at the event
bar, **signed in the event's direction** so positive means the market continued
the way the break pointed. Horizons are 3, 5, 10 and 24 **bars expressed in
wall-clock hours**, using `bias_measures.forward_map`: this tape stops an hour
a day and all weekend, so "10 bars ahead" on H1 crosses the break and silently
becomes thirteen hours. That is fault 11 on this desk's own list.

**Median and quartiles. Not a win rate and not a profit factor.** A win rate
needs a stop and a target, and those are a strategy.

**H1**

| horizon | after BOS (n) | p25 | med | p75 | after CHoCH (n) | p25 | med | p75 | any bar (n) | p25 | med | p75 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 3h | 697 | −0.76 | +0.06 | +0.87 | 664 | −0.77 | −0.06 | +0.59 | 24,065 | −0.60 | +0.04 | +0.67 |
| 5h | 703 | −0.97 | +0.09 | +1.26 | 660 | −0.98 | −0.03 | +0.93 | 23,597 | −0.79 | +0.06 | +0.92 |
| 10h | 611 | −1.41 | +0.13 | +1.73 | 602 | −1.32 | −0.01 | +1.30 | 22,443 | −1.10 | +0.14 | +1.39 |
| 24h | 567 | −1.96 | +0.17 | +2.49 | 554 | −1.95 | −0.16 | +2.32 | 20,385 | −1.75 | +0.26 | +2.42 |

**H4**

| horizon | after BOS (n) | p25 | med | p75 | after CHoCH (n) | p25 | med | p75 | any bar (n) | p25 | med | p75 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 12h | 210 | −0.70 | −0.11 | +0.92 | 130 | −0.53 | −0.15 | +0.38 | 6,026 | −0.59 | +0.08 | +0.76 |
| 20h | 191 | −0.93 | −0.09 | +1.06 | 121 | −0.87 | −0.17 | +0.63 | 5,564 | −0.79 | +0.12 | +1.06 |
| 40h | 153 | −1.28 | +0.06 | +1.55 | 90 | −1.67 | −0.26 | +1.10 | 4,433 | −1.16 | +0.24 | +1.70 |
| 96h | 125 | −1.51 | +0.06 | +1.57 | 93 | −1.75 | −0.50 | +1.40 | 3,906 | −1.40 | +0.32 | +2.19 |

`any bar` is the unqualified forward move on every bar, always long-signed: the
drift this tape had over the window, which is what the event columns have to
beat to mean anything. **A figure never appears on this desk without what
qualifies it**, and for this table that is both the baseline column and this
one:

| tf | horizon | SE of the BOS median | SE of the CHoCH median | **BOS med − any-bar med** |
|---|---|---|---|---|
| H1 | 3h | 0.08 | 0.07 | **+0.02 (0.2 SE)** |
| H1 | 5h | 0.09 | 0.09 | **+0.03 (0.3 SE)** |
| H1 | 10h | 0.13 | 0.12 | **−0.01 (0.1 SE)** |
| H1 | 24h | 0.19 | 0.19 | **−0.09 (0.5 SE)** |
| H4 | 12h | 0.11 | 0.11 | −0.19 (1.8 SE) |
| H4 | 20h | 0.13 | 0.15 | −0.21 (1.6 SE) |
| H4 | 40h | 0.22 | 0.28 | −0.19 (0.9 SE) |
| H4 | 96h | 0.35 | 0.36 | −0.26 (0.7 SE) |

*Closed form, 1.2533 × sd / √n, rather than a bootstrap, so the instrument
needs no seed and stays deterministic.*

**On the full sample there is nothing here at all, in either direction.** With
n near 700 instead of 58, the H1 gap between what follows a BOS and what
follows an arbitrary bar is **+0.02, +0.03, −0.01 and −0.09 ATR** — 0.2, 0.3,
0.1 and 0.5 standard errors, and **two of the four are negative**. **What
follows a BOS is the tape's own drift.** That is the whole result of this
section and it is a null.

The small sample's largest cell was +1.4 SE and this note already refused to
call it a finding. **It was right to, and the refusal is the part worth
keeping**: 1.4 SE on n=57 is what a table of eight medians produces when
nothing is there, and twelve times the data says nothing is there.

**H4 goes the other way, which is the cleanest demonstration available that
none of it is real.** Every H4 gap is *negative* — after a BOS the market does
slightly **less** than an arbitrary bar — reaching 1.8 SE at twelve hours.
**The two timeframes disagree in sign.** A real property of a break of
structure does not reverse when the same instrument is bucketed into four-hour
candles.

The one thing the table says cleanly is about *size*, not direction: every
median is **at or under 0.5 ATR in absolute value while the interquartile span
around it is 0.9 to 4.5 ATR**, and the event spreads match the baseline's.
**The event does not mark a moment when the market moves more than usual,
either.**

The CHoCH median is mildly negative at every horizon on both timeframes — the
market leaning back the way it came after a "change of character". It is the
only sign that is consistent across the two tables, it is still inside a
standard error or two everywhere, and it is reported because leaving it out
would be selective, not because it is a result.

---

## 6. The two fractal tie rules — the prediction, and the answer

Not a sensitivity anyone asked for. This repository contains **two different
fractal rules**:

- `bias_defs.fractal_structure` — not beaten by any neighbour **and** strictly
  beating at least one, which **admits a flat top** as a swing.
- `htf.rs::fractal_swings` — strictly greater than every neighbour, and its own
  doc comment says "a flat top is not a swing".

| tf | tie rule | source | swing highs | swing lows | labels differing | BOS | CHoCH |
|---|---|---|---|---|---|---|---|
| H1 | not beaten, beats one | `bias_defs` (**every number above**) | 3,436 | 3,480 | — | 733 | 684 |
| H1 | strictly beats all | `htf.rs::fractal_swings` (**the route**) | 3,409 | 3,454 | **0.76%** (196) | 727 | 680 |
| H4 | not beaten, beats one | `bias_defs` (**every number above**) | 937 | 941 | — | 234 | 156 |
| H4 | strictly beats all | `htf.rs::fractal_swings` (**the route**) | 935 | 939 | 0.15% (10) | 234 | 156 |

**On 2,000 bars these two were indistinguishable — identical swing counts,
identical event counts, zero labels differing — and the first version of this
note said so while refusing to conclude from it:** *"That is a fact about these
bars, not a proof that the rules are equivalent — a tie is possible and the
wider file has twelve times as many chances to contain one."*

**The wider file settled it. They are not the same object.** On H1 the two
rules find **27 fewer swing highs and 26 fewer swing lows** under the strict
rule, disagree on the structure label at **196 bars (0.76%, about one bar in
130)**, and produce **727 BOS against 733 and 680 CHoCH against 684**. On H4
the gap is smaller but not zero: 10 labels, and identical event counts.

Two things follow, and the second is the reason this section exists.

1. **Every number in this note is the `bias_defs` rule**, because that is the
   function the 2026-09-18 study measured and the one this instrument asserts
   itself against. It is stated on the table above rather than left to be
   inferred.
2. **The route ships `htf.rs::fractal_swings`.** So the markers a reader will
   see are a measurably different object from the one priced here — six fewer
   BOS and four fewer CHoCH over four years, and a structure label that
   disagrees on roughly one bar in 130. That is small. **It is not nothing, and
   nobody chose it**: the two files diverged because they were written
   separately, not because anyone decided a flat top should count on one screen
   and not on the other. They should be reconciled deliberately, in either
   direction, and whichever survives should be the one the next measurement
   runs on.

**The general lesson is the one the zigzag port already taught, and this is the
second instance in two days:** a rule that lives in two files agrees until the
sample is large enough to find the case where it does not, and validating
against a small slice — or against a description — finds nothing.

---

## What this means

**For the marker on the chart.** A CHoCH is worth drawing as *early notice that
the last swing against the trend has given way*, and nothing more. It is faster
than the structure label beside it — seven bars against twelve — and it is
absent altogether at about half of the turns a chart reader would point at. A
reader who treats its absence as "no turn here" will be wrong roughly half the
time, so **the absence of the marker must not be presented as information.**

**For the marker's neighbours, and this is sharper than the first version
said.** The route already serves a `fractal(2)` structure label and the card is
moving to `zigzag 3×ATR` on H1. On four years of the same bars the zigzag
**misses 1% of 4×ATR turns** — it sees essentially all of them — with a p90 lag
of 15 bars against the CHoCH's 37. The CHoCH is not a faster turn detector than
the row it sits beside; it is not a turn detector. **A CHoCH adds punctuation
to a structure row; it does not replace one**, and a UI that lets it look like
a faster version of the structure label will mislead.

**For the FLAT question.** Under the primary rule 42% of breaks produce no
marker. That is the correct behaviour for a rule that declines to name a
direction, but it should be visible: if the route draws markers, it should also
be able to say the structure is RANGE, or a reader will read the silence as
"nothing happened".

**For H4 specifically.** Its BOS is half again as common as its CHoCH where
H1's are near-equal, and its follow-through is negative at every horizon where
H1's is positive. Neither difference should drive anything, but a card showing
both rows should not assume the H1 measurement describes the H4 marker.

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
does for a trend line. Section 5 in particular is a description of what follows
a marker, and on the full sample it describes **nothing**: 0.3 standard errors
at the best H1 cell, with the sign reversing between timeframes.

**This is descriptive, not a hypothesis test, and the multiple-comparisons
position is that it does not need one and does not get one.** No claim was
pre-registered, no falsifier was named, and nothing was corrected for the
number of cells printed — there are sixteen medians, two reversal rules, two
timeframes and two structure variants in here, and **if any of them is treated
as a result, the arithmetic that closed the bias study applies immediately**:
the expected largest of sixty standard normals under pure noise is **+2.17**,
and the largest thing in this note is 1.8, on the timeframe whose sign
contradicts the other one. **No number in this note may be promoted to a claim,
a filter, a gate or a size, without its own registration with its own falsifier
written before it is run.** The moment a BOS or a CHoCH acquires an entry and a
stop it is a strategy and goes through `/research` like everything else.

**And it is still one instrument, one broker, one anchor, and four years
measured as one block.** The bias study's per-year table exists because four of
its five years disagreed. A single block can hide exactly that, and §0 is this
note's own demonstration that a smaller window said something different with
confidence.

---

## Reproduce

Every number in this note — both timeframes, the OHLC digests, the mismatch
counts — is printed by one command run from the repository root:

```
python py/research/smc_structure_measure.py
```

Python 3.9.13, pyarrow 21.0.0. Deterministic: no sampling, no seeds, no
permutations — every figure is a count or a quantile over a fixed bar series,
and two consecutive runs are byte-identical. It reads `data/bars`, overridden
by `FD_BARS`, exactly like the rest of the study; **with neither present it
falls back to the committed 2,000-bar H1 fixture and prints an H1 table only**,
which is how the first version of this note was measured. The data block it
prints is how you tell which one you are reading.

The canonical fingerprints in the data section come from the study's own tool:

```
py -3.9 py/research/bars_fingerprint.py data/bars/XAUUSD-1h.parquet data/bars/XAUUSD-4h.parquet
```

The instrument reuses `bias_defs.fractal_structure` (asserted, not assumed),
`bias_defs.atr_zigzag`, `bias_ref.reference_turns`, `bias_ref.latency`,
`bias_measures.forward_map` and `bars_fingerprint.NO_VOLUME`, rather than
reimplementing any of them — which is the only reason the lag and missed
columns here are comparable with the 2026-09-18 table at all.
