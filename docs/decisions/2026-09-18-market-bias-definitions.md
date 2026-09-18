# 2026-09-18 — How to determine market bias: seventeen definitions measured

**Status:** decision note, read-only study. Nothing on the desk changed and no
registered book was touched. The card's bias definition is a **presentation**
choice and may follow this note; the two running HTF books read a registered
prompt and do not.

**Asked by the owner:** *"study properly how to determine market bias — I'm
looking at H1 and it is obviously bullish"*, while the card's H1 row read
DOWN, confirmed at 05:00Z.

**Answer in one line:** he is right about the chart, the card is right about
its own rule, and the rule is the wrong one for a row meant to be glanced at.
**No definition tested carries usable directional information** — the best of
34 tests reaches exactly the size pure noise produces — so choosing one is a
legibility decision, and on legibility `fractal(2)` is a poor fit for H1.

---

## The data, fingerprinted

All three files exported by the VPS task from the cent terminal, copied
read-only at 07:48Z on 2026-09-18, broker anchor, never resampled.

| file | bars | first | last | sha256 (first 16) |
|---|---|---|---|---|
| `XAUUSD-1h.parquet` | 25,708 | 2022-05-16T11:00:00Z | 2026-09-18T06:00:00Z | `db5258a49b2807d1` |
| `XAUUSD-4h.parquet` | 6,728 | 2022-05-16T09:00:00Z | 2026-09-18T01:00:00Z | `d44b8eec8777b892` |
| `XAUUSD-15m.parquet` | 100,012 | 2022-06-27T12:45:00Z | 2026-09-18T07:30:00Z | `082fbffd123e6d30` |

Provenance on all three: `broker_symbol=XAUUSD.sc; server=VantageMarkets-Live
21; exported_at=2026-09-18T07:46:01Z; contract_size=1.0`. Recompute with
`py -3.9 py/research/bars_fingerprint.py <path>`. Format and reasoning:
`docs/research/WHICH-BARS.md`.

H1 and H4 tables are computed on the **exported** H1 and H4 files. The
anchor-robustness section is the only one that resamples, because comparing
two anchors requires building both from one 15m source. That aggregator was
validated first: rebuilt from the desktop 15m file it reproduces the
independently exported H4 on **6,585 of 6,587** complete buckets, the two
exceptions being the partial buckets at each file edge.

---

## 1. The owner's morning, bar by bar

H1, 2026-09-18 00:00Z–06:00Z. Window open 4346.28, close 4393.43:
**+47.15, +108 bp.** He is not mistaken; it is up.

| definition | 00Z | 01Z | 02Z | 03Z | 04Z | 05Z | 06Z |
|---|---|---|---|---|---|---|---|
| fractal(1) | **DOWN** | **DOWN** | flat | flat | flat | flat | flat |
| **fractal(2)** *(the card)* | UP | flat | flat | **DOWN** | **DOWN** | **DOWN** | **DOWN** |
| fractal(3) | flat | flat | UP | UP | UP | UP | flat |
| zigzag 1.5×ATR | **DOWN** | **DOWN** | **DOWN** | **DOWN** | UP | UP | UP |
| **zigzag 3×ATR** | UP | UP | UP | UP | UP | UP | UP |
| dow closes(2) | **DOWN** | **DOWN** | **DOWN** | **DOWN** | flat | flat | flat |
| ema 21/55 | UP | UP | UP | UP | UP | UP | UP |
| ema 20/50/200 | flat | flat | flat | flat | flat | flat | flat |
| adx20 +DI | UP | UP | UP | UP | UP | UP | UP |
| adx25 +DI | UP | UP | UP | UP | UP | UP | UP |
| donchian20 | UP | UP | UP | UP | UP | UP | UP |
| donchian55 | UP | UP | UP | UP | UP | UP | UP |
| supertrend(10,3) | UP | UP | UP | UP | UP | UP | UP |
| avwap day | UP | **DOWN** | **DOWN** | UP | UP | UP | UP |
| avwap week | UP | UP | UP | UP | UP | UP | UP |
| pdmid day | UP | UP | UP | UP | UP | UP | UP |
| pdmid week | **DOWN** | **DOWN** | **DOWN** | **DOWN** | **DOWN** | UP | UP |

**Nine of seventeen read UP on every bar of the window.** Six read DOWN at
some point in it, and at the final bar the card's `fractal(2)` is the **only
one of the seventeen still reading DOWN**.

### Why fractal(2) said DOWN, and why that is not a bug

The rule needs a **higher high and a higher low**, both from swings confirmed
by the fractal test — a bar with two bars either side that do not beat it. Two
consequences on this morning:

- The rally was about six bars old at 06:00Z. A fractal high inside it cannot
  be confirmed until two bars have printed past it, so the newest *confirmed*
  pair still described the overnight dip.
