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

## Amendment 2026-10-07 (1) — two implementation facts the registration did not anticipate

Written after the first smoke run (one month, `--seeds=20`, three arms) and
BEFORE the declared 24 invocations. Nothing in section 5 changes: same arms,
same windows, same 144 rows.

**(a) TRANSLATED must shift from the FILL, not from the level.** Section 3 says
the stop and target "shift by the amount the entry moved". On the pullback side
the fill IS the level, so the two readings are the same number and n3 never had
to choose. On the breakout side they are not: **a gap through a stop order fills
at the open, past the level.** Shifting by the level would then leave the trade
with a risk unit nobody designed — the opposite of what this arm exists to hold
constant. `Working::geometry` therefore carries the stop and target to the level
at placement (so the `no_room` check has something to read) and adds the
remaining distance at the fill. On `Pullback` the remainder is exactly zero, so
every 2026-10-06 number is unmoved.

**(b) `check_exit` prices a gapped stop at `bar.open`, which on a resting
order's fill bar is a price that PRECEDED the entry.** Measured, counted, and
**not corrected**.

The smoke run read `trend-pullback`, breakout 0.50 ATR, TRANSLATED, July 2025:
**expectancy −1.573R over 44 trades.** An average loss over one and a half risk
units cannot come from a stop; it comes from exits booked *beyond* the stop. The
engine's gap branch is `if bar.open <= stop { exit at bar.open }`, which is
correct for a position it has held since the previous bar and wrong for one a
resting order opened *inside* this bar: a breakout long fills at or above its
bar's open, so an adverse bar books its exit at a price from before the fill.
With `trend-pullback`'s 1.35-point risk unit, a two-point open is −1.5R by
itself.

The same artifact exists in the **pullback** arm — a limit long filling at its
level on a bar that opened below its stop — and was not counted there. Fixing it
would restate every 2026-10-06 receipt, so, exactly as brief section 4.vi
instructs for `wrong_side_stop`, it is **counted and printed**:
`LimitFills::exit_priced_before_the_fill`, on every row that carries one, with
its share of that row's fills. A row where the share is large is not a row to
read, and the result section says so row by row.

This is the sixth member of the class brief section 4.iv names, with a new
shape: not a flag a mode ignores, but **an exit price that is right for one
fill model and wrong for another**, silently shared between the two.

## Amendment 2026-10-07 (2) — a flag the shared binary ignores without error

The premise check of section 5 is worth stating as a defect rather than only as
a reason: `/e/rust/fd-n1bin/target/release/search.exe --limit=0.25,4,carry`
**runs, prints no `entry:` line, and exits 0.** That binary predates the flag, so
the argument is simply never read. Any job that reaches for the shared binary
with a flag introduced on another branch gets a silently market-entry run with a
resting-entry filename. Brief section 4.iv's rule — change the value and see
whether the number changes — caught it; nothing in the output would have.

## RESULT 2026-10-07 — the falsifier did not fire, nothing came through the gate, and the confirmation a stop order buys is the UNCONDITIONAL base rate

144 rows, 24 invocations, `xauusd` 15m, both windows, `--fixed --exit-mix
--seeds=200 --null-sides=coin`, receipts `receipts/stopentry-*.txt`, table
`receipts/stopentry-SUMMARY.txt`, 1m audit
`receipts/stopentry-1m-fillbar-audit.txt`.

### 1. The gate: ZERO

**0 of 144 rows clear PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on
BOTH windows.** Seventh programme, still none. `wrong_side_stop` is **0 on all
144 rows**, so nothing here is unreadable for that reason.

The closest row in the family, and it is the whole story in four lines:
`orb`, unguarded, TRANSLATED 0.25 ATR — window A **40 trades, PF 1.436,
expectancy +0.149R, "gate pass, inside the noise", percentile 83, null p50
0.939, null p95 1.917**; window B **35 trades, PF 1.006, expectancy +0.022R**.
One window, and PF 1.436 sits under its own null's 95th percentile of 1.917.

### 2. The falsifier: did NOT fire, on either arm

Best offset in the family per pairing, sum net R, against the market arm:

