# The exact argv of every run behind 2026-10-11-hold-cap-result.md

A receipt prints its resolved window (`bars:` / `bounds:`) but not the argv that
asked for it, and 25 of the record's 34 unreproducible rows come from an argv
recorded with an empty `--from`/`--to`. So the argv is written down here.
Both window ends are pinned explicitly on every single run.

Binary: `target-hc/release/search.exe`, built in this worktree from
`agent/hold-cap` at `--release -p fd-backtest`.

## Grid — 32 runs, file name `<cfg>-<win>-<max_hold_ms>-<guards>.txt`

    target-hc/release/search.exe \
      --market=xauduka --mode=hypotheses --data=/e/rust/flowdesk/data \
      --config=<config | config-swap> --interval=15m \
      --batch-file=docs/hypotheses/2026-10-11-hold-cap.toml --exit-mix \
      --max-hold=<14400000 | 43200000 | 86400000 | 259200000> \
      [--guards] \
      --from=<2010-06-01 | 2018-06-01> --to=<2018-06-01 | 2026-06-01>

    W1 = --from=2010-06-01 --to=2018-06-01   -> 190,889 of 378,749 bars
                                                2010-06-01 00:00 .. 2018-05-31 23:45
    W2 = --from=2018-06-01 --to=2026-06-01   -> 187,860 of 378,749 bars
                                                2018-06-01 00:00 .. 2026-05-31 23:45

Bar counts are identical across all four `--max-hold=` values inside a window,
so no comparison here is across different tape.

## Self-managed control — 3 runs, `sm-<max_hold_ms>.txt`

    target-hc/release/search.exe \
      --market=xauduka --mode=hypotheses --data=/e/rust/flowdesk/data \
      --config=config --interval=15m \
      --batch-file=docs/hypotheses/2026-10-11-hold-cap-selfmanaged.toml \
      --exit-mix --max-hold=<14400000 | 259200000 | 0> \
      --from=2018-06-01 --to=2026-06-01

## Pre-registration screen — 1 run, 10 `Exits::Engine` bases

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (= `29c8635`, this
worktree's base commit), `--market=xauduka --mode=hypotheses --interval=15m
--exit-mix --from=2018-06-01 --to=2026-06-01`, guards off, a throwaway batch
file of ten bases at their default parameters. Table in
`docs/decisions/2026-10-11-hold-cap.md` §2.

## Byte-identity fixtures — 5 runs x 3 binaries

Same five argv run against (a) the shared `29c8635` binary, (b) an independent
`--release` build of `29c8635` in `target-hc/`, (c) the post-change binary with
**no `--max-hold` flag**. All three agree byte for byte on all five.

    1  --market=xauduka --mode=hypotheses --config=config  --batch-file=<batch> --exit-mix          --from=2018-06-01 --to=2026-06-01
    2  --market=xauduka --mode=hypotheses --config=config  --batch-file=<batch> --exit-mix --guards --from=2018-06-01 --to=2026-06-01
    3  --market=xauduka --mode=rescore    --config=config  --batch-file=<batch> --exit-mix          --from=2018-06-01 --to=2026-06-01
    4  --market=xauduka --mode=costs      --config=config                                          --from=2018-06-01 --to=2026-06-01
    5  --market=xauusd  --mode=hypotheses --config=config-swap --batch-file=<batch> --exit-mix      --from=2024-06-01 --to=2026-06-01

    <batch> = docs/hypotheses/2026-10-11-hold-cap.toml
    all five also carry --data=/e/rust/flowdesk/data --interval=15m
