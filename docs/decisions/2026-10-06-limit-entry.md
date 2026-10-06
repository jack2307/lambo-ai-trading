# 2026-10-06-limit-entry: resting (limit) entry instead of market-at-next-open

**Registered:** 2026-10-06, before the first line of engine code. Branch
`agent/n3`, worktree `/e/rust/fd-b3`. Brief: `AGENT-BRIEF-2026-10-06-B.md`
(job B3).

## 0. The family, in one sentence

The engine fills every signal at the OPEN of the next bar and charges half the
spread on that fill (`engine.rs:apply_costs`, `entering = true`). This family
replaces that with a **resting limit order on the pullback side of the signal
bar's close**: the order fills only if price comes back to it, it fills at the
price it asked for, and it **earns** half the spread instead of paying it.

## 1. Hypothesis

For a given mechanism, routing the same signals through a resting limit order
raises net R over the window, because the spread term it reverses is larger
than the gate's whole required edge: this repo's own cost table puts gold at
**3.00-11.57% of R, median 7.44%**, at the stops the strategies actually use
(1.0-3.0 ATR), against a gate that asks +0.050R = **5.00% of R**.

## 2. The prior, and it is AGAINST the hypothesis

Written here first so a negative result is not read as a discovery and a
positive one is read as a surprise. The brief says this control is untouched;
**it is not** - three measurements already on this desk bear on it:

1. `docs/decisions/2026-09-16-limit-entry-run.txt` (`scripts/limit_entry.py`):
   XAUUSD 15m against 1m, 6,677 bars, every bar treated as a signal, 24 cells
   per side. `limit - market` in R/signal was **negative in 21 of 24 cells**
   for LONG and **24 of 24** for SHORT. Two defects that matter here: that
   script charges **no spread to either arm**, so the term this registration
   exists to measure is absent from it; and it exits at a fixed 8-bar close,
   so it has no stop and no target.
2. `py/live/mt5_executor.py` (`--max-join-r`): 81,789 observations, 15m gold
   2022-06 to 2026-09. After adverse entry drift +0.0926R, inside the band
   +0.0992R, after favourable +0.0938R - equal within 0.007R. **A better
   entry is worth its distance and nothing more.**
3. Same file, `--allow-favourable-join`: of 8 trades a favourable join added,
   7 were later stopped out against a 38% base rate (p = 0.006, n = 8).
   A limit entry is a favourable-drift join by construction.

What is NOT measured anywhere, and is this job: limit entry **conditional on a
real mechanism's signals, inside the backtest engine, with the spread term
reversed, against a matched null that also enters by limit**, read at the gate.

## 3. What the engine does today, and the change

`run_backtest_guarded` holds one `pending: Option<Intent>` and consumes it at
step 1 of the next bar at `bar.open`. There is no working-order concept and
`Intent::Enter` has no price field. The change is at ONE point in the loop:

* `TradingRules` gains `limit_entry: Option<LimitEntry { offset_atr, ttl_bars }>`,
  `serde(default)` so every existing config and receipt is untouched.
* With it `None`, the loop is today's loop and every published number stands -
  pinned by a parity test, not asserted.
* With it `Some`, an `Intent::Enter` from signal bar `s` becomes a working
  order at `L = close_s -/+ offset_atr * ATR_s` (minus for a long), live for
  bars `s+1` through `s+ttl_bars`.
* **It lives in the RULES, not in a strategy.** That is the point: the matched
  null (`control::RandomEntry`) emits `Intent::Enter` too, so it is routed
  through the identical order, the identical price rule and the identical fill
  selection, with no null-specific code. Section 9 of the brief is satisfied
  by construction rather than by claim.

### Fill rule, stated so it can be checked

* long fills iff `bar.low <= L`; short iff `bar.high >= L`.
* fill price `min(L, bar.open)` for a long, `max(L, bar.open)` for a short -
  an order cannot fill worse than a market that opened through it.
* the fill **earns** half the spread (`fill - spread/2` for a long). The exit
  still pays half, unchanged.
