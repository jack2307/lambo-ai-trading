# Served responses, for checking a contract against what the API actually emits

These are real responses from a real `fd-api`, saved so a consumer can diff its
expectations against the server's output without anything running.

## Why they exist

On 2026-09-18 a contract for `/api/chart/bars` was agreed between two sessions,
verified by serving the response, and was still wrong. `exported_at_ms` sits
inside `source`; the contract put it at top level. The client read
`data.exported_at_ms`, got `undefined` forever, and the chart's export age
never rendered — silently, because the caption is conditional. Not a wrong
number, a missing one.

The served JSON had been pasted into the very message that got it wrong. The
failure was not skipping verification; it was **verifying the code against
itself and calling it verifying the contract**. Serving a response answers
"does this work". It is silent on "is this what I promised", because the eye
scans output for RECOGNITION rather than for DIFFERENCE, and the disproof can
sit in the same paragraph as the claim and read as support.

Field NAMES were checked and were right. Field NESTING was never stated out
loud, so each side filled it in from the shape it already had in mind and both
agreed enthusiastically about different structures. That was the third such
miss in one day; the anchor and a `camelCase` attribute were the other two. The
thing that survives a careful read is the thing neither party thought to state.

So: the writer saves what the server actually emits, and the consumer diffs key
paths mechanically. The point is that **neither side has to be alert for it to
work** — a check that requires vigilance has the same failure mode as the thing
it is checking.

## What is here

| file | what it is |
|---|---|
| `chart-bars-5m.json` | a stored timeframe served straight from its file, `forming: null` because nothing finer is stored |
| `chart-bars-4h-forming.json` | the file-backed case with a populated forming bar, aggregated from 5m |
| `chart-bars-4h-no-finer.json` | the same timeframe with no finer series at all: `forming: null`, the branch that renders a sentence instead of a candle |
| `chart-bars-1d-refused.json` | the refusal, which a consumer also has to handle: an anchor that cannot be resampled, with the export command in the message |

The `forming: null` cases are here deliberately. That branch draws words rather
than a candle, so nobody looks at it, which is exactly why a sample of it is
worth more than a sample of the happy path.

### Price-bar levels (`/api/paper/levels`, added 2026-09-19)

| file | what it is |
|---|---|
| `paper-levels.json` | a real XAUUSD 15m response: the activity profile with POC/VAH/VAL, 12 unfilled fair value gaps, 76 order blocks, 155 liquidity pools, the session/day/week extremes |
| `paper-levels-unavailable.json` | the same market with no exported bars: every block `null`, one sentence, the export command in it |
| `paper-levels-1h.json` | `?tf=1h`, the non-default path: `timeframe` and `bar_ms` say 1h, every `age_bars` counts 1h bars, `swing_ids` read `1h-…`, and `window` is **ten trading days = 230 bars** where those same ten days are 879 bars of 15m |
| `paper-levels-4h-refused.json` | `?tf=4h` with 15m and 1h on disk: every block `null` and one sentence naming BOTH reasons neither file is rebucketed into 4h — the 21:00Z anchor and the trailing partial bucket. It names the 1h file, because the coarsest series that still fits is the one a resample would have read |

The last two are served by the real route from the **test fixture** rather
than off the live store, and the reason is that the two stores differ.
Checked 2026-09-19: this DEVELOPMENT store holds only 1m, 5m and 15m (21
files, seven symbols of each), so a coarse sample cannot be taken here. The
VPS that trades holds 1m, 5m, 15m, 1h, 4h and 1d for XAUUSD — the hourly
`flowdesk-htf-export` task writes the coarse three there and nowhere else —
so in production `?tf=4h` is ANSWERED rather than refused, and
`paper-levels-4h-refused.json` pins the refusal, not the desk's capability.
Their prices are a seeded walk; what they pin is what the ROUTE emits.
Refresh with `cargo test -p fd-api --test levels -- --ignored`.

