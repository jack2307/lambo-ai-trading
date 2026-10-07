#!/usr/bin/env bash
# Every run of docs/decisions/2026-10-07-news-inner-stop.md, in order.
#
# The binary is THIS worktree's own, built from agent/news-inner-stop
# (agent/instr-repair + cherry-picked ca9125c). The shared
# /e/rust/fd-instr-repair/target/release/search.exe does NOT know `newsonly:`
# or `news-pulse`, and brief section 1 records that an older binary swallows an
# unknown flag silently, so it is not used here.
#
# --data is read-only. Nothing under /e/rust/flowdesk is written.
set -u
cd "$(dirname "$0")/.."
BIN=./target/release/search.exe
DATA=/e/rust/flowdesk/data
LOG=receipts/nis_runlog.txt
: > "$LOG"

A_FROM=2022-01-01; A_TO=2026-01-01   # A', agent/n1's window, 127 USD impact-3 events
B_FROM=2018-01-01; B_TO=2022-01-01   # B', agent/n1's window, 134 events

run () {            # run <name> <interval> <from> <to> <extra flags...>
  local name=$1 iv=$2 from=$3 to=$4; shift 4
  echo "[$(date +%H:%M:%S)] $name" | tee -a "$LOG"
  "$BIN" --market=xauduka "--interval=$iv" --mode=hypotheses "--data=$DATA" \
         "--from=$from" "--to=$to" "--batch-file=receipts/nis-$iv.toml" \
         --fixed --exit-mix "$@" > "receipts/nis_$name.txt" 2>&1
  echo "    exit=$? rows=$(grep -cE '^nis' "receipts/nis_$name.txt")" | tee -a "$LOG"
}

# --- the gate: 16 rows x 2 windows x 2 guard arms x 2 intervals = 128 cells ---
run 1m_A_noguards 1m "$A_FROM" "$A_TO"
run 1m_A_guards   1m "$A_FROM" "$A_TO" --guards
run 1m_B_noguards 1m "$B_FROM" "$B_TO"
run 1m_B_guards   1m "$B_FROM" "$B_TO" --guards
run 5m_A_noguards 5m "$A_FROM" "$A_TO"
run 5m_A_guards   5m "$A_FROM" "$A_TO" --guards
run 5m_B_noguards 5m "$B_FROM" "$B_TO"
run 5m_B_guards   5m "$B_FROM" "$B_TO" --guards

# --- the zero-spread diagnostic, NO GATE: 16 rows x 2 windows = 32 cells ---
# The only way to separate "no direction" from "direction paid to the spread".
run 1m_A_spread0 1m "$A_FROM" "$A_TO" --spread=0.0
run 1m_B_spread0 1m "$B_FROM" "$B_TO" --spread=0.0

echo "[$(date +%H:%M:%S)] ALL DONE" | tee -a "$LOG"
