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

## Amendment 2026-10-07 (1) — two declaration errors of my own, and one cell family added

Appended, not rewritten.

1. **Section 5 says 384 cells. The grid in it is 192.** 16 stops x 3 rr x 2
   sides x 2 hold arms = 192, and I wrote 384. **192 were run**, which is fewer
   than declared, and the ledger below carries both numbers.
2. **Section 5 says 22,278 entries per cell. It is 22,279.** Stride 17 from the
   first bar whose signal-bar ATR is finite, over 378,749 bars of 15m.
3. **One cell family added**, for a claim section 8 makes and does not test:
   that the pooled table is a FLOOR for a mechanism that enters on expanding
   bars. Added: the same measurement split by the signal bar's true range over
   its ATR, in terciles — 8 stop sizes x 3 terciles x 2 sides x 3 rr = **144
   further descriptive cells**, 4 h cap only, `py/research/intrabar_conditioned.py`,
   receipt `receipts/intrabar-conditioned.txt`. It carries no gate either.

## RESULT 2026-10-07 — the error is one-sided, the threshold is 0.600 ATR, and 3.0% of the record sits under it

Receipts: `receipts/intrabar-precheck.txt`, `intrabar-resolution-table.txt`,
`intrabar-registry-census.txt`, `intrabar-published-census.txt`,
`intrabar-conditioned.txt`, plus the cells (`intrabar-cells.json`) and the
census rows (`intrabar-published-rows.json`). Code: `py/research/intrabar_*.py`.
No engine file was touched: the diff against `agent/crt` contains no Rust.

### 0. The gate: there is none, and nothing here passes or fails one

192 + 144 descriptive cells, 0 gate rows, 0 windows, 0 `search.exe`
invocations, 0 candidates.

### 1. Pre-checks

**P1 — span: PASSES.** `XAUDUKA-15m` 378,749 rows 2010-06-01 00:00 →
2026-05-31 23:45; `XAUDUKA-1m` 5,635,777 rows 2010-06-01 00:00 → 2026-05-31
23:59. `XAUUSD-1m` 100,000 rows 2026-06-02 20:15 → 2026-09-11 20:56 — it covers
no published window, and the brief's second-round claim about it is wrong.

**P2 — same tape: PASSES, exactly.** On 20,000 sampled 15m bars the worst
disagreement between a 15m extreme and the extreme of its own 1m bars is
**0.0000 price points**, on the high, the low and the open alike; 0 sampled bars
had no 1m bars under them. This is a clean comparison in a way n3's could not
be: n3 had to audit on XAUDUKA-1m while measuring on the XAUUSD tape.

### 2. The table — one row per stop size

4 h cap (the engine default every published row ran under), both sides, rr
1.0/1.5/1.8 pooled, 133,674 trades a row.

