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
