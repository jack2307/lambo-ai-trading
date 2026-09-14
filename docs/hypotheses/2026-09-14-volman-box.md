# 2026-09-14-volman-box: a breakout from a tight box travels two more boxes — Volman's measured move, mechanised

**Registered:** (commit time is authoritative) — after the method is
implemented and tested, before any run of this batch
**Status:** registered; amended once (the compression threshold), before any informative run
**Batch files:** `docs/hypotheses/2026-09-14-volman-box.toml` (the test, two
eight-year windows) and `docs/hypotheses/2026-09-14-volman-box-vantage.toml`
(context: the owner's recent-year criterion on the broker's own bars)

## Where this comes from

The owner asked for it (2026-09-14): Bob Volman's "three boxes" — measure
the height of a consolidation (box 1), copy it twice in the direction of
the breakout (boxes 2 and 3), enter on the breakout bar when it sits on
the right side of the 18-period EMA, stop beyond the far side of box 1,
take profit at the far edge of box 2 or box 3. The loop closed range
breakouts on gold on the long window — opening range, London range, ICT
sweeps, the volatility-conditional break, squeeze and Keltner breaks in the
recent-year screen — with direction nulls at the 43rd–54th, and the
standing rule says a closed family needs a new *mechanism*, not a new
stop. What is new here is not the entry (a box breakout is a range
breakout) but the exit structure: the stop and the target are both
denominated in the box's own height, so the risk unit is the consolidation
rather than an ATR, and the target is a measured move. That is a
different claim about *where a breakout goes*, and it is registered as
such, with the family's record on the table: the entry is expected to
carry no direction, and the test is whether the exit geometry changes the
arithmetic.

## Claim

On gold 5-minute bars, when the close of a bar exceeds the high (falls
below the low) of the preceding `boxBars` bars whose height is at most
1.5 ATR, and that close is above (below) the 18-period EMA and not more
than one ATR beyond the box edge, a position entered at the next open
with its stop at the box's far side and its target `boxes` box-heights
beyond the near side earns more than random entries with the same stop
and target geometry over the same bars, and more than the same entries
with their sides flipped. Two rows: the target at box 2's far edge
(`boxes = 1`, nominal reward-to-risk ≈ 1) and at box 3's (`boxes = 2`,
≈ 2). The box length is selected by the walk-forward from {12, 20, 30}
bars; everything else is pinned.

## Falsifier

Per row on the primary: the walk-forward's out-of-fold trades pass the
gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 trades), ≥ 95th percentile of
200 count-matched random entries with the same exits, and the direction
null (the entries with sides flipped, stops and targets mirrored) ≥ 95th;
the fixed replay at `boxBars = 20` reported alongside. Then the
confirmation window, the same. A row failing either window is closed and
the family stays closed. The Vantage context batch is reported in the
record under the owner's criterion and decides nothing about the claim.

## Base method

`NEW: volman-box` — `crates/fd-strategy/src/volman_box.rs`, implemented
and tested before this file is committed. Parameters `boxBars` (grid
{12, 20, 30}), `maxBoxAtr = 1.5`, `boxes` (1 or 2 per row), `emaPeriod =
18`, `atrPeriod = 14`, `maxBreakAtr = 1.0`. `Exits::Engine`: the engine
enforces the stop and the target, fills at the next open, and reads
nothing after the signal bar (tested). Not implemented, by intent: the
"early exit on sideways or a reversal bar with volume" (discretionary;
the Dukascopy feed has no volume) and the "trendline below the EMA"
re-measurement (no trendline in the method).

## Data

- Primary: `xauduka:5m` bounded `2010-06-01 → 2018-06-15` (never read on
  5m by any run; the 15m file of the same window has been).
- Confirmation: `xauduka:5m` bounded `2018-06-16 → 2026-05-31`.
- Context (the owner's criterion, separate batch): `xauusd:5m` (Vantage)
  `2025-04-11 → 2026-09-12`, walk-forward and fixed, the same nulls.
- Costs: Vantage's, $0.28 spread, swap-free, one ounce a unit.

## Sample needed

Boxes of 12–30 five-minute bars with a tight height occur several times a
day; the EMA and one-ATR filters thin them. Hundreds to thousands of trades
per window; the gate's 30 is not in question, the nulls are.

## What each outcome means

- A row passes both windows on all three conditions → the measured-move
  exit is what the breakout family lacked; a decision record and a
  paper-run proposal, which waits on the guards.
- Passes the primary only → closed; the record says which years.
- Fails the primary → closed; the family's verdict extends to this exit
  geometry, and the owner's context batch says what the last year looked
  like.

## Amendment (before any informative run): the compression threshold

The first run fired once in eight years at the registered parameters
(`in-sample-fixed.txt` under `one-trade/`: 1 trade; the walk-forward's
229 trades came from the 12-bar cell). `maxBoxAtr = 1.5` was written as if
the ATR were a 20-bar range; the ATR is one bar's, and the range of twenty
random-walk bars is three to four of them, so a box under 1.5 ATR almost
never exists. Amended to `maxBoxAtr = 3.0` — a twenty-bar box no taller
than three single-bar ATRs, which is a tight consolidation in Volman's
sense and a preset that can fire. Nothing else changes; the run that
produced this is kept under `one-trade/` and quoted nowhere as a result.
