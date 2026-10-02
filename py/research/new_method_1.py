"""Measures the three mechanisms registered in docs/hypotheses/2026-10-02-new-method-1.md.

Nothing here reads gold's own price path to decide WHETHER to trade. Method A and B are
triggered by rows of the published release calendar; method C by the week's opening hour.
The price path is read only to measure what happened afterwards, and (for A and C) to take
the sign of a move whose timing the calendar or the clock already fixed.

HARD RULE asserted below: no bar at or after 2025-09-23 is read, from any file. The desk's
only unspent hold-out lives after that instant and spending it is the owner's decision.

Units: every mean is reported twice, in ATR20d (the daily ATR20 at entry, which is R for a
one-daily-ATR stop) and in price units per ounce. Cost is 0.28 price units round trip,
half on entry and half on exit, per config/default.toml [markets.xauusd] spread.
"""
import sys
import numpy as np
import pandas as pd

CUT = pd.Timestamp("2025-09-23", tz="UTC")   # nothing at or after this is read, ever
COST = 0.28                                  # price units per ounce, round trip


def load_bars(path):
    df = pd.read_parquet(path, columns=["time", "open", "high", "low", "close"])
    # The store writes microsecond timestamps. Timestamp.value is always NANOseconds, so
    # a us-resolution int64 and a ns-resolution int64 compared against each other put every
    # search past the end of the array and every method returned zero trades. Force ns here.
    df["time"] = pd.to_datetime(df["time"], utc=True).astype("datetime64[ns, UTC]")
    df = df.sort_values("time").reset_index(drop=True)
    before = len(df)
    df = df[df["time"] < CUT].reset_index(drop=True)
    assert len(df) == 0 or df["time"].iloc[-1] < CUT, "hold-out cut failed"
    sys.stderr.write("  %s: %d bars kept, %d dropped at the 2025-09-23 cut, %s -> %s\n"
                     % (path, len(df), before - len(df),
                        df["time"].iloc[0], df["time"].iloc[-1]))
    return df


def sessionise(df):
    """Session = a distinct UTC date present in the store. Returns session index per bar,
    the first-bar row index of each session, and daily ATR20 keyed by session index.

    ATR20d at session k uses only sessions k-20..k-1, so it is known at the open of k."""
    d = df["time"].dt.date
    codes, uniq = pd.factorize(d)                       # already sorted
    df = df.assign(sess=codes)
    first = df.groupby("sess").head(1).index.to_numpy()
    agg = df.groupby("sess").agg(h=("high", "max"), l=("low", "min"), c=("close", "last"))
    pc = agg["c"].shift(1)
    tr = np.maximum(agg["h"] - agg["l"],
                    np.maximum((agg["h"] - pc).abs(), (pc - agg["l"]).abs()))
    atr = tr.rolling(20).mean().shift(1)                # previous 20 completed sessions
    return df, first, atr.to_numpy(), len(uniq)


def stats(moves, atrs, label, n_eras_sign=None):
    """moves: gross (exit-entry)*side in price units. atrs: ATR20d at entry."""
    moves = np.asarray(moves, float)
    atrs = np.asarray(atrs, float)
    ok = np.isfinite(moves) & np.isfinite(atrs) & (atrs > 0)
    moves, atrs = moves[ok], atrs[ok]
    n = len(moves)
    if n < 2:
        return dict(label=label, n=n, mean_atr=None, mean_px=None, t=None,
                    net_mean_atr=None, net_mean_px=None)
    r_gross = moves / atrs
    r_net = (moves - COST) / atrs
    sd = r_gross.std(ddof=1)
    return dict(label=label, n=n,
                mean_atr=r_gross.mean(), mean_px=moves.mean(),
                t=r_gross.mean() / (sd / np.sqrt(n)) if sd > 0 else None,
                net_mean_atr=r_net.mean(), net_mean_px=moves.mean() - COST,
                net_t=r_net.mean() / (r_net.std(ddof=1) / np.sqrt(n)) if sd > 0 else None,
                sd_atr=sd)


def fmt(s):
    def g(k, p=4):
        v = s.get(k)
        return "null" if v is None else ("%.*f" % (p, v))
    return ("%-46s n=%5d  gross %s ATR20d (%s px)  t=%s | net %s ATR20d (%s px) t=%s"
            % (s["label"], s["n"], g("mean_atr"), g("mean_px", 3), g("t", 2),
               g("net_mean_atr"), g("net_mean_px", 3), g("net_t", 2)))


# ---------------------------------------------------------------- the three mechanisms

