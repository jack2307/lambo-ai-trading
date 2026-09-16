# 2026-09-16-dealer-hedge-demand: a customer's option trade forces someone to buy gold, and it has to be done now

**Registered:** 2026-09-16 — **three months before the data it will be run on
exists.** The tape held 10 days when this was written. Everything below is a
pre-commitment; nothing in it may be changed by what the tape turns out to say.
**Status:** registered, **locked until the sample condition below is met**
**Script:** `scripts/dealer_hedge.py` (to be written; not before the lock lifts)

## The lock — read this before running anything

**This registration may not be run until the tape holds at least 500
non-overlapping qualifying bursts.** At the rate measured on 2026-09-16 that is
about December 2026. Running it early and "having a look" is the failure this
whole file exists to prevent, and the count is checkable from the data.

## Why it is worth registering now

`docs/decisions/2026-09-15-intraday-frontier.md` set the bar: a five-minute
mechanism on this account must produce about **0.9 bp of gross edge per trade**,
and the binding constraint is effect size, not cost. Five ideas were killed
against that arithmetic in two days without costing a research cycle. This is
the only candidate left that could clear it, and the reason is that its flow is
**forced**: a dealer who has just sold an option does not choose whether to
hedge, or when.

Every other payer story on the shelf needs someone to be wrong. This one only
needs someone to be obligated.

## Claim

A customer buys a call; the dealer is short that call and must buy gold. A
customer buys a put; the dealer must sell. The trade is a market order, it is
price-insensitive, and it happens within minutes of the print.

**Declared direction, and there is no mirror:** when net dealer hedge demand
over the window is **positive** (dealers must buy), gold **rises** over the next
five minutes. A result in the opposite direction closes this and is recorded as
such.

### Why the individual print is NOT the event, and this is the whole design

Measured on 2026-09-16 over five full days, 20,167 prints:

```
mean GROSS hedge demand   502,395 oz/day = 2.010% of COMEX daily volume
mean |NET| hedge demand    23,899 oz/day = 0.096%
a single >= $100k print      1,223 oz    = 0.0049%
```

**A single large print cannot move gold** — five thousandths of one percent of a
day. Any registration built on "a big print appears, then price moves" is dead
before it is written, and this one says so in advance.

What is left is the **net**, and the gross-to-net ratio is **21:1**: almost all
of this flow cancels against itself. That is the same lesson
`2026-09-15-quote-asymmetry` paid for, where 91.2% of one-sided quote movement
cancelled inside the minute. The object is therefore the **signed residual
accumulated over a window**, never a print and never a gross total.

Sizing the claim honestly: a burst carrying ~1% of the window's volume implies,
under a square-root impact law, roughly **2–4 bp**. That is the number the gates
below are set against, and it is an order of magnitude, not a measurement.

## What this is not

Price impact of order flow is textbook, and two option-tape ideas are already
sold commercially: **unusual options activity** (a claim about the buyer being
informed) and **gamma exposure / pinning** (a claim about volatility regime).
This is neither. It is a claim about an obligation, at a five-minute horizon,
on a feed that is not redistributed and priced at an account whose spread this
repository has measured tick by tick. The mechanism is not novel; **the
measurement is not available to anyone else**, and that is the honest statement.

## Construction — every constant fixed now

Per print in `data/gold/tape`:

    m      = (strike - underlying_price) / underlying_price
    delta  = +clip(0.5 - K*m, 0.02, 0.98)   for a CALL
             -clip(0.5 + K*m, 0.02, 0.98)   for a PUT
    side   = +1 if aggressor_side == 'BUY' else -1     (the dealer takes the other side)
    oz     = contracts * 100                            (COMEX gold option = 100 oz)
    hedge  = side * delta * oz                          (signed ounces the dealer must trade)

