# Registration — the hold ceiling as the thing that killed three mechanisms

**Written and committed BEFORE the first `search.exe` run of this axis.**
Branch `agent/m2`, worktree `/e/rust/fd-a2`. Axis: `[trading] max_hold_ms`.

## The claim handed to me

`[trading] max_hold_ms = 14_400_000` (4 h) applies to every market with no
per-market override, and the three-month search of 2026-09-23 recorded:

    tsmom          0 trades at all 3 cells
    volman-box     <= 1 trade at all 3 cells
    volume-thrust  0-4 trades at all 9 cells

The claim is that the 4-hour ceiling silently killed these three, that they
only ever reported "0 trades" and never "refused by the ceiling", and that
nobody has measured them at a horizon they could live in.

## Hypothesis

Lifting the per-market ceiling to 24 h and to 120 h (5 days) on `xauusd`
raises the trade count of `tsmom`, `volman-box` and `volume-thrust` enough to
be measurable (>= 30 trades, the configured gate's floor), and at least one
cell clears PF >= 1.200 and expectancy >= +0.050R on BOTH windows.

## What I predict instead, and why — stated before running

I read the source before writing this, and I do not believe the claim. Three
reasons, each of which the runs below will either confirm or refute:

1. **`tsmom` cannot see the ceiling at all.** `crates/fd-strategy/src/tsmom.rs:69`
   returns `Exits::Strategy`. `engine::check_exit` (`engine.rs:736`) returns
   `None` immediately for a self-managed position, **before** the
   `max_hold_ms` branch at `engine.rs:760`. So no value of `max_hold_ms` can
   change a single `tsmom` trade. Its 0 trades have another cause, and the
   record already names it: `warmup()` is `(lookbackDays + 5) * 288` bars —
   7,200 bars at `lookbackDays = 20` — against 6,044 and 5,862 bars of 15m
   `xauusd` in the two windows. The warmup alone exceeds the window.
   **Prediction: `tsmom` trade counts are IDENTICAL at 4 h, 24 h and 120 h.**

2. **For `volman-box` and `volume-thrust` a higher ceiling can only LOWER the
   trade count, never raise it.** Both return `Intent::None` while
   `ctx.position.is_some()` (`volman_box.rs:80`, `volume_thrust.rs:65`), and
   `[trading.guards] max_concurrent_positions = 1`. A TIMEOUT exit at 4 h
   frees the book for the next entry; removing it keeps the position open
   longer, so entries are crowded out. The ceiling was, if anything,
   *inflating* their counts. **Prediction: counts at 24 h and 120 h are
   <= counts at 4 h, on every row and both windows.**

3. **Their low counts are set at the ENTRY, not the exit.** `volman-box`
   needs a box no taller than `maxBoxAtr = 1.5` ATR and a breakout bar no
   further than `maxBreakAtr = 1.0` ATR; `volume-thrust` needs volume
   >= `volMult` x the 20-bar mean AND range >= 1.0 ATR AND a close inside the
   top/bottom 20% of the bar AND no thrust on the bar before. `max_hold_ms`
   is read only in `check_exit`. No exit rule can manufacture a setup.

## Falsifier — specific, and able to fire

I declare the axis **dead** if, at the 120 h ceiling, on both windows:

* every row of all three mechanisms is still under **40 trades**; **or**
* no cell clears the gate (PF >= 1.200 **and** expectancy >= +0.050R) on
  **both** windows.

Either firing means the ceiling is not what was holding these mechanisms
back, and the desk may drop the whole "raise the ceiling and new mechanisms
open up" direction.

I declare my own **prediction 1** refuted if any `tsmom` row's trade count
differs between the 4 h and the 120 h run on the same window. I declare
**prediction 2** refuted if any `volman-box` or `volume-thrust` row takes
MORE trades at 120 h than at 4 h.

## Multiplicity — counted before the runs

Main arm, guards on, `--fixed`, `--exit-mix`:

| rows | ceilings | windows | cells |
|---:|---:|---:|---:|
| 9 | 3 (4 h / 24 h / 120 h) | 2 | **54** |

The 9 rows are the three-month search's own registry cells, pinned:
`tsmom` lookbackDays 20/60/120; `volman-box` boxBars 12/20/30;
`volume-thrust` volMult 2.0/2.5/3.5 at the default riskReward 1.5.
(The record swept 9 `volume-thrust` cells over volMult x riskReward; I take
the volMult axis only, at the method's own default riskReward, to keep this
registration's cell count defensible. That is 3 of the record's 9, not a
re-selection of the best.)

Diagnostic A — the same 9 rows at 120 h with **guards off**, both windows:
**18 cells**. Reason: `flat_before_weekend_hhmm = 1640` makes a 5-day hold
impossible across a weekend, so a guarded 120 h run measures WEEKEND_FLAT,
not the ceiling. This arm separates the two. Diagnostic: no verdict of
"survives" may be read off it, because every number the desk has published
was measured with guards on.

Diagnostic B — `tsmom` at `lookbackDays = 5` (warmup 1,440 bars, which fits
the window), 120 h, guards on, both windows: **2 cells**. It tests cause 1
above — whether warmup, not the ceiling, is what empties `tsmom`. Diagnostic:
`lookbackDays = 5` is not a time-series-momentum lookback that any paper
supports and no verdict may be read off it.

**Declared total: 74 cells (54 main + 18 + 2 diagnostic).** The ledger at the
end states the number actually looked at against this 74.

## How it will be read

* Gate: `profit factor >= 1.200` AND `expectancy >= +0.050R`. Not moved.
* Minimum sample: **40 trades**. Below that nothing is concluded, per the
  brief's 96-vs-8 percentile pair on one rule at two sample sizes.
* Both windows, same parameters, no re-fit: A `2025-07-01 -> 2025-10-01`,
  B `2025-04-01 -> 2025-07-01`.
* Null: `--null-sides=exposure`, 200 seeds. Every percentile quoted carries
  its own `null p50` beside it, because this desk's null median profit factor
  is 0.867 — below 1.000 — so a high percentile means "lost less than random
  entry at the same cost, count and exposure", not "made money".
* `--exit-mix` on every row of every run: the share of positions closed by
  STOP / TARGET / TIMEOUT / a guard label, and the mean hold in minutes. A
  TIMEOUT count of zero on a row whose mean hold is far under the ceiling is
  the direct evidence that the ceiling is not binding.

## Two things I must check on myself, and will state either way

**Swap.** `[markets.xauusd.trading] swap_long_per_lot = 0.0` and
`swap_short_per_lot = 0.0`. The config says this was MEASURED on the account's
own history (2026-04-21 -> 2026-09-08, 340 closed positions, 87 of them held
overnight, swap 0.00, commission 0.00): the live Vantage account is swap-free,
and `symbol_info` shows the generic rate of -82.76 points = -$0.83 per ounce
per night for an account that does pay it. So a 5-day hold measured here pays
**no overnight financing**. Every 24 h and 120 h number in this record is a
number from a swap-free world, and the `swap:` line of every receipt will read
`long 0.00 / short 0.00`. For an account that pays the generic rate, a 5-day
long on 1 lot of XAUUSD.sc would carry about **-$4.15** of swap (5 nights x
$0.83 per ounce per night, contract size 1 oz) — which against a 100 USD book
is not a rounding error. I will not reprice it here: repricing is a recorded
amendment, not a run-time edit. I will say in the report that the long-ceiling
arms are unpriced for financing.

**The weekend.** A ceiling longer than a weekend means the `WEEKEND_FLAT`
guard, not the ceiling, decides when a long hold ends. That is what
Diagnostic A is for, and `--exit-mix` is what shows it on every row.

## What I will not do

No `cargo build`, no `cargo test`, no `cargo run` — the binary is
`/e/rust/fd-wt-crt/target/release/search.exe` (built from `agent/crt`,
2026-10-04 15:13, the commit this worktree stands on, drift null included).
The per-market `max_hold_ms` override needs **no code**: `engine.rs:323`
already reads `spec.trading.max_hold_ms.unwrap_or(config.trading.max_hold_ms)`
and `search.rs:334` already prints which of the two a run used. This axis is a
config edit and nothing more.

`config/default.toml` of this worktree stays the 4-hour control so the control
arm reproduces every earlier receipt byte for byte; the two lifted ceilings
live in sibling config directories of this worktree, `config-h24/` and
`config-h120/`, each a copy of `config/default.toml` with one line added under
`[markets.xauusd.trading]`. `config/accounts.toml` outside this worktree is
not touched. Note for whoever runs the suite: adding the override to
`config/default.toml` itself would fail
`crates/fd-backtest/tests/trading_rules.rs:76`, which asserts on purpose that
no market in that file declares a `max_hold_ms` override, because doing so is
a live behaviour change. The sibling directories keep that assertion true.

## Pre-commitment

Nothing from this axis goes on the funded account, whatever it shows. A result
of "the ceiling was never the constraint" is the finding, and I will report it
as the finding rather than go looking for a cell to rescue it.
