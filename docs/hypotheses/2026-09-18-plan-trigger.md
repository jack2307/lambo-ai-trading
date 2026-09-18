# 2026-09-18-plan-trigger: a model at the trigger, once a minute, beats a limit that fills itself

**Registered:** 2026-09-18, **before either book has seen a single bar, and
before the books exist.** Everything below is a pre-commitment; nothing in it
may be changed by what the books turn out to say.
**Status:** registered, not started. Runs only after
`2026-09-18-plan-entry` has passed its stage 1 (the model uses the plan
shape at all); nothing here is worth measuring on a book that answers
"market" every bar. Needs `GET /api/paper/m1` and `POST /api/paper/pending/act`
on `main` and deployed.
**Books:** `ai-xau-ds-plan` (variant `plan`, registered above) as control,
against `ai-xau-ds-plan-trigger` (variant `plan-trigger`) with its own coin
`ai-xau-ds-plan-trigger-coin`, seed 47.
**Code:** `py/live/ai_trader.py` (`VARIANTS["plan-trigger"]`, `TRIGGER_BLOCK`,
`fast_prompt`, `parse_trigger`, `--fast-poll`), `py/live/plan_entry_selftest.py`
(the fast-loop transitions), `py/live/prompt_variant_selftest.py` (the two
prompts byte-identical).
**Plan:** stage 2 of `docs/plans/2026-09-18-staged-ai-entry.md`.

## Claim

While a limit or stop waits, a model shown the one-minute bars since its
decision and asked a three-way question - **TRIGGER** (fill now at the
quote), **WAIT**, **CANCEL** - does better than the order left to its own
rules: it fills the plan when the level is rejected just short of the touch,
and withdraws it when the setup has died before the price gets there. "Does
better" is **net R over the same bars, or fill quality** (fill rate, missed-
winner R, entry price against the plan's price), by the rule below.

The mechanism, so it can be wrong: a limit is a price, and price is not the
only thing a trader watches at the level. Whether the M1 bars are rejecting
it or slicing through it is information the resting order cannot use.

## The prior

None on this desk: no book has ever had a model at the trigger. Two facts
bound the expectation.

- **The bare-bars result** (`mt5_executor.py`, above `--max-join-r`): entry
  drift carries no information about what follows, over 81,789 observations.
  A TRIGGER that fills a few points short of the limit costs those points and
  earns nothing for them on average. So the claim rests on CANCEL and on the
  fills TRIGGER *adds* (orders that would have expired untouched), not on
  better prices.
- **`thinking: disabled` is a quality change.** The fast question goes with
  reasoning off, because reasoning is 87% of the DeepSeek bill (830k of 1.3M
  tokens on 2026-09-18) and a question asked ten times per window cannot
  carry it. A model without reasoning may be worse at exactly this. That is
  why this is an arm and not a default, and why a `reasoning_effort: low`
  arm is named below as the next registration, not folded into this one.

## What the variant changes, in full

**The decision prompt is the same.** `plan-trigger` renders byte-identical to
`plan` - `prompt_variant_selftest.py` asserts it - so the two books' decisions
differ by nothing but the model's own sampling. The whole of the difference
is the fast loop:

- while `run.pending_order` is non-null and the book holds no position, every
  `--fast-poll` seconds (default 60, one M1 bar) the trader sends **the exact
  decision prompt string that produced the order, byte for byte** - kept in
  memory, never rebuilt - followed by `TRIGGER_BLOCK`: the plan restated (the
  model cannot otherwise know what it decided; the prompt carries the
  question, not the reply), the closed M1 bars after the decision bar from
  `GET /api/paper/m1` in the 15m rows' own number format, the latest quote
  and the forming minute, and the question. Sent with `thinking="off"`.
- **TRIGGER** posts `POST /api/paper/pending/act {"action": "trigger"}` on the
  book; **CANCEL** posts `cancel` with the model's reason; **WAIT** posts
  nothing. An unreadable reply is WAIT.
- **the coin never gets the fast question.** A coin that could be triggered
  would need an opinion, and then it would not be a coin. Its order fills by
  rule, as the `plan` book's does.
- **the `plan` book never runs the loop.** That is the comparison.

