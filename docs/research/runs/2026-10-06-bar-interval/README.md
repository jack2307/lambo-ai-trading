# 2026-10-06 bar-interval axis — receipts

The ten receipts and the two generated tables live in `receipts/` at the root
of this worktree (branch `agent/m3`), not duplicated here:

    A1-{15m,1h,4h}-window{A,B}.txt   Arm 1, standing max_hold_ms = 4 h
    A2-{1h,4h}-window{A,B}.txt       Arm 2, ceiling scaled to sixteen bars
    Z-bars-built.txt                 the 1h/4h aggregation, bucket census
    Z-summary.txt                    every one of the 130 cells, parsed

Read them with the record in docs/decisions/2026-10-06-bar-interval.md.

The 1h and 4h parquets are DERIVED from the 15m store file by
scripts/htf_bars_from_15m.py and are not committed; rerun that script to
rebuild them bit for bit.