* ATR, the stop and the target are the ones fixed at the SIGNAL bar. All six
  mechanisms here emit an absolute stop, so the stop level does not move with
  the better entry: risk per unit shrinks, and that is the whole edge.
* the fill bar is managed immediately, with the engine's existing
  stop-before-target pessimism.
* no fill by `s+ttl_bars` means cancelled, counted, **no trade**.
* a new signal replaces a working order; a signal while a position is open is
  dropped - both as `pending` behaves today, both counted.

### Resolution of the fill verdict - declared

The verdict is read on **15m bars**. The *touch* verdict is exact (a 15m low
is the minimum of its 1m lows); what 15m cannot resolve is the **sequence**
inside the fill bar - whether the limit filled before the stop level was
reached. That is audited separately on 1m, descriptively, never as a gate.

`XAUUSD-1m.parquet` covers **2026-06-02 to 2026-09-11 only** (100,000 rows)
and therefore does **not** cover either window of this job. The 1m audit runs
on `XAUDUKA-1m.parquet` (2010-06-01 to 2026-05-31), which does.

### Which way the errors point

In favour of the limit arm (optimism, declared): queue priority is not
modelled - a touch of the level is a fill; and for each pairing the **best**
of three offsets is read. Against it: stop-first ordering on the fill bar,
one working order at a time, signals dropped while in position.

## 4. Falsifier - specific, and it can fire

For each pairing (mechanism x guards x window) take the **best** of the three
limit offsets and compare **sum of net R over the window** against the market
arm of the same mechanism, same guards, same window.

> **FIRES, and the direction is closed, if the limit arm fails to beat the
> market arm in at least 16 of the 24 pairings (2/3).**

If it does not fire, the report must state the **breakeven fill rate**: the
fill rate at which the limit arm's R per signal equals the market arm's,
`f* = E_market / E_limit_per_fill`, per surviving pairing.

Every row also carries the gate - `PF >= 1.200 AND expectancy >= +0.050R AND
>= 40 trades` - on BOTH windows, and the fill rate of the method beside the
median fill rate of its null. A PF on a 30%-fill subset is reported as a
subset, never as the mechanism.

## 5. Multiple testing - declared before any run

| axis | values | n |
|---|---|---|
| mechanism (default params, `weekdays` filter, no sweep) | orb, trend-pullback, vwap-fade, volume-thrust, pdhl, rsi-reversal-vol | 6 |
| entry arm | market; limit 0.00 ATR; limit 0.25 ATR; limit 0.50 ATR (ttl = 4 bars, FIXED) | 4 |
| guards | on, off | 2 |
| window | A 2025-07-01 to 2025-10-01, B 2025-04-01 to 2025-07-01 | 2 |

**Declared: 96 gate rows across 16 `search.exe` invocations**, market `xauusd`
15m, `--mode=hypotheses --fixed --exit-mix`, `--seeds=200` (the brief's 7.3:
`--samples=` is not read in this mode), `--null-sides=coin`.

Not cells, and declared as such: the engine parity tests, and the 1m
intrabar-ordering audit (descriptive).

`ttl_bars` is **not** swept. `offset_atr` has exactly the three values above.
Any value outside this table that gets run is written into an amendment at the
foot of this file with its date, and counted in the ledger.

## 6. How it is read

* Primary: the falsifier of section 4, on sum net R, paired per mechanism.
* Secondary: the gate of section 5, both windows.
* Percentiles are printed with `null p50` beside them (brief section 5) and
  only for rows whose null ran through the same limit order.
* `--exit-mix` on every run, and a row whose own exit rule never fires is
  reported as that and not as a result (brief section 8).
* Under 40 trades: no conclusion, counted in the ledger anyway.

## 7. Signed

Written by agent n3 (Claude Opus 5, 1M context) on `agent/n3`, 2026-10-06,
before `limit_entry` existed in any file.


## Amendment 2026-10-06 (1) - the first run was INVALID, and what it was invalid for

