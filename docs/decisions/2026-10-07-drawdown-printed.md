# 2026-10-07 — the gate prints a risk number: `max_drawdown_usd` / `max_drawdown_pct` reach the receipt, and are checked before they are believed

**Registered:** committed ALONE, before the first line of code on this branch.
**Branch:** `agent/drawdown`, worktree `E:/rust/fd-drawdown`, cut from
`agent/instr-repair`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + `AGENT-BRIEF-ADDENDUM-5.md`
section G.
**Status:** registered.

## 0. What this axis is, and what it is not

Two parts, and they spend differently.

**Parts (1) and (2) are INSTRUMENT. They have NO GATE and spend 0 cells.**
No method is run to see whether it clears PF >= 1.200 / expectancy >= +0.050R /
>= 40 trades. Nothing here is a hypothesis about a market, and no percentile is
read. The deliverable is a column that exists and an arithmetic that has been
checked. Following the precedent of `2026-10-07-instrument-repair.md`: **added
beside, never in place of**, and proved bit-identical on everything already
published.

**Part (3) is a MEASUREMENT and it spends cells. Declared: 20.** They are the
rows the record itself already printed a positive verdict for at >= 40 trades,
named in §4 below before anything is run. The 20 are not chosen by me from a
sweep — they are chosen by the record — and the full batch each one sits in is
re-run because that is the only way to reproduce the row, while only the 20
declared rows are read.

## 1. Hypothesis, one sentence

Every profit figure in this record was printed without a risk figure beside it,
because `max_drawdown_usd` / `max_drawdown_pct` exist in
`engine.rs::Metrics` and `--mode=hypotheses` never printed them; printing them
will show that at least one row the record calls a survivor carries a drawdown
a real account could not hold.

## 2. Falsifier, specific and firable

**F1 (on the instrument, part 2).** `max_drawdown_*` has never been printed by
the mode that produced this record's ~9,858 rows, so it has never been read, so
it has never been checked. If a hand-built trade sequence with a drawdown known
by arithmetic disagrees with what `metrics_of` reports, **the quantity is wrong
and the finding of this job is that defect** — and the number is NOT then
printed across the record as if it were a measurement. Three sequences, written
before the code: a known drawdown, an all-winning sequence (drawdown must be
**0.00 USD**, and `pct` must be **0**, not `NaN`, because a curve that never
fell did measure a fall of zero), and a single trade.

**F2 (on the measurement, part 3).** If every one of the 20 declared rows comes
back with a drawdown a $10,000 account holds without flinching, then the claim
in §1 is false: the record's profit numbers were missing a risk number but no
decision would have changed, and that is the result to report.

**F3 (anti-smuggling).** If any figure already published moves — trades,
profit factor, expectancy, `expectancy_net`, null percentile, or any golden
parity field — the patch is not additive and is wrong regardless of how good
the new column looks. Checked by `to_bits()` equality, not by eye, exactly as
`instr-repair` checked `r_net == r` on a costless row.

## 3. What the quantity MEANS, to be stated in the receipt

`metrics_of` walks the trades in order, `equity += trade.pnl_usd`, and tracks
`peak - equity`. So it is a drawdown on the **CLOSED-TRADE equity curve**: the
worst peak-to-trough fall measured only at trade closes. It is **not** the
intrabar excursion — a position that went 3R against the book and came back to
close green contributes **nothing** to it, and `avg_mae` is the only field that
sees that. A receipt that prints the number must say which of the two it is, or
the reader will assume the worse one. The denominator of `pct` is to be stated
too, and checked against the oracle's golden files, which carry
`maxDrawdownPct` that no test has ever compared.

## 4. The 20 declared cells, named before the run