def events(path="data/news/events-extended.csv", currency="USD", impact=3, names=None):
    ev = pd.read_csv(path)
    ev["time_utc"] = pd.to_datetime(ev["time_utc"], utc=True)
    ev = ev[(ev["currency"] == currency) & (ev["impact"] == impact) & (ev["time_utc"] < CUT)]
    if names is not None:
        ev = ev[ev["name"].isin(names)]
    return ev.sort_values("time_utc").reset_index(drop=True)


def nanos(series):
    """int64 NANOseconds. Defensive about resolution: see the note in load_bars."""
    s = series.astype("datetime64[ns, UTC]")
    out = s.astype("int64").to_numpy()
    assert out[0] > 1_000_000_000_000_000_000, "not nanoseconds: resolution mismatch"
    return out


def bar_at_or_after(tns, ts):
    i = int(np.searchsorted(tns, pd.Timestamp(ts).value, side="left"))
    return i if i < len(tns) else None


def as_trades(rows):
    tr = pd.DataFrame(rows, columns=["t_in", "t_out", "side", "raw", "atr"])
    if len(tr):
        tr["t_in"] = pd.to_datetime(tr["t_in"], utc=True)
        tr["t_out"] = pd.to_datetime(tr["t_out"], utc=True)
    return tr


def method_a(df, first, atr, nsess, ev, rel_bars=4, hold_sess=3):
    """Continuation of the release-window move, held hold_sess sessions."""
    times = nanos(df["time"])
    o, c = df["open"].to_numpy(), df["close"].to_numpy()
    sess = df["sess"].to_numpy()
    rows = []
    for ts in ev["time_utc"]:
        i = bar_at_or_after(times, ts.to_datetime64())
        if i is None or i + rel_bars >= len(df):
            continue
        move = c[i + rel_bars - 1] - o[i]
        if move == 0:
            continue
        side = 1 if move > 0 else -1
        e = i + rel_bars                       # fill at the open of the next bar
        if e >= len(df):
            continue
        xs = sess[e] + hold_sess
        if xs >= nsess:
            continue
        x = first[xs]
        a = atr[sess[e]]
        rows.append((times[e], times[x], side, (o[x] - o[e]) * side, a))
    return as_trades(rows)


def method_b_leg(df, first, atr, nsess, ev, lo_h, hi_h, side):
    """One unconditional leg: from lo_h hours relative to the event to hi_h hours.
    side -1 = short (the pre leg), +1 = long (the post leg). No conditioning on the news."""
    times = nanos(df["time"])
    o = df["open"].to_numpy()
    sess = df["sess"].to_numpy()
    rows = []
    for ts in ev["time_utc"]:
        a = bar_at_or_after(times, (ts + pd.Timedelta(hours=lo_h)).to_datetime64())
        b = bar_at_or_after(times, (ts + pd.Timedelta(hours=hi_h)).to_datetime64())
        if a is None or b is None or b <= a or b >= len(df):
            continue
        rows.append((times[a], times[b], side, (o[b] - o[a]) * side, atr[sess[a]]))
    return as_trades(rows)


def method_b_control(df, first, atr, nsess, ev, hours, side, hour_of_day_hist):
    """The drift control for a one-sided leg: every window of the same length starting at
    the same hour-of-day (and minute), on days carrying no impact-3 event."""
    times = nanos(df["time"])
    o = df["open"].to_numpy()
    sess = df["sess"].to_numpy()
    ev_days = set(ev["time_utc"].dt.date)
    tt = df["time"]
    mask = (~tt.dt.date.isin(ev_days)) & tt.dt.hour.isin(hour_of_day_hist)
    idx = np.flatnonzero(mask.to_numpy())
    span = int(round(hours * 4))              # 15m bars
    idx = idx[idx + span < len(df)]
    raw = (o[idx + span] - o[idx]) * side
    return as_trades(list(zip(times[idx], times[idx + span],
                              [side] * len(idx), raw, atr[sess[idx]])))


