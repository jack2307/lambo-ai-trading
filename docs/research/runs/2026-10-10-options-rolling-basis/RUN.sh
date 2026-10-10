set -e
OUT=docs/research/runs/2026-10-10-options-rolling-basis
mkdir -p "$OUT"
BASE="--market=xauusd --interval=15m --mode=hypotheses --fixed --exit-mix --config=config-ofirst --data=/e/rust/flowdesk/data --seeds=200 --batch-file=docs/hypotheses/2026-10-09-options-one-window.toml --basis-ref=GC-1m"
for arm in guards noguards; do
  G=""; [ "$arm" = "guards" ] && G="--guards"
  for w in 120 480 1440; do
    f="$OUT/roll-$w-$arm.txt"
    echo "=== roll W=$w $arm -> $f"
    ./target-ob/release/search.exe $BASE --basis-roll=$w --basis-roll-min=5 $G > "$f" 2>&1
  done
  for off in 39.66 42.16 45.48; do
    f="$OUT/const-$off-$arm.txt"
    echo "=== const $off $arm -> $f"
    ./target-ob/release/search.exe $BASE --basis-offset=$off $G > "$f" 2>&1
  done
  for off in 41.26 43.70 45.78; do
    f="$OUT/parent-$off-$arm.txt"
    echo "=== parent-const $off $arm -> $f"
    ./target-ob/release/search.exe $BASE --basis-offset=$off $G > "$f" 2>&1
  done
done
echo DONE