| # | row | source receipt | trades on record |
|---|---|---|---|
| 1 | `close/fri-1630-1815` | `2026-09-13-close-reopen-drift/in-sample-fixed.txt` | 336 |
| 2 | `ict-B-balanced` | `2026-09-13-ict-sweep-mss-fvg/in-sample.txt` | 62 |
| 3 | `ict-B-allday` | same | 157 |
| 4 | `london/nyam` | `2026-09-13-london-range/in-sample.txt` | 257 |
| 5 | `london/early` | same | 203 |
| 6 | `orb/60m` | `2026-09-13-orb-ny/in-sample.txt` | 84 |
| 7 | `orb/60m-expansion` | same | 48 |
| 8 | `hold/04-06-short` | `2026-09-13-recent-year-hours/in-sample.txt` | 205 |
| 9 | `hold/16-18-long` | same | 699 |
| 10 | `hold/18-20-long` | same | 164 |
| 11 | `hold/20-22-long` | same | 463 |
| 12 | `macd-cross/asia` | `2026-09-13-recent-year-sessions/in-sample.txt` | 300 |
| 13 | `keltner-break/asia` | same | 262 |
| 14 | `ict-sweep-mss-fvg/asia` | same | 81 |
| 15 | `tsmom2/60d` | `2026-09-13-tsmom-2/in-sample-fixed.txt` | 75 |
| 16 | `tsmom2/20d` | same | 149 |
| 17 | `tsmom/20d` | `2026-09-13-tsmom/in-sample.txt` | 189 |
| 18 | `tsmom/60d` | same | 108 |
| 19 | `struct-80` | `2026-09-23-designed-1-cost-term/xauusd-guarded.txt` | 175 |
| 20 | `struct-80-f14` | same | 152 |

`ema-cross/asia` (PF 1.632, 99th percentile) is **excluded on purpose**: 37
trades is below the desk's 40 and the brief's ~40 floor, and the tool's own
`need 30` printed it as a pass. It will be shown in the table marked
"under the floor, not a candidate".

## 5. How to read the result

- A drawdown next to a profit factor, nothing more. **There is no drawdown
  gate** on this branch and none is proposed: the desk has not set one, and
  inventing a threshold here would be a new gate smuggled in under an
  instrument repair.
- `pct` is of the account the config sizes against (`starting_equity_usd`), so
  it answers "what fraction of the book" and not "what fraction of the risk
  taken".
- Addendum-5 §D still binds: any long-horizon row that only exists in the
  no-guards arm is not a tradeable candidate, and its drawdown is a reading of
  an arm the owner has forbidden. The table says so per row.
- The walk-forward rows concatenate out-of-sample folds, and **each fold's
  engine run restarts at `starting_equity_usd`** while `metrics_of` then walks
  the concatenation as one curve. The USD drawdown is therefore the drawdown of
  a book that was re-sized at each fold boundary, which is the only curve this
  record has. Stated, not hidden.

## 6. Note added 2026-10-07, after the run

See the RESULT section appended at the bottom of this file. Nothing above is
rewritten.

---

# RESULT — appended 2026-10-07, nothing above rewritten

## R0. Corrections to §4, made by appending rather than by rewriting

Four source attributions in §4 were wrong when it was written. The rows are the
same rows; the receipts they came from are not the ones named:

- cells 17/18 (`tsmom/20d`, `tsmom/60d`) are **not** from
  `2026-09-13-tsmom/in-sample.txt` — every row of that receipt is a fail. The
  published rows at those figures are
  `2026-09-13-tsmom-silver/in-sample-fixed.txt` (`tsmom/20d`, **xagduka** 15m,
  189 trades, PF 1.341) and `2026-09-14-tsmom-eurusd/in-sample-fixed.txt`
  (`tsmom/60d`, **eurduka** 15m, 108 trades, PF 1.809). Those two commands are
  what was re-run.
- cell 14 (`ict-sweep-mss-fvg/asia`, 81 trades) is from
  `2026-09-13-recent-year-sessions-5m/out-of-sample.txt` (**xauduka 5m**,
  2022-06-16 → 2025-04-10), not the 15m sessions receipt. On 15m that row is
  **9 trades** and the record itself printed it as `fail: only 9 trades` — so
  the 15m reading was never a candidate, in the record or here.
