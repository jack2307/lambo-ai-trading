"""Precheck 1 SPLIT BY REGIME — the measurement precheck 2 pointed at.

Precheck 2 found the profile's own shape stationary across the 2023-12 seam.
`vprofile-gold-p2b.py` found the reason: the POC and the value area read the
WITHIN-window relative weights, and the seam halved the unevenness of those
weights (gini 0.337 -> 0.180, cv 0.636 -> 0.319, KS D 0.82-0.85). A flat weight
column IS the time profile — the Rust test
`a_constant_volume_reproduces_the_time_profile_exactly` pins that as an
identity, not an approximation.

So the question precheck 1 asks has a different answer on each side of the
seam, and this is where the two-regime claim becomes a number: how often does
the tick-volume POC land in the same bucket as the time POC, in regime A vs
regime B?
"""
import sys

import numpy as np
import pandas as pd

SEAM = pd.Timestamp("2023-12-01", tz="UTC").value // 10**6

rows = pd.read_csv(sys.argv[1])
rows["regime"] = np.where(rows["start_ms"] < SEAM, "A (2022-06..2023-11)", "B (2023-12..2026-09)")

wide = rows.pivot_table(
    index=["window_days", "bucket_scheme", "regime", "start_ms"],
    columns="arm",
    values=["poc", "vah", "val", "bucket_size", "atr14"],
).dropna()

print("precheck 1, split at the 2023-12 seam: agreement between the tick-volume")
print("POC and the committed time POC, per regime.")
print()
hdr = (
    f"{'win':>4} {'bucket':<24} {'arm B':<26} {'regime':<22} {'n':>5} "
    f"{'same bkt':>9} {'exact =':>8} {'med|dPOC| ATR':>14} {'p90 ATR':>9} {'med|dVA| ATR':>13}"
)
print(hdr)
print("-" * len(hdr))

res = {}
for wd in sorted(rows["window_days"].unique()):
    for scheme in ["ATR14_OVER_4", "FIXED_0.10_USD", "FIXED_0.01_USD_ONE_TICK"]:
        for b in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
            for reg in sorted(rows["regime"].unique()):
                try:
                    sub = wide.xs((wd, scheme, reg), level=("window_days", "bucket_scheme", "regime"))
                except KeyError:
                    continue
                if len(sub) < 5:
                    continue
                a = sub[("poc", "A_TIME_AT_PRICE")].to_numpy()
                bb = sub[("poc", b)].to_numpy()
                bs = sub[("bucket_size", "A_TIME_AT_PRICE")].to_numpy()
                atr = sub[("atr14", "A_TIME_AT_PRICE")].to_numpy()
                d = np.abs(bb - a)
                dva = np.abs(
                    (sub[("vah", b)].to_numpy() - sub[("val", b)].to_numpy())
                    - (sub[("vah", "A_TIME_AT_PRICE")].to_numpy() - sub[("val", "A_TIME_AT_PRICE")].to_numpy())
                )
                same = float((d < bs).mean())
                exact = float((d < 1e-9).mean())
                print(
                    f"{wd:>4} {scheme:<24} {b:<26} {reg:<22} {len(d):>5} "
                    f"{100*same:>8.1f}% {100*exact:>7.1f}% {np.median(d/atr):>14.3f} "
                    f"{np.quantile(d/atr,0.9):>9.3f} {np.median(dva/atr):>13.3f}"
                )
                res[(wd, scheme, b, reg[0])] = (same, exact, len(d))
            print()

print("the one number this job turns on — share of windows where the tick-volume POC")
print("falls in the SAME bucket as the committed time POC, regime A vs regime B:")
print()
print(f"  {'win':>4} {'bucket':<24} {'arm B':<26} {'A':>8} {'B':>8} {'B - A':>8}")
for wd in sorted(rows["window_days"].unique()):
    for scheme in ["ATR14_OVER_4", "FIXED_0.10_USD", "FIXED_0.01_USD_ONE_TICK"]:
        for b in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
            ka, kb = res.get((wd, scheme, b, "A")), res.get((wd, scheme, b, "B"))
            if not ka or not kb:
                continue
            print(
                f"  {wd:>4} {scheme:<24} {b:<26} {100*ka[0]:>7.1f}% {100*kb[0]:>7.1f}% "
                f"{100*(kb[0]-ka[0]):>+7.1f}pp"
            )

print()
print("MAX_BUCKETS = 5000 selection on the one-tick scheme (price_levels.rs:432):")
for wd in sorted(rows["window_days"].unique()):
    tot = rows[(rows["window_days"] == wd) & (rows["bucket_scheme"] == "ATR14_OVER_4") & (rows["arm"] == "A_TIME_AT_PRICE")]
    tick = rows[(rows["window_days"] == wd) & (rows["bucket_scheme"] == "FIXED_0.01_USD_ONE_TICK") & (rows["arm"] == "A_TIME_AT_PRICE")]
    for reg in sorted(rows["regime"].unique()):
        nt = (tot["regime"] == reg).sum()
        nk = (tick["regime"] == reg).sum()
        print(f"  window_days={wd} regime {reg}: {nk}/{nt} windows survive a 0.01 USD bucket ({100*nk/max(nt,1):.1f}%)")
