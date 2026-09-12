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
- The Deribit public endpoint reaches back about a day. History not captured
  as it happens is gone; `fd-ingest --bin collect` is what captures it.

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
fd-ingest --bin collect --market=btc     # accumulate the options tape
fd-backtest --bin search --market=btc    # compare / sweep / wf / costs / null
scripts/api-parity.py                    # prove the browser cannot tell backends apart
```
