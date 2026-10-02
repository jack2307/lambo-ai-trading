"""M4 + M5 for 2026-10-02-new-method-2. Read-only; no bar file is written.

Everything is in New York clock minutes, because every claim under examination
is stated in that clock. Units are printed on every figure.
"""
import sys, math
import numpy as np
import pandas as pd
import pyarrow.parquet as pq

ROOT = "E:/rust/flowdesk"


def Phi(z):
    return 0.5 * (1.0 + math.erf(z / math.sqrt(2.0)))


def load(sym, path=None):
    t = pq.read_table(path or (ROOT + "/data/bars/%s-15m.parquet" % sym))
    df = t.to_pandas().sort_values("time").reset_index(drop=True)
    ny = df["time"].dt.tz_convert("America/New_York")
    df["m"] = ny.dt.hour * 60 + ny.dt.minute
    df["ny"] = ny
    df["date"] = pd.to_datetime(ny.dt.date)
    df["year"] = ny.dt.year
    df["wd"] = ny.dt.weekday
    return df


def tstat(x):
    x = np.asarray(x, dtype=float)
    if len(x) < 2 or x.std(ddof=1) == 0:
        return float("nan")
    return x.mean() / x.std(ddof=1) * math.sqrt(len(x))


def window_holds(df, lo, hi, entry_wd=None, shift_h=0):
    """Open-to-open move over [lo, hi) New York minutes, per calendar day.

    Entry = open of the first bar at or after `lo`; exit = open of the first bar
    at or after `hi` (the engine's convention: signal on a bar, fill at the next
    open, so the open of the bar at/after the boundary is the fill).
    shift_h shifts both boundaries by whole hours (the clock scan).
    """
    lo = (lo + shift_h * 60) % 1440
    hi = (hi + shift_h * 60) % 1440
    out = []
    g = df.groupby("date")
    dates = list(g.groups.keys())
    idx = {d: i for i, d in enumerate(dates)}
    for d in dates:
        sub = g.get_group(d)
        if entry_wd is not None and sub["wd"].iloc[0] != entry_wd:
            continue
        e = sub[sub["m"] >= lo]
        if e.empty:
            continue
        ei = e.index[0]
        x = sub[(sub["m"] >= hi) & (sub.index > ei)]
        if not x.empty:
            xi = x.index[0]
        else:
            # the exit boundary is not on this calendar day: take the first bar
            # at or after `hi` on a later day (the weekend / halt case)
            later = df[(df.index > ei) & (df["m"] >= hi) & (df["date"] > d)]
            if later.empty:
                continue
            xi = later.index[0]
        hours = (df["ny"].iloc[xi] - df["ny"].iloc[ei]).total_seconds() / 3600.0
        out.append((d, df["open"].iloc[ei], df["open"].iloc[xi], hours,
                    df["ny"].iloc[ei], df["ny"].iloc[xi]))
    return pd.DataFrame(out, columns=["date", "entry", "exit", "hours", "t_in", "t_out"])


