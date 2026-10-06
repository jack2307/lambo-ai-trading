# Exit anatomy: what the desk's record was actually measuring

Registered 2026-10-06, before any measuring run. Axis m9 (worktree
`/e/rust/fd-a9`, branch `agent/m9`).

## Why this axis exists

The CRT work (2026-10-04) found that the 4-hour hold ceiling
`max_hold_ms = 14_400_000` in `[trading]` closed 85-90% of the 1D positions,
so the 1D table did not measure CRT — it measured the ceiling. The brief also
names three mechanisms (`tsmom`, `volman-box`, `volume-thrust`) that reported
"0 trades" when the ceiling was the cause.

**No run before 2026-09-24 printed an exit reason.** So nobody knows how many
other rows in the desk's record are in the same position. `--exit-mix` prints
one extra line per row naming how each position left. This axis runs it over
every batch the binary can be asked for and every batch file in
`docs/hypotheses/`, and publishes one table.

This is a diagnostic, not a search. **There is no gate on this axis and no
cell to win.** A result of "only the 1D CRT rows were affected, the rest of
the record is clean" is the good outcome and will be reported as such.

## Hypothesis

One sentence: the 4-hour hold ceiling, the flat window and the position
guards close a material share of positions across the desk's record — not
only in the CRT 1D rows — so profit factors on the affected rows are
properties of the clock and the calendar rather than of the mechanism.

## Falsifiers — all three can fire

1. **Ceiling is local, not systemic.** If fewer than 5% of measured rows that
   have >= 40 trades show `TIMEOUT` > 50% of exits, the claim "the ceiling
   contaminates the record broadly" is DEAD and I will write that only the
   1D/CRT rows were affected.
2. **Targets are reachable.** If every row with >= 40 trades records at least
   one `TARGET` exit, the claim "some mechanisms have targets outside reach,
   which makes their PF meaningless" is DEAD for this record.
3. **The calendar is not a driver.** If `WEEKEND_FLAT` + `NEWS_FLAT` +
   `OPEN_LOSS_CAP` together are under 1% of exits in the guarded arm, the
   claim "part of the record is a consequence of the schedule" is DEAD.

## Thresholds for reading

- A row **is not measuring its mechanism** when `TIMEOUT` > 50% of its exits.
  Stated as a sharp line before any number was seen. A secondary, softer line
  (`TIMEOUT` > 25%) is reported alongside as a sensitivity, not as the rule.
- Minimum sample for any conclusion about a row: **40 trades** (brief §4: the
  same CRT rule on the same window gave PF 1.753 at 14 trades and PF 0.682 at
  178). Rows under 40 trades are printed but carry no conclusion.
- No gate. `profit factor >= 1.200 AND expectancy >= +0.050R` is not applied
  here because this axis selects nothing.
- Both windows: A `2025-07-01 → 2025-10-01`, B `2025-04-01 → 2025-07-01`.

## Protocol, fixed before running

