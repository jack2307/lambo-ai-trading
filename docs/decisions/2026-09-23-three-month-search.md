# 2026-09-23-three-month-search: what the best three-month rules did on the nine months they did not see

**Registration:** `docs/hypotheses/2026-09-23-three-month-search.md`, commit `0115da6`,
written and committed **before any run**.
**Status:** decided. The claim is **refuted**. 161 cells swept across 21 mechanisms;
25 out-of-sample tests run; **0 survived**. And the registration's own headline
prediction — that the search would clear 50% in sample — was itself **falsified**:
the best number anywhere in 161 cells is **+20.22%**, and it lost money out of sample.

## What the owner asked

"Dựa vào dữ liệu 3 tháng gần đây tìm ra cho tôi phương pháp trade tốt nhất với lãi
suất ít nhất 50%" — find the best method on the last three months, returning at
least 50%.

## The short answer

**Nothing reached 50%. Nothing reached half of it.** The best of 161 cells returned
**+20.22%** on the three months and **−7.69%** on the nine months before it, at the
50th percentile of its own null. Every one of the twenty-five cells carried out of
sample was refuted.

## How it was run

Four agents, four disjoint sets of mechanisms, four worktrees and branches
(`agent/three-month-1..4`, merged here). Each swept its mechanisms' own registry
grids on `xauusd:15m`, walk-forward 4 folds with **each cell's parameters pinned**
so the walk-forward had nothing left to select, guards on, the configured spread of
0.28, `select_by` expectancy, no filter added and no threshold moved. Each carried
its top five by walk-forward return onto **2025-09-23 → 2026-06-22** at the same
parameters with no re-fit, and scored them on the registered three legs:
expectancy > 0, PF ≥ 1.2, and ≥ 95th of a count-matched null (200 seeds, the
2026-09-23 null repair in force).

**Cells swept: 161.** Slice 1 45, slice 2 45, slice 3 45, slice 4 26. "The best of
N" means nothing without N, so N is here.

**Mechanisms, 21:** ema-cross, rsi-reversion, donchian-breakout, keltner-break
(slice 1); macd-cross, rsi2-pullback, squeeze-break, stoch-reversal, orb (slice 2);
pdhl, ict-sweep-mss-fvg, vwap-fade, doji-reversal, gap-fade (slice 3);
trend-pullback, volume-thrust, volman-box, tsmom, intraday-momentum, session-hold
(slice 4).

## The window is six days short, and was not extended

The registration asks 2026-06-23 → 2026-09-23. The 15m store's last bar is
**2026-09-17 13:00**, so every slice ran 5,731 bars over 2026-06-23 → 2026-09-17.
All four recorded it independently.

The cause is not a broken desk: the VPS exporter runs every five minutes and was
writing parquet at 23:01 on 2026-09-23, rc=0. It is the **local research copy**
that had not been pulled since 17 September. It was deliberately **not** refreshed
mid-program, because four slices quoting two different windows cannot be put in one
table. A shorter window cannot flatter an in-sample number.

## In sample: the best cell of each slice, all four on the same ranking

| slice | cells | best cell | walk-forward return | whole-window return |
|---|---:|---|---:|---:|
| 2 | 45 | `macd-cross fast=16 rr=1` | **+20.22%** | +31.80% |
| 1 | 45 | `donchian-breakout period=20 stopAtr=1.5` | **+12.90%** | — |
| 4 | 26 | `session-hold` (no grid) | **+1.63%** | −8.71% |
| 3 | 45 | `doji-reversal bodyMaxPct=0.05 rr=2` | **+1.18%** | −2.64% |

The owner's target is 50%. The best of 161 is 20.22%.

## Out of sample: the same cells, same parameters, no re-fit

The five nearest survival across all four slices, and the two that fell hardest:

| cell | slice | in sample | trades | return | PF | expectancy | null 95th | pct | legs cleared |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `keltner-break mult=1.5 rr=2` | 1 | +10.77% | 699 | +18.90% | 1.056 | +0.0502R | 1.156 | 79th | 1 of 3 |
| `keltner-break mult=2.5 rr=2` | 1 | +8.26% | 262 | +8.72% | 1.097 | +0.0443R | 1.205 | 78th | 1 of 3 |
| `macd-cross fast=16 rr=2` | 2 | +8.95% | 862 | +13.53% | 1.033 | +0.049R | 1.120 | 74th | 1 of 3 |
| `macd-cross fast=16 rr=1.5` | 2 | +12.70% | 895 | +10.65% | 1.028 | +0.035R | 1.120 | 72nd | 1 of 3 |
| `keltner-break mult=1.5 rr=1.5` | 1 | +8.92% | 731 | +8.13% | 1.025 | +0.0291R | 1.136 | 69th | 1 of 3 |
| `donchian-breakout p=20 stop=1.5` | 1 | +12.90% | 952 | **−29.27%** | 0.913 | −0.0395R | 1.100 | 26th | 0 of 3 |
| `trend-pullback fast=10 rr=1.5` | 4 | −5.22% | 925 | **−29.10%** | 0.794 | −0.267R | — | 1st | 0 of 3 |

