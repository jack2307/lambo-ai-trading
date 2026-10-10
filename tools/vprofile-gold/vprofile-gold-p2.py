"""Precheck 2 for agent/vprofile-gold: does the profile STAND STILL across the
2023-12 seam?

Three quantities per window, all dimensionless so arm A and arm B are directly
comparable: value-area width in ATR(14), POC position inside the window's own
price range (0..1), and concentration (POC-bucket share, plus normalised
entropy). Regime A = 2022-06-16..2023-11-30, regime B = 2023-12-01..2026-09-17.

Arm A (the committed time profile) is the CONTROL: if both arms shift, what
shifted is the market, not the volume column. Two-sample KS is written out
here rather than imported so the p-value's formula is visible.
"""
import sys

import numpy as np
import pandas as pd

SEAM = pd.Timestamp("2023-12-01", tz="UTC").value // 10**6


def ks_2samp(x, y):
    """Two-sample KS statistic and asymptotic p-value."""
    x = np.sort(np.asarray(x, dtype=float))
    y = np.sort(np.asarray(y, dtype=float))
    n1, n2 = len(x), len(y)
    allv = np.concatenate([x, y])
    cdf1 = np.searchsorted(x, allv, side="right") / n1
    cdf2 = np.searchsorted(y, allv, side="right") / n2
    d = float(np.max(np.abs(cdf1 - cdf2)))
    en = np.sqrt(n1 * n2 / (n1 + n2))
    lam = (en + 0.12 + 0.11 / en) * d
    # Kolmogorov distribution tail, summed until it stops mattering.
    p = 2.0 * sum((-1) ** (k - 1) * np.exp(-2.0 * k * k * lam * lam) for k in range(1, 101))
    return d, float(min(max(p, 0.0), 1.0))


rows = pd.read_csv(sys.argv[1])
rows["regime"] = np.where(rows["start_ms"] < SEAM, "A", "B")
rows["va_width_atr"] = (rows["vah"] - rows["val"]) / rows["atr14"]
rng = rows["max_high"] - rows["min_low"]
rows["poc_pos"] = (rows["poc"] - rows["min_low"]) / rng
rows["va_width_usd"] = rows["vah"] - rows["val"]

QUANTS = [
    ("va_width_atr", "value-area width / ATR(14,15m)"),
    ("poc_pos", "POC position in window range (0..1)"),
    ("peak_share", "POC-bucket share of total (concentration)"),
    ("entropy_norm", "normalised histogram entropy"),
]

for wd in sorted(rows["window_days"].unique()):
    for scheme in ["ATR14_OVER_4", "FIXED_0.10_USD", "FIXED_0.01_USD_ONE_TICK"]:
        sel = rows[(rows["window_days"] == wd) & (rows["bucket_scheme"] == scheme)]
        if sel.empty:
            continue
        print(f"\n=== window_days={wd}  bucket={scheme} ===")
        nA = (sel["regime"] == "A").sum() // sel["arm"].nunique()
        nB = (sel["regime"] == "B").sum() // sel["arm"].nunique()
        print(f"    windows: regime A ~{nA}, regime B ~{nB}")
        print(
            f"    {'quantity':<42} {'arm':<26} {'medA':>9} {'medB':>9} "
            f"{'|dmed|':>8} {'KS D':>7} {'KS p':>10}"
        )
        store = {}
        for q, label in QUANTS:
            for arm in ["A_TIME_AT_PRICE", "B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
                s = sel[sel["arm"] == arm]
                a = s.loc[s["regime"] == "A", q].dropna().to_numpy()
                b = s.loc[s["regime"] == "B", q].dropna().to_numpy()
                if len(a) < 5 or len(b) < 5:
                    continue
                d, p = ks_2samp(a, b)
                dm = abs(np.median(b) - np.median(a))
                store[(q, arm)] = (dm, p, d)
                print(
                    f"    {label:<42} {arm:<26} {np.median(a):>9.4f} {np.median(b):>9.4f} "
                    f"{dm:>8.4f} {d:>7.3f} {p:>10.3g}"
                )
            print()

        # The registered firing rule, evaluated.
        if scheme == "ATR14_OVER_4":
            print("    FALSIFIER P2 as registered (primary bucket = the route's own):")
            print("      fires iff, for >= 2 of 3 quantities, arm B has KS p < 0.01 AND")
            print("      |median shift| at least 2x the control arm A's.")
            for arm in ["B1_TICK_VOLUME_PER_BUCKET", "B2_TICK_VOLUME_SPREAD"]:
                hits = []
                for q, label in QUANTS[:3]:
                    kb = store.get((q, arm))
                    ka = store.get((q, "A_TIME_AT_PRICE"))
                    if not kb or not ka:
                        continue
                    dmB, pB, _ = kb
                    dmA, _, _ = ka
                    ratio = dmB / dmA if dmA > 0 else float("inf")
                    ok = pB < 0.01 and ratio >= 2.0
                    hits.append((label, pB, dmB, dmA, ratio, ok))
                n_ok = sum(1 for h in hits if h[5])
                print(f"      {arm}: {n_ok}/3 clauses met -> {'FIRES' if n_ok >= 2 else 'does NOT fire'}")
                for label, pB, dmB, dmA, ratio, ok in hits:
                    print(
                        f"        {label:<42} p={pB:<10.3g} |dmed|B={dmB:.4f} "
                        f"|dmed|A={dmA:.4f} ratio={ratio:.2f} {'MET' if ok else '-'}"
                    )