**K = 4.0, declared now.** The tape's `implied_volatility` is NaN, so a real
delta cannot be computed and this moneyness proxy is crude. **Sensitivity at
K ∈ {2, 4, 8} is reported as a diagnostic beside the gate, never as extra
cells**; if the verdict flips across K the claim is closed as unmeasurable
rather than reported at the best K.

    net_t   = sum of hedge over the 5 minutes ending at t     — THE SIGNAL
    gross_t = sum of |hedge| over the same window             — THE CONTROL

`net` is scaled by the trailing 20 trading days of its own dispersion, shifted
one window, so nothing at or after t enters the denominator.

## The control is inside the claim

**Gross demand must NOT predict direction.** Gross is how much hedging happened;
net is which way it had to go. If the gross arm clears the gate, the effect is
volatility or activity and not obligation, and this closes **regardless of what
the net arm scores**. Same machinery, same threshold, same fills — the shape
that worked on 2026-09-15.

## Fills, cost, and the clock

- Entry at the **first tick at least 1.0 s after the window closes**, at the ASK
  for a long and the BID for a short; exit five minutes later by the same rule.
  The spread is paid at the quotes that would have filled it — no spread
  constant appears anywhere. Machinery exists: `scripts/quote_asymmetry.py`.
- Latency curve at **0 / 1 / 5 / 30 s** reported; 1 s is the registered cell.
- **Alignment gate first, and it is not a return.** The tape is stamped UTC; the
  Vantage tick features are stamped on the **broker clock** (UTC+3 summer, +2
  winter) and stored unconverted — the fault `2026-09-15-quote-asymmetry`
  shipped. Before any number is read, the daily-break test from
  `2026-09-15-venue-residual` must show the two sources agreeing to the minute
  and shifting on the same DST weeks. A failure here stops the study.
- No trade may span the daily stop or a weekend.

## Falsifier — all eight, and the last one is new

1. At least **500** non-overlapping qualifying bursts (the lock).
2. Mean net return per trade **> 0**, spread paid from the filling quotes.
3. At or beyond the **97.5th percentile** of a sign permutation matched on both
   the hour and the volatility decile, **100,000 draws**.
4. The **day-block bootstrap 95% interval excludes zero** (20,000 resamples).
5. **No single calendar month carries more than 40% of the gross.**
6. **The gross control arm does NOT clear gate 3.**
7. The sign **survives the configured guards** (`max_trades_per_day = 4`,
   30-minute cooldown).
8. **The harness must pass its own self-test on this data** — a planted edge of
   known size recovered above the gate, pure noise below it, the pattern
   `scripts/quote_asymmetry_selftest.py` established. A result from an untested
   instrument is not admitted.

**Minimum detectable effect, computed before the falsifier** as the frontier
record now requires: at a five-minute horizon (dispersion 13.7 bp) and a 0.455 bp
round trip, **MDE = 2·13.7/√n + 0.455** — 1.59 bp at 500 bursts, 1.28 bp at 800.
The claim's own sizing says 2–4 bp. The design is therefore powered **if the
impact law holds**, and underpowered by a factor of about two if the true effect
is 1 bp. That is stated now so the record cannot later call a null "no effect".

## Data

- **In-sample:** the gold tape from **2026-09-06** to the date two thirds of the
  way through whatever exists when the lock lifts, against `XAUUSD.sc` ticks.
- **Out-of-sample, named now:** the final third, opened only after the in-sample
  result is written down.
- **Mechanism confirmation, opened with the out-of-sample and not before:** the
  **BTC** tape (`data/btc/tape`, Deribit) against `BTCUSD` — a different asset,
  a different options market, the same obligation.
- The tape is collected live by `fd-ingest --bin collect --market=gold`; history
  not captured as it happens is gone, so **the collector must not be stopped.**

## What each outcome means

- All eight gates, then the out-of-sample and BTC → decision record, then a
  **paper** proposal. Nothing places an order.
- Gate 6 fails → closed as activity, not obligation.
- Direction opposite to the declared one → closed. There is no mirror.
- Gates pass in-sample, fails out-of-sample → closed. No second out-of-sample.
