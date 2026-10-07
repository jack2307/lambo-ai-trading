# Registration: is there anything left of a long-horizon signal if it is only ever expressed INSIDE a session and flat across every 17:00 New York rollover?

Registered **2026-10-07**, branch `agent/rollover-flat` (cut from
`agent/instr-repair`). Committed **alone**, before the first line of strategy
code and before any `xauduka` number exists on this branch.

Brief: `/e/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` plus
`/e/rust/AGENT-BRIEF-ADDENDUM-5.md`. Both apply in full.

## 1. Hypothesis, one sentence

The long-horizon signals in this record (`tsmom`'s trailing N-day return sign,
`quiet-swing`'s quiet-tape K-session continuation) carry a gross edge that
survives being expressed as a **daily, in-session position that is flat across
every 17:00 New York rollover**, and the round-trip spread of re-entering every
session costs less than the financing the continuous expression pays.

## 2. Why this and not something else — the two measured numbers it stands on

`agent/pure-drift` (`receipts/pure-drift/RESULT.md`, section 4) measured, from
receipt-internal figures only, that financing is charged **per 17:00 New York
crossing, not per hour held**: `swap$/spread$` spans **0.014x -> 11,139x**
across eight rows that are all gold exposure on one instrument, and `px-1s`
holds **65.7% of the calendar clock** while paying **3-5%** of a continuous
book's financing, because a book flat 16:00-18:00 New York crosses no rollover.

The consequence it also measured, with `expectancy_net`:

    ts-l60   expectancy +0.370R  ->  expectancy_net -0.271R   (0.641R of carry)
    qs-h15   expectancy +0.130R  ->  expectancy_net -0.179R   (0.309R of carry)
    px-1s    expectancy +0.012R  ->  expectancy_net +0.011R   (0.001R of carry)

That agent declared the present question **NOT MEASURED**: whether `qs-h15` or
`ts-l60` would keep any of their arm-A edge if re-expressed as
flat-across-17:00 variants — "a different mechanism and it is not in this plan".

## 3. The trade-off, and the arithmetic that says it is worth measuring

**Given up:** one round trip per session instead of one per hold. `ts-l60`
holds **651 h** (~27 sessions) per trade, so ~27 round trips replace 1.

**Cost of a round trip:** per addendum section F, on the right ruler for these
mechanisms — **1.5 mean New York-day ranges over 20 days**, NOT 1.5 ATR(15m) —
`agent/pure-drift` measured **1.10% of R (IS) / 0.85% of R (OOS)**. So
~27 x 0.85-1.10% = **23-30% of R** per original-trade-equivalent.

**Bought back:** the whole carry — **0.641R** on `ts-l60`.

0.641R against ~0.27R is **not hopeless on paper**. It is also the brief
owner's mental arithmetic over two medians and **not a measurement**; per brief
section 8, if the measured numbers disagree with it, the measured numbers win
and this registration says so in section 13.

**What is also given up, and is NOT in that arithmetic:** the window
18:00-16:00 New York skips **2 of the session's 24 hours** (16:00-18:00), and
those two hours contain the daily gap. `agent/pure-drift` measured the drift
density of the hours the window DOES keep as **+1.115 R / 1,000 h** overnight
(18:00-09:30) and **+0.509** in the New York day on the 2018-2026 leg, so the
hours kept are the dense ones — but the gap itself is unmeasured and the flat
variant forgoes it. A declared, directional unknown, not a hedge.

## 4. The mechanisms, stated before they are written

Two new strategies. Both `Exits::Strategy` with an **empty grid**, so
`hypotheses::control_for` builds the `RandomHold` drift null — the same control
family `px-1s`, `qs-h15` and `ts-l60` were measured against, which is what
makes the comparison a comparison. Both carry `from`/`to`, so the control's
hold is the **window span** rather than a guess.

**`tsmom-flat`.** At the first bar at or after `from` New York each day: side =
sign of the trailing `lookbackDays` return, read exactly as `tsmom` reads it
(the close of the last bar on or before the same minute `lookbackDays` New York
days earlier). Enter with `tsmom::sizing_stop` (`riskDailyRanges` x the mean New
York-day range of the last `rangeDays` days, sizing only, never enforced). Exit
at the first bar outside the window. No mid-window flip: a flip inside the
window would be a second mechanism.

**`quiet-swing-flat`.** Same window and same sizing. The side is ON for session
S iff there is an `m` in `0..holdSessions` such that (a) at session `S-m` the
trailing `lookbackSessions` true-range sum sat strictly below the `quietPct`
quantile of the previous `windowSessions` values of that same statistic, and
(b) the sign of the trailing `lookbackSessions`-session return is the **same**
at every session from `S-m` to `S`. Side = that sign.

That is a **stateless, causal reconstruction** of `quiet-swing`'s hold clock
and flip exit, not a bit-identical one: the original restarts its clock only on
a fresh entry after an exit, and this rule restarts it whenever the quiet
condition fires again inside a run. **Declared as an approximation here, before
any number**; the consequence is that `quiet-swing-flat` can be ON for longer
runs than `quiet-swing` would hold, and the trade count is the thing to read
for that.

Everything either strategy reads comes from `quiet_swing::completed_sessions`
(which discards every bar of the session in progress and the oldest session it
reaches) or from a backward scan over `bars[..=i]`. Nothing after `i` exists to
be read. A unit test asserts the intent sequence over a truncated series is a
prefix of the sequence over the full one, as `tests/causality_quiet_swing.rs`
does for the parent.

## 5. The gate — not moved, and read as ONE condition

    profit factor >= 1.200  AND  expectancy >= +0.050R  AND  >= 40 trades
    on BOTH windows

Counted **by hand at 40**; the tool's verdict string says `need 30`.

Per addendum section A these are not three independent legs: at
`reward_risk = 1.8` with pure STOP/TARGET exits, PF 1.200 corresponds to a
40.0% win rate and +0.050R to 37.5%, so **PF is the binding leg and expectancy
is redundant** — 4 cells have passed expectancy while missing PF and 0 the
reverse. These mechanisms do not exit on STOP/TARGET at all (exits are the
strategy's, `reward_risk` never derives a target), so **which leg binds will be
reported from the measured rows, not assumed**.

**The gate is read on `expectancy_net`, not on `expectancy`** — brief
requirement 1, and the whole point of the job.

Percentiles are **not** the gate. `null p50` is written beside every one, a
`null p50` of `0.000` is read as "the control did not calibrate", and when
`count match` or `cost match` is outside the band the percentile is **not
published** (brief section 4). Per `agent/pure-drift` section 7 the `RandomHold`
control is handed **0.97x-1.39x** the method's time in the market on 14 of 15
rows; that defect is counted, not repaired, and no percentile is read as a
result in this plan either.

## 6. Rows — 8, frozen

`docs/research/designs/2026-10-07-rollover-flat.toml`. Four are **parity
anchors** that must reproduce `agent/n5` / `agent/pure-drift` to the printed
digit from a **different binary** (this one has to be built, because the shared
`fd-instr-repair` binary cannot know a strategy id that did not exist when it
was built):

| label | base | overrides | role |
|---|---|---|---|
| `px-1s` | `session-hold` | from 1800, to 1600, side 1, riskDailyRanges 1.5, rangeDays 20 | pure one-session long exposure: the floor any flat variant must beat to have a pattern in it at all; also the parity anchor for the window |
| `qs-h15` | `quiet-swing` | holdSessions 15 | the long-horizon baseline, 0.309R of carry |
| `ts-l20` | `tsmom` | lookbackDays 20 | the most-sampled long-horizon gate-passer |
| `ts-l60` | `tsmom` | lookbackDays 60 | the row this job exists for: 0.641R of carry |
| `qsf-h15` | `quiet-swing-flat` | holdSessions 15, from 1800, to 1600 | the flat variant of `qs-h15` |
| `qsf-h5` | `quiet-swing-flat` | holdSessions 5, from 1800, to 1600 | the flat variant at the hold `quiet-swing` ships |
| `tsf-l20` | `tsmom-flat` | lookbackDays 20, from 1800, to 1600 | the flat variant of `ts-l20` |
| `tsf-l60` | `tsmom-flat` | lookbackDays 60, from 1800, to 1600 | the flat variant of `ts-l60` |

`riskDailyRanges` is left at each parent's own default (**1.5** for the
`quiet-swing` family, **2.0** for the `tsmom` family) so that each flat variant
is measured in the **same R unit as the baseline it is the variant of**.
Cross-family R comparisons are therefore stated only with the declared
**x1.333** rescale, exactly as `agent/pure-drift` section 6 did.

Window **1800 -> 1600 New York** for every flat row: it is the window
`agent/pure-drift` already proved crosses **0 rollovers** (`px-1s`,
`swap$/spread$` 0.116x-0.188x, all of it holidays), so the one thing this plan
must not get wrong is inherited rather than re-derived.

## 7. Arms and windows — 3 x 2, 48 cells

| arm | guards | swap |
|---|---|---|
| **A** | off | 0.00 / 0.00 (`config/` — the desk's measured swap-free account) |
| **B** | off | -0.83 / -0.83 per lot-night (`config-swap/` — the generic symbol rate, an upper bound) |
| **C** | **on** | 0.00 / 0.00 |

Windows, split **by time**, the same two halves the record uses:

    IS   --from=2010-06-01 --to=2018-06-01
    OOS  --from=2018-06-01 --to=2026-06-01

Arm C is not a robustness check here, it is the **only tradeable arm**
(addendum section D: the owner has settled that nothing is held over a weekend,
so every long-horizon result in this record — including `agent/n5`'s four
gate-passers — lives in an arm the owner has forbidden). A flat variant should
be **compatible** with guards by construction, and `--exit-mix` is how that is
checked rather than asserted.

## 8. Falsifiers — specific, and firable

**F1 — the one this job exists to fire.** If **no** flat row (`qsf-*`, `tsf-*`)
clears the gate on **both** windows in **any** arm, then the residual edge of
the long-horizon mechanisms is **not carry**: it does not exist, and the desk
can close the last direction the brief owner still recommends.
*Fires on:* 0 of 8 flat-row cells clearing both legs, in all three arms.

**F2 — implementation, and it is a PRE-CHECK.** A flat variant must pay
financing like `px-1s` and not like its parent. If any flat row's
`swap$/spread$` in arm B exceeds **1.0x**, or its `expectancy_net` differs from
its `expectancy` by more than **0.010 R/trade**, the position is crossing
rollovers and the implementation is wrong. **This is checked on a `xagduka`
probe before a single `xauduka` cell is spent**; per brief section 5, a
falsifier that fires at pre-check costs **0 gate cells**.

**F3 — window artefact #10.** Per addendum section B, gold's one-session drift
is **-0.0040 R on 2010-2018 and +0.0205 R on 2018-2026**: the sign flips. If a
flat row clears the gate on the 2018-2026 leg and not on the 2010-2018 leg, it
is **written up as the tenth window artefact, not as a mechanism**. Nothing here
is built on "gold has positive drift".

**F4 — the governance claim.** The strong claim available to a flat variant is
that it is the **first long-horizon mechanism in this record that lives in the
tradeable arm**. If `WEEKEND_FLAT` fires on a flat row in arm C more than
**5 times per 1,000 trades** (`px-1s`'s measured rate is 1 in 2,014 = 0.5 per
1,000), that claim is **false** and is written as false.

**F1, F3 and F4 can all fire. Every reading is written whatever it says.**

## 9. What will be measured and reported, declared now

1. **`expectancy_net` beside `expectancy` on every row**, in both swap arms
   (brief requirements 1 and 2). If a flat row's `expectancy_net` is
   **unchanged between arm A and arm B**, that is reported as **the result**,
   with the `swap$/spread$` multiple beside it, the way `px-1s` reads 3-5%.
2. **Round trips per original-trade-equivalent, and the spread actually paid.**
   Measured two ways, both receipt-internal: (i) `spread$ / trades` over
   `risk_usd`, where `risk_usd` is recovered as
   `swap$ / (trades x (expectancy - expectancy_net))` from the arm-B pair; and
   (ii) the ratio of a flat row's `spread$` to its baseline's `spread$` over the
   same window, which is the extra round trips directly. Both are approximate
   because lots are re-sized every trade; both are printed numbers, and the
   difference between them is reported rather than averaged.
3. **`--exit-mix` on every row**, because an engine can print `SURVIVES` while
   the mechanism's own rule fires **0 times** (brief section 6a, `tsmom/120d`).
   A flat row whose exits are all `WEEKEND_FLAT` / `NEWS_FLAT` /
   `OPEN_LOSS_CAP` is **void**, as `buy-and-hold` in arm C is void.
4. **Which gate leg binds** at each mechanism's realised exit mix (addendum
   section A).
5. **Cost per R with the ruler named**, every time: "1.5 mean New York-day
   ranges over 20 days", never a bare "1.5 ATR" (addendum section F).

## 10. NOT measured, declared now so it cannot be claimed later

1. **Drawdown.** `max_drawdown_usd` / `max_drawdown_pct` exist in
   `engine.rs::Metrics` but `--mode=hypotheses` does not print them (addendum
   section G). **No drawdown criterion is offered in this registration**, and
   every profit number in the report will be stated as a return with no risk
   figure beside it.
2. **The 16:00-18:00 New York gap.** The flat variant forgoes it; this plan does
   not measure what is in it. A `session-hold` row on 1600 -> 1800 would measure
   it and is **not** declared, because it would cross the rollover and is
   therefore a different question.
3. **Venue spread around the daily break.** This feed's spread is a flat 0.28 at
   every hour; a real venue widens it exactly where this mechanism re-enters. So
   every spread figure here is a **lower bound on the real cost** of the flat
   expression, and that asymmetry is against the hypothesis.
4. **`xauusd`.** Four years cannot carry a sixteen-year question.
5. **Bit-identity with the parents.** `quiet-swing-flat`'s state rule is the
   declared approximation of section 4, not a port.
6. The two counted-not-repaired defects of brief section 7 (`wrong_side_stop`,
   `check_exit` pricing a gapped stop at `bar.open`). No row here uses a pending
   order and no row enforces a stop, so neither should be exercised; if
   `wrong_side_stop` is non-zero on any row, that row is reported as
   **unreadable**, per brief section 7.
7. **`data-sealed/`.** Not opened, read, pointed at or counted.

## 11. Multiplicity ledger — declared NOW

| | declared |
|---|---|
| gold cells (2 windows x 3 arms x 8 rows) | **48**, in **6 runs** |
| pre-check probe cells, `xagduka`, arm A + arm B, for F2 and "does the rule fire at all" | **16**, in 2 runs |
| grand total | **48 gold + 16 probe** |

No row will be added, no threshold moved, no window changed and no sample floor
lowered after a number is seen. Amendments are **dated notes appended to
section 13**; no line above is rewritten.

## 12. Settings, frozen

    --mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=200
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --batch-file=/e/rust/fd-rollover-flat/docs/research/designs/2026-10-07-rollover-flat.toml
    --config=/e/rust/fd-rollover-flat/config          (arms A and C)
    --config=/e/rust/fd-rollover-flat/config-swap     (arm B)
    --guards                                           (arm C only)

Binary: **built on this branch**, `--release`, into this worktree's own
`target/`, with `cargo +stable-x86_64-pc-windows-gnu` (the machine has no MSVC
linker). The shared `/e/rust/fd-instr-repair/target/release/search.exe` **cannot
be used**: a binary silently swallows a flag or an id it does not know, and both
new ids are new. The parity anchors of section 6 exist precisely so the new
binary is shown to reproduce the record before anything new is read from it.

Tests: `--release` and `-p fd-strategy` only. `cargo test --workspace` at debug
inflates `target/debug` to 11 GB (brief section 2) and is not run.

`--samples=`, `--direction-samples=`, `--rebate-share=`, `--trail=`,
`--spread=`, `--params=`, `--filters=`, `--strategy=`, `--batch=`,
`--companion=`, `--null-registered-stop` are **not passed**: none is read by
`--mode=hypotheses` for this plan. Every receipt must reproduce the
`flags: N passed, every one of them read by --mode=hypotheses` line, and the
`news:` source line is reproduced in the report because the engine reads
`data/news/events.parquet` from the default `data/` whatever `--data=` says.

Not touched: `config/accounts.toml` outside this worktree, `config/local.toml`,
the VPS 103.19.29.194, `main`, `data/gold/`, `data/btc/`,
`/e/rust/flowdesk/target/`, and the two `collect.exe` processes (pids 5044 and
38720), which are not signalled.

## 13. Amendments

### 2026-10-07, after the `xagduka` pre-check and before the first gold cell

**The probe ran, and half of F2 could not be fired on it.** `config-swap/`
sets a non-zero rate on **`xauduka` and `xauusd` only**; `[markets.xagduka]`
carries `swap_long_per_lot = 0.0` in **both** config trees (lines 465-466), and
the probe receipt confirms it in its own header: *"swap: long 0.00 / short 0.00
USD per lot per night"* with `--config=config-swap`. So a silver probe cannot
distinguish a flat variant from its parent on financing, and the **second
declared probe run (arm A) was not made at all**, because for `xagduka` the two
config trees are identical and the receipt would have been byte-identical.

**Probe cells: 16 declared, 8 viewed, in 1 run.** `receipts/rollover-flat/PROBE-xag-swapB.txt`.

**What the probe DID fire — the "does the rule fire at all" half, which it
answers yes to.** Both new ids are known to the new binary;
`quiet-swing-flat` books **2,423 trades** (`holdSessions` 5) and **2,763**
(15), `tsmom-flat` **3,883** (`lookbackDays` 20) and **3,764** (60) over
374,188 silver bars; the mechanism's **own rule** (`window closed`) is the exit
on every trade but one `END_OF_DATA` in all four rows, so brief section 6a's
trap is not sprung; mean hold is **1,408-1,417 min**, which is `px-1s`'s own
**1,406.5 min** on the same feed and the same window to within 0.8%.

**Where F2's financing half is checked instead, declared before the gold runs:**

1. **Against the charging function itself, not a restatement of it.**
   `crates/fd-strategy/tests/causality_rollover_flat.rs::no_position_either_strategy_opens_is_ever_charged_a_rollover`
   walks both strategies over an 80-session 15-minute fixture, tracks positions
   the way the engine tracks them, and asserts
   `fd_core::clock::swap_nights(entry, exit) == 0` on **every** closed
   position — the same function `engine.rs` multiplies by the rate. It also
   asserts the longest hold is ~22 h, so the zero is not the zero of a
   position that never opened.
2. **On the gold arm-B run**, which is inside the declared 48 cells and
   therefore costs nothing extra. The threshold stands as registered:
   `swap$/spread$ > 1.0x` or `|expectancy_net - expectancy| > 0.010 R/trade` on
   a flat row means the implementation is wrong and the result is void.

**Also measured on the probe and worth recording before the gold run, because
it bears on the trade-off arithmetic of section 3:** `tsf-l20` takes
**10.6x** the trades of `ts-l20` (3,883 vs 368) but pays only **7.1x** the
spread in dollars (7,202.28 vs 1,012.98 USD), because lots are re-sized every
session rather than once per multi-week hold. So "27 round trips instead of 1"
is an **upper bound on the dollar cost**, not the cost. The gold receipts carry
the same two printed figures and the report will use them.

**One reading limit found on the probe and carried forward:** the `RandomHold`
control's `cost match` is **0.65-0.75** on all four flat rows — outside the
band — so per brief section 4 **no percentile is published for them**, on silver
or on gold. This is the same instrument defect `agent/pure-drift` section 7
counted and did not repair.
