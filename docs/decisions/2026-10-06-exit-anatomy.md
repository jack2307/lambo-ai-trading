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
