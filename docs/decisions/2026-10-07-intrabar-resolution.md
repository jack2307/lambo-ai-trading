# 2026-10-07-intrabar-resolution: a resolution map for the 15m adjudication

**Registered:** 2026-10-07, before the first line of measurement code. Branch
`agent/intrabar`, worktree `/e/rust/fd-intrabar` (cut from `agent/crt`).
Brief: `AGENT-BRIEF-2026-10-07-NIGHT.md`. `df -h /e` before: **24 GB free**
(floor 8 GB).

## 0. THIS AXIS HAS NO GATE

Nothing here is a candidate, nothing is swept for an edge, and no row of this
work can pass or fail `PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades`.
It is a **measurement of the measuring instrument**. Every cell below is
descriptive and is counted as descriptive in the ledger.

**Report, do not fix.** The engine's both-touched rule is not changed, not
flagged behind a config key, not wrapped. Changing it would restate every
published receipt and the golden parity files with them — the same reason
`wrong_side_stop` is counted and left to the owner (brief §4(vi)).

## 1. The question, in one sentence

`engine.rs::check_exit` takes the **stop** whenever one bar's range covers both
the stop and the target, because OHLC cannot say which came first; so: **at
each stop size the desk's mechanisms actually use, what fraction of the 15m
verdicts does the 1m sequence overturn, and in which direction?**

## 2. Hypothesis, and it is a claim about DIRECTION, not size

For a **market-at-next-open** entry — which is every published row in this
record — the engine's stop-first rule can only ever **understate** a result,
never flatter it. The engine books a TARGET only on a bar that did **not**
touch the stop, and a 15m high is the maximum of its constituent 1m highs, so
a 15m TARGET verdict cannot be contradicted by the 1m sequence. The error is
therefore one-sided: **fake losses only, no fake wins.**

n3's 24.8% "fake win" figure is a property of a **resting (limit) entry**,
where the fill itself is an intrabar event and the target can be reached
before the order exists. That family is `agent/n3` only and is not in the
published record. If this hypothesis is right, the headline transfer of n3's
number onto the rest of the record is wrong, and the real exposure of the
record is a one-sided understatement whose size is what this axis measures.

## 3. Pre-checks — run FIRST, and if either fails the axis stops there

**P1 — span.** `XAUDUKA-15m` and `XAUDUKA-1m` must both cover 2010-06 →
2026-05. Checked by eye before this file was written: 15m = 378,749 rows,
2010-06-01 00:00 → 2026-05-31 23:45; 1m = 5,635,777 rows, 2010-06-01 00:00 →
2026-05-31 23:59, both `timestamp[ms, tz=UTC]`. **PASSES.**
`XAUUSD-1m` holds 100,000 rows, 2026-06-02 20:15 → 2026-09-11 20:56, so it
covers neither any published window nor this one; the brief's second-round
claim about it is wrong and is not used.

**P2 — same tape.** On every 15m bar of a declared 20,000-bar sample, the 15m
`high` must equal the maximum of its constituent 1m `high`s and the 15m `low`
the minimum of its 1m `low`s, to within 0.01 price points. **If more than 0.5%
of sampled bars fail, the two files are not the same tape, the comparison is
not clean, and this axis reports that and stops.** A 15m bar with no 1m bars
under it is counted as `missing`, never as agreement.

## 4. What is measured, and how

A trade is synthesised at **every 17th 15m bar** (stride 17 is coprime with the
96 bars of a day, so entries visit all 96 slots of the clock rather than six of
them), on **both sides**, over the whole 16 years. No strategy, no filter, no
selection: the point is the base rate of the ambiguity, not a method.

Each trade copies the engine exactly, read out of `engine.rs`:

* signal on bar `i-1`, fill at **`open` of bar `i`**;
* `entry = open_i + spread/2` for a long, `- spread/2` for a short
  (`apply_costs`, `entering = true`), `spread = 0.28` points
  (`[markets.xauduka.trading]`);
* `risk = k * ATR_{i-1}`, Wilder ATR period 14 seeded on the first 14 true
  ranges (`fd_indicators::atr`/`rma`), taken from the **signal** bar, NaN there
  refusing the trade;
* `stop = entry ∓ risk`, `target = entry ± risk * rr`; levels are **not**
  rounded and the hit test is raw `low <= stop` / `high >= target`, as
  `check_exit` does;
* a gap through a level fills at that bar's `open`;
* **stop checked before target**, then the clock;
* `trail` off (`[trading.trail] enabled = false`).

The 15m exit bar is, by construction, the **first** bar to touch either level,
so it is the **only** bar on the path that can be ambiguous. That is why the 1m
work is cheap and why it is exact rather than a sample: a trade whose 15m exit
bar does not contain both levels has a 15m verdict the 1m sequence cannot
overturn, and only the ambiguous ones are opened up on 1m.

On an ambiguous exit bar the 1m bars inside it are walked in time order and the
first level touched decides. **If a single 1m bar contains both levels the
trade is counted as `unresolved at 1m`** — reported as its own column and never
folded into either side (brief §7: `null != 0`).

### The table this axis exists to produce

One row per stop size, for each arm, carrying: trades, resolved share,
`both-touched` rate, 15m verdict mix, **disagreement rate**, **fake-loss rate**
(15m STOP, 1m shows target first), **fake-win rate** (15m TARGET, 1m shows stop
first), `unresolved at 1m`, and mean R under each adjudication.

