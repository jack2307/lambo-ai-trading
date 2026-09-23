"""Conditional structure of 15-minute gold: where price behaviour measurably differs.

Measurement only. Nothing here decides a trade, runs a method or reads a
profit factor; every column is a property of the bars. It exists so that a
method can be written from a measured conditional difference rather than from
a guess, and so that the difference can be read with its own count beside it.

**Data root.** Every invocation prints the root it read and the first and last
bar it saw. This program's design window physically ends 2025-09-23 and the
root is `data-sealed`; a receipt that names any other root is a deviation.

**Causality.** Every state variable is computed from bars at index <= t. The
forward columns are the thing being predicted and are never fed back into a
state. Thresholds on the state are absolute and scale-free (a return divided
by a trailing average true range), not quantiles of the whole window, so that
a rule written from this table does not need a number the future supplied.

Overlapping forward windows inflate a plain t-statistic by up to sqrt(h), so
every t is Newey-West with lag h.

Usage:
    python py/research/conditional_structure.py hours   --symbol=XAUDUKA-15m
    python py/research/conditional_structure.py zsize   --symbol=XAUDUKA-15m
    python py/research/conditional_structure.py cross   --symbol=XAUDUKA-15m
    python py/research/conditional_structure.py eras    --symbol=XAUDUKA-15m
"""

from __future__ import annotations

import sys

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data-sealed"
SPREAD = 0.28  # points per round trip, the configured Vantage cost


def arg(name: str, fallback: str) -> str:
    for a in sys.argv[1:]:
        if a.startswith(f"--{name}="):
            return a.split("=", 1)[1]
    return fallback


def load(symbol: str) -> pd.DataFrame:
    path = f"{DATA}/bars/{symbol}.parquet"
    df = pq.read_table(path).to_pandas()
    df = df.sort_values("time").reset_index(drop=True)
    print(f"data root: {DATA}")
    print(f"bars:      {symbol} {len(df)} from {df.time.iloc[0]} to {df.time.iloc[-1]}")
    return df


def features(df: pd.DataFrame, atr_period: int = 96) -> pd.DataFrame:
    """State at bar t, from bars <= t only; forward columns for measurement.

    `atr` here is the 96-bar mean true range ENDING AT t, so it includes bar t's
    own range. Bar t is complete when a decision is taken at its close, so this
    is causal — but it is a different normaliser from the one the decisive
    scripts use (`thinness_reversion.py` and after shift it back by one bar), and
    the two must not be compared to the last decimal.
    """
    c = df.close.to_numpy()
    h = df.high.to_numpy()
    l = df.low.to_numpy()
    prev_c = np.concatenate([[np.nan], c[:-1]])
    tr = np.maximum(h - l, np.maximum(np.abs(h - prev_c), np.abs(l - prev_c)))
    atr = pd.Series(tr).rolling(atr_period).mean().to_numpy()  # ends at t
    r = c - prev_c
    out = pd.DataFrame(
        {
            "time": df.time,
            "close": c,
            "r": r,
            "atr": atr,
            "z": r / atr,
            "hour": df.time.dt.hour,
            "dow": df.time.dt.dayofweek,
        }
    )
    for k in (1, 2, 4, 8, 16):
        out[f"f{k}"] = np.concatenate([c[k:], [np.nan] * k]) - c
    # Forward excursion in the direction of the last move, and against it,
    # over the next 8 bars: what a stop and a target would have met.
    for k in (4, 8):
        fav = np.full(len(c), np.nan)
        adv = np.full(len(c), np.nan)
        for i in range(len(c) - k):
            hi = h[i + 1 : i + 1 + k].max()
            lo = l[i + 1 : i + 1 + k].min()
            s = np.sign(r[i]) if np.isfinite(r[i]) else 0.0
            if s > 0:
                fav[i], adv[i] = hi - c[i], c[i] - lo
            elif s < 0:
                fav[i], adv[i] = c[i] - lo, hi - c[i]
        out[f"mfe{k}"] = fav
        out[f"mae{k}"] = adv
    return out


