# The weekend-flat guard cannot fire on this feed, and two real positions are open over the weekend

Found 2026-09-19 04:06Z, while the market was shut. The fix below is
**PROPOSED, not applied**: it changes a guard every live book runs under and
the funded account mirrors, so it is the owner's.

## What happened

At the Friday close the desk was holding **fourteen paper books** open, and
the funded cent account **33705331** was holding two real positions:

| book | side | lots | entry | stop | unrealised at 04:06Z |
|---|---|---|---|---|---|
| `ai-xau-opus-ctx-b` | LONG | 0.06 | 4387.39 | 4373.80 | −54.48 USC |
| `xau-ema` | LONG | 0.06 | 4387.34 | 4371.37 | −54.18 USC |

Account balance 9901.07 USC, equity 9792.41 USC. Both stops rest with the
broker, so they are not unguarded — but a Sunday gap through them fills at
the gap, not at the level.

`flat_before_weekend_hhmm = 1655` was supposed to prevent exactly this.

## Why it did not fire

The guard is bar-driven. `Guards::past_weekend_cutoff(close_ms)` is true when
the instant a bar CLOSES is a Friday at or after the cut-off in **New York**
local time. The comment in `crates/fd-backtest/src/guards.rs` states the
intent plainly:

> `1655` on a 15-minute feed flattens at the 16:45 bar's close (17:00, the
> last Friday print) instead of never — no Friday bar of that feed *opens* at
> or after 16:55.

**The 16:45 NY bar is not the last Friday print this desk receives.** Measured
on 2026-09-18:

| source | last 15m bar |
|---|---|
| the runs (`last_bar_time` on every book) | **20:30Z** (16:30 NY) |
| the terminal's own export, `data/bars/XAUUSD-15m.parquet` | **20:45Z**, O 4380.86 C 4378.32 |

So the broker DID print 20:45Z; the poller never fed it. `mt5_bars.py` asks
for the last CLOSED bar, and MetaTrader only closes a bar when a tick arrives
after its boundary. At the weekly close no further tick arrives, so the
20:45Z bar stays "forming" in the terminal until Sunday's first tick, and the
newest closed bar the poller can see all weekend is 20:30Z. The export task
reads the current bar too, which is why it has one the desk does not.

The arithmetic then closes the case: the last bar the runs receive closes at
20:45Z = **16:45 NY = minute 1005**, and the cut-off 1655 is **minute 1015**.
1005 < 1015, so no bar the desk ever sees is past the cut-off, and the guard
is unreachable. It has never fired and never could.

This is not specific to this Friday. It repeats every week, on every book
holding a position, and on every account mirroring one.

## The fix, proposed

Set **`flat_before_weekend_hhmm = 1640`**.

- The 20:15Z bar closes at 16:30 NY (minute 990) — below 1640, so it is not
  affected.
- The 20:30Z bar closes at 16:45 NY (minute 1005) — at or past 1640, so THIS
  bar flattens, and it is the last bar the desk actually receives.
- The margin between 1640 and 1645 is there so the rule does not hang on an
  exact equality.
- It is anchored in New York time, so the winter shift (the close moving from
  21:00Z to 22:00Z) needs no second value: the last arriving bar still closes
  at 16:45 NY.

What it costs: the desk stops taking entries and closes positions about
fifteen minutes earlier than the value on the file intended, and half an hour
before the actual close. That is the price of a guard that fires at all.

What it does NOT fix: a position opened and stopped out inside that last
unfed bar is invisible to the books either way, because the bar never
arrives. That is a separate question about the final fifteen minutes of the
week, and it is not worth answering with a rule the feed cannot support.

## Why it is not applied here

Three reasons, in order:

1. It changes a guard every live book runs under, and two of those books are
   mirrored into an account holding money. Guard changes are the owner's, and
   the desk records them as `guards_changed` rows precisely so they are never
   silent.
2. It cannot help the positions already open. The market is shut; nothing can
   be closed until Sunday's reopen.
3. The owner may prefer the opposite trade-off — to hold over weekends
   deliberately and rely on the broker's stops — and that is a legitimate
   choice this note should not pre-empt.

## What has to be decided, before Sunday 21:00Z

- **The two open real positions.** Leave them to their stops, or flatten at
  the reopen. Gold gaps at the Sunday open; the stops are 4-7 points below
  Friday's close, so a gap larger than that fills below the stop.
- **The guard value.** 1640, or leave 1655 and accept weekend holds.

Both are one-line changes once decided. Neither is made here.