def method_c(df, first, atr, nsess, thresh=0.5, hold_sess=1, hold_bars=None):
    """Reversion of the weekly reopen gap."""
    times = nanos(df["time"])
    o, c = df["open"].to_numpy(), df["close"].to_numpy()
    sess = df["sess"].to_numpy()
    t = df["time"]
    gap_at = np.flatnonzero((t.diff() > pd.Timedelta(hours=12)).to_numpy())
    rows = []
    for i in gap_at:
        a = atr[sess[i]]
        if not np.isfinite(a) or a <= 0:
            continue
        gap = o[i] - c[i - 1]
        if abs(gap) < thresh * a:
            continue
        side = -1 if gap > 0 else 1            # fade the gap
        e = i + 1                              # fill at the open of the second bar
        if e >= len(df):
            continue
        if hold_bars is not None:
            x = e + hold_bars
        else:
            xs = sess[e] + hold_sess
            if xs >= nsess:
                continue
            x = first[xs]
        if x >= len(df):
            continue
        rows.append((times[e], times[x], side, (o[x] - o[e]) * side, a))
    return as_trades(rows)


def method_c_control_deep(df, first, atr, nsess, thresh=0.5, hold_sess=1):
    """C's mechanism falsifier: the same fade on a same-size move during the DEEP London
    hours. If this is as large, the overshoot is not a liquidity effect."""
    times = nanos(df["time"])
    o = df["open"].to_numpy()
    sess = df["sess"].to_numpy()
    tt = df["time"]
    # one-bar move of the same size, measured at 09:00-12:00 UTC (London, deep book)
    mask = tt.dt.hour.isin([9, 10, 11]).to_numpy()
    idx = np.flatnonzero(mask)
    idx = idx[(idx > 0) & (idx + 2 < len(df))]
    rows = []
    for i in idx:
        a = atr[sess[i]]
        if not np.isfinite(a) or a <= 0:
            continue
        mv = o[i] - o[i - 1]
        if abs(mv) < thresh * a:
            continue
        side = -1 if mv > 0 else 1
        e = i + 1
        xs = sess[e] + hold_sess
        if xs >= nsess:
            continue
        x = first[xs]
        if x >= len(df):
            continue
        rows.append((times[e], times[x], side, (o[x] - o[e]) * side, a))
    return as_trades(rows)


# ---------------------------------------------------------------- reporting helpers

def report(tr, label, eras=None):
    """Prints the cell, its long/short split, the flipped cell, the hold<0 count, and the
    sub-era signs. Every number carries its unit."""
    if len(tr) == 0:
        print("%-46s n=    0  null (no trade)" % label)
        return None
    neg = int((tr["t_out"] <= tr["t_in"]).sum())
    s = stats(tr["raw"], tr["atr"], label)
    print(fmt(s))
    nl = int((tr["side"] > 0).sum())
    print("      long/short split: %d long / %d short (%.1f%% long)   hold<=0 trades: %d"
          % (nl, len(tr) - nl, 100.0 * nl / len(tr), neg))
    f = stats(-tr["raw"], tr["atr"], "  FLIPPED " + label)
    print(fmt(f))
    if neg:
        k = tr[tr["t_out"] > tr["t_in"]]
        print(fmt(stats(k["raw"], k["atr"], "  excl hold<=0 " + label)))
    if eras is not None:
        parts = []
        for (lo, hi) in eras:
            m = tr[(tr["t_in"] >= lo) & (tr["t_in"] < hi)]
            if len(m) < 2:
                parts.append("%s:null(n=%d)" % (lo.date(), len(m)))
            else:
                r = ((m["raw"] - COST) / m["atr"]).mean()
                parts.append("%s:%+.4f(n=%d)" % (lo.date(), r, len(m)))
        print("      net mean ATR20d by sub-era: " + "  ".join(parts))
    return s


def gate(s, tr, eras, name):
    """The three registered legs, evaluated on the NET mean."""
    if s is None:
        print("  GATE %s: null -- no trades" % name)
        return False
    legs = []
    legs.append(("1 net mean >= 0.066 ATR20d",
                 s["net_mean_atr"] is not None and s["net_mean_atr"] >= 0.066,
                 "%.4f" % s["net_mean_atr"]))
    legs.append(("2 t >= 3.0", s["net_t"] is not None and s["net_t"] >= 3.0,
                 "%.2f" % s["net_t"]))
    signs = 0
    for (lo, hi) in eras:
        m = tr[(tr["t_in"] >= lo) & (tr["t_in"] < hi)]
        if len(m) >= 2 and ((m["raw"] - COST) / m["atr"]).mean() > 0:
            signs += 1
    legs.append(("3 sign in >=3 of 4 sub-eras", signs >= 3, "%d of 4" % signs))
    ok = all(p for _, p, _ in legs)
    print("  GATE %s: %s" % (name, "CLEARS ALL THREE" if ok else "FAILS"))
    for t, p, v in legs:
        print("    leg %s : %s  (%s)" % (t, "pass" if p else "FAIL", v))
    return ok
