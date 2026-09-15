# The paper loop

**Status:** running since 2026-09-14 afternoon — steps 1–4 built (`675b5ef` guards, `677e82e` PaperBook + endpoints, `5ad33a5` panel, `b7db765` poller), step 5 in progress: the ten candidates of `CANDIDATES.md` run as ten independent books (`py/live/start_runs.py`) on three streams — `XAUUSD.sc` M15 and M5, `EURUSD.sc` M15 — each fed by a read-only MT5 poller every 10 s (`adcf12a`); the demo executor `py/live/mt5_executor.py` exists with its demo-only lock (tested: the live terminal is refused) and waits for a demo account. Design as written 2026-09-14 morning: The owner chose "A": build the disciplined
paper bot rather than keep searching for an edge that 26 registrations
have not found. This is what it is and what it is not.

## What it is

A process that receives **closed bars** for one market, runs **one
registered strategy** on them with the **same engine code the backtests
use** (fills at the next open, half-spread each side, the sizing stop, the
guards), keeps a **paper book** on disk, and shows the book on the
Overview page. No broker connection exists and none is added: the only
thing that "executes" is a JSON file.

The track record is the point. A bot whose fills are not recorded is an
opinion; this one's fills are a receipt of the same kind the research loop
produces, and the risk role and the news-desk read them the same way.

## The pieces

```
MT5 terminal (live account, read-only)
   │  copy_rates_from_pos every 5 s — py/live/mt5_bars.py
   ▼  POST /api/paper/bar  {market, tf, bar}          the closed bar — decided on
   ▼  POST /api/paper/tick {market, tf, bar, bid, ask} the forming bar — shown only
fd-api ── PaperRun (one per market:tf) ─────────────────────────────┐
   │  • rolling bar window (last N bars, N ≥ warmup)                  │
   │  • indicators recomputed on the window (fd-indicators)           │
   │  • Strategy::on_bar → Intent  (fd-strategy, filters incl. news:) │
   │  • PaperBook::step(bar, intent, guards) (fd-backtest::paper)     │
   │      open_position / check_exit / guard exits / close_position   │
   │      = the engine's own functions, exposed, not copied           │
   │  • persist: data/paper/<run-id>/state.json + fills.jsonl         │
   ▼                                                                   │
GET /api/paper/status  → Overview "Paper desk" panel                  │
   (carries `live`: the forming bar, dropped after 90 s, never in a book) │
POST /api/paper/start|stop  (config: market, tf, strategy, params,   │
                             filters, guards on/off)                  │
```

Binance BTC bars can feed the same endpoint from the existing kline
websocket later; the endpoint does not care who posts.

## Rules the loop enforces in code

- **Bars, not ticks.** A decision is made once per closed bar, exactly as
  in the backtest. A forming bar is never used — and since 2026-09-14 that
  is structural: the forming bar arrives on its own endpoint
  (`/api/paper/tick`), is held in `AppState::live_bars` under a separate
  mutex, is never persisted, and no run, strategy or guard can reach it.
  It exists because a desk whose newest number is fifteen minutes old
  reads as a dead feed; the Desk draws it and labels it as not traded on.
- **Fill model = the engine's.** Signal on bar i, fill at bar i+1's open
  with half the spread; stop/target/guard exits as `check_exit`. There is
  no second fill model to drift from the first.
- **Guards on, always.** `[trading.guards]` from config: daily cap, loss
  limit, cooldown, the open-loss cap, the notional cap, the weekend flat,
  the news flat. A paper run with guards off is not a paper run.
- **The news blackout is the news-desk's.** The events file is loaded at
  start and refreshed weekly (`py/ingest/news_calendar.py --refresh`); the
  status endpoint reports the events loaded and the next blackout, and the
  fills file records every entry refused or position closed by a guard.
- **Idempotent bars.** A bar already seen (same time) is ignored; a bar
  older than the last is rejected; a gap is recorded, not filled.
- **Restart-safe.** State is written after every bar; a restart reloads
  the book and the bar window from disk and asks the poller for the
  warm-up bars again.

## What it is not

- Not a live trader. Nothing here can send an order, and the risk role has
  said the promise is only as good as the absence of order-sending code —
  which `grep` can check and the review does check.
- Not a claim of an edge. The strategy it runs will be one the research
  loop closed as "no edge at this cost"; the loop's expectation is
  approximately zero minus spread, and the paper record is how that
  expectation gets measured against reality (slippage at 08:30, spread at
  the reopen, the guards' bite).
- Not a product yet. The client-facing MT5 bot builder is a separate
  repository; this loop is where its guard and blackout logic gets proven.

## Order of work

1. `fd-backtest::paper` — `PaperBook` over the engine's free functions
   (`open_position`, `check_exit`, guard exits, `close_position`), with the
   equity curve, `GuardState`, and a `step` that takes one bar and one
   intent. Tests: a scripted bar sequence gives the same trades as
   `run_backtest` on the same bars.
2. `fd-api` — `PaperRun` state, the three endpoints, persistence, the
   status JSON. Tests on the endpoint with synthetic bars.
3. `py/live/mt5_bars.py` (written) — the poller, verified against the
   export's clock.
4. Overview panel — the book, the open position, the last fills, the next
   blackout, the guard counters.
5. Run it: `session-hold` or `ema-cross` on `xauusd:15m`, guards on, for
   two weeks; the news-desk publishes the blackout schedule each Monday;
   the risk role reads the fills after.

## The store goes stale, and a new book warms from it

Found 2026-09-15 while starting the AI trader campaign, and it will bite again.

**The poller feeds the API, not the store.** `py/live/mt5_bars.py` posts closed
bars to `/api/paper/bar`, where they enter each running book's window in
memory. Nothing writes them back to `data/bars/`. So a book that is *running*
is current, and the parquet behind it silently falls behind by however long
the pollers have been up.

A book that is *started* warms from that parquet. On the day this was found the
store's `XAUUSD-15m` ended **Friday 11 Sep 20:45** while the live books were at
**Tuesday 15 Sep 14:30** — so a new book opened with an 89.8-hour hole at the
live edge, and the model driving it was shown thirty-nine bars from last week
and one from today. It reasoned about a support level three days and fifty
dollars away from the market.

Two more things compound it, both worth knowing:

* **`fd-api` loads the bar store at startup and holds it.** Refreshing the
  parquet changes nothing until the process is restarted, so the fix is two
  steps and doing only the first looks like the fix failing.
* **`py/ingest/mt5_export.py` lags the poller by about three hours.** Re-running
  it twice on the same afternoon both times ended the file at 11:45 UTC while
  the poller was posting 14:30 bars. The exporter's range end is short; the
  cause is not yet found and is on the backlog.

**The fix that works, in order:**

```
python py/ingest/mt5_export.py --symbols=XAUUSD.sc --timeframes=M15 --days=30
# restart fd-api so it reloads the parquet
# recreate the book, which now warms from fresh data
python py/live/mt5_bars.py --symbol=XAUUSD.sc --market=xauusd --tf=M15 --warm=600 --once --no-tick
```

The last line is what closes the exporter's three-hour lag: the poller reads
the terminal directly and posts every closed bar it has, and the API accepts
the ones newer than each book's last while refusing the rest. After it, the new
book held 600 continuous bars with six gaps, every one of them a weekend, a
daily 17:00–18:00 New York halt, or the Labor Day early close on 07 September.

**Check this before trusting any newly started book**, and especially before
one that shows a chart to a model: a hole at the live edge does not look like
an error, it looks like a quiet market, and the decision on the other side of
it looks like reasoning.
