#!/usr/bin/env bash
# D2: the same arm-G sweep with the xauusd hold cap lifted to seven days.
set -u
BIN=/e/rust/fd-wt-crt/target/release/search.exe
WT=/e/rust/fd-a9
OUT=$WT/receipts/nocap
mkdir -p "$OUT"
for w in "A 2025-07-01 2025-10-01" "B 2025-04-01 2025-07-01"; do
  set -- $w; wtag=$1; from=$2; to=$3
  for b in gold-intraday ict-m1 ict-m5 ict-oos; do
    "$BIN" --market=xauusd --mode=hypotheses --data=/e/rust/flowdesk/data \
      --config=$WT/config-nocap --from="$from" --to="$to" --batch="$b" \
      --fixed --exit-mix --seeds=200 --guards > "$OUT/G-$wtag-builtin-$b.txt" 2>&1
  done
  for f in "$WT"/docs/hypotheses/*.toml; do
    "$BIN" --market=xauusd --mode=hypotheses --data=/e/rust/flowdesk/data \
      --config=$WT/config-nocap --from="$from" --to="$to" --batch-file="$f" \
      --fixed --exit-mix --seeds=200 --guards \
      > "$OUT/G-$wtag-file-$(basename "$f" .toml).txt" 2>&1
  done
done