| arm family | breakout beat market | failed to beat | fires at |
|---|---|---|---|
| TRANSLATED (the registration's question) | **12 of 24** | 12 | 16 |
| TRANSLATED, resolution-unsafe mechanism dropped | **12 of 20** | 8 | 14 |
| ANCHORED (a WIDER-stopped method) | 20 of 24 | 4 | 16 |
| ANCHORED, resolution-unsafe dropped | 16 of 20 | 4 | 14 |

So the desk may NOT close the "enter on a stop so the move is confirmed"
direction on this evidence either, and the two halves of the resting-order
family stay open together. The threshold was n3's, unchanged, and it was not met
on either side.

**But 12-of-24 is not a coin either.** Of the 12 pairings the breakout arm beat,
**11 have a market arm that LOSES money** and the breakout arm loses less — the
trap n3's record names for percentiles, restated for differences. Only `orb`
window A has a market arm that makes money at a readable sample, and the
breakout arm beat it there: +5.97R against +4.43R unguarded, +5.36R against
+3.90R guarded, both at TRANSLATED 0.25 ATR.

### 3. THE NUMBER WORTH KEEPING: a stop order's confirmation is the base rate

Method fill rate minus its matched null's, over the 120 arm-rows that printed
both:

**median −0.25 points of rate, mean −0.23, and the method filled LESS than its
null in 62 of 120 rows.**

Set beside n3's pullback half — **median +0.40, less than its null in 35 of
105** — this is the finding of the night, and it closes something the falsifier
could not:

> A resting order on the **pullback** side fills no more often than a coin flip
> does. A resting order on the **breakout** side fills no more often than a coin
> flip does either, and if anything slightly less. **These six mechanisms'
> signals carry no information about whether price comes back AND no information
> about whether price runs.** Two opposite selection mechanisms, each measured
> against its own null on the same tape, and both read the unconditional base
> rate.

That is a statement about the mechanisms, not about the orders, and it is why
neither half of the family can be the missing edge. Per arm:

| arm | rows | median diff | mean diff | worst |
|---|---|---|---|---|
| breakout 0.00 anchored | 24 | +0.00 | −1.31 | −28.60 |
| breakout 0.25 anchored | 24 | −1.55 | −2.06 | −32.40 |
| breakout 0.50 anchored | 24 | +1.35 | +2.00 | −16.70 |
| breakout 0.25 translated | 24 | −1.75 | −2.04 | −32.40 |
| breakout 0.50 translated | 24 | +2.15 | +2.25 | −16.70 |

And on the one mechanism with a readable market arm the sign **flips between
adjacent windows**, which is brief section 3.2 arriving in a new place: `orb`
TRANSLATED 0.25, fill rate **88.9% vs the null's 83.3%** on window A and
**70.0% vs 81.5%** on window B.

### 4. The cost of confirmation is the OFFSET, and it is 3.6x to 7.3x the spread

n3's half of the family found the spread term real and recoverable. This half
pays that term and adds a second one that is much larger. `offset / stop`, with
each mechanism's realised market-arm stop and the spread cost of R the receipts
print:

| mechanism | realised stop | spread cost of R | 0.25 ATR offset | 0.50 ATR offset |
|---|---|---|---|---|
| `volume-thrust` | 4.210 ATR = 14.08 pts | 1.99% | 5.9% | 11.9% |
| `rsi-reversal-vol` | 3.310 ATR = 16.14 pts | 1.73% | 7.6% | 15.1% |
| `orb` | 2.681 ATR = 12.59 pts | 2.22% | 9.3% | 18.6% |
| `vwap-fade` | 1.000 ATR = 3.89 pts | 7.21% | 25.0% | 50.0% |
| `pdhl` | 0.714 ATR = 3.12 pts | 8.97% | 35.0% | 70.0% |
| `trend-pullback` | 0.378 ATR = 1.35 pts | 20.74% | 66.1% | 132.3% |

**Median spread cost 4.71% of R; median 0.25 ATR offset 17.2% of R (3.6x);
median 0.50 ATR offset 34.3% of R (7.3x).** Brief section 3.3 again with a new
numerator: the offset is a horizon quantity exactly as the spread is,
`offset / stop`, and at `trend-pullback`'s stop a 0.50 ATR order costs **more
than the whole risk unit** before the trade begins.

### 5. The breakeven fill rate, where the market arm makes money

`f* = E_market / E_breakout`. Only `orb` window A qualifies at a readable sample
(44 and 45 market-arm trades):

| pairing | arm | E_market | E_breakout | f* | actual fill rate |
|---|---|---|---|---|---|
| orb / A / guards | t025 | +0.0890R | +0.1380R | **64.5%** | 86.7% |
| orb / A / guards | t050 | +0.0890R | +0.1060R | 84.0% | 80.0% |
| orb / A / noguards | t025 | +0.0980R | +0.1490R | **65.8%** | 88.9% |
| orb / A / noguards | t050 | +0.0980R | +0.1220R | 80.3% | 82.2% |

This is the one place the two halves of the family genuinely differ. n3's limit
arm on the same pairing needed **138% to 387%** — above 100%, so no fill rate
could close its gap. The breakout arm needs **64.5–65.8%** at 0.25 ATR and fills
**86.7–88.9%**: it clears its own breakeven. It does so on **one window**, at a
sample of 40 trades, with a profit factor under its own null's 95th percentile,
and the same arm reads +0.022R on the window next to it. Clearing a breakeven is
not clearing the gate.

(`rsi-reversal-vol` window A also has a positive market arm, at **9 trades**: no
conclusion, counted.)

### 6. The resolution audit — the error is a FAKE LOSS, the mirror of n3's

1m audit, `XAUDUKA`, window A, 8,518 bars of 15m against 127,739 of 1m, breakout
side. Agreement between the 15m fill-bar verdict and the 1m sequence, by the stop
the mechanism actually uses, LONG side:

| stop | which mechanism | offset 0.25 | offset 0.50 |
|---|---|---|---|
| 0.3 ATR | `trend-pullback` (0.378) | 73.4% agree, **26.6% FAKE LOSS** | 67.7%, **32.3%** |
| 0.7 ATR | `pdhl` (0.714) | 91.5%, 8.5% | 67.3%, **32.7%** |
| 1.0 ATR | `vwap-fade` (1.000) | 97.8%, 2.2% | 90.6%, 9.4% |
| 1.2 ATR | the config default | 99.0%, 1.0% | 95.9%, 4.1% |
| 2.7 ATR | `orb` (2.681) | 99.9%, 0.1% | 100.0%, 0.0% |

**Fake WINS are 0.0% in every cell**, on both sides, at every stop — the exact
mirror of n3's pullback arm, where the fake wins ran to 24.8% and the fake-loss
column was the empty one. A long limit rests above its stop, so the stop cannot
be reached without passing the order; a long stop order rests above a stop the
bar can print **first**. Same artifact of 15m resolution, sign flipped with the
order.

**The engine's own count agrees, row by row.** 42 of the 144 rows carry
`exit_priced_before_the_fill > 0`, concentrated exactly where the audit says:

| arm | rows with any | median share of fills | worst |
|---|---|---|---|
| market, breakout 0.00 | 0 | 0% | 0% |
| breakout 0.25 / 0.50 anchored | 7 each | 0% | 6% |
| breakout 0.25 translated | 12 | 0.5% | **44%** |
| breakout 0.50 translated | 12 | 1.0% | **70%** |

Every one of the eight worst rows is `trend-pullback`, at 37–70% of its fills.
**Its TRANSLATED rows (−182R to −318R of sum net R, expectancy down to
−2.487R) are not evidence of anything**, and they are also the only reason the
TRANSLATED tally is 12-of-24 rather than 12-of-20. Dropping the mechanism
entirely leaves 20 pairings, **12 beats and 8 failures** — the falsifier still
does not fire, so the conclusion does not rest on them. That is the check n3 ran
and the answer is the same.

### 7. The null often beats these rows

**41 of the 73 rows with >= 40 trades sit BELOW their own matched null's median
profit factor** (n3: 36). The market arm is among them — `orb` window B, PF
0.760 against null p50 0.987 — so this is not something the resting order
introduced. The gap is widest on the arms that refused the most orders:
`trend-pullback` ANCHORED 0.50, PF 0.634 against null p50 **1.054**, on 47
trades with **120 orders refused for no room**.

### 8. What was NOT measured

* **`news-pulse`, and therefore the one direction the prior was FOR.** Section 5
  declares why and it stands: the strategy is on `agent/n1` and not in this
  worktree, and the `agent/n1` binary that has it silently ignores `--limit=`.
  This is the gap in the night's work; it is a merge, not a measurement, and
  **no cell was spent pretending otherwise.** The hypothesis of section 1 was
  built on n1's continuation rule, and the arm that would test a stop entry on a
  continuation signal was not run.
* **One instrument, one timeframe.** `xauusd` 15m. The offset cost of R is a
  horizon quantity exactly as the spread is, so 5m or 1h is a different answer.
* **Slippage through the level.** A real stop order is filled THROUGH its level,
  not at it; here a touch fills at the level and a gap fills at the open and
  nothing worse. Every breakout row is flattered by an unknown amount and the
  direction is known.
* **The signal set is still the market arm's.** One working order at a time,
  signals dropped while in position.
* **`XAUUSD-1m` covers neither window** (2026-06-02 to 2026-09-11). The audit
  ran on `XAUDUKA-1m`, a different venue, so section 6's rates are an estimate
  FOR this tape rather than a measurement OF it.
* **TTL** is 4 bars throughout and was not swept, as declared.

### 9. Multiple-testing ledger

| | declared | looked at |
|---|---|---|
| gate rows | 144 | **168** |
| invocations | 24 | **28** |

168 = 144 declared + 18 from a three-arm one-month smoke run at `--seeds=20`
(which is what found amendment (1)(b)) + 6 from the 10-day probe of the
`agent/n1` binary that established it ignores `--limit=`. 28 = 24 + 3 + 1.
Nothing outside the declared offsets, TTL, arms or windows was run. The 1m audit
(5 stop sizes x 3 offsets x 2 sides = 30 descriptive cells) carries no gate and
is counted apart.

### 10. Verdict

The mirror is built, it is one field on one struct, the null goes through it
untouched, and it buys nothing a gate can read. **Both directions of the
resting-order family stay OPEN on the registered falsifier** — neither fired —
but the family has now been measured from both sides, and the two halves agree
on something that matters more than either falsifier:

**the fill rate of a resting order on these mechanisms' signals is the fill rate
of a coin flip, whichever side of the close it rests on.** A pullback order
keeps the signals that retraced; a breakout order keeps the signals that ran;
neither subset is one the signals knew anything about. Until a mechanism exists
whose fill rate under one of these orders departs from its null's by more than a
point of rate, there is nothing for either order to select.