**Read `window` as two numbers and not one.** `days` is the span the window
was cut to — ten trading days, measured in the stamps, the same span on every
timeframe — and `bars` is what that span came to on the series being read:
879 on 15m, 230 on 1h, 10 on 1d. A client reading only one of them cannot
tell "ten days of 4h" from "ten bars of 4h". Nothing else needs rescaling by
the consumer either: `atr14` is the ATR(14) of the series asked for, and
every `*_atr` beside it (`displacement_body_atr`, `spread_atr`, and the
profile's `bucket_size_price` at a quarter of it) was measured against that
same number — so a 4h order block is a 4h body over a 4h ATR and never a 15m
one.

**`paper-levels.json` is 120 KB where the other samples here are one or two,
and that is the fact a consumer most needs from it.** The route does not cap
its lists, deliberately rather than by oversight: 879 bars of XAUUSD hold
about 150 confirmed swings, so buy-side liquidity is 73 pools, and choosing
twenty of them to send would be a RANKING. Ranking these levels is what
`docs/hypotheses/2026-09-18-smc-context.md` pre-commits the route not to do —
every mechanical use of them this desk has tested lost out of sample — so the
cap lives in the consumer, which owns its own choice. The prompt block in that
registration states its own maximum count in its own text for this reason.

Two things to read off the sample rather than assume. Every family is
FLATTENED onto its level: `kind`, `price`, `band_low`, `band_high`,
`formed_at_bar_ms`, `age_bars`, `state` and `rule` sit at the top of each gap,
block and pool and **not** under a `level` key — the last contract agreed here
in prose got the names right and the nesting wrong, and a client read
`undefined` forever. And `extremes.session` is the trading-day run IN PROGRESS
while `extremes.day` is the last COMPLETE one, the same convention
`/api/paper/htf` uses for "prior day", so a caption reading "today's high"
over `day` is wrong by a day.

### Orders resting at a price (stage 1 of `docs/plans/2026-09-18-staged-ai-entry.md`)

| file | what it is |
|---|---|
| `paper-order-status.json` | one `/api/paper/status` entry with `pending_order` set (a LONG limit with `invalidate_above` and `valid_bars: 3`); `pending` is `null` beside it, which is the rule and not a coincidence |
| `paper-order-pending-entry.json` | the same order as `/api/paper/pending` lists it: `intent_id` is the order's own `run:decided_bar_time`, and `pending_order` sits beside the market-intent fields (`bars` elided) |
| `paper-order-replies.json` | the bodies and replies of one session: the `intent` that rests it, `pending/act` refused with no tick, `cancel`, the `tick` reply with and without a fill, and a `trigger` that filled. Every reply is `{accepted, reason}` |
| `paper-order-fills.jsonl` | the `fills.jsonl` lines that session wrote, in order: `intent` (with `entry`, `zone`, `valid_bars`, invalidation), `cancelled_unfilled` (`reason: cancelled:<the caller's sentence>`), a second `intent`, the `opened` row of a trigger (`entry.price` is the ask it filled at, `entry.requested_price` the level, `filled_from: act`), and the `trade` row carrying the same `entry` |

Two things a consumer should read off these rather than assume. `entry.price`
on an `opened` row is what the fill was priced FROM, before the half-spread
entry cost; `entry_price` beside it is after that cost, which is why the two
differ by `spread / 2` on every row and not only on orders. And `pending_order`
is a sibling of `pending`, never nested in it: one is a market intent waiting
for the next open, the other an order waiting for a price, and a book carries
at most one of the two.

These were not served over HTTP. They are written by the handlers themselves
from the ignored test `write_order_samples` in `crates/fd-api/tests/paper.rs`
(`cargo test -p fd-api --test paper write_order_samples -- --ignored`), on the
synthetic `btc` tape that file uses, so they carry the same JSON the router
serialises and can be regenerated from any checkout without a store or a
terminal. Prices in them are the test's sine wave and mean nothing.

## Provenance, and how to tell when they are stale

Served 2026-09-18 from branch `chart-window` at 479979b, `XAUUSD.sc` on the
Vantage cent terminal, against a store exported that morning with
`--timeframes H4,M15,M5 --days 20`.

They describe the response SHAPE, not the market. Prices and stamps in them are
whatever gold was doing that morning and mean nothing; a diff should compare
key paths and types, never values.

To regenerate, from a checkout with a store holding the timeframes you want:

```
cargo run -p fd-api --bin fd-api -- --port=8205 --data=<store>
curl -s "http://127.0.0.1:8205/api/chart/bars?market=xauusd&tf=4h&n=10"
```

A forming bar needs a finer series that extends PAST the last closed coarse
bar — export the coarse and the fine together, or the coarse file will end
later than the fine one and `forming` will be null for a reason that has
nothing to do with the code.
