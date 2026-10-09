#!/bin/sh
# agent/flat-native: the remaining 7 declared runs.
SW=/e/rust/fd-stop-width/target-sw/release/search.exe
D=/e/rust/fd-flat-native/docs/research/designs/2026-10-09-flat-native.toml
R=/e/rust/fd-flat-native/receipts/flat-native
CA=/e/rust/fd-rollover-flat/config
CC=/e/rust/fd-flat-native/config-comm
COMMON="--mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=50 --interval=15m --data=/e/rust/flowdesk/data --market=xauduka"
IS="--from=2010-06-01 --to=2018-06-01"
OOS="--from=2018-06-01 --to=2026-06-01"

run() { # $1=outfile $2=config $3=window-flags $4=extra
  echo "=== $1 start $(date +%H:%M:%S)"
  "$SW" $COMMON --config="$2" $3 $4 --batch-file="$D" > "$R/$1" 2>&1
  echo "=== $1 exit=$? $(date +%H:%M:%S)"
}

run A-duka-oos.txt     "$CA" "$OOS" ""
run C-duka-is.txt      "$CA" "$IS"  "--guards"
run C-duka-oos.txt     "$CA" "$OOS" "--guards"
run Acomm-duka-is.txt  "$CC" "$IS"  ""
run Acomm-duka-oos.txt "$CC" "$OOS" ""
run Ccomm-duka-is.txt  "$CC" "$IS"  "--guards"
run Ccomm-duka-oos.txt "$CC" "$OOS" "--guards"
echo "ALL DONE $(date +%H:%M:%S)"
