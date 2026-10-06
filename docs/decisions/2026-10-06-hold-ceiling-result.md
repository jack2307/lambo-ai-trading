# 2026-10-06-hold-ceiling: the four-hour cap was acting on most positions, and lifting it helped nothing

**Registration:** `docs/decisions/2026-10-06-hold-ceiling.md`, commit `2d6ddda`,
written and committed **before any run**. Config arms: commit `b021cd4`.
**Status:** decided. The axis is **refuted**. 76 cells looked at, 76 declared,
**0 past the gate** on either window, at any ceiling, with guards on or off.

## The short answer, in three parts

1. **The 4-hour ceiling was real and was acting.** It closed **60.0%** of
   window A's `volume-thrust volMult=2.0` positions (30 of 50), **76.9%** of
   `volMult=2.5` (10 of 13) and **100%** of `volMult=3.5` (3 of 3). No receipt
   before 2026-09-24 could say that, because none counted exits. So the
   complaint that the cap was invisible is **correct**.

2. **Lifting it did not open anything.** At 24 h the cap stops binding
   (TIMEOUT falls to 4.3% / 8.3% / 33.3% on those same three rows) and at
   120 h it binds on nothing at all with guards on. Through that whole change
   **no cell reached PF 1.200 and +0.050R on either window**, let alone both.

3. **It made the one readable row worse.** `vthr-vm20` is the only row that
   ever exceeded 40 trades. Window A: PF **1.073**, expectancy **+0.086R** at
   4 h → PF **0.738**, **−0.078R** at 120 h. That is **−0.164R of expectancy
   bought by lifting the ceiling**, on the row the axis most needed.

## Two of the three mechanisms were never under the ceiling at all

**`tsmom` cannot see `max_hold_ms`.** `tsmom.rs:69` returns `Exits::Strategy`;
`engine::check_exit` returns `None` for a self-managed position at
`engine.rs:737`, **before** the `max_hold_ms` branch at `engine.rs:760`.
Measured, not inferred: the warmup diagnostic at `lookbackDays = 5` returns
**byte-identical rows** at the 4 h and the 120 h ceiling —

| window | trades | PF | expectancy | mean hold | exits |
|---|---:|---:|---:|---:|---|
| A, 4 h | 11 | 8.241 | +0.443R | **3,962.7 min (66.0 h)** | flip 2, END_OF_DATA 1, NEWS_FLAT 4, WEEKEND_FLAT 4 |
| A, 120 h | 11 | 8.241 | +0.443R | **3,962.7 min (66.0 h)** | identical |
| B, 4 h | 12 | 1.932 | +0.106R | **3,327.5 min (55.5 h)** | flip 3, END_OF_DATA 1, NEWS_FLAT 2, WEEKEND_FLAT 6 |
| B, 120 h | 12 | 1.932 | +0.106R | **3,327.5 min (55.5 h)** | identical |

A mean hold of 66 hours under a 4-hour ceiling, with **zero TIMEOUT exits**.
The ceiling is inert for this mechanism. `tsmom`'s 0 trades at its three
declared lookbacks are the **warmup**, exactly as the 2026-09-23 record said:
`warmup()` is `(lookbackDays + 5) * 288` bars = 7,200 at the shortest declared
lookback of 20, against **6,044** bars in window A and **5,862** in window B.
The warmup alone exceeds the window. No hold ceiling can fix that; a longer
feed or a coarser warmup would.

**`volman-box` is gated at the entry.** 6 trades in window A and 1 in window B
at `boxBars = 12`, and **0 at boxBars 20 and 30 in every one of the twelve
cells**, identical at all three ceilings. Its one window-A TIMEOUT out of six
positions (16.7%) is the whole of the ceiling's effect on it.

## A longer ceiling can only LOWER the trade count, and did

Both `volman-box` and `volume-thrust` return `Intent::None` while a position
is open, and `max_concurrent_positions = 1`. A TIMEOUT frees the book; removing
it does not. Counts, guards on:

