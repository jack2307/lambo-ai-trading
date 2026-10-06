#!/usr/bin/env bash
# The 24 invocations of amendment (2) of docs/decisions/2026-10-06-limit-entry.md:
# 2 windows x 6 entry arms x 2 guard arms, six mechanisms each = 144 rows.
set -u
cd /e/rust/fd-b3 || exit 1
BIN=./target/release/search.exe
DATA=/e/rust/flowdesk/data
CFG=/e/rust/fd-b3/config
BATCH=$CFG/n3-limit.toml
mkdir -p receipts
for win in A B; do
  if [ "$win" = A ]; then FROM=2025-07-01; TO=2025-10-01; else FROM=2025-04-01; TO=2025-07-01; fi
  for arm in market a000 a025 a050 c025 c050; do
    case $arm in
      market) L="off" ;;
      a000)   L="0.0,4,anchored" ;;
      a025)   L="0.25,4,anchored" ;;
      a050)   L="0.5,4,anchored" ;;
      c025)   L="0.25,4,carry" ;;
      c050)   L="0.5,4,carry" ;;
    esac
    for g in guards noguards; do
      if [ "$g" = guards ]; then G="--guards"; else G=""; fi
      out="receipts/n3-limit-$win-$arm-$g.txt"
      # shellcheck disable=SC2086
      $BIN --market=xauusd --mode=hypotheses --fixed --exit-mix \
        --data="$DATA" --config="$CFG" --batch-file="$BATCH" \
        --from=$FROM --to=$TO --limit=$L --seeds=200 --null-sides=coin $G \
        > "$out" 2>&1
      echo "$out  rc=$?"
    done
  done
done
