#!/usr/bin/env bash
# agent/range-sizing: the 12 runs that hold the 32 declared gate cells.
#
#   3 companion files  x  2 windows  x  2 guard arms  =  12 runs
#   each run prints 4 labels: atr/f1.0 atr/f2.0 fc/f1.0 fc/f2.0
#
# The two `atr/*` labels do not read the companion's VALUE, so they must print
# identical numbers in all three companion runs - positive control #2 of
# docs/decisions/2026-10-11-range-sizing.md. The 8 duplicates are deliberate.
#
# Windows are set on the COMMAND LINE because `[run]` cannot set them under
# --fixed (addendum 8 section VI).
set -u
cd "$(dirname "$0")/.." || exit 1

BIN=./target-rs/release/search.exe
DATA=/e/rust/fd-range-sizing/data-rs
TOML=docs/hypotheses/2026-10-11-range-sizing.toml
OUT=receipts/range-sizing
mkdir -p "$OUT"

for src in XAURFC15 XAURFC1M XAURFCSH; do
  for win in "W1:2010-06-01:2018-06-01" "W2:2018-06-01:2026-06-01"; do
    IFS=: read -r wl from to <<<"$win"
    for arm in off on; do
      g=""
      [ "$arm" = on ] && g=--guards
      f="$OUT/$wl-$src-guards-$arm.txt"
      echo "== $f"
      $BIN --market=xauduka --interval=15m --data="$DATA" --config=config \
        --mode=hypotheses --batch-file="$TOML" \
        --from="$from" --to="$to" --fixed --companion="$src" --exit-mix $g \
        >"$f" 2>&1
      echo "   exit $? $(grep -c '^atr/\|^fc/' "$f") label lines"
    done
  done
done
