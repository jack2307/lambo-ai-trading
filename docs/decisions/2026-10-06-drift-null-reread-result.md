# The gold-intraday record re-read against a drift-carrying null: one verdict falls, and it is the only one there was

**Date:** 2026-10-06
**Registration:** `docs/decisions/2026-10-06-drift-null-reread.md`, committed
alone at `272d9a5` before the first `search.exe` call, amended at `3f3a746`
before the confirmation runs were read.
**Branch:** `agent/m10`, cut at `f684b5e`.
**Binary:** `/e/rust/fd-wt-crt/target/release/search.exe`, built 2026-10-04
15:13 from `agent/crt`. No `cargo build` was run. Disk on `E:` was 32 GB free
before and 32 GB free after.
**Receipts:** `docs/research/runs/2026-10-06-drift-null-reread/`, nine files
(`{A,B,P}-{coin,ratio,exposure}.txt`) plus two seed-count checks
(`{A,P}-coin-200seeds.txt`).
**This axis had no gate to pass and produced no candidate.** It is a
measurement of the measuring instrument.

## Read this first, or the tables below will be read wrong

**The desk's median null has a profit factor of 0.867 — below 1.** Every
percentile in this document, under every null, means *"lost less than a random
book at the same cost, the same trade count and the same side exposure"*. It
never means "made money". On these 39 (row, window) pairs the nulls' own median
profit factors run **0.312 to 1.086**, and only two of 39 are above 1.000.
**The gate is the decision leg**: `PF >= 1.200 AND expectancy >= +0.050R`, and
`>= 30` trades, with nothing concluded under 40 trades. No cell in this
document is good.

## What was run

`--mode=hypotheses --batch=gold-intraday --market=xauusd`,
`--data=/e/rust/flowdesk/data`, walk-forward 4 folds, `select_by = expectancy`,
`min_trades_per_cell = 5`, spread 0.28 USD per round trip, swap 0.00,
`max_hold_ms` 4 h, guards off, no companion, no tape. News calendar: **747
events from `E:/rust/flowdesk/data/news/events.parquet`** — the directory
`--data=` named, printed in every receipt header. `data-sealed/` was not
opened, read or counted.

**Null draws: `--seeds=2000` on every run.** The brief names `--samples=` and
`--direction-samples=`; read from `crates/fd-backtest/src/bin/search.rs`,
**neither is read in `--mode=hypotheses`** (`--samples=` is read only by
`--mode=null-dir`, line 207; `--direction-samples=` only by `--mode=rescore`,
line 282). The null draw count in this mode is `--seeds=`, default 200. I
passed all three so the receipts record them; 2000 is the number that acted.
The declared step-down ladder was never used: the wall-clock probe came back at
0.43 s and the six core runs took 20–34 s each.

| window | span | bars kept | runtime at 2000 draws |
|---|---|---:|---:|
| A | 2025-07-01 → 2025-10-01 | 6,044 | 20–34 s per null |
| B | 2025-04-01 → 2025-07-01 | 5,862 | 22–34 s per null |
| P (confirmation) | 2022-06-16 → 2026-09-11 | 100,165 of 100,586 | 543 s measured on `P-coin`; the other two between about 600 s and 660 s from the receipts' file timestamps, which is the only timing I have for them |

P is the window `docs/decisions/2026-09-12-gold-intraday-batch-1.md` published
on, run so the central question could be put against figures that *were*
published.

## The answer to the central question

**One published verdict falls, and it is the only verdict the whole 117-cell
matrix had.** Across the three windows there are exactly **two** cells at or
above the 95th percentile under `coin`, both on window A, and exactly **one**
cell anywhere that the engine's own `survives()` called a survivor:

| window A, `ny-morning/donchian-breakout` | 73 trades, PF 1.704, expectancy +0.303R |
|---|---|
| `coin` (the record's null) | **98%**, null p50 0.946, null p95 1.557 → **`SURVIVES`** |
| `ratio` | 94%, null p50 **1.086**, null p95 **1.752** → `gate pass, inside the noise` |
| `exposure` | 95%, null p50 **1.074**, null p95 **1.721** → `gate pass, inside the noise` |

The `exposure` column prints `95%` because the receipt rounds (`{p:.0}%`) while
`survives()` compares the unrounded figure against 95.0. The independent check
is in the row itself: **the null's own 95th percentile is 1.721 and the method's
profit factor is 1.704.** The method is below the 95th percentile of its own
drift-matched null, by 0.017 of profit factor. That bounds the true percentile
to **[94.5, 94.95]** — it printed 95 so it is at least 94.5, and it is under the
p95 value so at most 94.95.

The control was matched on the quantities that matter: **signed share of time
+0.365 for the method against +0.390 achieved by the control** (gap 0.025,
inside the 0.05 band), **count match 0.85**, **time in market ratio 1.17**, both
inside the 0.25 band. Cost match **0.74**, which is 0.01 outside the band and is
the one check this row does not pass; it is stated rather than absorbed.

**And the figure that says what the row actually was:** against a coin, the
noise this rule was compared to had a median profit factor of 0.946 — a losing
book. Against a control that leans long in the New York morning the way the
method leans, for the same count and the same cost, **the noise has a median
profit factor of 1.074 and a 95th percentile of 1.721**. A random entry that
borrows the method's drift exposure *makes money on this window*, and makes more
of it at the top of its distribution than the method did. The 98th percentile
was gold going up between July and September 2025, not a breakout rule.

Window B, the same rule, same parameters: **PF 0.752, expectancy −0.099R, 28th
percentile, 41 trades.** It was never a two-window finding. Under brief §3 it
had nothing even before this.

## The table: every mechanism × its percentile under coin / ratio / exposure

`p50` is the null's own median profit factor and is printed beside every
percentile, as required. `ssM` is the method's signed share of time; `ssN` is
what the `exposure` control achieved.

### Window A — 2025-07-01 → 2025-10-01, 6,044 bars, 2000 draws

| mechanism | trd | PF | expect | coin (p50) | ratio (p50) | exposure (p50) | Δpct | ssM → ssN | cnt / cost / time |
|---|---:|---:|---:|---|---|---|---:|---|---|
| intraday/ema-cross | 54 | 0.703 | −0.186R | 20% (0.887) | 19% (0.888) | 18% (0.892) | −2 | +0.165 → +0.163 | 1.37 / 1.37 / 1.56 |
| intraday/rsi-reversion | 69 | 0.701 | −0.153R | 17% (0.886) | 18% (0.862) | 19% (0.866) | +2 | −0.498 → −0.498 | 1.41 / 1.62 / 1.72 |
| intraday/donchian-breakout | 199 | 1.119 | +0.062R | 86% (0.902) | 85% (0.925) | 84% (0.928) | −2 | +0.238 → +0.238 | 0.79 / 0.64 / 1.08 |
| intraday/bb-fade | 396 | 0.828 | −0.081R | 33% (0.892) | 33% (0.888) | 34% (0.881) | +1 | −0.143 → −0.140 | 0.50 / 0.48 / 0.89 |
| ny-morning/ema-cross | **14** | 2.567 | +0.445R | 100% (0.936) | 100% (0.936) | 100% (0.947) | +0 | +0.047 → +0.055 | 2.36 / 2.17 / 1.90 |
| **ny-morning/donchian-breakout** | 73 | 1.704 | +0.303R | **98%** (0.946) | 94% (1.086) | **95%** (1.074) | −3 | +0.365 → +0.390 | 0.85 / 0.74 / 1.17 |
| london-open/donchian-breakout | 48 | 0.765 | −0.115R | 37% (0.854) | 37% (0.856) | 37% (0.850) | +0 | +0.171 → +0.187 | 0.96 / 1.12 / 1.34 |
| asia/rsi-reversion | 56 | 0.924 | −0.029R | 50% (0.918) | 50% (0.928) | 51% (0.913) | +1 | −0.013 → −0.007 | 0.20 / 0.18 / 0.33 |
| asia/bb-fade | 31 | 0.713 | −0.168R | 24% (0.913) | 24% (0.910) | 22% (0.919) | −2 | −0.143 → −0.140 | 1.45 / 1.29 / 3.29 |
| expansion/donchian-breakout | 41 | 1.062 | +0.022R | 71% (0.869) | 67% (0.909) | 62% (0.940) | **−9** | +0.233 → +0.251 | 1.00 / 1.26 / 0.94 |
| expansion/ema-cross | **3** | 0.000 | −0.489R | 0% (0.910) | 0% (0.771) | 0% (0.909) | +0 | −0.029 → +0.008 | 4.67 / 6.23 / 3.66 |
| compression/rsi-reversion | **26** | 0.926 | −0.030R | 65% (0.549) | 65% (0.552) | 66% (0.504) | +1 | −0.343 → −0.600 | 0.12 / 0.09 / 0.18 |
| compression/bb-fade | 54 | 1.153 | +0.060R | 73% (0.837) | 73% (0.842) | 72% (0.856) | −1 | −0.048 → −0.053 | 0.31 / 0.30 / 0.54 |

Median |coin → exposure| move: **1.0 percentile point**, max −9.
Median |coin → ratio| move: 0.0 points, max −4.
Gate: **one** row passes (`ny-morning/donchian-breakout`), and it loses its
`SURVIVES` as above. `ny-morning/ema-cross` is at 100% on **14 trades** and
fails the gate on count alone; nothing is concluded from it.

### Window B — 2025-04-01 → 2025-07-01, 5,862 bars, 2000 draws

| mechanism | trd | PF | expect | coin (p50) | ratio (p50) | exposure (p50) | Δpct | ssM → ssN | cnt / cost / time |
|---|---:|---:|---:|---|---|---|---:|---|---|
| intraday/ema-cross | 46 | 0.895 | −0.054R | 42% (0.946) | 42% (0.947) | 42% (0.949) | +0 | −0.014 → −0.006 | 1.63 / 1.46 / 1.63 |
| intraday/rsi-reversion | 216 | 0.959 | −0.016R | 53% (0.947) | 52% (0.945) | 53% (0.946) | +0 | −0.001 → −0.000 | 0.50 / 0.42 / 0.88 |
| intraday/donchian-breakout | 155 | 1.023 | +0.017R | 61% (0.977) | 62% (0.969) | 62% (0.970) | +1 | +0.095 → +0.089 | 1.09 / 1.11 / 1.21 |
| intraday/bb-fade | 439 | 0.778 | −0.104R | 9% (0.961) | 10% (0.955) | 8% (0.962) | −1 | −0.031 → −0.029 | 0.48 / 0.48 / 0.94 |
| ny-morning/ema-cross | **14** | 0.214 | −0.571R | 9% (0.832) | 9% (0.824) | 9% (0.823) | +0 | −0.168 → −0.116 | 0.71 / 0.81 / 0.74 |
| ny-morning/donchian-breakout | 41 | 0.752 | −0.099R | 28% (0.928) | 27% (0.932) | 26% (0.942) | −2 | +0.085 → +0.086 | 0.98 / 1.11 / 1.20 |
| london-open/donchian-breakout | 40 | 0.835 | −0.101R | 38% (0.932) | 37% (0.938) | 38% (0.932) | +0 | +0.000 → +0.006 | 1.00 / 0.78 / 1.37 |
| asia/rsi-reversion | **28** | 0.994 | +0.002R | 51% (0.986) | 47% (1.032) | 47% (1.027) | −4 | −0.186 → −0.223 | 1.11 / 1.27 / 1.25 |
| asia/bb-fade | 58 | 0.681 | −0.187R | 7% (0.990) | 10% (0.952) | 12% (0.918) | +5 | +0.248 → +0.227 | 1.21 / 1.19 / 1.98 |
| expansion/donchian-breakout | 39 | 0.714 | −0.115R | 33% (0.871) | 30% (0.888) | 30% (0.887) | −3 | −0.216 → −0.215 | 0.87 / 1.04 / 1.03 |
| expansion/ema-cross | **4** | 0.833 | −0.086R | 54% (0.755) | 54% (0.755) | 56% (0.715) | +2 | +0.282 → +0.312 | 2.00 / 1.81 / 2.03 |
| compression/rsi-reversion | **29** | 1.014 | +0.008R | 69% (0.312) | 68% (0.394) | 68% (0.382) | −1 | −0.595 → −1.000 | 0.10 / 0.11 / 0.19 |
| compression/bb-fade | 75 | 1.320 | +0.092R | 82% (0.834) | 78% (0.875) | 77% (0.929) | −5 | −0.483 → −0.504 | 0.25 / 0.26 / 0.45 |

Median |coin → exposure| move: **1.0 percentile point**, max +5.
**Zero** cells at or above the 95th under any null. One row passes the gate
(`compression/bb-fade`, PF 1.320, 75 trades) and it is at the 82nd/77th — it
never had a percentile to lose, and its count match is **0.25** and its cost
match **0.26**, both far outside the band, so its percentile is not quotable
under any setting. Window A has it at PF 1.153 / 73rd and failing the gate.

### Window P — 2022-06-16 → 2026-09-11, 100,165 bars, 2000 draws (the published window)

| mechanism | trd | PF | expect | coin (p50) | ratio (p50) | exposure (p50) | Δpct | ssM → ssN | cnt / cost / time |
|---|---:|---:|---:|---|---|---|---:|---|---|
| intraday/ema-cross | 1024 | 0.766 | −0.133R | 1% (0.900) | 1% (0.900) | 1% (0.899) | +0 | +0.056 → +0.065 | 1.17 / 1.02 / 1.64 |
| intraday/rsi-reversion | 1267 | 0.890 | −0.059R | 41% (0.901) | 57% (0.881) | 56% (0.882) | **+15** | −0.602 → −0.599 | 1.31 / 1.00 / 1.86 |
| intraday/donchian-breakout | 3488 | 0.901 | −0.030R | 34% (0.921) | 30% (0.926) | 28% (0.926) | −6 | +0.131 → +0.135 | 0.74 / 1.02 / 0.91 |
| intraday/bb-fade | 3348 | 0.888 | −0.069R | 32% (0.907) | 30% (0.907) | 32% (0.905) | +0 | −0.023 → −0.014 | 0.99 / 0.70 / 2.00 |
| ny-morning/ema-cross | 279 | 0.871 | −0.070R | 36% (0.908) | 36% (0.909) | 36% (0.908) | +0 | −0.015 → −0.012 | 1.29 / 0.99 / 1.76 |
| ny-morning/donchian-breakout | 1192 | 0.922 | −0.023R | 47% (0.928) | 47% (0.928) | 46% (0.927) | −1 | +0.064 → +0.054 | 0.75 / 1.05 / 1.01 |
| london-open/donchian-breakout | 692 | 0.865 | −0.051R | 33% (0.897) | 29% (0.908) | 28% (0.913) | −5 | +0.150 → +0.160 | 0.94 / 0.83 / 1.09 |
| asia/rsi-reversion | 385 | 0.939 | −0.029R | 64% (0.905) | 70% (0.889) | 70% (0.888) | +6 | −0.282 → −0.269 | 1.05 / 0.88 / 1.51 |
| asia/bb-fade | 719 | 0.914 | −0.047R | 52% (0.910) | 56% (0.904) | 55% (0.904) | +3 | −0.087 → −0.070 | 1.45 / 1.09 / 2.18 |
| expansion/donchian-breakout | 923 | 0.854 | −0.069R | 24% (0.916) | 22% (0.921) | 22% (0.919) | −2 | −0.061 → −0.061 | 0.69 / 0.54 / 1.21 |
| expansion/ema-cross | 214 | 0.959 | −0.022R | 69% (0.893) | 66% (0.903) | 66% (0.901) | −3 | −0.189 → −0.180 | 1.24 / 1.02 / 1.65 |
| compression/rsi-reversion | 318 | 0.660 | −0.148R | 7% (0.862) | 7% (0.846) | 7% (0.840) | +0 | −0.333 → −0.330 | 0.59 / 0.56 / 1.56 |
| compression/bb-fade | 1277 | 0.814 | −0.102R | 20% (0.873) | 23% (0.864) | 25% (0.863) | +5 | −0.204 → −0.197 | 0.56 / 0.49 / 1.15 |

Median |coin → exposure| move: **3.0 percentile points**, max +15.
**Zero** cells pass the gate; zero reach the 95th under any null. Every
expectancy is negative. This reproduces the published verdict — thirteen
hypotheses, zero survivors — and the drift control does not change it.

## Did the published percentiles collapse? The honest accounting

**On the published window, no — because there was nothing at the 95th to
collapse.** The published addendum's best figure was the 59th percentile and its
worst was the 3rd; my `coin` column reproduces that shape (1st to 69th, best
69th). Under `exposure` the best is the 70th. **Thirteen of thirteen published
percentiles survive a drift-carrying null in the only sense available to them:
none was ever a claim, and none becomes one.** That is the result the brief said
would be worth having even if it read "all survive", and it is what the
published window says.

**On window A, yes — the one verdict there was.** Not a 95 → 40 crash; a
`SURVIVES` → `inside the noise`, which is the thing the record publishes.

A caveat I cannot remove, and it is the reason the `coin` column and not the
published column is my baseline:

| row | published PF (2026-09-12) | my PF today | published pct | my `coin` pct |
|---|---:|---:|---:|---:|
| intraday/ema-cross | 0.768 | 0.766 | 1% | 1% |
| intraday/rsi-reversion | 0.890 | 0.890 | 38% | 41% |
| london-open/donchian-breakout | 0.872 | 0.865 | 48% | **33%** |
| expansion/donchian-breakout | 0.856 | 0.854 | 41% | **24%** |
| expansion/ema-cross | 0.895 | **0.959** | 49% | **69%** |

Median |published → my `coin`| across the 13 rows is **5.0 percentile points**
and the maximum is **20**, at the *same* null setting. **This is not the seed
count.** I measured that separately: re-running `coin` at the record's
**200** draws against my 2000 moves the percentile by a median of **1.0 point**
and at most **5 points** (window A: median 1.0, max 4; window P: median 1.0,
max 5) — receipts `A-coin-200seeds.txt`, `P-coin-200seeds.txt`. The cause is
**the bar file**: the published run names **100,249 bars** for
2022-06-16 → 2026-09-11, and today's `XAUUSD-15m.parquet` holds **100,586**
bars of which **100,165** fall inside that span. Read either way, 100,249 is
neither figure, so it is not the same file. Trade counts differ by 1 to 4 on every row and one profit
factor differs by **0.064** (`expansion/ema-cross`, 218 → 214 trades). **The
published gold-intraday percentiles are not reproducible to better than about
five points on today's data, and the drift-null question is smaller than that
irreproducibility on this batch.** The clean comparison is `coin` → `exposure`
*within one run*, which is what every Δpct column above is.

## The falsifiers, each answered

| falsifier | fired? | where |
|---|---|---|
| **1 — the collapse falsifier** (fires if no cell `>= 95th` under `coin` loses it under `exposure`) | **did NOT fire** | `ny-morning/donchian-breakout` on window A loses `SURVIVES` at both `ratio` and `exposure`, on 73 trades with count 0.85, time 1.17 and a drift match of 0.025 — the axis found exactly one thing to overturn, and it was the only verdict in the matrix |
| **2 — the inertness falsifier** (fires below a 3-point median move) | **FIRED on A and B** (median 1.0 point each); did **not** fire on P (median 3.0, which is not below 3.0) | the drift control is close to inert on a three-month window of this batch and is not inert on four years |
| **3 — the side-match falsifier** (fires if the match fails on a majority) | **did NOT fire** | the signed share of time is matched inside the 0.05 band on **36 of 39** (row, window) pairs; the three outside are `compression/rsi-reversion` on A (26 trades, gap 0.257) and on B (29 trades, gap 0.405) and `ny-morning/ema-cross` on B (14 trades, gap 0.052) — all three are thin rows whose count match is 0.10–0.71 and none is quotable anyway. The receipt's `** the control's side ratio is not the method's **` flag fires on 5 of 13 (A), 4 of 13 (B) and 0 of 13 (P), but that flag reads the long share **by trade count**, which `exposure` deliberately does not match; it is design, not defect |
| **4 — the one-sidedness falsifier** (fires if nothing has drift exposure) | **did NOT fire** | 9 of 13 rows on A, 7 of 13 on B and 7 of 13 on P have `|signed share of time| > 0.10`; the extremes are **−0.602** (`intraday/rsi-reversion`, P, 1267 trades) and **−0.595** (`compression/rsi-reversion`, B) |

So: **one falsifier fired — inertness, on both three-month windows — none fired
on the published window, and the collapse falsifier did not fire anywhere.**
The axis is not dead, and it is also not large.

## The number I did not expect, and it is the one I would keep

Falsifier 4 not firing matters more than it looks. **The control achieved the
method's drift exposure almost exactly** — +0.238 → +0.238, −0.498 → −0.498,
−0.602 → −0.599, +0.365 → +0.390, median gap **0.006 to 0.009** across the
three windows — **and the percentile still moved by a median of 1 to 3
points.** This batch is genuinely one-sided, the instrument genuinely removed
the one-sidedness, and the readout barely changed. That is a much stronger
negative than "there was no drift to take away".

Where it is not inert, it points the way `2026-10-02-drift-null.md` predicted
and could not support. On the 7 one-sided, drift-matched rows of the published
window with at least 30 trades, the correlation between the signed share of
time and the `coin` → `exposure` percentile move is **r = −0.882**, and 5 of 7
move in the predicted direction: long-leaning rows fall (`donchian-breakout`
−6, `london-open` −5), short-leaning rows rise (`rsi-reversion` **+15**,
`asia/rsi-reversion` +6, `compression/bb-fade` +5). Window A gives r = −0.615
on 8 rows, 6 of 8 agreeing. Window B gives r = **+0.984** on **3** rows, 0 of 3
agreeing, which is three rows and is reported because it disagrees. Pooled over
all 18 rows: **r = −0.492, agreeing on 11 of 18**.

That decision recorded the prediction as "unsupported" on three session rows
with a signed share of about 0.12. On four years of bars and signed shares out
to −0.602 it is supported, in the direction it named. **It is supported as a
direction of movement, not as a change of verdict**: the largest move on the
published window is `intraday/rsi-reversion` going from the 41st to the 56th
percentile with a profit factor of **0.890** and an expectancy of **−0.059R**.
The record's `coin` reading of that row was too *low*, and the repaired reading
is still a losing book at the 56th percentile of a null whose own median is
0.882. **Nothing is rescued by this. One row of the record was understated and
it was understated about a loss.**

## Multiplicity ledger

| | cells |
|---|---:|
| declared in the registration, before the first run | **117** (13 × 2 windows × 3 nulls = 78 core, plus 13 × 1 × 3 = 39 confirmation) |
| core matrix, looked at | 78 |
| published-window confirmation, looked at | 39 |
| **overrun:** two seed-count checks, `coin` at 200 draws on A and on P | **26** |
| wall-clock probe (elapsed time only; no PF, no percentile read) | 0 |
| **total looked at** | **143** |

The overrun is 26 cells, declared here with its reason: the published → `coin`
gap of up to 20 points needed a cause, and the only way to rule out the draw
count was to run the record's draw count. It ruled it out (median 1.0 point) and
sent the cause to the bar file instead. **No parameter declared in the
registration was raised after seeing a result.** The seeds ladder was never
stepped. The one reading I changed — from the rounded `pct` column to the
verdict column — was recorded as a dated amendment before the confirmation runs
were read, and it made the criterion stricter.

For scale: `agent/new-method-2` incurred 247 looks against 6 declared and had
20.6% of cells at or above the 95th. This work incurred 143 against 117 and has
**2 of 39** (row, window) pairs at or above the 95th under `coin` — 5.1% —
which is about what a correct null should give.

## What I could NOT measure, and why

1. **The exact percentile of the one cell that changed verdict.** The receipt
   prints `{p:.0}%`, so `ny-morning/donchian-breakout` under `exposure` reads
   `95%` for anything in [94.5, 95.5). The verdict column settles which side of
   95.0 it is on, and the printed `null p95` of 1.721 against a method profit
   factor of 1.704 bounds it to [94.5, 94.95] — but **the figure itself is not
   in any receipt** and getting it needs a printer change, which needs a build,
   which the brief forbids. I did not build.
2. **Whether the published 2026-09-12 percentiles were correct on their own
   data.** Today's `XAUUSD-15m.parquet` is not the file that run read: it holds
   100,586 bars, of which 100,165 fall inside the published range, against the
   100,249 that run names for that span, trade
   counts differ by 1–4 on all 13 rows, and `expansion/ema-cross`'s profit
   factor differs by 0.064. The published table is therefore **not bit-
   reproducible** and the published → `coin` column mixes the null question with
   a data change. I report `coin` → `exposure` within one run instead and say so
   wherever the published figures appear.
3. **Whether the one changed verdict would change on more data.** It is 73
   trades on one three-month window. Brief §4's warning applies to it directly:
   the same rule and window gave `PF 1.753 / 96th / 14 trades` and
   `PF 0.682 / 8th / 178 trades` in the CRT work, the only difference being
   sample size. 73 trades is above the 40 I registered and well below anything
   comfortable, and the same rule on window B is at the 28th with PF 0.752.
4. **The direction null.** Still unrepaired
   (`2026-10-02-drift-null.md`, item 3) and `--null-sides=` does not reach it,
   so no direction percentile on any of these rows is readable. I did not open
   that registration.
5. **Why `compression/rsi-reversion`'s control collapses to a signed share of
   −1.000 on window B** (against the method's −0.595) while its count match is
   0.10 and its time-in-market ratio 0.19. The row is unquotable on three
   checks at once and I did not chase it; it is 29 trades.

## What this changes in the record

- `docs/decisions/2026-09-12-gold-intraday-batch-1.md` stands. Its thirteen
  percentiles were never claims and none becomes one under a drift-carrying
  null; its "zero survivors" verdict reproduces at 2000 draws under all three
  nulls.
- **No new candidate.** Zero cells pass the gate on both windows; the two that
  pass on one window each (`ny-morning/donchian-breakout` on A,
  `compression/bb-fade` on B) are at the 28th and 73rd on the other.
- The one figure a reader should take away is not a percentile: **on window A,
  a random entry matched to this rule's side exposure, trade count and cost
  earns a median profit factor of 1.074 and a 95th percentile of 1.721, and the
  rule earned 1.704.** The rule was the drift, measured with worse luck than
  the top twentieth of coin-free noise.
- The `2026-10-02` prediction about the direction of the correction now has
  support on four years of bars (r = −0.882 on 7 rows), and it corrects a loss
  *upward*. It should be recorded as supported in direction and irrelevant in
  consequence.