## 5. Multiple testing — declared before any run

| axis | values | n |
|---|---|---|
| stop size, ATR multiples | 0.10, 0.20, 0.30, 0.378, 0.45, 0.50, 0.60, 0.714, 0.85, 1.00, 1.20, 1.50, 2.00, 2.681, 3.310, 4.210 | 16 |
| reward-to-risk | 1.0, 1.5, 1.8 | 3 |
| side | LONG, SHORT | 2 |
| hold cap | engine default 4 h (16 bars of 15m, `max_hold_ms = 14_400_000`); extended 48 h (192 bars) | 2 |

**Declared: 384 descriptive cells, 0 gate cells, 0 `search.exe` invocations.**
Entries per cell: every 17th 15m bar from the end of ATR warmup, which is
**22,278 bars** of the 378,749, so 22,278 trades per cell and ~8.55 M trade
adjudications in all.

The 16 stop sizes are not free choices. Five are n3's audit points
(0.30, 0.714, 1.00, 1.20, 2.681); four more are the realised median stops n3
measured for `trend-pullback` (0.378), `orb` (2.681), `rsi-reversal-vol`
(3.310), `volume-thrust` (4.210); 0.85 is `doji-reversal`'s realised stop as
printed in the published receipts; 1.20 is the config default; 1.50 and 2.00
are the commonest declared `stopAtr` in the registry; 0.10, 0.20, 0.45, 0.50
and 0.60 are there **only** to fill in the gap between 0.30 and 1.00 that n3
left open, which is the brief's question 1.

Anything run outside this table is written into a dated amendment at the foot
of this file and counted.

### Two censuses, also declared, also descriptive

**C1 — the registry.** Every strategy in `builtin::register_all`, with its
declared stop parameter(s) from `defaults()` and `grid()` read out of the
source. A strategy whose stop is **structural** (a swing, a range edge, a
previous-day level) has **no** declared ATR multiple, and its row says
`structural` and carries the realised figure only where one was measured
somewhere. Where no measurement exists anywhere, the cell is **`null` — not
measured**, never 0 and never estimated.

**C2 — the published record.** Every receipt on every branch of this repo that
carries the printed line `the method's own realised stop: median <x> ATR`,
paired with the row label, base strategy, trade count, profit factor,
percentile and verdict on the same row. This is a **complete** census of those
rows, not a sample and not a selection: all of them are tabulated and binned by
stop size. 15 branches carry receipts; ~500 receipt files; the line count will
be reported as found.

## 6. Falsifiers — specific, and both can fire

**F1 (the axis is useless if this fires).** *FIRES if the disagreement rate
exceeds 1.0% of trades at every stop size at or above 0.30 ATR, on both hold
arms.* That would mean the resolution limit is not a tail of the record but all
of it, and the right report is "nothing measured on 15m bars is readable",
not a threshold.

**F2 (my own section 2, attacked).** *FIRES if the fake-win rate exceeds 0.10%
of trades at any stop size on a market-at-next-open entry.* Section 2 claims
this is structurally zero. If it is not zero, my framing is wrong, n3's number
transfers further than I said, and that is the finding.

## 7. How it is read — thresholds fixed now

Per cell, on the disagreement rate (15m verdict != 1m verdict, as a share of
all trades in the cell):

* **<= 1.0% — readable.** The 15m verdict is the 1m verdict for practical
  purposes at that stop size.
* **1.0% to 5.0% — carries a caveat.** The row is quotable with the rate
  printed beside it.
* **> 5.0% — not evidence.** The row's win/loss mix is an artifact of the bar
  resolution at that stop size.

The **threshold** reported for question 1 is the smallest stop size in the grid
whose disagreement rate stays at or below 1.0% on both hold arms and both
sides, with the largest stop size that is still above 5.0% named beside it.

**Minimum sample:** a rate is printed only on at least 2,000 trades that
reached a stop/target verdict in that cell. Below that the cell prints `null`
and the reason, and no threshold is read off it.

A mechanism is assigned to a band by its **realised** stop where one has been
measured, and by its **declared** `stopAtr` where it has one and no measurement
exists. Which of the two a row used is printed on the row.

## 8. What this axis will NOT answer, declared now

* **One venue, one instrument, one timeframe pair.** XAUDUKA 15m against
  XAUDUKA 1m. The published rows measured on the `XAUUSD` tape get a rate
  estimated **for** them, not measured **of** them — the same limit n3 declared,
  and `XAUUSD-1m` cannot close it (pre-check P1).
* **Ticks.** 1m bars have their own both-touched ambiguity and it is reported
  as `unresolved at 1m` rather than resolved by a guess.
* **Unconditional entries.** These trades are not a mechanism's trades. A
  mechanism that enters on a volatility expansion sits on bars whose ranges are
  wider than the average, so its ambiguity rate at a given stop is **higher**
  than this table's, not lower. The table is therefore a **floor** on the
  artifact rate for such a mechanism, and that direction is stated on it.
* **Trailing stops and guard exits.** `trail` is off by default and every
  published number was measured without one; guard exits are a different rule
  and are not adjudicated here.
* **`wrong_side_stop`.** A separate defect, counted by the engine, 0 on the 144
  rows checked, and the owner's to decide (brief §4(vi)). Not re-measured here.

## 9. Signed

Written by the `intrabar` agent (Claude Opus 5, 1M context) on
`agent/intrabar`, 2026-10-07, before any measurement code existed in any file.
