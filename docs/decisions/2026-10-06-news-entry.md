# Entering ON the economic calendar: the family the engine could not say

**Date:** 2026-10-06 (pre-registration; written and committed before the first
line of code)
**Question:** a scheduled release produces a volatility expansion whose *timing
is known in advance*. Does the first move after the release carry a directional
edge that 15m bars can take — either **with** it (breakout) or **against** it
once it is spent (reversion)?
**Outcome:** open. This file is the registration, not the result.

## 0. Why this family has never been rejected

`crates/fd-strategy/src/filter.rs` has exactly one calendar-aware gate,
`Filter::News { before_min, after_min, min_impact, currencies }`, and it only
knows how to **block**: "No entries from `before_min` minutes before to
`after_min` minutes after any scheduled event." There is no way to *require*
that an entry sit near an event. `data/news/events.parquet` carries 747 events
(2010-01-08 → 2027-12-08) and across five desk programmes they have been used
only to stay out of the way. Entry-on-the-calendar has never been expressible,
so it has never been measured and never been falsified.

## 1. Hypothesis

A high-impact US release expands gold's 15m range by a known amount at a known
minute. **Breakout branch:** the direction of the first move after the release
persists long enough to pay 1.8R against a stop placed one impulse-range away.
**Reversion branch:** the first move overshoots, and entering against it once
it has run for one to four bars pays the same geometry.

Prediction, stated so it can come back no: at the desk's own gate, on two
non-overlapping multi-year windows, **both branches produce zero surviving
cells**, because the expansion is real but symmetric — it is a variance event,
not a drift event — and the spread is widest in exactly the minutes the rule
trades.

## 2. Premise and clock, measured BEFORE declaring the rule

Disclosed because it was looked at first: the two checks below are instrument
checks (does the calendar line up with the bars; is there an expansion at all),
not outcome checks. No profit factor, expectancy or trade count was looked at
before this file was committed.

Median 15m range at offset *k* from each USD impact-3 release, divided by that
day's median 15m range:

| offset | −60 | −30 | −15 | **0** | +15 | +30 | +45 | +60 | +120 |
|---|---|---|---|---|---|---|---|---|---|
| XAUDUKA 15m (504 events) | 1.06 | 1.33 | 1.78 | **5.96** | 2.92 | 2.88 | 2.40 | 2.58 | 1.78 |
| XAUUSD 15m (137 events) | 0.91 | 1.21 | 1.50 | **7.02** | 2.95 | 3.29 | 2.67 | 3.04 | 2.04 |

- **Clock: both stores are epoch-ms UTC and so is the calendar
  (`time_utc` with `Z`).** The peak sits at offset **0** on both, sharply. A
  one-hour (DST) error would have smeared the peak four bars wide across half
  the sample, and does not. 504 of 517 XAUDUKA events and 137 of 139 XAUUSD
  events land exactly on a 15m bar open.
- **The premise is confirmed and large**: ~6–7× the day's median range on the
  release bar, still 2–3× an hour later, and ~1.8× in the 15 minutes *before*.
  So this registration is not testing whether the expansion exists. It is
  testing whether it is *directional*.

## 3. What is being built

1. `news::window_in` / `news::in_news_window` — the arithmetic of "is `t`
   inside `[event + from, event + to)` of some event of impact ≥ I of these
   currencies", with **signed** offsets. `blackout_in` becomes this with
   `(−before, +after)`, so the existing gate keeps its exact semantics.
2. `news::last_event_at_or_before` — the newest qualifying event at or before
   `t`, so a strategy can anchor on a release.
3. **`Filter::NewsOnly { from_min, to_min, min_impact, currencies }`** — the
   inverse of `News`: entries **only** while the signal bar's open time is
   inside `[event + from_min, event + to_min)`. Spelling
   `newsonly:<from>/<to>[:impact[:CCY|CCY]]`, minutes, signed, `/` as the range
   separator so `newsonly:-30/0` is unambiguous. `for_market` scopes it exactly
   as `News` is scoped.
