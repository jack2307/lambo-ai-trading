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
- **A new MT5 terminal cannot be started over SSH on the VPS.** `mt5.initialize`
  on a terminal that is not already running HANGS - no error, no timeout, no
  log line - because MT5 needs the interactive session's window station and an
  SSH command does not have one. `query session` on 103.19.29.194 shows
  `Administrator  2  Disc`: a disconnected-but-alive interactive session, and
  every working terminal (`MT5-cent`, `MT5-demo`, `MT5-v12`) lives inside it,
  started by hand in RDP days ago. So a new account needs a person in RDP once:
  launch the terminal, log in, enable AutoTrading, put the symbol in Market
  Watch, then **disconnect rather than log off** - logging off destroys session
  2 and every terminal in it.
  Measured 2026-09-25, on a first attempt to bring up `C:\MT5-v10`: the copy
  succeeded, the login script read and deleted its password file, and then
  `initialize` sat for ten minutes with no terminal process ever appearing.
- **A SCHEDULED TASK CANNOT SUBSTITUTE FOR THE DOUBLE-CLICK, and the line above
  used to imply it could.** Registering the terminal as a task with
  `schtasks /IT` (or an Interactive `LogonType`) DOES start it, in the logged-on
  session, un-elevated, holding the Python bridge port - and Python still cannot
  reach it: `(-10005, 'IPC timeout')` on every `initialize`, and
  `(-10001, 'IPC send failed')` in the pollers. Measured 2026-09-25 by
  falsifying two hypotheses in turn: elevation (dropped `/rl HIGHEST`, no
  change) and a port conflict (the terminal's own log shows
  `MCP bind error on 127.0.0.1:22346`, real - 22346 is the single port the
  MetaTrader5 package speaks over, so only ONE terminal per machine can serve
  Python - but giving v12 the port alone did not make it reachable).
  What distinguishes a reachable terminal from an unreachable one is NOT who
  started it but WHAT THE SESSION WAS DOING at the time, and the terminal's own
  log shows it: `C:\MT5-v12\logs\<date>.log` records
  `MetaTrader 5 x64 build ... started` for the instance launched while the owner
  was CONNECTED, and **nothing at all** - not one line - for every instance
  launched while `query session` showed `Administrator 2 Disc`. Those hang before
  they can even write a log line.
  So the rule is: **the terminal must be started while somebody is actually
  CONNECTED to the RDP session, not merely logged on.** A disconnected session
  keeps windows that already exist - which is why a terminal started while
  connected keeps working for days after the owner disconnects - but gives a NEW
  process no desktop to create one on. Sequence that works: connect, double-click,
  WAIT for the quote window and the balance to appear, then disconnect.
  Nothing automates the double-click. Three approaches were tried and all failed:
  `Invoke-CimMethod` from SSH (session 0), `schtasks /IT /rl HIGHEST`, and
  `schtasks /IT` un-elevated.
  THE COST OF LEARNING THIS: killing the hand-started terminal to test the port
  hypothesis took the pollers down with it and froze every book for 96 minutes.
  Do not kill a working terminal to test anything - start a second installation
  instead, or test on the demo one.
- **ONE TERMINAL PER MACHINE SERVES PYTHON.** Because of the fixed 22346 port,
  a second funded account cannot have its own executor on the same VPS while
  another terminal holds the bridge. `flowdesk-bars-export`'s task argument
  names the terminal explicitly and `initialize()` relaunches it, so whichever
  terminal that argument names will take the port back within five minutes -
  point it at the terminal whose account the executors need.
- **`CARGO_TARGET_DIR` is NOT set, and this line used to claim it was.** It
  said agent worktrees share `E:/rust/flowdesk/target` "so three of them
  cannot fill the disk". They do not share it — nothing sets the variable — so
  every worktree builds its own target, 12–32 GiB each. On 2026-09-24 four
  design agents took a 301 GiB volume down to **640 KiB free**, and the note
  that was supposed to prevent exactly that was recording an intention as a
  fact. Corrected rather than deleted, because both halves are traps:
  - **When it is unset (the default), watch the disk.** `cargo clean` in a
    finished worktree returns 12–14 GiB; the primary tree's own target was
    34.7 GiB. And **cargo failing on a full disk does not say so**: every line
    reads `rustc-LLVM ERROR: ... No space left on device` or
    `failed to write ...rmeta: There is not enough space on the disk`, with no
    compile error and no failing assertion. A build or test that dies with
    LLVM errors and no test output is a disk check, not a code review.
  - **If you do set it, sharing it bakes the worktree's path into test
    binaries**, because several tests reach `config/` through
    `env!("CARGO_MANIFEST_DIR")`. Delete the worktree and those binaries stay
    in the shared target still naming a directory that is gone: the same test
    then PASSES under `cargo test -p <crate>` and FAILS under `--workspace`,
    with `config/default.toml … NotFound` pointing at some `fd-wt-*` path. It
    is not a config fault and not a flaky test. `cargo clean -p <crate>` for
    whatever failed, then re-run. Measured 2026-09-21: two crates, and the
    clean also freed 46 GiB.
  - Either way, a binary in *another* tree's target can answer your command.
    An agent ran `E:/rust/flowdesk/target/release/search.exe` from its own
    worktree and got "unknown strategy" for a strategy that existed only on
    its branch. Run `./target/release/<bin>` from inside your own tree and
    check the receipt names your strategy rather than refusing it.
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
