# The rebate-optimal family is empty, and the identity says why before the data does

**Written:** 2026-10-06. **Agent:** n6, branch `agent/n6`.
**Registration:** `docs/decisions/2026-10-06-rebate-optimal-family.md`, committed
alone at `813301c` before the first run.
**Receipts:** `receipts/n6_R1.txt` … `receipts/n6_R12.txt` (12 runs, 264 rows).
**Status:** the registered falsifier **FIRED**. The family admits nothing.

## 1. The family

A mechanism whose gross edge is ~0 but whose net clears the gate on the
introducing-broker rebate, so that the optimum would be the mechanism trading
**most often at a gross profit factor near 1.0** rather than the one with the
best signal.

## 2. The gate: 0 cells, on both windows. 0 of 264.

**No row passes the gate (PF >= 1.200, expectancy >= +0.050R, >= 40 trades) on
both windows A and B, under either rebate model, in either guard arm.**

| | window A (2025-07-01 -> 10-01) | window B (2025-04-01 -> 07-01) |
|---|---|---|
| passes all three legs (engine) | 1 of 22 — `atr/hivol-london` only | **0 of 22**, all six runs |
| passes the gate, percentiles aside | keltner-break, rsi2-pullback, volume-thrust, atr/hivol-london, atr/hivol-orb60 (+ orb at 35 trades, under my floor) | squeeze-break; macd-cross (guarded arm only) |
| **intersection of the two windows** | **empty** | **empty** |

The engine's single pass is not a family member and is not a candidate:

- `atr/hivol-london`, window A: 40 trades, gross PF 2.449 -> net 2.504. The
  runner itself labels it **"passed gross too"** — the credit moved nothing.
  Its gross PF of 2.449 is nowhere near the declared band.
- It sits at **exactly 40 trades**, the brief's floor, and the brief's own
  sample-size figure is that the same rule at the same window gives
  `PF 1.753 / 14 trades` and `PF 0.682 / 178 trades`.
- **It fails window B in all six window-B runs**: net PF 1.060–1.095,
  net expectancy 0.021–0.027R, and 37–39 trades, under my 40-trade floor as
  well as under the gate.
- Its run prints `1 of 22 passed; noise at the 95th percentile yields about
  1.1 of 22 by luck` — at or below chance at this width.

### The family proper: 30 in-band cells, 0 crossings

Cells with gross PF inside the **pre-declared band `[0.95, 1.05]`** and >= 40
trades: **30 of the 264**, spread over `vwap-fade`, `bb-fade`,
`rsi-reversion`, `donchian-breakout`, `keltner-break` and `pdhl`.
**None crosses the gate, on either window, under either model, in either arm.**

The best in-band cell in all 264 is `n6/vwap-fade`, window A, unguarded,
spread 0.21, model B (per-lot):

        238 trades   gross PF 1.045 -> net PF 1.090   reb/R 1.68%
        net expectancy 0.038R        gate wants 0.050R

It misses the expectancy leg by **0.012R** and the profit-factor leg by
**0.110**. The largest credit anywhere in the band is `pdhl` on window B at
**reb/R 2.61% of R** — against a gate that asks **5.00% of R**. *The family's
biggest rebate is 52% of what the gate demands.*

## 3. The falsifier fired, and where

Registered: *the falsifier fires if no row with gross PF in `[0.95, 1.05]` and
>= 40 trades is carried across all three gate legs on both windows.*

**It fired twice over.** No in-band row crossed on *either* window, let alone
both; and no row of any kind crossed on both. The registration's weaker second
falsifier (identity (I) is wrong) did **not** fire — (I) held exactly.

So the desk's conclusion is the one the brief named as the falsifier's
meaning: **the rebate rescues cells already standing at the line and does not
create a family.** Measured, that rescue is real but tiny and never repeats
across windows:

| row | window | gross PF | net PF | what the credit did |
|---|---|---|---|---|
| `rsi2-pullback` | A, 233 trades | 1.159 | **1.214** | crossed the PF leg; holds under both models, both arms (147 trades guarded), both spreads |
| `macd-cross` | B, 237 trades | 1.188 | **1.220** | crossed the PF leg — **guarded arm only** |

Both were already within **0.041 and 0.012 of PF 1.200 gross**. Both are
outside the declared band. And they are *anti-correlated across the windows*:
`rsi2-pullback` is 0.932–1.073 on window B, `macd-cross` is 0.674–0.702 on
window A. The credit does not make a row survive out of sample; it makes a row
that already nearly passed in sample nearly pass by a hair more.