4. **Strategy `news-pulse`** (a new id; none of the 19 already-measured
   mechanisms is touched or rewritten).

## 4. The rule, frozen

On bar `i`, let `j = i − (probeBars − 1)` and let `e` be the newest qualifying
event at or before `bars[j].time`. Bar `j` is the **release bar** when
`bars[j]` is the first bar at or after `e` (`j == 0` or `bars[j−1].time < e`)
and `bars[j].time − e ≤ maxLagMin`. Then:

- **impulse** = `bars[i].close − bars[j].open`, over the release bar and the
  `probeBars − 1` bars after it.
- **gate**: `|impulse| ≥ minMoveAtr × ATR(14)` read at `j − 1`, i.e. the
  **pre-release** ATR, so the gate is not contaminated by the expansion it is
  trying to detect.
- **side**: `mode = 0` → with the impulse (breakout); `mode = 1` → against it
  (reversion).
- **stop**: one impulse *range* (`max(high) − min(low)` over
  `bars[j..=i]`) beyond the entry on the losing side, × `stopImpulse = 1.0`.
  A failed break is a return through the impulse; that is the invalidation, and
  an ATR(14) stop would be ~1/6 of the release bar alone.
- **target**: none — the engine's `reward_risk = 1.8`.
- the fill is the next bar's open (the engine's rule) and
  `max_hold_ms = 4 h` applies (`Exits::Engine`).
- one entry per release: the trigger bar is a single bar per event.

Frozen values: `minImpact = 3`, `atrPeriod = 14`, `maxLagMin = 15`,
`stopImpulse = 1.0`, `reward_risk = 1.8` (config), `stop` structural.
`grid()` is **empty** — nothing is chosen from the data, every row is a
separately declared rule, and runs use `--fixed` (fixed parameters over the
whole window, no selection).

Swept, as declared rows and not as a search:

| axis | values |
|---|---|
| `probeBars` | 1, 2, 4 |
| `mode` | 0 breakout, 1 reversion |
| `minMoveAtr` | 0.5, 1.5 |

= **12 rows.**

## 5. The null ALSO enters on the calendar — and at the same minute

This is the axis's life-or-death point (brief §9). A coin-flip null that may
enter on any bar would spend ~99.8% of its entries in ordinary hours, while
`news-pulse` only ever enters in the single most volatile 15 minutes of the
month. A percentile read against that would measure "higher volatility", not
"mechanism".

The fix is structural rather than argued: the matched null in
`hypotheses.rs` is `RandomEntry` wrapped in **the hypothesis's own filters**
(`Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) }`),
so a `newsonly:` filter on the row puts the control inside the same window.
Each row carries the filter that admits **exactly the bar its own `probeBars`
fires on**:

| `probeBars` | signal-bar offset | filter on the row |
|---|---|---|
| 1 | +0 min | `newsonly:0/15:3` |
| 2 | +15 min | `newsonly:15/30:3` |
| 4 | +45 min | `newsonly:45/60:3` |

So the control draws from the **same single bar per event** as the method. What
remains different is (a) the `minMoveAtr` gate, which the engine's entry-rate
calibration absorbs by scaling the control's rate to the method's trade count,
and (b) the direction, which is the claim. The null is therefore a direction
null at matched timing, matched count and matched stop (`CostMatch::Method`).

**Declared instrument risk, before running.** `matched_rate` probes at
`entryRate = 0.02` on the gated bars and scales linearly. With ~130 admissible
bars in a window the probe draws ~2–3 trades, and if it draws 0 the rate stays
at 0.02 and the control is badly under-matched. `matched_rate` is frozen by
`docs/hypotheses/2026-09-23-matched-null-repair.md` and will **not** be
touched. Instead: every row's achieved `null trades` median is read against
`COUNT_MATCH_BAND = 0.25`, and any row outside it is reported **unmatched and
its percentile withheld**, with the gate alone doing the work.

