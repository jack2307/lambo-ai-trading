# 2026-09-24-live-coin-parity: does the backtest's random-entry null describe random entry on the live feed

**Registered:** (commit time is authoritative) — before the books exist and before
a single live coin trade is posted
**Status:** open

## Where this came from, and what it is not

The owner asked to run the coin books that look good on paper: *"Chạy thử mấy
chiến lược coin đang tốt bên paper đi."*

Two things had to be said before building anything, and they change what gets
built:

1. **The coin books are already running on paper.** They are the controls of the
   nine AI books and have been since those books started.
2. **A coin book is not a strategy.** It has no entry rule of its own — it posts
   a mirrored side only on bars its model chose, and it sits out the 242 bars
   `ai-xau-ds-ctx` sat out. There is nothing to "promote".

So this is not that. **Running the best-looking coin books would be selecting the
maximum of nine noisy draws at 20–43 trades**, which is the error the whole record
is about, and it would answer nothing.

What can be built is a standalone random-entry book with its own entry rule — and
that turns out to be worth building for a reason that has nothing to do with the
owner's question. **Nobody has ever checked that the backtest's random-entry null
describes random entry on the live feed.** Every percentile this desk has ever
published is read against that null, and it has only ever been measured inside the
backtest, against stored bars, at a charged spread.

## Claim

Random entry on the live feed, at the backtest null's own rule, produces a profit
factor close to what the backtest null reports — and specifically **slightly
better**, because the live spread is smaller than the one the backtest charges.

## The numbers this rests on, all already measured

- **The backtest's random-entry null has a median profit factor of 0.856–0.874**,
  across 11 separate measurements of 200 seeds each on long gold windows
  (`docs/research/runs/2026-09-24-cost-matched-null/`, `…/2026-09-23-designed-1-cost-term/`).