Written after reading the first 16 receipts and BEFORE any corrected run. The
first 16 receipts are kept on the branch as `n3-limit-*` under
`receipts/invalid-first-run/` and none of their numbers may be quoted.

**The defect.** Section 3 above says "all six mechanisms here emit an absolute
stop, so the stop level does not move with the better entry: risk per unit
shrinks, and that is the whole edge." That sentence is true and the
implementation of it was wrong in a way the registration did not anticipate.
`trend-pullback` stops at a recent swing low, which on 15m gold is often a
fraction of an ATR from the close - its realised median stop over window A is
**0.378 ATR = 1.35 points**. An order resting 0.50 ATR (about 1.8 points)
below that close therefore lands **below its own stop**. `open_position`
computes `risk = (entry - stop).abs()`, which is still positive, so the
position opens with its "stop" ABOVE a long entry - and `check_exit` books
hitting it as an exit at a **profit**.

The row that exposed it, window A, no guards, offset 0.50:

| | market arm | limit arm |
|---|---|---|
| trades | 225 | 147 |
| profit factor | 0.544 | **1.512** |
| expectancy | -0.341R | **+0.287R** |
| exits | STOP 147 / TARGET 77 | **STOP 139** / TARGET 7 |

139 stop-outs and 7 targets cannot make +42R. Every one of those stops was a
fabricated win. The same arithmetic inflated `pdhl` in the other direction
(expectancy -0.858R at offset 0.25) and produced matched-null profit factors
of **46.7, 94.1 and 119.0**, because the null is routed through the same order
and collected the same free money.

**The fix, and it is a refusal and not a correction.** An order that would rest
beyond its own stop or its own target is **not placed**: it is not an order a
desk can place. Counted as `LimitFills::no_room` and printed on the row.
Pinned by `tests/limit_entry.rs::an_order_beyond_its_own_stop_is_refused_and_not_filled_at_a_profit`.

**A PRE-EXISTING defect found on the way, and NOT fixed.** The same arithmetic
can bite the market arm: a long whose fill bar GAPS below the strategy's
absolute stop opens with that stop above its entry and can only end in profit
at it. That is a defect of the next-open fill model, it predates this work, and
correcting it would restate every published receipt and the oracle parity files
with them. It is **counted** instead - `BacktestResult::wrong_side_stop`,
printed on any row that carries one - and left for the owner to decide.

## Amendment 2026-10-06 (2) - a second arm, because the registered one does not measure the spread

Written at the same moment, before the corrected run.

Section 3 is right that an anchored absolute stop makes the risk unit shrink
with a better entry. What it did not say is that **an arm which shrinks the
risk unit is not measuring the spread at all.** `trend-pullback` at 0.50 ATR of
offset carries a realised median stop of 0.284 ATR where its market arm carries
0.378 ATR, and with `reward_risk` scaling the target off that risk, the target
moves nearer in price too. That arm converts the mechanism into a
tighter-stopped, nearer-targeted version of itself. Whatever it measures, it is
not "ăn spread".

So `LimitEntry` gains `carry_stop` and the measurement gains an arm:

* **ANCHORED** (`--limit=<off>,<ttl>,anchored`) - the registered arm. The stop
  and target stay where the signal bar put them. Risk shrinks.
* **TRANSLATED** (`--limit=<off>,<ttl>,carry`) - the stop and an absolute
  target are shifted by the same amount the entry moved. The trade's geometry
  is the one the signal designed, and the only differences left from the market
  arm are **the sign of the half spread** and **the trades that never filled**.
  That is the brief's question, and the falsifier of section 4 is read on THIS
  arm. (At offset 0.00 the two arms are identical, so TRANSLATED is run at
  0.25 and 0.50 only.)

**Revised multiple-testing ledger.** Arms become six: market; anchored 0.00,
0.25, 0.50; translated 0.25, 0.50. With 6 mechanisms x 2 guard arms x 2
windows that is **144 gate rows across 24 invocations**, replacing the 96/16
declared above. The 96 rows of the invalid first run are counted in the ledger
as rows looked at, because they were.

