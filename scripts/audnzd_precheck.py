"""The three pre-checks the registration demands, run before any gate cell.

They are deliberately computed here rather than read off a backtest receipt,
because each of them is a property of the SERIES and spending gate cells to
learn one would put the answer after the looks instead of before them.

  F2  cost/R = spread / (k x ATR14), median per window. F2 fires at >= 10.0%
      of R at k = 1.5 -- the level that would say this instrument is no
      cheaper than the silver leg (14.69%) and so says nothing about the
      family.

  (2) Variance ratio VR(h) and the AR(1) half-life. VR < 1 is mean reverting.
      The half-life sets the horizon, and `agent/n2` measured that a >= 40
      trade floor on a 3-month window forbids any horizon past about a day --
      which is why this registration uses four-year windows.

  (3) corr(z_t, ln P_{t+h} - ln P_t): the SIGN of the relationship. Negative
      is reversion, positive is continuation. F3 fires on a positive sign in
      both windows, because that is a rule standing in front of the train.

Every t-statistic here is computed on NON-OVERLAPPING samples. The desk's own
record notes that overlapping windows inflate t by about sqrt(overlap), and
the +6.74 drift figure it carried for weeks is suspected of exactly that. An
inflated t here would manufacture a premise.
"""

import datetime as dt
import math
import sys

import pyarrow.parquet as pq

BARS = "E:/rust/flowdesk/data/bars/AUDNZD-15m.parquet"
SPREAD = 0.00030          # QUOTE, not a measurement -- see config/default.toml
WINDOWS = {
    "A 2022-06..2026-05": (dt.datetime(2022, 6, 1), dt.datetime(2026, 5, 31)),
    "B 2018-06..2022-06": (dt.datetime(2018, 6, 1), dt.datetime(2022, 6, 1)),
}
LOOKBACKS = (96, 192, 480)
HORIZONS = (("1 bar", 1), ("1 day", 96), ("1 week", 480))


def load():
    t = pq.read_table(BARS, columns=["time", "high", "low", "close"])
    time = [x.as_py().replace(tzinfo=None) for x in t.column("time")]
    return (time, t.column("high").to_pylist(), t.column("low").to_pylist(),
            t.column("close").to_pylist())


def atr14(high, low, close):
    """Wilder, the same recursion the engine's `atr` uses."""
    n = len(close)
    out = [float("nan")] * n
    tr = [float("nan")] * n
    for i in range(1, n):
        tr[i] = max(high[i] - low[i], abs(high[i] - close[i - 1]), abs(low[i] - close[i - 1]))
    if n < 15:
        return out
    seed = sum(tr[1:15]) / 14.0
    out[14] = seed
    prev = seed
    for i in range(15, n):
        prev = (prev * 13.0 + tr[i]) / 14.0
        out[i] = prev
    return out


def median(xs):
    s = sorted(x for x in xs if x == x)
    if not s:
        return float("nan")
    m = len(s) // 2
    return s[m] if len(s) % 2 else (s[m - 1] + s[m]) / 2.0


def pearson(xs, ys):
    pairs = [(a, b) for a, b in zip(xs, ys) if a == a and b == b]
    n = len(pairs)
    if n < 3:
        return float("nan"), float("nan"), n
    mx = sum(a for a, _ in pairs) / n
    my = sum(b for _, b in pairs) / n
    sxy = sum((a - mx) * (b - my) for a, b in pairs)
    sxx = sum((a - mx) ** 2 for a, _ in pairs)
    syy = sum((b - my) ** 2 for _, b in pairs)
    if sxx <= 0 or syy <= 0:
        return float("nan"), float("nan"), n
    r = sxy / math.sqrt(sxx * syy)
    if abs(r) >= 1.0:
        return r, float("inf"), n
    t = r * math.sqrt((n - 2) / (1 - r * r))
    return r, t, n


def ln_z(close, period):
    """Same definition as the `lnz` indicator: NaN before a complete window."""
    n = len(close)
    z = [float("nan")] * n
    lc = [math.log(c) if c > 0 else float("nan") for c in close]
    for i in range(period - 1, n):
        w = lc[i + 1 - period:i + 1]
        if any(v != v for v in w):
            continue
        mean = sum(w) / period
        var = sum((v - mean) ** 2 for v in w) / (period - 1)
        sd = math.sqrt(var)
        if sd <= 0:
            continue
        z[i] = (lc[i] - mean) / sd
    return z


