# flowdesk

Rust port of the gold-options-flow prototype. The JavaScript original at
`E:/nodejs/gold-options-flow` is the **oracle**: every phase must reproduce its
numbers before moving on, and `tests/golden/` holds what it published.

## Read before changing anything

- `ui/DESIGN.md` — the design language. Two modes, one accent, numbers in
  monospace, and the rule that a figure never appears without what qualifies
  it. A screen added without reading this will be in the wrong mode.
- `.claude/agents/README.md` — the review team and its order of authority.
  The numbers outrank the agents; three roles veto alone; nothing is a vote.
- `docs/decisions/` — what has already been tried and why it was closed.
  Check before proposing an experiment; the historian role exists to do this.

## Standing constraints

- Paper trading only. Nothing may acquire the ability to send an order.
- An LLM may veto or cut size. It never chooses direction, sets a price, or
  increases size.
- A failing gate is the result. It is never re-tuned to pass.
- Unverified formulas stay as swappable models, marked ⚠.

## Traps this codebase has already paid for

- JavaScript's `a ?? b` does not fall back on NaN. The port did, and drifted a
  bar.
- Read the market's overrides, not the top-level config table. `[trading]`,
  `[levels.cluster]` and `[big_trades]` all describe gold; BTC differs on every
  one. This has been shipped twice. Locked by `fd-backtest/tests/trading_rules.rs`
  and `fd-engine/tests/settings.rs`.
- `Math.round` and `toFixed` are different roundings and the oracle uses both.
  Use `fd_core::js_round_to` / `js_to_fixed`; never `(x * scale).round()`.
- An in-sample score is not a result. Quote the walk-forward, and place it
  inside the null distribution (`search --mode=null`, `--mode=null-dir`).
- A session or regime gate needs a **matched** null (the control wrapped in the
  same `Filtered` gates): noise traded only in the New York morning clears the
  1.2 gate 5% of the time on its own. Hypotheses are declared in
  `fd-backtest/src/hypotheses.rs` with a reason each; that file is not a grid.
- Swap is charged (`Trade::swap_usd`, 17:00 New York, Wednesday x3) from the
  market config. The Vantage account here is **swap-free — measured from its
  own deal history**, so the config says zero; `symbol_info.swap_long` shows
  the generic rate in **points** (`swap_mode=1`), not dollars. Read the mode
  before trusting the number.
- The MT5 account currency is **USC** (cents); `contract_size` 1.0 on
  `XAUUSD.sc` is one ounce in dollar terms. Spread, swap and P&L per lot in
  this workspace are per ounce; multiply by 100 for a standard 100 oz lot.
- The Deribit public endpoint reaches back about a day. History not captured
  as it happens is gone; `fd-ingest --bin collect` is what captures it.
- The OTL feed (gold tape and GC bars) renders times in the account's zone,
  **UTC+7**, including the `+00:00`-suffixed expirations. Both the oracle and
  the port read it that way until 2026-09-12; now `[sources.reference]
  utc_offset_hours = 7` (and `reference.utcOffsetHours` in the oracle) is
  subtracted at parse time. Do not remove it because a zone marker says UTC;
  re-run the shift scan in `docs/decisions/2026-09-12-otl-timestamps.md` if
  the feed or the account changes.
- MT5 returns bar times on the **broker's clock** (Vantage: UTC+3 in NY
  summer, UTC+2 in winter), and any request larger than the terminal's "Max
  bars in chart" fails with `Invalid params` instead of truncating. The
  exporter handles both; do not call the terminal directly.
- The MT5 terminal on this machine is a **live** account. Read-only calls
  only (`copy_rates_*`, `symbol_info`); no order function is ever imported.

## Toolchain

`stable-x86_64-pc-windows-gnu`. Building anything that uses `windows-sys`
needs the WinLibs binutils on PATH:
`/c/Users/Admin/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin`.
Cargo is at `/c/Users/Admin/.cargo/bin/cargo.exe`. No MSVC is needed for
anything here; the workspace is portable and runs unchanged on Linux.

## Running it

```
fd-api --port=8138                       # API; the UI proxies here
cd ui && npx vite --port=5180            # UI at http://localhost:5180 (IPv6 bind)
fd-ingest --bin collect --market=btc     # accumulate the options tape (Deribit websocket)
fd-ingest --bin collect --market=gold    # same for OTL: polls every 10 min, also grows GC-1m bars
python py/ingest/mt5_export.py           # Vantage XAUUSD/BTCUSD bars -> data/bars (read-only)
fd-backtest --bin search --market=btc    # compare / sweep / wf / costs / null
fd-backtest --bin search --market=xauusd # same, on the broker's own gold bars
fd-backtest --bin search --market=xauusd --mode=hypotheses  # declared batch vs matched nulls (--batch=gold-intraday|ict-m1|ict-m5|ict-oos)
python py/ingest/dukascopy_to_parquet.py <csv> data/bars/XAUDUKA-1m.parquet  # four years of gold minutes, out-of-sample structure
scripts/api-parity.py                    # prove the browser cannot tell backends apart
```
