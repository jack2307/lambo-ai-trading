# Registration — why 34 decision-record rows no longer re-run to their published number

**Date:** 2026-10-11 (registered before the first engine run of this axis)
**Branch:** `agent/receipt-reconcile`, cut from `agent/stop-width` (`29c8635`).
**Binary:** `/e/rust/fd-stop-width/target-sw/release/search.exe`, built
2026-10-08 20:57 from `agent/stop-width` with the `lbar_line` patch. **Nothing
is built on this axis.**
**NO GATE.** Nothing here passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`. This
axis repairs the RECORD. No gate cell is spent; the ledger below counts
diagnostic re-runs, not candidates.

## The population — counted, not inherited

`agent/pfr-audit` (`76c0b57`) reported **51** published band keys whose command
still runs but which no longer print a cell at the published trade count, of
which **34** sit in `docs/decisions/`.

I did not take 34 on trust. I rebuilt the set from that axis's own surviving
artifacts — `gate_legs_rows.json` (6,589 rows carrying both a PF and an
expectancy), `cmd_of.json`, `repro_band.json` and all **88** completed runs —
with `py/research/receipt_reconcile_rows.py`, pairing on **the same label AND
the same trade count**, exactly the rule `pfr-audit` declared.

    vanished band keys                      51
    of them sitting in docs/decisions/      34

**My count is 34.** The criterion is stated so it can be disagreed with: a row
counts when (a) its `PF_usd` is inside `pfr-audit`'s declared band
[0.700, 1.700], (b) at least one blob carrying it also echoed the argv that
made it, (c) that argv completed `exit 0` in the `pfr-audit` run set, (d) no
cell in that run carries the same label at the published trade count, and
(e) at least one blob carrying the row is a file under `docs/decisions/`.

Precedent for not forcing the number: brief §8 — the measurement wins — and the
"6 receipts say `data-sealed`" claim that was really **48 blobs in five
directories**.

## Hypothesis (one sentence)

Every one of the 34 rows fails to reproduce for a reason that can be named from
the receipt's own header and the engine's own source, without re-deciding the
hypothesis the row was testing.

## THE CLASSIFICATION TABLE — declared here, before any run

Classes are declared **before** running, and each carries the test that assigns
a row to it. A row that fits none of them is reported as its own class, not
forced into one.

    W1  STORE BACKFILLED UNDER AN OMITTED `--from`
        test: the published receipt's own `bars:` line starts LATER than the
        re-run's, the command has no `--from=`, and the trade-count ratio
        tracks the bar-count ratio.
        positive control: pin `--from=<published first bar date>` and the
        published trade count must COME BACK.

    W2  STORE GREW AT THE LIVE END UNDER AN OMITTED `--to`
        test: published `bars:` ENDS EARLIER than the re-run's, no `--to=`,
        and the drift is a handful of trades.
        positive control: pin `--to=<published last bar date>` and the
        published trade count must COME BACK.

    P1  A PRESET THAT DID NOT PIN A SWEPT GRID AXIS
        test: the TOML's `overrides` names a key that is ALSO in the
        strategy's `grid()`, so the walk-forward re-selected it. The engine
        source documents this and names the file: `hypotheses.rs:1347-1351` —
        "the walk-forward would replace the registered value with the grid's
        and the row would test something else — which it did, once
        (`2026-09-13-vwap-fade.md`)". Fixed by `f4c8e7c`.
        positive control: delete the colliding override from a COPY of the
        TOML; the published number must come back, because an unpinned
        override is what the published run effectively had.

    G1  GUARD SEMANTICS CHANGED UNDER A PINNED WINDOW
        test: both window ends pinned, bar counts identical, `--guards` on,
        and the published receipt's own `guards:` line describes behaviour the
        re-run no longer shows.

    E1  RESIDUAL ENGINE DRIFT AT A PINNED BOUNDARY
        test: both ends pinned, bar counts identical, no guards, no grid
        collision, and the drift is <= 1 trade. Named, not explained away.

    U   UNREADABLE — the binary, the flag, the batch file or the data needed is
        gone. Reported as `KHÔNG ĐỌC ĐƯỢC`. `null != 0 != []`.

**Classes declared not to apply, with the reason, so they are not reached for
later:**

- **Silently swallowed CLI flag.** Every flag in all 34 published commands is
  in the shared binary's own flag list; `pfr-audit` rewrote only the binary,
  batch-file, `--data=` and `--config=` paths. The brief's own guard against
  this class is the source-scanning flag test on `agent/instr-repair`.
- **`null_sides` inert in `[run]`**, and `[run]` unable to set a window under
  `--fixed`. Both are real, and both move **percentiles and verdict words**,
  not the **trade count** — which is the only thing that makes a row unpairable
  here.
- **Null width shrinking as `1/sqrt(n)`**, and **seats in the TOML**. Same
  reason: they move the percentile, not the trade set. They are the right
  explanation for a *verdict* that no longer reproduces; these 34 rows do not
  reproduce their *trade count*.
- **Unit (`PF_usd` vs `PF_r`).** Cannot be the cause for any of the 34: the
  unit changes neither the trade count nor the trade set, and these rows are
  unpairable precisely because the trade count moved. The FALSIFIER below is
  the formal version of this.
- **Seeds.** `pfr-audit` MEASURED `--seeds=1` against `--seeds=200` printing the
  method row byte-identical, and `hour-screen` measured `--seeds=5` against
  `--seeds=100` agreeing to the last digit on 3/3 rows. Seeds are not the
  default explanation for anything here.
- **`data-sealed/`.** Not opened. Not the cause: every one of the 34 commands
  echoes the live store.

## FALSIFIER — specific and fireable

**If the MAJORITY of the 34 turn out to be a reporting-unit mistake** — a
`PF_usd` published as if it were `PF_r`, with the measurement itself sound —
then the record is not contaminated at the measurement layer, the cure is a
WRITING RULE (always declare the unit) and not a re-run of 6,000 rows, and this
axis says so and stops.

**It is already pointed at its own answer:** a unit mistake cannot move a trade
count, and all 34 are unpairable *because the trade count moved*. So I expect
this falsifier NOT to fire, and I am stating in advance what that costs: every
row that lands in **P1** or **G1** is a desk conclusion standing on air, and
for each of those I must name **which conclusion**.

## Multiple-testing ledger — declared BEFORE running

Diagnostic re-runs only; each is a POSITIVE CONTROL for one class, not a test of
a hypothesis.

    W1 controls (pin `--from` to the published first bar)        7 commands
       xauduka 5m x4 TOMLs ; xauduka 15m x1 ; xauduka 5m OOS x2
    W2 controls (pin `--to`  to the published last bar)          3 commands
       xauusd 15m x1 ; xauusd 5m x2
    P1 controls (override deleted from a COPY of the TOML)       2 commands
       btc 5m ; btc 15m
    G1 / E1 reads                                                0 commands
       the two re-runs already in the `pfr-audit` set are read as they are
    ------------------------------------------------------------------
    DECLARED CEILING                                            12 commands

A control counts as PASSED only if the published trade count returns
**exactly**. A control that returns a different number is reported as a control
that FAILED, and its rows move to a residual class — it is never rounded into
agreement.

## How the record is repaired

Brief §5 and §8: **a correction is an appended dated note; no line above it is
rewritten.** For each of the 34 I write: the published number, the number it
re-runs to now, the class, and **which desk conclusion depends on it.** That
last clause is the point of the job — a wrong number nobody leans on is not the
same object as a wrong number holding up a direction.

## Not promised

- No new measurement program. If a row needs one to be adjudicated, it is
  written down as an OPEN item and the axis moves on.
- No drawdown criterion (addendum 5 G).
- No percentile and no verdict word is re-derived. `SURVIVES` depends on the
  seat in the TOML and on null width; this axis reads trade sets.
- `main` untouched, no history rewritten, no published receipt edited —
  additions only. `data-sealed/` not opened. `config/local.toml` and
  `config/accounts.toml` outside this worktree untouched, `--data=` read-only,
  `collect.exe` pid 5044 / 38720 not touched.
- Bar count and print count of my own runs are printed, because the store is
  being written while it is read.
