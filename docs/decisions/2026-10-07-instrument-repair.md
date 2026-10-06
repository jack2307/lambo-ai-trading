# 2026-10-07 — two measured instrument defects, repaired: `r` cannot see financing, and `--mode=rescore` drops flags in silence

**Registered:** committed ALONE, before the first line of code on this branch.
**Branch:** `agent/instr-repair`, worktree `E:/rust/fd-instr-repair`, cut from
`agent/n5`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-NIGHT.md`, section 4(i) and 4(iv).
**Status:** registered.

## 0. This axis has NO GATE and spends NO cells

Nothing here is a hypothesis about a market. No method is run to see whether it
passes PF >= 1.200 / expectancy >= +0.050R / >= 40 trades, no percentile is
read, and the multiplicity ledger of this branch is **0 gate cells**. One
single row is re-run, and it is re-run as an INSTRUMENT TEST: its known numbers
are the oracle, and the new figure is checked for separating two arms the old
figure could not. If that row's gate figures moved at all, the patch is wrong.

The value of the axis is that every measurement AFTER it is readable. It is
not a search for a method.

## 1. Defect (i), as measured by `agent/n5`

`close_position` (`crates/fd-backtest/src/engine.rs`) books

    points   = exit - entry            (both already shifted by half the spread)
    pnl_usd  = points x lots x contract - commission + swap
    r        = points / risk

and `metrics_of` sets `expectancy = mean(r)`, `total_r = sum(r)`. So **spread
is inside `r`** (it moved the fill prices) while **commission and swap are
not**. `profit_factor` reads `pnl_usd` and sees all three.

The measured consequence, `receipts/n5/A-duka-is.txt` vs `B-duka-is.txt`, row
`qs-h15` (`quiet-swing`, `holdSessions = 15`), `xauduka` 15m,
2010-06-01 to 2018-06-01, 190 trades, no guards:

| arm | swap/lot/night | PF | expectancy | swap$ |
|---|---|---|---|---|
| A | 0.00 | **1.262** | **+0.130 R** | 0 |
| B | -0.83 | **0.594** | **+0.130 R** | **-6166** |

Expectancy is identical to three decimals across a $6,166 financing charge.
**Half the gate cannot see the largest cost of an overnight-holding mechanism.**

## 2. The repair, and why it is ADDITIVE

Changing `r` would rewrite every published receipt and every golden parity
file. It is therefore NOT changed. Following the precedent of
`2026-09-21-restated-pnl-contract-size.md` (a new carried field, old meaning
untouched):

1. `Trade` gains **`risk_usd: Option<f64>`** — `risk x lots x contract_size` at
   the fill, the USD the position was SIZED under. `#[serde(default)]`, so a
   trade booked before the field exists reads `None`, which is "not known" and
   is never 0.
2. `Trade::r_net()` returns `pnl_usd / risk_usd` — the same result in the same
   unit, **with commission and swap in it**. It is `None` when `risk_usd` is.
3. `Metrics` gains **`expectancy_net`** and **`total_r_net`**, computed from
   `r_net`. `NaN` — not 0 — when any trade in the set cannot report one.
4. `r`, `expectancy`, `total_r`, `profit_factor` and every other existing field
   keep their exact current definitions and values.

**Default OFF, in the only sense that matters for a receipt:** the new figures
print only on a row where `swap$ != 0` or `commission_per_lot != 0`. On a
zero-swap, zero-commission run — which is every published receipt of this
workspace, `commission_per_lot = 0.0` in `config/default.toml` — the printed
format is unchanged and `r_net == r` identically.

## 3. Falsifier for the repair — specific and firable

**F1 (correctness).** A unit test asserts `r_net == r` exactly when
`swap = 0` and `commission = 0`, and `r_net < r` when swap is charged against
the position. If `r_net != r` in the costless case, every published receipt
would be restated and the patch is rejected.

**F2 (the only test that proves the patch is worth anything).** Re-run exactly
one row — `qs-h15` on `xauduka` 15m, 2010-06-01 to 2018-06-01, no guards,
`--fixed`, arm A (swap 0.00) and arm B (swap -0.83) — and require:

* `trades`, `profit_factor` and `expectancy` **reproduce n5 to the printed
  digit** in both arms (190 / 1.262 / 0.130 and 190 / 0.594 / 0.130). Any
  movement means the patch changed a published number and FIRES F2.
* `expectancy_net` **differs between the two arms** by a margin larger than
  the printing precision, while `expectancy` stays identical.

If `expectancy_net` is also identical across the two arms, the new quantity is
as blind as the old one and the repair has failed.

## 4. Defect (iv), as measured

`--mode=rescore` takes no `exit_mix` parameter (`search.rs`, `run_rescore`),
so `--exit-mix` passed to it is dropped **in silence**, and it never prints the
`cost ... % of R` line that the hypotheses branch prints from
`report.control_stop`. `rescore_hypothesis` already COMPUTES that control stop
and simply does not carry it out on `RescoreRow`.

Consequence, measured: every receipt of `2026-09-23-rebate-rescore.md` and of
`agent/n6` is a receipt with **no exit mix**, so no reader of them can rule out
a `tsmom/120d`-shaped row whose own rule fired 0 times (defect 4(iii)).

**Repair:** `run_rescore` takes `exit_mix` and prints the same two things the
hypotheses branch prints — the exit mix with the mean hold, and the
cost-matched control stop with `cost X% of R`. `RescoreRow` carries
`control_stop`.

## 5. The fifth case of a class — and the mechanism against it

Known cases of "a flag a mode ignores in silence": `--exit-mix` and the
`cost % of R` line in `rescore`; `--samples=` and `--direction-samples=` not
read under `--mode=hypotheses`; `--rebate-share=` read ONLY in `rescore`.

**Repair:** `search.rs` declares, in one table, every flag it reads and which
`--mode=` values read it. Before any mode runs, it compares the flags actually
passed against that table and prints, in the receipt header:

* one loud line per passed flag the current mode does not read, naming which
  modes do read it;
* one line per argument that is not a flag this binary has at all.

A receipt therefore **denounces itself** for every flag it ignored, and a
reader of an old receipt can tell the two cases apart by whether the line is
there.

**The staleness guard is the point, not the warning.** A test scans the binary's
own source for every flag literal it reads (`arg("x"`, `"--x="`, `== "--x"`) and
fails if one is missing from the table. Adding a flag read without declaring who
reads it stops the test suite being green. That is what makes this class of
defect unable to recur in silence, rather than merely documented once more.

## 6. What is NOT touched

* `wrong_side_stop` (brief 4(vi)) — the owner's decision, left exactly as it is.
* `r`, `expectancy`, `total_r`, `profit_factor`, `usd_per_r`, `Rebate::credited`
  and the golden parity files.
* `config/accounts.toml` outside this worktree, `config/local.toml`,
  `data-sealed/`, the two `collect.exe` processes and
  `/e/rust/flowdesk/target/release/`.
* `main`.

## 7. Build and test discipline, declared

`cargo +stable-x86_64-pc-windows-gnu`, `CARGO_TARGET_DIR` private to this
worktree, tests at `--release` and only `-p fd-backtest -p fd-core`.
`df -h /e` before and after, 8 GB floor, checked mid-way.

## 8. Amendments

Dated notes appended below. No line above is rewritten.
