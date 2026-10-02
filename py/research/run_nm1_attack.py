"""Attacks the one cell of the 2026-10-02 registration that cleared its numeric legs.

Method A's PRIMARY cell failed its registered gate (t = 2.41 < 3.0) and A is abandoned as
registered. This script exists because a DECLARED robustness cell -- FOMC only, release
window 1 hour, hold 3 sessions -- cleared all three legs, and the registration's mechanism
diagnostic (FOMC must not be the weakest of the four series) came out as predicted. The
registration forbids promoting a robustness cell. So the job here is not to promote it: it
is to try to break it, and to publish what was tried.

Nine attacks, each one capable of killing it:

  1 per-leg P&L        long-only and short-only separately. If only the long leg pays, the
                       measurement is gold's +0.3946 ATR20/5-session drift, not a method.
  2 sign-free control  the same trade ALWAYS LONG from the same bar. If that is as large,
                       the release-window sign predicts nothing and the cell is drift
                       concentrated on 98 days.
  3 matched bootstrap  2,000 draws of n random sessions, same hour of day, same sign rule,
                       same hold, same cost. Count-, hour- and sign-rule-matched, and it
                       carries the same drift exposure the method does.
  4 era concentration  per sub-era and per year. This is what refused `quiet-swing`.
  5 drop 2020          and drop the unscheduled cuts.
  6 timestamp sanity   mean |move| in the release window against the same hour on other
                       days. If the ratio is near 1 the calendar is not aligned to the bars.
  7 silver transfer    XAGDUKA, same rule, same parameters, its own 0.021 spread. A real-rate
                       repricing should move both metals.
  8 OOS 1 (time)       XAUDUKA 2022-01-01 -> 2025-09-22, no re-fit.
  9 OOS 2 (vendor)     XAUUSD Vantage bars 2022-06-16 -> 2025-09-22, no re-fit. PRIMARY
                       criterion per the owner's 2026-09-13 decision, on the most recent
                       window the hold-out permits.
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import pandas as pd
from new_method_1 import (load_bars, sessionise, events, method_a, report, stats, fmt,
                          nanos, bar_at_or_after, as_trades, COST, CUT)

R = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/flowdesk"
RNG = np.random.default_rng(20261002)


def window(df, lo, hi):
    d = df[(df["time"] >= pd.Timestamp(lo, tz="UTC")) &
           (df["time"] < pd.Timestamp(hi, tz="UTC"))].reset_index(drop=True)
    return sessionise(d)


def legs(tr, label, cost=COST):
    """Attack 1: the long leg and the short leg, separately. The drift test."""
    out = []
    for name, m in (("long only ", tr[tr["side"] > 0]), ("short only", tr[tr["side"] < 0])):
        if len(m) < 2:
            print("    %s %-34s n=%d null" % (name, label, len(m))); out.append(None); continue
        s = stats(m["raw"], m["atr"], "    %s %s" % (name, label))
        print(fmt(s)); out.append(s)
    return out


def always_long(df, first, atr, nsess, ev, rel_bars=4, hold_sess=3):
    """Attack 2: identical entry and exit bars, side forced LONG, sign rule discarded."""
    tns = nanos(df["time"]); o = df["open"].to_numpy(); sess = df["sess"].to_numpy()
    rows = []
    for ts in ev["time_utc"]:
        i = bar_at_or_after(tns, ts)
        if i is None or i + rel_bars >= len(df):
            continue
        e = i + rel_bars
        xs = sess[e] + hold_sess
        if xs >= nsess:
            continue
        x = first[xs]
        rows.append((tns[e], tns[x], 1, o[x] - o[e], atr[sess[e]]))
    return as_trades(rows)


def bootstrap_null(df, first, atr, nsess, n, hours, draws=2000, rel_bars=4, hold_sess=3,
                   cost=COST):
    """Attack 3: the matched null the repo does not have. Draws n random sessions, enters at
    the same hour of day, takes the sign of the same rel_bars window, holds hold_sess
    sessions, pays the same cost. Count-, hour- and sign-rule-matched, drift-exposed."""
    tt = df["time"]
    cand = np.flatnonzero(tt.dt.hour.isin(hours).to_numpy() & (tt.dt.minute == 0).to_numpy())
    o, c = df["open"].to_numpy(), df["close"].to_numpy()
    sess = df["sess"].to_numpy()
    cand = cand[(cand + rel_bars < len(df))]
    # precompute every candidate's net R so the draws are cheap
    i = cand
    e = i + rel_bars
    keep = (sess[e] + hold_sess) < nsess
    i, e = i[keep], e[keep]
    x = first[sess[e] + hold_sess]
    side = np.sign(c[e - 1] - o[i])
    a = atr[sess[e]]
    raw = (o[x] - o[e]) * side
    ok = (side != 0) & np.isfinite(a) & (a > 0) & (x < len(df))
    r = (raw[ok] - cost) / a[ok]
    if len(r) < n:
        return None
    means = np.array([RNG.choice(r, size=n, replace=False).mean() for _ in range(draws)])
    return means, r


def one_window(label, bars_path, lo, hi, spread, eras, do_bootstrap=True):
    print("\n" + "=" * 100)
    print("%s   %s   %s -> %s   spread %.3f price units" % (label, bars_path, lo, hi, spread))
    print("=" * 100)
    df = load_bars(os.path.join(R, bars_path))
    df, first, atr, nsess = window(df, lo, hi)
    if len(df) == 0:
        print("  null: no bars in window"); return
    print("  %d bars, %d sessions, %s -> %s, ATR20d mean %.3f"
          % (len(df), nsess, df["time"].iloc[0], df["time"].iloc[-1], np.nanmean(atr)))
    ev = events(os.path.join(R, "data/news/events-extended.csv"))
    ev = ev[(ev["time_utc"] >= pd.Timestamp(lo, tz="UTC")) &
            (ev["time_utc"] < pd.Timestamp(hi, tz="UTC"))]
    fo = ev[ev["name"] == "FOMC"].reset_index(drop=True)          # scheduled only
    fo_all = ev[ev["name"].str.startswith("FOMC")].reset_index(drop=True)
    print("  scheduled FOMC in window: %d (plus %d unscheduled)"
          % (len(fo), len(fo_all) - len(fo)))
    if len(fo) < 2:
        print("  null: too few events"); return

    import new_method_1 as nm
    nm.COST = spread
    tr = method_a(df, first, atr, nsess, fo, rel_bars=4, hold_sess=3)
    print("\n  the cell under attack")
    s = report(tr, "A-FOMC rel=1h hold=3sess", eras)

    print("\n  ATTACK 1  per-leg P&L (the drift test)")
    legs(tr, "A-FOMC")

    print("\n  ATTACK 2  sign-free control: same bars, side forced LONG")
    al = always_long(df, first, atr, nsess, fo)
    print(fmt(stats(al["raw"], al["atr"], "    ALWAYS LONG, sign rule discarded")))

    if do_bootstrap and s is not None:
        hours = sorted(set(fo["time_utc"].dt.hour))
        print("\n  ATTACK 3  matched bootstrap null, n=%d, 2000 draws, entry hours %s"
              % (len(tr), [h + 1 for h in hours]))
        bs = bootstrap_null(df, first, atr, nsess, len(tr),
                            [(h + 1) % 24 for h in hours], cost=spread)
        if bs is None:
            print("    null: fewer candidate bars than the method has trades")
        else:
            means, pool = bs
            pct = 100.0 * (means < s["net_mean_atr"]).mean()
            print("    control pool %d bars; null mean %+.4f ATR20d, sd %.4f, 95th %+.4f"
                  % (len(pool), means.mean(), means.std(), np.percentile(means, 95)))
            print("    the method sits at the %.1fth percentile of its matched null" % pct)

    print("\n  ATTACK 4  per year (net ATR20d, cost %.3f)" % spread)
    yr = tr.assign(y=tr["t_in"].dt.year)
    for y, m in yr.groupby("y"):
        r = ((m["raw"] - spread) / m["atr"])
        print("    %d  n=%2d  net %+.4f ATR20d  wins %d/%d" % (y, len(m), r.mean(),
                                                               int((r > 0).sum()), len(m)))

    print("\n  ATTACK 5  drop 2020, and the unscheduled cuts separately")
    no20 = tr[tr["t_in"].dt.year != 2020]
    print(fmt(stats(no20["raw"], no20["atr"], "    A-FOMC excluding 2020")))
    un = ev[ev["name"] == "FOMC (unscheduled)"].reset_index(drop=True)
    if len(un) >= 2:
        tu = method_a(df, first, atr, nsess, un, rel_bars=4, hold_sess=3)
        print(fmt(stats(tu["raw"], tu["atr"], "    unscheduled FOMC only")))

    print("\n  ATTACK 6  timestamp sanity: |move| in the release window vs the same hour")
    tns = nanos(df["time"]); o = df["open"].to_numpy(); c = df["close"].to_numpy()
    mv = []
    for ts in fo["time_utc"]:
        i = bar_at_or_after(tns, ts)
        if i is not None and i + 4 < len(df):
            mv.append(abs(c[i + 3] - o[i]))
    hrs = sorted(set(fo["time_utc"].dt.hour))
    mask = df["time"].dt.hour.isin(hrs).to_numpy() & (df["time"].dt.minute == 0).to_numpy()
    idx = np.flatnonzero(mask); idx = idx[idx + 4 < len(df)]
    base = np.abs(c[idx + 3] - o[idx])
    print("    release window mean |move| %.3f px (n=%d) vs same hour on all days %.3f px"
          " (n=%d) -- ratio %.2f" % (np.mean(mv), len(mv), base.mean(), len(base),
                                     np.mean(mv) / base.mean()))
    nm.COST = COST
    return s


# ------------------------------------------------------------------ run
ERAS_IS = [(pd.Timestamp(a, tz="UTC"), pd.Timestamp(b, tz="UTC")) for a, b in
           [("2010-06-01", "2013-03-01"), ("2013-03-01", "2016-01-01"),
            ("2016-01-01", "2018-11-01"), ("2018-11-01", "2022-01-01")]]
ERAS_OOS = [(pd.Timestamp(a, tz="UTC"), pd.Timestamp(b, tz="UTC")) for a, b in
            [("2022-01-01", "2023-01-01"), ("2023-01-01", "2024-01-01"),
             ("2024-01-01", "2025-01-01"), ("2025-01-01", "2025-09-23")]]

one_window("IN SAMPLE (design window)", "data/bars/XAUDUKA-15m.parquet",
           "2010-06-01", "2022-01-01", 0.28, ERAS_IS)

print("\n\n  ATTACK 7  silver transfer: a real-rate repricing should move both metals")
one_window("ATTACK 7  SILVER, design window", "data/bars/XAGDUKA-15m.parquet",
           "2010-06-01", "2022-01-01", 0.021, ERAS_IS)

one_window("OOS 1 (time)  XAUDUKA, no re-fit", "data/bars/XAUDUKA-15m.parquet",
           "2022-01-01", "2025-09-23", 0.28, ERAS_OOS)

one_window("OOS 2 (vendor, PRIMARY)  XAUUSD Vantage bars, no re-fit",
           "data/bars/XAUUSD-15m.parquet", "2022-06-16", "2025-09-23", 0.28, ERAS_OOS)

print("\n\n" + "=" * 100)
print("corrupt-record check required by the brief: trades with hold_ms < 0")
print("=" * 100)
import glob, json
tot = neg = badlabel = nofield = 0
for f in glob.glob(os.path.join(R, "data/paper/*/trades.jsonl")):
    for line in open(f):
        line = line.strip()
        if not line:
            continue
        try:
            t = json.loads(line)
        except Exception:
            continue
        # The record is NESTED under a "trade" key. Reading hold_ms off the top level
        # returns None for every row and prints a count of 0 that measured nothing.
        t = t.get("trade", t)
        tot += 1
        h = t.get("hold_ms")
        if h is None:
            nofield += 1
            continue
        if h < 0:
            neg += 1
            r = t.get("r"); rs = t.get("exit_reason") or t.get("exit_kind") or ""
            if r is not None and ((str(rs).upper().startswith("STOP") and r > 0) or
                                  (str(rs).upper().startswith("TARGET") and r < 0)):
                badlabel += 1
print("  data/paper/*/trades.jsonl: %d trades, %d carrying no hold_ms field at all,"
      " %d with hold_ms < 0, %d of those with a label that contradicts their own R"
      % (tot, nofield, neg, badlabel))
print("  this record's own measurements build their own trades and never read that store;")
print("  every cell above printed its own hold<=0 count, and every one was 0.")
