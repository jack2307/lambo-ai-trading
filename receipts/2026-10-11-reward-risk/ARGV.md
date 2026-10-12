# The exact argv of every run behind 2026-10-11-reward-risk-result.md

A receipt prints its resolved window (`bars:` / `bounds:`) but not the argv that
asked for it, and 25 of the record's 34 unreproducible rows come from an argv
recorded with an empty `--from`/`--to` (addendum 11 §VII). The argv therefore
lives here, in a file beside the receipt rather than inside it — which is the
owner's decision of 2026-10-11 §0.1, and the reason `--reward-risk=` prints a
header line only when it is passed.

**Both window ends are pinned explicitly on every single run.**

Binary: `target-rr/release/search.exe`, built in this worktree from
`agent/reward-risk` with `cargo +stable-x86_64-pc-windows-gnu --release
-p fd-backtest`.

## Grid — 40 runs, file name `<cfg>-<win>-rr<ratio>-<guards>.txt`

    target-rr/release/search.exe \
      --market=xauduka --mode=hypotheses --data=/e/rust/flowdesk/data \
      --config=<config | config-swap> --interval=15m \
      --batch-file=docs/hypotheses/2026-10-11-reward-risk.toml --exit-mix \
      --seeds=200 \
      --reward-risk=<0 | 0.9 | 1.8 | 3.6 | 7.2> \
      [--guards] \
      --from=<2010-06-01 | 2018-06-01> --to=<2018-06-01 | 2026-06-01>

    W1 = --from=2010-06-01 --to=2018-06-01   -> 190,889 of 378,749 bars
    W2 = --from=2018-06-01 --to=2026-06-01   -> 187,860 of 378,749 bars

Bar counts are printed by every run and are identical across all five
`--reward-risk=` values inside a window, so no comparison here is across
different tape.

## Controls

    ctl-explicit-<none|0|1.8|7.2>.txt
      ... --batch-file=docs/hypotheses/2026-10-11-reward-risk-explicit.toml \
          --config=config --exit-mix --seeds=20 [--reward-risk=<v>] \
          --from=2018-06-01 --to=2026-06-01

    ctl-selfmanaged-<none|0|1.8|7.2>.txt
      ... --batch-file=docs/hypotheses/2026-10-11-reward-risk-selfmanaged.toml \
          (otherwise identical)

    probe-run-inert.txt      --batch-file=...-runinert.toml, NO --reward-risk flag
    probe-run-inert-ref.txt  --batch-file=...-reward-risk.toml, NO flag  (the reference)
    probe-flag-hygiene.txt   the three misspellings and the five bad values
    probe-seeds-200.txt / probe-seeds-20.txt   --seeds= moves the control only

## Pre-registration screen — 4 runs, 6 bases

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (= `29c8635`, this
worktree's base commit), `--market=xauduka --mode=hypotheses --interval=15m
--exit-mix --config=config`, both windows, both guard arms, a throwaway batch
file of the six `target: None` bases at their default parameters. Table in
`docs/decisions/2026-10-11-reward-risk.md` §2.

## Byte-identity fixtures — 5 argv x 3 binaries

Same five argv run against (a) the shared `29c8635` binary, (b) an independent
`--release` build of `29c8635` in this worktree's `target-rr/`, (c) the
post-change binary **passed no `--reward-risk` flag**.

    1  --market=xauduka --mode=hypotheses --config=config      --batch-file=<batch> --exit-mix          --from=2018-06-01 --to=2026-06-01
    2  --market=xauduka --mode=hypotheses --config=config      --batch-file=<batch> --exit-mix --guards --from=2018-06-01 --to=2026-06-01
    3  --market=xauduka --mode=rescore    --config=config      --batch-file=<batch> --exit-mix          --from=2018-06-01 --to=2026-06-01
    4  --market=xauduka --mode=costs      --config=config                                               --from=2018-06-01 --to=2026-06-01
    5  --market=xauduka --mode=hypotheses --config=config-swap --batch-file=<batch> --exit-mix          --from=2010-06-01 --to=2018-06-01

    <batch> = docs/hypotheses/2026-10-11-reward-risk.toml
    all five also carry --data=/e/rust/flowdesk/data --interval=15m
    none of the five passes --seeds=, so all five run the shipped 200
