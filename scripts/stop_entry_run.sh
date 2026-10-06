#!/usr/bin/env bash
# The 24 invocations declared in docs/decisions/2026-10-07-stop-entry.md:
# 2 windows x 6 entry arms x 2 guard arms, six mechanisms each = 144 rows.
#
# The arms mirror agent/n3's exactly, with the fourth field of --limit flipped
# from `pullback` to `breakout`, so the two halves of the resting-order family
# are read against one another without a second set of market rows.
set -u
cd /e/rust/fd-stop-entry || exit 1
BIN=./target/release/search.exe
DATA=/e/rust/flowdesk/data
CFG=/e/rust/fd-stop-entry/config
BATCH=$CFG/n3-limit.toml
mkdir -p receipts
for win in A B; do
  if [ "$win" = A ]; then FROM=2025-07-01; TO=2025-10-01; else FROM=2025-04-01; TO=2025-07-01; fi
  for arm in market b000 b025 b050 t025 t050; do
    case $arm in
      market) L="off" ;;
      b000)   L="0.0,4,anchored,breakout" ;;
      b025)   L="0.25,4,anchored,breakout" ;;
      b050)   L="0.5,4,anchored,breakout" ;;
      t025)   L="0.25,4,carry,breakout" ;;
      t050)   L="0.5,4,carry,breakout" ;;
    esac
    for g in guards noguards; do
      if [ "$g" = guards ]; then G="--guards"; else G=""; fi
      out="receipts/stopentry-$win-$arm-$g.txt"
      # shellcheck disable=SC2086
      $BIN --market=xauusd --mode=hypotheses --fixed --exit-mix \
        --data="$DATA" --config="$CFG" --batch-file="$BATCH" \
        --from=$FROM --to=$TO --limit=$L --seeds=200 --null-sides=coin $G \
        > "$out" 2>&1
      echo "$out  rc=$?"
    done
  done
done
