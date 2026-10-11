#!/bin/bash
# 12 declared POSITIVE CONTROLS. Each pins back what moved, and the published
# trade count must return exactly. Nothing is built; the shared binary is used.
EXE=/e/rust/fd-stop-width/target-sw/release/search.exe
R=/e/rust/fd-receipt-reconcile
H=$R/docs/hypotheses
O="$1"
mkdir -p "$O"
run () { n="$1"; shift; echo "### $n"; "$EXE" "$@" --data=/e/rust/flowdesk/data --config=$R/config > "$O/$n.txt" 2>&1; echo "exit $?"; }

# ---- W1: pin --from to the published first bar -------------------------------
run w1-london-fix      --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-london-fix.toml       --seeds=200 --from=2022-06-16 --to=2025-04-10
run w1-volcond         --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-volcond-breakout.toml --seeds=200 --from=2022-06-16 --to=2025-04-10
run w1-london-range-d  --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-london-range.toml     --seeds=200 --from=2022-06-16 --to=2025-04-10
run w1-pdhl            --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-pdhl.toml             --seeds=200 --from=2022-06-16 --to=2025-04-10
run w1-gold-m15        --market=xauduka --interval=15m --mode=hypotheses --batch-file=$H/2026-09-13-gold-m15-check.toml   --seeds=200 --from=2022-06-16 --to=2025-04-10
run w1-orb-ny-oos      --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-orb-ny.toml           --seeds=200 --from=2022-06-16 --to=2026-06-01
run w1-orb-ny-mn-oos   --market=xauduka --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-orb-ny-matched-null.toml --seeds=200 --from=2022-06-16 --to=2026-06-01

# ---- W2: pin --to to the published last bar ---------------------------------
run w2-close-drift     --market=xauusd  --interval=15m --mode=hypotheses --fixed --batch-file=$H/2026-09-13-close-reopen-drift.toml --seeds=300 --from=2025-04-11 --to=2026-09-12
run w2-london-range-u  --market=xauusd  --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-london-range.toml     --seeds=200 --from=2025-04-11 --to=2026-09-12
run w2-orb-ny-is       --market=xauusd  --interval=5m  --mode=hypotheses --batch-file=$H/2026-09-13-orb-ny.toml           --seeds=200 --from=2025-04-11 --to=2026-09-12

# ---- P1: the colliding grid override deleted from a COPY of the TOML --------
run p1-vwap-nopin      --market=btc     --interval=5m  --mode=hypotheses --batch-file=C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/rr/ctl/vwap-fade-nopin.toml  --seeds=200
run p1-btc-m15-nopin   --market=btc     --interval=15m --mode=hypotheses --batch-file=C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/rr/ctl/btc-m15-nopin.toml    --seeds=200