def report(name, h, df, unit, spread, window_label):
    """Raw move, and the same move detrended two ways. Units stated."""
    if h.empty:
        print("  %-28s no holds" % name)
        return None
    h = h.copy()
    h["raw"] = h["exit"] - h["entry"]
    # drift control 1: the sample's own unconditional drift per hour over the window
    per_h = (df["open"].iloc[-1] - df["open"].iloc[0]) / (
        (df["ny"].iloc[-1] - df["ny"].iloc[0]).total_seconds() / 3600.0)
    h["ex_sample"] = h["raw"] - per_h * h["hours"]
    # drift control 2: the hold's own calendar week, pro-rated by the hold's length
    wk = df.set_index("ny")["open"].resample("7D").agg(["first", "last"])
    wkmove = {}
    for ts, row in wk.iterrows():
        wkmove[ts] = (row["last"] - row["first"]) if pd.notna(row["first"]) else np.nan
    keys = np.array(sorted(wkmove.keys()))
    def week_of(ts):
        i = np.searchsorted(keys, ts, side="right") - 1
        return wkmove[keys[i]] if 0 <= i < len(keys) else np.nan
    h["wkmove"] = [week_of(t) for t in h["t_in"]]
    h["ex_week"] = h["raw"] - h["wkmove"] * (h["hours"] / 168.0)
    print("  %-28s %s | %4d holds, mean hold %.1f h" % (name, window_label, len(h), h["hours"].mean()))
    print("      raw move          %+8.4f %s/hold  (t %+6.2f)  sum %+10.1f" % (
        h["raw"].mean(), unit, tstat(h["raw"]), h["raw"].sum()))
    print("      minus sample drift %+8.4f %s/hold  (t %+6.2f)   [unconditional drift %+.5f %s/h]" % (
        h["ex_sample"].mean(), unit, tstat(h["ex_sample"]), per_h, unit))
    print("      minus own week     %+8.4f %s/hold  (t %+6.2f)" % (
        h["ex_week"].mean(), unit, tstat(h["ex_week"])))
    print("      spread charged %.4f %s/hold -> net of raw %+8.4f, net of sample-drift excess %+8.4f" % (
        spread, unit, h["raw"].mean() - spread, h["ex_sample"].mean() - spread))
    yr = h.groupby(h["t_in"].dt.year)
    print("      per year (holds, raw, minus-sample-drift, minus-own-week) in %s/hold:" % unit)
    for y, sub in yr:
        print("        %d %4d %+8.3f %+8.3f %+8.3f" % (
            y, len(sub), sub["raw"].mean(), sub["ex_sample"].mean(), sub["ex_week"].mean()))
    return h