Why byte-identical matters: DeepSeek serves an exact prefix of a previous
request from cache at one fiftieth of the fresh price (read 2026-09-18 from
api-docs.deepseek.com; cache life "hours to days", no SLA). The decision
prompt is ~6.4 KB; the tail is a few hundred bytes. `plan_entry_selftest.py`
pins that every fast prompt `startswith` the decision prompt. The hit rate is
not assumed: every `trigger_check` row carries `usage.cached_input` and the
first read of this book reports the measured share.

**A restart forgets.** The prompt lives in the process; an order from before
the process started gets no fast question (the desk state in a rebuilt prompt
would differ, so the bytes would) and fills by rule. Said once on the console
and visible in the record as an order with no `trigger_check` rows. Those
orders are counted and reported, and excluded from the fill-quality numbers.

## What will be measured, in this order

Every comparison is **per shared bar** between `ai-xau-ds-plan-trigger` and
`ai-xau-ds-plan` on bars where both produced a real answer; N is shared bars.

**Stage 1 - does the loop run, and does it ever act.** Fast calls per
waiting order; the share of TRIGGER, WAIT, CANCEL; the measured cached share
and cost per call. **Pre-committed: if over the first 30 waiting orders on
the trigger book the model never once answers TRIGGER or CANCEL, the loop is
a cost with no behaviour, and the book is stopped and recorded as such.**
There is nothing to compare.

**Stage 2 - fill quality, three numbers, each with its N.**
- fill rate: filled / (limit+stop plans posted), both books;
- missed-winner R: for every `cancelled_unfilled` row, the same-side entry at
  the open after the decision bar with the plan's stop/target and the desk's
  max hold, from the bars - the same arithmetic as the plan-entry
  registration, on both books;
- entry against the plan's price: for every fill, fill price minus the plan's
  entry price, signed in the trade's favour, in R. A rule-only fill is 0 by
  construction (it fills AT the price); a TRIGGER fill is whatever the quote
  was. This number is what TRIGGER costs.

**Stage 3 - the one that decides: net R** against the `plan` book over the
shared bars, each beside its trade count and its coin.

> **THE RULE, pre-committed.** Read at **30 closed trades** on the trigger
> book (window 1) and again at **60** (window 2). The claim survives only if,
> in BOTH windows, EITHER the trigger book's net R is above the plan book's
> over the shared bars, OR - with net R not below - its fill quality is
> better on **both** fill rate and missed-winner R. Otherwise **the model at
> the trigger is dropped and the rule-only fill stays.** No extension.
>
> **The yardstick for "above" and "better" is the two coins.** `plan-coin`
> and `plan-trigger-coin` run identical mechanics on different seeds; the
> difference between THEM over the same bars is what seed noise looks like
> at this N. A difference between the model books smaller than the
> difference between their coins is not a difference.

## The cold-window confound, and the exact rule for it

**A new book starts with an EMPTY window**, and this one starts later than
its control, so it is colder for longer.

**THE RULE, pre-committed:** a decision row - and every `pending_skip` and
`trigger_check` row that followed it - counts **only when the book saw a full
window, forty bars**, read from the row's own stored prompt (`LAST <n> BARS
of <market>:<tf>`, excluded when `n < 40`). A `trigger_check` row's prompt
begins with the decision prompt, so it carries the same `n` and is read the
same way. Rows below forty are the warm-up, reported with their count. **Both
arms are filtered by the same rule.**

## Multiple comparisons, stated now

Net R decides; fill quality is the one pre-declared rescue and it needs both
of its numbers to agree. The TRIGGER/WAIT/CANCEL shares, the cost, the cached
share and the entry-against-plan number are descriptive and never a result.
No subgroup - TRIGGER fills only, CANCEL saves only, by session, by
`valid_bars` - may be read as a finding.

Two registrations share the `plan` book (this one and `2026-09-18-plan-entry`);
any threshold applied to either is halved (0.025 for a nominal 0.05), and a
result that survives one and not the other is reported as exactly that.

The counterfactual for a single act is imperfect and is said so here: on a
given bar the `plan` book's own order is not the same order as the trigger
book's - same model, separate calls, different plans - so "what the rule
would have done with THIS order" is not observable trade by trade. The
comparison is book against book over shared bars, not act against act.

## What would make this wrong, stated now

- **The cache.** No SLA. If the cached share comes back low the loop costs
  what a decision costs, and the cost line is read beside any benefit. The
  prompt order is worth having at zero hits; the loop may not be.
- **The M1 feed.** `/api/paper/m1` aggregates the API's own ticks; a gap
  shows as missing minutes and the block says `unavailable`. The share of
  fast calls made with the feed down is reported; a TRIGGER on an empty
  since-block is a guess and is counted as one.
- **Thinking off.** Named above. A null result here does not close the
  question for a model that can reason at the trigger; it closes it for one
  that cannot, at this cost.
- **Pseudo-replication and regime**, as in the plan-entry registration.

## What comes after this one

**Queued, not started: a `reasoning_effort: low` arm** on the same fast
question, its own book and coin, if and only if this registration's stage 1
shows the loop acting. It asks whether the quality lost to `thinking: off` is
the thing that mattered. Not folded in here because a book with two knobs
turned cannot say which one did the work.

## What the record carries

- `trigger_check` rows: the whole fast prompt (the decision prompt again plus
  the tail), the reply, `action`, `verdict.reason`, `acted`, the API's reply
  to the act, `usage` with `cached_input` hoisted, `latency_ms`, `cost_usd`,
  `m1_state` (`ok, n bars` / `unavailable`), and `bar_time` = the decision
  bar's, so every fast row joins to its decision row;
- `pending_skip` rows on the bars the book was not asked, carrying the order;
- `fills.jsonl`: `cancelled_unfilled` with `reason: "cancelled:<the model's
  reason>"` for a CANCEL, and the fill row for a TRIGGER;
