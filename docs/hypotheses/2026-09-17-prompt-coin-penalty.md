# 2026-09-17-prompt-coin-penalty: one sentence in the prompt, not the market, is why this model has never traded

**Registered:** 2026-09-17, **before the second book has seen a single bar.**
Everything below is a pre-commitment; nothing in it may be changed by what the
books turn out to say.
**Status:** registered, running from the next launcher start
**Books:** `ai-xau-opus-ctx` (base, already running, untouched) against
`ai-xau-opus-ctx-b` (variant `no-coin-penalty`), each with its own coin
**Code:** `py/live/ai_trader.py` `COIN_CLAUSE` / `--prompt-variant`

## A prompt change is a new hypothesis, and this is where it is registered

The desk's standing rule is that a book's id means one thing. It has been
applied to models twice this week — `ai-xau-sol-ctx` was stopped rather than
repointed when its model became unreachable, and terra got its own id rather
than inheriting sol's. The same rule applies to prompts, and more sharply: the
prompt is the entire input. Editing the running book's prompt would leave one
equity curve produced by two different questions, intact and unreadable.

So the base book is not touched, a second book is created, and the comparison
between them is the experiment. That also means this file has to exist before
the first bar, because a prompt change discovered to be interesting after the
fact is not a finding, it is a story about a number.

## The observation

`ai-xau-opus-ctx` has answered NONE on every bar it has ever been asked about.

    rows in decisions.jsonl                 99
    of which CLI auth failures               20   (logged unreachable, not stand-asides)
    real answers                             79
    of those, entries                         0
    posted to the desk                        0

Measured on the copy held on the desktop, 2026-09-16 03:10Z to 2026-09-17
05:00Z. The operator reports 104 of 104 on the VPS, which is the same fact a
few hours later; the denominator that matters is **real answers**, not rows,
because an auth failure is a missing answer and not a stand-aside. Both counts
give the same rate.

Each stand-aside carried a coherent, specific reason — the model is reading the
bars and declining, not failing.

On the same prompt, same market, same timeframe, same bars:

    deepseek-flash   `ai-xau-ds-ctx`     7 entries / 77 real answers   9.1%
    gpt-5.6-sol      `ai-xau-sol-ctx`    6 entries / 94 real answers   6.4%

## Claim

The opening sentence of the shared prompt reads:

> You are being measured against a coin that takes the same trades at the same
> bars with a random side, **so a trade you are not actually confident in is
> worse than no trade: it hands the coin a free sample.**

The bolded clause is an asymmetric penalty. It states a cost for a bad trade
and no cost for a missed one. For a cautious, instruction-following model,
standing aside is then the literal-safe answer on every bar, and it will be
the literal-safe answer on every bar forever, because no market state makes it
false.

**Declared direction:** with that clause removed and nothing put in its place,
`claude-opus-5` enters at a rate comparable to the other models on the same
prompt. If it does not, the clause is not the cause.

## What the variant changes, in full

The coin is still named, because the coin is still the measurement. What goes
is the instruction about what to conclude from it.

    base             ...with a random side, so a trade you are not actually
                     confident in is worse than no trade: it hands the coin a
                     free sample.
    no-coin-penalty  ...with a random side.

102 characters, one sentence-half, and **nothing is added**. No nudge toward
trading, no base rate, no "you may be being too cautious". Any of those would
be a second change and the result could not say which one did the work. The
rest of the prompt — desk state, market context, bars, the JSON contract, and
the line saying "NONE is a real answer and is often the right one" — is
byte-identical, and that was verified against the prompt in git rather than by
reading it.

## What settles it

**Stage one, the trade rate. N = 50 decided bars** on `ai-xau-opus-ctx-b`,
counting real answers only and excluding any row logged `unreachable`. About
twelve hours of 15-minute bars, so roughly a trading day.

