# 2026-09-18-smc-context: structural price levels as CONTEXT for a model, which is not the claim that failed

**Registered:** 2026-09-18, **before the route it reads exists** and before
either book has seen a bar. Everything below is a pre-commitment; nothing in
it may be changed by what the books turn out to say.
**Status:** registered, NOT built and NOT started. The block is written only
once d1's `/api/paper/levels` is on `main`; the books are started by a5 in a
quiet window after it is deployed.
**Books:** `ai-xau-ds-ctx` (base, already running, untouched) as control,
against `ai-xau-ds-ctx-smc` (variant `smc-context`), with its own coin
`ai-xau-ds-ctx-smc-coin` and **seed 43** — distinct from 23, 29, 37 and 41.
**Model:** `deepseek-flash`, the same as its control. A variant tested
against a different model's book measures the model.
**Code, when it is written:** `py/live/smc_context.py`,
`py/live/ai_trader.py` (`VARIANTS`), `py/live/start_ai_traders.ps1`,
`py/live/start_ai_runs.py`, `py/live/prompt_variant_selftest.py`.

## The claim, and why it is not the one that already failed

**This claim:** *a model deciding from 15m bars alone decides differently
when it can also see where the structural price levels are.* It is a claim
about the **decision**, not about the levels: they could change what the
model does while predicting nothing, in which case the answer is that they
change decisions for the worse and the experiment says so.

**The claim that failed, repeatedly, is that these levels predict price as
mechanical rules.** Named, because "price levels" is a phrase that has
already cost this desk a great deal:

- **`2026-09-13-ict-sweep-mss-fvg`** — the same family as this route's
  content. The full ICT chain (higher-timeframe FVG → liquidity sweep →
  market structure shift → lower-timeframe gap → retrace entry) passed its
  gate on the only three months of broker M1 available, then lost on four
  years of out-of-sample minutes: **PF 0.75–0.77, expectancy −0.17 to
  −0.19R, 613–1,428 trades**, with the direction rule already at the 79th
  percentile of a coin flip in sample. Closed.
- **`2026-09-13-pdhl`** — yesterday's high and low, as a fade and as a break.
  **Every row at the 1st–6th percentile** of a null gated to its own session:
  worse than random entry in the same hours. Closed without opening the
  confirmation window.
- **`2026-09-13-vwap-fade`** — price stretched from VWAP. PF 0.951 on 5,828
  trades, direction null at the **13th–15th percentile**, so if anything a
  stretch continues. Closed.
- **`2026-09-13-london-range`, `2026-09-13-orb-ny`, `2026-09-13-gap-fade`,
  `2026-09-13-volcond-breakout`, `2026-09-14-volman-box`** — session and
  range extremes as breakout or fade triggers. All closed, none survived.
- And the **twenty-five** registrations that tested the *options* tape's
  levels — POC, value area, gamma walls, whale support and resistance, max
  pain — as mechanical rules. None survived. That family is the subject of
  `2026-09-17-otl-context`, which is the sister experiment to this one and
  asks the same kind of question about a different set of levels.

**So the prior is not neutral and this registration does not pretend it is.**
Every mechanical use of these levels this desk has tested has failed. What
has never been tested is whether a model that can see them decides
differently — and that is a question about the model, answerable in days
rather than months, with a stage-1 kill that costs one extra book.

## What the levels route is, and what it is NOT

The block reads `/api/paper/levels?market=xauusd&tf=15m` at decision time.
Per d1's specification it serves: POC / VAH / VAL from the activity profile,
unfilled fair value gaps, order blocks, buy-side and sell-side liquidity with
their swept state, and session / day / week extremes — each carrying a price
or a band, the bar it formed on, its age, its state, and **the rule that
produced it**.

**It carries no verdict.** No composite score, no "bullish structure", no
zones labelled with words, no votes between levels. If the route ever grows
one, this book must not read it: a scored level is the route deciding, and
this experiment is about whether the *model* decides differently when shown
facts. That is a pre-commitment, not a preference.

## What is added to the prompt, and nothing else is

`smc-context` = base **plus one block**, `PRICE LEVELS`, inside the existing
`MARKET CONTEXT — computed from the same bars, for convenience; none of it is
a signal` section, under that same framing. The coin clause is the **base**
one: this variant changes one thing, not two.

The block lists the nearest levels **above** and **below** the last close,
each with its kind, its distance in **points and in ATR**, its **age**, and
its **state**. Pre-committed properties:

- **A fixed maximum count**, stated in the block itself, so the prompt cannot
  grow with the market's mess. A prompt whose length depends on how many
  order blocks happen to be unfilled is a prompt whose token cost and whose
  legibility vary with the thing being measured.
- **The ordering rule is stated in the block**, so the model is not left to
  infer why these levels and not others. Nearest-first by distance from the
  last close, above and below listed separately.
