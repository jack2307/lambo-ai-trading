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

## Addendum, 2026-09-14 (small hours): four more, found by the reviews

All found by reading receipts against code during the night's reviews, all
fixed with tests before the next registration ran, none changing a verdict:

1. **The sizing range counted Sunday evenings as days** (`average_day_range`,
   data-integrity on `2026-09-13-close-reopen-drift`): a six-hour Sunday
   session entered the 20-day mean as a full day, shrinking R by 6–9%,
   uniformly. Fixed at `f65a061`: a day whose bars span less than half the
   longest day in the window is not a day; half-days and one-bar-a-day feeds
   still count.
2. **The hold null guessed half the lookback** for a rebalanced hold
   (adversary on `2026-09-13-tsmom-silver`): 27% short of the 20-day row's
   realised hold, 2.4× the 120-day row's. Fixed at `f65a061` to the method's
   mean realised hold, then — because *"a fixed 23-day random hold cannot
   produce a 356-day, +23R trade"* (adversary on `2026-09-14-tsmom-eurusd`)
   — at `e0d46c7` to a log-normal draw per trade around the geometric mean
   of the realised holds, with the realised log-sd.
3. **The hold null exited one bar late** (adversary on
   `2026-09-14-intraday-momentum`): it signalled on the bar where the
   minutes had elapsed, and the engine fills at the next open, so every
   window null held one bar longer than the method — 15 minutes on a
   two-hour hold, and across the daily halt or the weekend when that bar
   was the day's last (the 75-minute null on gold held to 18:00, and to
   Sunday on 399 Fridays; sized-null p50 0.677 against the direction null's
   0.619 on the same window). Fixed at `4eaef94`: signal when
   `held + bar interval ≥ hold`. The session-hold records before it
   (`2026-09-13-close-reopen-drift`, `2026-09-13-friday-weekend-hold`, the
   `recent-year-hours` context) carry sized-null percentiles from the late
   control; on all of them the direction null — the method's own trades,
   unaffected — decided the row the same way, and the records say so.
4. **Filters gate entries only** (data-integrity on `2026-09-13-tsmom-silver`):
   `weekdays` does not stop a self-managed method from acting on a signal at
   the Sunday reopen bar as an exit. Not changed — a registration that wants
   no Sunday exits has to say so in the method — and stated in every tsmom
   record since.

The pattern of the first record holds: none of these was visible in a
table. Two came from an adversary comparing the two null columns to each
other, one from a data-integrity replay of the method's own rule, one from
reading a trade list's exit stamps.
