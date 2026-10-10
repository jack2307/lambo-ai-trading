"""Why precheck 2 did not fire: the seam is a LEVEL shift of the tick column,
and the POC / value area read only the WITHIN-WINDOW relative weights.

So the quantity to test for stationarity is not the tick level but the shape of
the tick column inside a window. Measured here three ways, all dimensionless,
per window, then compared across the 2023-12 seam the same way precheck 2 did.
"""
import numpy as np
import pandas as pd
import pyarrow.parquet as pq

SEAM = pd.Timestamp("2023-12-01", tz="UTC")


def ks_2samp(x, y):
    x = np.sort(np.asarray(x, float))
    y = np.sort(np.asarray(y, float))
    n1, n2 = len(x), len(y)
    allv = np.concatenate([x, y])
    d = float(np.max(np.abs(np.searchsorted(x, allv, "right") / n1 - np.searchsorted(y, allv, "right") / n2)))
    en = np.sqrt(n1 * n2 / (n1 + n2))
    lam = (en + 0.12 + 0.11 / en) * d
    p = 2.0 * sum((-1) ** (k - 1) * np.exp(-2.0 * k * k * lam * lam) for k in range(1, 101))
    return d, float(min(max(p, 0.0), 1.0))


df = pq.read_table(r"E:\rust\flowdesk\data\bars\XAUUSD-15m.parquet").to_pandas()
df["ts"] = pd.to_datetime(df["time"], utc=True)
df["ms"] = (df["ts"] - pd.Timestamp("1970-01-01", tz="UTC")).dt.total_seconds().mul(1000).round().astype("int64")
step = int(np.median(np.diff(df["ms"].to_numpy())))
gap = df["ms"].diff().fillna(0) >= step * 4
df["day"] = gap.cumsum()

print(f"bar step = {step/60000:.0f} min; trading-day runs = {df['day'].nunique()}")

per = []
for d, g in df.groupby("day"):
    v = g["volume"].to_numpy(float)
    if len(v) < 10 or v.sum() <= 0:
        continue
    p = v / v.sum()
    s = np.sort(p)[::-1]
    gini = 1.0 - 2.0 * np.sum(np.cumsum(np.sort(p)) - np.sort(p) / 2.0) / len(p)
    per.append(
        {
            "start": g["ts"].iloc[0],
            "bars": len(v),
            # Dimensionless shape of the tick column inside the window.
            "cv": v.std(ddof=0) / v.mean(),
            "top10_share": s[: max(1, len(s) // 10)].sum(),
            "gini": gini,
            "entropy_norm": float(-(p[p > 0] * np.log(p[p > 0])).sum() / np.log(len(p))),
            # The LEVEL, for contrast: this is what the seam moved.
            "median_ticks": float(np.median(v)),
        }
    )
per = pd.DataFrame(per)
per["regime"] = np.where(per["start"] < SEAM, "A", "B")

print()
print("within-window SHAPE of the tick column (what POC/value area actually read),")
print("vs the LEVEL of it (what the 2023-12 seam moved):")
print(f"  {'quantity':<34} {'medA':>10} {'medB':>10} {'ratio B/A':>10} {'KS D':>7} {'KS p':>11}")
for q in ["cv", "top10_share", "gini", "entropy_norm", "median_ticks"]:
    a = per.loc[per["regime"] == "A", q].to_numpy()
    b = per.loc[per["regime"] == "B", q].to_numpy()
    d, p = ks_2samp(a, b)
    ma, mb = np.median(a), np.median(b)
    tag = "  <- LEVEL" if q == "median_ticks" else ""
    print(f"  {q:<34} {ma:>10.4f} {mb:>10.4f} {mb/ma:>10.3f} {d:>7.3f} {p:>11.3g}{tag}")

print()
print(f"windows: regime A = {(per['regime']=='A').sum()}, regime B = {(per['regime']=='B').sum()}")
print()
print("the LEVEL, month by month, beside the SHAPE (cv), to show which one has the seam:")
per["ym"] = per["start"].dt.strftime("%Y-%m")
m = per.groupby("ym").agg(med_ticks=("median_ticks", "median"), cv=("cv", "median"), n=("cv", "size"))
prev = None
for k, r in m.iterrows():
    mark = ""
    if prev is not None and r["med_ticks"] / prev < 0.5:
        mark = "   <<<< LEVEL STEP"
    print(f"  {k}  median_ticks={r['med_ticks']:>8.1f}  cv={r['cv']:.3f}  n={int(r['n']):>2}{mark}")
    prev = r["med_ticks"]