Two arms, mirroring how the record was produced
(`docs/decisions/2026-09-23-three-month-search.md`: "`--mode=hypotheses
--fixed`, 200 seeds, guards on"):

- **Arm G (guards on)** — `--fixed --guards --exit-mix`
- **Arm U (guards off)** — `--fixed --exit-mix`

Arm U is the binary's default footing ("guards: off"). The pair is what
isolates how much of an exit mix belongs to the guards rather than to the
rule.

`--seeds=1` throughout. The exit mix is a property of the method's own
trades and does not depend on the matched null, so the null runs are pure
cost here. **Consequence declared up front: no percentile in any receipt of
mine may be read, and I will publish none.** Integrity check: one batch is
run at `--seeds=1` and at the default `--seeds=200` and the exit lines
compared; if they differ, the seed reduction is abandoned.

Market `xauusd` 15m, `--data=/e/rust/flowdesk/data`. `data-sealed/` is not
touched. The `news:` line of every receipt is recorded as the binary printed
it.

## Multiple-testing book — declared before running

Rows available, counted from source, not estimated:

| source | rows |
|---|---|
| `gold-intraday` (built in) | 13 |
| `ict-m1` (built in) | 4 |
| `ict-m5` (built in) | 4 |
| `ict-oos` (built in, the two `ict-B*` presets) | 2 |
| 45 batch files in `docs/hypotheses/*.toml` | 303 |
| **total rows per (window × arm)** | **326** |

**Declared: 326 rows × 2 windows × 2 arms = 1304 row-readings.** No cell is
selected from any of them; a row-reading here produces an exit histogram, not
a candidate. The book is published at the end as actual versus declared.

If the run cost forces a smaller set, the cut is written as a dated note at
the end of this file with the reason, and the rows dropped still count as
declared.

## Disclosure

One `search.exe` invocation preceded this registration: `--batch=__nope__`,
to make the binary print the list of batch names it knows. It refused the
batch and measured no row. Recorded here rather than left out.

## Writing rules adopted

Exit labels are the engine's own (`ExitKind::label`, `GuardKind::label`):
`STOP`, `TARGET`, `TIMEOUT`, `SIGNAL`, `END_OF_DATA`, `OPEN_LOSS_CAP`,
`WEEKEND_FLAT`, `NEWS_FLAT`. The map is keyed on `Trade::exit_reason`, so a
strategy's own wording (for example `flat window`, which the `flat:HHMM-HHMM`
filter emits as an `Intent::Exit`) appears under that wording and is reported
separately from `SIGNAL`. "Not measured" is printed as `null`, never as 0.

## Note 2026-10-06 (a) — seed reduction abandoned, and it cost nothing

The integrity check ran `gold-intraday`, window A, arm G at `--seeds=1` and at
`--seeds=200`: the thirteen `exits:` lines are byte-identical, which is what
the design predicted (the exit mix is a property of the method's own trades).
But the 200-seed run took 1.96 s against 0.79 s, so the reduction buys
nothing. **The whole sweep therefore runs at `--seeds=200`, the record's own
setting.** Nothing else in the protocol changes, and I still publish no
percentile — this axis has no gate and reads none.

## Note 2026-10-06 (b) — DIAGNOSTIC D1, declared before it is run

Reading the source while the sweep ran turned up something the brief gets
wrong, and it has to be checked rather than asserted.

`Exits::Strategy` (`crates/fd-strategy/src/registry.rs:113`) means the engine
imposes nothing, and `engine_exit` returns early on a self-managed position
(`crates/fd-backtest/src/engine.rs:737`, *"A stop on a self-managed entry is a
risk unit for sizing and R, not an order"*). Four mechanisms declare it:
`tsmom`, `session-hold`, `intraday-momentum`, `quiet-swing`. **For those four
the 4-hour ceiling cannot bind at all** — so the brief's "`max_hold_ms`
silently killed `tsmom` (0 trades in all 3 cells)" cannot be the mechanism of
those zeros, whatever else is.

D1 asks what the zeros are instead: run `2026-09-13-tsmom.toml` on a window
long enough that a 60-day and a 120-day lookback can warm up on 15m bars.
**It cannot produce a survivor and will not be read as one** — it is a
longer window chosen after seeing a zero, which is exactly the move the gate
exists to refuse. It can only say whether the zeros are a warm-up artefact of
the three-month bounds. Falsifier for D1: if the rows are still at 0 trades
with the lookback fully warmed, the warm-up explanation is dead too and I
report the zeros as unexplained.

D1 adds 3 rows x 1 window x 1 arm = **3 row-readings** to the declared book.

## Note 2026-10-06 (c) — D1 answered, and it forces D1-U, declared here before running

D1 (`receipts/D1-tsmom-long-window.txt`, 2022-01-01 → 2025-07-01, arm G):
the three `tsmom` rows return **163, 206 and 106 trades**, so the three zeros
on the three-month window were a warm-up artefact of the bounds, not the hold
ceiling. **D1's falsifier did not fire.** Mean hold 3,895 / 3,626 / 4,107
minutes — 65 to 68 hours, against a 4-hour ceiling — which is the measured
proof that a self-managed position bypasses `max_hold_ms` entirely.

The binary printed `SURVIVES` on two of those rows. **They are not
survivors**, for the reason declared in note (b) — a longer window chosen
after seeing a zero — and now for a second reason the exit mix supplies:
`tsmom/120d`'s 106 exits are 47 `NEWS_FLAT` + 58 `WEEKEND_FLAT` + 1
`END_OF_DATA`, and its own rule (`120-day return flipped`) fired **zero
times**. A row whose every exit was placed by the news window and the Friday
cut-off is not a measurement of the rule.

**D1-U, declared before running:** the same batch, same window, `--guards`
off. One arm, 3 rows, **3 row-readings** added to the book. It can show what
the rule's own exit does when nothing else is closing the position. It cannot
produce a survivor either, for the same window reason.

## Note 2026-10-06 (d) — DIAGNOSTIC D2, declared before it is run

The sweep's arm G shows the three highest profit factors in the whole record
sitting on rows whose positions are closed by the clock: `orb` cells at
TIMEOUT 71-72% with PF 2.3-2.5. The CRT decision record states the direction
as settled — *"a truncated hold can destroy a passing row, never manufacture
one"* (`docs/decisions/2026-10-04-crt.md`). That sentence has never been
measured. It can be, with a config value and no code change.

**D2.** `config-nocap/` is a copy of `config/` with ONE line added under
`[markets.xauusd.trading]`: `max_hold_ms = 604_800_000` (seven days), the
per-market override the config already supports
(`crates/fd-core/src/config.rs:499`, exercised by
`crates/fd-backtest/tests/trading_rules.rs:95`). Seven days rather than zero
so a position still has to end and `END_OF_DATA` does not absorb the answer.
`config/` itself is untouched, so every receipt already written stays
readable.

Arm G only, both windows, every batch — paired row by row against the main
sweep. **326 rows x 2 windows x 1 arm = 652 row-readings** added to the book.

It can answer one question: when the cap is lifted, does a row's profit
factor go UP or DOWN? If capped rows systematically look better than uncapped
ones, the cap manufactures passing rows and the quoted sentence is wrong. It
cannot produce a survivor — a row measured under a hold the record did not use
is not a cell of the record, and nothing here is selected.

Falsifier for D2: if lifting the cap leaves the profit factor of
ceiling-dominated rows unchanged or higher, then the cap is not inflating
those numbers and the CRT record's sentence stands.

# RESULTS, 2026-10-06

196 receipts in `receipts/exitmix/`, 1,304 row-readings, 103,005 exits
accounted for. Integrity check on the parse: the exit counts sum to
**103,005** and the rows' own trade counts sum to **103,005** — every trade is
in exactly one bucket, none invented, none lost.

Every receipt's `news:` line reads
`747 events (2010-01-08 to 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet`,
which is `--data=/e/rust/flowdesk/data`, the store this axis was told to use.
`data-sealed/` was not opened.

## Table 1 — how each mechanism's positions close

Arm G (guards on, the footing the record was produced on), both windows
pooled, `xauusd` 15m. `*` marks a mechanism that declares `Exits::Strategy`
and therefore has no engine stop, no engine target and **no hold ceiling** —
for those rows `%TIMEOUT` is 0 by construction, not by behaviour.
`rows w/o` is rows that took no trade at all, where the exit mix is `null`
rather than 0.

| mechanism | exits | rows w/ trades | rows w/o | %STOP | %TARGET | %TIMEOUT | %FLAT_WINDOW | %WEEKEND_FLAT | %NEWS_FLAT | %OPEN_LOSS_CAP | %SIGNAL | mean hold min |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| bb-fade | 4883 | 24 | 0 | 49.9 | 20.2 | 3.6 | 4.0 | 1.2 | 0.8 | 0.0 | 20.2 | 80 |
| crt | 7324 | 48 | 6 | 46.3 | 40.7 | 11.5 | 0.0 | 1.0 | 0.5 | 0.0 | 0.0 | 75 |
| doji-reversal | 1125 | 30 | 0 | 68.4 | 25.0 | 4.2 | 1.2 | 0.0 | 1.2 | 0.0 | 0.0 | 40 |
| donchian-breakout | 3872 | 26 | 0 | 27.2 | 18.2 | **22.9** | 5.8 | 1.5 | 1.1 | 0.0 | 23.0 | 130 |
| ema-cross | 1284 | 26 | 0 | 50.7 | 22.4 | 14.8 | 6.1 | 0.0 | 1.9 | 0.0 | 4.1 | 129 |
| gap-fade | 49 | 10 | 10 | 46.9 | 42.9 | 10.2 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 81 |
| ict-sweep-mss-fvg | 403 | 44 | 2 | 19.9 | 14.1 | **53.1** | 5.7 | 3.0 | 4.2 | 0.0 | 0.0 | 201 |
| intraday-momentum * | 468 | 8 | 0 | 0.0 | 0.0 | 0.0 | 20.1 | **19.7** | 0.0 | 0.0 | 60.3 | 72 |
| keltner-break | 2585 | 18 | 0 | 38.7 | 27.4 | **23.9** | 6.9 | 1.7 | 1.0 | 0.0 | 0.0 | 126 |
| macd-cross | 3282 | 18 | 0 | 49.2 | 29.0 | 9.7 | 9.1 | 1.7 | 1.0 | 0.0 | 0.0 | 102 |
| orb | 1972 | 52 | 8 | 26.0 | 13.6 | **57.8** | 0.0 | 0.2 | 2.4 | 0.0 | 0.0 | 185 |
| pdhl | 912 | 30 | 0 | 58.7 | 35.5 | 4.3 | 1.5 | 0.0 | 0.0 | 0.0 | 0.0 | 41 |
| rsi-reversion | 1808 | 24 | 0 | 47.1 | 5.8 | 6.7 | 7.9 | 1.8 | 0.7 | 0.0 | 29.6 | 103 |
| rsi2-pullback | 2307 | 18 | 0 | 35.9 | 37.4 | 14.5 | 8.5 | 2.2 | 1.5 | 0.0 | 0.0 | 121 |
| session-hold * | 5246 | 90 | 6 | 0.0 | 0.0 | 0.0 | 0.0 | 1.8 | 1.6 | **13.3** | 83.0 | 188 |
| squeeze-break | 499 | 18 | 0 | 46.3 | 35.5 | 9.0 | 6.6 | 1.6 | 1.0 | 0.0 | 0.0 | 73 |
| stoch-reversal | 4006 | 18 | 0 | 51.1 | 27.1 | 11.1 | 8.1 | 1.6 | 0.9 | 0.0 | 0.0 | 107 |
| trend-pullback | 3939 | 30 | 0 | 63.2 | 32.6 | 1.8 | 1.0 | 1.4 | 0.0 | 0.0 | 0.0 | 24 |
| tsmom * | 0 | 0 | 22 | null | null | null | null | null | null | null | null | null |
| volman-box | 366 | 8 | 2 | 28.4 | 34.4 | **33.9** | 0.0 | 2.2 | 1.1 | 0.0 | 0.0 | 150 |
| volume-thrust | 223 | 30 | 0 | 29.1 | 8.5 | **62.3** | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 185 |
| vwap-fade | 4311 | 26 | 0 | 62.0 | 29.9 | 4.3 | 2.0 | 0.7 | 1.0 | 0.0 | 0.0 | 58 |

Arm U (guards off) reproduces every figure within about one point on every
mechanism, with the four guard columns at exactly 0 — which is the check that
the guard columns are the guards and nothing else. The arm-U table is in
`receipts/exitmix/U-*`; the one place the two arms differ by a lot is
`session-hold`, whose mean hold is **188 min with guards and 303 min without**
because the open-loss cap is ending 13.3% of its holds early.

## Answer 1 — the ceiling is NOT a broad contamination, and the registered falsifier FIRED

Rows with at least 40 trades whose positions were closed by the four-hour
ceiling more than half the time:

| arm | window | rows >= 40 trades | TIMEOUT > 50% | share | TIMEOUT > 25% | share |
|---|---|---:|---:|---:|---:|---:|
| G | A | 206 | 11 | 5.3% | 32 | 15.5% |
| G | B | 201 | 7 | 3.5% | 27 | 13.4% |
| U | A | 207 | 10 | 4.8% | 33 | 15.9% |
| U | B | 202 | 8 | 4.0% | 27 | 13.4% |

The registered falsifier was *"fewer than 5% of measured rows with >= 40
trades show TIMEOUT > 50%"*. Three of the four arm-window combinations are
under 5% (3.5%, 4.0%, 4.8%) and one is just over (5.3%). Under the brief's
own both-windows rule a claim needs both windows, and window B gives 3.5% and
4.0%. **So the falsifier fired: the claim that the four-hour ceiling broadly
contaminates the desk's record is dead.** About 95% of the record's
well-sampled rows are not ceiling-dominated, and that is the headline answer
to question 1.

Dropping the sample floor looks worse (arm G: 91 of 596 rows with any trade,
15.3%), but those are rows of 1 to 20 trades where a single timeout is 5-50%
of the mix, and the brief's own 40-trade rule says they carry no conclusion.
The floor is kept.

**At mechanism level, six mechanisms are materially ceiling-bound and two of
them the brief did not name.** `volume-thrust` 62.3%, `orb` 57.8%,
`ict-sweep-mss-fvg` 53.1%, `volman-box` 33.9%, `keltner-break` 23.9%,
`donchian-breakout` 22.9%. `orb` and `ict-sweep-mss-fvg` are the two nobody
flagged — and `ict-sweep-mss-fvg` is the ICT expert's three presets, a named
line of the record, whose mean hold is 201 minutes against a 240-minute cap.

## Answer 1b — NOT PRE-REGISTERED, and the most consequential number here

Found after the fact, by looking. It was not a declared prediction, it has had
no out-of-sample test of its own, and it is reported as an observation rather
than as a result.

Arm G, >= 40 trades, both windows pooled, banded by the share of positions the
ceiling closed. "Pass gate" is `PF >= 1.200 AND expectancy >= +0.050R` —
applied only to describe where passing rows sit, never to select one.

| band | rows | median PF | pass gate | pass rate |
|---|---:|---:|---:|---:|
| TIMEOUT = 0% | 112 | 0.856 | 13 | 11.6% |
| 0 < TIMEOUT <= 5% | 73 | 0.814 | 0 | 0.0% |
| 5 < TIMEOUT <= 25% | 163 | 0.917 | 19 | 11.7% |
| 25 < TIMEOUT <= 50% | 41 | 1.056 | 14 | 34.1% |
| TIMEOUT > 50% | 18 | **1.462** | 11 | **61.1%** |

Arm U, independently: median PF 0.822 / 0.787 / 0.918 / 1.062 / 1.331, pass
rates 14.3% / 0.0% / 9.1% / 40.5% / 55.6%. **Two arms, two windows, the same
ordering across the top three bands.**

So the ceiling is rare, but it is not randomly placed. **11 of the 57 rows
that pass the gate in arm G (19.3%) are rows where the ceiling closed more
than half the positions, and 25 of 57 (43.9%) where it closed more than a
quarter.** The highest profit factors in the whole sweep are `orb` London
cells at TIMEOUT 71-85%:

| window | cell | trades | TIMEOUT | PF | expectancy |
|---|---|---:|---:|---:|---:|
| A | `gold-m15-check/m15/london`, which is byte-identical to `london-range/london/nyam`, `volcond-breakout/hivol/london` and `rebate-rescore/atr/hivol-london` | 51 | 71% | 2.548 | +0.250R |
| A | `volcond-breakout/hivol/london` | 50 | 72% | 2.345 | +0.226R |
| B | the first cell again, window B | 46 | 85% | 0.999 | -0.002R |

The same cell is **PF 2.548 at 71% timeout in window A and PF 0.999 at 85% in
window B**, which is also the cleanest demonstration in this sweep of why one
window is not a result.

`docs/decisions/2026-10-04-crt.md` states the direction of this bias as
settled: *"a truncated hold can destroy a passing row, never manufacture
one."* The table above is consistent with the opposite. That sentence needs a
measurement rather than an argument, and that is D2.

## Answer 2 — targets ARE reachable; the registered falsifier FIRED

49 to 51 rows per window with >= 40 trades record zero `TARGET`. **Almost all
of them are mechanisms that have no engine target at all**: `session-hold` and
`intraday-momentum` declare `Exits::Strategy`, so there is no engine target to
reach, and a zero there is a fact about the engine's contract rather than
about the rule's reach.

Restricted to the 309 arm-G rows with >= 40 trades whose mechanism is
engine-exited, the rows with zero `TARGET` number **2** — and they are one
cell counted twice, `rsi-reversion/ny` in window B, which appears identically
in `recent-year-screen.toml` and `recent-year-screen-5m.toml`: 56 trades,
STOP 24, SIGNAL 15, FLAT_WINDOW 9, TIMEOUT 6, WEEKEND_FLAT 2, TARGET 0,
PF 0.625. Its own `RSI back to midline` exit fires first, which is why.

**So the CRT `1d-opposite` row — 0 targets in 29 trades across two windows —
is not a pattern in this record. It is the exception.** Every other
engine-exited, well-sampled row in the desk's record reaches its target at
least once, and no profit factor in the record has to be thrown away on that
ground.

`rsi-reversion` is nonetheless the one mechanism whose target is nearly
decorative: **5.8% of its 1,808 exits**, against 47.1% stops. Not out of
reach; very rarely reached.

## Answer 3 — the calendar cuts 3.8% of exits overall, and up to 67% of one row

Arm G, all 596 rows with trades, 50,864 exits:

| bucket | exits | share |
|---|---:|---:|
| STOP | 21,350 | 41.97% |
| TARGET | 12,517 | 24.61% |
| SIGNAL (the rule's own wording) | 7,101 | 13.96% |
| TIMEOUT | 5,941 | 11.68% |
| FLAT_WINDOW (a batch's own `flat:` filter) | 1,946 | 3.83% |
| WEEKEND_FLAT | **749** | **1.47%** |
| OPEN_LOSS_CAP | 698 | 1.37% |
| NEWS_FLAT | 502 | 0.99% |
| END_OF_DATA | 60 | 0.12% |

`WEEKEND_FLAT + NEWS_FLAT + OPEN_LOSS_CAP = 3.83%` of all exits, so the
registered falsifier (*under 1% and the calendar claim is dead*) **did not
fire**. In arm U all three are exactly 0, which is the control. Note that
`FLAT_WINDOW` is a further 3.83% of exits placed by a clock, and it is not a
guard at all — it is the `flat:1630-1815` filter the batches carry, and it
reaches the exit map under the strategy's own wording rather than under a
guard label.

The aggregate is small; the concentration is not. Per row:

- **67%** of `btc-us-hours/hold/asia`'s 51 positions in window B were closed
  by the open-loss cap (34 of 51) — two thirds of a row's trades ended on a
  risk guard, not on the rule.
- `session-hold` pooled: **13.3%** `OPEN_LOSS_CAP`.
- 17 rows with >= 10 trades carry `WEEKEND_FLAT` at 19-20% of their exits —
  every Friday, by construction, on any rule still holding at 16:40 New York.
- `intraday-momentum`: 19.7% `WEEKEND_FLAT` plus 20.1% `FLAT_WINDOW`, so
  **40% of its exits are placed by a clock** and 0% by a stop or a target.

**23 of 398 paired rows (5.8%) cross the gate's PF 1.200 line when the guards
are switched on or off, with nothing else changed.** The largest single move:
`btc-us-hours/hold/off-hours`, window A, **PF 2.328 unguarded against 0.894
guarded, on identical 64 trades** — a factor of 2.6 on one row from the guard
configuration alone. In the other direction
`recent-year-hours/hold/02-04-long` window B goes 0.980 to 1.210, so a guard
setting moves a row from failing to passing.

Any receipt in this record that does not state its guard arm is therefore
unreadable at the gate. The three-month search's own record does state it
(`--fixed`, 200 seeds, guards on), so that one is safe.

## tsmom: the brief's claim about it is wrong, and D1 says what the zeros were

`tsmom` declares `Exits::Strategy`, and `engine_exit` returns early on a
self-managed position (`crates/fd-backtest/src/engine.rs:737`) — **the
four-hour ceiling cannot touch it.** All 22 `tsmom` row-readings in this sweep
returned 0 trades, so their exit mix is `null`, and `--exit-mix` is
structurally unable to explain a zero-trade row.

D1 (`receipts/D1-tsmom-long-window.txt`, 2022-01-01 to 2025-07-01, arm G; not
a gate cell and not read as one): the same three rows return **163, 206 and
106 trades** with mean holds of **3,895 / 3,626 / 4,107 minutes** — 65 to 68
hours under a 4-hour cap, measured. The zeros were the three-month bounds
failing to warm up a 20/60/120-day lookback on 15m bars, not the ceiling.

D1 then produced the most striking exit mix in this axis. `tsmom/120d`, 106
trades, PF 2.236, printed `SURVIVES` by the binary:

    exits: END_OF_DATA 1, NEWS_FLAT 47, WEEKEND_FLAT 58; mean hold 4106.8 min

**Its own rule — `120-day return flipped` — fired zero times.** Every closed
position was closed by the news window or the Friday cut-off. D1-U
(`receipts/D1U-tsmom-long-window-unguarded.txt`) is the same rule and window
with the guards off:

| row | arm G trades | arm G PF | arm U trades | arm U PF | arm U exits |
|---|---:|---:|---:|---:|---|
| tsmom/60d | 163 | 1.339 | 27 | 3.608 | `60-day return flipped 26, END_OF_DATA 1` |
| tsmom/20d | 206 | 1.209 | 61 | 1.488 | `20-day return flipped 60, END_OF_DATA 1` |
| tsmom/120d | 106 | 2.236 | 1 | inf | `END_OF_DATA 1` |

**The guards multiply the trade count of a hold mechanism by 6.0x, 3.4x and
106x** by flattening the position and letting it be re-entered afterwards. A
row reporting "163 trades" contains **27** decisions the rule itself ever
made. Since sample size is the whole basis on which this desk believes or
disbelieves a number (brief section 4: the same rule, same window, PF 1.753 at
14 trades and PF 0.682 at 178), **every trade count on a hold mechanism in the
guarded record is inflated, and the percentile read against it was read
against the wrong n.** Neither arm of D1 is a survivor and neither is claimed
as one.

## D2 — the four-hour cap DOES manufacture edge on the rows it binds hardest

98 receipts in `receipts/nocap/`, 652 row-readings, arm G, both windows, the
`xauusd` hold cap lifted from 4 hours to 7 days by a per-market override in
`config-nocap/` and nothing else changed. Every receipt prints
`max hold: 168 h (604800000 ms) from [markets.<id>.trading] max_hold_ms = 604800000`,
so the override is in the receipt and not only in the plan.

638 rows pair one-to-one with the capped sweep on `(window, batch, label)`.

| band, by TIMEOUT share UNDER the cap | rows | median PF change when the cap is lifted | PF lower | higher | unchanged | median hold, capped to uncapped |
|---|---:|---:|---:|---:|---:|---|
| TIMEOUT = 0% (the control) | 112 | **+0.000** | 0 | 0 | **112** | 111 to 111 min |
| 0 < TIMEOUT <= 5% | 72 | -0.001 | 38 | 30 | 4 | 51 to 54 min |
| 5 < TIMEOUT <= 25% | 157 | -0.002 | 83 | 71 | 3 | 108 to 128 min |
| 25 < TIMEOUT <= 50% | 39 | **+0.022** | 11 | 27 | 1 | 152 to 226 min |
| TIMEOUT > 50% | 18 | **-0.065** | **13** | 5 | 0 | 188 to 329 min |

The first row is the integrity check: 112 rows whose positions the cap never
closed have a profit factor change of exactly zero, on all 112. The config
change touched only what it was supposed to touch.

**The registered D2 falsifier did not fire on the band that matters.** On the
18 rows the cap bound hardest, lifting it made the profit factor **worse on 13
of 18** and the median fell 0.065. So on those rows the truncation was
flattering the rule, not destroying it, and
`docs/decisions/2026-10-04-crt.md`'s *"a truncated hold can destroy a passing
row, never manufacture one"* is **false as stated**. The mechanism is not
mysterious: a breakout that is 188 minutes old and in profit gets marked out
at the bar's close instead of being left to give the move back, and the median
hold of those rows more than doubles (188 to 329 minutes) once it is allowed
to run.

**But the size of the problem is small, and that has to be said as plainly as
the direction.** Of the **56** rows that pass the gate under the cap with >= 40
trades, **4 stop passing when it is lifted** — three distinct cells:

| window | cell | base | PF capped to uncapped | expectancy capped to uncapped |
|---|---|---|---|---|
| A | `crt/4h-opposite` | crt | 1.225 to **0.997** | +0.075R to **-0.068R** |
| A | `crt/4h-rr15` | crt | 1.250 to **1.054** | +0.141R to +0.123R |
| A | `box/b2` (in `volman-box.toml` and `volman-box-vantage.toml`) | volman-box | 1.376 to **1.279** | +0.063R to **+0.035R** |

And **3 rows pass only once the cap is lifted**, all CRT 4H cells in window B:
`crt/4h-opposite`, `crt-flip/4h-opposite`, `crt-nocap/4h-rr15`.

So the honest reading is: the cap moves roughly **7% of the record's
gate-passing rows across the gate line**, in both directions, and it moves the
CRT 4H cells across it in *opposite* directions in the two windows. The four
largest profit factors in the record — the `orb` London cells at 71-72%
TIMEOUT — still pass without the cap (2.548 to 2.295 and 2.345 to 2.142), so
they are not artefacts of it; they are just small samples that disagree
between windows.

**The consequence for the CRT branch.** That record reports 1D as NOT
MEASURED and 1H/4H as refuted. D2 says the 4H cells also cross the gate line
when the hold cap moves — losing a pass in window A, gaining one in window B.
The refutation at 4H is therefore softer than the record states: it survives
on direction (the cap flattered 4H in window A and 4H still failed overall)
but the 4H verdict is cap-sensitive and deserves the same "measured under a
cap that bound 30% of its positions" caveat the 1D rows got. The 1H verdict,
at 9.8% TIMEOUT, is not affected.

# Multiple-testing book — actual against declared

| item | declared | actually read |
|---|---:|---:|
| main sweep: 326 rows x 2 windows x 2 arms | 1,304 | **1,304** |
| D1 (`tsmom`, long window, arm G) | 3 | **3** |
| D1-U (same, guards off) | 3 | **3** |
| D2 (326 rows x 2 windows, cap lifted, arm G) | 652 | **652** |
| **total** | **1,962** | **1,962** |

No overrun. Of the 1,304 main row-readings, 1,276 are distinct on
`(arm, window, batch, label)`; the 28 repeats are labels that appear twice
inside one batch file. 319 distinct `(batch, label)` record rows were read,
spanning 236 distinct `(mechanism, cell)` pairs — the duplication is real and
it is the record's own: `orb/60m` at 44 trades and PF 1.341 appears under five
different names in five different batch files, and a reader counting receipts
would count it five times.

**Nothing was selected.** This axis has no gate and proposed no cell. The gate
appears in these tables only as a way of saying where the record's passing
rows sit, and every row it touches was already in the record.

# What this axis could NOT measure

1. **A zero-trade row.** 106 of the 1,304 main row-readings took no trade, so
   their exit mix is `null`. `--exit-mix` reports how positions closed; it is
   silent on why none opened. The `tsmom` zeros needed a second window to
   explain and the `gap-fade` (20 rows) and `orb` (16 rows) zeros are still
   unexplained here.
2. **The `SIGNAL` bucket is not one thing.** The exit map is keyed on
   `Trade::exit_reason`, so a strategy's own wording and the `flat:` filter's
   `flat window` both land there. I split `flat window` out because its
   wording is fixed, but a rule with several exit reasons of its own cannot be
   separated from a rule with one. 13.96% of exits are in that bucket.
3. **Per-trade attribution.** The mix is counts, not R. A row where the cap
   closed 20% of positions may have had 80% of its profit in those 20%, and
   nothing here can say so. D2 answers the question at row level, which is
   coarser.
4. **BTC and the other markets.** Everything here is `xauusd` 15m, as the
   brief set. Rows designed for BTC, EUR and silver (`btc-us-hours`,
   `tsmom-eurusd`, `tsmom-silver`, `fx-local-hours`, …) were re-measured on
   gold, so their exit mixes describe how those mechanisms behave on gold and
   are **not** reproductions of the original receipts. The 67%
   `OPEN_LOSS_CAP` figure is the clearest case: that is `btc-us-hours/hold/asia`
   run on gold.
5. **Whether any of this would survive out of sample.** Nothing here was
   proposed, so nothing needed to.