- The 03Z bar printed a low of 4334.35 against 02Z's 4343.32. Once that
  became the newest confirmed low and the newest confirmed high was also
  lower, the rule reads DOWN — correctly, about a structure the market had
  already left.

**The lag is the price of the rule, not a defect in it.** A definition that
will not name a swing until the bars after it have printed cannot also be
instant. There is no definition in the table below that is both, and the
study's job is to price that trade-off rather than to pretend it away.

---

## 2. The trade-off: latency against whipsaw

H1, 25,708 bars. Reference turns are a **hindsight** ATR-zigzag — the turning
points a chart reader would point at afterwards, computed acausally on
purpose so that nothing causal is graded against its own output. Threshold is
`k × ATR at the bar`, never a fixed point value: gold ran 1,620 → 5,600 in
this sample and a constant threshold would mark a different kind of event in
2022 than in 2026.

754 turns at 4×ATR (one per 34 bars, median swing 140 bp); 182 at 8×ATR (one
per 141 bars, median swing 332 bp).

*lag* is the median bars from the turn until the label agrees. *early* is the
share of turns where it already agreed — not a virtue, it means the label was
on that side while the market was still going the other way. *missed* is the
share where it never agreed before the **next** turn; those are counted, never
dropped, because dropping them flatters a slow definition by deleting the
turns it slept through.

| definition | U/D/F % | flips /100 | undone ≤3 bars | lag (4×ATR) | early | missed | lag (8×ATR) |
|---|---|---|---|---|---|---|---|
| fractal(1) | 31/28/41 | 7.8 | 9% | 8 | 13% | 13% | 9 |
| **fractal(2)** | 31/27/43 | 4.7 | **1%** | 12 | 17% | 23% | 14 |
| fractal(3) | 32/26/42 | 3.3 | **0%** | 16 | 17% | 31% | 18 |
| zigzag 1.5×ATR | 54/46/0 | 27.8 | **65%** | **2** | 32% | 1% | 1 |
| **zigzag 3×ATR** | 55/45/0 | 6.1 | 16% | **6** | 8% | **1%** | 6 |
| dow closes(2) | 32/29/39 | 5.2 | 3% | 11 | 17% | 21% | 11 |
| ema 21/55 | 40/30/30 | **1.7** | 6% | 12 | 3% | 35% | 25 |
| ema 20/50/200 | 31/20/50 | **0.5** | 1% | 10 | 2% | **54%** | 52 |
| adx20 +DI | 36/33/31 | 4.3 | 26% | 10 | 5% | 19% | 11 |
| adx25 +DI | 26/23/50 | 2.7 | 18% | 13 | 3% | 37% | 15 |
| donchian20 | 53/46/0 | 3.2 | 6% | 13 | 8% | 16% | 12 |
| donchian55 | 53/47/0 | 1.1 | 1% | 25 | 31% | 38% | 29 |
| supertrend(10,3) | 52/48/0 | 2.5 | 2% | 13 | 12% | 19% | 12 |
| avwap day | 53/47/0 | 19.7 | 57% | 3 | 12% | 2% | 3 |
| avwap week | 54/46/0 | 9.2 | 53% | 6 | 15% | 15% | 11 |
| pdmid day | 55/45/0 | 8.0 | 48% | 8 | 9% | 9% | 10 |
| pdmid week | 56/43/0 | 2.8 | 46% | 13 | 33% | 33% | 29 |

The same table on H4 (6,728 bars, 167 turns at 4×ATR):

| definition | flips /100 | undone ≤3 | lag (4×ATR) | missed |
|---|---|---|---|---|
| fractal(1) | 7.2 | 5% | 7 | 12% |
| **fractal(2)** | 4.6 | **0%** | 12 | 21% |
| fractal(3) | 3.8 | 0% | 15 | 26% |
| zigzag 3×ATR | 5.6 | 9% | 6 | 4% |
| ema 21/55 | 1.8 | 9% | 16 | 29% |
| donchian20 | 2.6 | 3% | 10 | 16% |
| supertrend(10,3) | 2.4 | 1% | 10 | 12% |
| avwap day | 34.3 | **72%** | 2 | 0% |

**The trade-off is monotone and there is no free corner.** Read the fractal
family top to bottom: 8 → 12 → 16 bars of lag buys 9% → 1% → 0% of flips
undone within three bars. Read `zigzag 1.5×ATR` and `avwap day` as the
warning: two bars of lag, and **two thirds of their flips are reversed within
three bars.** A row that changes its mind twenty times per hundred bars is not
faster, it is noise with a direction attached.

**`zigzag 3×ATR` is the one cell that is not on the frontier's bad side.** Six
bars of lag — half of `fractal(2)` — with 16% undone and, decisively, it
**misses 1% of turns against fractal(2)'s 23%**.

---

