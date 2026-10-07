# 2026-10-07 — pure exposure vs the gate, vs the drift null, and vs the best pattern in the record

**Registered:** commit time is authoritative. Written and committed ALONE,
before any `xauduka` number on this branch exists.
**Branch:** `agent/pure-drift`, worktree `E:/rust/fd-pure-drift` (cut from
`agent/instr-repair`).
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` (round 4).
**Status:** registered.

## 1. The question, one sentence

`agent/n5` measured that at a horizon of >= 5 sessions the desk's
**cost-matched, count-matched, side-matched, exposure-matched RANDOM hold stops
losing money** — 16 of 23 arm-A rows with >= 40 trades print `null p50 > 1.000`
(1.091 - 2.137) where the desk's null median at <= 4 h is **0.867** — while
**no cell of 96 reached the 95th percentile of that same null** (highest 94%).
Read literally that says the drift is real and the added skill is not, so the
question seven programmes never asked is: **can this desk collect that drift
with no pattern at all — pure long exposure — and does the pattern buy anything
per unit of exposure that exposure does not already have?**

## 2. What is new, and what is not

No code is written and no strategy is added. Two registry entries express pure
exposure and **neither has ever been measured with real costs on a long window
anywhere in this record**:

- **`buy-and-hold`** (`crates/fd-strategy/src/builtin.rs`) — "enter long on the
  first tradable bar and never exit", `Exits::Strategy`, so the 4 h
  `max_hold_ms` never reaches it. It is named in the code as "the control every
  other result must beat" and the record contains **no receipt that ran it**.
- **`session-hold`** (`crates/fd-strategy/src/session_hold.rs`) with `side`
  pinned and a New York window wide enough to cover a whole session — pure long
  exposure **chopped into one-session trades**, which is the only form of pure
  exposure the 40-trade gate can express at all.

The pattern comparison is the record's own best rows at this horizon, re-read
with the **repaired** binary that prints `expectancy_net` / `total_r_net`:
`qs-h15`, `ts-l20`, `ts-l60` — the three `xauduka` cells that cleared the gate
on both legs in `agent/n5`, whose receipts were produced by a binary that could
not see financing in expectancy at all.

## 3. Pre-registration probe — ALREADY SPENT, declared here, numbers not read

Run on **`xagduka`** (silver, 2015-01-01 -> 2017-01-01, `--seeds=10`), which is
**not in this plan**, purely to learn the plumbing. 3 rows + one header-only
`xauduka` invocation (`--limit=0`, no row printed, used to read the bar range).
What the probe established, as mechanics and not as result:

1. `buy-and-hold` runs under `--mode=hypotheses`: 1 trade, `END_OF_DATA`, long
   share 1.000, exposure share of time +1.000.
2. **Its matched null returns `NaN`** — `matched null NaN trades median`,
   `null p50 NaN`, percentile printed as `null`. So **`buy-and-hold` has no
   readable percentile and no readable null**, and this registration does not
   ask for one. Declared now, before the gold run.
3. `session-hold` with `from = 1800, to = 1700` (a window wrapping across the
   feed's daily break) takes **0 trades** — entry needs a previous bar
   *outside* the window and no bar ever falls in 17:00-18:00 New York. That
   shape is **discarded here, before the plan**, and is the reason the plan uses
   `to = 1600`.
4. `session-hold` with `from = 1800, to = 1600, side = 1` takes **485 trades in
   2 years**, mean hold 1,425 min, exits 485/485 on its own rule
   (`window closed`), long share **1.000 method vs 1.000 null** — the side
   match that `agent/n5`'s direction-switching rows kept failing
   (`** the control's side ratio is not the method's **` on 5 of 8 rows) is
   **exact** for a one-sided row, by construction.
5. On that same row the exposure line read `time in market method 691,290 min
   vs the null's median 864,990 min — ratio 1.25  ** the control did not
   collect the drift the method did **`. **Declared as a read limit now:** the
   `RandomHold` control is given ~25% MORE time in the market than a
   session-hold row, which in a positive-drift band flatters the control. Any
   percentile on these rows is read with that in it, and the percentile is not
   a gate leg either way.

## 4. Windows — by time, no overlap, same split as `agent/n5` so the numbers compare

`xauduka` 15m, 378,749 bars, 2010-06-01 00:00 -> 2026-05-31 23:45 (read off the
binary's own `bars:` line, this worktree, 2026-10-07).

| window | from | to |
|---|---|---|
| DUKA-IS | 2010-06-01 | 2018-06-01 |
| DUKA-OOS | 2018-06-01 | 2026-06-01 |

`--to` exclusive. `xauusd` is **not** run: 4 years cannot carry a 16-year
exposure question and the trade floor already bit there in `agent/n5`.

**Declared as a prediction, to be checked after:** the IS leg contains gold's
2011-2015 decline and the OOS leg contains the 2019-2026 rise, so pure long
exposure is expected to be much weaker on IS than on OOS **even at swap = 0**,
and an IS/OOS split is therefore the harshest honest test of it. If pure
exposure passes only OOS, it passes nothing.

## 5. Rows — 8, frozen

| label | base | pinned | role |
|---|---|---|---|
| `bh` | `buy-and-hold` | — | pure exposure, undivided. 1 trade a leg: **exempt from the gate by construction**, carries M1 only (section 7) |
| `px-1s` | `session-hold` | `from=1800, to=1600, side=+1, riskDailyRanges=1.5, rangeDays=20` | **the row.** Pure long exposure chopped into one-session trades, thousands of them, gate-expressible |
| `px-ny` | `session-hold` | `from=0930, to=1600, side=+1, riskDailyRanges=1.5, rangeDays=20` | the same thing at ~390 min of exposure a day — a smaller DOSE, same rule |
| `px-on` | `session-hold` | `from=1800, to=0930, side=+1, riskDailyRanges=1.5, rangeDays=20` | the complement dose, ~930 min a day |
| `px-short` | `session-hold` | `from=1800, to=1600, side=-1, riskDailyRanges=1.5, rangeDays=20` | **the sign test.** Identical exposure, opposite side, identical spread. See F3 |
| `qs-h15` | `quiet-swing` | `holdSessions = 15` | best pattern in the record: gate pass on both `xauduka` legs, arm A |
| `ts-l20` | `tsmom` | `lookbackDays = 20` | the most sampled gate-passer at this horizon (183 / 185 trades) |
| `ts-l60` | `tsmom` | `lookbackDays = 60` | the highest PF in the record at this horizon (2.183 OOS) |

`riskDailyRanges = 1.5` / `rangeDays = 20` is **copied from `quiet-swing`'s
defaults deliberately**, so the pure-exposure rows and `qs-h15` carry the same R
unit and their R figures are comparable. `tsmom` sizes at 2.0 daily ranges, so
its R unit is 1.33x larger and this is stated wherever its R is put beside
theirs. **`buy-and-hold` has no sizing parameter at all** — the engine's 15m-ATR
fallback sets its R — so **`bh`'s R is NOT comparable to any other row here**
and its result is read in USD and in M1 only. Declared now.

Warmup, per brief section 8: `session-hold` warmup is `atrPeriod + 5` = 19 bars
and `rangeDays = 20` sessions; `buy-and-hold` warmup is 1 bar. Neither can be
warmup-starved on a 187,000-bar leg. `tsmom/60d` needs (60+5)x288 = 18,720 bars
and has them. **No row here is warmup-limited, and the windows are 8 years
precisely so that the `tsmom` trap of a 3-month window cannot recur.**

## 6. Arms — 3, declared, and which one is real

| arm | guards | swap | config |
|---|---|---|---|
| **A** | **off** | 0.00 / 0.00 | `config/` — the desk's measured account (340 closed positions, 87 overnight, swap 0.00) |
| **B** | off | -0.83 / -0.83 per lot-night | `config-swap/` — the generic symbol rate (-82.76 points), charged both ways, so an **upper bound** on financing |
| **C** | **on** | 0.00 / 0.00 | `config/` `--guards` |

**Arm A is the real arm for every row in section 5, and arm C cannot express
any of them.** `[guards] flat_before_weekend_hhmm = 1640` flattens before every
weekend and `max_open_loss_r = 2.0` closes at 2 R of the sizing unit, so in arm
C `buy-and-hold` cannot hold, and `agent/n5` already measured the horizon
parameter **ceasing to exist** in that arm (`qs-h5/h10/h15/h20` printing
identical numbers to the digit). Arm C is run and reported because the brief
asks for both, **not** as a stricter reading of arm A: it is a different
mechanism and is labelled as one.

Arm B x arm C is deliberately not run: a book flattened before every weekend has
few nights to charge, and the question is not guards x swap.

## 7. The measure — declared BEFORE the run, because the gate cannot express this question

The gate is **not moved**: `PF >= 1.200 AND expectancy >= +0.050R AND >= 40
trades, on BOTH legs`. Counted by hand at **40**, because the tool's own verdict
string says `need 30` and a 30-39 trade row prints as passing a floor it has not
passed (brief section 4).

**The gate is the wrong instrument for an undivided hold, and that is said here
before any number is seen.** A 16-year hold is 1 trade, so it fails the count
leg whatever it earns, and `expectancy >= +0.050R` *per trade* is a far harder
bar for a row that takes 2,000 one-session trades than for one that takes 90
multi-month trades, for no reason connected to the merit of either. The missing
quantity is **return per unit of exposure**. So, declared now:

- **M1 = net R per 1,000 hours of position time**
  = `1000 x expectancy_net / (mean_hold_min / 60)`.
  Both inputs are printed per row by this binary (`mean hold ... min` on the
  `--exit-mix` line; `expectancy_net` on the net line, which the binary prints
  only where swap or commission is non-zero — in arm A `r_net == r` by
  construction and `expectancy` IS `expectancy_net`, asserted by
  `engine.rs::r_net_is_r_when_nothing_but_the_spread_was_paid`). Unit: R per
  1,000 h.
- **M2 = net R given up to financing per trade** = `expectancy_net -
  expectancy`, printed directly by the repaired binary in arm B. Unit: R/trade.

**M1 and M2 are NOT gates and no threshold is set for them.** They are used for
exactly two comparisons, declared now:
1. `bh` and the `px-*` rows against each other, to test the chop-invariance
   claim: pure long exposure holds the same position over the same nights
   whether it is booked as 1 trade or 2,000, so its net result should differ
   only by `(N - 1)` round-trip spreads. If M1 moves far more than that, the
   difference is not spread and the receipts say what it is.
2. the pure-exposure rows against `qs-h15` / `ts-l20` / `ts-l60` **on the same
   window and the same arm**, to answer "does the pattern add anything per unit
   of exposure". If a pure-exposure row's M1 is >= the best pattern row's M1,
   the pattern added nothing per unit of exposure **on this window**, and that
   is the whole claim — it is not a gate pass for either.

Percentiles are reported with `null p50` beside them and are **not** a gate leg,
per brief section 4, and on these rows they are further discounted by the
exposure ratio defect of section 3.5. `bh` has no percentile at all
(section 3.2).

## 8. Falsifiers — specific and firable

**F1 (the main one).** FIRES if **no** pure-exposure row with >= 40 trades
(`px-1s`, `px-ny`, `px-on`) clears the gate on **both** DUKA-IS and DUKA-OOS in
**arm A**.
*Reading when it fires:* the drift that the control measures is **not
collectable by a long position on this book**, and "the drift is real" is a fact
about the null, not an opportunity. The desk should not move to exposure sizing.

**F2 (the honest-naming one).** FIRES if a pure-exposure row clears the gate in
arm A but fails it in arm B. *Reading when it fires:* **"an account perk, not a
strategy"**, in those words, exactly as `agent/n5` had to write them.

**F3 (the sign test — new, and the sharpest thing here).** `px-1s` and
`px-short` are the SAME exposure, the SAME window, the SAME spread, opposite
sides. So their expectancies must satisfy
`expectancy(long) + expectancy(short) ~= -2 x spread/R`, and the drift per
session in R is **`(expectancy(long) - expectancy(short)) / 2`** — a direct
measurement, from the method side rather than from the null.
F3 FIRES if that half-difference is **not positive** on both legs of arm A.
*Reading when it fires:* at the one-session horizon there is no harvestable
long drift at all on this feed, and the `null p50 > 1.000` result that prompted
this whole registration is a property of the control's construction (its 1.25x
exposure over-allocation, section 3.5) rather than of gold.

**F1 and F3 can both fire. Both readings are worth having and both are written
whatever they say.**

## 9. Multiplicity ledger — declared NOW

| | declared |
|---|---|
| gold cells (window x arm x row) | **48** = 2 windows x 3 arms x 8 rows, in **6 runs** |
| pre-registration probe cells | **3** (`xagduka`, section 3) + 1 header-only `xauduka` invocation with `--limit=0` that printed no row |
| grand total | **48 gold + 4 probe** |

No row will be added, no threshold moved, no window changed after a number is
seen. Amendments are **dated notes appended to section 11**; no line above is
rewritten.

## 10. Settings, frozen

    --mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=200
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --config=/e/rust/fd-pure-drift/config          (arms A and C)
    --config=/e/rust/fd-pure-drift/config-swap     (arm B)
    --guards                                        (arm C only)

Binary: `/e/rust/fd-instr-repair/target/release/search.exe`, built 2026-10-07
01:35 from `agent/instr-repair`, the one with `expectancy_net` / `total_r_net`
and the `ALWAYS` / `BY_MODE` flag audit. **No build. `cargo` is not invoked on
this branch.**

`--samples=`, `--direction-samples=`, `--rebate-share=`, `--trail=`,
`--spread=`, `--params=`, `--filters=`, `--strategy=`, `--batch=`,
`--companion=`, `--null-registered-stop` are **not passed**: none of them is
read by `--mode=hypotheses` for this plan, and the probe confirmed the binary
prints `flags: 12 passed, every one of them read by --mode=hypotheses` for
exactly the set above. Every receipt reproduces that line.

`data-sealed/` is not opened, read, pointed at or counted. `config/accounts.toml`
outside this worktree, `config/local.toml`, the VPS and `main` are not touched.
`collect.exe` pids 5044 / 38720 are not signalled and `data/gold/`, `data/btc/`,
`/e/rust/flowdesk/target/` are not written. The `news:` source line of every
receipt is reproduced in the report, because the engine reads
`data/news/events.parquet` from the default `data/` whatever `--data=` says.

## 11. Amendments
