"""Would a limit entry beat the market entry the desk uses now?

    python scripts/limit_entry.py

The desk fills at the OPEN of the bar after the decision. A limit sits below
(for a long) and fills only if price comes back to it.

This is NOT the question `entry_granularity.py` answered. That one showed a
RANDOM 1m entry has the same expected fill as the open, because best and worst
are symmetric. A limit is CONDITIONAL, so its distribution is not symmetric —
and it has a cost the random entry does not: the trades it never gets into.

Adverse selection is the whole question. A limit fills on the moves that come
back to it and misses the ones that run, and the ones that run are the winners.

Both arms are measured over the same horizon from the same signal, so the
period's drift lands on both and cancels. Run for LONG and for SHORT, because
one direction alone in a trending window measures the trend, not the method.
"""
from __future__ import annotations

import datetime as dt

import pyarrow.parquet as pq

STOP_ATR = 1.2      # [trading] stop_atr — R is this many ATR
HORIZON_BARS = 8    # two hours; the desk's maximum hold is four
OFFSETS_R = [0.1, 0.2, 0.3, 0.5]
WAIT_MIN = [5, 15, 30]


def load(tf: str):
    t = pq.read_table(f"data/bars/XAUUSD-{tf}.parquet")
    c = {n: t.column(n).to_pylist() for n in ("time", "open", "high", "low", "close")}
    ms = [int(x.timestamp() * 1000) if hasattr(x, "timestamp") else int(x) for x in c["time"]]
    return list(zip(ms, c["open"], c["high"], c["low"], c["close"]))


def main() -> int:
    m1, m15 = load("1m"), load("15m")
    lo, hi = m1[0][0], m1[-1][0]
    m15 = [b for b in m15 if lo <= b[0] <= hi]
    by_min = {t: (t, o, h, l, c) for t, o, h, l, c in m1}

    trs, atr, prev = [], {}, None
    for t, o, h, l, c in m15:
        trs.append((h - l) if prev is None else max(h - l, abs(h - prev), abs(l - prev)))
        prev = c
        if len(trs) >= 14:
            atr[t] = sum(trs[-14:]) / 14.0

    print("XAUUSD.sc - %s to %s, %d bars of 15m against 1m\n" % (
        dt.datetime.utcfromtimestamp(m15[0][0] / 1000).strftime("%Y-%m-%d"),
        dt.datetime.utcfromtimestamp(m15[-1][0] / 1000).strftime("%Y-%m-%d"), len(m15)))
    print("`market` is what the desk does today: always in, at the next open.")
    print("`limit`  waits for a better price and takes NO trade if it never comes.\n")

    for side in ("LONG", "SHORT"):
        dirn = 1.0 if side == "LONG" else -1.0
        print("--- every bar treated as a %s signal ---" % side)
        print("%-8s %-6s %8s %12s %12s %14s" % (
            "offset", "wait", "fill %", "market", "limit", "limit - market"))
        print("%-8s %-6s %8s %12s %12s %14s" % ("(R)", "(min)", "", "R/signal", "R/signal", "R/signal"))
        for off in OFFSETS_R:
            for wait in WAIT_MIN:
                filled = signals = 0
                pnl_market = pnl_limit = 0.0
                for k in range(len(m15) - HORIZON_BARS - 1):
                    a = atr.get(m15[k][0])
                    if not a:
                        continue
                    r = STOP_ATR * a
                    if r <= 0:
                        continue
                    nxt = m15[k + 1]
                    market = nxt[1]
                    limit = market - dirn * off * r
                    later = m15[k + 1 + HORIZON_BARS][4]
                    signals += 1
                    pnl_market += dirn * (later - market) / r

                    hit = False
                    for m in range(wait):
                        bar = by_min.get(nxt[0] + m * 60_000)
                        if not bar:
                            continue
                        if (dirn > 0 and bar[3] <= limit) or (dirn < 0 and bar[2] >= limit):
                            hit = True
                            break
                    if hit:
                        filled += 1
                        pnl_limit += dirn * (later - limit) / r

                if not signals:
                    continue
                pm, pl = pnl_market / signals, pnl_limit / signals
                print("%-8.2f %-6d %7.1f%% %+12.4f %+12.4f %+14.4f" % (
                    off, wait, 100.0 * filled / signals, pm, pl, pl - pm))
        print()

    print("Both arms run over the same horizon from the same signal, so the period's")
    print("drift lands on both and cancels. `limit - market` positive means the better")
    print("price on the fills paid for the trades that got away.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
