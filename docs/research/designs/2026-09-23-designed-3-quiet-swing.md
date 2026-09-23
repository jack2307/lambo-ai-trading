# designed-3 (lower frequency): quiet-tape multi-day continuation — **REFUSED, 0 methods proposed**

**Agent:** designed-3, angle 3 (lower frequency) of
`docs/hypotheses/2026-09-23-designed-methods.md`
**Branch:** `agent/designed-3`, worktree `E:/rust/fd-wt-dm3`
**Data root, every figure below:** `E:/rust/flowdesk/data-sealed` — the sealed
store, which physically ends 2025-09-22 23:45 UTC. `E:/rust/flowdesk/data` was
not opened, for any purpose, at any point. Every receipt prints its root.
**Methods proposed: ZERO.** One was designed, implemented, tested and measured;
it is refused, and the measurements are the result.

---

## What the angle was and what it turned out to be

The brief: everything on this desk is 15-minute intraday, that choice was
inherited rather than justified, and it is the worst possible choice for costs
because `cost/R = spread/stop`. So build something that holds for days, where
the stop is tens of points against the same 0.28 spread, and see whether the
desk's failure record is a frequency artefact rather than an edge problem.

**The cost half of that is exactly right and it is not enough.** The method
built here has a median sizing stop of **41.57 USD per ounce** on the broker's
own bars, so the configured spread costs **0.67% of R** where an intraday
two-dollar stop pays **14.0%** — the break-even bar falls by a factor of
twenty-one, measured, not argued. The method is net positive on every window
measured. It still cannot reach a profit factor of 1.2 with any reliability,
because the edge on offer is about **4% of R** and no amount of cost relief
turns 4% of R into a 1.2 profit factor. Lowering the cost bar stops an edge
being destroyed; it does not create one.

That is evidence for the registration's **explanation 2**, reached from the
cost side instead of from another failed search.

---

## 1. Mechanism — what was true before anything was fitted

Two facts, both published long before this data existed:

1. **Realised volatility clusters** (Engle 1982, and every ARCH paper since).
   Absolute returns are positively autocorrelated at the daily horizon in every
   liquid market. So "the tape has been quiet for two weeks" is a statement
   about the next two weeks as well as the last two, and it is computable from
   completed sessions alone.
2. **Signal extraction.** If an observed `K`-session move is a drift term plus
   noise, the posterior mean of the drift given that move scales as
   `var(drift) / (var(drift) + var(noise))`. The *same* observed move implies
   *more* drift when the noise variance is small. So whatever short-horizon
   continuation exists should be detectable when realised range is low and
   should wash out when it is high.

That is a prediction about **where** continuation lives, not a claim that it
exists. The design window says it is where the prediction puts it. Over 3,958
Dukascopy gold sessions (2010-06-01 → 2025-09-22, sessions counted from 17:00
New York), the two-sided `K = 10`-session momentum earns, over the following
five sessions, in units of the 20-session ATR:

| trailing 10-session true-range sum | n | 2-sided momentum | t | always-long in the same bucket | long share |
|---|---|---|---|---|---|
| bottom third of its trailing year | 1,323 | **+0.1649 ATR20** | +3.48 | +0.0469 | 53% |
| middle third | 1,068 | +0.0193 | +0.38 | +0.1375 | 55% |
| top third | 1,294 | **−0.0027** | −0.07 | +0.1511 | 54% |

The long share in the quiet bucket is 53%, so net drift exposure is 0.06 and
the bucket's own drift of +0.0469 contributes about +0.003 of the +0.1649. The
effect is not gold's drift. Split into three five-year blocks it holds its sign
and its size: **+0.136 (2010–2015), +0.177 (2015–2020), +0.137 (2020–2025)**,
with individual t of 1.74, 2.37 and 1.86.

### Why I did not believe the t, and what I used instead

