# 2026-09-13-ict-sweep-mss-fvg: the ICT sweep → MSS → FVG chain has an edge on gold minutes at Vantage costs

**Registered:** 2026-09-13 — reconstructed after the run from the built-in batches; the out-of-sample market was named before that run (`ict-oos`)
**Status:** decided → docs/decisions/2026-09-13-ict-sweep-mss-fvg.md
**Batch file:** `docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.toml`

## Claim

A higher-timeframe fair value gap, a liquidity sweep that closes back inside,
a displacement close through structure and a retrace into the impulse's gap
mark where smart money has taken liquidity and reversed; entering there with
the stop beyond the sweep captures the move at 2–3R.

## Falsifier

Fails the gate or sits inside the matched null in-sample; or survives
in-sample and fails on four years of Dukascopy minutes; or the direction null
puts the side of the trade inside a coin flip.

## Base method

`ict-sweep-mss-fvg` (ported from the expert's manual, `fd-strategy::ict`).

## Data

- In-sample: `xauusd:1m` (three months, the terminal's cap)
- Out-of-sample: `xauduka:1m` (four years) — opened after the in-sample table was written
- Costs: spread $0.28/oz, swap-free

## Sample needed

Sixty trades per preset for the gate to mean anything at nine cells.

## What each outcome means

See the decision record: survived in-sample, failed out-of-sample, closed.
