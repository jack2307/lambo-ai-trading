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