| stop ATR | both-touched | fake LOSS | fake WIN | unres. at 1m | disagreement | band | mean R 15m | mean R 1m | understated by |
|---|---|---|---|---|---|---|---|---|---|
| 0.100 | 59.9% | 10,275 | **0** | 15,518 | **7.69%** | NOT EVIDENCE | -1.8465 | -1.6637 | +0.1828 R |
| 0.200 | 33.4% | 11,150 | **0** | 2,266 | **8.34%** | NOT EVIDENCE | -1.0040 | -0.8104 | +0.1936 R |
| 0.300 | 17.1% | 6,945 | **0** | 579 | **5.20%** | NOT EVIDENCE | -0.6651 | -0.5466 | +0.1185 R |
| 0.378 | 10.2% | 4,456 | **0** | 280 | **3.33%** | caveat | -0.5160 | -0.4408 | +0.0752 R |
| 0.450 | 6.4% | 2,883 | **0** | 180 | **2.16%** | caveat | -0.4251 | -0.3769 | +0.0482 R |
| 0.500 | 4.6% | 2,127 | **0** | 136 | **1.59%** | caveat | -0.3758 | -0.3403 | +0.0355 R |
| 0.600 | 2.7% | 1,289 | **0** | 88 | **0.96%** | readable | -0.3085 | -0.2869 | +0.0216 R |
| 0.714 | 1.5% | 769 | **0** | 63 | **0.58%** | readable | -0.2578 | -0.2450 | +0.0128 R |
| 0.850 | 0.9% | 479 | **0** | 62 | **0.36%** | readable | -0.2147 | -0.2066 | +0.0081 R |
| 1.000 | 0.6% | 336 | **0** | 57 | **0.25%** | readable | -0.1823 | -0.1765 | +0.0058 R |
| 1.200 | 0.4% | 250 | **0** | 55 | **0.19%** | readable | -0.1489 | -0.1446 | +0.0044 R |
| 1.500 | 0.3% | 191 | **0** | 62 | **0.14%** | readable | -0.1167 | -0.1134 | +0.0033 R |
| 2.000 | 0.2% | 105 | **0** | 56 | **0.08%** | readable | -0.0845 | -0.0826 | +0.0019 R |
| 2.681 | 0.2% | 68 | **0** | 45 | **0.05%** | readable | -0.0632 | -0.0620 | +0.0012 R |
| 3.310 | 0.1% | 50 | **0** | 34 | **0.04%** | readable | -0.0524 | -0.0516 | +0.0008 R |
| 4.210 | 0.1% | 26 | **0** | 23 | **0.02%** | readable | -0.0417 | -0.0413 | +0.0004 R |

The 48 h arm is in the receipt and moves nothing material: 7.69 / 8.34 / 5.20 /
3.33 / 2.16 / 1.59 / 0.97 / 0.58% down the same column.

**ANSWER 1 — the threshold is 0.600 ATR.** Below it the 15m verdict carries a
caveat; **below 0.300 ATR it is not evidence**; at 0.600 and above it is
readable. n3's three points reproduce on a clean tape and the gap between them
is filled: 2.7 ATR gives 99.95% agreement, 1.0 ATR 99.75%, 0.3 ATR 94.80%.

**ANSWER 2 — "fake wins" are ZERO, in all 4,277,568 trade adjudications.**
F2 does not fire, and it is a structural fact rather than a lucky count:
`check_exit` books a TARGET only on a bar that did **not** touch the stop, and
P2 shows a 15m low IS the minimum of its 1m lows to 0.0000 points, so no 1m bar
inside that bar can reach the stop. **For a market-at-next-open entry the
engine's both-touched rule cannot flatter a result; it can only understate
one.** n3's 24.8% is a property of a RESTING ENTRY — there the fill itself is an
intrabar event — and **it does not transfer to the rest of the record.**

**The number worth keeping, and it is the opposite of what the job feared:**
at `trend-pullback`'s realised stop of 0.378 ATR the engine's pessimism costs
**+0.0752 R per trade** — larger than the gate's whole expectancy leg of
+0.050 R. At 0.300 ATR it is +0.1185 R per trade. A tight-stopped mechanism can
be refused by this desk's gate on bar resolution alone.

### 3. The pooled table is a FLOOR, measured (the amendment's cell family)

Disagreement by the signal bar's true range over its ATR, terciles cut at 0.712
and 1.067, 4 h cap:

| stop ATR | quiet | middle | expansion | expansion - quiet |
|---|---|---|---|---|
| 0.300 | 2.96% | 4.71% | 7.92% | +4.96 pts |
| 0.378 | 1.75% | 2.90% | 5.36% | +3.61 pts |
| 0.450 | 1.05% | 1.82% | 3.60% | +2.55 pts |
| 0.500 | 0.80% | 1.31% | 2.67% | +1.87 pts |
| 0.600 | 0.54% | 0.79% | **1.56%** | +1.03 pts |
| 0.714 | 0.35% | 0.45% | 0.93% | +0.58 pts |
| 1.000 | 0.17% | 0.22% | 0.36% | +0.19 pts |
| 1.500 | 0.12% | 0.11% | 0.20% | +0.08 pts |

The claim holds at every stop size, and it has a consequence: **for a mechanism
that enters on expansion bars the threshold is 0.714 ATR, not 0.600**, because
0.600 reads 1.56% in that tercile.

