# Generate the parity fixtures for the method-level indicators in
# `crates/fd-indicators`, from the same Python the 2026-09-18 bias study was
# measured with. Read-only except for the fixtures it writes.
#
#   python py/research/method_fixtures.py
#
# WHY A FIXTURE AND NOT A DESCRIPTION. `bias_defs.py` is the file every number
# in docs/decisions/2026-09-18-market-bias-definitions.md came out of. A Rust
# port that agrees with the prose but not with the function would ship
# measurements that were taken on a different series - and the zigzag already
# proved that is not hypothetical: three of four independent choices differed
# from the Python and all three still reproduced the owner's morning table.
# Seven bars of agreement discriminates nothing. So each port is pinned bar
# for bar against this file's output on real bars, and these fixtures are that
# output.
#
# WHICH BARS. The 2,000 real XAUUSD H1 bars already in the repo as
# `crates/fd-api/tests/fixtures/zigzag-h1-xauusd.csv` - the last 2,000 of the
# 25,708-bar file the study was measured on. The full file is not in a
# desktop checkout (data/ is git-ignored), and inventing bars would test the
# port against an arithmetic exercise rather than against a market: gaps,
# weekends, limit bars and flat sessions are exactly where an anchor rule or a
# band recursion goes wrong.
#
# A CONSEQUENCE, STATED SO NOBODY DISCOVERS IT LATER: that slice runs from May
# to September 2026, so it is entirely inside New York daylight time and the
# broker offset is +3 on every bar of it. The +2 branch of `shift_hours` is
# NOT exercised by these fixtures. It is covered instead by a unit test on
# synthetic bars across the 2026-11-01 changeover, which is the honest split -
# real bars for the arithmetic, a constructed clock for the clock.
import datetime as dt
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

from bias_defs import UP, DOWN, FLAT, T, O, H, L, C, atr, supertrend, anchored_vwap, shift_hours

FIXTURES = os.path.join(ROOT, "crates", "fd-api", "tests", "fixtures")
BARS_CSV = os.path.join(FIXTURES, "zigzag-h1-xauusd.csv")


