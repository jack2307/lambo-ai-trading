# <id>: <the claim in one line, stated so it can be wrong>

**Registered:** <date> — written before any code or run for this hypothesis
**Status:** registered | implemented | in-sample run | out-of-sample run | decided (→ docs/decisions/<file>)
**Batch file:** `docs/hypotheses/<id>.toml`

## Claim

One paragraph. What structure in the market this exploits and why it would
exist. "Because traders do X" is a reason; "because it backtests well" is not.

## Falsifier

What result kills it. Numbers: the gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30
trades), the null percentile (≥ 95th of the matched null), the direction null
(≥ 95th), and — the one that decides — the out-of-sample market and window
named below, which nobody looks at until the in-sample result is written down.

## Base method

Existing strategy id, or `NEW: <id>` with a spec the implementer can build
from: inputs, the exact entry rule in bar terms, stop, target, and what the
method must NOT read (anything at index > i).

## Data

- In-sample: `<market>:<timeframe>` (e.g. `xauusd:1m`, three months)
- Out-of-sample: `<market>:<timeframe>` (e.g. `xauduka:1m`, four years) —
  **chosen now, opened only after the in-sample result is recorded**
- Costs: the market's config; note anything unusual

## Sample needed

How many trades before the answer means anything, given the grid size. Say it
here, before running.

## What each outcome means

- Survives in-sample AND out-of-sample AND direction null → decision record,
  then a paper run; nothing more.
- Survives in-sample only → the record says so; the claim is closed, not
  narrowed.
- Fails in-sample → closed. Do not widen the grid.
