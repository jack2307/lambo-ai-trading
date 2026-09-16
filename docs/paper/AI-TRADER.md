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

Books run on the same stream in **matched pairs**, one pair per model:

| model | reached through | its book | its coin |
|---|---|---|---|
| `gpt-5.6-sol` | the **ChatGPT plan**, via the Codex CLI | `ai-xau-sol-ctx` | `ai-xau-sol-ctx-coin` |
| `claude-opus-5` | the **Claude plan**, via the Claude Code CLI | `ai-xau-opus-ctx` | `ai-xau-opus-ctx-coin` |

No metered API key is involved in either. Owner's instruction, 2026-09-15:
*"cho dùng gói đừng dùng API nữa"*.

**The OpenAI model is `gpt-5.6-sol`, and that is not a renaming of `gpt-5`.**
A ChatGPT account refuses `gpt-5`, `gpt-5-codex` and `codex-mini-latest`
outright — *"not supported when using Codex with a ChatGPT account"* — and
`gpt-5.6-sol` is what Codex itself defaults to on this plan, read from its own
banner rather than guessed. It is therefore a **different model** from the one
the first campaign ran for half an hour on the API, which is why that
campaign's books (`ai-xau`, `ai-xau-coin`) were **closed rather than
repointed**: one book holding two models is a record nobody can read, and the
badge would have flagged it as mixed, correctly.

Those books closed at **+$25.62 each** — the model and its coin both went
long, both made the same money. One trade proves nothing, but it is a fair
picture of what the control is for: an outcome identical to a coin's carries
no information about direction, however well the model explained itself.

- the **model book** takes the side the model asked for.
- the **coin book** takes a trade at the **same bar**, with the **same stop
  distance**, on a **coin-flip side**.

Each model gets its **own** coin, and this is not duplication for its own
sake. The two models decide on different bars and set different stops, so one
shared coin book could be matched to at most one of them; against the other it
would be a different experiment wearing the same name.

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

## Amendment, 2026-09-16: the decider is told more

The first prompt carried forty bars of OHLC and nothing else. Two things
followed from that, and the owner found the second one:

* The model was **proposing trades into rules it could not see**. On
  2026-09-16 the desk refused four of `ai-xau-sol`'s proposals for hitting a
  daily cap the model had never been told about, and sized four more down by a
  notional cap it had never been told about either. It was also setting stops
  without being told the ATR it is sized against.
* It knew nothing about the desk's own calendar, so it could propose an entry
  into a news blackout the desk would refuse.

The prompt now carries two more blocks.

**The desk's state.** Equity and net, ATR and what 1R is in price, the live
spread, the daily trade cap and how many proposals it has already cost, the
notional cap, the daily loss limit, the cooldown, the maximum hold, and the
next scheduled release with the blackout window around it. The guard numbers
are **parsed from `config/default.toml`**, not typed here: a limit quoted to
the model that did not match the one enforced would be worse than silence,
because the model would plan around a rule that is not the rule.

**Market context.** EMA(21) and EMA(55), RSI(14), the session, today's and the
previous day's high and low, the range of the window, and the last eight hours
aggregated to 1h from the same bars so it cannot disagree with them.

### Why the second block is defensible here and would not be in a sweep

I argued against it first, and I was importing the wrong discipline. In a
parameter sweep every added input is another cell and another chance for
selection noise, which is why `docs/decisions/` is full of registrations that
died. **This is a forward test with a falsifier fixed in advance and a coin
control.** There is no selection: the model cannot overfit data it has not
seen, and the coin measures whatever it does. Extra inputs can genuinely help
or genuinely hurt, and the campaign reports which.

The cost that IS real: if the campaign passes, we will not know which input did
it. That is a second-order question and it stays open. The first-order question
is whether any of this beats a coin, and nothing here makes that easier to fake.

### What this amendment costs

The four books opened on the sparse prompt are **closed**, because a prompt
change is a decider change and one book holding two deciders is a record nobody
can read. They closed at:

| book | trades | net |
|---|---|---|
| `ai-xau-sol` | 4 | +$151.36 |
| `ai-xau-sol-coin` | 4 | +$13.89 |
| `ai-xau-opus` | 1 | −$27.01 |
| `ai-xau-opus-coin` | 1 | −$27.01 |

Five trades between them. Nothing there is a result, and the +$137 gap on the
Codex pair is four trades of noise — it is recorded so that nobody later
remembers it as a promising start.

Closing them also means this campaign **cannot say whether the added context
helped**, because the sparse arm is too short to compare against. Running both
prompts side by side would answer that, at twice the token cost and while the
first-order question is still unanswered. Not done; available on request.

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

### Two models means two chances, and the threshold moves to pay for it

Added 2026-09-15, before the first live trade of either campaign, because it
is worthless declared afterwards.

Running two models against two coins is **two chances to clear p ≤ 0.05**. At
that threshold a pair of independent campaigns produces at least one false
winner about 9.75% of the time — nearly one run in ten would hand back a
"result" that is the second ticket in a raffle. The loop has closed thirty-two
registrations without this mistake and will not start now.

So each campaign is judged at **p ≤ 0.025** (Bonferroni over the two), and the
other two conditions are unchanged and apply per campaign. A model that clears
0.04 has **not** passed; it has produced the most interesting failure so far,
which is a different sentence and must be written as one.

Adding a third model later moves the threshold again, and a model may not be
added after seeing another's results and then judged at the old number.
Whichever model looks better at the end was not selected for on this basis:
both were registered here, before either traded.

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
powershell -NoProfile -File py\live\start_ai_traders.ps1
```

which is one process per model, guarded by a system-wide mutex so it cannot be
run twice. Individually:

```
# OpenAI's model on the ChatGPT plan. The `codex/` prefix is what routes it
# there; a bare `gpt-*` still means the metered API, deliberately, so nothing
# falls back to a paid key by accident.
python py/live/ai_trader.py --model=codex/gpt-5.6-sol --market=xauusd --tf=15m \
    --run=ai-xau-sol --control=ai-xau-sol-coin --seed=7

# Anthropic's model on the Claude plan. `provider_of` sends every Claude model
# to the CLI by default; `--provider anthropic` forces the metered API.
python py/live/ai_trader.py --model=claude-opus-5 --market=xauusd --tf=15m \
    --run=ai-xau-opus --control=ai-xau-opus-coin --seed=11
```

Codex needs a one-time `codex login` (a browser flow). The binary is **not**
the npm package: `npm i -g @openai/codex` leaves a shim that throws on Windows
and shadows the working `codex.exe` the Codex desktop app ships, so
`codex_bin()` probes candidates with `--version` and takes the first that
actually runs.

The two coins take different seeds so their flip sequences are independent.
With one seed both controls would flip identically, and on any bar where both
models happened to trade, the two campaigns would share a control's luck —
which is the one thing a control may not do.

### What the plan's model costs, measured

A decision through the Claude CLI, measured on this machine 2026-09-15:
**~$0.036 and ~1.7-4.7s**, steady state, because the prompt cache is reused
across processes. At one decision per 15-minute bar that is roughly $3.50 a
day. Through the Codex CLI the same decision takes **~16-19s** — an order of
magnitude slower, comfortably inside the 90s timeout and the 15-minute bar,
but worth knowing before anyone points this at a one-minute chart.

Two details are worth keeping in view. The first call after a cold cache cost
$0.22, six times the steady-state figure, so a restart loop that never warms
would be expensive in a way the average hides. The second is that this spends
**plan quota, not cash** — the same quota the owner's interactive sessions
draw on, so a campaign left running competes with the desk's own work.

One decision per closed bar, at most one open position, both books driven from
the same call. The full prompt and reply of every decision go to
`data/paper/ai-xau/decisions.jsonl` beside the book, in the same shape and for
the same reason as the advisor's log: a summary cannot be replayed against a
changed prompt, and replay is the only way a mistake gets fixed rather than
counted.