- **The backtest charges a spread of 0.28.** The live logger's measured median on
  XAUUSD.sc is **0.220** from 2,315 samples over 12.8 hours
  (`config/default.toml:317`, which also records that 0.28 "is above every spread
  ever observed").
- At the registry's 1.5 ATR stop, the charged spread costs **7.50% of R**
  (`docs/research/runs/2026-09-24-repair-d/`). The measured spread costs
  0.220/0.280 of that, **5.89% of R**.

## The prediction, arithmetic written out before any trade exists

With the config's `reward_risk` of 1.8, expectancy in R is `1.8w − (1 − w)`, so a
profit factor of `1.8w/(1 − w)`.

| | cost per trade | expectancy | profit factor |
|---|---:|---:|---:|
| backtest null, spread 0.28 | 7.50% of R | −0.096 R | **0.858** |
| live feed, spread 0.220 | 5.89% of R | −0.080 R | **0.881** |

**So: pooled live random entry should land near 0.88, and must not land near 1.0.**
The gap between 0.858 and 0.881 is the whole content of the parity check, and it is
small enough that it needs a few hundred trades to see — which is stated here so
that a number at 60 trades is not read as a result.

## AMENDED before a single trade was posted: the fixed prediction above is wrong

The six books were created and the arithmetic was checked against the live feed
before any poster started. It does not hold, and the reason matters more than the
number.

**The prediction above used a historical median ATR. The books trade at whatever
ATR obtains.** Measured on the live feed at 2026-09-24, gold 15m **ATR14 is 8.92
points** — about **3.6x** the 2.49 implied by the 2022-2025 median that
`docs/research/runs/2026-09-24-repair-d/` reports. So a 1.5 ATR stop is **13.37
points**, not 3.73, and the round trip costs:

| | stop | cost/R |
|---|---:|---:|
| 2022-2025 median ATR, charged spread 0.28 | 3.73 pts | 7.50% |
| 2022-2025 median ATR, measured spread 0.220 | 3.73 pts | 5.89% |
| **today's ATR, measured spread 0.220** | **13.37 pts** | **1.65%** |

With `reward_risk` 1.8, profit factor is `(b - c)/(b(1 + c))`, so **c = 1.65% implies
a profit factor of about 0.97** — not 0.88, and close enough to 1.00 that the
falsifier as written above would have **refuted a null that was behaving correctly.**

That is the error this registration existed to catch, caught on itself. It is
recorded rather than rewritten.

### The falsifier that replaces it

A fixed number cannot work, because the cost these books pay is set by the
volatility they happen to trade through. So the prediction becomes relative, and the
comparison is against the cost the trades actually paid:

- Record, per trade, the **realised stop distance in points** and therefore the
  realised `cost/R`. Take its median over the pooled trades.
- The predicted profit factor is `(1.8 - c) / (1.8 x (1 + c))` at that realised
  median `c`, using the measured spread of 0.220 — **computed from the trades' own
  cost, not from a historical median.**
- **Confirmed** if the pooled profit factor is within **0.06** of that prediction.
- **Refuted** if it is more than 0.06 above — live random entry beating its own cost
  model means the null every published percentile is read against is wrong.
- **Refuted** if it is more than 0.06 below — something in the live fills costs more
  than the spread, which would be worth knowing on its own.

The 0.06 band is wider than the gap between the charged and measured spread at any
plausible ATR, which is deliberate: this test can say whether live random entry
matches its cost model, and it cannot say which spread figure is right.

**And the headline consequence, stated now rather than discovered later:** at
today's volatility a coin on gold loses only about 1.65% of R per trade, so a
pooled profit factor near 0.97 is the *correct* result and is **not** evidence that
random entry works. It is evidence that gold's ATR is currently large enough to make
the spread nearly irrelevant — which is the same finding the cost table already
carries, from the other direction.

## Falsifier

On the **pooled** trades of all six books, at 200 pooled trades and again at 400:

- **Refuted if the pooled profit factor is at or above 1.00.** Live random entry
  making money would mean the null every published percentile is read against is
  wrong, which is a defect far more important than anything about coins.
- **Refuted if the pooled profit factor is below 0.75 or above 0.98.** The
  prediction is 0.88 and the band is deliberately wider than the arithmetic on
  either side; outside it, something in the cost model or the live fills is not
  what the backtest thinks.
- **Confirmed only in the band 0.84–0.93**, which straddles both the backtest's
  0.858 and the live prediction's 0.881 and does not let me claim to have resolved
  which.

## The second claim, and the one the owner's question really tests

**Six books, identical rule, different seeds, will spread far apart at small trade
counts and converge as the count grows.** At 20–40 trades the existing nine coin
books span **0.762 to 2.178**; six fresh books at the same size should span
something similar, and by 200 trades each the spread should be visibly narrower.

This is registered because it is the honest answer to "run the good coin": there is
no good coin, there is a distribution, and watching six identical things disagree by
a factor of three is the most direct way to see it. If they do **not** spread at
small counts, or do **not** narrow as counts grow, I have misunderstood the noise
and that is worth knowing.

## The rule, fixed now

Matched to the backtest null rather than chosen:

- `market` xauusd, `tf` 15m, strategy `external`, guards on, **no session or news
  filter** — the null runs with guards and no filters.
- **Entry rate 0.04 per closed bar**, the top of `RandomEntry`'s own grid
  (`crates/fd-backtest/src/control.rs:73` carries 0.01/0.02/0.04). About 3.8 trades
  a day at 96 bars.
- **Side 50/50 from a seeded stream**, seeds 101–106, one per book, so every book
  replays.
- **Stop 1.5 × ATR14**, the value `RandomEntry::default_params()` carries; target
  from the config's `reward_risk`.
- Six books: `coin-live-101` … `coin-live-106`.

**No Rust changes.** `external` is already a registered strategy that takes posted
intents and decides nothing itself, and `paper.rs` refuses an intent to any book
that is not `external`. `RandomEntry` is deliberately **not** in the strategy
registry and stays out of it — registering it would put a control into every
`--mode=sweep` run.

## What this costs

**Nothing.** These books call no model, so unlike the nine AI books they burn no
DeepSeek or Opus API. They are paper, so no money is at risk. The only cost is six
more rows in the desk's status.

## Timescale, said now so nobody reads an early number

At 3.8 trades a day a single book reaches 200 trades in about **53 days**. Pooled
across six, 200 trades arrive in about **9 days** and 400 in about **18**. **The
pooled number is the one the falsifier is written against**; the six individual
books exist to show the spread, and any single book's profit factor before a few
hundred trades is an illustration of noise rather than a measurement of anything.

## What is pre-committed

1. **The pooled number is published at 200 and 400 trades whatever it says**,
   including if it is above 1.00 and embarrassing to the argument I have been
   making to the owner all week.
2. **No book is dropped, restarted or reseeded** for performing badly or well. A
   reseeded coin is a selected coin.
3. **Nothing here goes near real money**, whatever it shows. Account 33708517 was
   stopped by the owner on 2026-09-24 and this programme does not restart it.
4. **If the pooled figure lands above 1.00**, the registration that follows is
   about the null and not about coins, and the funded books' published percentiles
   are re-read before anything else happens.