I looked at roughly 70 configurations before this one (§6). At 70 looks an
isolated t of 3.5 is close to what noise produces, so the t is not the
evidence. The evidence I was willing to lean on was the three-block
replication above and a **second instrument**: the same frozen method on
Dukascopy silver, same fifteen years, same vendor, same guards. Signal
extraction is a property of the estimation problem, not of gold, so silver
should show it.

**Silver shows nothing: profit factor 1.012, expectancy +0.0062 R over 783
trades and 15.3 years.** That is the number that decided this.

---

## 2. Cost arithmetic at this stop distance

`cost/R = spread / stop`, and the stop here is `riskDailyRanges` (1.5) × the
average New York-day high-low range of the last `rangeDays` (20) days.
Measured across the method's own trades, guards on:

| window | sizing stop p10 / median / p90 (USD per oz) | spread as a share of R, p10 / median / p90 |
|---|---|---|
| Vantage XAUUSD.sc, 2022-06 → 2025-09 | 28.85 / **41.57** / 64.54 | 0.43% / **0.67%** / 0.97% |
| Dukascopy gold, 2010-06 → 2025-09 | 17.49 / **27.98** / 47.39 | 0.59% / **1.00%** / 1.60% |
| an intraday method stopped 2.00 USD away | 2.00 | **14.0%** |

So the break-even edge falls from 14.0% of R to 0.67% of R — twenty-one times
smaller on the broker's own recent bars, seventeen times over the fifteen
years. The arithmetic the brief set out is confirmed exactly.

Silver is the counter-example that makes the point about units: its spread is
0.021 against a median stop of 0.78 USD per ounce, so it pays **2.70% of R** —
four times gold's share, on a 50-ounce contract. A wide stop in *dollars* is
not the same thing as a wide stop relative to the instrument's own spread.

**One thing I did not do with this.** A sibling agent measured a method at
profit factor 0.973 — losing money — sitting at the 98th percentile of its
count-matched `RandomEntry` null, because `hypotheses::control_for` leaves the
control's `stopAtr` at its 1.5 default whatever the method uses. A wide stop
can therefore buy a percentile outright. This method is routed to `RandomHold`
instead, and that control **is** sized like the method — which is precisely why
its two sizing parameters are named `riskDailyRanges` and `rangeDays`:
`control_for` copies exactly those two names onto the null. The stop distance
is published above in points regardless, because a percentile is not evidence
and the arithmetic is.

---

## 3. Expected annual trade count

Measured, guards on:

| window | trades | sessions in window | per year (258 sessions) |
|---|---|---|---|
| Vantage XAUUSD.sc, 2022-06 → 2025-09 | 136 | 840 (768 after warm-up) | **41.6** |
| Dukascopy gold, 2010-06 → 2025-09 | 773 | 3,958 | **50.5** |
| Dukascopy silver, 2010-06 → 2025-09 | 783 | 3,955 | 51.1 |

The count is comfortably over the standing floor of 30, and **the guards are
why**. A five-session hold with one concurrent position and a filter that fires
about half the sessions produces only 22–28 trades a year on its own — I
measured that before implementing it, and it was the reason I nearly abandoned
the angle. The news flat (60/30 around USD impact-3 releases) closed 40 of 136
positions and the weekend flat closed 55, and each forced close is followed by
a fresh entry at the next session boundary. **The guards roughly halve the mean
hold and raise the trade count by about 50%.** That is worth recording on its
own: on this desk, the guards are not a tax on a multi-day method's trade count,
they are its main source.

**On one withheld year specifically.** The method needs 71 completed sessions
before its first decision (`lookbackSessions` 10 + `windowSessions` 60 + 1, plus
one more because the scan always discards the oldest session it reaches, which
may be truncated by the start of the data). The withheld year is 23,308 bars
≈ 253 sessions, so about **181 tradable sessions** remain. At the measured
0.177 trades per tradable session that is **about 32 trades — 28 to 36**. Right
on the floor, and capable of landing under it. Anything measured there is one
sampling accident away from being unscoreable.

---

## 4. What the weekend flat costs it