### 4. ANSWER 3 — the registry: ZERO mechanisms declare a stop under the threshold, ONE measures under it

`builtin::register_all` registers **32** strategies. 14 declare an ATR stop, 11
hand the engine a structural price level, and 7 give no stop at all and take the
engine's `[trading] stop_atr = 1.2 ATR` fallback.

**Every declared ATR stop in the registry, default or grid: 1.0 (`vwap-fade`),
1.2 (`far-stop-break`, and the config fallback), 1.5, 2.0, 3.0, 4.0, 6.0. The
smallest is 1.0 ATR. NONE is below 0.600.** Four parameters whose names contain
"stop" are not in ATR and are not binned against this table: `stopMode` (a
switch: `orb`, `far-stop-break`), `stopGapMult` (a multiple of the gap:
`gap-fade`), `stopPrice` (price points: `rsi-reversal-vol`), `minStopPoints`
(price points: `far-stop-break`).

A structural stop has no declared multiple, so it is placed by its **realised**
median over every published row of it. 21 of the 32 have one; **11 are `null` —
never measured anywhere**: `level-reversion`, `maxpain-magnet`, `flow-momentum`,
`flow-at-level`, `buy-and-hold`, `external`, `session-hold`,
`intraday-momentum`, `tsmom`, `companion-unconfirmed`, `quiet-swing`.

**Of the 21 measured, exactly ONE sits below 0.600 ATR: `trend-pullback`,
median 0.356 ATR (min 0.149, max 1.180, over 101 rows) — the caveat band.** The
next tightest are `pdhl` at 0.758 and `doji-reversal` at 0.836, both readable.
Two bases in the record are not in `register_all` on this branch: `news-pulse`
(4.863 ATR) and `ratio-reversion` (2.095 ATR), both readable.

### 5. ANSWER 4 — the published record, by name

A complete census of every receipt that prints `the method's own realised stop:
median <x> ATR`: 560 receipt files across 15 branches plus
`docs/research/runs/` on HEAD; 6,906 rows parsed, 5,424 carrying a realised
stop, **3,781 distinct published rows** after de-duplicating the same
(file, label, trades, stop, market, timeframe) across branches that copy each
other's receipts.

| band | rows | with >= 40 trades |
|---|---|---|
| NOT EVIDENCE (< 0.300 ATR) | **31** | 29 |
| caveat (0.300 - 0.600 ATR) | **82** | 67 |
| readable (>= 0.600 ATR) | 3,668 | 2,698 |

**113 of 3,781 rows (3.0%) sit below the threshold.** The record's median
realised stop is **2.009 ATR** and 3,509 of 3,781 rows are at or above 1.0 ATR.
So the answer the brief said would also be a good one is the answer: **almost
every mechanism ran at a stop wide enough, and a thin tail of rows is touched.**

The 113, split by what they actually are:

* **43 are n3's own RESTING-ENTRY arm** (`n3-limit-*`, offsets `a`/`l`/`c`) —
  the one family where the error runs the other way and flatters the row. n3
  found that, named it, and the family passed nothing through the gate anyway.
* **70 are market-at-next-open rows: 60 `trend-pullback` and 10 `pdhl`.** For
  these the error is **understatement**, so correcting it could only make them
  **less bad**, never better than the gate.

**And the one number that closes the question: of all 113 below-threshold rows,
those showing a gate-readable figure — PF >= 1.200 with >= 40 trades — number
10, and ALL TEN are n3 resting-entry rows.** Not one market-entry row anywhere
in the published record is both below the resolution threshold and carrying a
figure a gate would read as positive. By name:

