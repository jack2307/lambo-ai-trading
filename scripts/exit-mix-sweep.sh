#!/usr/bin/env bash
# One --exit-mix receipt per (batch, window, guard arm). Reads only the
# shared data store; writes only into this worktree.
set -u
BIN=/e/rust/fd-wt-crt/target/release/search.exe
WT=/e/rust/fd-a9
OUT=$WT/receipts/exitmix
mkdir -p "$OUT"
run() { # arm windowtag from to batchkey batchflag
  local arm=$1 wtag=$2 from=$3 to=$4 key=$5 flag=$6
  local extra=""
  [ "$arm" = "G" ] && extra="--guards"
  "$BIN" --market=xauusd --mode=hypotheses --data=/e/rust/flowdesk/data \
    --config=$WT/config --from="$from" --to="$to" "$flag" \
    --fixed --exit-mix --seeds=200 $extra \
    > "$OUT/$arm-$wtag-$key.txt" 2>&1
}
for arm in G U; do
  for w in "A 2025-07-01 2025-10-01" "B 2025-04-01 2025-07-01"; do
    set -- $w; wtag=$1; from=$2; to=$3
    for b in gold-intraday ict-m1 ict-m5 ict-oos; do
      run "$arm" "$wtag" "$from" "$to" "builtin-$b" "--batch=$b"
    done
    for f in "$WT"/docs/hypotheses/*.toml; do
      key="file-$(basename "$f" .toml)"
      run "$arm" "$wtag" "$from" "$to" "$key" "--batch-file=$f"
    done
  done
done