Measured by running the *same* `Guards` struct twice with
`flat_before_weekend_hhmm` set to 1640 and to 0 and nothing else altered, so
the news windows, the open-loss cap and the notional cap are identical in both:

| window | | trades | expectancy | profit factor | return | mean hold |
|---|---|---|---|---|---|---|
| Vantage 2022-06 → 2025-09 | flat 16:40 NY | 136 | +0.0434 R | 1.218 | +5.75% | 2.64 d |
| | weekend flat OFF | 130 | +0.0148 R | 1.075 | +1.90% | 3.30 d |
| Dukascopy 2010-06 → 2025-09 | flat 16:40 NY | 773 | +0.0375 R | 1.131 | +30.89% | 2.64 d |
| | weekend flat OFF | 717 | +0.0426 R | 1.158 | +33.19% | 3.43 d |

**The weekend flat does not cost this method anything; on the recent window it
is worth most of what the method earns, and on the fifteen years it costs
0.005 R a trade.** On the broker's own 3.27 years, removing it takes expectancy
from +0.0434 R to +0.0148 R and the profit factor from 1.218 to 1.075 — the
Friday-to-Sunday gap was, on that window, a coin flip the method was better off
not holding. On the fifteen years the sign reverses and the size is a fifth as
large. Two windows, opposite signs, both small: **the honest reading is that
the weekend flat is worth about zero to a multi-day gold method, and that the
+0.029 R on the recent window is noise the size of the whole edge.** The brief
expected the weekend flat to cut a multi-day method and change what it is. It
does change the mean hold, from 3.3–4.5 days to 2.6, and it changes the P&L by
less than the P&L's own standard error.

---

## 5. Design-window numbers

Every row: `run_hypothesis_fixed_guarded` — the same function the three-month
search's out-of-sample leg calls — at the frozen parameters, guards on, spread
as configured, 200 null seeds, no folds, no selection, no re-fit. Binary:
`crates/fd-backtest/src/bin/designed_3.rs`, run from this worktree's own
`target/release/` (the target directory is **not** shared; a sibling was bitten
by a stale binary in the primary tree).

| | Vantage XAUUSD.sc 2022-06→2025-09 | Dukascopy gold 2010-06→2025-09 | Dukascopy silver 2010-06→2025-09 |
|---|---|---|---|
| trades | 136 (41.6/yr) | 773 (50.5/yr) | 783 (51.1/yr) |
| expectancy | +0.0434 R | +0.0375 R | +0.0062 R |
| profit factor | **1.218** | **1.131** | **1.012** |
| return | +5.75% | +30.89% | +2.80% |
| win rate | 43.4% | 47.0% | 45.7% |
| mean hold | 2.64 days | 2.64 days | 2.58 days |
| max drawdown | 5.30% | 14.95% | 20.89% |
| drift null as the registered machinery builds it | 100th pct, **count match 3.71 — UNMEASURED** | 100th pct, **count match 3.13 — UNMEASURED** | 100th pct, **count match 3.14 — UNMEASURED** |
| drift null with its entry rate calibrated (count match 1.03) | **82nd** pct (null 95th PF 1.442) | **96th** pct (null 95th PF 1.115) | **96th** pct (null 95th PF 1.011) |
| direction null | **84th** pct (null 95th PF 1.686) | **92nd** pct (null 95th PF 1.140) | 86th pct (null 95th PF 1.054) |
| long / short trades | 83 / 53 (61% long) | 439 / 334 (57% long) | 400 / 383 (51% long) |
| long P&L / short P&L | +12.81 / **−7.06 USD** | +2730.39 / **+358.41 USD** | −626.49 / +906.29 USD |

**No window clears all four legs, and the two gold windows fail on different
legs** — the 3.27-year window clears the profit factor and fails both nulls,
the 15.3-year window clears the drift null and fails the profit factor and the
direction null. That pattern is what a method sitting on the noise floor looks
like; a real edge fails the same leg or none.

