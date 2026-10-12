#!/usr/bin/env bash
# agent/l3-target: the runs that hold the declared cells.
#
#   PART A - control (a), a REPRODUCTION with a known published answer:
#     agent/range-sizing's own method, TOML and companion file, rebuilt from
#     its own script. It must read back PF_r 0.7115 / 0.7030 (W1 guards-ON)
#     and 0.7873 / 0.7667 (W2 guards-ON). If it does not, the tooling is wrong
#     and nothing else here may be read (falsifier F6).
#     1 companion x 2 windows x 2 arms = 4 runs x 4 labels = 16 lines, 0 new cells.
#
#   PART B - the L3 axis:
#     3 companion files x 2 windows x 2 arms = 12 runs x 8 labels = 96 lines.
#     The four L2/* labels do not read the distance column, so they must print
#     identical numbers in all three companion runs: positive control (d),
#     32 deliberate duplicate lines.
#
# Windows are pinned on the COMMAND LINE at BOTH ends (addendum 11 section VII:
# an argv with an empty --from or --to is not a reproducible specification, and
# 25 of 34 rows of the record broke on exactly that). `[run]` cannot set them
# under --fixed either (addendum 8 section VI).
set -u
cd "$(dirname "$0")/.." || exit 1

BIN=./target-l3/release/search.exe
DATA=/e/rust/fd-l3-target/data-l3
OUT=receipts/l3-target
mkdir -p "$OUT"

WINDOWS=("W1:2010-06-01:2018-06-01" "W2:2018-06-01:2026-06-01")

run_one() {
  local toml="$1" src="$2" wl="$3" from="$4" to="$5" arm="$6"
  local g=""
  [ "$arm" = on ] && g=--guards
  local f="$OUT/$wl-$src-guards-$arm.txt"
  echo "== $f"
  $BIN --market=xauduka --interval=15m --data="$DATA" --config=config \
    --mode=hypotheses --batch-file="$toml" \
    --from="$from" --to="$to" --fixed --companion="$src" --exit-mix $g \
    >"$f" 2>&1
  echo "   exit $? $(grep -cE '^(atr|fc|L2|L3)/' "$f") label lines"
}

case "${1:-all}" in
  a|all)
    for win in "${WINDOWS[@]}"; do
      IFS=: read -r wl from to <<<"$win"
      for arm in off on; do
        run_one docs/hypotheses/2026-10-11-range-sizing.toml XAURFC1M "$wl" "$from" "$to" "$arm"
      done
    done
    ;;
esac

case "${1:-all}" in
  b|all)
    for src in XAUL3TRU XAUL3SHF XAUL3CON; do
      for win in "${WINDOWS[@]}"; do
        IFS=: read -r wl from to <<<"$win"
        for arm in off on; do
          run_one docs/hypotheses/2026-10-11-l3-target.toml "$src" "$wl" "$from" "$to" "$arm"
        done
      done
    done
    ;;
esac
