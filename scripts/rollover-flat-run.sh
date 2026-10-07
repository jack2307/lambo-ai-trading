#!/usr/bin/env bash
# The six declared runs of docs/decisions/2026-10-07-rollover-flat.md.
#
# 2 windows (split by time) x 3 arms x 8 rows = 48 cells.
#   arm A  guards off, swap 0.00          config/
#   arm B  guards off, swap -0.83/-0.83   config-swap/   (generic symbol rate)
#   arm C  guards ON,  swap 0.00          config/
#
# Binary is built on this branch: the shared fd-instr-repair binary cannot know
# a strategy id that did not exist when it was built, and a binary swallows an
# id it does not know in silence.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="$here/target/release/search.exe"
rows="$here/docs/research/designs/2026-10-07-rollover-flat.toml"
out="$here/receipts/rollover-flat"
mkdir -p "$out"

common=(--mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=200
        --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
        "--batch-file=$rows")

run() { # run <name> <from> <to> <config> [extra...]
  local name="$1" from="$2" to="$3" cfg="$4"; shift 4
  echo "[$(date +%H:%M:%S)] $name"
  "$bin" "${common[@]}" "--from=$from" "--to=$to" "--config=$cfg" "$@" \
    > "$out/$name.txt" 2>&1
  echo "[$(date +%H:%M:%S)] $name exit=$?"
}

run A-duka-is  2010-06-01 2018-06-01 "$here/config"
run A-duka-oos 2018-06-01 2026-06-01 "$here/config"
run B-duka-is  2010-06-01 2018-06-01 "$here/config-swap"
run B-duka-oos 2018-06-01 2026-06-01 "$here/config-swap"
run C-duka-is  2010-06-01 2018-06-01 "$here/config" --guards
run C-duka-oos 2018-06-01 2026-06-01 "$here/config" --guards
echo "[$(date +%H:%M:%S)] all six done"
