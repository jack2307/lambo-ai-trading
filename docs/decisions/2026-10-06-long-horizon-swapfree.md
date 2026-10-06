# 2026-10-06 — long-horizon gold holds on a swap-free account (>= 5 sessions)

**Registered:** commit time is authoritative. Written and committed ALONE,
before any gold number on this branch exists.
**Branch:** `agent/n5`, worktree `E:/rust/fd-b5`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-06-B.md` (programme 6, round 2).
**Status:** registered.

## 1. Hypothesis — one sentence

At a horizon of **five sessions or more** — the only horizon band where gold's
net drift has been measured POSITIVE (+0.3946 ATR20 per 5 sessions, t = +6.74,
`docs/hypotheses/2026-10-02-drift-null.md`) — a directional gold hold on this
desk's **measured swap-free account** (340 closed positions 2026-04-21 to
2026-09-08, 87 held overnight, swap **0.00**; generic symbol rate
**-82.76 points = -$0.83/oz/night**, `config/default.toml`
`[markets.xauusd.trading]`) clears the gate on both an in-sample and an
out-of-sample window, and the gain is NOT merely the financing it does not pay.

Every desk mechanism measured to date lives at horizons <= 4 h, i.e. inside the
band where net drift is negative. The >= 5-session band has never been scanned.

## 2. What is new, and what is NOT

The family is **expressible with the existing binary** — no new strategy is
written, and none of the 19 already-measured mechanisms is rewritten. Two
registered bases already declare `Exits::Strategy` (so `max_hold_ms` never
reaches them, `engine.rs` `check_exit`) and can be PINNED to a long horizon:

- `quiet-swing` (`crates/fd-strategy/src/quiet_swing.rs`) — `holdSessions` is
  an explicit session-count horizon. It has only ever been read at
  `holdSessions = 5`, guards ON, and the sweep never touched it because the
  strategy declares an EMPTY grid
  (`docs/research/designs/2026-09-23-designed-3-frozen.toml`).
- `tsmom` (`crates/fd-strategy/src/tsmom.rs`) — holds the side of the trailing
  `lookbackDays` return until the sign flips.

**What has never been measured** is therefore the HORIZON AXIS itself: the
hold length swept upward on a window long enough to carry the warmup, in the
**no-guards** arm (the only arm where a 5+ session hold is allowed to exist at
all), with **overnight financing as an explicit scenario**.

Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (2026-10-04 15:13, carries
the drift-control null). No build; `cargo` is not invoked by this registration.

## 3. Family membership rule — declared BEFORE the numbers

A row belongs to this family only if its **measured mean hold >= 5 sessions
(>= 120 h)**, read off the `--exit-mix` line of its own receipt. Rows under
that are reported but marked **out-of-family** and cannot carry the result.
This rule exists because `tsmom` was measured holding 66 h (2.75 sessions) and
66 h is NOT this family.

A row is also void if `--exit-mix` shows its own exit rule fired **0 times**
(the `tsmom/120d` trap: PF 2.236, `SURVIVES`, exits all `NEWS_FLAT` /
`WEEKEND_FLAT`).

## 4. Falsifier — specific and firable

**FIRES** if, across all 96 declared cells, **no cell** clears the gate
(PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades) on **both** the
in-sample and the out-of-sample window of the same market, in the **no-guards,
swap = 0** arm.

Conclusion when it fires: the positive-drift band at >= 5 sessions contains no
exploitable edge either, and the last unscanned horizon band is closed.

**SECOND, SEPARATE FALSIFIER (the honest-naming one).** If a cell clears the
gate at `swap = 0` but FAILS at the generic rate, it is reported as **"an
account perk, not a strategy"** — in those words — and not as a surviving
mechanism.

## 5. Windows — declared with the reason, before running

`xauduka` is the PRIMARY window, not `xauusd`, and the reason is sample size,
declared now: `quiet-swing` took 136 trades in 3.27 years of `xauusd` at
`holdSessions = 5`; a hold cannot re-enter while open, so 20 sessions is
roughly a quarter of that, and `xauusd` split in two gives ~20 trades a leg —
**under the 40-trade floor**. 16 years of `xauduka` at 20 sessions gives ~90 a
leg.

| window | market | from | to | bars on disk |
|---|---|---|---|---|
| DUKA-IS | `xauduka` 15m | 2010-06-01 | 2018-06-01 | 378,749 total, 2010-06-01 to 2026-05-31 |
| DUKA-OOS | `xauduka` 15m | 2018-06-01 | 2026-06-01 | same file |
| VAN-IS | `xauusd` 15m | 2022-06-16 | 2024-09-01 | 100,586 total, 2022-06-16 to 2026-09-17 |
| VAN-OOS | `xauusd` 15m | 2024-09-01 | 2026-09-18 | same file |

Split is strictly by time, no overlap, `--to` exclusive. The `xauduka` IS leg
deliberately carries the known 2010-2012 P&L concentration (14% of trades, 48%
of R) so that the OOS leg is free of it; the OOS leg is the one that counts.

**Declared in advance as a measurement limit, not a result:** `tsmom` warmup is
`(lookbackDays + 5) * 288` bars of 15m, so `lookbackDays = 120` needs 36,000
and `250` needs 73,440. The `xauusd` legs hold ~47,000 and ~49,000 bars, so
`tsmom/120d` will be warmup-starved there and `tsmom/250d` is expected to
return **0 trades on both `xauusd` legs**. That is the warmup-vs-window
constraint the round-1 agent identified, stated here before it is seen.

## 6. Rows — 8, frozen

| label | base | pinned | why |
|---|---|---|---|
| `qs-h5` | `quiet-swing` | `holdSessions = 5` | the registered horizon, re-read with no guards |
| `qs-h10` | `quiet-swing` | `holdSessions = 10` | two weeks of drift exposure |
| `qs-h15` | `quiet-swing` | `holdSessions = 15` | three weeks |
| `qs-h20` | `quiet-swing` | `holdSessions = 20` | a month; the longest that keeps >= 40 trades on 8 years |
| `ts-l20` | `tsmom` | `lookbackDays = 20` | the shortest lookback the method ships |
| `ts-l60` | `tsmom` | `lookbackDays = 60` | its default |
| `ts-l120` | `tsmom` | `lookbackDays = 120` | the cell that produced the guards trap |
| `ts-l250` | `tsmom` | `lookbackDays = 250` | a year of lookback; only `xauduka` can carry it |

All other parameters stay at the strategy defaults. No filters. Pinning
`lookbackDays` empties `tsmom`'s grid, which is what makes `control_for` build
the `RandomHold` **drift** null rather than the stop-and-target one — the only
control a self-managed multi-day hold can be read against.

## 7. Arms — 3, declared

| arm | guards | swap | what it is for |
|---|---|---|---|
| A | OFF | 0.00 / 0.00 | **the real arm.** `flat_before_weekend_hhmm = 1640` would close every hold before every weekend and so would delete the entire family |
| B | OFF | -0.83 / -0.83 | the generic symbol rate, charged. What the perk is WORTH, and what happens if the broker withdraws it |
| C | ON | 0.00 / 0.00 | the comparison the record has always printed; expected to inflate the sample (3.4x-106x measured on `tsmom`) and to kill the horizon |

Arm B's config lives in `config-swap/` inside this worktree only;
`config/accounts.toml` outside this worktree is not touched. The short-side
rate is **not in the record** — only the -82.76-point long rate is — so arm B
charges the same magnitude both ways and is therefore an **upper bound** on the
financing cost, stated as such.

The fourth combination (guards ON x swap charged) is **deliberately not run**:
the guards arm flattens before the weekend, so it has almost no nights to
charge, and the question is not guards x swap.

## 8. Settings, frozen

`--mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=200`,
`--interval=15m`, `--data=/e/rust/flowdesk/data`, spread as configured (0.28),
trail off, `--config=` pointing at this worktree. `--fixed` means the pinned
parameters are read over the whole window with no selection, so the window
named is the whole measurement.

`--samples=` and `--direction-samples=` are NOT read in this mode and are not
passed. `--rebate-share=` is NOT read in this mode and is not passed: no
number here is rebate-credited.

`data-sealed/` is not opened, not read, not pointed at and not counted. The
`news:` source line of every receipt is reproduced in the report, because the
engine reads `data/news/events.parquet` from the default `data/` whatever
`--data=` says.

## 9. Multiplicity ledger — declared NOW

**96 (run, row) cells** = 8 rows x 4 windows x 3 arms, in 12 runs.

Already spent before this registration and declared here: **1 cell** of
runtime probing on `xagduka` (silver, `holdSessions = 5`, `--seeds=10`), run
only to time the binary. Silver is not in this plan and its number is not read.

Grand total declared: **96 gold cells + 1 silver timing cell**.

## 10. How it will be read

- Gate: PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades. Not moved.
- Under 40 trades: no conclusion, in either direction.
- Percentiles are reported with `null p50` beside them always, and are NOT a
  gate leg. The desk's null median has PF < 1, so a high percentile means
  "lost less than a cost-matched, count-matched, side-matched random hold",
  never "made money".
- One window is nothing. A cell counts only if it clears the gate on the IS
  **and** the OOS leg of the same market.
- Both guards arms are reported for every cell that clears anything, per brief
  section 8.
- Amendments to this file are DATED NOTES APPENDED BELOW. No line above is
  rewritten.

## 11. Amendments

### 2026-10-06, after the twelve runs — an imprecision in section 3, recorded not repaired

Section 3 defined family membership as "mean hold >= 5 sessions (>= 120 h)" and
treated the two as the same thing. They are not. `quiet-swing` at
`holdSessions = 5` holds **5, 6 or 7 sessions by its own count** (printed in
the exit mix: 112 / 99 / 17 on DUKA-IS) but only **103-112 h of wall clock**,
because a session count skips the weekend and a clock does not. By the session
clause `qs-h5` is in the family; by the 120-h clause it is out.

**Nothing is rewritten and nothing turns on it:** `qs-h5` fails the gate in
every one of its four arm-A cells (PF 1.081-1.181), so it carries no result
under either reading. Every cell that does clear the gate holds 158 h or more
and is in-family under both clauses. The two clauses are recorded as the
separate tests they are, for the next registration to state properly.

### 2026-10-06 — section 5's declared prediction, checked

Section 5 declared before running that `tsmom/250d` would return **0 trades on
both `xauusd` legs** and that `tsmom/120d` would be warmup-starved there. It
returned exactly 0 trades on both `xauusd` legs, and `tsmom/120d` returned **1
trade** on VAN-IS and **4** on VAN-OOS. The prediction stands as made.