`--null-sides=coin` (the default, and the null every published percentile was
read against). The method is two-sided by construction — the side is the sign
of the impulse — so its long share is expected inside the 0.40–0.60 band where
`docs/hypotheses/2026-10-02-drift-null.md` says the drift control should not
move a row. **Contingency, declared:** any row whose printed long share falls
outside 0.40–0.60 is re-run with `--null-sides=exposure`, and those cells are
added to the ledger.

## 6. Windows — and why the brief's two cannot be used at the gate

The brief's windows are A `2025-07-01 → 2025-10-01` and B
`2025-04-01 → 2025-07-01`. The calendar carries **9** USD impact-3 events in A
and **8** in B. One entry per release caps the book at 9 and 8 trades. The
gate asks for **≥ 40**. Those windows are *structurally* unable to answer this
question, and this is a property of the event arrival rate (~32 USD impact-3
releases a year), not of the rule.

Declared in advance, under brief §4: the gate is read on `xauduka` 15m
(2010-06-01 → 2026-05-31 on disk, Vantage's costs on Dukascopy's bars) over two
**non-overlapping** windows:

- **A′ `2022-01-01 → 2026-01-01`** — 127 USD impact-3 events
- **B′ `2018-01-01 → 2022-01-01`** — 134 USD impact-3 events

Both clear the 40-trade floor even if `minMoveAtr` rejects half the releases.
The brief's window A is still run once on `xauusd`, as a plumbing and
sample-size demonstration, and is **explicitly not read at the gate**.

## 7. Cost: every number here is an optimistic upper bound, and the fan says by how much

The engine charges one constant spread for a whole run. The real spread is at
its widest in exactly the minutes this rule trades — §2 measures the release
bar at 6–7× the day's median range, and a dealer widens with it. The configured
`xauduka` spread is **0.28** per round trip, which is the desk's *average*
read. So **every figure produced under this registration is a ceiling, not an
estimate**, and the honest output is the spread at which the conclusion breaks.

Declared fan on window A′, no-guards arm, all 12 rows:
`--spread=` **0.00, 0.14, 0.28 (config), 0.56, 1.12, 2.24**.

0.00 is in the fan on purpose: if a branch cannot clear the gate at **zero
cost**, the mechanism is dead for reasons that have nothing to do with
execution, and no spread argument can save it. Cost as a fraction of R is
`spread / stop` and therefore follows the **horizon, not the instrument**
(brief §7.1), so the stop size and the `cost-matched null: … cost X% of R`
line the tool prints are quoted beside every number, never a remembered
percentage.

## 8. Guards

Both arms, every row: `--guards` and no `--guards`, reported side by side
(brief §8). `--exit-mix` on every run, and a row whose own rule never fires an
exit — all `NEWS_FLAT` / `WEEKEND_FLAT` / `END_OF_DATA` — is reported as not
having been measured at all, whatever its profit factor says.

## 9. Gate — not adjustable

    profit factor ≥ 1.200  AND  expectancy ≥ +0.050R  AND  ≥ 40 trades

On **both** windows A′ and B′, in the **same** arm. One window is nothing.
A percentile is not a gate; the desk's median null has PF 0.867, so `null p50`
is printed next to every percentile quoted.

## 10. Falsifier — specific, and it can fire

1. **Primary.** If both the breakout branch (`mode = 0`, 6 rows) and the
   reversion branch (`mode = 1`, 6 rows) pass **0 cells** on **both** A′ and
   B′ in **both** guard arms, then the scheduled volatility expansion — a 6–7×
   range event whose minute is known years ahead — **contains no directional
   edge reachable by 15m bars**, and this family is closed. A whole family
   everyone's intuition says must be there, shut with a number.
2. **Cost falsifier.** If a cell passes at 0.28 but the conclusion breaks at
   any spread **below** the release-minute spread, the pass is an artefact of
   charging average cost for the most expensive minute of the month, and the
   falsifier has fired on the measurement rather than on the rule.
