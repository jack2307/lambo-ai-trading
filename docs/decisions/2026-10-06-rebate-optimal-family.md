# Registration — the rebate-optimal family: gross edge ~0, net positive on the rebate

**Written:** 2026-10-06, before the first run of this axis.
**Agent:** n6, branch `agent/n6`, worktree `/e/rust/fd-b6`.
**Status:** REGISTERED. Nothing below this line may be rewritten; corrections
are appended as dated notes at the end.

## 1. The family, in one sentence

A mechanism whose **gross** edge is indistinguishable from zero but whose
**net** result clears the gate because the introducing-broker rebate is paid
per trade rather than per correct guess — so the optimum would be the mechanism
that **trades the most at a gross profit factor near 1.0**, a criterion by
which this desk has never once ranked a scan.

## 2. The prediction I am registering, and the arithmetic behind it

I derived a closed form before running, and I am registering it as the
prediction rather than as a result, so that the run can contradict it.

The credit on one trade is `share x spread x lots x contract_size` (one
round-turn spread, `crates/fd-backtest/src/rebate.rs`). The risk on one trade
is `stop x lots x contract_size`. **Lots and contract size cancel**, so

        reb/R  =  share x spread / stop        and        cost/R  =  spread / stop

        =>   reb/R  =  share x (cost/R)                                  (I)

(I) is an identity, not an estimate. It was checked against the tool before
registering, on a row outside this axis' universe
(`docs/hypotheses/smoke-test.toml`, window A): at share 0.45 the tool prints
`reb/R 1.81%`, at 0.90 it prints `3.62%` and at 0.225 it prints `0.91%`
(exactly 2x and exactly 1/2); halving the spread to 0.14 at share 0.45 prints
`0.92%`. Linear in share, linear in spread, as (I) requires.

From (I), with `e` for expectancy in R:

        gross_e  =  frictionless_e  -  cost/R
        net_e    =  frictionless_e  -  (1 - share) x cost/R              (II)

**(II) is the whole axis.** `1 - share > 0`, so every point of cost/R bought by
tightening the stop to trade more often still costs `(1 - share)` of itself
after the refund. The requirement the signal must meet,
`frictionless_e >= 0.05 + (1 - share) x cost/R`, therefore **rises** as the
mechanism trades more. "Optimise for the rebate" has an optimum only if
`share >= 1.0` — a full refund of the round-turn spread — or, in the per-lot
model, only if `per_lot >= spread x contract_size`.

**So my registered prediction is that the falsifier fires**, and the figure I
expect to publish is the break-even rebate rather than a surviving cell. I
state this now so that a surviving cell would be a genuine surprise and not a
threshold I moved.

## 3. Falsifier — specific, and it can fire

**The falsifier fires if** no row in the universe below, on **both** windows A
and B, has a gross profit factor inside the declared near-1.0 band **and** is
carried across all three gate legs by the credit.

**The falsifier does NOT fire** (and the axis opens something) if at least one
row with gross PF in `[0.95, 1.05]` and >= 40 out-of-sample trades reaches
`PF net >= 1.200` **and** `expectancy net >= +0.050R` on **both** windows,
under **at least one** of the two rebate models.

The band is declared here, before any run: **gross PF in `[0.95, 1.05]`**, and
"trades the most" is read as the trade-count ordering of the run's own
out-of-sample count column, reported in full and not filtered.

A second, weaker falsifier that can fire independently: identity (I) is wrong,
i.e. the printed `reb/R` is not `share x spread / stop`. If (I) fails, section
2 is void and the axis must be decided by measurement alone.

## 4. Multiplicity, declared before the first run

**Universe:** the 22 rows of `docs/hypotheses/2026-09-23-rebate-rescore.toml`,
copied **verbatim** (`base`, `filters`, `overrides` unchanged; only `label`
re-prefixed `n6/`), because `--mode=rescore` reads only `--batch-file=` and
cannot call a built-in batch. Those 22 rows are the desk's own 29 closed
constructs and they carry the highest out-of-sample trade counts the record
has (stoch-reversal 1297, bb-fade 994, macd-cross 923, vwap-fade 860,
trend-pullback 687 on the 2025-09/2026-09 year), which is exactly the end of
the distribution this axis asks about. **No new mechanism is written** — the
brief forbids that and this axis does not need it: the family is a *selection
criterion over the existing universe*, not a new rule.

