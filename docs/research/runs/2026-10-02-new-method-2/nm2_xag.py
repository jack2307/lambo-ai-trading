"""Silver's 20-day sign: the long/short split and the drift it was exposed to.

Read-only. Reproduces the shape of `tsmom` as the record describes it: at the
first bar at or after 16:15 New York on a weekday, hold the side of the trailing
20-day return; flip on a sign change. Measured here as the per-weekday return
attributable to the held side, not as the engine's trade list, because the
question is only the side's exposure and the drift it rode.
"""
import math
import numpy as np
import pandas as pd
import pyarrow.parquet as pq

ROOT = "E:/rust/flowdesk"


def tstat(x):
    x = np.asarray(x, float)
    return x.mean() / x.std(ddof=1) * math.sqrt(len(x)) if len(x) > 1 and x.std(ddof=1) else float("nan")


def load(sym):
    t = pq.read_table(ROOT + "/data/bars/%s-15m.parquet" % sym)
    df = t.to_pandas().sort_values("time").reset_index(drop=True)
    ny = df["time"].dt.tz_convert("America/New_York")
    df["m"] = ny.dt.hour * 60 + ny.dt.minute
    df["ny"] = ny
    df["date"] = pd.to_datetime(ny.dt.date)
    df["wd"] = ny.dt.weekday
    return df


for sym, unit, spread in (("XAGDUKA", "$/oz silver", 0.021), ("XAUDUKA", "$/oz gold", 0.28)):
    df = load(sym)
    # the 16:15 New York decision price per weekday
    wd = df[(df["wd"] <= 4) & (df["m"] >= 975)]
    dec = wd.groupby("date").first()[["open", "ny"]].rename(columns={"open": "p"})
    dec = dec.sort_index()
    print("=" * 100)
    print("%s  %d weekday decision bars at or after 16:15 New York, %s -> %s"
          % (sym, len(dec), dec.index[0].date(), dec.index[-1].date()))
    for lab, a, b in (("window 1  2010-06 -> 2018-06", "2010-06-01", "2018-06-15"),
                      ("window 2  2018-06 -> 2026-05", "2018-06-16", "2026-05-31")):
        d = dec[(dec.index >= a) & (dec.index < b)].copy()
        if len(d) < 40:
            continue
        d["fwd"] = d["p"].shift(-1) - d["p"]           # next weekday's decision price
        d["look"] = d["p"] - d["p"].shift(14)          # ~20 calendar days = 14 weekdays
        d["side"] = np.sign(d["look"])
        d = d.dropna(subset=["fwd", "side"])
        d = d[d["side"] != 0]
        d["pnl"] = d["side"] * d["fwd"]
        nlong = int((d["side"] > 0).sum()); nshort = int((d["side"] < 0).sum())
        drift = d["fwd"].mean()
        print("  %s   %d weekday steps" % (lab, len(d)))
        print("      unconditional step %+.5f %s per weekday (t %+.2f)  -- what being long pays for nothing"
              % (drift, unit, tstat(d["fwd"])))
        print("      sides held: long %d (%.1f%%), short %d (%.1f%%)"
              % (nlong, 100.0 * nlong / len(d), nshort, 100.0 * nshort / len(d)))
        print("      the sign's return   %+.5f %s per step (t %+.2f)" % (d["pnl"].mean(), unit, tstat(d["pnl"])))
        print("      SIDES FLIPPED       %+.5f %s per step (t %+.2f)" % (-d["pnl"].mean(), unit, -tstat(d["pnl"])))
        ex = d["pnl"] - d["side"] * drift   # remove the drift the held side was exposed to
        print("      minus the drift the held side rode  %+.5f %s per step (t %+.2f)"
              % (ex.mean(), unit, tstat(ex)))
        lo = d[d["side"] > 0]["pnl"]; sh = d[d["side"] < 0]["pnl"]
        print("      long steps %+.5f (t %+.2f) | short steps %+.5f (t %+.2f)"
              % (lo.mean(), tstat(lo), sh.mean(), tstat(sh)))
    print()