def main():
    what = sys.argv[1]

    if what == "gold":
        xd = load("XAUDUKA")
        print("=" * 100)
        print("M5 - GOLD, the drift control the repository does not have")
        print("XAUDUKA 15m, %d bars %s -> %s; spread 0.28 price units a round turn (0.14 each side)"
              % (len(xd), xd['ny'].iloc[0], xd['ny'].iloc[-1]))
        print()
        segs = [("window 1 of `fri`  2018-06-16 -> 2025-04-10", "2018-06-16", "2025-04-10"),
                ("window 2 of `fri`  2010-06-01 -> 2018-06-15", "2010-06-01", "2018-06-15")]
        for lab, a, b in segs:
            d = xd[(xd["date"] >= a) & (xd["date"] < b)].reset_index(drop=True)
            print("-" * 100)
            print(lab, "   %d bars" % len(d))
            report("fri 16:30 -> Sun 18:30 LONG", window_holds(d, 16 * 60 + 30, 18 * 60 + 15, entry_wd=4),
                   d, "$/oz", 0.28, lab)
            print()
            report("close 16:30 -> 18:30 LONG Mo-Th", window_holds(
                d[d["wd"] <= 3].reset_index(drop=True), 16 * 60 + 30, 18 * 60 + 15), d, "$/oz", 0.28, lab)
            print()
        # the Vantage confirmation window of `close`
        xv = load("XAUUSD")
        d = xv[(xv["date"] >= "2025-04-11") & (xv["date"] < "2026-09-12")].reset_index(drop=True)
        print("-" * 100)
        print("window 2 of `close`  2025-04-11 -> 2026-09-11, XAUUSD (Vantage)   %d bars" % len(d))
        report("close 16:30 -> 18:30 LONG Mo-Th", window_holds(
            d[d["wd"] <= 3].reset_index(drop=True), 16 * 60 + 30, 18 * 60 + 15), d, "$/oz", 0.28,
            "2025-04 -> 2026-09")
        print()
        report("fri 16:30 -> Sun 18:30 LONG", window_holds(d, 16 * 60 + 30, 18 * 60 + 15, entry_wd=4),
               d, "$/oz", 0.28, "2025-04 -> 2026-09")

    elif what == "cost":
        print("=" * 100)
        print("M4.3 - a constant 0.28 price-unit spread is not a constant test")
        xd = load("XAUDUKA"); xv = load("XAUUSD")
        print("  year  feed      mean price $/oz   0.28 as basis points of price")
        for nm, d in (("XAUDUKA", xd), ("XAUUSD", xv)):
            for y, sub in d.groupby("year"):
                p = sub["close"].mean()
                print("  %4d  %-8s  %10.2f        %8.2f bp" % (y, nm, p, 0.28 / p * 10000.0))

    elif what == "boundary":
        print("=" * 100)
        print("M4.2 - the 17:00-17:59 New York bucket: does the daily break exist in both windows?")
        for sym in ("XAUDUKA", "XAGDUKA", "EURDUKA", "XAUUSD"):
            d = load(sym)
            print("\n  %s" % sym)
            print("    year  bars  bars in 17:00-17:59 NY  bars per weekday  weekdays with a 16:15 bar")
            for y, sub in d.groupby("year"):
                wd = sub[sub["wd"] <= 4]
                n1700 = int(((sub["m"] >= 1020) & (sub["m"] < 1080)).sum())
                ndays = wd["date"].nunique()
                has1615 = wd[wd["m"] == 975]["date"].nunique()
                print("    %4d %7d %12d %21.1f %13d" % (
                    y, len(sub), n1700, len(wd) / max(ndays, 1), has1615))

    elif what == "source":
        print("=" * 100)
        print("M4.1 - are the two windows of `fri` the same file segment?")
        cur = load("XAUDUKA")
        bak = load("XAUDUKA", ROOT + "/data/bars/XAUDUKA-15m.parquet.bak")
        print("  XAUDUKA-15m.parquet      %7d bars  %s -> %s" % (len(cur), cur['ny'].iloc[0], cur['ny'].iloc[-1]))
        print("  XAUDUKA-15m.parquet.bak  %7d bars  %s -> %s" % (len(bak), bak['ny'].iloc[0], bak['ny'].iloc[-1]))
        print("  => window 1 of `fri` (2018-06 -> 2025-04) is inside the .bak; window 2 (2010-06 -> 2018-06)")
        print("     exists only in the merged file, downloaded 2026-09-13. Two downloads, one file.")
        ov = cur.merge(bak[["time", "open", "close"]], on="time", suffixes=("", "_bak"))
        if len(ov):
            d1 = (ov["open"] - ov["open_bak"]).abs().max()
            d2 = (ov["close"] - ov["close_bak"]).abs().max()
            print("  overlap %d bars; max |open diff| %.6f, max |close diff| %.6f price units"
                  % (len(ov), d1, d2))
        else:
            print("  overlap 0 bars")

    elif what == "shift":
        print("=" * 100)
        print("M4.4 - clock shift scan. A shift that recovers window 1's effect inside window 2")
        print("        would be a timezone or session-boundary explanation. +/-7 h is the OTL trap.")
        xd = load("XAUDUKA")
        for lab, a, b in (("gold window 1  2018-06 -> 2025-04", "2018-06-16", "2025-04-10"),
                          ("gold window 2  2010-06 -> 2018-06", "2010-06-01", "2018-06-15")):
            d = xd[(xd["date"] >= a) & (xd["date"] < b)].reset_index(drop=True)
            dw = d[d["wd"] <= 3].reset_index(drop=True)
            per_h = (d["open"].iloc[-1] - d["open"].iloc[0]) / (
                (d["ny"].iloc[-1] - d["ny"].iloc[0]).total_seconds() / 3600.0)
            print("\n  %s   (unconditional drift %+.5f $/oz per hour)" % (lab, per_h))
            print("   shift h | holds | raw $/oz per hold |     t | minus sample drift |     t")
            for s in range(-8, 9):
                h = window_holds(dw, 16 * 60 + 30, 18 * 60 + 15, shift_h=s)
                if h.empty:
                    print("   %+7d |     0 |" % s); continue
                raw = h["exit"] - h["entry"]
                ex = raw - per_h * h["hours"]
                print("   %+7d | %5d | %+17.4f | %+5.2f | %+18.4f | %+5.2f"
                      % (s, len(h), raw.mean(), tstat(raw), ex.mean(), tstat(ex)))

    elif what == "eushift":
        print("=" * 100)
        print("M4.4b - the same scan on the euro row, both windows. 1e-4 of price = 1 pip.")
        ed = load("EURDUKA")
        for lab, a, b in (("euro window 1  2010-06 -> 2018-06", "2010-06-01", "2018-06-15"),
                          ("euro window 2  2018-06 -> 2026-05", "2018-06-16", "2026-05-31")):
            d = ed[(ed["date"] >= a) & (ed["date"] < b)].reset_index(drop=True)
            dw = d[d["wd"] <= 4].reset_index(drop=True)
            print("\n  %s" % lab)
            print("   shift h | holds | SHORT excess pips/hold (window minus 8/24 of its own day) |     t")
            for s in range(-8, 9):
                h = window_holds(dw, 3 * 60, 11 * 60, shift_h=s)
                if h.empty:
                    print("   %+7d |     0 |" % s); continue
                raw = (h["exit"] - h["entry"]) * 1e4
                day = d.groupby("date")["open"].agg(["first", "last"])
                dm = h["date"].map(lambda x: (day["last"].get(x, np.nan) - day["first"].get(x, np.nan)) * 1e4)
                ex = raw - dm * (8.0 / 24.0)
                ex = ex.dropna()
                print("   %+7d | %5d | %+55.3f | %+5.2f" % (s, len(h), -ex.mean(), -tstat(ex)))

    elif what == "drift":
        print("=" * 100)
        print("M5.0 - gold's unconditional drift, restated in ATR20 per 5 sessions")
        for sym, a, b in (("XAUUSD", "2025-09-13", "2026-09-12"),
                          ("XAUDUKA", "2018-06-16", "2025-04-10"),
                          ("XAUDUKA", "2010-06-01", "2018-06-15")):
            d = load(sym)
            d = d[(d["date"] >= a) & (d["date"] < b)]
            day = d.groupby("date").agg(o=("open", "first"), h=("high", "max"),
                                        l=("low", "min"), c=("close", "last"))
            tr = pd.concat([day["h"] - day["l"], (day["h"] - day["c"].shift()).abs(),
                            (day["l"] - day["c"].shift()).abs()], axis=1).max(axis=1)
            atr20 = tr.rolling(20).mean()
            fwd = day["c"].shift(-5) - day["c"]
            x = (fwd / atr20).dropna()
            print("  %-8s %s -> %s  %4d sessions  mean %+.4f ATR20 per 5 sessions, t %+.2f"
                  % (sym, a, b, len(x), x.mean(), tstat(x)))

    elif what == "corrupt":
        import json, os, glob, collections
        print("=" * 100)
        print("M4.5 - corrupt trade records in the shared paper engine")
        tot = bad = lab = 0
        per = collections.Counter()
        times = []
        badtimes = []
        for p in sorted(glob.glob(ROOT + "/data/paper/*/trades.jsonl")):
            book = os.path.basename(os.path.dirname(p))
            for line in open(p, encoding="utf-8", errors="replace"):
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                except Exception:
                    continue
                tot += 1
                t = r.get("exit_time") or r.get("entry_time") or r.get("time")
                if t:
                    times.append(str(t)[:10])
                hm = r.get("hold_ms")
                if hm is not None and hm < 0:
                    bad += 1
                    per[book] += 1
                    if t:
                        badtimes.append(str(t)[:10])
                    rr = r.get("r")
                    reason = str(r.get("exit_reason") or r.get("label") or r.get("reason") or "")
                    if rr is not None and reason.upper().startswith("STOP") and rr > 0:
                        lab += 1
        print("  trade records read: %d" % tot)
        print("  records with hold_ms < 0: %d (%.2f%% of all records)" % (bad, 100.0 * bad / max(tot, 1)))
        print("  of those, label STOP with r > 0 (label contradicts its own R): %d" % lab)
        print("  by book: %s" % dict(per))
        if times:
            print("  all paper records span %s -> %s" % (min(times), max(times)))
        if badtimes:
            print("  corrupt records span  %s -> %s" % (min(badtimes), max(badtimes)))
        print("  the four claims' windows are 2010-06 -> 2026-09 BACKTEST receipts in")
        print("  docs/research/runs/, produced by `search`, not by the paper engine.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