**Runs:** 12, each the same 22 rows.

| # | window | guards | spread | --rebate-share= | what it is |
|---|---|---|---|---|---|
| R1 | A | off | 0.28 | 0.45 | model A (share of spread), as configured |
| R2 | A | off | 0.28 | 0.392857 | model B, $11/standard lot |
| R3 | A | on | 0.28 | 0.45 | model A, guarded arm |
| R4 | A | on | 0.28 | 0.392857 | model B, guarded arm |
| R5 | B | off | 0.28 | 0.45 | |
| R6 | B | off | 0.28 | 0.392857 | |
| R7 | B | on | 0.28 | 0.45 | |
| R8 | B | on | 0.28 | 0.392857 | |
| R9 | A | off | 0.21 | 0.45 | model A at the logger's measured median |
| R10 | A | off | 0.21 | 0.523810 | model B at 0.21 — **where the two models disagree** |
| R11 | B | off | 0.21 | 0.45 | |
| R12 | B | off | 0.21 | 0.523810 | |

**Declared cell count: 12 x 22 = 264 (run, row) cells.** Every cell is
reported; the trade-count ordering is a *reading* of the full table and not a
filter applied before reporting.

**How model B is expressed as a share.** `config/accounts.toml` records
`per_lot = 11.0` as NOT IMPLEMENTED and likelier true; a standard lot is 100x
this workspace's gold contract, so $11/standard lot is **$0.11 per lot here**.
Because trades carry a constant `rules.spread` (`engine.rs:723`), a flat
per-lot credit is *exactly* a share-of-spread credit at
`share = per_lot / spread`: **0.11/0.28 = 0.392857** and
**0.11/0.21 = 0.523810**. This is an algebraic re-expression of the same
money, not an approximation, and it is why the binary can measure a model it
does not implement. The models therefore coincide by construction at whatever
spread the share is solved for, and separate at any other — which is the
disagreement `accounts.toml` warns about, now measured on both sides of it.

## 5. How it will be read

- **Gate (section 5 of the brief, not adjustable):** `PF >= 1.200` AND
  `expectancy >= +0.050R` AND **>= 40 trades**. The engine's own
  `PromisingGate` carries `min_trades = 30`; **the 40-trade floor is applied
  by me on top of the engine's verdict**, and any row passing the engine at
  30-39 trades is recorded as a FAIL on trades.
- **Both windows, or nothing.** A pass on one window is not a pass.
- **Percentiles are not a gate** and are quoted with the matched null's own
  p50 beside them, which the runner prints.
- **Both guard arms are reported** (brief section 8); `--exit-mix` is on for
  every run, and a row whose own rule never fires is named as such.
- Seeds and direction draws stay at the runner's defaults (200 / 1000), the
  values the 2026-09-23 receipts were read at.

## 6. What this axis cannot settle, declared in advance

1. **Slippage and platform fees are not modelled.** A mechanism that trades
   more pays more of both. Every net figure here is an **upper bound**.
2. **The rebate is a term, not an edge.** If the broker changes the share, the
   mechanism dies the same day. A line that lives only on the rebate carries
   counterparty risk, not edge.
3. **The per-lot figure is not the owner's.** $11/standard lot is the
   record's own estimate. If the two models reach different verdicts, this
   axis is UNDECIDED until the owner supplies the real per-lot number, and
   the report must say so rather than pick a side.

## 7. Prior art this axis must not be confused with

`docs/decisions/2026-09-23-rebate-rescore.md` rescored these same 22 rows and
got 0/22, on the **2025-09-13 -> 2026-09-12** year, at share 0.45 only, spread
0.28 only, one guard arm. This axis differs in four measured ways: **windows A
and B** of the brief (never rescored), **both rebate models**, **both guard
arms**, and **the trade-count ordering** rather than the profit-factor
ordering. Where it agrees with 2026-09-23 it says so.
