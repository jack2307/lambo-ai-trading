# instr-repair — two measured instrument defects, repaired: result

Registration: `docs/decisions/2026-10-07-instrument-repair.md`, committed alone
at `c26b1f1` before the first line of code.
Branch: `agent/instr-repair`, worktree `E:/rust/fd-instr-repair`, cut from
`agent/n5`.
Binary: `target/release/search.exe` built in this worktree with
`cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin search`
(md5 `c2cc0592c565e45553d356617dab3edb`). Private target dir, no shared
`CARGO_TARGET_DIR`.
`--data=/e/rust/flowdesk/data`, read-only. **`data-sealed/` was not opened,
read, pointed at or counted.** `news:` source, identical in all five receipts:
**747 events (2010-01-08 -> 2027-12-08) from
`E:/rust/flowdesk/data\news\events.parquet`**, scope USD.

## 0. This axis has no gate

**0 gate cells spent.** No cell was read against PF >= 1.200 / expectancy
>= +0.050R / >= 40 trades, and no percentile here is a result. One row is
re-run with its published figures as the oracle. Everything below is an
instrument test.

## 1. Defect (i) — `expectancy` is blind to financing. Repaired, additively.

`close_position` now records, beside the fields it always did:

* `Trade::risk_usd` — one R in USD, `risk x lots x contract_size` at the fill;
* `Trade::r_net` — `pnl_usd / risk_usd`, i.e. `r` **with commission and swap in
  it**, computed as `points / risk + (swap - commission) / risk_usd` so that a
  trade paying neither has `r_net` bit-identical to `r`;
* `Metrics::expectancy_net` and `Metrics::total_r_net` — the same two
  statistics over `r_net`, and `NaN` (never 0) if any trade in the set cannot
  report one.

`r`, `expectancy`, `total_r` and `profit_factor` keep their exact definitions.
The new line prints only where it can differ — a row with `swap$ != 0` or a
non-zero `commission_per_lot`.

### The measurement: one row, two swap arms

`qs-h15` (`quiet-swing`, `holdSessions = 15`), `xauduka` 15m,
2010-06-01 -> 2018-06-01 (190,889 of 378,749 bars), no guards, `--fixed`,
`--null-sides=exposure --seeds=200`. Arm A reads `config/` (swap 0.00, the
desk's measured swap-free account); arm B reads `config-swap/` (-0.83 USD per
lot per night, charged both ways as an upper bound).

| | arm A, swap 0.00 | arm B, swap -0.83 | separated? |
|---|---|---|---|
| trades | 190 | 190 | — |
| profit factor | **1.262** | **0.594** | yes (and always did) |
| `expectancy` | **+0.130 R** | **+0.130 R** | **NO — identical to 3 decimals** |
| `total_r` | +24.61 R | +24.61 R | **NO** |
| swap$ | 0 | **-6166** | yes |
| **`expectancy_net`** | **+0.130 R** (by identity) | **-0.265 R** | **YES** |
| **`total_r_net`** | +24.61 R (by identity) | **-50.42 R** | **YES** |

The gap arm B prints on its own line is **-0.395 R per trade** of financing.
`expectancy` says this row made +0.130 R a trade in both arms; `expectancy_net`
says it made +0.130 R in one and **lost 0.265 R** in the other, and the sign
flip is what the old column could not express.

Arm A's receipt is **byte-identical** to `receipts/n5/A-duka-is.txt`'s `qs-h15`
block over all seven of its lines — trade count, PF, expectancy, both null
quantiles, count match, long share, spread paid, exposure and exit mix. Arm B's
differs from `receipts/n5/B-duka-is.txt` by **exactly one inserted line**, the
`expectancy_net` line. **No published number moved.**

Receipts: `A-swap0.txt`, `B-swapcharged.txt`.

### F1, the correctness falsifier: did not fire

`crates/fd-backtest/src/engine.rs`, `mod net_r_tests` — four tests at
`--release`:

* `r_net` is `r` **bit-identical** (`to_bits()`) when swap and commission are
  zero, and so are `expectancy_net` and `total_r_net`;
* `r_net` falls and `r` does not when one night of financing is charged
  (0.500 R -> 0.417 R on a -1.66 USD swap over a 20.00 USD risk unit);
* `r_net` also sees commission (0.500 R -> -0.100 R at 3.00 USD per lot round
  turn);
* a set holding one trade without a risk unit reports `NaN`, not 0.

## 2. Defect (iv) — `--mode=rescore` dropped `--exit-mix` in silence. Repaired.

`run_rescore` now takes `exit_mix` and prints it, and `RescoreRow` carries the
`control_stop` that `rescore_hypothesis` had been computing and discarding, so
the mode prints its `cost ... % of R` line like every other scoring mode.

Measured first, as the brief asks: the two published rescore receipts
(`docs/research/runs/2026-09-23-rebate-rescore/`, 22 constructs each) contain
**no `exits:` line and no `cost ... % of R` line**, confirming they are
receipts with no exit mix whether or not the flag was passed.

