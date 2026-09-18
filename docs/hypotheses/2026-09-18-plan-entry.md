# 2026-09-18-plan-entry: a model that can name its entry price beats the same model taking the next open

**Registered:** 2026-09-18, **before either book has seen a single bar, and
before the books exist.** Everything below is a pre-commitment; nothing in it
may be changed by what the books turn out to say.
**Status:** registered, not started. The API half (pending orders on the paper
book, `cancelled_unfilled` rows, `run.pending_order`) is being built by the
engine sessions against the contract quoted below; the Python half is on
branch `agent/plan-prompt` and mocks that contract in its tests. The books are
NOT created and nothing is started until both halves are on `main` and
deployed.
**Books:** `ai-xau-ds-ctx` (base, already running, untouched) as control,
against `ai-xau-ds-plan` (variant `plan`) with its own coin
`ai-xau-ds-plan-coin`, seed 43.
**Code:** `py/live/ai_trader.py` (`VARIANTS`, `PLAN_BLOCK`, `parse(plan=)`,
`sane`, `plan_body`, `coin_plan`), `py/live/plan_entry_selftest.py`,
`py/live/prompt_variant_selftest.py`, `py/live/mt5_executor.py`
(`--mirror-pending`, OFF), the fd-api pending-order routes (engine sessions).
**Plan:** stage 1 of `docs/plans/2026-09-18-staged-ai-entry.md`.

## Claim

A model deciding from the same 15m bars, under the same rules, enters better
when it may say **where** it wants in - a limit below the last close for a
LONG, a stop above it - instead of taking the next bar's open or standing
aside. "Better" is one number: **net R over the same bars is higher than the
market variant's**, after the trades that never fill are counted against it.

The mechanism, stated so it can be wrong: the model's stand-asides are often
"the level is 6 points away". Today that is a NONE and the trade never
happens; the coin-penalty registration measured 7 entries in 77 real answers
for this model. A limit lets that answer become a trade that fills only if
the pullback comes, at the price the model named. The cost is every pullback
that does not come while the trade would have worked from the open.

## The prior, and it is against the claim

Two measurements already on this desk say limit entries should NOT be
expected to help, and they are written here so a positive result is read as
a surprise and a negative one is not read as a discovery.

1. **On bare bars, entry drift carries no information.** `mt5_executor.py`,
   above `--max-join-r`: 81,789 observations of 15m gold 2022-06 to 2026-09,
   R = 1.2 x ATR14, 16-bar horizon, measured from the mirror's own fill:
   after adverse drift +0.0926R, inside the band +0.0992R, after favourable
   +0.0938R. Equal within 0.007R. A better entry is worth exactly its
   distance and nothing more.

2. **Conditional on a signal, a better price was the first leg to the
   stop.** Same file, `--allow-favourable-join`: replaying favourable joins
   over the 55 live entries to 2026-09-17, 7 of the 8 trades it added were
   ones the book was later stopped out of, against a 38% base rate
   (p = 0.006, n = 8), for -1.20R after the mirror's own fill. **A limit
   entry is a favourable-drift join by construction**: it fills only when
   the price has come back against the direction of the trade. That is the
   argument this registration has to beat, and n = 8 is small enough that it
   can be beaten.

The one number on the other side: the boundary-market cost of the next-open
fill measured 0.14 pt (`docs/plans/2026-09-18-staged-ai-entry.md`, from the
entry-lag work). Small. A limit has to save more than that on the trades that
fill to pay for the ones that do not.

## What the variant changes, in full

`plan` = base **plus one block**, `PLAN_BLOCK` in `ai_trader.py`, appended
after the answer-shape paragraph and before the hourly blocks so it sits in
the cacheable prefix. The coin clause is the **base** one. No options block,
no higher-timeframe block. The selftest asserts that removing the block from
the rendered prompt returns base exactly.

The block replaces the answer shape with:

```
{"side": "LONG" | "SHORT" | "NONE",
 "entry": {"type": "market" | "limit" | "stop", "price": <price or null>},
 "zone": [<lo>, <hi>] or null, "stop": <price>, "target": <price or null>,
 "valid_bars": <1 to 4>, "invalidate_above": <price or null>, "invalidate_below": <price or null>,
 "reason": "<one sentence>"}
```