The falsifier threshold is unchanged in form and is now read twice: once on
the TRANSLATED arm (the registration's real question) and once on the ANCHORED
arm (reported, but as a different method and not as a cheaper entry).


## RESULT 2026-10-06 - the falsifier did not fire, and nothing came through the gate

144 rows, 24 invocations, `xauusd` 15m, both windows, `--fixed --exit-mix
--seeds=200 --null-sides=coin`, receipts `receipts/n3-limit-*.txt`.

### 1. The gate: ZERO

**0 of 144 rows clear PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades
on BOTH windows.** Sixth programme, still none. The family does not open
anything.

### 2. The falsifier: did NOT fire, on either arm

Best offset in the family per pairing, on sum net R, against the market arm:

| arm family | limit beat market | failed to beat | fires at |
|---|---|---|---|
| TRANSLATED (the registration's question) | 19 of 24 | 5 | 16 |
| ANCHORED (a tighter method) | 22 of 24 | 2 | 16 |

So the spread term is real and recoverable. The desk may NOT close the "enter
on a limit to pay less" direction on this evidence.

### 3. The one number worth keeping, and it is the opposite of the headline

**Of the 24 pairings, the market arm made money in 4. Only 2 of those have
>= 40 trades - `orb`, window A, guarded and unguarded. The limit arm made LESS
in both:**

| pairing | trades | market sumR | best TRANSLATED | best ANCHORED |
|---|---|---|---|---|
| orb / A / guards | 44 | **+3.90** | +1.91 (0.50) | +5.08 (0.00) |
| orb / A / noguards | 45 | **+4.43** | +2.39 (0.25) | +5.64 (0.00) |
| rsi-rev / A / guards | 9 | +0.07 | +0.53 | +0.45 |
| rsi-rev / A / noguards | 9 | +0.07 | +0.53 | +0.45 |

And the **breakeven fill rate** on those two says it cannot be recovered at
all: `f* = E_market / E_limit` is **138% to 387%** against an actual fill rate
of **67-80%**. A figure above 100% means no fill rate closes the gap - the
limit arm is worse per fill as well as filling less often.

So the 19-of-24 tally is almost entirely pairings where the market arm LOSES
money and the limit arm loses less. "Loses less" is not "makes money", and it
is the same trap section 5 names for percentiles, restated for differences.

### 4. A resting order's selection is the UNCONDITIONAL base rate

Because the order lives in the rules, the matched null rests its orders under
the same price rule - so the fill rate has a null to be read against. Over 105
arm-rows:

**method fill rate minus its null's: median +0.40 points of rate, mean +1.25,
and the method filled LESS than its null in 35 of 105 rows.**

A mechanism whose signals led moves that do not come back would fill well
below its null. None of these six does, to within a point of rate. **The
signals carry no information about whether price comes back**, which means the
adverse selection a limit order imposes is neither worse nor better than
random - and it also means the mechanisms have no edge in the one dimension a
resting order is sensitive to.

### 5. A measured reason to disbelieve the biggest numbers in my own table

The 1m audit (`XAUDUKA`, window A, 8,518 bars of 15m against 127,739 of 1m)
asks the only question 15m OHLC cannot settle: inside the fill bar, did the
order fill BEFORE the target level was reached, or did the bar print that high
first and `check_exit` credit a target the sequence never allowed?

Agreement between the 15m verdict and the 1m sequence, by the stop the
mechanism actually uses:

| stop | which mechanism | offset 0.25 | offset 0.50 |
|---|---|---|---|
| 0.3 ATR | `trend-pullback` (0.378) | 85.3% agree, **9.1% fake wins** | 66.0% agree, **24.8% fake wins** |
| 0.7 ATR | `pdhl` (0.714) | 99.1%, 0.8% | 97.9%, 1.9% |
| 1.0 ATR | `vwap-fade` (1.000) | 99.9%, 0.1% | 99.5%, 0.5% |
| 1.2 ATR | the config default | 99.9%, 0.1% | 99.8%, 0.1% |
| 2.7 ATR | `orb` (2.681) | 100.0%, 0.0% | 100.0%, 0.0% |

`trend-pullback` produced the largest limit-arm gains in the whole table
(+60.67R and +56.21R on window B at offset 0.50). Its stop is 1.35 points,
under five spreads, and **a quarter of its fill-bar outcomes at that offset are
look-ahead artifacts of the bar resolution.** Those rows are not evidence.
Dropping `trend-pullback` entirely leaves 20 pairings, 15 beats and 5 failures
- the falsifier still does not fire, so the conclusion does not rest on them.

### 6. Where the gain comes from, and it is exactly the arithmetic

Cost as a fraction of R on the market arm, window A, with the stop beside it
(brief section 7.1):

| mechanism | realised stop | cost of R |
|---|---|---|
| `rsi-reversal-vol` | 3.310 ATR = 16.14 pts | 1.73% |
| `volume-thrust` | 4.210 ATR = 14.08 pts | 1.99% |
| `orb` | 2.681 ATR = 12.59 pts | 2.22% |
| `vwap-fade` | 1.000 ATR = 3.89 pts | 7.21% |
| `pdhl` | 0.714 ATR = 3.12 pts | 8.97% |
| `trend-pullback` | 0.378 ATR = 1.35 pts | 20.74% |

The limit arm's gain tracks this column. It is largest exactly where the
spread is a large fraction of R, which is what reversing the sign of the
spread should do - and the two mechanisms at the top of that column are the two
whose bar resolution cannot support the measurement. The term is real; it is
not where the missing edge is.

### 7. The null often beats these rows

36 rows with >= 40 trades sit BELOW their own matched null's median profit
factor, the biggest limit-arm wins among them: `trend-pullback` offset 0.50,
window B, PF **1.505** against **null p50 1.543**; window A, PF 1.052 against
null p50 1.790. The resting order helps a random entry at least as much as it
helps a method, which is section 9 of the brief measured rather than asserted.

### 8. What was NOT measured

* **One instrument, one timeframe.** `xauusd` 15m only. The spread cost of R
  is a horizon quantity (brief 7.1), so a 5m or 1h run is a different answer
  and was not made.
* **Queue priority.** Price touching the level is a fill. Every limit row is
  flattered by an unknown amount, and the direction is known.
* **The signal set is the market arm's.** A limit arm's non-fills leave the
  book free earlier, so it could see signals the market arm's open position
  suppressed. Not modelled; one working order at a time, dropped while in
  position.
* **Partial fills, slippage on the resting order, and order size against
  book depth.** None of it.
* **`XAUUSD-1m` does not cover either window** - it holds 2026-06-02 to
  2026-09-11 and 100,000 rows. The audit ran on `XAUDUKA-1m`, which is a
  different venue from the `XAUUSD` tape the 144 rows were measured on. The
  artifact rates of section 5 are therefore an estimate FOR this tape, not a
  measurement OF it.
* **A stop-entry arm** (resting on the breakout side rather than the pullback
  side) is the obvious mirror of this family and was not run.

### 9. Multiple-testing ledger

| | declared | looked at |
|---|---|---|
| gate rows | 96, amended to 144 | **246** |
| invocations | 16, amended to 24 | **41** |

The 246 is 144 corrected rows + 96 rows of the invalid first run + 6 rows of a
single 20-seed smoke run used to check the receipt format. The 41 is 24 + 16 +
1. Nothing outside the declared offsets, TTL or windows was run. The 1m audit
(5 stop sizes x 3 offsets x 2 sides = 30 descriptive cells) carries no gate and
is counted separately.

### 10. Verdict

The mechanism works, the arithmetic is the arithmetic, and it buys nothing a
gate can read. **The direction "enter on a limit so the spread is cheaper"
stays OPEN on this evidence** - the falsifier did not fire - but the only
pairing in the set that had a profitable market arm at a readable sample size
said the limit arm is worse at any fill rate, and the one thing the family
measured cleanly is that these six mechanisms' signals say nothing about
whether price comes back.