**Survivors: 0 of 25.** Five cells kept a positive expectancy out of sample; not one
reached PF 1.2, and not one reached the 95th percentile. The single best-performing
cell in sample flipped 42 points, from +12.90% to −29.27%.

This is the registration's first named outcome: *"a three-month search buys a
number, not an edge."* Here it did not even buy much of a number.

## The registration's own prediction was wrong, and that matters more than the result

The registration opens: *"That search will succeed. Over ~6,000 fifteen-minute
bars... the best of hundreds on a window that short clears 50% by arithmetic, not
by edge."* **It did not.** The best of 161 cells reached 20.22%.

The registration did name this branch — *"Nothing clears 50% even in sample. Also
possible, and would say the window is too short to produce even a lucky number at
this risk level"* — so it is a pre-registered outcome and not a surprise. But the
stated expectation was the opposite, and it was wrong.

**Why it was wrong: the guards.** 1% risk per trade, a daily cap of 20, the news
flat at 60/30, the weekend flat, a 30-minute cooldown, and the notional ceiling
that sized down 459 of 952 entries on one cell. Multiplying 1% risk by the number
of trades those guards leave on 5,731 bars caps what any cell can print. The guards
are why the search could not manufacture a lucky 50% — they worked, in the
unglamorous direction of refusing to produce an impressive-looking number.

A second registration claim was also wrong. It cites *"the best expectancy this
desk has ever measured is +0.011R (`macd-cross`, 923 trades)"* as the ceiling the
50% target sits 29× above. On the nine months, unfiltered `macd-cross fast=16 rr=2`
measures **+0.049R over 862 trades** — about 4.5× that figure on a comparable
count. The verdict on that cell is unchanged (PF 1.033 is not 1.2; the 74th is not
the 95th) and the target is still far out of reach, but "+0.011R, best ever" was
not the ceiling it was presented as.

## What the two funded books measure on the same three months

Neither live book is a cell of this sweep — both run at default parameters with
session and news filters, the sweep ran bare grids all day — so slice 2 measured
them again in their **actual live configuration** on the same window
(`slice-2-funded-rows.txt`, `--mode=hypotheses --fixed`, 200 seeds, guards on).

**`xau-macd-asia` — live on 33708517 right now — contradicts the record downward.**

| | three months (this run) | the record's headline |
|---|---:|---:|
| trades | 97 | 300 |
| profit factor | **1.134** | **1.56** |
| expectancy | +0.031R | — |
| matched-null percentile | **76th** | **100th** |
| count match | 0.93, inside band | — |

The standing gate is PF 1.2 and the 95th. **On the most recent three months this
book does not clear it.** Since the owner made the recent Vantage year primary on
2026-09-13, and these three months are the newest part of that same year, the
failure to reproduce PF 1.56 is on the criterion he chose. The record's own
three-year figure of 0.91 looks more representative than its one-year figure.

Whether it keeps running is the owner's decision and he was told the same day,
unprompted, as pre-commitment 3 of the null-repair registration requires. Nothing
in this record changes a live book.

**`xau-stoch` — no longer on money — was worse than a coin.** 384 trades, PF
**0.836**, expectancy **−0.087R**, **14th** percentile (count match 0.69,
unmatched, and the miss direction is conservative). Below the median of random
entry over nearly 400 trades. All nine `stoch-reversal` grid cells also lost over
the whole window, worst −28.23%, the worst mechanism in slice 2 by a distance.

This does not contradict the record, which never claimed an edge for it — the
candidate entry was explicit that PF 1.48 came from 35 trades of one week, in
sample. It contradicts letting live money keep finding out. The owner stopped
account 33705331 before this number existed; the number now says he was right to.
`accounts.toml` has `vantage-cent enabled = false`, and the VPS confirms exactly
two executors running, both on 33708517: `ai-xau-ds-ctx` and `xau-macd-asia`.

*(Slice 2's report described `xau-stoch` as "on the funded account right now". That
was true when the null-repair registration was written and is not true now. It was
checked against `accounts.toml` and the live process list before being relayed.)*

