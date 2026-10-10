"""Precheck 1 for agent/vprofile-gold: is the tick-volume profile
distinguishable from the bar-count (time) profile?

Compared by PRICE DISTANCE (USD, and in ticks of 0.01 USD since digits = 2),
on the same bars, buckets, windows and value_area_pct. Reads the rows the Rust
harness dumped; decides nothing the registration did not declare.
"""
import sys

import numpy as np
import pandas as pd

rows = pd.read_csv(sys.argv[1])
TICK = 0.01
SEAM = pd.Timestamp("2023-12-01", tz="UTC").value // 10**6

rows["start"] = pd.to_datetime(rows["start_ms"], unit="ms", utc=True)
rows["regime"] = np.where(rows["start_ms"] < SEAM, "A", "B")

key = ["window_days", "bucket_scheme", "start_ms"]
wide = rows.pivot_table(
    index=key,
    columns="arm",
    values=["poc", "vah", "val", "bucket_size", "atr14", "peak_share", "entropy_norm", "buckets"],
)
wide = wide.dropna()

print("precheck 1 — POC / value-area distance between arms, by window length and bucket scheme")
print("(distance in USD and in ticks of 0.01 USD; 'same bucket' = distance < bucket_size)")
print()
hdr = (
    f"{'win':>4} {'bucket scheme':<24} {'arm B':<26} {'n':>5} "
    f"{'med|dPOC|':>10} {'p90|dPOC|':>10} {'med tick':>9} {'p90 tick':>9} "
    f"{'same bkt':>9} {'=0':>7} {'med|dVAH|':>10} {'med|dVAL|':>10} {'med VAwid A':>11} {'med VAwid B':>11}"
)
print(hdr)
print("-" * len(hdr))

summary = {}
for wd in sorted(rows["window_days"].unique()):
    for scheme in ["ATR14_OVER_4", "FIXED_0.10_USD", "FIXED_0.01_USD_ONE_TICK"]:
        sub = wide.xs(wd, level="window_days", drop_level=False)
        sub = sub.xs(scheme, level="bucket_scheme", drop_level=False)
        if sub.empty:
            continue
        for b in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
            a = sub[("poc", "A_TIME_AT_PRICE")].to_numpy()
            bb = sub[("poc", b)].to_numpy()
            d = np.abs(bb - a)
            bs = sub[("bucket_size", "A_TIME_AT_PRICE")].to_numpy()
            dvah = np.abs(sub[("vah", b)].to_numpy() - sub[("vah", "A_TIME_AT_PRICE")].to_numpy())
            dval = np.abs(sub[("val", b)].to_numpy() - sub[("val", "A_TIME_AT_PRICE")].to_numpy())
            wa = sub[("vah", "A_TIME_AT_PRICE")].to_numpy() - sub[("val", "A_TIME_AT_PRICE")].to_numpy()
            wb = sub[("vah", b)].to_numpy() - sub[("val", b)].to_numpy()
            same = float((d < bs).mean())
            exact = float((d < 1e-9).mean())
            print(
                f"{wd:>4} {scheme:<24} {b:<26} {len(d):>5} "
                f"{np.median(d):>10.3f} {np.quantile(d, 0.9):>10.3f} "
                f"{np.median(d) / TICK:>9.1f} {np.quantile(d, 0.9) / TICK:>9.1f} "
                f"{100 * same:>8.1f}% {100 * exact:>6.1f}% "
                f"{np.median(dvah):>10.3f} {np.median(dval):>10.3f} {np.median(wa):>11.3f} {np.median(wb):>11.3f}"
            )
            summary[(wd, scheme, b)] = (np.median(d), np.quantile(d, 0.9), same, exact)

print()
print("FALSIFIER P1 as registered: fires iff, at the ONE-TICK bucket scheme,")
print("  median|dPOC| <= 1 tick (0.01 USD) AND p90|dPOC| <= 1 tick.")
for wd in sorted(rows["window_days"].unique()):
    for b in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
        k = (wd, "FIXED_0.01_USD_ONE_TICK", b)
        if k not in summary:
            continue
        med, p90, same, exact = summary[k]
        fired = med <= TICK + 1e-12 and p90 <= TICK + 1e-12
        print(
            f"  window_days={wd} {b}: median={med:.4f} USD ({med/TICK:.1f} ticks), "
            f"p90={p90:.4f} USD ({p90/TICK:.1f} ticks)  ->  {'FIRES' if fired else 'does NOT fire'}"
        )

# How far apart are the two POCs relative to the window's own price range and ATR?
print()
print("scale of the disagreement, ATR14_OVER_4 (the route's own bucket):")
for wd in sorted(rows["window_days"].unique()):
    sub = wide.xs(wd, level="window_days", drop_level=False).xs(
        "ATR14_OVER_4", level="bucket_scheme", drop_level=False
    )
    for b in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
        d = np.abs(sub[("poc", b)].to_numpy() - sub[("poc", "A_TIME_AT_PRICE")].to_numpy())
        atr = sub[("atr14", "A_TIME_AT_PRICE")].to_numpy()
        print(
            f"  window_days={wd} {b}: median |dPOC| = {np.median(d):.3f} USD "
            f"= {np.median(d/atr):.3f} ATR(14,15m);  p90 = {np.quantile(d/atr, 0.9):.3f} ATR"
        )
