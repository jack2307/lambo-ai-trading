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

What it costs, stated exactly: the position closes at the 20:30Z bar's
close, **20:45Z - fifteen minutes before the market shuts at 21:00Z**, not
hours before it. Entries stop about thirty minutes early, because a signal on
the 20:30Z bar would fill on the 20:45Z bar, which is the one that never
arrives. That is the whole price of a guard that fires at all.

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

## Amendment 2026-09-19 04:30Z - why be flat at all, measured

The owner asked the right question: why close before the week ends? The
answer is not that weekend holds lose money. It is that **a stop does not
work across a gap**, and on this instrument the gap is now bigger than the
stop.

**What the desk already knew.** `2026-09-13-friday-weekend-hold` tested the
hold as a strategy on 15m gold 2010-2018 and closed it: 400 holds, profit
factor 0.983, net −$73 over 401 weekends, negative in every year 2014-2018.
So the hold is roughly a coin flip with costs - an argument for indifference,
not for fear.

**What was missing, and is measured here.** Fear belongs to the SHAPE of the
risk, not its mean. Gap sizes from the desk's own `XAUDUKA-15m.parquet`,
864 weekends, 2010-06-01 to 2026-05-31, against the 12-point stop the live
books size to (1.2 x ATR(14) was 11.93 on 2026-09-18):

| | whole file, 864 weekends | last 12 months, 42 weekends |
|---|---|---|
| median gap | 0.97 pts (0.05%) | **13.55 pts** |
| 90th | 6.32 pts | 79.49 pts |
| 95th | 12.27 pts | 100.52 pts |
| 99th | 33.84 pts | - |
| worst | 117.58 pts (6.65% of price) | 117.58 pts = **9.8 stops** |

Over the whole file only 5.2% of weekends gapped more than one stop. **In the
last twelve months the MEDIAN weekend gap is larger than the stop.** Gold
quadrupled in price over the window, but the median in percent also grew,
from 0.05% to about 0.29%, so this is a real change in behaviour and not a
scale artefact.

**What that does to a position.** A stop is a promise that the loss stops at
1R. A gap through it is filled at the gap, so the promise is void exactly
when it matters. On the two positions open right now - 0.06 lots each, about
6 USC per point each, 12 USC per point together, entries near 4387:

| Sunday opens | loss on the pair | share of the 9,901 USC account |
|---|---|---|
| at Friday's close | −109 USC | 1.1% |
| a median recent gap down, 13.6 pts | about −270 USC | 2.7% |
| the 90th, 79.5 pts | about −1,060 USC | 10.7% |
| the worst on file, 117.6 pts | about −1,525 USC | 15.4% |

A gap up pays the same way, and the registered study says the two sides very
nearly cancel. **The trade is therefore a coin flip on 10-15% of the account,
taken every Friday, for no expectancy** - which is the one bet a desk with a
1% risk rule has already decided it does not take.

**So the guard is worth having, and fifteen minutes is what it costs.** That
is the recommendation: `1640`. If the owner would rather hold, the honest
version of that choice is to cut Friday's size, not to keep full size and
call the stop protection.
