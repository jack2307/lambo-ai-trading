"""Would deciding on one-minute bars give the desk a better entry?

    python scripts/entry_granularity.py

The AI fills at the NEXT 15m bar's OPEN. Deciding on 1m bars would let it fill
at any of the fifteen one-minute opens inside that window instead. This measures
the spread of those fills against the risk unit the desk sizes with (R), and
against the spread it pays.

The number that decides the question is not how WIDE that choice is. It is
whether the best and the worst are symmetric around the 15m open. If they are,
a finer bar does not move the expected fill at all — it only widens the
variance, and capturing the good half requires predicting which minute, which
is strictly harder than the direction problem the desk has not solved.
"""
from __future__ import annotations

import datetime as dt
import pyarrow.parquet as pq

SPREAD = 0.22   # measured p50 of XAUUSD.sc, 2,315+ samples
STOP_ATR = 1.2  # [trading] stop_atr: R is this many ATR


def load(tf: str):
    t = pq.read_table(f"data/bars/XAUUSD-{tf}.parquet")
    c = {n: t.column(n).to_pylist() for n in ("time", "open", "high", "low", "close")}
    ms = [int(x.timestamp() * 1000) if hasattr(x, "timestamp") else int(x) for x in c["time"]]
    return list(zip(ms, c["open"], c["high"], c["low"], c["close"]))


def pct(xs, q):
    xs = sorted(xs)
    k = (len(xs) - 1) * q
    i = int(k)
    return xs[i] + (xs[min(i + 1, len(xs) - 1)] - xs[i]) * (k - i)


def main() -> int:
    m1, m15 = load("1m"), load("15m")
    lo, hi = m1[0][0], m1[-1][0]
    m15 = [b for b in m15 if lo <= b[0] <= hi]
    print("overlap: %s -> %s (%d 15m bars)" % (
        dt.datetime.utcfromtimestamp(m15[0][0] / 1000).strftime("%Y-%m-%d"),
        dt.datetime.utcfromtimestamp(m15[-1][0] / 1000).strftime("%Y-%m-%d"), len(m15)))

    trs, atr, prev = [], {}, None
    for t, o, h, l, c in m15:
        trs.append((h - l) if prev is None else max(h - l, abs(h - prev), abs(l - prev)))
        prev = c
        if len(trs) >= 14:
            atr[t] = sum(trs[-14:]) / 14.0

    buckets: dict[int, list] = {}
    for t, o, *_ in m1:
        buckets.setdefault(t - (t % 900_000), []).append(o)

    rows = []
    for t, o15, *_ in m15:
        a, opens = atr.get(t), buckets.get(t)
        if not a or not opens or len(opens) < 10:
            continue
        r = STOP_ATR * a
        if r <= 0:
            continue
        rows.append({
            "R": r,
            "spread_R": SPREAD / r,
            "range_R": (max(opens) - min(opens)) / r,
            "best_R": (o15 - min(opens)) / r,
            "worst_R": (max(opens) - o15) / r,
        })

    print("%d bars scored\n" % len(rows))
    print("%-44s %8s %8s %8s" % ("", "p50", "p90", "p99"))
    for label, key in (("spread / R (the whole round-trip cost)", "spread_R"),
                       ("width of the 1m entry choice / R", "range_R"),
                       ("BEST 1m open vs the 15m open / R", "best_R"),
                       ("WORST 1m open vs the 15m open / R", "worst_R")):
        v = [r[key] for r in rows]
        print("%-44s %8.3f %8.3f %8.3f" % (label, pct(v, .5), pct(v, .9), pct(v, .99)))

    best, worst = pct([r["best_R"] for r in rows], .5), pct([r["worst_R"] for r in rows], .5)
    print("\nbest %+.3fR vs worst %-+.3fR  -> asymmetry %+.4fR" % (best, -worst, best - worst))
    print("A finer bar is worth having only if that asymmetry is positive and large.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