3. **Zero-cost falsifier.** If nothing passes at `--spread=0.00`, no execution
   improvement can ever rescue the family.
4. **Instrument falsifier.** If the method's trade count and the control's
   diverge by more than `COUNT_MATCH_BAND` on most rows, or a row's own exit
   rule never fires, the measurement is void and is reported as void — not as
   a result.

It can fire: 1 fires on the expected outcome, and 2–4 fire on specific printed
lines (`cost-matched null`, `null trades`, the `--exit-mix` line).

## 11. Multiple testing — declared BEFORE the first run

| block | cells (run × row) |
|---|---|
| A′ `2022-01-01→2026-01-01`, 12 rows × 2 guard arms | 24 |
| B′ `2018-01-01→2022-01-01`, 12 rows × 2 guard arms | 24 |
| spread fan, A′ no-guards, 12 rows × 5 further spreads | 60 |
| brief window A on `xauusd`, 12 rows, no guards (NOT read at the gate) | 12 |
| **declared total** | **120** |

Contingent, counted if used: 12 rows × 1 window × `--null-sides=exposure` = 12,
only for rows printing a long share outside 0.40–0.60.

The real `(run, row)` count is published at the end against this number. Any
cell looked at beyond it is listed in a dated note appended to this file —
never by editing a line above.

## 12. What this will not say

- Nothing here measures entering **before** a release. §2 shows the 15 minutes
  before a release already run ~1.8× the day's median range (thinning
  liquidity), and that is a separate, unmeasured branch.
- Nothing here measures 1m or 5m bars. A 6× range event inside one 15m bar may
  well be a tradable sequence of 1m bars; this registration cannot say, and a
  null result here must not be quoted as if it could.
- Nothing here measures limit entries, which is how a desk would really take a
  release, nor the slippage a market order eats in those seconds.
- `events.parquet` is 747 events dominated by **NFP (203), CPI (203) and FOMC
  (143+)**. "High-impact USD news" here means those three, not a broad
  calendar. `events-extended.parquet` (1,147 rows) exists on disk and is **not**
  used; the engine installs `events.parquet`, and the `news:` line of every
  receipt is quoted to prove which.

## What each role said

- **data-integrity:** NO OBJECTION — clock verified against the bars
  themselves (§2), store and calendar source quoted from the receipt header.
- **adversary:** the window substitution in §6 is the move to watch. It is
  declared before any result, its reason is an event arrival rate that can be
  counted independently, and the brief's own windows are run and published
  anyway.
- **risk:** both guard arms, every row.
- **execution-realist:** BLOCK on any number quoted without §7's fan. The
  spread in these minutes is not 0.28.
- **researcher:** the expansion is confirmed; only its direction is in doubt.
- **historian:** first time the desk can even spell this hypothesis.

## What would reopen this

A 1m or 5m measurement of the same releases; a limit-entry fill model; a
per-minute spread series around releases so cost stops being a constant; or
the pre-release branch of §12.

---

## Note added 2026-10-06, after the four main runs and before the spread fan

Nothing above is rewritten; this is an append, as §6 requires.

**Why the fan has to run on B′ too.** §7 declared the spread fan on window A′
only, on the expectation (§1) that nothing would survive and a fan would only
need to show that nothing survives at zero cost either. The four main runs
produced **one** row through the gate on **both** windows
(`np-brk-p4-m15`: A′ PF 1.636 / +0.106R / 96 trades, B′ PF 1.734 / +0.114R /
90 trades, and the same in the guarded arm). Falsifier 2 — "the conclusion
breaks at a spread below the release-minute spread" — is therefore a live test
rather than a formality, and it cannot be answered on one window when the
survival claim rests on two.

Added, and counted in the ledger: the same fan on B′ — 12 rows × 5 further
spreads = **60 further cells**, bringing the declared total from 120 to 180.
No row, parameter, window or threshold is changed; only the cost axis is
extended to the window the survival claim already stands on.

