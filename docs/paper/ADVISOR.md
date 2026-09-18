# The advisor: an AI that may say no, and a way to find out whether it was right

A model now sits between a strategy's decision and the fill. This is what it
may do, what it may never do, and — the part that took the most thought — how
anyone will ever know whether it helped.

## The one power it has

An advisor returns a single number in `[0, 1]`.

| it wants to | can it |
|---|---|
| refuse a trade the strategy decided to take | **yes**, at 0 |
| make that trade smaller | **yes**, between 0 and 1 |
| choose long or short | no |
| move a stop or a target | no |
| set a price | no |
| make a trade larger | no |
| take a trade the strategy did not ask for | no |

Not one of those "no"s is a prompt instruction. **There is no field in the
request in which any of them could be expressed.** A model that argues at
length for a long it invented has nowhere to put the argument, and
`crates/fd-backtest/src/paper.rs` has a test for each refusal, including
`an_advisor_can_never_make_a_position_larger` (which feeds it 1.5, 10,
infinity and NaN) and `an_advisor_cannot_open_a_trade_the_strategy_did_not_ask_for`.

That asymmetry is the whole reason this is safe to run. The worst a broken,
confused or hostile advisor can do is stop the desk trading, which is the same
outcome as not running the desk.

The cut applies to the **lots and to nothing else**. Risk is a price distance,
so R, the stop and the target stay the strategy's own numbers and a half-size
trade is the same trade at half the money. A cut that lands below the venue's
minimum lot becomes a veto rather than an order a broker would bounce.

## Where it runs, and why it is a poll

A paper book decides on the close of bar N and fills at the open of bar N+1.
That gap is five or fifteen minutes. **The advisor lives entirely inside it**,
in a separate process, and speaks through two routes:

```
GET  /api/paper/pending     runs whose next bar would open a trade
POST /api/paper/advice      one verdict, plus the whole conversation
```

`fd-api` has no HTTP client, no model key and no notion of a prompt. It cannot
call out and does not wait for anybody. If the advisor is down, slow, or has
never been started, the intent fills exactly as the strategy decided it — the
default is *no intervention*, and it is the default by construction rather than
by a timeout someone has to remember to set.

A verdict names the intent it is about (`<run>:<bar time>`). One that arrives
after the intent has filled names a bar that is no longer last, and is logged
and dropped rather than applied to whatever is pending now. That is the entire
staleness protection and it needs no clock.

## Which model, and why it is a question the log answers

Either provider, and a mixed panel on purpose:

```
python py/live/advisor.py --model=gpt-4o                       # all three on OpenAI
python py/live/advisor.py --agent-model risk=gpt-4o --agent-model news=claude-opus-5
```

The API is chosen from the model's name (`claude*` to Anthropic, `gpt*`, `o1`,
`o3`, `o4` to OpenAI) and the key comes from `ANTHROPIC_API_KEY` or
`OPENAI_API_KEY`. An agent whose key is missing is **named at startup and does
not run** — a panel silently one advisor short is a panel whose verdicts mean
something different from what the log will say they mean. A model name neither
rule recognises is refused rather than guessed at; `--provider` settles it.

Neither vendor SDK is imported. The whole of what this process needs from
either API is "send one prompt, read one string", and two hundred lines of
`urllib` beat two dependencies with versions to track and a release cadence
that is not ours.

**The mixed panel is not a compromise, it is the experiment.** Every turn
already records the model that answered it, so after a few hundred
consultations `scripts/advisor_review.py --agents` says which provider's
objections were worth listening to — on this desk's own trades, at this desk's
own costs, rather than on somebody's benchmark. Running risk on one vendor and
news on another for a month is a cheaper and more relevant comparison than any
published evaluation, because the thing being measured is money on these ten
books.

## The shadow book, which is the actual point

Here is the problem with every "AI risk layer" ever shipped: **a veto has no
outcome.** The trade did not happen, so nobody can say whether refusing it saved
money or cost it. A year of such logs teaches nothing, because every entry says
"the model objected" and none says whether it should have.

So every run keeps **two books on the same bars**:

- `book` — hears the advisor.
- `shadow` — same strategy, same parameters, same guards, same bars, **never
  advised**.

The difference between them *is* the advisor's effect, in dollars, over exactly
the same market. Not an estimate of it.

The two diverge: once one takes a trade the other refused, the strategy is
asked from two different states and stays that way. **That divergence is the
answer, not a bug in it.** A veto that saved $80 on Monday may cost a position
that would have been open on Tuesday, and only running both books to the end
prices that honestly.

Every advice event records both running totals, so the bill is legible at any
moment without replaying anything:

```json
{"kind":"advice","book_net_usd":-228.65,"shadow_net_usd":-191.02,"shadow_trades":17}
```

Read that pair as: *the advisor has cost $37.63 so far.*

## The conversation log

`data/paper/<run>/advice.jsonl`, one JSON object a line, append-only. Its own
file and never `fills.jsonl`: the book's log is replayed on restart and this
one never is, and once whole prompts are in it, it will be orders of magnitude
larger. Mixing them would make a book's reload scan megabytes of transcript to
find its trades.

