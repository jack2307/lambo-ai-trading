# 2026-09-13-close-reopen-drift: gold rises across the New York close and through the early evening

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** decided — closed; `docs/decisions/2026-09-13-close-reopen-drift.md`
**Batch file:** `docs/hypotheses/2026-09-13-close-reopen-drift.toml`

## Where this comes from

The recent-year screen (`2026-09-13-recent-year-screen.md`) nominated
nothing under its own criterion, but its long-window context showed one
structure at the 100th percentile of both nulls on 460–560 sessions:
two-hour long holds at 16:00–18:00, 18:00–20:00 and 20:00–22:00 New York
on 2022–25, with the same blocks at the 94th–96th on the last year. The
16–18 row also carried a flaw the reviewers found — its Friday leg is a
50-hour weekend hold — and a risk unit built for a bar, not a timed hold.
This registration tests the claim properly, under the loop's original rule:
long window primary, the Friday leg separated, sizing on daily ranges.

## Claim

Held long from the last bars before the 17:00 New York close through the
reopen and the early evening, gold's return is positive beyond what random
holds of the same length earn, and beyond the same intervals with the side
flipped, on seven years of Dukascopy bars (2018-06 → 2025-04) and again on
the broker's own bars (2025-04 → 2026-09). The account is swap-free
(measured), so the 17:00 rollover costs nothing; the reopen spread was
measured at $0.26–0.27 (`py/ingest/mt5_spread_probe.py`, 4.6 M ticks), so
the flat $0.28 model is honest for it.

## Falsifier

