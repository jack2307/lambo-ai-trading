# 2026-09-13-volcond-breakout: a range break pays only when volatility is high in absolute terms

**Registered:** 2026-09-13 03:20 — before any run; threshold chosen from the
two windows' ATR distributions, not from any result
**Status:** decided → docs/decisions/2026-09-13-volcond-breakout.md (failed the primary window; confirmation not opened)
**Batch file:** `docs/hypotheses/2026-09-13-volcond-breakout.toml`

## Claim

Three break methods (ICT, ORB-NY, London-range) passed on the 2025–26
Vantage window (annualised volatility 24%) and failed on 2022–25 (13%) with
the same drift. A break's travel scales with volatility; the spread does not.
So the claim is conditional: **the London-range and opening-range breaks
have positive expectancy when ATR14/close on five-minute bars is at or above
0.075%, and none below it.** The threshold is the 25th percentile of the
high-volatility window and the 75th percentile of the low one (measured
before this file was written: Vantage median 0.100%, Dukascopy 2022–25
median 0.059%), so both windows contain both regimes — 75% and 30% of bars
respectively above the line.

## Falsifier

The high-volatility rows must pass the gate and sit ≥ 95th percentile of a
null gated to the same hours *and the same volatility condition*, on the
primary window **and** the confirmation window; the direction nulls (gated
the same way) must be outside. The low-volatility complement rows are the
contrast: if they pass too, volatility was not the condition. If the
high-volatility rows fail on the primary window, the three earlier results
were the 2025–26 regime and nothing more; closed.

## Base method

`orb` (exists). London preset (`rangeStart 0200, rangeMinutes 360,
entryUntil 1200, maxRangeAtr 100`) and the New York preset (`rangeStart 0820,
rangeMinutes 60, entryUntil 1200`). Grid `riskReward` × `entryBufferPips`.

## Data

- Primary: `xauduka:5m` 2022-06-16 → 2025-04-10
- Confirmation: `xauusd:5m` 2025-04-11 → 2026-09-11 — opened after the
  primary result is recorded
- Nulls gated to hours + `volabs:14:0.075-9` (or `0-0.075` for the contrast)

## Sample needed

30% of the primary window's bars qualify: ~200 London breaks and ~100 New
York breaks. ≥ 60 walk-forward trades per high-vol row.

## What each outcome means

- High-vol rows survive both windows, low-vol rows do not → a conditional
  edge: record, paper-run proposal *with the condition as a hard gate*.
- High-vol rows survive only the confirmation window → the regime finding
  again; closed.
- High-vol rows fail the primary → closed. The three earlier survivals are
  explained, and no further breakout variant is run without a new reason.