| row | A 4 h | A 24 h | A 120 h | B 4 h | B 24 h | B 120 h |
|---|---:|---:|---:|---:|---:|---:|
| tsmom-lb20 / lb60 / lb120 | 0 | 0 | 0 | 0 | 0 | 0 |
| vbox-bb12 | 6 | 6 | 6 | 1 | 1 | 1 |
| vbox-bb20 / bb30 | 0 | 0 | 0 | 0 | 0 | 0 |
| vthr-vm20 | **50** | 46 | **44** | **53** | 51 | **51** |
| vthr-vm25 | 13 | 12 | 12 | 14 | 13 | 13 |
| vthr-vm35 | 3 | 3 | 3 | 2 | 2 | 2 |

Not one row took more trades at a longer ceiling. The best count anywhere in
this axis is **53 trades over three months**, at the 4-hour cap the axis set
out to remove.

## Where the exits went — the `--exit-mix` evidence

`vthr-vm20`, window A, the only row with a sample worth reading:

| ceiling | guards | trades | STOP | TARGET | TIMEOUT | WEEKEND_FLAT | NEWS_FLAT | mean hold |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 4 h | on | 50 | 12 | 8 | **30 (60.0%)** | 0 | 0 | 189.0 min |
| 24 h | on | 46 | 25 | 15 | **2 (4.3%)** | 3 | 1 | 443.1 min |
| 120 h | on | 44 | 24 | 15 | **0** | 4 (9.1%) | 1 | 531.1 min |
| 120 h | off | 43 | 25 | 15 | **3 (7.0%)** | — | — | 1,317.2 min |

Window B, same row: TIMEOUT **19 of 53 (35.8%)** at 4 h, **0 of 51** at 24 h
and at 120 h — the 24 h and 120 h rows are identical in every figure, so on
this window **24 h already releases the constraint completely**.

**Where the hold went instead.** With guards on, a 120 h ceiling does not buy
a 120 h hold: mean hold is 531.1 min (8.9 h) because
`flat_before_weekend_hhmm = 1640` closes the book each Friday — WEEKEND_FLAT
takes 9.1% of window A's exits at 120 h, and was the brief's own warning.
Turn the guards off and the mean hold goes to **1,317.2 min (22.0 h)** and
TIMEOUT returns on 7.0% of positions. So at 120 h the ceiling is the binding
constraint on **7%** of positions, against **60%** at 4 h.

## Swap: these are numbers from a world with no overnight financing