| stop ATR | PF | expect | trades | row | receipt |
|---|---|---|---|---|---|
| 0.294 | 3.622 | +0.311 | 118 | `pullback`/`trend-pullback` | `n3-limit-B-l050-guards` |
| 0.284 | 2.176 | +0.332 | 141 | `pullback`/`trend-pullback` | `n3-limit-A-l050-guards` |
| 0.149 | 1.786 | +0.283 | 153 | `pullback`/`trend-pullback` | `n3-limit-B-l025-guards` |
| 0.294 | 1.702 | +0.307 | 122 | `pullback`/`trend-pullback` | `n3-limit-B-l050-noguards` |
| 0.284 | 1.512 | +0.287 | 147 | `pullback`/`trend-pullback` | `n3-limit-A-l050-noguards` |
| 0.239 | 1.505 | +0.457 | 123 | `pullback`/`trend-pullback` | `n3-limit-B-c050-noguards` |
| 0.464 | 1.363 | +0.198 | 195 | `vwap-fade`/`vwap-fade` | `n3-limit-A-l050-guards` |
| 0.149 | 1.325 | +0.269 | 158 | `pullback`/`trend-pullback` | `n3-limit-B-l025-noguards` |
| 0.580 | 1.201 | 0.000 | 46 | `pdhl`/`pdhl` | `n3-limit-B-l025-guards` |
| 0.589 | 1.200 | -0.011 | 44 | `pdhl`/`pdhl` | `n3-limit-B-a025-guards` |

n3 said of two of these — the +60.67R and +56.21R window-B rows — "those lines
are not evidence." **That judgement is right, and it is also the full extent of
the damage.** The largest gains in the whole record do sit on an unreadable
stop; they sit on an unreadable stop **inside the resting-entry family**, which
is withdrawn, and the market-entry record behind it stands.

### 6. A contradiction with the brief, reported and not acted on

Brief section 8 says `data-sealed/` has never been consumed. Five receipts
already in this repo print `data: E:/rust/flowdesk/data-sealed` in their own
headers: `docs/research/runs/2026-09-24-cost-matched-null/`
{`registry-stops-guarded`, `xauduka-15y-guarded-corrected`,
`xauduka-same-dates-guarded-corrected`, `xauusd-guarded-corrected`,
`xauusd-unguarded-corrected`}`.txt`. I did not open, read, point `--data=` at,
or count anything in `data-sealed/`; the finding comes from the published
receipts' own text. **For the owner to reconcile.**

### 7. What was NOT measured

* **One venue.** XAUDUKA 15m against XAUDUKA 1m. 1,957 of the 3,781 published
  rows were measured on the `xauusd` tape and 888 on `xauduka`; for the former
  this table is an estimate **for** them, not a measurement **of** them.
  `XAUUSD-1m` cannot close that gap (P1).
* **Timeframes other than 15m.** 84 of the 3,781 rows are `1h` or `4h`. The
  ambiguity is a function of bar range over stop, so a 1h bar at the same ATR
  stop is strictly worse than a 15m one, and this table does not bound it.
* **Ticks.** 1m bars carry their own both-touched ambiguity and it is reported
  as `unresolved at 1m` rather than resolved by a guess: 15,518 of 133,674
  trades at 0.100 ATR, 579 at 0.300, 88 at 0.600, 55 at 1.200.
* **Conditioning on a mechanism's own signals.** Section 3 bounds the direction
  of that error without removing it.
* **Trailing stops, guard exits, `wrong_side_stop`.** All untouched.
* **The 11 `null` mechanisms' stops.** Nothing in the record measures them and
  they are not estimated.

### 8. Multiple-testing ledger

| | declared | looked at |
|---|---|---|
| gate rows | 0 | **0** |
| descriptive cells | 192 (written 384 in error) + 144 by amendment = 336 | **336** |
| `search.exe` invocations | 0 | **0** |
| published rows censused | all of them | **3,781** (6,906 parsed, 5,424 with a stop) |
| registry strategies censused | all of them | **32** |

### 9. Verdict

The engine's both-touched rule is **safe in the only direction that matters**:
on a market entry it cannot invent a win, and across 4.28 M adjudications it
never did once. What it does instead is **tax tight stops** — +0.0752 R per
trade at 0.378 ATR, which is more than the gate's own expectancy requirement.
3.0% of the published record sits below the 0.600 ATR line where that tax bites,
and the only rows under that line that look *good* are the resting-entry rows
n3 had already withdrawn. **The record is confirmed, not undermined. Report
only — the rule is not changed.**
