"""How big is the weekend gap on gold, and does a stop survive it?

The question the owner asked: why be flat before the weekend at all? The
registered studies say a weekend HOLD has no edge; they do not say what the
gap costs a position that is already open with a stop. This measures that,
from the desk's own Dukascopy 15m gold on disk.

A weekend is any gap in the 15m series longer than 12 hours. For each one:
  gap = first open after the break - last close before it
and it is expressed in points and in units of a 12-point stop, which is
roughly what the live books size (1.2 x ATR(14), ~11.9 on 2026-09-18).
"""
import sys
import pandas as pd

STOP_PTS = 12.0
path = r"E:\rust\flowdesk\data\bars\XAUDUKA-15m.parquet"
df = pd.read_parquet(path)
cols = {c.lower(): c for c in df.columns}
t = cols.get("time") or cols.get("timestamp") or cols.get("t")
o, c = cols.get("open"), cols.get("close")
df = df[[t, o, c]].rename(columns={t: "time", o: "open", c: "close"}).sort_values("time")
# The column is already a timestamp in this store; the epoch branches are
# kept for a file that is not.
if pd.api.types.is_datetime64_any_dtype(df["time"]):
    df["dt"] = pd.to_datetime(df["time"], utc=True)
elif df["time"].max() > 10_000_000_000:
    df["dt"] = pd.to_datetime(df["time"], unit="ms", utc=True)
else:
    df["dt"] = pd.to_datetime(df["time"], unit="s", utc=True)

df["prev_close"] = df["close"].shift(1)
df["prev_dt"] = df["dt"].shift(1)
df["hours"] = (df["dt"] - df["prev_dt"]).dt.total_seconds() / 3600.0
breaks = df[df["hours"] > 12].copy()
breaks["gap"] = breaks["open"] - breaks["prev_close"]
breaks["abs"] = breaks["gap"].abs()
breaks["in_stops"] = breaks["abs"] / STOP_PTS
breaks["pct"] = 100 * breaks["abs"] / breaks["prev_close"]

print("file: %s" % path)
print("bars %d, from %s to %s" % (len(df), df["dt"].iloc[0].date(), df["dt"].iloc[-1].date()))
print("weekend breaks found: %d (a gap in the series longer than 12h)" % len(breaks))
print()
q = breaks["abs"].quantile([0.5, 0.8, 0.9, 0.95, 0.99, 1.0])
print("absolute gap, in POINTS         in units of a %.0f-point stop" % STOP_PTS)
for lbl, v in [("median", q[0.5]), ("80th", q[0.8]), ("90th", q[0.9]),
               ("95th", q[0.95]), ("99th", q[0.99]), ("worst", q[1.0])]:
    print("  %-8s %8.2f pts   %8.2f%% of price   %6.2f stops" % (lbl, v, 100 * v / breaks["prev_close"].mean(), v / STOP_PTS))
print()
over = (breaks["abs"] > STOP_PTS).mean() * 100
over2 = (breaks["abs"] > 2 * STOP_PTS).mean() * 100
print("weekends whose gap alone exceeds one stop  : %.1f%%" % over)
print("weekends whose gap alone exceeds two stops : %.1f%%" % over2)
print()
print("the last ten weekends in the file")
for _, r in breaks.tail(10).iterrows():
    print("  %s  close %8.2f -> open %8.2f   gap %+7.2f pts  (%+.2f stops)"
          % (r["dt"].strftime("%Y-%m-%d %H:%MZ"), r["prev_close"], r["open"], r["gap"], r["gap"] / STOP_PTS))
print()
recent = breaks[breaks["dt"] >= "2025-09-01"]
if len(recent):
    rq = recent["abs"].quantile([0.5, 0.9, 0.95, 1.0])
    print("last 12 months only (%d weekends): median %.2f, 90th %.2f, 95th %.2f, worst %.2f pts"
          % (len(recent), rq[0.5], rq[0.9], rq[0.95], rq[1.0]))
    print("  worst in units of a %.0f-pt stop: %.2f" % (STOP_PTS, rq[1.0] / STOP_PTS))