**Second thing this note has to record, because it was not anticipated.** The
guarded arm takes **0 trades on 8 of the 12 rows**. The guard config prints
`news flat 60/30 (impact≥3, USD)`: the desk's own live risk guard forbids
entries from 60 minutes before to 30 minutes after a high-impact USD release.
Rows at `probeBars` 1 and 2 enter at +0 and +15 minutes, inside that blackout,
so the guard refuses every one of them. Only `probeBars = 4` (+45 min) clears
it. This family was not merely unexpressed by the engine; **two thirds of it
is prohibited by the running configuration**, which is a second and
independent reason it had never been measured. It is reported as a fact about
the desk, not corrected: `[trading.guards]` is not touched.

**Third, the exit mix, which qualifies the survivor severely.** On all four
main runs the `probeBars = 4` rows leave by the engine's 4-hour cap in
**80–97 of 90–118 trades** (`TIMEOUT`), with the registered one-impulse-range
stop firing 6–16 times and the 1.8R target 3–5 times. So the thing measured is
**"enter 45 minutes after a release in the direction of the move so far and
hold for four hours"**; the stop and target geometry of §4 is very nearly
decorative. Any reading of the survivor that credits the invalidation rule is
wrong, and the brief's §8 trap is half-fired here: the rule's own exits fire,
but in under 10% of its trades.

---

## Note added 2026-10-06, results and the multiple-testing ledger

Append-only. No line above is altered.

### The gate

One row of twelve passes on **both** windows in **both** guard arms:

| row | window | trades | PF | expectancy | pct (null p50) | exit mix |
|---|---|---|---|---|---|---|
| `np-brk-p4-m15` | A' no guards | 96 | **1.636** | **+0.106R** | 99% (0.940) | STOP 6 / TARGET 3 / TIMEOUT 87 |
| `np-brk-p4-m15` | B' no guards | 90 | **1.734** | **+0.114R** | 94% (0.873) | STOP 7 / TARGET 3 / TIMEOUT 80 |
| `np-brk-p4-m15` | A' guards | 96 | **1.647** | **+0.107R** | 99% (0.940) | NEWS_FLAT 1 / STOP 6 / TARGET 3 / TIMEOUT 86 |
| `np-brk-p4-m15` | B' guards | 90 | **1.730** | **+0.114R** | 94% (0.873) | STOP 7 / TARGET 3 / TIMEOUT 80 |

`np-brk-p4-m05` passes A' (PF 1.768 / +0.137R / 107 trades) and **fails** B' on
expectancy in the fourth decimal (PF 1.220 / +0.0499R / 118 trades) — one
window, so it is not a pass. Count match 1.15, cost match 0.99 and long share
0.561 on that A' row: inside every band, so its percentile is readable.
`np-brk-p4-m15`'s count match is 1.28, just outside `COUNT_MATCH_BAND`, so its
99% / 94% is reported as **unmatched** and the gate alone carries it.

**The breakout branch is monotone in the probe, and the reversion branch is its
mirror.** On A' no guards: breakout PF 1.042 (probe 1) then 1.102 / 1.245
(probe 2) then **1.768 / 1.636** (probe 4); reversion, on the *same entries with
the side flipped*, 1.113 / 1.015 then 0.835 / 0.726 then **0.562 / 0.560**.
Flipping the side of 96 identical entries takes PF 1.636 to 0.560. That is the
direction test, and it is not a coin.

### Falsifier 1 — did NOT fire

Both branches were to pass 0 cells on both windows. The reversion branch did
exactly that: 0 of 24 cells, PF 0.22–1.11, every percentile at or below 68%.
The breakout branch did not. **The family is not closed.** The registration's
own §1 prediction — "both branches produce zero surviving cells" — is wrong,
and is left standing above exactly as it was written.

### Falsifiers 2 and 3 — the cost fan, and the first measurement of the spread at a release