def nw_t(x: np.ndarray, lag: int) -> float:
    """Newey-West t-statistic of the mean of x with `lag` lags."""
    x = x[np.isfinite(x)]
    n = len(x)
    if n < 30:
        return float("nan")
    d = x - x.mean()
    g0 = (d @ d) / n
    var = g0
    for k in range(1, lag + 1):
        if k >= n:
            break
        gk = (d[k:] @ d[:-k]) / n
        var += 2.0 * (1.0 - k / (lag + 1.0)) * gk
    if var <= 0:
        return float("nan")
    return x.mean() / np.sqrt(var / n)


def cont_row(sub: pd.DataFrame, h: int) -> dict:
    """Continuation of the last move over the next h bars, in points."""
    s = np.sign(sub.r.to_numpy())
    f = sub[f"f{h}"].to_numpy()
    c = s * f
    ok = np.isfinite(c) & (s != 0)
    c = c[ok]
    return {
        "n": int(len(c)),
        "mean_pts": float(np.mean(c)) if len(c) else float("nan"),
        "t": nw_t(c, h),
        "absf_pts": float(np.mean(np.abs(f[np.isfinite(f)]))) if np.isfinite(f).any() else float("nan"),
    }


def hours(df: pd.DataFrame) -> None:
    f = features(df)
    print()
    print("== range and continuation by hour of day (UTC) ==")
    print("absf = mean |close[t+h]-close[t]|, points.  cont = mean sign(r_t)*(close[t+h]-close[t]), points.")
    print(f"{'h':>3} {'bars':>7} {'atr96':>8} {'absf4':>8} {'cont1':>9} {'t1':>7} {'cont4':>9} {'t4':>7} {'cont8':>9} {'t8':>7}")
    for hr in range(24):
        sub = f[f.hour == hr]
        tr = float(sub.atr.mean())
        r1, r4, r8 = cont_row(sub, 1), cont_row(sub, 4), cont_row(sub, 8)
        print(
            f"{hr:>3} {len(sub):>7} {tr:>8.3f} {r4['absf_pts']:>8.3f} "
            f"{r1['mean_pts']:>9.4f} {r1['t']:>7.2f} {r4['mean_pts']:>9.4f} {r4['t']:>7.2f} {r8['mean_pts']:>9.4f} {r8['t']:>7.2f}"
        )
    print()
    print("== mean true range of the bar itself, by hour (points) — the cost denominator ==")
    g = f.groupby("hour")[["r"]].apply(lambda s: pd.Series({"n": len(s), "meanAbsR": float(s.r.abs().mean())}))
    print(g.to_string())


def zsize(df: pd.DataFrame) -> None:
    f = features(df)
    edges = [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 1e9]
    print()
    print("== continuation conditional on the size of the last bar, |z| = |r_t| / ATR96 ==")
    print("Absolute scale-free thresholds, fixed before looking. mfe/mae over the next 8 bars, points.")
    print(f"{'bucket':>12} {'n':>8} {'cont1':>9} {'t1':>7} {'cont4':>9} {'t4':>7} {'cont8':>9} {'t8':>7} {'mfe8':>8} {'mae8':>8}")
    az = f.z.abs()
    for a, b in zip(edges[:-1], edges[1:]):
        sub = f[(az >= a) & (az < b)]
        if len(sub) < 50:
            continue
        r1, r4, r8 = cont_row(sub, 1), cont_row(sub, 4), cont_row(sub, 8)
        label = f"{a:g}-{b:g}" if b < 1e8 else f">{a:g}"
        print(
            f"{label:>12} {len(sub):>8} {r1['mean_pts']:>9.4f} {r1['t']:>7.2f} "
            f"{r4['mean_pts']:>9.4f} {r4['t']:>7.2f} {r8['mean_pts']:>9.4f} {r8['t']:>7.2f} "
            f"{float(sub.mfe8.mean()):>8.3f} {float(sub.mae8.mean()):>8.3f}"
        )


