"""Gold's unconditional drift, gross and net of the measured spread, by horizon.

This is NOT a trading cell and cannot promote anything. It is a descriptive measurement of
the instrument, added after the three registered mechanisms were decided, because Method B's
hour-matched control threw off a by-product worth more than B was: gold's gross drift over a
four-hour window is positive and overwhelming, and net of 0.28 price units it is negative and
overwhelming.

Two things had to be fixed before that by-product could be quoted as a finding.

  OVERLAP. The control in `method_b_control` starts a window at EVERY 15-minute bar in the
  chosen hours, so a 4-hour window shares 15 of its 16 bars with the next one. n = 59,376
  windows are nowhere near 59,376 independent observations and the t-statistics printed beside
  them are inflated by roughly sqrt(16) = 4. The means are unbiased; the t's are not. Here
  every horizon is measured on STRICTLY NON-OVERLAPPING windows, stride = the window length.

  HORIZON. The desk's standing drift figure is +0.3946 ATR20 per 5 sessions, t = 6.74. Cost
  is a fixed 0.28 price units however long the window, so whether the drift is collectable is
  a question about horizon and cannot be answered at one horizon. Measured here at 4 hours,
  1 session, 5 sessions and 20 sessions, gross and net, on the same bars.
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import pandas as pd
from new_method_1 import load_bars, sessionise, COST

R = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/flowdesk"


def drift(df, first, atr, nsess, label, horizons_bars, horizons_sess, cost=COST):
    o = df["open"].to_numpy()
    sess = df["sess"].to_numpy()
    print("\n  %s" % label)
    print("    %-14s %7s  %10s %8s   %10s %8s   %9s" %
          ("horizon", "n", "gross ATR", "t", "net ATR", "t", "gross px"))
    rows = []
    for name, span in horizons_bars:
        idx = np.arange(0, len(df) - span, span)          # stride = span: no overlap
        a = atr[sess[idx]]
        raw = o[idx + span] - o[idx]
        ok = np.isfinite(a) & (a > 0)
        rows.append((name, raw[ok], a[ok]))
    for name, ks in horizons_sess:
        starts, a_l, raw_l = [], [], []
        k = 0
        while k + ks < nsess:
            i, j = first[k], first[k + ks]
            if np.isfinite(atr[k]) and atr[k] > 0:
                starts.append(i); a_l.append(atr[k]); raw_l.append(o[j] - o[i])
            k += ks                                        # stride = ks sessions: no overlap
        rows.append((name, np.array(raw_l), np.array(a_l)))
    for name, raw, a in rows:
        if len(raw) < 3:
            print("    %-14s %7d  null" % (name, len(raw))); continue
        g = raw / a
        n_ = (raw - cost) / a
        tg = g.mean() / (g.std(ddof=1) / np.sqrt(len(g)))
        tn = n_.mean() / (n_.std(ddof=1) / np.sqrt(len(n_)))
        print("    %-14s %7d  %+10.4f %8.2f   %+10.4f %8.2f   %+9.3f"
              % (name, len(raw), g.mean(), tg, n_.mean(), tn, raw.mean()))


HB = [("4 hours", 16), ("1 hour", 4)]
HS = [("1 session", 1), ("5 sessions", 5), ("20 sessions", 20)]

for lab, path, lo, hi in (
        ("DESIGN WINDOW  XAUDUKA 15m", "data/bars/XAUDUKA-15m.parquet", "2010-06-01", "2022-01-01"),
        ("OOS 1          XAUDUKA 15m", "data/bars/XAUDUKA-15m.parquet", "2022-01-01", "2025-09-23"),
        ("OOS 2          XAUUSD 15m", "data/bars/XAUUSD-15m.parquet", "2022-06-16", "2025-09-23")):
    df = load_bars(os.path.join(R, path))
    df = df[(df["time"] >= pd.Timestamp(lo, tz="UTC")) &
            (df["time"] < pd.Timestamp(hi, tz="UTC"))].reset_index(drop=True)
    df, first, atr, nsess = sessionise(df)
    print("\n" + "=" * 100)
    print("%s  %s -> %s   %d bars, %d sessions, ATR20d mean %.2f px, median %.2f px"
          % (lab, lo, hi, len(df), nsess, np.nanmean(atr), np.nanmedian(atr)))
    print("=" * 100)
    drift(df, first, atr, nsess, "non-overlapping windows, cost %.2f px round trip" % COST,
          HB, HS)

print("\n" + "=" * 100)
print("the inflation the overlap caused, on the design window, at 4 hours")
print("=" * 100)
df = load_bars(os.path.join(R, "data/bars/XAUDUKA-15m.parquet"))
df = df[(df["time"] >= pd.Timestamp("2010-06-01", tz="UTC")) &
        (df["time"] < pd.Timestamp("2022-01-01", tz="UTC"))].reset_index(drop=True)
df, first, atr, nsess = sessionise(df)
o = df["open"].to_numpy(); sess = df["sess"].to_numpy()
for stride, what in ((1, "every bar starts a window (what method_b_control did)"),
                     (16, "stride 16 bars = no overlap")):
    idx = np.arange(0, len(df) - 16, stride)
    a = atr[sess[idx]]; raw = o[idx + 16] - o[idx]
    ok = np.isfinite(a) & (a > 0)
    g = raw[ok] / a[ok]; n_ = (raw[ok] - COST) / a[ok]
    print("    %-52s n=%6d  gross %+.4f ATR t=%5.2f | net %+.4f ATR t=%6.2f"
          % (what, len(g), g.mean(), g.mean() / (g.std(ddof=1) / np.sqrt(len(g))),
             n_.mean(), n_.mean() / (n_.std(ddof=1) / np.sqrt(len(n_)))))
