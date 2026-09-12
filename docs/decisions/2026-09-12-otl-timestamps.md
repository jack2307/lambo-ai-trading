# OTL data was stamped in UTC+7 and labelled UTC — in the oracle and in the port

**Date:** 2026-09-12
**Question:** Are the gold tape and GC bars from live.otldata.com on the clock
the code says they are on?
**Outcome:** No: every OTL timestamp was 7 hours ahead of UTC. The port reproduced
the oracle exactly, so the parity gates never saw it. The owner chose to fix
**both** sides the same day so the parity gate stays meaningful; done and
verified below.
**Status:** CLOSED — fixed in oracle and port, data re-stamped, gate green.

## What was measured

`GC-1m.parquet` (OTL reference closes, 6,719 minutes, 2026-09-07 → 09-12) against
`XAUUSD-1m.parquet` (Vantage, converted to UTC by a rule independently verified
against Binance at r=0.9996). One-minute log-return correlation at shifts from
−8h to +8h in 15-minute steps.

## Evidence

- Correlation peaked at exactly **−7h: 0.9867** (n=6,693). At 0h it was −0.04;
  the next best shift was 0.028. Two gold series that do not correlate at all
  until shifted by seven hours are the same series on different clocks.
- The GC bars had no minutes at stamped hour 04; the option tape had no prints
  there either. 04:00 − 7h = 21:00 UTC = 17:00 ET, the CME daily break.
- Every expiration in the tape was stamped 00:30 — with a `+00:00` suffix.
  00:30 − 7h = 17:30 UTC = 13:30 ET, the COMEX gold option expiry time. The
  feed's zone marker is a label, not a fact.
- Print volume peaked at stamped 19:00–23:00; less 7h that is 12:00–16:00 UTC,
  the New York morning where COMEX volume actually lives.
- UTC+7 is Vietnam; the feed renders times in the account's local zone.

## The fix

| Where | Change |
|---|---|
| oracle `config/default.json` | `reference.utcOffsetHours: 7` (+ a note saying it was measured) |
| oracle `src/ingest/reference.js` | `tradesFromChartData` / `candlesFromChartData` subtract the offset from prints, expiration and candles; `feedUtcOffsetMs()` reads config lazily; tests may pass `utcOffsetMs` explicitly |
| oracle `data/bars/GC-1m.json` | shifted −7h once; `GC-1m.utc-offset-applied` marker refuses a second run |
| port `config/default.toml` | `[sources.reference] utc_offset_hours = 7` |
| port `fd-core` | `SourceConfig::utc_offset_ms()` |
| port `fd-ingest/otl.rs` | `parse_naive` ignores any zone suffix; `trades_from_chart_data` / `bars_from_chart_data` take `utc_offset_ms`; new test pins that prints and expiry move together and DTE does not |
| port `backfill --market=gold` | reads the offset from config |
| goldens | gold files re-exported by the fixed oracle (the oracle publishes its inputs, so its inputs moved); BTC files untouched except the embedded candle series, which had grown |
| port `data/gold` | deleted and rebuilt by `migrate-json --market=gold` (content ids hash the timestamp, so a re-stamped print is a new print — appending would have doubled the store) |

## Verification

- Shift scan re-run on the rebuilt `GC-1m.parquet`: **0h: 0.9867**, −7h: −0.03,
  ±1h: ≈0. The peak moved exactly where it should.
- Every one of the 11,770 stored gold prints now expires at **17:30 UTC**; the
  empty hour is **21 UTC** (17:00 ET).
- Backtest numbers before and after are identical for all nine strategies
  (`gold-backtests.json`, trades and net P&L equal to the cent) — as predicted,
  nothing but the labels moved.
- `scripts/api-parity.py`: **GATE PASS**, 449 bars and nine strategies identical
  between the fixed Node server and the fixed `fd-api`.
- Node: 94 tests pass. Workspace: all crates green, clippy clean.

## What each role said

- **data-integrity:** BLOCK lifted. The labelled clock is the contract with every
  other dataset; it now holds. Wants the shift scan kept as the check.
- **adversary:** SURVIVED — four independent fingerprints said +7 before, and the
  same four say 0 after. Notes that a `+00:00` marker being wrong is the kind of
  thing a parser "trusts" by default; the port's `parse_naive` now trusts nothing.
- **risk:** NO OBJECTION. A stale-tape guard against the wall clock can now work
  on gold; it could not before.
- **historian:** the config comments about "New York open" are now true of the
  data they describe. First time the oracle has been changed since the port
  began; the reason is recorded here so it is not mistaken for drift.

## What would reopen this

The feed or the account changing zone. The check is the shift scan above; it
takes a minute and needs any UTC-verified gold series.

## What this does not say

- It does not say the oracle's gold results were wrong. They were internally
  consistent and remain so — the numbers did not change.
- It does not say the offset is always 7: it is a config number for that reason.