`np-brk-p4-m15`, both windows, gate = PF >= 1.200 AND expectancy >= +0.050R:

| spread | A' PF / exp | B' PF / exp | both windows |
|---|---|---|---|
| 0.00 | 1.741 / +0.120 | 1.951 / +0.140 | pass |
| 0.14 | 1.687 / +0.113 | 1.839 / +0.127 | pass |
| **0.28** (config) | 1.636 / +0.106 | 1.734 / +0.114 | **pass** |
| 0.56 | 1.486 / +0.083 | 1.540 / +0.089 | pass |
| 1.12 | 1.300 / +0.054 | 1.220 / **+0.042** | **BREAKS** (B' expectancy) |
| 2.24 | 0.992 / -0.000 | 0.791 / -0.045 | breaks |

**The conclusion breaks between 0.56 and 1.12 — two to four times the
configured spread.** Falsifier 3 does not fire: it passes at zero cost too, so
the survival is not an artefact of mispricing in the other direction either.

Whether falsifier 2 fires turns on the spread this rule actually pays, and §7
said the desk had no per-minute series for it. **It does.**
`data/spreads/XAUUSD_sc.csv` is 8,967 timestamped bid/ask reads off the live
account's own `XAUUSD.sc`, 2026-09-15 02:01 to 2026-09-17 03:50, and that span
contains FOMC at 2026-09-16 18:00 UTC and its press conference at 18:30.
Measured, in price units:

| window around FOMC 18:00 | p50 | p90 | max |
|---|---|---|---|
| -120 to -60 min | 0.215 | 0.220 | 0.220 |
| -15 to 0 min | 0.220 | 0.250 | 0.260 |
| +0 to +5 min | 0.220 | 0.260 | 0.260 |
| +5 to +15 min | 0.250 | 0.260 | 0.260 |
| **+45 to +60 min** (this row's entry) | **0.250** | **0.260** | **0.260** |

The spread widens around the release by about 14–18%, from 0.22 to 0.25–0.26,
and **its maximum anywhere near the release, 0.260, is below the 0.28 the
engine already charges.** On this evidence falsifier 2 does **not** fire: the
break point, 0.56 to 1.12, sits two to four times above the worst spread
measured at the minute this rule enters.

**That evidence is thin and must not be quoted as settled.** The logger takes
one read about every 20 seconds (n = 3 a minute), so a blowout lasting seconds
is invisible to it; it is one FOMC, not a sample of releases; and it is 2026
data priced against 2018–2026 bars. What it does establish is the **sustained**
spread at +45 to +60 minutes after a release, which is the minute this row
trades, and that is the quantity the fan needed. A rule entering at +0 min
would need the first-seconds spread, which this log cannot see — and two
thirds of the rows that would have needed it take no trades under the guard
anyway.

One accident worth naming: `probeBars = 4` is simultaneously the only probe the
desk's own news guard permits (its blackout ends at +30 min) and the only probe
whose entry minute is far enough from the release for the spread to have come
back. Nothing in §4 chose it for either reason.

### Falsifier 4 — fired, in two places, and both are instrument findings

1. **`matched_rate` cannot calibrate a control on a gate this narrow.** The
   smoke run (one calendar year, about 32 admissible bars a row) produced count
   matches of 0.61, 0.60, 0.29, 0.27 and **0.04** — the control took 1 trade
   against the method's 25. The cause is structural and was declared in §5:
   `matched_rate` probes once at `entryRate = 0.02` and scales linearly, so on
   about 32 admissible bars the probe draws 0 to 1 trades and the scaling has
   no purchase. On the four-year windows (about 127 admissible bars) the
   matches land at 0.82 to 1.52, most inside the band. **`matched_rate` was
   not touched**, as §5 promised; the consequence is that a `newsonly:` row
   needs years, not months, before its percentile is readable at all. This is
   a previously unrecorded limit of the measuring instrument, found by this
   axis.
2. **The registered invalidation almost never fires.** 80 to 97 of 90 to 118
   trades on every `probeBars = 4` row leave by the engine's four-hour cap.
   Recorded in the previous note; repeated here because it is the single most
   important qualification on the surviving number.

### The brief's own windows, run and published

`xauusd` 15m, `--from=2025-07-01 --to=2025-10-01`, no guards, all 12 rows:
**6 to 9 trades a row**, against a 40-trade floor — the §6 arithmetic,
confirmed by the tool. The spread of outcomes at that size is the §5 warning
made visible: `np-brk-p2-m05` reads PF **4.200** on 9 trades and
`np-rev-p2-m05` reads PF **0.077** on the same 9 bars with the side flipped.
Neither number means anything. Receipt: `receipts/n1_briefwindowA_xauusd.txt`.

### Multiple-testing ledger

| block | declared | looked at |
|---|---|---|
| A' 2022-01-01 to 2026-01-01, 12 rows x 2 guard arms | 24 | 24 |
| B' 2018-01-01 to 2022-01-01, 12 rows x 2 guard arms | 24 | 24 |
| spread fan A', 12 rows x 5 further spreads | 60 | 60 |
| spread fan B', 12 rows x 5 further spreads (added in the note above) | 60 | 60 |
| brief window A on `xauusd`, 12 rows, no guards | 12 | 12 |
| **plumbing smoke run, `xauduka` 2025-01-01 to 2026-01-01, 12 rows, seeds 40 — NOT DECLARED** | 0 | **12** |
| **total** | **180** | **192** |

**Twelve cells beyond the declaration.** They are the single smoke run that
checked the new filter and the new strategy against the real store before the
registered windows were touched. It sits on a window that is neither A' nor B'
and overlaps A', its seeds were 40 rather than 200, and its numbers are quoted
nowhere above except as the `matched_rate` evidence in falsifier 4. It should
have been declared in §11 as a plumbing cell and was not. Recorded here rather
than left out.

`--null-sides=exposure` was **not** used and its 12 contingent cells were not
spent: every row's long share landed inside the 0.40–0.60 band on the
four-year windows (0.407 to 0.593), which is where
`docs/hypotheses/2026-10-02-drift-null.md` says the drift control should not
move a row. The one-year smoke run did read 0.64 to 0.93 long, which is worth
knowing on its own: over a single bull year the direction of the first move
after a release is itself long-biased, and only over four years does it come
back inside the band.

### Outcome

The family is **not** closed. One pre-registered rule — *45 minutes after a
high-impact USD release, take the direction of the move so far and hold* —
clears the desk's gate on two non-overlapping four-year windows, in both guard
arms, survives to twice the configured spread, and inverts to PF 0.56 when its
side is flipped on identical entries.

It is **not** a candidate. Its invalidation rule fires in under 10% of its
trades, so what has been measured is a four-hour directional hold and not the
geometry that was registered; its percentile is unmatched on count; it has
never been read on a second instrument, a second timeframe, or with a fill
model that reflects what a market order does in those minutes. The next step is
a direction null and a decision record, not a position.

### Receipts

    receipts/news-entry.toml                 the 12 registered rows
    receipts/n1_A_noguards.txt               A' 2022-01-01 to 2026-01-01
    receipts/n1_A_guards.txt                 the same, --guards
    receipts/n1_B_noguards.txt               B' 2018-01-01 to 2022-01-01
    receipts/n1_B_guards.txt                 the same, --guards
    receipts/n1_fan_{A,B}_s{0.00,0.14,0.56,1.12,2.24}.txt   the cost fan
    receipts/n1_briefwindowA_xauusd.txt      the brief's window A, not read at the gate

Every receipt's header carries `news: 747 events (2010-01-08 to 2027-12-08)
from E:/rust/flowdesk/data/news/events.parquet` and
`news scope: USD (news_currencies)`, so the calendar and the scope each run
read are quotable rather than assumed. `data-sealed/` was not opened, read or
counted.
