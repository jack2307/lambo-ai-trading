"""The ONE thing a 15m fill verdict cannot settle, measured on 1m.

Registered in docs/decisions/2026-10-06-limit-entry.md as a descriptive audit,
never a gate.

A long limit rests BELOW the signal bar's close and its stop sits below that
again, so price cannot reach the stop without passing the order first: the
engine's stop-before-target reading of the fill bar is not pessimism there, it
is arithmetic. The TARGET is the real exposure. It sits ABOVE the order, so a
15m bar can print a high that reached it BEFORE the order ever filled — and
`check_exit` tests the whole bar's high against a position it opened mid-bar.
That is a win the sequence never allowed, and it flatters the limit arm.

This counts them. For every 15m bar in the window, the previous bar is treated
as a signal (both sides, separately), an order is rested at `offset x ATR14`,
the engine's own fallback stop (1.2 ATR) and target (1.8 R) are derived from
the fill, and the fill bar is then judged twice: once from its 15m OHLC the way
the engine judges it, and once by walking its 1m bars in order from the minute
the order actually filled.

Usage:  python n3_fillbar_1m_audit.py <from> <to>
"""
from __future__ import annotations

import datetime as dt
import sys

import pyarrow.compute as pc
import pyarrow.parquet as pq

BARS = "E:/rust/flowdesk/data/bars"
SYMBOL = "XAUDUKA"          # the only gold tape with 1m over these windows
ATR_PERIOD = 14
STOP_ATR = float(sys.argv[3]) if len(sys.argv) > 3 else 1.2   # [trading] stop_atr, or argv[3]
REWARD_RISK = 1.8           # [trading] reward_risk
SPREAD = 0.28               # [markets.xauusd.trading] spread
OFFSETS = [0.0, 0.25, 0.5]
TTL = 4


def ms(day: str) -> int:
    d = dt.datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=dt.timezone.utc)
    return int(d.timestamp() * 1000)


def load(tf: str, lo: int, hi: int):
    t = pq.read_table(f"{BARS}/{SYMBOL}-{tf}.parquet", columns=["time", "open", "high", "low", "close"])
    stamps = t.column("time")
    # Stored as a timestamp; compare in the same unit by going via epoch ms.
    epoch = pc.multiply(pc.divide(pc.cast(stamps, "int64"), 1000), 1)  # us -> ms when us
    unit = stamps.type.unit if hasattr(stamps.type, "unit") else "us"
    div = {"s": 1e-3, "ms": 1.0, "us": 1e3, "ns": 1e6}[unit]
    epoch = [int(int(v) / div) for v in pc.cast(stamps, "int64").to_pylist()]
    o, h, l, c = (t.column(k).to_pylist() for k in ("open", "high", "low", "close"))
    return [(epoch[i], o[i], h[i], l[i], c[i]) for i in range(len(epoch)) if lo - 40 * 86_400_000 <= epoch[i] < hi]


def atr14(bars):
    out, trs, prev = {}, [], None
    for t, o, h, l, c in bars:
        trs.append((h - l) if prev is None else max(h - l, abs(h - prev), abs(l - prev)))
        prev = c
        if len(trs) >= ATR_PERIOD:
            out[t] = sum(trs[-ATR_PERIOD:]) / ATR_PERIOD
    return out


def main() -> int:
    lo, hi = ms(sys.argv[1]), ms(sys.argv[2])
    m15 = load("15m", lo, hi)
    m1 = load("1m", lo, hi)
    atr = atr14(m15)
    by_min = {}
    for t, o, h, l, c in m1:
        by_min[t] = (o, h, l, c)
    print(f"{SYMBOL} {sys.argv[1]} to {sys.argv[2]}: {len(m15)} bars of 15m, {len(m1)} of 1m; stop {STOP_ATR} ATR, target {REWARD_RISK} R")
    print("fill-bar verdict, 15m OHLC (what the engine reads) against 1m sequence")
    print()
    hdr = "%-5s %-6s %8s %10s %10s %10s %10s %10s" % (
        "side", "offset", "fills", "agree", "FAKE WIN", "fake loss", "15m TGT", "15m STOP")
    print(hdr)
    for side in ("LONG", "SHORT"):
        d = 1.0 if side == "LONG" else -1.0
        for off in OFFSETS:
            fills = agree = fake_win = fake_loss = n_tgt = n_stop = 0
            for k in range(len(m15) - TTL - 1):
                t_s, _, _, _, c_s = m15[k]
                a = atr.get(t_s)
                if not a or a <= 0 or m15[k][0] < lo:
                    continue
                level = c_s - d * off * a
                entry = level - d * SPREAD / 2.0
                risk = STOP_ATR * a
                stop = entry - d * risk
                target = entry + d * REWARD_RISK * risk
                # Only the first working bar is audited: it is the one the fill
                # and the exit can share, and it is where the whole question is.
                t_f, o_f, h_f, l_f, c_f = m15[k + 1]
                touched = (l_f <= level) if d > 0 else (h_f >= level)
                if not touched:
                    continue
                fills += 1
                # --- the engine's reading: whole bar, stop before target
                if (l_f <= stop) if d > 0 else (h_f >= stop):
                    v15 = "STOP"
                elif (h_f >= target) if d > 0 else (l_f <= target):
                    v15 = "TARGET"
                else:
                    v15 = "OPEN"
                n_stop += v15 == "STOP"
                n_tgt += v15 == "TARGET"
                # --- the 1m reading: nothing before the fill minute counts
                v1 = "OPEN"
                filled = False
                for mi in range(15):
                    b = by_min.get(t_f + mi * 60_000)
                    if b is None:
                        continue
                    _, h1, l1, _ = b
                    if not filled:
                        if (l1 <= level) if d > 0 else (h1 >= level):
                            filled = True
                        else:
                            continue
                    # Same minute as the fill is judged too; stop first, as
                    # the engine does inside a bar it cannot order.
                    if (l1 <= stop) if d > 0 else (h1 >= stop):
                        v1 = "STOP"
                        break
                    if (h1 >= target) if d > 0 else (l1 <= target):
                        v1 = "TARGET"
                        break
                if not filled:
                    # 1m has a gap over the minute the 15m low belongs to; the
                    # bar is counted but its sequence is unknown, so it is not
                    # charged to either side.
                    v1 = v15
                if v1 == v15:
                    agree += 1
                elif v15 == "TARGET":
                    fake_win += 1
                elif v15 == "STOP" and v1 in ("TARGET", "OPEN"):
                    fake_loss += 1
                else:
                    pass
            if not fills:
                continue
            print("%-5s %-6.2f %8d %9.1f%% %9.1f%% %9.1f%% %9d %9d" % (
                side, off, fills, 100.0 * agree / fills, 100.0 * fake_win / fills,
                100.0 * fake_loss / fills, n_tgt, n_stop))
    print()
    print("FAKE WIN = the 15m bar recorded a TARGET whose level was only reached")
    print("before the order filled. It is the one new look-ahead a resting entry")
    print("adds, and it flatters the limit arm.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
