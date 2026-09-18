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
| `gpt-5.6-terra` | the **ChatGPT plan**, via the Codex CLI | `ai-xau-terra-ctx` | `ai-xau-terra-ctx-coin` |
| `claude-opus-5` | the **Claude plan**, via the Claude Code CLI | `ai-xau-opus-ctx` | `ai-xau-opus-ctx-coin` |
| `deepseek-flash` | the DeepSeek API, metered and cheap | `ai-xau-ds-ctx` | `ai-xau-ds-ctx-coin` |

No metered API key is involved in either. Owner's instruction, 2026-09-15:
*"cho dùng gói đừng dùng API nữa"*.

**`gpt-5.6-sol` was retired on 2026-09-17 and `ai-xau-sol-ctx` was stopped, not
repointed.** On the VPS every call came back *"The 'gpt-5.6-sol' model is not
supported when using Codex with a ChatGPT account"* — the same account and the
same CLI version that still answers on the desktop, which differs only in its
binary and in an `auth.json` predating that day. The campaign had no model on
the machine it ran on, and 99 decisions is where it stops. Its book keeps
everything: `/api/paper/stop` writes `final.json` and removes only `state.json`.

**Terra is a different model, not a rename, and this was checked.** The
account's own list — `%USERPROFILE%\.codex\models_cache.json`, fetched
2026-09-17T05:00Z by client 0.148.0 — holds six models and carries **both**
`gpt-5.6-sol` (priority 4) and `gpt-5.6-terra` (priority 7) as separate live
entries, beside `gpt-5.6-luna`. A rename cannot put two names in one list at
one moment. **Read the two books accordingly: by the vendor's own ordering
terra ranks BELOW sol, so this is a substitution rather than an upgrade and the
curves are not continuous with each other.** It runs **paper-only** to begin
with — a campaign with no decisions has nothing anyone has read, and mirroring
one into the funded account on day one is a question, not a default.

**The OpenAI model is one of the plan's own, and that is not a renaming of `gpt-5`.**
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

## Amendment, 2026-09-16: a third arm, and the threshold moves again

`deepseek-flash` joins on the owner's request, and the threshold every arm is
judged at moves with it. **This is written before the new books took a trade**,
because a threshold declared afterwards is worth nothing.

| arms | each judged at |
|---|---|
| 2 | p ≤ 0.0250 |
| **3** | **p ≤ 0.0167** |
| 4 | p ≤ 0.0125 |

Three independent campaigns at p ≤ 0.05 would produce at least one false
winner about 14% of the time. Bonferroni over three is 0.05/3 = 0.0167, and
the other two conditions are unchanged and still apply per campaign.

### Why this arm is worth an arm

The other two are frontier models reached through subscriptions. This one is
deliberately the cheap one, and the question it answers is not "can DeepSeek
trade" but **"does model quality matter here at all?"** If a model costing a
dollar a month scores the same as Opus 5, that is a finding about the task and
not about the model — and it is the cheapest finding available.

Measured on the real prompt, 2026-09-16:

| | latency | tokens per decision | cost, one 15m book |
|---|---|---|---|
| `deepseek-flash` | **1.5 s** | 2,658 in / 195 out | **$1.03–2.06 / month** |
| `deepseek-v4-pro` | 2.7 s | 2,711 in / 113 out | $4.01–8.02 / month |
| `claude-opus-5` (plan) | 5.8 s | 26,509 total | ~$80 / month equivalent |
| `gpt-5.6-sol` (plan) | 16.6 s | 17,148 total | plan quota |

`deepseek-flash` was chosen over `v4-pro` for being the sharper contrast as
well as the cheaper one. If it turns out too weak to produce a readable
decision, `v4-pro` is the fallback — and swapping it would be a NEW arm with
new books, not a change to this one.

The cost table is worth reading twice. A direct API call spends 2,853 tokens on
the question; the CLI route spends 26,509 on the same question, because
95% of a CLI call is the harness. The subscriptions are not cheaper, they are
prepaid.

### What this arm does not get

No special treatment. Same prompt, same guards, same sizing, same 10,000 USC,
its own coin on its own seed. If it is better, the comparison says so; if the
API is down, its badge goes quiet exactly like Opus's did for eight hours on
2026-09-15.

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
python py/live/ai_trader.py --model=codex/gpt-5.6-terra --market=xauusd --tf=15m \
    --run=ai-xau-terra-ctx --control=ai-xau-terra-ctx-coin --seed=31

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

## The first three trades on the demo account do not count

2026-09-16. The AI books and their coin controls were mirrored onto the Vantage
demo account 26108386. The first three fills are an artefact and must be
excluded from any comparison between the books and the account:

    ai-xau-ds-ctx        book 4338.31   account 4348.95   +10.64
    ai-xau-sol-ctx       book 4342.92   account 4348.98    +6.06
    ai-xau-sol-ctx-coin  book 4342.92   account 4348.98    +6.06

That is not slippage. MetaTrader starts with its Algo Trading button off, and
every `order_send` from the Python API comes back `10027 AutoTrading disabled
by client` until someone turns it on — the account, the login and the
connection are all fine, and the terminal simply refuses to let a program
trade. Thirty-six orders were refused at that gate. When it was opened, three
books were already long, and the reconciler did what a reconciler does: it made
the account match the book, at the price available *then*.