Read the drift-null row carefully. The registered machinery reports the **100th
percentile** on all three instruments, and that number is worth nothing: the
null takes 3.1–3.7 times the method's trades and `count_matched()` says so.
With the entry rate calibrated so the count match is 1.03, the same control at
the same 200 seeds puts the method at the **82nd** percentile on the broker's
bars. A three-figure percentile collapsed to a two-figure one by fixing the
count. This is the already-recorded hold-null defect and §7 says where it lives.

The drift decomposition is the other thing to read. On the broker's 3.27 years
the short leg loses 7.06 USD while the long leg makes 12.81 — so *that* window's
profit factor of 1.218 is partly gold's own drift, and neither null has any
drift exposure to control for it. On the fifteen years the short leg is
positive (+358 USD), which is the one figure that argues the effect is real
rather than beta. Silver's short leg carries the whole of its (nonexistent)
result. Three instruments, three different stories about the same mechanism.

---

## 6. How many variants I tried — my own multiplicity

Stated in full, because the design-window number above cannot be read without
it. Everything on the sealed store only.

**Structure screens (no trading, no costs — forward returns in ATR units):**

| # | what | cells |
|---|---|---|
| 1 | trailing K-session sign → forward N-session return, K∈{3,5,10,20,60} × N∈{1,3,5,10}, two series | 20 |
| 2 | the same split against the unconditional drift, K∈{5,10,20,60,120} × N∈{3,5} | 10 |
| 3 | failed 20-session extreme (probe through a multi-day high/low that closes back inside), N∈{3,5}, both sides | 2 |
| 4 | contracted-range breakout, 10-session range in the bottom third of its trailing year, N∈{3,5} | 2 |
| 5 | week as the unit: prior K-week sign, K∈{1,2,4,12,26}; plus Friday's close in the top/bottom 20% of the week's range | 7 |
| 6 | Lo–MacKinlay variance ratios, 6 intraday horizons + 6 session horizons, two series — descriptive, no direction taken | 0 |
| 7 | K-session momentum × range-regime tercile, K∈{5,10} × 3 buckets | 6 |
| 8 | the surviving cell split into five-year blocks, and the ATR distribution inside the quiet bucket | 0 |
| | **subtotal** | **47** |

**Trading configurations (session-level simulation, spread charged, one
position at a time, weekend truncation):**

| # | what | cells |
|---|---|---|
| 9 | quiet quantile ∈ {1/3, 1/2, 1 (no filter)} × stop ∈ {1.0, 1.5, 2.0} × ATR20 | 9 (3 distinct signals) |
| 10 | percentile window ∈ {60, 120, 258} × quantile ∈ {1/3, 1/2} × hold ∈ {3, 5} sessions | 12 |
| 11 | K = 5, window ∈ {40, 60} × quantile ∈ {0.4, 0.5} | 4 |
| 12 | K = 10, window ∈ {30, 40} × quantile ∈ {0.4, 0.5} | 4 |
| | **subtotal** | **29 (23 distinct signal configurations)** |

**Through the guarded engine: exactly ONE parameter set**, run on three
instruments × three guard settings. Nothing was re-fitted after the engine
saw it; the parameters below are the ones the session-level stage chose.

**Total: 47 structural screens + 23 distinct trading configurations = 70
looks.** Two of the parameter choices were made *against* the design-window
number and I want that on the record, because it is the only part of this that
resists the multiplicity: `windowSessions = 60` was chosen over 120 and 258,
which both had larger measured edges, because the longer windows cost 130 and
270 sessions of warm-up and would have eaten a third of a one-year test window;
and `quietPct = 0.50` was chosen over 1/3, which also had a larger edge,
because 1/3 fired too rarely to reach 30 trades. Both choices traded measured
edge for trade count.

---

## 7. Defects found

### 7.1 `max_hold_ms` is four hours and it applies to every method in the record

`config/default.toml` sets `[trading] max_hold_ms = 14_400_000`, and
`engine::trading_rules_for` reads it from the shared table with **no
per-market override path** — `fd-backtest/tests/trading_rules.rs:49` even
asserts BTC and gold carry the same value. `engine::check_exit` then closes any
`Exits::Engine` position once `bar.time - entry_time > max_hold_ms`.

