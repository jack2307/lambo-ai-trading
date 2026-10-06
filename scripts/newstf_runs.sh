#!/bin/sh
# Every run declared in docs/decisions/2026-10-07-news-instrument-timeframe.md §9.
set -e
S=/e/rust/fd-n1bin/target/release/search.exe
D=/e/rust/flowdesk/data
cd /e/rust/fd-news-tf
run() { # name market interval batchfile from to guardflag
  out="receipts/newstf_$1.txt"
  echo "[$(date +%H:%M:%S)] $1"
  $S --market=$2 --interval=$3 --data=$D --config=config --mode=hypotheses \
     --batch-file=$4 --fixed --exit-mix --from=$5 --to=$6 $7 > "$out" 2>&1
}
B=receipts/news-entry.toml
run I1_noguards      xauusd 15m $B 2022-06-16 2024-09-01 ""
run I1_guards        xauusd 15m $B 2022-06-16 2024-09-01 --guards
run I2_noguards      xauusd 15m $B 2024-09-01 2026-09-18 ""
run I2_guards        xauusd 15m $B 2024-09-01 2026-09-18 --guards
run Ifull_noguards   xauusd 15m $B 2022-06-16 2026-09-18 ""
C5=receipts/newstf-5m-clock.toml
run clock5_A_noguards xauduka 5m $C5 2022-01-01 2026-01-01 ""
run clock5_A_guards   xauduka 5m $C5 2022-01-01 2026-01-01 --guards
run clock5_B_noguards xauduka 5m $C5 2018-01-01 2022-01-01 ""
run clock5_B_guards   xauduka 5m $C5 2018-01-01 2022-01-01 --guards
P5=receipts/newstf-5m-probe.toml
run probe5_A_noguards xauduka 5m $P5 2022-01-01 2026-01-01 ""
run probe5_A_guards   xauduka 5m $P5 2022-01-01 2026-01-01 --guards
run probe5_B_noguards xauduka 5m $P5 2018-01-01 2022-01-01 ""
run probe5_B_guards   xauduka 5m $P5 2018-01-01 2022-01-01 --guards
C1=receipts/newstf-1m-clock.toml
run clock1_A_noguards xauduka 1m $C1 2022-01-01 2026-01-01 ""
run clock1_A_guards   xauduka 1m $C1 2022-01-01 2026-01-01 --guards
run clock1_B_noguards xauduka 1m $C1 2018-01-01 2022-01-01 ""
run clock1_B_guards   xauduka 1m $C1 2018-01-01 2022-01-01 --guards
P1=receipts/newstf-1m-probe.toml
run probe1_A_noguards xauduka 1m $P1 2022-01-01 2026-01-01 ""
run probe1_A_guards   xauduka 1m $P1 2022-01-01 2026-01-01 --guards
run probe1_B_noguards xauduka 1m $P1 2018-01-01 2022-01-01 ""
run probe1_B_guards   xauduka 1m $P1 2018-01-01 2022-01-01 --guards
echo "[$(date +%H:%M:%S)] ALL DONE"