- the console line, once, for an order from before the process started.

## Signed

Written by the TRADER session (Claude Fable 5.1, `backcom-vantage`,
2026-09-18) on branch `agent/plan-prompt`, against the API contract as given
by the lead. The `/api/paper/m1` shape assumed - `{bars: [{time, open, high,
low, close, ticks, spread_mean}], forming, unavailable}`, complete minutes
oldest first - is the contract's; the trader renders whatever subset of those
fields arrives and says `unavailable` when none do.


## Amendment 2026-09-18 16:55Z - the four books' FIRST trade is excluded, and why

Written before any result of these campaigns was read.

**What happened.** The books were created at 16:38:36Z and their traders were
started in the same minute, against an fd-api that was still the OLD binary
(32b98ff) - the pending-order contract had not been deployed yet. An unknown
field on a JSON body is ignored, not refused, so a plan would have been
accepted as a plain market intent. The traders were killed at 16:39:48Z, but
not before each of the four books had taken exactly one trade:

| | |
|---|---|
| decided at | 16:39:13Z (trigger arm) / 16:39:16Z (plan arm) |
| on bar | 08:45:00Z - the warm-up window's last bar, 7h45m stale |
| answer | `side LONG`, `entry {"type":"market","price":null}` |
| then | a `gap` row, 30 missing bars |
| filled | 16:30:00Z bar open, LONG 4377.79 -> 4377.51, STOP |
| result | pnl -0.02, **R -0.0513**, 0.06 lots, on all four books alike |

**Why the cold-window rule above does not catch it.** That rule reads the
row's own `LAST <n> BARS` and excludes `n < 40`. This row says 40: a book
created mid-session is seeded with the poller's stored window, so the prompt
was full while the last bar in it was eight hours old. The confound here is
not a short window, it is a STALE one.

**THE RULE, pre-committed, in addition to the cold-window rule:** a row counts
only when its decision bar is the CURRENT bar - `at - bar_time` less than two
bar lengths (30 minutes at 15m) - and only when its book's server carried the
pending-order contract, which fd-api first reported at **16:50:21Z** (version
f2e9038). Rows before that stamp, and rows decided on a bar more than two bar
lengths old, are excluded from the plan rate, the fill rate, the
missed-winner R, the price saved and the net R, and are reported separately
with their count. Both arms and both coins are filtered by the same rule; on
these four books it excludes exactly one trade each, the one tabled above.

**It is not deleted.** The rows stay in `fills.jsonl` and `decisions.jsonl`
where they happened, and every figure quoted from these books says how many
rows the rule removed. A campaign whose first trade is quietly missing is
worse than one that says which trade it is not counting.

**The general trap, worth carrying to the next book.** Any book created
mid-session decides immediately on the stale last bar of its seeded window,
and the intent then fills at the next bar the poller feeds it - seconds later
on the wall clock, hours later on the bar clock, with a `gap` row between.
The fix is to create books in the quiet window and start their traders only
after the poller has fed the run a current bar; until that is enforced in
`start_ai_runs.py`, this exclusion is how the record stays honest.