Two record kinds.

**`consultation`** — written when a verdict is posted, whether it is applied or
not:

```json
{
  "kind": "consultation",
  "at": 1789412345678,
  "intent_id": "xau-macd-asia:1789411500000",
  "pending_now": "xau-macd-asia:1789411500000",
  "applied": true,
  "size_factor": 0.5,
  "raw_size_factor": 0.5,
  "reason": "CPI in 40 minutes and the book is already short gold",
  "transcript": [
    {"agent":"risk","model":"claude-opus-5","prompt":"...","response":"...","latency_ms":2210,"size_factor":0.5,"reason":"..."},
    {"agent":"news","model":"claude-opus-5","prompt":"...","response":"...","latency_ms":1840,"size_factor":1.0,"reason":"..."},
    {"agent":"arbiter","model":"claude-opus-5","prompt":"...","response":"...","latency_ms":1960,"size_factor":0.5,"reason":"..."}
  ]
}
```

**`advice`** (in `fills.jsonl`, beside the fill it changed) — what the book did
about it, with both books' totals at that instant.

### Three things the schema does on purpose

**Prompts and responses are stored whole.** A summary cannot be replayed
against a changed prompt, and replay is the only way a past mistake ever gets
*fixed* rather than merely counted. The cost is disk, which is the cheapest
thing in this system.

**Every turn is kept, not just the verdict.** `agent`, `model`, its own
`size_factor` and its own `reason`. So the question "which advisor's objections
were worth listening to" is answerable per agent, and an agent that is always
wrong can be removed on evidence rather than on taste.

**An allow is logged as loudly as a veto.** A log of only the interventions
cannot be scored: without the allows there is no denominator, and "the panel
looked at this and waved it through" is exactly the record needed when a waved-
through trade loses $300.

## How a past mistake actually gets fixed

Counting is not fixing. The log is shaped for three operations, in order of
how much they teach:

1. **Score.** `scripts/advisor_review.py` joins the consultations to the trades
   and to the shadow book, and answers per agent: how many vetoes, what they
   saved, what they cost, net; the same for cuts; and whether stated confidence
   tracks being right. A veto that prevented a loser is worth `|pnl|`; one that
   prevented a winner cost `pnl`. Both are in the shadow book.

2. **Replay.** Take the stored prompts, run them through a *changed* prompt or
   a different model, and compare the new verdicts against the outcomes already
   known. This is the only cheap experiment in the whole system: it costs model
   calls and no market risk, and it is why the prompts are stored whole.

3. **Register.** A prompt change that looks better on replay is a hypothesis,
   and it goes through `docs/hypotheses/` like everything else — declared,
   then tested on consultations the replay never touched. The loop's own
   history is thirty registrations of ideas that looked good on the data that
   produced them.

## What this is not

It is **not a way to make the desk trade more**, and the code cannot be
persuaded otherwise.

It is **not a strategy**. Nothing here decides a direction. Thirty closed
registrations say this repository has found no mechanism that carries
tradeable direction at Vantage cost, and bolting a model onto the front of one
does not change that. An advisor can only make a losing book lose more slowly,
or a winning one win less.

It is **not on by default**, and as of 2026-09-18 no verdict has ever been
applied to any book. The shadow book runs regardless, which costs one extra
`PaperBook` per run and means that on the day an advisor is first switched on,
the comparison starts from a book that has been tracking the real one all
along.

Two sentences that used to be here were true when written and are not now. They
are corrected rather than deleted, because the second of them is the one a
person would read to decide whether applying verdicts is safe.

**`advice.jsonl` exists.** It said it did not. Counted 2026-09-18: 21 files,
86 consultations. Eighty-five carry `dry_run: true` — written by `advisor.py`
itself, since a dry run posts nothing for the server to record — and one is a
deliberate probe from 2026-09-15 (`"probe: can it enlarge a trade?"`, intent
`xau-stoch:1`) that named an intent which was not pending and was therefore
dropped by the staleness rule it was testing. **`applied` is `false` on all 86.**
So "no run has ever been advised" is still true in the sense that matters — no
book has ever been changed — and the way to check that claim is the `applied`
field, not the absence of the file.

**It is no longer paper only.** `py/live/mt5_executor.py` is still the only
code that can send an order, but since 2026-09-17 it can send one to a REAL
funded cent account: the owner funded it and asked for it, and the demo-only
wall became a permission with three independent keys — `--allow-real` on the
command line, `--account`, and `real_money` for that account in
`config/accounts.toml`, with the terminal's own login checked against the
registry's. A paper book that is mirrored is therefore a real position, so a
verdict that shrinks that book shrinks a real order and a veto refuses one.

Nothing in the advisor's own powers changed, and the asymmetry at the top of
this document still holds everywhere: it can refuse or shrink, never enlarge,
never open, never choose a direction. The worst case is still a desk that does
not trade. But "the worst case is harmless" is a different claim from "this is
paper", and only the first one is still true.

Neither correction was found by reading this file. They were found by counting
the records and reading the executor while writing `py/live/start_advisor.ps1`,
which is the launcher that was also missing — the panel had never run on the
VPS at all, because nothing started it.
