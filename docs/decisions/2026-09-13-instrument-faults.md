# Three engine faults found by reading trade lists, and what they touched

**Date:** 2026-09-13 (late morning)
**Question:** When a walk-forward and a fixed replay of the same pinned row
disagreed (tsmom-2: PF 0.78 vs 2.03 on nearly the same 75 trades), was the
market or the instrument responsible?
**Outcome:** The instrument, three times over. Each fault is fixed in its own
commit with a pinning test, and every earlier record that could have been
touched was re-run and either unchanged in outcome or re-checked on its
window. No outcome in `docs/decisions/` changes.

## The faults

1. **A sizing stop on a strategy-managed hold was enforced** (`f66c7bf`).
   `check_exit` skipped the engine's stop/target/clock for `Exits::Strategy`
   only when the entry carried no stop. tsmom-2's entries carry a stop for
   sizing, so the first run tested a stopped, four-hour TSMOM: 1,237 trades
   where the claim has 75 (`runs/2026-09-13-tsmom-2/stop-enforced/`). Now a
   self-managed position closes only on the strategy's signal or the end of
   its window. Test: `tests/self_managed.rs`.
2. **A preset naming a parameter at its default was not pinned** (`f66c7bf`).
   `Preset::grid` read "pinned" as "differs from the default". `gap/1atr`
   (1.0, the default), `tsmom2/60d` (60, the default) and every `*/fixed` row
   were re-selected by the walk-forward and equalled their grid rows. Pinned
   now means named in the overrides. Fixed receipts were never affected.
3. **A position open at a fold's end ran to the end of the data** (`9fde7c1`).
   Past `Range.to` the strategy was not consulted but the book was not
   flattened; a self-managed hold opened in fold 1 (window to 2021-03-08)
   closed 2025-04-09 at −27R, one such trade per fold. Engine-exit positions
   leaked less (closed by their stop or clock after the boundary). Now the
   book is flat at the first bar past the range. Test: `tests/self_managed.rs`.

And one null that was not measuring what its label said: for a method that
decides exits from the side it holds, `DirectionFlipped` kept the exit rule
inside the direction null (median PF 1.56 on tsmom-2). Replaced for
`Exits::Strategy` methods by a permutation of the method's own trades'
sides (`9ae323f`, `runs/2026-09-13-tsmom-2/flip-replayed/` keeps the old
receipts).

## What was re-run

- **btc-us-hours** (`session-hold`, walk-forward): `hold/us-hours` PF 0.781 →
  0.881 (37th → 26th percentile), `hold/off-hours` 0.946 → 0.946, `hold/asia`
  0.892 → 0.892; the null's 95th percentile fell from 2.98 to 1.24 — the
  runaway trades had been in the null as well. Outcome unchanged: every row
  fails the gate. Prior receipts under `runs/2026-09-13-btc-us-hours/before-fold-close/`.
- **london-fix** (`session-hold`): the batch file has no lower bound and the
  Dukascopy file has since been extended to 2018, so the re-run is on
  2018–25 (1,401 holds per row), not the record's 2022–25 (578). On the
  longer window: `fix/pm-into` 0.935 (92nd), `fix/pm-after` 0.932 (96th, gate
  fail on profit factor), `fix/am-into` 0.536, `fix/am-after` 0.683. Outcome
  unchanged. Prior receipts under `…/before-fold-close/`.
- **tsmom** (the first registration) was parked on sizing before any of
  this; not re-run — `tsmom-2` is its re-run.
- Every `Exits::Engine` record: a fold boundary can now cut at most one
  four-hour trade per fold; not re-run, and the records' percentiles should
  be read as ±1 trade per fold.

## What this does not say

- It does not say the earlier closed verdicts were wrong; each was checked
  and stands.
- It does not say the engine is now correct; it says three more things it
  did wrong are now things it is tested not to do. The skill's standing rule
  is the residue: read the trades before the tables.
