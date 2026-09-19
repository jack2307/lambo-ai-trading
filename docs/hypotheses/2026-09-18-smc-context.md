# 2026-09-18-smc-context: structural price levels as CONTEXT for a model, which is not the claim that failed

**Registered:** 2026-09-18, **before the route it reads exists** and before
either book has seen a bar. Everything below is a pre-commitment; nothing in
it may be changed by what the books turn out to say.
**Status:** BUILT 2026-09-19 on branch `agent/smc-prompt`, RECONCILED the same
day against the route as it actually shipped and then against its LIVE output,
NOT started. The block, the
prompt variant and the three-file wiring exist; the selftest now runs against
d1's own captured responses — `docs/api-samples/paper-levels.json`, 252 levels
off the live store, and `paper-levels-unavailable.json` — rather than against
a fixture invented from prose, and that reconciliation found two unit bugs a
hand-made fixture could not have caught. The books are started by a5 in a
quiet window once the route is deployed to the VPS. What was built and the
three places it departs from the lines below are in "What is now built" at
the foot of this file; what the shipped route then changed — including the
two unit bugs and four paragraphs of that section it corrects — is in
"Reconciled with the route as shipped" after it; the live rendering then
forced one more change, in "One price, one line" at the very foot, which is
the one to read for what the code does today.
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

## What is now built (amendment, 2026-09-19)

Written after the code and before any bar. The claim, the falsifier and the
three stages above are untouched: they were pre-committed and nothing here
may move them.

> **Written before the route shipped, and four of its paragraphs are now
> wrong.** They are left exactly as written rather than edited, because what
> this session assumed from a prose brief is part of the record. The
> corrections are in "Reconciled with the route as shipped" below, which is
> the section to read for what the code does today.

**The files.** `py/live/smc_context.py` (the block), the `smc-context` entry
in `py/live/ai_trader.py`'s `VARIANTS` with a `{smc_block}` slot beside the
HTF and OTL slots, the launcher row in `py/live/start_ai_traders.ps1`, the
book in `py/live/start_ai_runs.py`, and `py/live/smc_context_selftest.py`.
`py/live/prompt_variant_selftest.py` pins the variant list at eight.
The three-file rule is covered: `campaign_wiring_selftest.py` passes, which
is what would have caught the 2026-09-18 pair that logged `HTTP 404` for half
an hour while reporting nothing wrong.

**What the block shows.** One header saying what it is and what it is not,
then the levels the route served, in this file's own words: activity profile
(POC, value area high and low), unfilled fair value gaps with the fraction
filled, order blocks with UNTESTED / TESTED / BROKEN, buy- and sell-side
liquidity with `swept` and the bar that swept it, and session / day / week
extremes. Each line carries the price or the two-price band in the market's
quote units, the distance from the last close in **both** price and ATR(14),
the **age in bars** of the timeframe named on the line, and the **state**.
Ordering is nearest-first by distance from the last close, **above and below
listed separately**, with a third short section for a band the last close is
inside; the distance to a band is to its nearer edge.

**The cap: six a side, stated in the block.** The prompt ends with the forty
15m bars the model decides on. Measured: `htf_context` renders 43 lines on
its fixture and `otl_context` 17; at six a side this block renders 30 on a
full book and 27 with nothing straddling, and the selftest pins that ceiling
and pins that flooding the route with 120 levels does not lengthen it. At
twelve a side the arithmetic is 42 lines — level with the HTF block and
longer than the bars it sits in front of. Levels past the cap are **counted
on their own line**, not dropped silently: how many there are is itself a
fact about the tape.

**Failure states.** `ok` / `stale` / `thin` / `unavailable`, and every one of
them renders a block that says which. Staleness is judged against the bar
being decided, not the wall clock, on the finest timeframe the route reports
provenance for — the same rule and the same two-bar threshold as
`htf_context`, because a wall-clock threshold cannot tell a stopped export
from a weekend. There is no fallback to the base prompt on any of them, and
every decision row carries `smc` (`ok <n> levels`, `thin`, `stale <n> bars`,
`unavailable`, or `n/a`) so stage 1 can count only the warm `ok` bars.

**The route's own strings reach nothing.** Stricter than `htf_context` had to
be, because this route carries *the name of the rule that produced each
level* and that name is another crate's prose. It is recorded in `gather` and
never rendered. `kind` and `state` are mapped through this file's own
vocabulary and anything outside it renders as "not in this block's
vocabulary". The two exceptions are parsed, not printed: a swing id matching
`(h4|d1|…)-(hi|lo)-<ms>` is re-rendered as "the H4 swing high from
2026-09-18 12:00Z", and a timeframe token matching `\d{1,3}[mhdw]` is
re-rendered lowercase. The selftest feeds a marker through every one of those
fields, in all four states, and checks it appears nowhere.