## Defects found during the run, all published rather than smoothed over

**1. Slice 4 ran its first two stages with no news calendar installed**, so the news
flat named in its own guards header was inert. It was caught by cross-checking
against the audited `search --mode=hypotheses --fixed` printer, which does install
it: 917 vs 910 trades on one cell, profit factors apart in the second decimal,
NEWS_FLAT refusals in one run and none in the other. It **changed which cell ranked
fifth**, so the carried five were not the same five. Both stages were re-run and
the defective receipts published as `slice-4-*-without-news-calendar.txt` rather
than deleted.

**2. Slice 3 ranked its cells by plain whole-window return**, not the walk-forward
return the registration specifies, while slices 1, 2 and 4 all used walk-forward.
It flagged this itself, unprompted, and asked for reconciliation before the record
was written. Reconciled toward the registration's own wording, and slice 3
restated. **The top five changed completely**, with only one mechanism keeping any
place. The restated five were run out of sample too: **0 of 5 again**, none
clearing even one leg, four of them under the 30-trade floor.

The ordering here is a real weakness and the record states it rather than
presenting the restated column as the plan: slice 3's original five had already
been run out of sample and **all those numbers were seen before the ranking
changed**. What protects the correction is that both orderings and both
out-of-sample tables are in the record, the originals untouched beside the restated
ones, and both answers are the same answer.

**3. The hold null is still not count-matched, and the defect is wider than
recorded.** `session-hold` and `intraday-momentum` go down the `RandomHold` path,
which is not count-matched: controls of 1,289 and 2,722 trades against methods of
191 and 179 — ratios **6.75** and **15.20**. Slice 4 printed those percentiles as
**UNMEASURED with the ratio beside them** rather than quoting the existing
printer's 12th and 52nd, which is the correct call. It also established that
**`tsmom` joins that bucket**: pinning a cell empties its grid, and `hypotheses.rs`
takes the hold-null path whenever the exit is self-managed *and* the preset grid is
empty. It did not reach the leg here only because all three `tsmom` cells took zero
trades. This extends the already-recorded hold-null defect to a third mechanism and
to every future cell-level run of it.

**4. Count matching missed the 0.25 band on many rows.** Slice 1: 0.73, 0.76, 0.79,
0.80. Slice 2: 0.74, 0.71, 0.75. In every case the control took **fewer** trades
than the method, which widens the control's distribution and **raises** the 95th —
so the mismatch is conservative and cannot be what produced the failures. Two rows
that matched cleanly (0.97 and 1.01) failed anyway, at the 78th and the 14th.

## Cells that could not be scored at all

**15 of slice 4's 26 cells produced no walk-forward book, and no filter was
loosened to rescue them.** All 3 `tsmom` cells took **0 trades** over the whole
three months at every lookback — the warmup alone exceeds the window. All 3
`volman-box` cells took ≤1 trade. All 9 `volume-thrust` cells took 0–4. A fold needs
5 training trades to select, so there is no book.

Slice 4 ranked these **last** rather than at the 0.00% an empty book reports,
because otherwise every mechanism that never fired would outrank every mechanism
that fired and lost. `null` is not `0`. Its top five are therefore the top five of
the **eleven** cells that produced a book, and every count is printed.

In slice 3, four of the five restated cells rest on **one measured fold and two
trades**, printing an infinite profit factor because neither trade lost. The folds
column sits to the left of the return column in that receipt for exactly that
reason.

## Deviation from the registration

The registration says the top five **overall** are carried forward. Splitting by
mechanism across four agents meant each carried its own five, so twenty went out of
sample, and twenty-five once slice 3 restated. That is **more** out-of-sample tests
than registered, not fewer, so it cannot flatter the result — but it is not what
was written down, and all four slices recorded it independently.

## What did not move

No threshold: the gate stayed at the config's 30 trades / PF 1.2 / 0.05R, both null
readings at the 95th, spread at 0.28, no re-fit on the nine months, no filter
added, the feed not extended. `ui/src/lib/verdicts.ts` untouched by every slice.

**Pre-commitment 3 of the registration holds: nothing from this search goes on the
funded account, whatever it showed.** It showed nothing, so there is nothing to
resist.

## Receipts

`docs/research/runs/2026-09-23-three-month-search/` — twenty files, including the
two superseded `slice-4-*-without-news-calendar.txt` and the three superseded
`slice-3-*` originals beside their restatements. Runners:
`crates/fd-backtest/src/bin/slice_three_month.rs`, `three_month_2.rs`,
`three_month_slice3.rs`, `three_month.rs`.