def cross(df: pd.DataFrame) -> None:
    f = features(df)
    az = f.z.abs()
    big = az >= 2.0
    print()
    print("== the same, split by hour: continuation after a bar of |z| >= 2 ==")
    print(f"{'h':>3} {'n':>7} {'cont4':>9} {'t4':>7} {'cont8':>9} {'t8':>7} {'mfe8':>8} {'mae8':>8} {'atr':>8}")
    for hr in range(24):
        sub = f[big & (f.hour == hr)]
        if len(sub) < 50:
            print(f"{hr:>3} {len(sub):>7}   (too few)")
            continue
        r4, r8 = cont_row(sub, 4), cont_row(sub, 8)
        print(
            f"{hr:>3} {len(sub):>7} {r4['mean_pts']:>9.4f} {r4['t']:>7.2f} {r8['mean_pts']:>9.4f} {r8['t']:>7.2f} "
            f"{float(sub.mfe8.mean()):>8.3f} {float(sub.mae8.mean()):>8.3f} {float(sub.atr.mean()):>8.3f}"
        )
    print()
    print("== quiet bars (|z| < 0.5) by hour: continuation ==")
    quiet = az < 0.5
    print(f"{'h':>3} {'n':>7} {'cont4':>9} {'t4':>7} {'cont8':>9} {'t8':>7}")
    for hr in range(24):
        sub = f[quiet & (f.hour == hr)]
        if len(sub) < 50:
            continue
        r4, r8 = cont_row(sub, 4), cont_row(sub, 8)
        print(f"{hr:>3} {len(sub):>7} {r4['mean_pts']:>9.4f} {r4['t']:>7.2f} {r8['mean_pts']:>9.4f} {r8['t']:>7.2f}")


ERAS = [("2010-06", "2015-06"), ("2015-06", "2020-06"), ("2020-06", "2025-09-23")]


def eras(df: pd.DataFrame) -> None:
    f = features(df)
    az = f.z.abs()
    print()
    print("== stability: the same conditional cells over three disjoint multi-year eras ==")
    print("A cell that only exists in one era is the shape the 2026-09-14 synthesis recorded.")
    cells = {
        "|z|>=2 all hours": az >= 2.0,
        "|z|>=2 h12-16": (az >= 2.0) & f.hour.between(12, 16),
        "|z|>=3 all hours": az >= 3.0,
        "|z|<0.5 all hours": az < 0.5,
        "|z|<0.5 h0-6": (az < 0.5) & f.hour.between(0, 6),
    }
    for name, mask in cells.items():
        print()
        print(f"-- {name}")
        print(f"{'era':>18} {'n':>8} {'cont4':>9} {'t4':>7} {'cont8':>9} {'t8':>7}")
        for a, b in ERAS:
            sub = f[mask & (f.time >= a) & (f.time < b)]
            if len(sub) < 50:
                print(f"{a+'..'+b:>18} {len(sub):>8}   (too few)")
                continue
            r4, r8 = cont_row(sub, 4), cont_row(sub, 8)
            print(f"{a+'..'+b:>18} {len(sub):>8} {r4['mean_pts']:>9.4f} {r4['t']:>7.2f} {r8['mean_pts']:>9.4f} {r8['t']:>7.2f}")


def eff_ratio(df: pd.DataFrame, h: int) -> pd.DataFrame:
    """Path efficiency over the next h bars: |net move| / total travel.

    A random walk has E|f_h| / E[sum |r|] = 1/sqrt(h) exactly (Gaussian
    increments: E|f_h| = sigma*sqrt(2h/pi), E|r| = sigma*sqrt(2/pi)). Above
    that line the next h bars are more directional than chance; below it they
    oscillate. This is a second moment of the path and carries no sign, so it
    is estimated far more precisely per observation than a conditional mean —
    and a method that enters on the break itself needs no sign from the table.
    """
    c = df.close.to_numpy()
    r = np.abs(df.r.to_numpy())
    n = len(c)
    net = np.full(n, np.nan)
    trav = np.full(n, np.nan)
    net[: n - h] = np.abs(c[h:] - c[: n - h])
    # travel over bars t+1..t+h
    cs = np.concatenate([[0.0], np.nan_to_num(r).cumsum()])
    trav[: n - h] = cs[1 + h :] - cs[1 : n - h + 1]
    out = df.copy()
    out["net"] = net
    out["trav"] = trav
    return out