**Two departures from the lines at the head of this file, recorded rather
than quietly applied.** They were set by a5 in the build instruction and they
do not touch the claim.

- **The book ids are `ai-xau-ds-smc` and `ai-xau-ds-smc-coin`**, not
  `ai-xau-ds-ctx-smc` / `-ctx-smc-coin`. The control is unchanged:
  `ai-xau-ds-ctx`, same model, same bars.
- **The seed is 53, not 43.** 43 had been taken by `ai-xau-ds-plan` on
  2026-09-18, between this registration being written and the code being
  written. Two books on one seed share a coin's luck on every bar where both
  traded, which is the one thing a control may not do.
- **The route is called as `?market=xauusd`, with no `&tf=15m`.** The
  contract a5 handed over serves every timeframe at once and carries
  provenance per timeframe, so the block shows levels from all of them and
  names each one's timeframe on its own line. That is more than this file
  said it would show, not less, and it is why an age has to carry a
  timeframe: "4 bars" is forty minutes on 15m and sixteen hours on 4h.

**Nothing is started.** No process was launched, no book was created, and
`/api/paper/levels` does not exist yet. Until it does, every decision this
book would make carries `smc: unavailable` — a recorded result, but not the
experiment, which is why a5 starts it only after the route is deployed.

Written by Claude Opus 5 (trader session, worktree `fd-wt-smc`, branch
`agent/smc-prompt`), 2026-09-19.

## Reconciled with the route as shipped (2026-09-19, later the same day)

`/api/paper/levels` landed on `main` with two captured samples and the DTO
doc comments. The samples are the contract now; the prose brief the section
above was written from is superseded wherever they disagree, and they
disagreed in eight field names, in the grouping, in being single-timeframe,
and in two units. The claim, the falsifier and the three stages are still
untouched.

**The selftest's fixtures are now the route's own responses**, not a
paraphrase of them: `docs/api-samples/paper-levels.json` (120 KB, 252 levels
off the live store) and `paper-levels-unavailable.json`. That change is worth
more than the renames, because everything passed against the invented fixture
and two of the bugs below are ones an invented fixture cannot contain.

**Two unit bugs, both found by rendering the real response.**

- **Staleness was measured with `htf_context`'s arithmetic and this route is
  not `htf`.** `htf` compares an H4 bar to a 15m decision bar, where the
  newest closed H4 bar is always at least one whole H4 bar behind, so it
  measures from that bar's close. This route runs ON the decision timeframe:
  its `computed_at_bar_ms` and the decision bar are the same series and on a
  healthy desk the same bar. The close-to-start formula reported a perfectly
  current response as **minus one bar behind**. It is start to start now.
- **A millisecond gap divided by `bar_ms` is not a bar count.** The market is
  shut about a third of the time: the sample's window holds **879 stored bars
  across 1,308 bar-lengths of clock**, and the route's own `age_bars` counts
  the 879. The block had derived "swept N bars ago" that way and printed a
  pool as *swept 675 bars ago when it was 594 bars old* — swept before it
  existed. Sweeps are a UTC stamp now, and the staleness tolerance is stated
  in **minutes of clock** rather than dressed up as bars.

**The staleness tolerance is now the measured daily hole plus two bars**, 90
minutes on 15m, and this is a correction to the section above rather than a
rename. `htf.rs` measured hour 21Z holding exactly zero 15m bars against
332–348 in every neighbouring hour, so an export exactly ONE stored bar
behind at the daily roll is over an hour of clock behind. A flat two-bar
tolerance in clock would have withheld the block once a night. There is a
test for that case.

**What the real response changed in the block.**

- **Single timeframe.** `timeframe` and `bar_ms` are top-level and no level
  carries its own, so the timeframe is named once in the header and ages are
  bare bar counts. The third departure bullet above — that the route serves
  every timeframe at once — is simply wrong and the per-line timeframe it
  justified is gone. The route is called `?market=<id>`, as that bullet said.
- **Swing ids are not printed at all.** They are all this response's own 15m
  swings, so nothing the model can see in this prompt joins to them, and each
  is another crate's string. What is printed is how many swings a pool is
  made of and how far apart they sit in ATR, which are numbers. The `SWING_ID`
  and timeframe-token parsers the section above describes are deleted with
  the multi-timeframe contract that needed them.
- **`direction` is rendered as what happened, never as BULLISH or BEARISH.**
  The engine's own doc comment says the name "says which side of price the
  imbalance is on and NOT what price will do next". Printing the word invites
  exactly the reading forty closed registrations refuted, so a gap reads
  "left by an up move" and a block "before a down move".
