# 2026-09-13-london-fix: gold drifts down into the afternoon fix and recovers after it

**Registered:** 2026-09-13 02:12 (commit d98576d) — before any run
**Status:** decided → docs/decisions/2026-09-13-london-fix.md (failed the primary; confirmation not opened)
**Batch file:** `docs/hypotheses/2026-09-13-london-fix.toml`

## Claim

The LBMA afternoon fix (15:00 London = 10:00 New York in both DST regimes,
give or take the week the two zones disagree) is a scheduled auction where
producers and refiners sell. Published work on the pre-2015 fix found prices
drifting *down* in the half hour before it and partly recovering after. The
claim here is that trace of it survives: **short from 09:30 to 10:00 New
York loses less, or earns more, than a coin flip of the same hold; long from
10:00 to 10:30 likewise.** A drift claim, tested with the drift control.

## Falsifier

For each row: the gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 holds) and ≥ 95th
percentile of 200 random holds of the same length entered at the same
minute; the direction null (same holds, coin-flip side) outside. Both windows
must survive. The morning-fix rows (05:30 → 06:00 New York, the 10:30 London
fix) are the contrast: the literature's effect was in the afternoon fix.

## Base method

`session-hold` (exists): `from/to` in New York hhmm, `side`. Control:
`null-hold` with `holdMinutes` = the row's window, entering only where the
row's `hours:` filter allows (the window's first bar).

## Data

- Primary: `xauduka:5m` 2022-06-16 → 2025-04-10
- Confirmation: `xauusd:5m` 2025-04-11 → 2026-09-11 — opened after the
  primary result is recorded
- Weekdays only. No flat window needed (the holds end by 10:30).

## Sample needed

~700 holds per row on the primary. Plenty; a 30-minute hold on a $0.28
spread needs a drift of a few dollars to show, which is what the claim is.

## What each outcome means

- Afternoon rows survive both windows and the drift control, morning rows
  do not → a scheduled-flow effect; record; paper-run proposal.
- Survive primary only → cost or regime; record says which.
- Fail primary → the fix effect, if it existed, is gone at retail cost. Closed.
