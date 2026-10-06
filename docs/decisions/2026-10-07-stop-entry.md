# 2026-10-07-stop-entry: a resting entry on the BREAKOUT side, the mirror of the limit family

**Registered:** 2026-10-07, before the first line of engine code on this branch.
Branch `agent/stop-entry`, worktree `/e/rust/fd-stop-entry`, cut from `agent/n3`.
Brief: `AGENT-BRIEF-2026-10-07-NIGHT.md`.

## 0. The family, in one sentence

`agent/n3` built a resting entry order that sits on the **pullback** side of the
signal bar's close: it **earns** half the spread and it only ever fills on a move
back against the trade. This family turns that order round. The order rests on
the **breakout** side — above the close for a long — so it **pays** half the
spread **plus** the offset distance, and it fills **only when price has already
moved the way the signal said**.

Both are the same object in the same place in the code. Only the sign of the
offset, the touch test and the sign of the half spread differ.

## 1. Hypothesis

For a given mechanism, routing the same signals through a resting order on the
breakout side raises sum net R over the window, because the confirmation it
demands removes the signals that never moved — and that selection is worth more
than the entry distance plus the spread it pays.

## 2. The prior, and what it is built on

Written first so a negative result is not read as a discovery.

**For the hypothesis — one measurement, and it is the only non-zero thing in six
programmes.** `agent/n1` found a rule read **in the direction of the move** after
a release that holds on two four-year windows (PF 1.636 / 1.734) and inverts to
0.560 when its direction is flipped. A continuation rule is the kind of rule a
stop entry serves; a breakout-side order is a continuation filter expressed in
the fill model instead of in a strategy.

**Against the hypothesis — three, all on this desk.**

1. `docs/decisions/2026-10-06-limit-entry.md` section 4, measured on 105
   arm-rows: the method's resting-order fill rate minus its matched null's has
   **median +0.40 points of rate**, and the method filled **less** than its null
   in **35 of 105** rows. The *pullback*-side order's selection is the
   unconditional base rate. If the breakout-side difference is also ~0, these six
   mechanisms carry no information about whether price runs either, and the
   family is measuring nothing but the order.
2. `py/live/mt5_executor.py --max-join-r`, 81,789 observations, 15m gold
   2022-06 to 2026-09: after adverse entry drift +0.0926R, inside the band
   +0.0992R, after favourable drift +0.0938R — equal within 0.007R. **Entry
   distance is worth its distance and nothing more.** A breakout entry is an
   adverse-drift join by construction, so this says the offset is a pure cost.
3. Brief section 3.1: entry has negative expectancy on these mechanisms **at
   spread zero** (`ema-cross` -0.147R, `rsi-reversion` -0.144R). Selecting a
   subset of a negative-expectancy signal set has no reason to make it positive
   unless the selection itself carries information.

**What is NOT measured anywhere, and is this job:** a breakout-side resting
entry, conditional on a real mechanism's signals, inside the engine, with the
spread term paid and the offset distance paid, against a matched null that rests
the same order, read at the gate — and the fill rate read against that null's.

## 3. The change, and the one property that must not break

`TradingRules.limit_entry` already exists and already lives **in the RULES and
not in a strategy**. That is load-bearing and is kept: `control::RandomEntry`
emits `Intent::Enter` like any method, so it is routed through the identical
order, the identical price rule and the identical fill selection, with **no
null-specific code**. `tests/limit_entry.rs::the_matched_null_rests_its_orders_too`
pins it. This registration adds **one field to that struct** and no new path:

* `LimitEntry` gains `side_of_close: RestSide`, `serde(default)` = `Pullback`,
  so every config and every receipt before today is untouched.
* `RestSide::Breakout` changes exactly three things inside `Working`:
  * **the level**: `close + offset*ATR` for a long (minus for a short), the
    opposite sign to `Pullback`;
  * **the touch test**: a long fills iff `bar.high >= L`, a short iff
    `bar.low <= L`;
  * **the fill price**: a gap through the level fills at the **open**, not at
    the level — `max(L, bar.open)` for a long — and the fill then **PAYS** half
    the spread, i.e. exactly `apply_costs(.., entering = true, ..)`, the same
    charge the market arm takes.
* Everything else is shared with the pullback arm unchanged: TTL, one working
  order at a time, a fresh signal cancels, the guard check at fill time, the
  `no_room` refusal, the `LimitFills` counters, the fill-bar management under
  the engine's stop-before-target pessimism.

**If this registration needed a separate path for the null, it would be wrong.**
It does not: the field is on the rules.

### Why the gap rule flips between the two arms, stated so it can be checked