N comes from the rates above. At the ~9% the other models show, the chance of
seeing zero entries in 50 bars is 0.91^50 = **0.9%** — so zero at N = 50 is
not bad luck. And the base book's 0/79 puts a 95% upper bound of **3.7%** on
its own true entry rate, against which:

    entries in 50 bars    P(at least this many | p = 3.7%)
    2                     0.56      inconclusive
    3                     0.28      inconclusive
    4                     0.12      inconclusive
    5                     0.038     confirms

**The rule, declared now:**

- **≥ 5 entries in the first 50 decided bars — the claim is supported.** The
  clause was the cause.
- **0 entries — the claim is falsified.** Not "needs longer": 0.9% under the
  alternative is the sample speaking.
- **1 to 4 — inconclusive**, and the window extends once to 100 bars, after
  which ≥ 8 supports and < 8 falsifies. One extension, declared here, so it
  cannot become a habit of waiting for the answer to arrive.

**Stage two, whether it is any good, and it is honest to say this will take
months.** Only after stage one is the -b book's R against its coin worth
reading. From the desk's own per-trade dispersion — mean +0.127R, sd 1.063R,
n = 55 — detecting a +0.127R edge at 95% needs about 270 trades, which at a 9%
entry rate is 3,000 bars, about 31 trading days for one book. Stage one
answers a question about the prompt in a day. Stage two answers a question
about the model, and not this month. **A profitable first week is not stage
two arriving early; it is noise, and this file says so in advance.**

## What would falsify it

- `ai-xau-opus-ctx-b` also entering at or near zero. Then the caution is the
  model's, or the market genuinely offered nothing, and the prompt is
  exonerated. This is the outcome the desk should expect to be able to
  publish, and it is worth as much as the other one.
- The base book beginning to trade on its own during the same window, which
  would mean the difference was the period and not the prompt. Both books are
  read over the same bars precisely so this is visible.

## What this does not say

It does not say the clause is wrong. A prompt that makes a model cautious may
be producing exactly the behaviour the desk wants, and a model that trades 9%
of bars is not thereby better than one that trades none — that is stage two's
question and stage one cannot touch it.

It does not say anything about the other models. Their rates are used here
only to set N, and a clause that binds one model need not bind another.

It does not make `ai-xau-opus-ctx`'s record worthless. 79 reasoned
stand-asides on known bars is a real observation about a model under a stated
instruction, and it is the control for this experiment.

And it does not test the coin. The coin is unchanged, still named in both
prompts, still flipping — on a **different seed** for the -b book, 17 against
11, so the two campaigns do not share one control sequence.

## The cold-window confound, and the exact rule for it

**A new book starts with an EMPTY window.** `start_ai_runs.py` creates it,
the poller feeds it one bar, and the trader decides on that one bar while the
control — running for days — is deciding on forty. Observed in this book's
own first row: *"the lone 15m bar closed near its low…"*. At 15m it takes
about ten hours to reach forty.

So a difference measured over that stretch is partly the prompt and partly
the window, and nothing separates them after the fact unless it is decided
now. Raised by a5 from the first two rows, and written here before any
disagreement rate has been computed.

**THE RULE, pre-committed:** a decision row counts toward the disagreement
rate, the trade count, the win rate and every R figure **only when the book
saw a full window** — forty bars. Rows below that are excluded from the
headline numbers and reported separately as the warm-up, with their count
stated.

**It is measured, not counted.** The criterion is read out of the row's own
stored prompt, which carries `LAST <n> BARS of <market>:<tf>`, and the row is
excluded when `n < 40`. Checked 2026-09-18: that line parses from 55 of 55
decision rows in `ai-xau-ds-ctx`.

Deliberately not "the first forty rows". Position is a proxy for window size
and a bad one — a restart, a poller outage or a missed bar makes the fortieth
row and the fortieth bar different things, and the proxy would then exclude
warm rows and admit cold ones while looking exactly as tidy. The prompt says
what the model actually saw, so that is what decides.

**Both arms are filtered by the same rule**, including the control's own rows
on those bars. A comparison in which one side is filtered is not a
comparison.

*Added 2026-09-18, after this book was already running and before any disagreement rate had been computed for it. The confound is the same one and the rule is the same rule; what is not claimed is that it was written before this book's first bar.*