and states the rules the API enforces: a limit sits on the pullback side of
the last close, a stop on the breakout side; an order still unfilled after
`valid_bars` bars is **cancelled and counted as a missed trade, never as a
stand-aside**; stop and target are judged against the ENTRY price. It says
nothing about WHEN a limit is better than a market entry - that is the
question, and a sentence about it would be the answer leaking in.

`sane()` refuses, locally and on the record (`refused_locally`): a limit on
the wrong side of the last close, a stop-entry on the wrong side, a stop or
target on the wrong side of the entry, `valid_bars` outside 1..4, a malformed
zone, an entry type that is not one of the three (including an omitted one -
it is NOT quietly read as market). A market entry is checked exactly as the
market books are.

**The coin** takes the same entry TYPE and the same `valid_bars` at mirrored
distances: as-is when it lands on the book's side, reflected about the last
close when it lands on the other, with the two invalidation levels swapping
roles. A limit 6 below on the book is a limit 6 above on the coin. It shares
the fill mechanics, so the difference between the two books is direction and
level choice, not mechanics. (`coin_plan`; the market books' coin arithmetic
is untouched and the selftest pins it bit for bit.)

**While an order waits, the book is not asked again.** A new bar with
`run.pending_order` non-null is a `pending_skip` row and posts nothing: a new
intent - a NONE included - replaces the pending order, and `valid_bars` would
then never mean what it says. The order lives or dies by its own rules.

## The API contract this is written against

Quoted so the record says what the trader assumed, whatever the engine ends
up serving:

- `POST /api/paper/intent` takes `entry: {type, price}`, `zone`, `valid_bars`
  (limit/stop only, default 2), `invalidate_above/below`; one pending order
  per run; a new intent replaces it.
- `/api/paper/run/{id}` and `/api/paper/status` expose `run.pending_order`.
- A limit fills when the touched side crosses its price, a stop when it
  trades through, from the tick feed the API already receives; expiry and
  invalidation write `{"kind": "cancelled_unfilled", "reason": "expired" |
  "invalidated" | "replaced" | "cancelled:<text>"}` to `fills.jsonl`. **A
  tick gap over the window is unfilled**, never an assumed fill.

One thing the contract does not carry and the executor needs: `lots` on
`pending_order`. Without it `--mirror-pending` refuses (written down as
`refused-pending`) and mirrors the position at market on the fill, as the
flag-off path does. Stage 1 is paper only, so this blocks nothing yet.

## What will be measured, in this order

Every comparison is **per shared bar** between `ai-xau-ds-plan` and
`ai-xau-ds-ctx`: bars on which both books produced a real answer (rows logged
`unreachable` excluded on both sides). N is the number of shared bars, not
rows.

**Stage 1 - the plan rate. The cheap kill.** Of the plan book's real answers,
how many name a limit or a stop rather than market or NONE. **Pre-committed:
if fewer than 5 of the first 100 shared bars carry a limit or stop, the model
is not using the shape, the claim cannot be tested by this book, and it is
stopped and recorded as such.** A model that answers "market" on every bar is
the market variant with a longer prompt.

**Stage 2 - fill rate and what the misses were.** Of the limit/stop plans
posted, the fraction that filled; and for every `cancelled_unfilled` row, the
**missed-winner R**: what the same side, entered at the OPEN of the bar after
the decision bar with the plan's own stop and target and the desk's maximum
hold, would have returned - computable from the bars and computed the same way
for every miss. Reported with its count. A miss that would have lost is a
save; a miss that would have won is the cost the claim has to carry.

