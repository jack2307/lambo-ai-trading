# The paper loop

**Status:** running since 2026-09-14 afternoon — steps 1–4 built (`675b5ef` guards, `677e82e` PaperBook + endpoints, `5ad33a5` panel, `b7db765` poller), step 5 in progress: `ema-cross` on `xauusd:15m`, guards on, `weekdays` + `news:60-30` (USD), fed by the read-only MT5 poller every 10 s. Design as written 2026-09-14 morning: The owner chose "A": build the disciplined
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
   ▼  POST /api/paper/bar {market, tf, bar}
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
POST /api/paper/start|stop  (config: market, tf, strategy, params,   │
                             filters, guards on/off)                  │
```

Binance BTC bars can feed the same endpoint from the existing kline
websocket later; the endpoint does not care who posts.

## Rules the loop enforces in code

- **Bars, not ticks.** A decision is made once per closed bar, exactly as
  in the backtest. A forming bar is never used.
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