- cells 19/20 (`struct-80`, `struct-80-f14` at 175 / 152 trades) are from
  `2026-09-24-repair-c/era-era2-h72.txt`, which ran on **`data-sealed`** and
  with a `max_hold_ms = 72 h` config in a scratchpad that no longer exists.
  `data-sealed` is closed (brief §9), so the row was re-run on the **live**
  store with a rebuilt 72 h config. It reproduced the published figures exactly,
  which is itself evidence that the two stores do not differ on that window.

**20 cells declared, 20 read.** Three rows outside the list are quoted in §R2
and are marked as context, not as results.

## R1. The instrument: it is arithmetically right, and it is a CLOSED-EQUITY drawdown

**F1 did not fire. `max_drawdown_usd` is correct.** Eight hand-built sequences
in `crates/fd-backtest/tests/drawdown.rs`: a known 800.00 USD peak-to-trough
fall (and the curve's net is -300.00 USD on the same trades, so the two numbers
are not each other); a later-but-smaller fall that must not displace the worst
one; an all-winning curve — **0.00 USD and 0%, not `NaN`**; a single losing
trade (250.00 USD = 2.5% of 10,000); a single winning trade (0.00 USD); and an
empty cell (percent `NaN`, and the printer refuses rather than print the 0.0
the struct carries there).

**Which of the two drawdowns it is: the closed-trade one.** `metrics_of` walks
the trades in order, adds `pnl_usd`, tracks `peak - equity`. Every trade in the
fixture carries `mae = -3.0` R and a curve of winners still reports 0.00 USD,
so the figure is **blind to what a position did while it was open**. The printed
line says exactly that on every row and names `avg_mae` beside it as the only
field that does see it.

**Two findings about the figure itself, both now pinned by tests:**

1. **`max_drawdown_pct` divides by the curve's HIGHEST equity, not the peak the
   fall started from.** `-1,000` then `+11,000` from a 10,000 book prints
   **5.00%** where the account lived through **10.0%**. The percent is
   therefore a **floor** on the conventional max-drawdown-percent and can never
   overstate it. The USD figure needs no denominator and is exact on every
   curve; it is the one to quote.
2. **It matches the oracle.** `tests/parity.rs` compared `maxDrawdownUsd` and
   never `maxDrawdownPct`, although the golden files have carried the percent
   all along. The comparison was added and **passes on all 9 btc strategies and
   the gold set**, so the denominator convention is the oracle's, faithfully
   ported — not a defect this port introduced. Checked by mutation: scaling the
   Rust value by 1.01 makes all 9 fail, so the new comparison bites.

**Nothing moved (F3 did not fire).** The drawdown prints on its **own line**
below the row, the way `expectancy_net` was added the same day — the table is
not widened. `gate_header()` and `gate_row()` were lifted into pure functions
so two tests can assert **byte equality against the published record**: the
header line, and the `hold/04-06-short session-hold 205 1.481 …` row, both read
out of `docs/research/runs/2026-09-13-recent-year-hours/in-sample.txt` and
rebuilt from their own figures. The full `--release` suite on `fd-backtest` +
`fd-core` is green and golden parity passes with one more field compared than
before.

**Scale of what was missing:** 1,742 gate rows in this repository's receipts
(`docs/` plus the `n5` and `instr-repair` receipt stores) and **16 lines in the
whole record that mention a drawdown at all**, none of them from this printer —
they come from `designed_3.rs` and the `diag_*` examples.

## R2. The table — the record's best rows, with the risk figure beside them

Re-run with `target/release/search.exe` off `agent/drawdown`,
`--data=E:/rust/flowdesk/data`, `--exit-mix`; commands in
`receipts/drawdown/COMMANDS.md`. **`expectancy_net` is not a column because
every one of these rows paid zero swap and zero commission**, where `r_net == r`
bit-for-bit and the printer suppresses the line. That is a property of the rows,
not of the patch — and §R3.4 is about exactly that zero.

