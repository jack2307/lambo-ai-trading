"""Generate the 18 declared hypothesis rows for agent/vprofile-gold.

Registered in docs/decisions/2026-10-10-vprofile-gold.md, cell book revised to
72 in the 2026-10-10 note (18 rows x 2 windows x 2 guard arms).
"""
import sys

HEAD = """\
# agent/vprofile-gold - value-area reversion on the price profile, with the
# COMMITTED time profile as the control arm.  Registered in
# docs/decisions/2026-10-10-vprofile-gold.md; 18 declared rows here, run over
# two windows and two guard arms = 72 cells.
#
# `measure` is the axis:
#   0 = TIME_AT_PRICE          - fd_engine::activity_profile, the committed
#                                function, which never reads `volume`.  CONTROL.
#   1 = TICK_VOLUME_AT_PRICE   - fd_engine::volume_profile, PER_TOUCHED_BUCKET:
#                                the same histogram with the bar's `volume` as
#                                the weight instead of 1.0.  ONE variable.
#   2 = TICK_VOLUME_AT_PRICE   - the same, SPREAD_OVER_TOUCHED_BUCKETS.
#
# The volume is MT5 TICK volume - price changes per bar, not size; the parquet
# metadata says so (`volume = "tick_volume (price changes per bar), not
# contracts"`), because CFD real volume is always zero.  Every Dukascopy feed
# publishes 0 or null, so the two volume arms exist ONLY on xauusd:15m, which
# is why both windows below are carved out of that one feed.
#
# The windows cut at the 2023-12 seam, where the within-window SHAPE of the
# tick column steps and never returns (gini 0.337 -> 0.180, entropy 0.959 ->
# 0.988, KS D 0.82-0.85 over 1,100 trading days).
[run]
in_sample = "xauusd:15m"
in_sample_from = "2022-06-16"
in_sample_to = "2023-11-30"
out_of_sample = "xauusd:15m"
out_of_sample_from = "2023-12-01"
seeds = 100
direction_samples = 1000
fixed = true
# NOTE: every key in [run] is DOCUMENTATION.  `batch_from_file` reads only the
# `[[hypothesis]]` tables; the windows, `--fixed`, `--seeds` and `--null-sides`
# come from the CLI (AGENT-BRIEF-ADDENDUM-8 section VI).  The receipt header is
# the record of what actually ran.
"""

MEASURES = [
    (0, "time", "TIME_AT_PRICE (committed activity_profile; CONTROL, reads no volume)"),
    (1, "tvol", "TICK_VOLUME_AT_PRICE / PER_TOUCHED_BUCKET"),
    (2, "tvsp", "TICK_VOLUME_AT_PRICE / SPREAD_OVER_TOUCHED_BUCKETS"),
]

out = [HEAD]
n = 0
for m, mtag, mwhy in MEASURES:
    for days in [1, 5]:
        for thr in [0.00, 0.25, 0.50]:
            n += 1
            out.append(
                f"""
[[hypothesis]]
label = "{mtag}/d{days}/t{thr:.2f}"
base = "vprofile-reversion"
overrides = {{ measure = {m}.0, windowDays = {days}.0, thresholdAtr = {thr:.2f}, \
bucketsPerAtr = 4.0, valueAreaPct = 0.70, bufferAtr = 0.10, maxRiskAtr = 3.0, \
minTargetAtr = 0.25, atrPeriod = 14.0 }}
why = "{mwhy}; prior {days} complete NY day(s); close >= {thr:.2f} ATR(14,15m) outside the value area; target the POC"
"""
            )
assert n == 18, n
open(sys.argv[1], "w", newline="\n", encoding="utf-8").write("".join(out))
print(f"wrote {n} hypothesis rows to {sys.argv[1]}")