def effratio(df: pd.DataFrame) -> None:
    f = features(df)
    print()
    print("== path efficiency by hour of day: |net move over h bars| / total travel over those bars ==")
    print("A random walk sits at 1/sqrt(h) exactly: 0.5000 at h=4, 0.3536 at h=8, 0.2500 at h=16.")
    for h in (4, 8, 16):
        g = eff_ratio(f, h)
        print()
        print(f"-- h = {h} bars ({15*h} minutes), random-walk line {1/np.sqrt(h):.4f}")
        print(f"{'hourUTC':>8} {'n':>7} {'E|net|':>8} {'E[trav]':>9} {'effRatio':>9} {'vs walk':>9} {'netPts/0.28':>12}")
        for hr in range(24):
            sub = g[(g.hour == hr) & np.isfinite(g.net) & np.isfinite(g.trav) & (g.trav > 0)]
            if len(sub) < 200:
                continue
            en, et = float(sub.net.mean()), float(sub.trav.mean())
            er = en / et
            print(f"{hr:>8} {len(sub):>7} {en:>8.3f} {et:>9.3f} {er:>9.4f} {er*np.sqrt(h):>9.4f} {en/SPREAD:>12.2f}")


def effstate(df: pd.DataFrame) -> None:
    """Path efficiency conditioned on the state of the bars before t."""
    f = features(df)
    az = f.z.abs()
    # compression: travel over the last k bars divided by the range covered
    c = f.close.to_numpy()
    r = np.abs(f.r.to_numpy())
    n = len(c)
    for k in (8, 16):
        cs = np.concatenate([[0.0], np.nan_to_num(r).cumsum()])
        past_trav = np.full(n, np.nan)
        past_net = np.full(n, np.nan)
        past_trav[k:] = cs[k + 1 :] - cs[1 : n - k + 1]
        past_net[k:] = np.abs(c[k:] - c[: n - k])
        f[f"pastER{k}"] = past_net / past_trav
        f[f"pastTrav{k}"] = past_trav
    print()
    print("== forward path efficiency conditional on the PAST path's efficiency (h=8 forward, k=8 past) ==")
    print("Both computed from bars <= t for the state and > t for the forward leg. Walk line 0.3536.")
    g = eff_ratio(f, 8)
    ok = np.isfinite(g.net) & np.isfinite(g.trav) & (g.trav > 0) & np.isfinite(g.pastER8)
    edges = [0.0, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 1.01]
    print(f"{'pastER8':>12} {'n':>8} {'E|net|':>8} {'E[trav]':>9} {'fwdER':>8} {'vs walk':>9}")
    for a, b in zip(edges[:-1], edges[1:]):
        sub = g[ok & (g.pastER8 >= a) & (g.pastER8 < b)]
        if len(sub) < 200:
            continue
        en, et = float(sub.net.mean()), float(sub.trav.mean())
        print(f"{a:.2f}-{b:.2f}".rjust(12) + f" {len(sub):>8} {en:>8.3f} {et:>9.3f} {en/et:>8.4f} {en/et*np.sqrt(8):>9.4f}")
    print()
    print("== forward path efficiency conditional on the last bar's size |z| ==")
    print(f"{'|z|':>12} {'n':>8} {'E|net|':>8} {'E[trav]':>9} {'fwdER':>8} {'vs walk':>9}")
    zedges = [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 1e9]
    for a, b in zip(zedges[:-1], zedges[1:]):
        sub = g[ok & (az >= a) & (az < b)]
        if len(sub) < 200:
            continue
        en, et = float(sub.net.mean()), float(sub.trav.mean())
        label = f"{a:g}-{b:g}" if b < 1e8 else f">{a:g}"
        print(f"{label:>12} {len(sub):>8} {en:>8.3f} {et:>9.3f} {en/et:>8.4f} {en/et*np.sqrt(8):>9.4f}")


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "hours"
    df = load(arg("symbol", "XAUDUKA-15m"))
    lo, hi = arg("from", ""), arg("to", "")
    if lo:
        df = df[df.time >= lo]
    if hi:
        df = df[df.time < hi]
    if lo or hi:
        print(f"window:    {len(df)} bars, {df.time.iloc[0]} to {df.time.iloc[-1]}")
    df = df.reset_index(drop=True)
    print(f"spread:    {SPREAD} points per round trip (configured)")
    {"hours": hours, "zsize": zsize, "cross": cross, "eras": eras, "effratio": effratio, "effstate": effstate}[mode](df)


if __name__ == "__main__":
    main()