The three went on to bank +38.28, +37.92 and +37.92. They are excluded anyway.
A profit earned from a worse entry than the book's is no more comparable than a
loss would have been, and the direction of the error is the point: **the longer
a book has been right, the worse the mirror's entry**, so late joins are biased
and the bias flatters nothing consistently.

The executor now refuses to join a position the book opened more than
`--max-adopt-bars` (default 1) ago, and logs `not-adopted` when it does. The
mirror sits that trade out and joins on the next one, where it can enter within
a bar of the book. Age is measured against the book's own `last_bar_time`
rather than the wall clock: over a weekend or a feed outage the wall clock
would call every position stale while the book has not advanced a bar.

Measured on the same account, in a deliberate one-lot round trip: **slippage
0.00 on entry and +0.01 on exit, 236 ms and 240 ms, spread 0.29 (29 points),
and zero commission** — the spread is the whole cost here. That is what the
mirror should be measuring, and it is a different number entirely from the six
to eleven points above.

## What the plan routes would cost if they were billed

2026-09-16. Measured from `decisions.jsonl`, priced from the vendors' own pages
the same day. A 15-minute book closes 96 bars a day and the trader asks once
per close, so a full day is 96 calls per model.

    claude-opus-5      $5.00 in / $0.50 cache read / $25.00 out per MTok
    gpt-5.6-sol        $4.00 in / $0.40 cached     / $20.00 out per MTok
    deepseek-flash     $0.30 in / $0.006 cached    /  $1.20 out per MTok (peak)

    model              calls  fresh in  cached in   out    projected per day
    claude-opus-5         94   266,661    552,421  2,502   $1.71
    codex/gpt-5.6-sol     93         -          -      -   $0.74 to $1.83
    deepseek-flash        37    79,415        128 81,736   $0.16 to $0.32

Three things about these numbers, each of which changes how they should be
read.

**Only DeepSeek's is a bill.** It is on the metered API and the figure is what
the account is actually charged. The other two are counterfactuals: those calls
drew on a plan quota, which is a real cost and not a dollar one, and the desk
records `cost_usd: null` for them rather than `$0.00` for exactly that reason.

**The GPT figure is a range because Codex only reports one number.** Its banner
prints a total with no input/output split and no cache share, so the range runs
from "every token a cache read, Claude's own 0.3% output share" to "every token
fresh input". The truth is somewhere inside and this desk cannot narrow it
without a split the CLI does not give.

**These are the CLI route's token counts, not the task's.** Opus through the
Claude CLI spends 8,713 input tokens per call on this question; DeepSeek
straight at its API spends 2,146 on the same one. The difference is the CLI's
own system prompt, re-read every call and mostly served from cache, which is
why the cached share is 67% and why the figure is not ten times worse than it
is. Pointing these books at the APIs directly would change the input shape
substantially, and nothing here measures what that would cost.

Output volume is a model property and not a route one: Opus emits 27 tokens per
call here and DeepSeek emits 2,209, because one answers in JSON and the other
reasons out loud on the way there. At $25/MTok that difference would dominate
any direct-API comparison, and it is the reason the cheap model is not as cheap
as its input price suggests.


## 2026-09-18: terra's outage, and the account behind it

`ai-xau-terra-ctx` answered nothing between **10:15:13Z and 17:03:03Z** - 28
consecutive rows reading `codex exited 1: ... You've hit your usage limit ...
try again at Oct 17th, 2026 11:32 AM`. Its last real answer before the gap was
at 10:00:25Z. The book has 124 rows, of which 96 are real answers and 6 are
entries; the 28 dead rows are NOT decisions and are excluded the same way
every `unreachable` row on this desk is - the model was not consulted, so it
did not stand aside.

**What the CLI's own cache hid.** The owner reset the plan on the web and the
machine went on refusing. `~/.codex/auth.json` on the VPS had been written
2026-09-17 10:59 and carried `chatgpt_plan_type: "free"` with
`chatgpt_subscription_last_checked: 2026-09-17T03:59Z`. The entitlement is
cached in that file, so nothing done on the web reaches the CLI until it
re-authenticates. `codex login --device-auth` is the headless way: it prints
a URL and a one-time code, the owner approves in his own browser, and no
credential is typed on the VPS.

**The account changed, and the record must say so.** Before: the free tier on
`support@backcom.io`. After, from 17:10:31Z: `appjevn@gmail.com`, plan
`plus`, subscription active until **2026-09-23T17:48:08Z**. Same model string,
same route, different payer - which is exactly why `codex/gpt-5.6-terra`
names the route and not just the model. A probe at 17:11Z answered `OK` and
reported 11,988 tokens used for a one-word reply, in line with the CLI route's
overhead measured above.

**It expires on 2026-09-23.** Four days from the re-login. When it lapses the
book will start writing usage-limit rows again, which is a silent stop: the
run stays "alive", the executor stays attached, and only the decision rows
say anything is wrong. Whoever reads this next should check the newest row's
`response` before believing a quiet terra book.