**xauusd 15m, 2025-09-13 → 2026-09-12 — one window, one book, so these rows are
comparable with each other.** `starting_equity_usd = 100.00` on this market (a
cent account: 100 USD = 10,000 USC), so read the percent, not the USD.

| row | trades | PF | expect | max dd USD | max dd % (floor) | net USD | net / dd | verdict today | record |
|---|---|---|---|---|---|---|---|---|---|
| `macd-cross/asia` | 300 | 1.558 | +0.276 R | 17.16 | 9.29% | +83.27 | **4.85** | SURVIVES | 1.558 SURVIVES |
| `hold/04-06-short` | 205 | 1.507 | +0.173 R | 14.70 | 10.70% | +37.36 | 2.54 | SURVIVES | 1.481 → 1.507 SURVIVES |
| `hold/16-18-long` | 197 | 1.374 | +0.150 R | 15.96 | 12.10% | +27.24 | 1.71 | gate pass, in the noise | 1.390, same verdict |
| `hold/18-20-long` | 164 | 1.362 | +0.221 R | 16.79 | 12.04% | +32.45 | 1.93 | SURVIVES | 1.386 SURVIVES |
| `keltner-break/asia` | 262 | 1.286 | +0.154 R | 9.47 | **6.88%** | +34.80 | **3.67** | SURVIVES | 1.286 SURVIVES |
| `hold/20-22-long` | 164 | 0.862 | -0.147 R | 47.39 | 40.16% | -19.09 | — | fail | 0.856, same verdict |

**xauduka 15m, 2022-06-16 → 2025-04-10 — a 10,000 USD book, same hypotheses.**
All four reproduce the published figures exactly.

| row | trades | PF | expect | max dd USD | max dd % (floor) | net USD | verdict today | record |
|---|---|---|---|---|---|---|---|---|
| `hold/16-18-long` | 560 | 1.508 | +0.104 R | 1,375.73 | 8.45% | +6,151.20 | SURVIVES | identical |
| `hold/20-22-long` | 463 | 1.238 | +0.121 R | **3,144.68** | **17.72%** | +7,749.46 | SURVIVES | identical |
| `hold/18-20-long` | 464 | 1.140 | +0.045 R | 1,965.29 | 15.34% | +2,297.59 | fail (PF, expectancy) | identical |
| `hold/04-06-short` | 578 | 0.588 | -0.292 R | **14,656.61** | **139.89%** | -13,972.40 | fail | identical |

**The rest of the declared list.**

