# LamboAITrading

Options-flow analytics, strategy research and a live paper desk for COMEX gold
and BTC, in Rust, ported from the JavaScript prototype in
`E:\nodejs\gold-options-flow`.

## This connects to a broker

It did not always, and the sentence that used to be here said so. It does now,
so the change is stated rather than quietly dropped:

- The paper desk decides. Every book, rule-driven or model-driven, runs against
  bars and holds positions that exist only in `data/paper/`.
- `py/live/mt5_executor.py` **mirrors one paper book into one MetaTrader
  account**, and it is the only file in this repository that can place an
  order. It refuses any account whose `trade_mode` is not DEMO, refuses a
  login other than the one named on its command line, refuses an order whose
  notional exceeds ten times account equity, refuses one that would take the
  account's margin level under 500%, and refuses to join a trade the book
  opened more than a bar ago or more than a quarter of its stop distance away.
- No strategy logic lives on the live side. A mirrored book can never disagree
  with the paper book it came from about what the rule said - only about what
  the fill was, and measuring that difference is the whole point of running it.

`config/local.toml` holds the keys and is not in the repository. Neither is
`config/guards.toml`, which is a machine's runtime override of the risk rules.

See `deploy/README.md` for running this on a server.

## Why this exists

The prototype works and has 94 tests, but it hit three ceilings: it cannot make
sub-second decisions on tick data, its parameter sweeps are single-threaded, and
it stores everything as JSON. The bottleneck that actually matters is the second
one — searching the hypothesis space is how an edge gets found, and there is no
edge yet.

**A rewrite does not create edge.** The backtest gate fails today and will fail
after the port. What the port buys is the ability to search faster, handle tick
data, and keep latency predictable.

## The parity discipline

This is the rule the whole migration rests on:

> Rust must reproduce the JavaScript oracle's numbers on the same tape before
> the corresponding JavaScript is deleted.

`tests/golden/` holds 16 files exported by `research/export-golden.js` in the
prototype: level snapshots, every profile mode, every swappable model, indicator
series, the options timeline, and every strategy's backtest trade by trade. The
exporter is deterministic — re-running it produces byte-identical files — so a
parity failure always means the port changed something, never that the oracle
drifted.

Each crate carries its own parity test against those files. A phase is not done
until its gate passes.

## Layout

```
crates/fd-core/         domain types, flow classification, market conventions, config
crates/fd-indicators/   causal indicator library (a test proves causality for all of them)
config/default.toml     every threshold, with measured values marked apart from placeholders
tests/golden/           the oracle
```

Planned, not yet written: `fd-engine` (levels, profile, max pain, clusters),
`fd-strategy`, `fd-backtest`, `fd-store`, `fd-ingest`, `fd-features`, `fd-live`,
`fd-api`, plus `py/` for training and `ui/` for the React front end.

## Build

```bash
cargo check --workspace --all-targets
cargo test --workspace          # includes the parity gates
cargo clippy --workspace -- -D warnings
```

### Toolchain note

The MSVC toolchain is the target, but installing the Visual Studio C++ build
tools needs administrator rights that were not available when this workspace was
set up, so development currently runs on `stable-x86_64-pc-windows-gnu`, whose
linker rustup bundles. That is fine for phases 0–2, which are pure computation.
It must be revisited before the ONNX runtime lands in phase 5, where the
prebuilt binaries are MSVC.

To switch back once the build tools are installed:

```bash
rustup override set stable-x86_64-pc-windows-msvc
```

## Rules carried over from the prototype

1. No unverified formula is presented as correct. Break-even and whale levels are
   swappable models behind a registry, and the UI marks them.
2. Aggressor side is never called "buy to open" — the tapes carry no open/close
   flag.
3. Max pain here is a positioning proxy derived from the tape. Neither feed
   publishes open interest, so it is not OI max pain and must not be labelled as
   such.
4. Indicators are causal; a test enforces it for every one of them.
5. A failing backtest gate suppresses signals rather than being re-tuned.
