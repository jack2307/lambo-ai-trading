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
- **`fd-api` serves two ports and the difference between them is the only
  security boundary it has.** 8138 is loopback with no authentication, and the
  desk's own processes depend on that; 8139 requires a session on every path
  and is the only one a tunnel may point at. Never "authenticate unless the
  peer is loopback": `cloudflared` runs on the same machine, so every request
  off the internet wears a loopback address and such a rule authenticates
  nobody while reading as safe. See `crates/fd-api/src/auth.rs`.

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
- Correcting a number in `config/default.toml` **restates every stored trade
  computed from it**, silently. `eur-hours` sized 256.3 lots at contract 1.0
  and booked the exit at 1000.0 two hours after the correction landed,
  reporting $176.85 on a $100 book for a move worth $0.18. A trade now
  carries its own `contract_size` and `spread` (`Live`, `Trade`) and is
  priced from them; one that carries neither is an ESTIMATE and the counts
  say so. Locked by `fd-backtest/tests/contract_size.rs`; see
  `docs/decisions/2026-09-21-restated-pnl-contract-size.md`.
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
- Agent worktrees share `CARGO_TARGET_DIR=E:/rust/flowdesk/target` so three of
  them cannot fill the disk — and that **bakes the worktree's path into test
  binaries**, because several tests reach `config/` through
  `env!("CARGO_MANIFEST_DIR")`. Delete the worktree and those binaries stay in
  the shared target still naming a directory that is gone: the same test then
  PASSES under `cargo test -p <crate>` and FAILS under `--workspace`, with
  `config/default.toml … NotFound` pointing at some `fd-wt-*` path. It is not
  a config fault and not a flaky test. `cargo clean -p <crate>` for whatever
  failed, then re-run. Measured 2026-09-21: two crates, and the clean also
  freed 46 GiB.
- **A launcher that redirects its own output into a fixed file cannot be run
  twice.** `Start-Process` hands each child the parent's handles, so the
  processes a launcher starts hold its log open for as long as they live; the
  next run's redirect then fails before a single line executes, the child
  dies instantly, and `Win32_Process.Create` has *already* returned 0 with a
  pid, so the caller is told it started. Measured 2026-09-21:
  `start_ai_traders.ps1 -Detached` reported success twice and did nothing,
  its log frozen at 2026-09-19 11:05 alongside traders carrying that same
  start time. The tell is a log whose timestamp does not move; the proof is
  `[IO.File]::Open(path,'Open','Write','None')` throwing "used by another
  process". Fixed there with a per-run timestamped name. **Any script that
  spawns long-lived children must not redirect into a path it will reuse** -
  this applies to ad-hoc deploy wrappers too, and one of them hit it the same
  night.
- **`git worktree remove` follows a junction and deletes what it points at.**
  An agent worktree needs `ui/node_modules`, and the cheap way to give it one
  is `mklink /J` to the primary tree's. Remove the worktree without removing
  the junction first and the primary tree's `node_modules` is emptied - 300
  directories down to 2, `npx tsc` suddenly reporting that TypeScript is not
  installed, and nothing else on the machine touched. Measured 2026-09-22.
  **Always `cmd /c rmdir <link>` first** - it removes the reparse point and
  not the target - **and check the target survived** before removing the
  worktree. The repair is `npm ci` and costs a few minutes; the danger is
  that the symptom looks like a broken toolchain rather than a deletion. The
  live desk is unaffected either way: it serves a built `dist`, not
  `node_modules`.

## Toolchain

`stable-x86_64-pc-windows-gnu`. Building anything that uses `windows-sys`
needs the WinLibs binutils on PATH:
`/c/Users/Admin/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin`.
Cargo is at `/c/Users/Admin/.cargo/bin/cargo.exe`. No MSVC is needed for
anything here; the workspace is portable and runs unchanged on Linux.

## Running it

```
fd-api --port=8138                       # API; the UI proxies here. NO AUTH, loopback, the desk's own bus
fd-api --set-password                    # set the password for the authenticated listener (once)
fd-api --auth                            # ALSO serve 8139, where every request needs a session — the tunnel's port
cd ui && npx vite --port=5180            # UI at http://localhost:5180 (IPv6 bind)
fd-ingest --bin collect --market=btc     # accumulate the options tape (Deribit websocket)
fd-ingest --bin collect --market=gold    # same for OTL: polls every 10 min, also grows GC-1m bars
python py/ingest/mt5_export.py           # Vantage XAUUSD/BTCUSD bars -> data/bars (read-only)
fd-backtest --bin search --market=btc    # compare / sweep / wf / costs / null
fd-backtest --bin search --market=xauusd # same, on the broker's own gold bars
fd-backtest --bin search --market=xauusd --mode=hypotheses  # declared batch vs matched nulls (--batch=gold-intraday|ict-m1|ict-m5|ict-oos)
python scripts/research-run.py docs/hypotheses/<id>.toml --stage in|oos   # the /research pipeline's runner; receipts in docs/research/runs/
python py/ingest/dukascopy_to_parquet.py <csv> data/bars/XAUDUKA-1m.parquet  # four years of gold minutes, out-of-sample structure
scripts/api-parity.py                    # prove the browser cannot tell backends apart
```
