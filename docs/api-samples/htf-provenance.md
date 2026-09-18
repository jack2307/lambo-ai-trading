# `/api/paper/htf` — served responses

Two real responses from a real `fd-api`, saved so a consumer can diff its
expectations against what the server emits without anything running.

**This file folds into `README.md` once `chart-window` reaches main.** That
branch creates `docs/api-samples/README.md`; this branch creates the directory
independently, and two branches both writing that README is a merge conflict
for whoever merges second, in a file whose entire purpose is to be read rather
than resolved. So the provenance sits here for a day and moves.

## What is here

| file | what it is |
|---|---|
| `htf-three-timeframes.json` | all three blocks present, `unavailable: null`, `unavailable_by_tf: {}` |
| `htf-h1-missing.json` | the MIXED state: `h4` and `d1` answer, `h1` is null with its own reason |

The mixed one is the point. It is the state at launch — H4 exported, H1 not yet
— and it is the state that broke the old shape: `unavailable` is a single
sentence joining every reason, so with three timeframes it cannot say which row
it is about, and a card that printed it under a good H4 row would be
confidently wrong about the row above it. `unavailable_by_tf` keys each reason
by the timeframe it belongs to. In that sample the map has exactly one key,
`"1h"`, and `"4h"` is absent because H4 is fine.

Note in `htf-three-timeframes.json` that `h1.structure.break_level` and
`break_side` are `null` while the four swing points are populated: that is a
`RANGE` label, which has no single price whose break changes it. Null there is
the rule declining to invent a level, not a warmup.

## Provenance, and how to tell when they are stale

Served 2026-09-18 from branch `htf-h1`, against a **BTCUSDT** store built by
bucketing the repo's real `BTCUSDT-15m.parquet` (2024-09-12 to 2026-09-12) into
1h, 4h and 1d.

**BTC on purpose, and it is not the market this route is for.** Gold's H4 and
D1 candles start at 21:00 UTC and cannot be built by bucketing a finer series —
that is the whole reason `stored_only` refuses to resample. BTC trades around
the clock, so an epoch-anchored bucket really is the bucket, which makes it the
one market where a store built this way holds honest bars. These samples
therefore describe the response SHAPE and say nothing about any market.

One consequence is visible and worth naming so nobody reads it as a bug: every
`d1.prior_week_*` field is `null`. `weeks_of` splits on the weekend hole, and
BTC has no weekend, so the whole series is one week and there is no prior week
to report. Against gold those fields are populated.

A diff should compare key paths and types. Never values: prices and stamps here
are whatever BTC was doing, and a check that goes red when the market moves is
a check somebody turns off.

To regenerate:

```
cargo run -p fd-api --bin fd-api -- --port=8207 --data=<store>
curl -s "http://127.0.0.1:8207/api/paper/htf?market=btc"
```
