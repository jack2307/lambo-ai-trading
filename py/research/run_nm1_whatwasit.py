"""Two last diagnostics on the cell that inverted out of sample.

A-FOMC measured +0.5165 ATR20d net at the 100th percentile of a matched null on
2010-06 -> 2021-12, and -0.2937 / -0.2225 ATR20d at the 6.9th / 17.1th percentile on the
two out-of-sample windows. Something produced the in-sample number. These two tests say
what, as far as it can be established.

  10 accrual by session  where the in-sample gain appears. Slow-moving capital predicts a
                         smooth accrual over sessions 1-3. FOMC is always a Wednesday and a
                         3-session hold from Wednesday evening therefore always exits on the
                         following Monday, so the hold always spans a weekend. If the whole
                         gain is the weekend, the mechanism is not what was registered -- and
                         the desk's own [trading.guards] weekend flat would forbid the trade.
  11 Wednesday placebo   the same rule on NON-FOMC Wednesdays at the same hour, same hold.
                         FOMC is always Wednesday, so this is the tight placebo: it separates
                         "FOMC" from "Wednesday evening at 19:00 UTC held over a weekend".
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import pandas as pd
from new_method_1 import (load_bars, sessionise, events, method_a, stats, fmt,
                          nanos, bar_at_or_after, as_trades, COST)

R = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/flowdesk"


def win(path, lo, hi):
    df = load_bars(os.path.join(R, path))
    df = df[(df["time"] >= pd.Timestamp(lo, tz="UTC")) &
            (df["time"] < pd.Timestamp(hi, tz="UTC"))].reset_index(drop=True)
    return sessionise(df)


def accrual(df, first, atr, nsess, ev, label, cost=COST):
    """Net ATR20d from entry to the open of session+k, for k = 1..5, on the same entries."""
    tns = nanos(df["time"]); o = df["open"].to_numpy(); c = df["close"].to_numpy()
    sess = df["sess"].to_numpy()
    print("  %s" % label)
    for k in (1, 2, 3, 4, 5):
        rows = []
        for ts in ev["time_utc"]:
            i = bar_at_or_after(tns, ts)
            if i is None or i + 4 >= len(df):
                continue
            mv = c[i + 3] - o[i]
            if mv == 0:
                continue
            side = 1 if mv > 0 else -1
            e = i + 4
            xs = sess[e] + k
            if xs >= nsess:
                continue
            x = first[xs]
            rows.append((tns[e], tns[x], side, (o[x] - o[e]) * side, atr[sess[e]]))
        tr = as_trades(rows)
        s = stats(tr["raw"], tr["atr"], "    exit at session +%d" % k)
        dow = pd.to_datetime(tr["t_out"]).dt.day_name().value_counts().to_dict()
        print(fmt(s) + "   exit weekday: %s" % dow)


def weekday_placebo(df, first, atr, nsess, ev, hours, label, cost=COST, weekday=2):
    """The same rule entered at the same hour on the same weekday, on weeks with no FOMC."""
    tns = nanos(df["time"]); o = df["open"].to_numpy(); c = df["close"].to_numpy()
    sess = df["sess"].to_numpy()
    fomc_days = set(ev["time_utc"].dt.date)
    tt = df["time"]
    mask = ((tt.dt.dayofweek == weekday) & tt.dt.hour.isin(hours) & (tt.dt.minute == 0)
            & (~tt.dt.date.isin(fomc_days))).to_numpy()
    rows = []
    for i in np.flatnonzero(mask):
        if i + 4 >= len(df):
            continue
        mv = c[i + 3] - o[i]
        if mv == 0:
            continue
        side = 1 if mv > 0 else -1
        e = i + 4
        xs = sess[e] + 3
        if xs >= nsess:
            continue
        x = first[xs]
        rows.append((tns[e], tns[x], side, (o[x] - o[e]) * side, atr[sess[e]]))
    tr = as_trades(rows)
    s = stats(tr["raw"], tr["atr"], label)
    print(fmt(s))
    nl = int((tr["side"] > 0).sum())
    print("      long/short split: %d long / %d short (%.1f%% long)   hold<=0: %d"
          % (nl, len(tr) - nl, 100.0 * nl / max(len(tr), 1),
             int((tr["t_out"] <= tr["t_in"]).sum())))
    return s


for lab, path, lo, hi in (("IN SAMPLE  XAUDUKA", "data/bars/XAUDUKA-15m.parquet",
                           "2010-06-01", "2022-01-01"),
                          ("OOS 1      XAUDUKA", "data/bars/XAUDUKA-15m.parquet",
                           "2022-01-01", "2025-09-23")):
    print("\n" + "=" * 100)
    print(lab + "   " + lo + " -> " + hi)
    print("=" * 100)
    df, first, atr, nsess = win(path, lo, hi)
    ev = events(os.path.join(R, "data/news/events-extended.csv"))
    ev = ev[(ev["name"] == "FOMC") & (ev["time_utc"] >= pd.Timestamp(lo, tz="UTC")) &
            (ev["time_utc"] < pd.Timestamp(hi, tz="UTC"))].reset_index(drop=True)
    dow = ev["time_utc"].dt.day_name().value_counts().to_dict()
    print("  FOMC events: %d, weekday: %s" % (len(ev), dow))
    print("\n  ATTACK 10  accrual by session (net ATR20d, cost %.2f)" % COST)
    accrual(df, first, atr, nsess, ev, "same entries, exit moved out one session at a time")
    hours = sorted(set(ev["time_utc"].dt.hour))
    print("\n  ATTACK 11  Wednesday placebo, same hours %s, no FOMC that day, hold 3 sessions"
          % hours)
    weekday_placebo(df, first, atr, nsess, ev, hours,
                    "    NON-FOMC Wednesday, same rule")
    print("    for comparison, the cell itself:")
    tr = method_a(df, first, atr, nsess, ev, rel_bars=4, hold_sess=3)
    print(fmt(stats(tr["raw"], tr["atr"], "    A-FOMC")))