## 3. Information: there is none, and this is the part that matters

Directional agreement between the label at bar *t* and the sign of the
forward move, on **non-overlapping** samples — 94% of overlapping windows
share a day with another and the t-statistic inflates about fourfold, which is
what closed `2026-09-15-pair-residual`.

Forward returns are taken over **wall-clock time, not a bar count**. This tape
stops an hour a day and all weekend; "24 bars ahead" on H1 crosses the break
and silently becomes thirty hours. That is fault 11 on this desk's own list.

The null **circularly shifts** the label series against the returns. A plain
shuffle would destroy the label's own run structure and make the baseline far
too tight — a definition holding one call for forty bars would beat a shuffle
without carrying any information at all. A shift keeps every run intact and
breaks only the alignment, which is the thing under test.

Best six at the 24-hour horizon, 2,000 draws:

| | definition | hit rate | vs null | z | null pct | n |
|---|---|---|---|---|---|---|
| H1 | ema 21/55 | 53.7% | +3.3 | **+1.96** | 95.0 | 886 |
| H1 | dow closes(2) | 52.9% | +2.8 | +1.67 | 94.0 | 882 |
| H1 | fractal(3) | 52.9% | +2.6 | +1.49 | 92.2 | 853 |
| H4 | fractal(2) | 54.1% | +3.7 | **+1.98** | 97.7 | 712 |
| H4 | ema 20/50/200 | 54.3% | +3.5 | +1.64 | 94.0 | 561 |
| H4 | donchian20 | 53.4% | +3.0 | +1.79 | 93.7 | 888 |

**At the 4-hour horizon nothing on either timeframe exceeds +1.9 points and
most are below the null.** At 24 hours the best is +3.7.

### The arithmetic that closes it

Seventeen definitions × two horizons = **34 tests**. At n≈890 the standard
error of a hit rate is **1.68 points**, so the best result is **z = +1.98**.

**The expected largest of 34 standard normals under pure noise is +1.94.**

The best cell in the entire grid is the size noise produces when you look 34
times. Not one definition stands out from the set, and the per-year split
says the same thing without any arithmetic:

| definition | 2022 | 2023 | 2024 | 2025 | 2026 |
|---|---|---|---|---|---|
| ema 21/55 | +3 | +7 | **−3** | +4 | +2 |
| dow closes(2) | −0 | +2 | +5 | +3 | +1 |
| fractal(3) | **−4** | +5 | +2 | +5 | +0 |
| fractal(2) | **−10** | −2 | −2 | +2 | **−4** |
| ema 20/50/200 | +8 | +4 | **−5** | +5 | −0 |

*(points above the circular-shift null; n per cell 101–205, so SE is 3.5–5.0
points and every one of these numbers is inside noise.)*

Every candidate has a negative year. Nothing here would have told anyone
which way to lean in 2024.

---

## 4. Anchor robustness

**H1 has no anchor question at all, and this is now measured rather than
argued.** Built from one 15m file on both anchors, the two H1 series share all
25,020 timestamps with **identical OHLC** — the broker's offset is a whole
number of hours, so hourly candles cannot differ. Every price-based definition
labels 0.0% of bars differently. Only the ones anchored to a *day* move:

| definition | H1 labels differing | H4 labels differing |
|---|---|---|
| donchian20 / donchian55 | 0.0% | **1.2% / 1.4%** |
| ema 20/50/200 | 0.0% | 5.1% |
| zigzag 3×ATR | 0.0% | 6.0% |
| supertrend(10,3) | 0.0% | 6.9% |
| ema 21/55 | 0.0% | 7.5% |
| adx25 +DI | 0.0% | 13.0% |
| fractal(3) | 0.0% | 16.0% |
| fractal(2) | 0.0% | 18.5% |
| fractal(1) | 0.0% | **29.5%** |
| dow closes(2) | 0.0% | **37.5%** |
| avwap day | **7.4%** | 21.6% |
| pdmid day | **7.5%** | 13.8% |

On H4 the two anchors share **no timestamp whatsoever** — broker candles open
21/01/05/09/13/17Z and epoch ones 00/04/08/12/16/20Z — so each series was
sampled at the other's bar closes, which is what a card would actually have
shown at one instant.

**A correction to how the step-0 finding has been quoted, including by me.**
"Structure was the only anchor-robust definition" was about the *statistic*:
its WITH-minus-AGAINST return held at +0.552R → +0.597R while EMA, pdmid and
Donchian flipped sign or collapsed. It was **not** about the labels, and the
labels are not robust — `fractal(2)` labels 18.5% of H4 bars differently
between anchors, and Donchian, whose statistic collapsed to +0.003R, is the
most label-stable definition in the table at 1.2%. Label stability and
statistic stability are different properties and the earlier sentence should
not be read as claiming both.

---