So **every `Exits::Engine` method ever measured on this desk was force-closed
after at most 16 fifteen-minute bars**, whatever its signal horizon. The
three-month search's five strategies are all engine-managed. The desk has never
measured a multi-day hold outside `tsmom`, `buy-and-hold`, `session-hold` and
`intraday-momentum`, which are the four `Exits::Strategy` methods in the
registry. "This desk is 15-minute intraday" is not only a statement about the
bar width; there is a four-hour clock underneath it that no registration chose.

**The judgement call, stated because a sibling made the opposite one.** Three
routes were available: (a) `Exits::Engine` and accept four hours, which is not
the angle; (b) `Exits::Strategy`, which escapes the clock but routes the method
to the `RandomHold` null and leaves the percentile legs unreadable; (c) raise
`max_hold_ms`, which restates every stored number in the record and is not an
agent's change to make. **I took (b).** The percentile legs are reported as
UNMEASURED with the count-match ratio beside them, and expectancy and the
profit factor carry the verdict. I also re-checked the realised holds rather
than assuming them: geometric-mean hold 2.19–2.24 days, shortest 795 minutes,
longest 5–7 days. The holds really are multi-day.

Route (c) remains the blocker for anybody who wants a multi-day method measured
through `Exits::Engine` with a real stop and target. It is a lead's call.

### 7.2 The drift null is hold-matched but never count-matched

`RandomHold` carries an `entryRate` parameter and
`hypotheses::matched_control_for` will set it — but
`run_hypothesis_fixed_guarded` computes the rate as

```rust
let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, result.trades.len(), guards));
```

so on the **drift** branch `rate` is `None`, `entryRate` stays at its default
`1.0`, and the control re-enters on every bar it is flat. It is therefore
matched in hold *length* and unmatched in *frequency*. `tsmom` is always in the
market, so the two coincide there and nothing showed. A method that trades in a
slice of the sessions gets a null with several times its trades:

| instrument | method trades | null median trades | count match | percentile as reported | percentile at count match 1.03 |
|---|---|---|---|---|---|
| Vantage gold | 136 | 504 | 3.71 | 100th | **82nd** |
| Dukascopy gold | 773 | 2,420 | 3.13 | 100th | **96th** |
| Dukascopy silver | 783 | 2,458 | 3.14 | 100th | **96th** |

A null that trades 3.7× as often has a much tighter profit-factor
distribution, so the method's percentile is inflated toward 100 by arithmetic.
`count_matched()` correctly reports every one of these as out of band, so the
machinery is honest about it — but the printed percentile is the number a reader
takes, and on this method it was wrong by 18 points.

The calibrated figures above were produced by running the *same* `RandomHold`
control at an entry rate bisected to the method's trade count, inside
`designed_3.rs`. **Nothing shared was changed.** Both figures are published.

### 7.3 Neither null controls for the instrument's unconditional drift

Not a code defect, a reading trap, and it bites low-frequency methods
specifically. The count-matched null takes random *sides* and the direction
null flips half of them, so both have approximately zero net drift exposure. A
method that is long-biased and holds for days has a great deal of it. Gold's
unconditional forward five-session move over 2022-06 → 2025-09 is **+0.3946 of
a 20-session ATR** (t = +6.74); over 2010-2025 it is +0.1258 (t = +4.88). A
long-only multi-day gold method on either window clears both nulls on its side
ratio alone and has no edge whatsoever.

This is why the method here is two-sided and why the side split and each leg's
P&L is printed beside every result. It is also why **I refuse to submit the
obvious alternative**: a weekly-reset long-only hold measures +0.179 ATR8 per
week on the recent window (t = +3.26), would clear the gate comfortably, and is
`buy-and-hold` with weekend flats. The registry already has that control.

### 7.4 Operational: the disk was at 100%

