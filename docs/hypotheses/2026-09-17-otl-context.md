# 2026-09-17-otl-context: options-flow positioning as CONTEXT for a model, which is not the claim that failed twenty-five times

**Registered:** 2026-09-17, **before the book has seen a single bar.**
Everything below is a pre-commitment; nothing in it may be changed by what the
books turn out to say.
**Status:** registered, running from the next launcher start
**Books:** `ai-xau-ds-ctx` (base, already running, untouched) against
`ai-xau-ds-ctx-otl` (variant `otl-context`), each with its own coin
**Code:** `py/live/otl_context.py`, `py/live/ai_trader.py` `VARIANTS`

## This is a new hypothesis, and it is not the old one wearing a new name

Twenty-five registrations in `docs/decisions/` tested levels of exactly this
kind — POC, value area, gamma walls, whale support and resistance, max pain —
as **mechanical rules**, and not one survived out of sample. That record is
the reason this file has to say what it is doing differently before it starts,
because "options levels" is a phrase that has already cost this desk a great
deal.

The old claim: *these levels predict price, so trade them.* Dead, repeatedly.

This claim: *a model that is deciding from bars alone decides differently when
it can also see where the option premium sits.* It is a claim about the
**decision**, not about the level, and the two are independent — the context
could change what the model does while predicting nothing, in which case the
answer is that it changes decisions for the worse and the experiment says so.

Stage one below deliberately measures only whether the decisions differ at
all. If they do not, nothing else matters and the question is closed in a day
for the price of one extra API book.

## What is added, and nothing else is

One block inside the existing `MARKET CONTEXT — computed from the same bars,
for convenience; none of it is a signal` section, under that same framing. The
coin clause is the **base** one: this variant changes one thing, not two.

Contents, each line with its unit, and the block states in its own text that
the numbers are computed by a third party from the COMEX gold options tape and
describe **positioning, not direction**:

- gamma wall (`summary.max_gex_strike`), USD/oz
- all-DTE POC / VAH / VAL, USD/oz
- whale support / resistance across every contract in the window, USD/oz
- ATM futures price and the 1-day expected move, USD/oz
- average IV now against N hours ago, as both numbers and the difference
- bull vs bear premium for daily-expiry and weekly-expiry contracts, USD,
  each stating the window it covers
- the largest prints of the last 8 hours: strike, C/P, aggressor side, premium

**The premium lines say "daily-expiry" and "weekly-expiry" and never "0DTE",**
and the difference is not pedantry. The feed's contract class is `daily`,
`weekly` or `monthly`; days-to-expiry is a separate quantity this block does
not compute. A daily-expiry contract is usually today's and is not always, so
a line labelled 0DTE would be asserting something unchecked. The selftest
fails if the string "0DTE" appears in the block.

Bull and bear follow the feed's own four buckets, verified against a real
response: bull is a call bought or a put sold, bear is a put bought or a call
sold, with `side` read as the aggressor. The `premium` column is used as
published rather than recomputed as price x size x 100 — the two agree on 173
of 200 sampled prints and the feed's own number is the one its other
endpoints are consistent with.

**What the endpoint actually is**, since the methodology table is misleading
and cost a round: `alldte-data?tf=` is not a snapshot aggregate. It is a
lookback window over the whole tape. `tf=weekly` returned 7,915 prints across
3.8 days and 39 contracts of every class, each print carrying its symbol, with
a `contracts` list mapping symbol to class and the level columns flat rather
than run-length encoded. `tf=daily` returns a bare `[]` at some hours. So the
block takes `tf=weekly` — the only window that can produce a weekly figure —
and one fetch yields the premium split, the whale levels and the window, which
is fewer calls than the first draft made.

**The clock.** The feed renders every time in UTC+7, including strings that
carry a `+00:00` suffix which is not true — measured 2026-09-12, pinned by a
test in fd-ingest, recorded at `docs/decisions/2026-09-12-otl-timestamps.md`.
Seven hours are subtracted at parse time and the block prints UTC. The offset
is read from `config/default.toml` rather than written into the new module, so
one file still owns it.

## The failure mode is recorded, not papered over

If the feed is unreachable, or its newest number is older than one bar, the
block says so **in the prompt** — so the model knows it is deciding without
the context — and the row carries `otl: unavailable` or `otl: stale <n>m` in
`decisions.jsonl`.

There is no fallback to the base prompt. A silent fallback would put
base-prompt decisions inside this book's numbers with nothing in the record
able to separate them, and the comparison would then be against a mixture. The
bars where the context was absent are excluded from the primary measure and
counted separately; if they are more than a third of the sample the result is
reported as inconclusive on availability grounds rather than quietly averaged.

## What settles it, in order of cost

**(1) Disagreement rate, and this is the cheap one that gates the rest.** On
bars where both books were asked and the feed was present, how often do the
two sides differ — counting NONE as a side, so standing aside against entering
is a disagreement.

**N = 60 shared bars with the feed present.** At deepseek-flash's measured
entry rate of 7/77 = 9.1% on the base prompt, two independent books at that
rate disagree on about 17% of bars by chance alone; 60 bars is enough to
separate "inert" from "changes something" without waiting for trades.

- **Disagreement ≥ 25% — the context changes decisions.** Proceed to (2).
- **Disagreement ≤ 5% — the context is inert and this is closed.** No further
  measure is taken, because a context that does not change the decision cannot
  change the result, and the remaining questions are moot.
- **Between — extend once to 120 shared bars, then decide on the same
  thresholds.** One extension, declared now.

**(2) Trade count**, same as the opus-b registration: does the -otl book
enter at a rate comparable to its control, or does the extra context make it
more cautious, or less? Reported, not gated on.

**(3) R against its coin, and against the control's R on shared bars.** This
is the expensive one and it is honest to say how expensive: from the desk's
own per-trade dispersion — mean +0.127R, sd 1.063R, n = 55 — distinguishing a
+0.127R edge needs about 270 trades, which at a 9% entry rate is 3,000 bars,
roughly 31 trading days. **A profitable first week is not this arriving early.
It is noise, and this file says so in advance.**

## What would falsify it

- **Disagreement near zero.** The model reads the block and decides the same
  thing anyway. This is a real result and the likely one, given that a model
  asked to name what in the bars it is acting on may simply ignore a block it
  was told is not a signal.
- **-otl's R not distinguishable from the control's** after (3)'s sample. Then
  the context changed decisions without improving them, which is the second
  most likely outcome and is worth recording as clearly as the first.
- **The feed absent on more than a third of bars.** Then the experiment did
  not run, whatever the numbers say, and the honest report is that.

## What this does not say

It does not revive the twenty-five closed registrations. Nothing here claims
the levels predict price, and a positive result at stage (1) would claim only
that a model behaves differently when shown them.

It does not test the block's *design*. Which fields, how many hours of IV
history, which contract the whale levels come from — all of those are choices
made once, before the data, and a later result cannot be used to justify
having picked differently. Changing any of them is a new registration.

It does not test Opus. `ai-xau-ds-ctx-otl` runs deepseek-flash precisely
because that model already makes some entries on the base prompt; running this
on a book that has never traded would measure nothing twice.

And it does not measure the feed's own quality. The numbers are taken as the
third party publishes them. If they are wrong, this experiment measures the
effect of being shown wrong numbers — which is still a fact about the
decision, and is not a fact about gold.