Every receipt's header reads `swap: long 0.00 / short 0.00 USD per lot per
night; spread 0.28`. That zero is **measured, not assumed**: the config records
340 closed positions on this account between 2026-04-21 and 2026-09-08, 87 of
them held overnight, swap 0.00 and commission 0.00 — the live Vantage account is
swap-free. `symbol_info` shows the generic rate of **−82.76 points = −$0.83 per
ounce per night** for an account that does pay it.

So the 24 h and 120 h arms are **unpriced for financing**, and must be read
that way. At the generic rate, the 120 h guards-off mean hold of 22.0 h is
about one night, ≈ **−$0.83 per lot**; a genuine 5-night hold would be ≈
**−$4.15 per lot**, against a **100 USD** book. On `vthr-vm20`'s realised stop
of 10.53 points, one night of swap is **7.9% of R** — larger than the 2.66% of
R the spread costs that row. Nothing here is repriced: repricing is a recorded
amendment, not a run-time edit. But no 5-day result on this axis should be
quoted to an account that pays swap without that number beside it.

## Percentiles, each with its own null median

Reported, never used to decide — the gate is the decision surface. The highest
percentile in the whole axis is `vbox-bb12` at **99th against a null whose own
p50 is 0.854** (window B, 120 h, guards off) on **2 trades**. The brief's
96-vs-8 pair on one rule at two sample sizes is exactly this: two trades is
not a reading. The only percentiles on a sample above 40 trades are
`vthr-vm20` at 68th / null p50 0.926 (A, 4 h), 24th / 1.050 (A, 24 h), 13th /
1.010 (A, 120 h), 36th / 0.992 (B, 4 h), 33rd / 1.046 (B, 24 h) and 34th /
1.062 (B, 120 h) — i.e. the row gets **further inside its own null** as the
ceiling rises.

## Falsifier: fired

The registration declared the axis dead if, at 120 h on both windows, every
row is under 40 trades **or** no cell clears the gate on both windows.

* **Clause two fired outright**: 0 of 76 cells cleared PF ≥ 1.200 **and**
  expectancy ≥ +0.050R on either window, let alone both. No receipt printed
  `SURVIVES` or `gate pass` anywhere.
* **Clause one fired for two of the three mechanisms** (`tsmom` 0 trades,
  `volman-box` ≤ 6) and did **not** fire for `volume-thrust`, which reached
  44 and 51 trades at 120 h. That row is the honest test, and it fails the
  gate at every ceiling on both windows.

**My own predictions, written before the runs:**

* Prediction 1 — `tsmom` identical at every ceiling: **confirmed**, and
  confirmed in the strong form by the diagnostic, which holds 66 h under a 4 h
  cap with no TIMEOUT.
* Prediction 2 — no row takes more trades at 120 h than at 4 h under the same
  guard setting: **confirmed on all 18 row-window pairs**.
* Prediction 3 — the low counts are set at the entry: **confirmed**.
  `volman-box boxBars=20/30` takes 0 trades in all sixteen of its cells (2
  rows × 4 arms × 2 windows), and the ceiling is only read in `check_exit`.

## Multiplicity ledger

**Declared 76** (54 main + 18 guards-off + 2 warmup diagnostic + 2 in the
dated amendment). **Looked at 76**: 8 runs × 9 rows + 4 runs × 1 row. No row
was added after a result, no threshold moved, no parameter raised. The
amendment that added the last 2 was written and committed before those two
ran, and says what it would decide either way.

## What this axis did NOT measure, and why

* **A `tsmom` cell on a window long enough for its warmup.** The axis fixes
  two three-month windows; `tsmom`'s shortest declared lookback needs 7,200
  bars of warmup and the windows hold 6,044 and 5,862. Measuring
  time-series momentum on `xauusd` needs a longer feed (the 15m store runs
  2022-06-16 → 2026-09-17, so it exists) or `xauduka`'s seven years. That is a
  different axis and it is **not measured here**. `null`, not 0.
* **Swap on a paying account.** The engine charges 0.00 because the account is
  measured swap-free. The arithmetic above (−$4.15 for five nights, 7.9% of R
  per night on `vthr-vm20`'s stop) is derived from the symbol's generic rate,
  **not run through the engine**.
* **A guards-off 4-hour control.** The guards-off arm was run only at 120 h,
  so the guards-on/off difference and the ceiling difference are not fully
  crossed. The guards-on arm is the one comparable to every published receipt,
  and the guards-off arm exists only to separate WEEKEND_FLAT from TIMEOUT at
  120 h.
* **`volume-thrust` across `riskReward`.** The record swept volMult ×
  riskReward, 9 cells; this registration took the volMult axis at the default
  riskReward 1.5, 3 cells. The other 6 are **not measured** at a lifted
  ceiling.
* **Any market but `xauusd`.** One market, one timeframe (15m), one feed.

## What did not move

Gate at the config's PF 1.2 / +0.05R / 30 trades. Spread 0.28, the configured
value, not the measured p50 of 0.220. Trail off. Null `exposure`, 200 seeds,
on every run. `config/default.toml` left at 4 h, so the control arm reproduces
every earlier receipt and
`crates/fd-backtest/tests/trading_rules.rs:76` — which asserts on purpose that
no market in the tracked file declares a `max_hold_ms` override — stays true.
No `cargo build`, `cargo test` or `cargo run`; `/e` held at 32 GB free before
and after. Calendar: 747 events from
`E:/rust/flowdesk/data/news/events.parquet`, the `--data=` root this run was
given and not the default `data/`. `data-sealed/` never opened.

## Recommendation

**Drop the "raise the ceiling and new mechanisms open up" direction.** The
ceiling was binding — on up to 100% of a row's positions — and removing it
produced no survivor, fewer trades, and a worse expectancy on the one row with
a readable sample. If `tsmom` is to be measured at all, the constraint to
attack is the **warmup against the window length**, not `max_hold_ms`.

One thing worth keeping from this axis regardless of the verdict: the
per-market override now exists as config and every receipt's header names the
horizon it ran under. Any future receipt at a non-default ceiling is therefore
self-identifying, which is what the 2026-09-24 note asked for.
