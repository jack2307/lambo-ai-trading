# flowdesk

Options-flow analytics and strategy research for COMEX gold and BTC, ported to
Rust from the JavaScript prototype in `E:\nodejs\gold-options-flow`.

Paper only. Nothing here connects to a broker.

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
