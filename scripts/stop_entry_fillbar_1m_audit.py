"""The ONE thing a 15m fill verdict cannot settle for a BREAKOUT order, on 1m.

Registered in docs/decisions/2026-10-07-stop-entry.md as a descriptive audit,
never a gate. It is the mirror of scripts/limit_fillbar_1m_audit.py and the
error it hunts points the OTHER way.

A long LIMIT rests BELOW the signal close with its stop below that again, so
price cannot reach the stop without passing the order: there the exposure was
the TARGET, and n3 measured FAKE WINS.

A long STOP rests ABOVE the signal close and its stop sits BELOW the close, so
a 15m bar can print its low - through the stop - BEFORE it ever prints the high
that filled the order. `check_exit` then tests that whole bar against a
position it opened mid-bar, and prices a gapped stop at `bar.open`, which also
preceded the fill. Both book a loss the sequence never allowed: a FAKE LOSS.

For every 15m bar the previous bar is treated as a signal (both sides,
separately), a stop order is rested at `offset x ATR14` on the breakout side,
the engine's own fallback stop (argv[3] ATR) and target (1.8 R) are derived from
the FILL, and the fill bar is judged twice: once from its 15m OHLC the way the
engine judges it, and once by walking its 1m bars in order from the minute the
order actually filled.

Usage:  python stop_entry_fillbar_1m_audit.py <from> <to> [stop_atr]
"""
from __future__ import annotations

import datetime as dt
import sys

import pyarrow.compute as pc
import pyarrow.parquet as pq

BARS = "E:/rust/flowdesk/data/bars"
SYMBOL = "XAUDUKA"          # the only gold tape with 1m over these windows
ATR_PERIOD = 14
STOP_ATR = float(sys.argv[3]) if len(sys.argv) > 3 else 1.2
REWARD_RISK = 1.8
SPREAD = 0.28
OFFSETS = [0.0, 0.25, 0.5]
TTL = 4


def ms(day: str) -> int:
    d = dt.datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=dt.timezone.utc)
    return int(d.timestamp() * 1000)


def load(tf: str, lo: int, hi: int):
    t = pq.read_table(f"{BARS}/{SYMBOL}-{tf}.parquet", columns=["time", "open", "high", "low", "close"])
    stamps = t.column("time")
    unit = stamps.type.unit if hasattr(stamps.type, "unit") else "us"
    div = {"s": 1e-3, "ms": 1.0, "us": 1e3, "ns": 1e6}[unit]
    epoch = [int(int(v) / div) for v in pc.cast(stamps, "int64").to_pylist()]
    o, h, l, c = (t.column(k).to_pylist() for k in ("open", "high", "low", "close"))
    return [(epoch[i], o[i], h[i], l[i], c[i]) for i in range(len(epoch))
            if lo - 40 * 86_400_000 <= epoch[i] < hi]


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
    by_min = {t: (o, h, l, c) for t, o, h, l, c in m1}
    print(f"{SYMBOL} {sys.argv[1]} to {sys.argv[2]}: {len(m15)} bars of 15m, {len(m1)} of 1m; "
          f"BREAKOUT side, stop {STOP_ATR} ATR, target {REWARD_RISK} R")
    print("fill-bar verdict, 15m OHLC (what the engine reads) against the 1m sequence")
    print()
    print("%-5s %-6s %8s %10s %10s %10s %10s %10s %9s" % (
        "side", "offset", "fills", "agree", "FAKE LOSS", "fake win", "15m TGT", "15m STOP", "gapfills"))
    for side in ("LONG", "SHORT"):
        d = 1.0 if side == "LONG" else -1.0
        for off in OFFSETS:
            fills = agree = fake_win = fake_loss = n_tgt = n_stop = gapped = 0
            for k in range(len(m15) - TTL - 1):
                t_s, _, _, _, c_s = m15[k]
                a = atr.get(t_s)
                if not a or a <= 0 or m15[k][0] < lo:
                    continue
                # BREAKOUT: above the close for a long.
                level = c_s + d * off * a
                t_f, o_f, h_f, l_f, c_f = m15[k + 1]
                touched = (h_f >= level) if d > 0 else (l_f <= level)
                if not touched:
                    continue
                # A gap through a stop order fills AT THE OPEN, worse.
                tape = max(level, o_f) if d > 0 else min(level, o_f)
                if tape != level:
                    gapped += 1
                entry = tape + d * SPREAD / 2.0
                risk = STOP_ATR * a
                stop = entry - d * risk
                target = entry + d * REWARD_RISK * risk
                fills += 1
                # --- the engine's reading: whole bar, stop before target, and a
                # gapped stop priced at the bar's OPEN.
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
                    o1, h1, l1, _ = b
                    if not filled:
                        if (h1 >= level) if d > 0 else (l1 <= level):
                            filled = True
                            # The minute's own fill price, and the stop and
                            # target that follow from it.
                            tape1 = max(level, o1) if d > 0 else min(level, o1)
                            e1 = tape1 + d * SPREAD / 2.0
                            stop = e1 - d * risk
                            target = e1 + d * REWARD_RISK * risk
                        else:
                            continue
                    if (l1 <= stop) if d > 0 else (h1 >= stop):
                        v1 = "STOP"
                        break
                    if (h1 >= target) if d > 0 else (l1 <= target):
                        v1 = "TARGET"
                        break
                if not filled:
                    # 1m has a gap over the minute the 15m high belongs to; the
                    # bar is counted but its sequence is unknown, so it is not
                    # charged to either side.
                    v1 = v15
                if v1 == v15:
                    agree += 1
                elif v15 == "STOP" and v1 in ("TARGET", "OPEN"):
                    fake_loss += 1
                elif v15 == "TARGET" and v1 in ("STOP", "OPEN"):
                    fake_win += 1
            if not fills:
                continue
            print("%-5s %-6.2f %8d %9.1f%% %9.1f%% %9.1f%% %9d %9d %9d" % (
                side, off, fills, 100.0 * agree / fills, 100.0 * fake_loss / fills,
                100.0 * fake_win / fills, n_tgt, n_stop, gapped))
    print()
    print("FAKE LOSS = the 15m bar recorded a STOP whose level was only reached")
    print("BEFORE the order filled. It is the look-ahead a breakout entry adds,")
    print("and unlike the limit arm's fake win it points AGAINST the method.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