def main():
    time, high, low, close = load()
    atr = atr14(high, low, close)
    print("bars: %d  %s .. %s" % (len(close), time[0].date(), time[-1].date()))
    print("spread used: %.5f  (QUOTE from config, not a measurement)" % SPREAD)

    for label, (lo, hi) in WINDOWS.items():
        idx = [i for i, t in enumerate(time) if lo <= t < hi]
        if not idx:
            print("\n%s: no bars" % label)
            continue
        a, b = idx[0], idx[-1] + 1
        print("\n=== %s   %d bars" % (label, b - a))

        # ---- F2: cost/R by stop size -------------------------------------
        print("  F2  cost/R = spread / (k x ATR14), median:")
        for k in (1.5, 2.0, 3.0):
            stops = [k * atr[i] for i in range(a, b) if atr[i] == atr[i] and atr[i] > 0]
            med_stop = median(stops)
            pct = 100.0 * SPREAD / med_stop if med_stop == med_stop and med_stop > 0 else float("nan")
            print("        k=%.1f  stop %.6f  ->  %6.3f%% of R" % (k, med_stop, pct))
            if k == 1.5:
                print("              F2 %s (fires at >= 10.0%%)"
                      % ("FIRES" if pct >= 10.0 else "does not fire"))

        # ---- (2) variance ratio + half-life ------------------------------
        lc = [math.log(close[i]) for i in range(a, b)]
        r1 = [lc[i + 1] - lc[i] for i in range(len(lc) - 1)]
        v1 = sum(x * x for x in r1) / len(r1)
        print("  (2) variance ratio (1 = random walk, < 1 = mean reverting):")
        for h in (32, 96, 480):
            rh = [lc[i + h] - lc[i] for i in range(0, len(lc) - h, h)]   # NON-overlapping
            if len(rh) < 10:
                print("        VR(%3d): too few non-overlapping samples (%d)" % (h, len(rh)))
                continue
            vh = sum(x * x for x in rh) / len(rh)
            print("        VR(%3d) = %.3f   (n=%d non-overlapping)" % (h, vh / (h * v1), len(rh)))

        # AR(1) on the log level, non-overlapping in the sense that each bar
        # contributes one observation of a one-step change.
        xs = lc[:-1]
        ys = r1
        r, t, n = pearson(xs, ys)
        if r == r:
            mx = sum(xs) / len(xs)
            my = sum(ys) / len(ys)
            sxx = sum((v - mx) ** 2 for v in xs)
            beta = sum((v - mx) * (w - my) for v, w in zip(xs, ys)) / sxx
            lam = 1.0 + beta
            hl = -math.log(2) / math.log(lam) if 0 < lam < 1 else float("nan")
            print("        AR(1) beta = %+.6f  t = %+.2f  n=%d" % (beta, t, n))
            print("        half-life = %s bars%s"
                  % ("%.0f" % hl if hl == hl else "n/a (not mean reverting)",
                     "  = %.1f days" % (hl / 96.0) if hl == hl else ""))

        # ---- (3) F3: the SIGN --------------------------------------------
        print("  (3) corr(z, future log move) -- NEGATIVE = reversion, POSITIVE = continuation")
        for lb in LOOKBACKS:
            z = ln_z(close[a:b], lb)
            row = []
            for hname, h in HORIZONS:
                fwd = [(lc[i + h] - lc[i]) if i + h < len(lc) else float("nan")
                       for i in range(len(lc))]
                # non-overlapping: step by h so each future move is disjoint
                zs = [z[i] for i in range(0, len(z) - h, h)]
                fs = [fwd[i] for i in range(0, len(z) - h, h)]
                r, t, n = pearson(zs, fs)
                row.append("%s r=%+.3f t=%+.2f n=%d" % (hname, r, t, n))
            print("        lookback %3d:  %s" % (lb, "   |   ".join(row)))

    return 0


if __name__ == "__main__":
    sys.exit(main())