**Stage 3 - entry price saved.** For every filled limit/stop: fill price
against the open of the bar after the decision bar, signed in the trade's
favour, in points and in R (the plan's own stop distance). Mean and
distribution, with N.

**Stage 4 - the one that decides: net R.** Net R of the plan book against
the market book over the shared bars, each beside its trade count, each
beside its own coin.

> **THE RULE, pre-committed.** Read at **30 closed trades** on the plan book
> (window 1) and again at **60** (window 2 = trades 31 to 60). The claim
> survives only if the plan book's net R over the shared bars is **above the
> market book's in BOTH windows.** If it is not above in either window at
> the moment that window closes, **limit entries are dropped**: the plan
> book and its coin are stopped, the result recorded, and only the cadence
> funnel (stages 2 to 4 of the plan) survives - the funnel does not depend on
> limit entries paying. No extension. "Above" means above; a tie is not
> above.

Both windows exist because the 2026-09-14 synthesis found trend signs at the
95th percentile on a first window and gone on a second. One window is a
story; two agreeing is the least that is not.

## The cold-window confound, and the exact rule for it

**A new book starts with an EMPTY window.** `start_ai_runs.py` creates it,
the poller feeds it one bar, and the trader decides on that one bar while the
control - running for days - is deciding on forty. At 15m it takes about ten
hours to reach forty.

**THE RULE, pre-committed:** a decision row counts toward the plan rate, the
fill rate, the missed-winner R, the price saved and the net R **only when the
book saw a full window - forty bars.** Rows below that are excluded and
reported separately as the warm-up, with their count.

**It is measured, not counted.** The criterion is read out of the row's own
stored prompt, which carries `LAST <n> BARS of <market>:<tf>`, and the row is
excluded when `n < 40`. Position in the file is not a proxy: a restart, a
poller outage or a missed bar makes the fortieth row and the fortieth bar
different things. A `pending_skip` or `trigger_check` row belongs to the
decision row it followed and inherits its `n`.

**Both arms are filtered by the same rule**, the control's rows on those bars
included.

## Multiple comparisons, stated now

One headline number decides: net R against the market book, by the rule
above. The plan rate, fill rate, missed-winner R and price saved are
**descriptive** - reported with their N, never promoted to a result on their
own, however striking. No subgroup - limit-only, stop-only, by session, by
side, by `valid_bars` - may be read as a finding; a subgroup that looks
better is the reason the next registration gets written, not a result of
this one.

Two registrations share this book: this one (plan against market) and
`2026-09-18-plan-trigger` (plan-trigger against plan). They are two tests on
overlapping bars. Any significance threshold applied to either is halved
(0.025 for a nominal 0.05), and a number that survives one comparison and not
the other is reported as exactly that.

## What would make this wrong, stated now

- **Pseudo-replication.** Two books deciding on the same bars are not two
  samples; N is shared bars.
- **The coin.** If the plan book beats the market book and its own coin does
  too, the difference is the fill mechanics or the bars, not the model.
  The coin is read first.
- **Regime.** A limit entry in a trend that keeps going misses everything;
  in a range it fills everything. Thirty trades will sit mostly in one. The
  two windows are the only defence here and they are not much of one.
- **The tick feed.** A paper limit fills from the API's tick feed; a gap is
  an unfilled order. The fill rate is therefore partly the feed's uptime,
  and the `cancelled_unfilled` reason field is how a feed gap is separated
  from a pullback that never came.
- **Prompt length.** The plan block is ~1,400 characters more prompt. The
  entry rate could move for that reason alone; the plan rate in stage 1 is
  read against the market book's entry rate over the same bars for that
  reason.

## What this does not say

It does not say whether a model at the trigger helps; that is the next
registration, and it is only worth running if this one passes stage 1.

It does not put anything on the funded account. `mt5_executor.py
--mirror-pending` exists, is tested, and is OFF; the launcher does not pass
it. It stays off until this registration's rule has been read at 30 trades,
and turning it on is a separate decision with its own line in the record.

It does not test `thinking`. The plan book runs with the same decision
thinking as the market book (the provider's default).

## What the record carries

- every decision row: `prompt_variant: "plan"`, `prompt_layout`, `entry`
  (hoisted from the decision), the whole prompt and reply, `refused_locally`
  naming which `sane()` rule refused;
- `pending_skip` rows for bars the book was not asked on, carrying the
  waiting order;
- `fills.jsonl`: fills as today, plus `cancelled_unfilled` with its reason;
- the coin's book with the mirrored plan and the same `valid_bars`.

## Signed

Written by the TRADER session (Claude Fable 5.1, `backcom-vantage`,
2026-09-18) on branch `agent/plan-prompt`, against the API contract as given
by the lead, before the engine's routes were read - the contract is quoted
above so the two can be compared when they land.