| row | store / window | trades | PF | expect | max dd USD | max dd % (floor) | net USD | verdict today | record |
|---|---|---|---|---|---|---|---|---|---|
| `close/fri-1630-1815` | xauduka 15m 2018-06→2025-04, fixed | 336 | **2.092** | **+0.051 R** | 139.83 | **1.17%** | +1,866.53 | SURVIVES | 2.064 / +0.054 R |
| `tsmom/60d` | eurduka 15m 2010-06→2018-06, fixed | 108 | 1.809 | +0.284 R | 792.36 | 6.02% | +3,151.79 | SURVIVES | identical |
| `tsmom/20d` | xagduka 15m 2010-06→2018-06, fixed | 189 | 1.333 | +0.103 R | 1,222.77 | 9.38% | +2,010.22 | gate pass, in the noise | 1.341 SURVIVES |
| `tsmom2/60d` | xauduka 15m 2018-06→2025-04, fixed | 75 | 1.990 | +0.287 R | 553.88 | 4.52% | +2,263.62 | gate pass, in the noise | 2.031 SURVIVES |
| `tsmom2/20d` | same | 149 | 1.294 | +0.091 R | 616.50 | 5.35% | +1,337.93 | gate pass, in the noise | 1.325 SURVIVES |
| `ict-sweep-mss-fvg/asia` | xauduka 5m 2022-06→2025-04 | 81 | 1.251 | +0.139 R | 642.60 | 5.48% | +1,075.89 | SURVIVES | identical |
| `struct-80` | xauusd 15m 2023-06→2024-06, 72 h hold, guards | 175 | 1.395 | +0.105 R | 7.05 | 5.84% | +17.68 | SURVIVES | identical |
| `struct-80-f14` | same | 152 | 1.492 | +0.113 R | 6.35 | 5.30% | +16.73 | SURVIVES | identical |
| `ict-B-balanced` | xauusd 1m → 2026-09-12 | 65 | 1.346 | +0.204 R | 7.61 | 6.44% | +12.22 | SURVIVES | 62 tr, 1.261 |
| `ict-B-allday` | same | 145 | 1.131 | +0.086 R | 11.91 | 9.75% | +11.09 | fail (PF) | 157 tr, 1.350 SURVIVES |
| `london/nyam` | xauusd 5m → 2026-09-15 | 258 | 1.319 | +0.084 R | 8.23 | 6.48% | +19.89 | SURVIVES | identical |
| `london/early` | same | 204 | 1.274 | +0.082 R | 7.72 | 6.43% | +14.08 | gate pass, in the noise | identical |
| `orb/60m` | same | 84 | 1.206 | +0.102 R | 6.23 | 5.67% | +6.55 | gate pass, in the noise | 1.249 SURVIVES |
| `orb/60m-expansion` | same | 48 | 1.269 | +0.109 R | 3.08 | 2.89% | +4.43 | gate pass, in the noise | 1.284 SURVIVES |

Three rows **outside** the declared list, quoted as context and not as results:
`ict-C-loose` (230 trades, PF 1.226, dd 8.28%, SURVIVES today); `struct-80` over
the full 2022-06→2025-09 window at the 4 h default (1,437 trades, PF 0.973, dd
19.01 USD = 19.01%, fail — reproducing the sealed-store receipt figure for
figure on the live store); and `ema-cross/asia` (37 trades, PF 1.622, dd 4.18
USD = 3.72%), which is **under the 40-trade floor and not a candidate**, exactly
as §4 said before the run.

## R3. The answer to the question that was asked

**Is there a row in the record that passes the gate with a drawdown nobody
could hold? Not one that ruins a 10,000 book — but the risk figure separates
rows the gate called equal, and the record's best-looking survivor turns out to
be its cheapest one in an arm the owner has forbidden.**

1. **Named: `hold/20-22-long`, xauduka, PF 1.238 on 463 trades, 100th
   percentile, SURVIVES — with 3,144.68 USD of closed-equity fall.** The
   printed 17.72% is a floor; the book's peak is at most 17,749 USD and at
   least 10,000, so the conventional reading is **between 17.7% and 31.4% of
   the equity it had at the time**. `hold/16-18-long`, same window, same batch,
   same arm, clears the gate at **PF 1.508 for 1,375.73 USD** — a higher profit
   factor for **44% of the fall**. Nothing in the record before today could
   tell those two apart.
