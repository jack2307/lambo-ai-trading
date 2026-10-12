# The exact argv of every run behind 2026-10-11-argv-sidecar-result.md

Written by hand, in the shape `agent/hold-cap` established at
`receipts/2026-10-11-hold-cap/ARGV.md` — and this is the last job that should
have to. Every run below also wrote a **machine-generated** sidecar through the
flag this job adds, so from here a file of this kind is produced by the binary
rather than typed out afterwards. The three examples kept in `sidecars/` are
what it produces.

Binary, unless a run says otherwise: `target-as/release/search.exe`, built in
this worktree from `agent/argv-sidecar` with
`cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin search
--target-dir target-as`.

**Both window ends are pinned explicitly on every run of F1 phase B and F2.**
Phase A of F1 is the one exception and it is the point of the exercise: it is a
published argv reproduced *verbatim*, `--to` only, `--from` omitted.

## 1. Store state at the time of these runs

    XAUDUKA-5m    1,135,389 bars    2010-06-01 00:00 .. 2025-xx (file total)
    XAUDUKA-15m     378,749 bars
    XAUUSD-15m      100,586 bars

Bar counts per run are in each run's own sidecar, section 2, together with the
digest of the bars the engine was handed.

## 2. F2 — byte identity. 5 argv × 4 binaries = 20 runs

Driver: `fixtures.sh` (sets a, b, c) and `fixtures-sc.sh` (set d, which adds
`--argv-sidecar=<out>/sidecar/<n>.md` to each). All five carry
`--data=/e/rust/flowdesk/data --interval=15m`.

    f1  --market=xauduka --mode=hypotheses --config=config      --batch-file=docs/hypotheses/2026-09-13-pdhl.toml --exit-mix          --from=2018-06-01 --to=2026-06-01
    f2  --market=xauduka --mode=hypotheses --config=config      --batch-file=docs/hypotheses/2026-09-13-pdhl.toml --exit-mix --guards --from=2018-06-01 --to=2026-06-01
    f3  --market=xauduka --mode=rescore    --config=config      --batch-file=docs/hypotheses/2026-09-13-pdhl.toml --exit-mix          --from=2018-06-01 --to=2026-06-01
    f4  --market=xauduka --mode=costs      --config=config                                                                            --from=2018-06-01 --to=2026-06-01
    f5  --market=xauusd  --mode=hypotheses --config=config-swap --batch-file=docs/hypotheses/2026-09-13-pdhl.toml --exit-mix          --from=2024-06-01 --to=2026-06-01

The four binaries:

    a  /e/rust/fd-stop-width/target-sw/release/search.exe      (= 29c8635, the round's shared binary)
    b  target-as/release/search.exe built from 29c8635 in THIS worktree, independently
    c  target-as/release/search.exe post-change, no --argv-sidecar passed
    d  target-as/release/search.exe post-change, --argv-sidecar passed

Result: `F2-byte-identity.txt`. One md5 per fixture across all four.

## 3. F1 — the positive control. 4 published commands × 3 phases = 12 runs

The four published commands are reproduced out of
`docs/research/runs/<id>/in-sample.txt`'s own `$ …` echo line. Every one of
them is `--to=2025-04-10` with **no `--from=`**:

    $ search.exe --market=xauduka --interval=5m --mode=hypotheses \
        --batch-file=docs/hypotheses/<id>.toml --seeds=200 --to=2025-04-10

      <id> = 2026-09-13-london-fix | 2026-09-13-volcond-breakout
             2026-09-13-london-range | 2026-09-13-pdhl

Phases, driver `f1-controls.sh` + `f1-phaseC.sh`:

    A   the above, verbatim                      + --argv-sidecar=<out>/sc/<k>-A.md
    B   the above + --from=2022-06-16             + --argv-sidecar=<out>/sc/<k>-B.md
    C   argv rebuilt FROM <k>-B.md SECTION 3 ALONE by replay.py, which reads
        nothing else in the file, + --argv-sidecar=<out>/sc/<k>-C.md

All phases add `--data=/e/rust/flowdesk/data --config=config --seeds=20`.

**`--seeds=20` instead of the published 200, and it is declared:** seeds drive
only the matched null's `null p50` / `null p95`, the percentile and the verdict
word. The method row's trade count and PF do not read them, and the trade count
is the only thing F1 compares. No percentile and no verdict word is read from
any run in this job.

`2022-06-16` is not a guess: it is what the published receipts print in their
own prose — `bars: 198716 from 2022-06-16 00:00 to 2025-04-09 23:55` — which is
the whole point. The information existed; the argv did not carry it.

Result: `F1-POSITIVE-CONTROL.txt`.

## 4. F3 — the blind probe. 5 argv × 1 deliberately-looking build = 5 runs

`probe.py on` inserts one line into the sidecar block of `search.rs`:

    println!("argv-sidecar: wrote {}", arg("argv-sidecar", "<none>"));

then the same five argv of section 2 are run against a build of that. The claim
under test is that **both** guards catch it: the in-tree source test
`sidecar_tests::the_sidecar_block_writes_to_no_stream`, and `diff -r`. Reverted
with `probe.py off` afterwards; the final tree does not contain it.

Result: `F3-BLIND-PROBE.txt`.

## 5. F4 — the inert second path. 1 run

A batch file carrying `[run] argv_sidecar = "…"` and one `[[hypothesis]]`,
through `--mode=hypotheses` with **no** `--argv-sidecar=` on the command line.

Result: `F4-INERT-PATH.txt`, and the same fact is held in-tree by
`sidecar_tests::the_batch_file_run_table_cannot_ask_for_a_sidecar`.

## 6. Not a run: the census of the record as it stands

`PAST-COVERAGE.txt` is a grep over `receipts/`, `docs/research/runs/` and
`docs/decisions/` on this branch. It spends no binary time and it is how the
result bounds what the sidecar does **not** fix.