## 4. The identity, and why the axis was decidable before it was run

Registered before the run and confirmed by it. The credit is
`share x spread x lots x contract_size`; the risk is
`stop x lots x contract_size`. **Lots and contract size cancel:**

        reb/R  =  share x spread / stop  =  share x (cost/R)             (I)

(I) held exactly in every receipt. At share 0.45 / 0.90 / 0.225 on a probe row
the tool printed `reb/R` 1.81% / 3.62% / 0.91%; halving the spread halved it.
Consequence worth writing on the wall: **the rebate returns exactly `share` of
the spread cost, never more, whatever the mechanism, instrument, lot size or
frequency.** It is a 45% discount on spread and nothing else. Nothing about
trading more often changes the ratio.

Hence

        net_e  =  frictionless_e  -  (1 - share) x cost/R                (II)

Tightening the stop to trade more buys `1/share = 2.22x` as much cost as
rebate, so the requirement on the signal **rises** with frequency. The family
has an optimum only if `share >= 1.0`, or in per-lot terms only if
`per_lot >= spread x contract_size`.

**(II) is visible in the table, not just in the algebra.** Over the 15 rows
with >= 40 trades on window A unguarded, Pearson `r = -0.496` between `reb/R`
and gross PF, and at the extremes it is monotone. The only two rows whose
credit alone exceeds the gate's 5.00%-of-R requirement are **the two worst
gross rows in the whole table**:

| row | reb/R | cost/R | stop | gross PF | net PF |
|---|---:|---:|---:|---:|---:|
| `trend-pullback` | **9.26%** | 20.6% | **1.36 pts** | **0.637** | 0.742 |
| `doji-reversal` | **6.38%** | 14.2% | **1.98 pts** | **0.470** | 0.525 |
| `atr/hivol-london` (best gross) | 0.63% | 1.40% | 20.0 pts | 2.449 | 2.504 |

Stop sizes are quoted beside every cost figure as brief §7.1 requires;
they are recovered as `stop = share x spread / (reb/R)`, exact by (I), because
`--mode=rescore` prints no cost line (see §7).

## 5. The two rebate models: they separate, and they agree

`config/accounts.toml` implements `share_of_spread = 0.45` and records
`per_lot = 11.0` as *"NOT IMPLEMENTED, AND LEFT WRITTEN DOWN BECAUSE IT IS THE
LIKELIER TRUTH"*. A standard lot is 100x this workspace's gold contract, so
$11/standard lot is **$0.11 per lot here**. Because trades carry a constant
`rules.spread` (`engine.rs:723`), the per-lot model is *algebraically* a
share-of-spread model at `share = per_lot / spread` — **0.392857 at spread
0.28, 0.523810 at spread 0.21** — so the binary measured a model it does not
implement, exactly, with no hand arithmetic.

**They separate where the file says they do.** At spread 0.28 model A pays
more ($0.126 vs $0.110 a lot); at spread 0.21 model B pays more ($0.110 vs
$0.0945). Measured on the best in-band cell, `vwap-fade`:

| | model A (share 0.45) | model B ($11/std lot) |
|---|---:|---:|
| spread 0.28 | net PF 1.069, reb/R 1.92% | net PF 1.063, reb/R 1.67% |
| spread 0.21 | net PF 1.084, reb/R 1.45% | net PF **1.090**, reb/R 1.68% |

The crossover is real and it is now measured on both sides of 0.21.

**But the two models do not disagree about the verdict, and the axis is
therefore decided rather than left open.** The gap between the models in band
is at most **0.25 percentage points of R**; the shortfall to the gate is
**1.2 to 4.5 percentage points of R** — an order of magnitude larger. Both
models give 0 in-band passes on both windows. The models bracket the question
and both land far on the same side of it.

### The break-even rebate — the figure worth keeping

For the credit alone to meet +0.050R at a gross PF of 1.0, (I) requires
`share x spread / stop >= 0.05`. The family's own rows have stops of
**4.8 to 22.9 points** (from their printed `reb/R`), so the family needs

- **share 0.86 to 3.04** of the round-turn spread — from 86% of it to three
  times it; or equivalently
- **$24 to $85 per standard lot.**