## 5. Recommendation

Because **no definition carries information**, this is a choice about what is
legible on a card, not about what predicts. That framing is the
recommendation's main content and should survive any disagreement with the
specific picks.

**H1 row: `zigzag 3×ATR`.** Six bars of median lag against `fractal(2)`'s
twelve, 16% of flips undone within three bars, and it misses 1% of turns
where `fractal(2)` misses 23%. On the owner's own morning it read UP on all
seven bars. It must be displayed as the **live** leg — the value in force at
that bar — and never redrawn, because a zigzag drawn afterwards looks
prescient and is not; every number above is the live version.

**H4 row: keep `fractal(2)`.** On H4 its whipsaw is the best in the table
(0% undone), the slower row is where lag is affordable, and it is what the
two registered HTF books read. Changing it would make their record two
campaigns under one id.

**Yes, the H1 row should be faster than the H4 row.** They are answering
different questions — "what is the chart doing now" and "what is the larger
structure" — and giving both the same rule is what produced a DOWN on a row
the owner was reading for the opposite purpose.

**If a single definition is wanted for both:** `supertrend(10,3)` — 2.5 flips
per 100 bars, 2% undone, 13 bars of lag, 0.0% anchor-sensitive on H1 and 6.9%
on H4. It is the best-behaved compromise, and it is slower than the zigzag.

**Do not use:** `zigzag 1.5×ATR`, `avwap day` and `pdmid day` on a card. They
turn in two or three bars and reverse **48–65%** of those turns within three
bars. `pdmid` additionally carries the sign flip between anchors from step 0
and is the one definition this desk has already measured changing its answer
with the bucketing.

### What this note does not license

No definition here should size a trade, gate an entry, or move a stop on the
strength of anything measured above. The information column is empty and the
prior it was meant to overturn stands: on the one-regime sample this desk
holds, no definition separated with-trend from against-trend, and the
registered `htf-filter` book is still the only pre-committed test of whether a
structure rule helps or hurts. **A bias row is a description of the chart. It
is not a signal, and the card should say so in as many words.**

---

## Addendum, 2026-09-18: the recommendation was independently reimplemented, and the reimplementation is why this section exists

d1 ported `zigzag 3×ATR` into the route in Rust and compared against this
study's own function rather than against its description. **Zero mismatches
on all 25,708 bars** — 14,214 UP / 11,481 DOWN / 13 FLAT, and UP on all seven
bars of the owner's morning.

Getting there required pinning four choices that the prose above did not:
highs and lows rather than closes; ATR at the **current** bar rather than at
the pivot, so the threshold moves under an open leg; the reversal taken on
the bar that crosses, so a closed-bar recomputation is identical and an
**intrabar** one is not; and FLAT only before the first pivot — 13 bars of
the 25,708, never again. They are now in `py/research/bias_defs.py`, which
is in the repository precisely because a definition that lives in one
session and a paragraph is not a definition.

**The finding worth keeping is d1's, and it validates this note's method
from the outside.** Three of d1's four independent choices differed from this
study's — `>=` where this has strictly `>`, down tested before up on a bar
that could seed either direction, and a running high *and* low before the
first pivot rather than one wandering scalar. **Every one of those three
reproduces the owner's morning.** Seven bars of agreement discriminates
between none of them; only the aggregate row does. Had the port been
validated against the case that prompted the study, three different rules
would have passed.

One property fell out of the comparison for free: d1's labels are derived in
one pass from the pivot list — the label at bar *t* is the direction set by
the last pivot confirmed at or before *t* — while this study's are streamed
bar by bar. Those agreeing on every one of 25,708 bars **is** the no-repaint
property holding on real data, and it is a better demonstration than a
synthetic prefix test because nothing about the data was chosen to make it
pass.

`crates/fd-api/tests/fixtures/zigzag-h1-xauusd.csv` now holds the last 2,000
bars with this function's own output as the label column, so `cargo test`
re-runs the comparison without needing the VPS export. **Changing
`atr_zigzag` will break that test**, which is the intended coupling: the
route must not drift away from the definition these numbers were measured
on.

---

*Study: `py/research/bias_defs.py`, `bias_ref.py`, `bias_measures.py`,
`bias_run.py`, run with `FD_BARS` pointing at the fingerprinted files above. One bug worth recording: the first reference-turn generator
initialised `direction = 0` with two unguarded branches, so at direction 0
both fired every bar and the only reversal it could detect was a single bar
whose range exceeded the threshold. Its first pivot was bar 21,918 of 25,708 —
the 2026-01-28 crash — and the 21,917 bars before it produced nothing. The
table it printed looked ordinary: latency 0 almost everywhere and 60% of turns
missed, which reads as "instant but unreliable" and was entirely an artefact.
It was caught by asking why a 121-point threshold was producing eight-bar
legs.*