def load_bars():
    """The bars out of the zigzag fixture, as bias_defs tuples.

    The time column is epoch milliseconds of the file's naive-UTC stamps,
    which is what `shift_hours` expects to be handed.
    """
    bars = []
    with open(BARS_CSV, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            f = line.split(",")
            ms = int(f[0])
            when = dt.datetime(1970, 1, 1) + dt.timedelta(milliseconds=ms)
            bars.append((when, float(f[1]), float(f[2]), float(f[3]), float(f[4])))
    return bars


def ms_of(when):
    return int((when - dt.datetime(1970, 1, 1)).total_seconds() * 1000)


def supertrend_line(bars, n=10, mult=3.0):
    """The band in force, under `bias_defs.supertrend`'s own recursion.

    The study measured the DIRECTION; a chart needs the line as well. Rather
    than write a second recursion for it, this walks the one in `bias_defs`
    and records the band it is standing on, then asserts its own direction
    equals `supertrend()`'s labels bar for bar. So the line cannot drift from
    the series the numbers were measured on: if it did, this file would fail
    before it wrote anything.
    """
    a = atr(bars, n)
    line, direction_out = [None] * len(bars), [None] * len(bars)
    upper = lower = None
    direction = 1
    for t in range(len(bars)):
        if a[t] is None:
            continue
        mid = (bars[t][H] + bars[t][L]) / 2.0
        bu, bl = mid + mult * a[t], mid - mult * a[t]
        if upper is None:
            upper, lower = bu, bl
        else:
            upper = bu if (bu < upper or bars[t - 1][C] > upper) else upper
            lower = bl if (bl > lower or bars[t - 1][C] < lower) else lower
        if bars[t][C] > upper:
            direction = 1
        elif bars[t][C] < lower:
            direction = -1
        direction_out[t] = direction
        line[t] = lower if direction == 1 else upper

    labels = supertrend(bars, n, mult)
    for t in range(len(bars)):
        want = UP if direction_out[t] == 1 else (DOWN if direction_out[t] == -1 else FLAT)
        if want != labels[t]:
            raise SystemExit(f"the line walk disagrees with bias_defs.supertrend at bar {t}")
    return line, direction_out


def avwap_series(bars, period):
    """The VWAP value itself, under `bias_defs.anchored_vwap`'s own bucketing.

    Same argument as `supertrend_line`: the study measured the label (price
    above or below), a chart draws the level, and this records the level from
    the identical loop and then checks its own labels against the measured
    function.
    """
    out = [None] * len(bars)
    key = None
    total = 0.0
    count = 0
    for t, b in enumerate(bars):
        srv = b[T] + dt.timedelta(hours=shift_hours(b[T]))
        k = (srv.year, srv.month, srv.day) if period == "day" else srv.isocalendar()[:2]
        if k != key:
            key, total, count = k, 0.0, 0
        total += (b[H] + b[L] + b[C]) / 3.0
        count += 1
        out[t] = total / count

    labels = anchored_vwap(bars, period)
    for t, b in enumerate(bars):
        want = UP if b[C] > out[t] else (DOWN if b[C] < out[t] else FLAT)
        if want != labels[t]:
            raise SystemExit(f"the value walk disagrees with bias_defs.anchored_vwap at bar {t}")
    return out


def num(v):
    """Full precision, so the fixture pins the float and not a rounding."""
    return "" if v is None else repr(v)


def write(name, header, rows):
    path = os.path.join(FIXTURES, name)
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(header)
        for row in rows:
            fh.write(",".join(row) + "\n")
    print(f"wrote {path} ({len(rows)} rows)")


def main():
    bars = load_bars()
    print(f"{len(bars)} bars, {bars[0][T]} .. {bars[-1][T]}")
    offsets = sorted({shift_hours(b[T]) for b in bars})
    print(f"broker offsets present in this slice: {offsets}")

    line, direction = supertrend_line(bars)
    write(
        "supertrend-h1-xauusd.csv",
        "# XAUUSD H1, the same 2000 real bars as zigzag-h1-xauusd.csv.\n"
        "# supertrend and direction produced by py/research/method_fixtures.py, walking\n"
        "# py/research/bias_defs.py::supertrend(bars, 10, 3.0) and checked against its labels.\n"
        "# Empty columns are the ATR(10) warmup, where the Python emits no direction at all.\n"
        "# time_ms,open,high,low,close,supertrend,direction\n",
        [
            [str(ms_of(b[T])), num(b[O]), num(b[H]), num(b[L]), num(b[C]), num(line[t]),
             "" if direction[t] is None else str(direction[t])]
            for t, b in enumerate(bars)
        ],
    )

    day = avwap_series(bars, "day")
    week = avwap_series(bars, "week")
    write(
        "avwap-h1-xauusd.csv",
        "# XAUUSD H1, the same 2000 real bars as zigzag-h1-xauusd.csv.\n"
        "# avwapDay / avwapWeek produced by py/research/method_fixtures.py, walking\n"
        "# py/research/bias_defs.py::anchored_vwap(bars, 'day'|'week') and checked against\n"
        "# its labels. Anchors are on the BROKER's clock (UTC+3 in NY summer, +2 in winter);\n"
        "# every bar of this slice is summer, so it pins the +3 branch only.\n"
        "# No column is ever empty: an anchored mean is defined from its first bar.\n"
        "# time_ms,open,high,low,close,avwapDay,avwapWeek\n",
        [
            [str(ms_of(b[T])), num(b[O]), num(b[H]), num(b[L]), num(b[C]), num(day[t]), num(week[t])]
            for t, b in enumerate(bars)
        ],
    )


if __name__ == "__main__":
    main()