Format evidence, `E-rescore-costR.txt` (`ema-cross`, a stop-based base, 2 years,
`--seeds=10 --direction-samples=20` — **a format probe, not a reading**):

    cost-matched null: control stop 1.500 ATR = 1.78 points, cost 15.74% of R (named stopAtr, copied)
    the method's own realised stop: median 1.619 ATR = 1.92 points over 789 trades
    exits (walk-forward, out of sample): END_OF_DATA 1, STOP 178, TARGET 108, TIMEOUT 110, opposite cross 10; mean hold 249.8 min
    exits (whole window, what the direction null is a percentile OF): STOP 384, TARGET 169, TIMEOUT 201, opposite cross 35; mean hold 168.7 min

On `qs-h15`, which manages its own exits and therefore has no stop,
`D-rescore-exitmix.txt` prints

    cost/R: NOT MEASURABLE on this row — it manages its own exits, so it has no stop to divide the spread by

which is the §7 rule: not measurable is not 0%.

## 3. The mechanism against the whole defect class

`search.rs` now declares every flag it reads and which `--mode=` values read it
(`ALWAYS`, `BY_MODE`), and prints the audit in the receipt header before any
number. Caught in the act, `C-flag-audit.txt`:

    flags:    ** --direction-samples IS NOT READ BY --mode=hypotheses — only by --mode=rescore — so nothing below was changed by it **
    flags:    ** --rebate-share IS NOT READ BY --mode=hypotheses — only by --mode=rescore — so nothing below was changed by it **
    flags:    ** --exitmix is not a flag this binary has; it changed nothing in this run **

and, when nothing is ignored, the positive statement a reader can rely on:

    flags:    12 passed, every one of them read by --mode=hypotheses

**The staleness guard is the mechanism, not the warning.** A test scans the
binary's own source for every flag literal it reads (`arg("x"`, `"--x="`,
`== "--x"`) and fails if one is absent from the table, and a second test fails
if the table names a flag no literal reads. Adding a flag read without
declaring who reads it now breaks the test suite.

### A SIXTH case of the class, found by the mechanism on its first run

`D-rescore-exitmix.txt`, header:

    flags:    ** --null-sides IS NOT READ BY --mode=rescore — only by --mode=hypotheses — so nothing below was changed by it **

`--null-sides=` is parsed and **printed in the header of every mode**, but only
`run_hypotheses` passes it to a null; `rescore_hypothesis` keeps the coin-flip
null it published on purpose. So a rescore receipt carrying
`null sides: exposure - ...` in its header was **claiming a control it did not
run**. Nothing is restated here — the header line is the one every rescore
receipt already carried — but from this commit the same receipt also says the
flag was ignored.

A **seventh** is recorded and deliberately not repaired, in the registration's
amendment: `arg()` returns the FIRST match, so a duplicated flag silently uses
the earlier value.

### An EIGHTH, handed in mid-job by the coordinator — recorded, not repaired, 0 cells

`agent/news-tf` measured that **`null p50` prints `0.000` where the true state
is "the control could not be calibrated"** — §7's own `null != 0` rule broken by
the instrument. Two adjacent windows, same row, same binary: at **69** admitted
events the count match is 1.13-1.17 and the null is real; at **70** it is
**0.03-0.04** — the control drew **2** trades against the method's **61** — and
the receipt prints **`null p50 0.000`**, which reads as "the median control made
nothing" when it means "there is no distribution to take a median of".

Not repaired here: it is a third surface in a job registered for two, it would
invalidate all five receipts above, and it costs a full release test cycle on a
disk that touched 9.5 GB free during this job against an 8 GB floor. Full
numbers are in the registration's amendments. **Nothing was run for it: 0 cells,
and the 69-vs-70 measurement is `agent/news-tf`'s, not reproduced here.**

## 4. Multiplicity ledger

| | declared | spent |
|---|---|---|
| gate cells | **0** (this axis has no gate) | **0** |
| instrument-test rows | 1 row x 2 swap arms | 2 |
| header/format probes, explicitly not readings | — | 3 (`C`, `D`, `E`) |

## 5. What was NOT touched

`wrong_side_stop` (brief 4(vi)) — left exactly as it is, the owner's decision.
`r`, `expectancy`, `total_r`, `profit_factor`, `usd_per_r`, the golden parity
files, `config/accounts.toml` outside this worktree, `config/local.toml`,
`data-sealed/`, the two `collect.exe` processes,
`/e/rust/flowdesk/target/release/`, and `main`.

## 6. What was NOT measured, and why

* **Whether `expectancy_net` changes any verdict in the record.** It cannot:
  every published receipt of this workspace ran at `swap = 0` and
  `commission_per_lot = 0.0`, where `r_net == r`. The only rows it can move are
  rows that charge financing or commission, and the only such rows that exist
  are `agent/n5`'s arm B, which already failed on PF.
* **`fd-api` was not compiled.** Its test fixture gained the two new fields
  (`crates/fd-api/src/paper.rs`, `mod carried_basis`), but building that crate
  pulls the axum/reqwest tier that brief §1(b) says cost 11 GB of
  `target/debug` last night, so it was not built. The change is two `None`
  fields on a struct literal; it is unverified by a compiler, and said so here
  rather than claimed.
* **No out-of-sample leg, no second window, no percentile is read**, by design:
  nothing here is a claim about a market.