The owner's own book records **$10–12 per standard lot**
(`docs/decisions/2026-09-13-close-reopen-drift.md`). **This family needs a
rebate 2 to 8.5 times better than any figure in the desk's records.** It is
not undecided for want of the owner's number: it is decided at every per-lot
figure the record contains, and reopens only at a figure 2x the best of them.

## 6. The rebate is a term, not an edge

**A line that lives only on the rebate is a line with counterparty risk, not a
line with edge.** The share is a commercial arrangement on an account. If the
broker changes it, the mechanism dies that day — and nothing in this measured
table would have warned of it, because the credit never touched a gross
column. One third to three fifths of the edge the gate demands coming from a
negotiated term is a *concentration*, not a discovery.

## 7. What I could NOT measure, and three tool defects

1. **`--mode=rescore` silently ignores `--exit-mix`.** The flag is read once,
   at `crates/fd-backtest/src/bin/search.rs:245`, and the printer lives at
   `:1208` inside the *hypotheses* runner; `run_rescore` (`:1246`) has no
   `exit_mix` parameter at all. So brief §8's standing requirement — always
   turn `--exit-mix` on and check the rule actually fires — **cannot be
   satisfied in this mode. Every receipt in this axis is a receipt without an
   exit mix, and so is every receipt in
   `docs/decisions/2026-09-23-rebate-rescore.md`.** This is a fifth instance
   of the flag-class defect brief §7.3 names, and it means I cannot rule out
   a `tsmom/120d`-style row whose own rule never fired.
2. **`--mode=rescore` prints no `cost ... % of R` line**, so brief §7.1 cannot
   be followed directly. Worked around exactly via (I); the stops in §4 are
   derived, not printed.
3. **Most percentiles in this mode are unmatched.** 15 of 22 rows in R1 carry
   `** outside the band: this percentile is unmatched **`. Percentiles are not
   a gate (brief §5) so no verdict here rests on one, and per §5 the matched
   null's own p50 is quoted beside every one: for `vwap-fade` R1 the matched
   null is **p50 0.890 / p95 1.161 gross, p50 0.935 / p95 1.238 net** over 200
   runs, so its "82nd percentile" means *loses less than 82% of random entries
   at the same cost*, not *profitable*.
4. **Slippage and platform fees are not modelled.** This family trades
   hundreds of times per quarter and would pay both. **Every net figure here
   is an upper bound** and the true figures are worse.
5. **Four rows cannot be asked the question on these windows.** On window A
   `gap-fade`, `volman-box` and `tsmom` take **0 trades** and
   `ict-sweep-mss-fvg` takes 2. `tsmom`'s zero is **warmup**, per brief §7.2,
   not the hold ceiling. These are 1m/5m methods on 15m bars, as
   2026-09-23 already found; the finding is reproduced, not new.
6. **A 13th guards-only gate crossing.** `macd-cross` on window B crosses the
   PF leg in the guarded arm (net 1.220) and fails in the unguarded one (net
   1.155), on 237 vs 240 trades. Guards also moved `rsi2-pullback` from 233 to
   **147** trades on window A (-37%) and `stoch-reversal` from 302 to **330**
   (+9%). Brief §8's count stands and grows.

## 8. Multiplicity ledger

| | declared | actually viewed |
|---|---:|---:|
| (run, row) cells | **264** (12 runs x 22 rows) | **264** — every receipt, every row |
| non-hypothesis tool probes | 4, declared in registration §2 | 4 (`smoke-test.toml`: shares 0.45/0.90/0.225, spread 0.14) |

No threshold was moved, no window substituted, no sample floor lowered. The
band `[0.95, 1.05]`, the 40-trade floor and the both-windows rule are as
registered. The only reading added after the fact is the §3 table of
credit-moved rows, which is reported **as a failure of the family** (both rows
are outside the band and neither survives both windows), not as a candidate.

## 9. Agreement and disagreement with the record

`docs/decisions/2026-09-23-rebate-rescore.md` got 0/22 on a different year at
share 0.45 only and concluded *"a 45% refund on a cost you pay in full cannot
turn a losing method into a winning one."* This axis **agrees and sharpens it
into an identity**: the refund is exactly `share` of `cost/R` by (I), so the
claim needs no measurement at all — and adds the per-lot model, both brief
windows, both guard arms, and the break-even figure of **$24–85 per standard
lot against a recorded $10–12**.

No merge to `main`. `config/accounts.toml` untouched outside this worktree.