Per row, on the primary with the registered parameters replayed (no grid,
`fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 sessions), ≥ 95th
percentile of random holds of the same length on the same bars, sized the
same; direction null (the same intervals, sides permuted) ≥ 95th. The
confirmation must agree. The rows are the *parts* of the evening, so the
record can say which part carries the drift; a pass on the whole window
alone is a pass.

## Base method

`session-hold` with a sizing stop: `riskDailyRanges` × the mean New York-day
range of the last `rangeDays` days, used by the engine for lots and R and
not enforced (the clock is the exit), as in `tsmom`. `weekdays:MoTuWeTh`
excludes the Friday leg; the `Fr` row carries it alone.

## Rows

- `close/1630-1815` — the break itself, Mon–Thu (entry signalled on the
  16:15 bar, filled at the 16:30 open; exit signalled on the 18:15 bar,
  filled at the 18:30 open). Amended once: the first spelling gated 16:25,
  a minute no 15-minute bar carries, and took no trades.
- `close/1630-2000`, `close/1630-2200` — the break plus the evening.
- `close/1815-2200` — the evening without the break (the contrast).
- `close/1400-1630` — the afternoon before the close (the contrast).
- `close/fri-1630-1815` — the Friday leg only: a weekend hold.

## Data

- Primary: `xauduka:15m` bounded `2018-06-16 → 2025-04-10`.
- Confirmation: `xauusd:15m` from `2025-04-11`.

## Sample needed

Four sessions a week for seven years is ~1,400 holds per row; the floor is
not in question. The question is the null.

## What each outcome means

- The break rows pass both windows and the contrasts do not → the drift is
  the close-to-reopen move; a paper run of that hold, sized as registered,
  and the first record in this loop with two windows behind it.
- The evening rows pass and the break row does not → the drift is the Asian
  open, not the settlement; record it as such.
- Nothing passes the primary → the long-window context of the screen was
  the 2022–25 rally seen through 560 sessions; closed.

## In-sample (2018-06-16 → 2025-04-10, `xauduka` 15m, 160,368 bars)

Fixed replay (`in-sample-fixed.txt`, the test for a strategy-managed hold;
300 sized random holds per row) and the direction null (`direction-close-*.txt`,
1,000 permutations of the same trades' sides):

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
close/1630-1815         1355   1.254   0.006    0.599    0.706  100%  100th       fail: expectancy 0.006R < 0.05R
close/1630-2000         1355   1.237   0.011    0.760    0.859  100%  100th       fail: expectancy 0.011R < 0.05R
close/1630-2200         1355   1.258   0.019    0.837    0.936  100%  100th       fail: expectancy 0.019R < 0.05R
close/1815-2200         1397   0.985  -0.001    0.830    0.936   99%   99th       fail: profit factor 0.985 < 1.2
close/1400-1630         1375   0.739  -0.017    0.784    0.913   21%   25th       fail: profit factor 0.739 < 1.2
close/fri-1630-1815      336   2.064   0.054    0.785    1.177  100%  100th       SURVIVES
```

Walk-forward (`in-sample.txt`, the check): the same order — 1.288 / 1.234 /
1.292 / 1.061 / 0.826 / 2.300, the Friday row at 0.061R on 270 sessions.

Reading before the confirmation: the break rows sit at the 100th percentile
of both nulls on 1,355 sessions and fail the registered expectancy gate,
because R here is one full daily range and a two-hour hold moves a tenth of
it; that is the registration's own unit, and the gate stands as written. The
evening without the break (`1815-2200`) is flat; the afternoon before it is
negative. The Friday leg — the weekend hold the screen's 16–18 row had hidden
— is the one row that passes everything.

## Confirmation (2025-04-11 → 2026-09-11, `xauusd` 15m, 33,589 bars)

Fixed replay (`out-of-sample-fixed.txt`) and direction null (`direction-close-*-oos.txt`):

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
close/1630-1815          282   1.299   0.012    0.892    1.262   96%   96th       fail: expectancy 0.012R < 0.05R
close/1630-2000          282   1.637   0.039    0.934    1.250  100%  100th       fail: expectancy 0.039R < 0.05R
close/1630-2200          282   1.355   0.038    0.955    1.270   97%   98th       fail: expectancy 0.038R < 0.05R
close/1815-2200          289   1.195   0.022    0.986    1.307   88%   91st       fail: profit factor 1.195 < 1.2
close/1400-1630          289   0.955  -0.003    0.968    1.260   47%   52nd       fail: profit factor 0.955 < 1.2
close/fri-1630-1815       68   1.720   0.061    0.959    1.761   95%   91st       gate pass, inside the noise
```

Walk-forward (`out-of-sample.txt`): 1.400 (97%) / 1.800 (100%) / 1.420 (98%) /
1.254 (89%) / 0.962 (48%) / the Friday row 1.893 on 55 sessions, 0.069R, 94%.
"Nothing survived" in both files.

The Friday leg passed the primary and does not clear either null on the
confirmation's 68 Fridays (95th of random holds on the fixed replay, 94th on
the walk-forward, 91st on direction); the falsifier said the confirmation must
agree, and it does not. The break rows keep their sign and rank on the second
window and fail the same gate. Closed under this registration.

## Amendment, 2026-09-19: the confirmation's 68 is 68 trades and 67 Fridays

**What this file says**, under "Confirmation": the receipt row
`close/fri-1630-1815` carries **68** trades, and the paragraph beneath it
reads *"does not clear either null on the confirmation's 68 Fridays"*. The
decision of the same name carries 68 in its title and in its outcome line.

**What the other document says.**
`docs/hypotheses/2026-09-13-friday-weekend-hold.md` says *"the **67** Vantage
Fridays since 2025-04"*, and `docs/research/BACKLOG.md` says 67.

**Which is right: both numbers, counting different things.** The resolution is
already in `docs/decisions/2026-09-13-close-reopen-drift.md`, in a
data-integrity caveat that has never been carried back here: *"the
confirmation's 68 is **67 weekend holds plus an end-of-data stub**."* So the
receipt's trade count is 68 and the number of Friday-into-Sunday holds is 67;
the 68th row is the sample running out, not a weekend.

The same decision uses 67 whenever it is talking about weekend holds rather
than about the receipt — *"its confirmation miss on 67 weekend holds is a
power problem"*, and *"It says 67 weekend holds could not confirm 336"* — and
68 when quoting the row. Both are correct in their place; what is wrong is the
word **Fridays** attached to 68, here and in the decision's title.

**What does not change.** Nothing. The verdict rests on the null percentiles,
not the count: 95th of random holds on the fixed replay, 94th on the
walk-forward, 91st on direction, against a falsifier that required the
confirmation to agree. The decision's own reopening condition says the extra
row would not have mattered either way — *"68 could not decide; 100 more will
not either"*.

The line above is left as written. A record that quietly changes a count is a
record whose counts cannot be checked.

*Written 2026-09-19 by the records session, from
`docs/decisions/2026-09-13-close-reopen-drift.md`,
`docs/hypotheses/2026-09-13-friday-weekend-hold.md` and
`docs/research/BACKLOG.md`. Nothing is recomputed. Raised as item 4 of
`docs/research/VERDICTS.md` section 4.*
