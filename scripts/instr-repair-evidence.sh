#!/usr/bin/env bash
# ONE ROW, TWO SWAP ARMS — the instrument test of
# docs/decisions/2026-10-07-instrument-repair.md.
#
# Arm A reads config/ (xauduka swap 0.00, the desk's measured swap-free
# account). Arm B reads config-swap/ (the generic symbol rate, -0.83 USD per
# lot per night, charged both ways as an upper bound). Everything else is held
# at agent/n5's frozen settings, so `trades`, `OOS PF` and `expect` must
# reproduce receipts/n5/{A,B}-duka-is.txt to the printed digit.
#
# --data is READ-ONLY. data-sealed/ is not opened, read, pointed at or counted.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
bin="$here/target/release/search.exe"
out="$here/receipts/instr-repair"
mkdir -p "$out"

row="--batch-file=$here/docs/research/designs/2026-10-07-instr-repair-one-row.toml"
common=(
  --market=xauduka
  --mode=hypotheses
  --fixed
  --exit-mix
  --null-sides=exposure
  --interval=15m
  --from=2010-06-01
  --to=2018-06-01
  --data=/e/rust/flowdesk/data
  "$row"
)

"$bin" "${common[@]}" --seeds=200 "--config=$here/config"      > "$out/A-swap0.txt"       2>&1
"$bin" "${common[@]}" --seeds=200 "--config=$here/config-swap" > "$out/B-swapcharged.txt" 2>&1

# And the flag audit itself, caught in the act: the same row with three flags
# this mode does not read — two real ones and a misspelling — so the header has
# to say so. `--seeds=20` because this run is read for its header, not its
# percentile, and 20 seeds is not a distribution.
"$bin" "${common[@]}" --seeds=20 "--config=$here/config" \
  --direction-samples=1000 --rebate-share=0.5 --exitmix > "$out/C-flag-audit.txt" 2>&1 || true

# D and E: the rescore repair. FORMAT EVIDENCE, NOT READINGS — the sample
# counts are deliberately too small for a percentile, and the point is that
# `--mode=rescore` now prints an exit mix and a `cost ... % of R` line at all.
# D is the self-managed row (no stop, so cost/R is NOT MEASURABLE, not 0%);
# E is a stop-based base over two years, so the line has a denominator.
"$bin" --market=xauduka --mode=rescore --exit-mix --seeds=20 --direction-samples=50 \
  --rebate-share=0.45 --interval=15m --from=2010-06-01 --to=2018-06-01 \
  --data=/e/rust/flowdesk/data "$row" "--config=$here/config" --null-sides=exposure \
  > "$out/D-rescore-exitmix.txt" 2>&1

"$bin" --market=xauduka --mode=rescore --exit-mix --seeds=10 --direction-samples=20 \
  --rebate-share=0.45 --interval=15m --from=2016-06-01 --to=2018-06-01 \
  --data=/e/rust/flowdesk/data "--config=$here/config" \
  "--batch-file=$here/docs/research/designs/2026-10-07-instr-repair-cost-probe.toml" \
  > "$out/E-rescore-costR.txt" 2>&1

grep -nE "^qs-h15|expectancy_net|^flags:|% of R|^exits \(" "$out"/*.txt