- **A census in the header, which is new and is not a ranking.** The route
  caps and ranks nothing on purpose, and the sample carries 252 levels of
  which **135 of 155 liquidity pools are already swept and 62 of 76 order
  blocks already broken**. A model shown the nearest twelve with no idea they
  were twelve of 252, most of them spent, would read a tidy tape. Two header
  lines say how many there were, by family, with the spent count.
- **The POC is measured from its price, not its bucket.** It is the one level
  carrying both, and the bucket is an artefact of the histogram's resolution.

**The cap stays at six a side and the measured length changes.** On the real
response the block is **35 lines** — 12 of header and census, 9 a side, 5 for
the three bands the live close happens to sit inside — against `htf_context`'s
43, `otl_context`'s 17 and the prompt's forty bars. At twelve a side the same
response renders 47, which is longer than the bars it sits in front of. The
selftest pins the ceiling and pins that a nine-level tape is no shorter, so
the cost does not move with the thing being measured.

**Still not started.** The route is on `main`; nothing has been launched, no
book created, and nothing deployed. Until the route is on the VPS every
decision would carry `smc: unavailable`, which is a recorded result and not
the experiment.

**One thing in the samples that is still ambiguous, for d1.** The prior day's
high appears **twice** at the same price: once as a `PRIOR_DAY_HIGH`
liquidity pool carrying `swept` and the bar that swept it, and once as
`extremes.day.high` carrying `COMPLETE`. Both are real and they say different
things, so the block prints both rather than picking — but on the live sample
that spends two of the six slots above the close on one price. Deduplicating
would be this block deciding which of two route facts matters, which the
registration forbids it from doing, so it is raised here instead.

Written by Claude Opus 5 (trader session, worktree `fd-wt-smc`, branch
`agent/smc-prompt`), 2026-09-19.

## One price, one line (amendment, 2026-09-19, third pass)

The ambiguity raised at the foot of the section above got worse when the
block was rendered against the **live** route rather than the captured
sample. Three of the six slots above the close were one number said three
ways — an equal-highs pool, the `PRIOR_DAY_HIGH` pool and
`extremes.day.high`, all at 4381.20, all +2.88 away, all 126 bars old — and
the cap then hid 88 other levels behind them. Half the above-side context was
one price.

**a5's decision, 2026-09-19: collapse in the block, not on the route.** The
route is right to serve all three; they are three different questions that
happen to have one answer today, and deduplicating on the route would destroy
that distinction for every other reader. So levels the block would print at
the same price render as **one line carrying every fact the constituents
had** — each kind, each state, the sweep with its stamp, the pool size, the
spread. Where they disagree on a state, as SWEPT and COMPLETE do here because
they answer different questions, **both states are on the line**.

**This is not the ranking clause above.** That clause forbids the block from
choosing which of two facts matters. This keeps both and stops repeating a
price; one line naming three facts at one price carries strictly more than
three lines naming one price three times. The code says so in those terms at
`smc_context.COLLAPSE_TOL_PRICE`, so the next reader does not undo it as a
violation.

**The tolerance is half of the last digit the block prints — 0.005 quote
units — and not a fraction of ATR, because the fraction of ATR was tried and
measured wrong.** A hundredth of an ATR is 0.125 USD/oz at the sample's ATR
of 12.46, and it folded an equal-highs pool at 4367.60 into the prior day's
high at 4367.48: two levels the route calls separate, twelve cents apart,
printed under one price and one distance. That is not "stop repeating a
price", it is inventing one. The rule is the reader's instead — levels are
folded only when the block **would have printed them identically** — with a
relative floor for float noise, since one price arriving down three code
paths need not be bit-identical. Like with like only: a price inside a band
is not the same level, and two bands agree only when both edges do.

**What it frees.** On the live shape, three objects at one price become one
line: **two of the six slots above the close**, and the two levels that move
up into them were previously among the 88 hidden. On the captured sample the
two-way case at 4367.48 frees **one** slot, and seven levels response-wide
are folded. The census counts them — "7 of them sit at a price another level
already names and are folded into its line rather than repeating it" — the
same honesty the cap's overflow line owes, and the overflow line now counts
levels **and** prices when the two differ.

The block is 36 lines on the captured sample, against `htf_context`'s 43 and
the forty bars it precedes. The cap stays at six a side.

**Still nothing started.** The launcher row for `ai-xau-ds-smc` is commented
on `main`, deliberately, so the books and the trader exist only when the
owner decides to spend on them. It was not uncommented, and the selftest now
**asserts** it is commented rather than merely noting it.

Written by Claude Opus 5 (trader session, worktree `fd-wt-smc`, branch
`agent/smc-prompt`), 2026-09-19.