`E:` reached 301G/301G with 652K free mid-run and a release link failed with
`rustc-LLVM ERROR: IO failure on output stream: No space left on device`. The
target directories are **not** shared: `fd-wt-dm1` 12G, `fd-wt-dm2` 11G,
`fd-wt-dm4` 2.2G, `flowdesk-htf` 13G, `flowdesk-lag` 16G, `flowdesk` 32G. I
cleared only my own worktree's `target/debug` and did not touch anyone else's.
Four concurrent agents each building their own `target/` will fill this disk.

---

## 8. Why this is refused rather than submitted

The registration says a null submission is valid and preferred, and that an
agent submitting filler has made the program worse. This method fails a
falsifier leg **on its own design window**, on both gold windows, and its
nearest sibling instrument shows nothing at all. Submitting it would mean
predicting that a percentile of 82 or 92 will rise past 95 on a *smaller*
sample. That is a coin flip dressed as a hypothesis, and it would spend an
eighth of the program's multiplicity to buy one.

What the program gets instead is the measurement, which answers the angle:

1. **Gold has no linear return predictability at any horizon from 30 minutes to
   60 sessions.** Lo–MacKinlay variance ratios on 362,802 Dukascopy 15-minute
   returns and 3,957 session returns sit within 1–7% of 1.0. The only
   deviations with |z| > 2 are at 30 and 60 minutes (VR 0.9878, z = −3.17; VR
   0.9792, z = −3.01) — about 1–2% of variance, four to eight times smaller
   than the spread can collect at any intraday stop. At session scale the
   ratios drift to 0.889 by q = 60 with |z| < 0.7: not distinguishable from 1.
2. **Every two-sided low-frequency directional signal I measured is at or below
   gold's unconditional drift.** Two-sided momentum at K = 5, 10, 20, 60 and
   120 sessions; weekly momentum at 1, 2, 4, 12 and 26 weeks; failed multi-day
   extremes; contracted-range breakouts. Not one beats always-long.
3. **The one conditional effect that replicates across three independent
   five-year blocks yields profit factor 1.131 on 15.3 years of gold, 1.218 on
   3.27 years of the broker's own gold, and 1.012 on 15.3 years of silver** —
   at a stop twenty-one times wider, relative to the spread, than any intraday
   method on this desk has used.
4. **So the frequency explanation is refuted by measurement rather than by a
   failed fit.** The cost arithmetic works precisely as the brief said it
   would. What it reveals is that there was about 4% of R to save, and the edge
   underneath is the same 4% of R. The desk's record of profit factors near 1.0
   is not an artefact of holding for hours.

The code, the tests and the frozen parameters stay in this branch so the claim
is reproducible and so a lead who judges the number worth a multiplicity slot
can run it without re-deriving anything. `2026-09-23-designed-3-frozen.toml`
says on its face that it is not a submission.

## Reproducing every figure

```sh
# from inside this worktree — NOT the primary tree's target/
export PATH="/c/winlibs/mingw64/bin:$PATH"
cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin designed_3
./target/release/designed_3.exe --market=xauusd  --interval=15m \
  --data=E:/rust/flowdesk/data-sealed --from=2022-06-16 --to=2025-09-23 --seeds=200
./target/release/designed_3.exe --market=xauduka --interval=15m \
  --data=E:/rust/flowdesk/data-sealed --from=2010-06-01 --to=2025-09-23 --seeds=200
./target/release/designed_3.exe --market=xagduka --interval=15m \
  --data=E:/rust/flowdesk/data-sealed --from=2010-06-01 --to=2025-09-23 --seeds=200

# causality: a truncated walk is a prefix of the full walk, and the forming
# session is invisible however extreme it is
cargo +stable-x86_64-pc-windows-gnu test -p fd-strategy --test causality_quiet_swing
```

The structure screens of §1 and §6 were Python over the same sealed parquet
files; they measure forward returns and take no trades, and every figure they
produced that this note relies on is reproduced by the engine runs above.
