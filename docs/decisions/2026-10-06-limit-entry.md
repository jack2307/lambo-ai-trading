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