A resting *limit* never does better than its price, so a gap through it fills at
the level (n3's rule). A resting *stop* never does better than its price either
— and for a stop order "better" is the other direction, so a gap **through** it
fills at the open, which is **worse** than the level. Both arms are charged the
pessimistic side of the gap. Pinned by a test.

### The two stop/target arms, as in n3

* **ANCHORED** (`anchored`): the strategy's absolute stop and target stay where
  the signal bar put them. A breakout entry is **further** from its stop, so the
  risk unit **GROWS** and an absolute target moves **nearer in R**. That is a
  wider-stopped, nearer-targeted version of the mechanism — a different method,
  not a confirmation filter. Reported, but not the question.
* **TRANSLATED** (`carry`): the stop and an absolute target shift by the amount
  the entry moved, so the trade geometry is **identical** to the market arm's.
  The only differences left are **when the entry happened** and **which signals
  happened at all**. That is the brief's question — confirmation selection with
  geometry held constant — and the falsifier is read on **this** arm.

At offset 0.00 the two are identical, so TRANSLATED runs at 0.25 and 0.50 only,
matching n3's ledger exactly.

### Which way the errors point

**Against the breakout arm (declared):** the fill bar is managed immediately, so
a bar that printed its low before its high books a stop the sequence may not
have allowed — a **fake loss**, the mirror of n3's fake win. Also: one working
order at a time, signals dropped while in position, gaps charged at the open.
**In favour of it:** the touch of the level is a fill with no queue or slippage
model (a stop order in real life is filled *through* the level, not at it), and
for each pairing the **best** of the offsets in a family is read.

### Resolution of the fill verdict — declared

Read on **15m** bars. The *touch* verdict is exact (a 15m high is the max of its
1m highs). What 15m cannot settle is the **sequence inside the fill bar**:
whether the level was reached before or after the stop and the target were. For
a breakout long the stop sits **below** the order, so — unlike n3's pullback arm
— price **can** reach the stop without passing the order, and the engine's
whole-bar stop-before-target reading can book a loss that happened before the
entry existed. That is audited on 1m, descriptively, **never as a gate**.

n3 measured the size of this class on the pullback side and it is **entirely a
function of how small the mechanism's stop is**: at 2.7 ATR (`orb`) the 15m
verdict was **100.0%** right, at 0.3 ATR (`trend-pullback`) only **66.0%**, with
**24.8%** of fills fake wins. **Any row of mine whose mechanism stops inside
~0.7 ATR is not evidence**, I say so on the row, and the conclusion is restated
with those mechanisms dropped.

`XAUUSD-1m.parquet` holds 2026-06-02 to 2026-09-11 only and covers neither
window, so the 1m audit runs on `XAUDUKA-1m` (2010-06 to 2026-05) — a different
venue, so its rates are an estimate FOR the `xauusd` tape and not a measurement
OF it.

## 4. Falsifier — specific, and it can fire

For each pairing (mechanism x guards x window) take the **best** offset within
an arm family and compare **sum of net R over the window** — which already
charges every signal the arm never filled, because a non-fill contributes 0 —
against the **market arm** of the same mechanism, same guards, same window.

> **FIRES, and the "enter on a stop so the move is confirmed" direction is
> CLOSED, if the breakout arm fails to beat the market arm in at least 16 of the
> 24 pairings (2/3).**

Read on TRANSLATED (the question) and reported again on ANCHORED (a different
method). Threshold and form identical to n3's, so the two halves of the
resting-order family are comparable line for line.

If it does not fire, the report states the **breakeven fill rate**
`f* = E_market / E_breakout_per_fill` per pairing whose market arm makes money.

## 5. Multiple testing — declared before any run

| axis | values | n |
|---|---|---|
| mechanism (`config/n3-limit.toml`, default params, `weekdays`, no sweep) | orb, trend-pullback, vwap-fade, volume-thrust, pdhl, rsi-reversal-vol | 6 |
| entry arm | market; breakout 0.00 anchored; 0.25 anchored; 0.50 anchored; 0.25 translated; 0.50 translated | 6 |
| guards | on, off | 2 |
| window | A 2025-07-01 to 2025-10-01, B 2025-04-01 to 2025-07-01 | 2 |

**Declared: 144 gate rows across 24 `search.exe` invocations**, market `xauusd`
15m, `--mode=hypotheses --fixed --exit-mix --seeds=200 --null-sides=coin`. The
same windows, mechanisms, offsets and TTL (4 bars, FIXED, not swept) as n3, so
the pullback and breakout halves are read against one another without a second
set of market rows.

Not cells, and declared as such: the engine parity and unit tests, and the 1m
intrabar-sequence audit (descriptive, 5 stop sizes x 3 offsets x 2 sides = 30).

### `news-pulse`: declared NOT RUN, and the reason is a premise of the brief failing

The brief asks for a `news-pulse` arm "if it can be measured". **Measured
2026-10-07, before registering: it cannot, from here.** `news-pulse`,
`Filter::NewsOnly` and `news::window_in` live on `agent/n1` and **do not exist in
this worktree** (`grep -rn news-pulse crates/` = 0 hits), while the shared binary
`/e/rust/fd-n1bin/target/release/search.exe`, which has them, **silently ignores
`--limit=`** — it printed no `entry:` line and raised no error for
`--limit=0.25,4,carry`. Stop-entry x news-pulse therefore needs the two branches
merged, which is a different job. Brief section 0 says stop rather than
improvise, and the brief itself says to prefer the ordinary mechanism so as not
to collide with the three news jobs running in parallel. **Zero cells spent on
it.**

## 6. How it is read

* Primary: the falsifier of section 4, sum net R, paired per mechanism,
  TRANSLATED.
* Secondary: the gate — `PF >= 1.200 AND expectancy >= +0.050R AND >= 40
  trades`, on BOTH windows.
* **Fill rate, and the null's fill rate beside it**, on every row, with n3's
  exact statistic reproduced for this arm: median and mean of *method rate minus
  null rate*, and the count of rows where the method filled LESS than its null.
  If that difference is ~0, the row is reported as saying nothing about whether
  price runs.
* **`null p50` beside every percentile**, and the count of rows with >= 40
  trades sitting BELOW their own null's median PF — n3 found 36 of them,
  including its biggest wins.
* `--exit-mix` on every run; a row whose own rule never fires is reported as
  that and not as a result (brief section 4.iii).
* Any row with `wrong_side_stop != 0` is unreadable and is reported, not fixed
  (brief section 4.vi).
* Rows whose mechanism's realised stop is under ~0.7 ATR are marked
  resolution-unsafe and the conclusion is restated without them.
* Under 40 trades: no conclusion, counted in the ledger anyway.

## 7. Signed

Written by the stop-entry agent (Claude Opus 5, 1M context) on
`agent/stop-entry`, 2026-10-07, before `RestSide` existed in any file.