- **Every number carries a unit.** Prices in the market's quote units, named;
  distances in both points and ATR, because a distance without a volatility
  scale is not comparable across regimes. This desk spent 2026-09-17
  removing five numbers whose units lived only in prose.
- **No level is ranked, scored or recommended by the block.** It reports what
  the route reported, in our words, in distance order.

## Failure is a result, not an exception

Route unreachable, stale, or thin: the block **says so in its own words**,
the decision row records it, and there is **no silent fallback to the base
prompt**. A context-absent decision inside a context-present book makes the
record a mixture with nothing to separate it.

**The route's own text never reaches the prompt** — only our composition of
its numbers. This is the lesson of `9a5dbfb` and it is a pre-commitment here
rather than a discovery later: `htf_context` printed the route's
`unavailable` sentence verbatim, and that sentence is a join written in
another crate, so adding a timeframe to the route would have changed the
wording inside a registered campaign's prompt with nobody editing the book
and no diff to notice. A prompt variant whose wording depends on another
file's prose is not a controlled variable.

## The cold-window confound

Identical to the rule registered for the other three books on 2026-09-18, and
pre-committed here before this book exists:

A new book starts with an **empty window** — the poller feeds it one bar and
the trader decides on that bar while the control decides on forty. A row
counts toward every headline number **only when the book saw a full
forty-bar window**, measured from the row's **own stored prompt**, which
carries `LAST <n> BARS of <market>:<tf>`; the row is excluded when `n < 40`.
Not "the first forty rows": position is a proxy for window size and a bad one,
because a restart or a missed bar makes the fortieth row and the fortieth bar
different things. **Both arms are filtered by the same rule**, the control's
rows on those bars included.

## What will be measured, in this order

**Stage 1 — disagreement rate. The cheap kill, and it runs first.**
On bars where both the control and `smc-context` produced a decision, the
block state read `ok`, and **both** saw a full window, the fraction on which
the two chose differently (counting NONE as a choice).

> **Pre-committed:** if the two disagree on **fewer than 10% of the first 100
> shared warm `ok` bars**, the levels are not changing the decision, the claim
> is answered NO, and **both books are stopped and the result recorded.**

**Stage 2 — trade count, then win rate**, for the variant against the control
over the same bars. Reported before any R figure, because a book that barely
trades produces an R number with no sample behind it and the R number is the
one people read first.

> **Pre-committed MINIMUM:** fewer than **a third of the control's entries
> over the same 100 decided bars** and the book is abandoned and recorded as
> "the levels closed the book". The control's measured rate as of 2026-09-17
> is 7 entries in 56 decided bars, so the floor is roughly 4 per 100.

**Stage 3 — R per trade and total R, against the control and against its own
coin, each with its N.** No R figure is reported without its trade count
beside it. The coin is what says whether a difference is the prompt or the
bars.

## What would make this wrong, stated now

- **Pseudo-replication.** Books deciding on the same bars are not independent
  samples. Comparisons are per shared bar and the N reported is the number of
  shared bars, not the number of rows.
- **Multiple comparisons across the family.** This is the **third** prompt
  variant on the same control, after `otl-context` and `htf-context`. Three
  variants each given a 10% disagreement gate and an R comparison is three
  chances at a false positive, and the 2026-09-18 bias study is the standing
  reminder of what that costs: across 34 tests the largest z was +1.98 when
  noise alone predicts +1.94. **A positive R result from any one of the three
  is to be read against three, not one**, and this file says so before any of
  them reports.
- **The levels are not independent of the bars.** Every level the route
  serves is computed from the same price history the model already sees in
  its forty bars. The block cannot add information the bars do not contain;
  at most it makes some of it salient. A positive result is therefore a claim
  about attention, not about data, and should be described that way.
- **Small n.** The control takes about 7 entries per 56 decided bars. Stage 3
  will not have a usable R sample for weeks, and stage 1 is the only part of
  this expected to answer quickly.

## Selftest, pre-committed

The same shape as `prompt_variant_selftest.py` already applies to the other
variants, and the three-file rule from `campaign_wiring_selftest.py`:

1. base, `no-coin-penalty`, `otl-context`, `htf-context` and `htf-filter`
   render **byte-identical** to before the change;
2. `smc-context` **is** base plus the block — remove the block and base
   returns exactly;
3. every level line in the block carries a unit;
4. a marker fed through any route-supplied string appears **nowhere** in the
   rendered block, in every state;
5. the variant, the launcher row and the `start_ai_runs` book all exist and
   agree — the three-file rule, which is in place because two of the three
   were done on 2026-09-18 and the third was not, and two traders logged
   `HTTP 404` for half an hour while reporting nothing wrong.

## Signed

Written by Claude Opus 5 (session `backcom-vantage`, 2026-09-18) before
`/api/paper/levels` exists, so that the record of what was expected predates
the first number. Scope set by a5 on the owner's instruction; the route is
d1's.