2. **The ordering by profit factor is not the ordering by return per unit of
   fall.** On the one window where the comparison is legitimate (xauusd 15m,
   same cent book, same dates, five rows at or above the gate's PF leg):
   `keltner-break/asia` is **fifth of five by profit factor and second by net
   per USD of fall** (3.67 against `hold/04-06-short`'s 2.54 and
   `hold/16-18-long`'s 1.71), and `hold/16-18-long` moves from third to fifth.
   A desk reading PF alone chooses a different row than a desk reading both.
   This is not a criterion and nothing is selected by it; it is the measurement
   that the two orderings differ.
3. **The record's highest profit factor has the smallest drawdown AND lives in
   the forbidden arm.** `close/fri-1630-1815`: PF 2.092, 336 trades, 100th
   percentile, fall **139.83 USD = 1.17%** of a 10,000 book — and a mean hold
   of **3,017.6 minutes (50.3 hours)** whose every exit is `window closed`
   after the weekly close, i.e. **it holds across the weekend**, at
   `swap: long 0.00 / short 0.00`, `guards: off`, long share **1.000**.
   Addendum-5 §D (never hold over a weekend) and §C (carry is counted per
   crossing of 17:00 NY, and a weekend crossing is the triple) together say the
   figure is a drawdown of a book that paid nothing for the only thing the
   method does. Its expectancy is **+0.051 R against a +0.050 R gate leg**.
4. **The same caveat on all four long-horizon rows** (`tsmom/60d` eurduka,
   `tsmom/20d` xagduka, `tsmom2/60d`, `tsmom2/20d`): mean holds of 14 to 28
   days, `guards: off`, swap 0.00 per lot-night. `tsmom/60d` eurduka shows the
   best profit-per-fall in the whole declared list (+3,151.79 USD against
   792.36 USD = 3.98) and Addendum-5 §C measured financing turning `ts-l60`
   from +0.370 R to **-0.271 R**. **A drawdown measured with no carry on a
   60-day hold is not that trade's drawdown**, and these four are the rows
   where the missing `expectancy_net` line of §R2 is the whole story.
5. **A defect the new column exposed, counted and not fixed: the engine trades
   past ruin.** `hold/04-06-short` on xauduka prints **139.89% of peak** — the
   closed-equity curve fell from a 10,477 USD peak to about **-4,180 USD** and
   the run carried on. `size_position` computes `risk_usd = equity *
   risk_per_trade_pct` and then `lots = (…).max(rules.min_lot)`, so once equity
   is negative the sizing clamps to the minimum lot and keeps opening positions
   forever; nothing refuses the trade and nothing stops the run. Consequence for
   reading the record: **a blown-up row's profit factor and expectancy are
   flattered**, because its post-ruin losses are taken at `min_lot` rather than
   at the risk fraction. Every row where this can bite is already a fail row,
   so no published pass is affected — this is a count in the manner of brief
   §7, not a repair.
6. **Four rows the record printed as `SURVIVES` no longer survive, and the
   drawdown is not why**: `tsmom2/60d` (PF 2.031 → 1.990, percentile 98 → 93),
   `tsmom2/20d` (1.325 → 1.294, 96 → 86), `tsmom/20d` xagduka (1.341 → 1.333,
   100 → 93), `orb/60m` (1.249 → 1.206, 100 → 88). Trade counts are identical
   and the expectancies move by a hundredth of an R or less, so this is the
   2026-09-21 USD restatement (profit factor reads `pnl_usd`) and the
   2026-09-24 cost-matched null reaching rows that were published before them —
   not anything this branch did. Reported because a reader comparing this table
   with the old receipts will see it and should know which repair it belongs to.

## R4. What this does NOT say

- **No drawdown gate is proposed.** The desk has not set one; inventing a
  threshold inside an instrument repair would be a new gate smuggled in. Brief
  §4 is unchanged, and `survives()` is untouched.
- **The percent is a floor twice over**: once from the denominator (§R1.1) and
  once from the zero financing on the long-horizon arm (§R3.4).
- **No cross-instrument drawdown comparison is made.** xauusd here is a 100 USD
  cent book and xauduka/xagduka/eurduka are 10,000 USD books; the USD columns
  are not comparable between the two tables and the percents only loosely.
- **These are closed-trade falls.** The deepest point an open position reached
  is in none of them. `avg_mae` (-0.07 R to -1.76 R on these rows) is the only
  figure that sees it, and a true intrabar drawdown would have to be built from
  the per-bar equity curve the engine already keeps — a different job, not
  started here.
- **A reproduction is not a confirmation.** Where today's figures match the
  published ones exactly, that says the engine is stable since that receipt, not
  that the row is tradeable: eleven of the twenty rows are in the no-guards arm
  and four of those hold for weeks.
