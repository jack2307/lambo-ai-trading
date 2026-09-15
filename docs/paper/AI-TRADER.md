# The AI trader campaign: a model in the lineup, on the same terms as everybody else

The owner asked for a campaign in which the AI trades. This is it, and the
design is one sentence: **the model gets a paper book and a coin-flip control
on the same bars, and it has to beat the coin.**

Everything else here is detail about making that a fair fight.

## Why it is not the advisor

`docs/paper/ADVISOR.md` describes a model that may only say no — it can refuse
a trade a tested strategy decided, or make it smaller, and there is no field in
the request in which a direction could be expressed. That limit exists because
an advisor sits in front of *other people's* strategies and a model that could
overrule them would make their results unreadable.

This is a different question. Here the model is not gating anybody: it is a
candidate, standing in its own book, next to nine rule-based ones. So it is
allowed to choose a side — and it is allowed *because* it is measured, not
instead of.

The advisor's rule is unchanged and still enforced in code. These are two
different routes with two different powers:

| route | who uses it | what it may do |
|---|---|---|
| `POST /api/paper/advice` | the advisor panel | refuse or shrink somebody else's trade |
| `POST /api/paper/intent` | the AI trader | propose its own trade, on its own book |

## The control, which is the whole experiment

Two books run on the same stream:

- **`ai-xau`** — takes the side the model asked for.
- **`ai-xau-coin`** — takes a trade at the **same bar**, with the **same stop
  distance**, on a **coin-flip side**.

Same entry times, same hours, same sizing, same guards, same news blackout. The
only difference between the two books is whether the direction came from a
model or from a random number.

That is the loop's permuted-sides null, run forward in real time instead of
resampled afterwards, and it is the reason this campaign can fail. After a few
hundred trades, if `ai-xau` is not clearly ahead of `ai-xau-coin`, **the model
has no direction information** and the campaign says so.

Everything a model produces looks like reasoning. A coin produces no reasoning
at all. Running them side by side is the only way to tell the difference from
the outside.

## What the model can and cannot do

It posts `{run, bar_time, side, stop, target, reason}` and nothing else.

- **It cannot act on the bar it was shown.** The intent goes into the same
  `pending` slot every rule uses and fills at the **next** bar's open. There is
  no path by which an outside decider gets the bar it just looked at.
- **It cannot arrive late and still trade.** `bar_time` names the bar it
  decided on; if that bar is no longer the run's last, the intent is refused
  and logged. Slowness costs a trade, never a bad fill.
- **It cannot set an unbounded trade.** The run's strategy is `external`, whose
  `exits()` is `Exits::Engine`, so the stop, the target and the maximum hold
  belong to the desk.
- **It cannot choose a size.** There is no size field. Sizing is the engine's
  one percent of equity against the trailing range, identical to every other
  book, because a comparison between differently-sized books is not one.
- **It cannot steer a rule-based book.** The route refuses any run whose
  strategy is not `external`.
- **It cannot reach a broker.** This is a paper book.
  `py/live/mt5_executor.py` is still the only code that can send an order and
  it still refuses any account that is not a demo.

## The falsifier, declared before the first trade

The campaign is judged on the **difference between the two books**, never on
the AI book alone. A book that made money while the coin made more has found
nothing.

It is called a failure when, at **200 closed trades** on each side:

1. `ai-xau` net is **not above** `ai-xau-coin` net, **or**
2. the AI's win rate is **within 4 points** of the coin's, **or**
3. an exact two-sided sign test on the AI's own up-rate does not reject a fair
   coin at **p ≤ 0.05**.

Two hundred trades is roughly ten days at the desk's current rate. At that
count a 4-point edge over the coin is about the smallest thing the sample can
resolve, which is why the number is 4 and not 1.

**No outcome of this makes anything live.** A pass means the question moves to
a demo account with the executor's locks intact; it does not mean money.

## What is expected

Thirty-two registrations are closed in `docs/decisions/` and none survived out
of sample. Not one of them was beaten by a model, because none was tried — but
none of them failed for want of intelligence either. They failed because
intraday direction at retail cost is close to a coin, and a model reading the
same bars has the same bars.

The honest prior is that this campaign closes with the AI book and the coin
book inside each other's noise. It is worth running anyway, for one reason: it
is cheap, it is falsifiable, and **the transcript of every decision is kept**,
so a failure is readable rather than merely recorded. If the model loses to a
coin, the log says what it was thinking while it did.

## Running it

```
python py/live/ai_trader.py --model=gpt-5 --market=xauusd --tf=15m
```

One decision per closed bar, at most one open position, both books driven from
the same call. The full prompt and reply of every decision go to
`data/paper/ai-xau/decisions.jsonl` beside the book, in the same shape and for
the same reason as the advisor's log: a summary cannot be replayed against a
changed prompt, and replay is the only way a mistake gets fixed rather than
counted.
