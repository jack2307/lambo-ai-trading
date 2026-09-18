# Nine candidate bias definitions, each computed CAUSALLY: the label at bar t
# uses bars up to and including t and nothing after. Read-only.
#
# Every definition returns one of "UP", "DOWN", "FLAT" per bar. FLAT means the
# definition declines to call a direction - it is a real answer, not a gap,
# and the measures below count it as such.
import datetime as dt
from zoneinfo import ZoneInfo

NEW_YORK = ZoneInfo("America/New_York")
UP, DOWN, FLAT = "UP", "DOWN", "FLAT"


# ---------------------------------------------------------------- the series
def shift_hours(naive_utc):
    dst = naive_utc.replace(tzinfo=dt.timezone.utc).astimezone(NEW_YORK).dst() or dt.timedelta(0)
    return 3 if dst else 2


def resample(bars, hours, epoch_anchor=False):
    """15m -> H-hour candles. Drops the first and last buckets, which are the
    only ones that can be partial, so every bar returned is whole."""
    out, cur, key, counts = [], None, None, []
    for t0, o, h, l, c in bars:
        naive = t0.replace(tzinfo=None) if hasattr(t0, "tzinfo") else t0
        off = 0 if epoch_anchor else shift_hours(naive)
        t = naive + dt.timedelta(hours=off)
        k = (t.year, t.month, t.day, (t.hour // hours) * hours if hours < 24 else 0)
        if k != key:
            if cur:
                out.append(cur[:5])
                counts.append(cur[5])
            key = k
            start = dt.datetime(*k[:3], k[3])
            soff = 0 if epoch_anchor else shift_hours(start)
            cur = [start - dt.timedelta(hours=soff), o, h, l, c, 1]
        else:
            cur[2] = max(cur[2], h)
            cur[3] = min(cur[3], l)
            cur[4] = c
            cur[5] += 1
    if cur:
        out.append(cur[:5])
        counts.append(cur[5])
    # Trim partial edges: the first and last bucket of a resample are the only
    # ones the source file can truncate. Validated against the independently
    # exported H4: 6,585 of 6,587 complete buckets identical, the two
    # exceptions being exactly these edges.
    full = max(counts[1:-1]) if len(counts) > 2 else max(counts)
    keep = [i for i in range(len(out)) if counts[i] == full]
    if keep:
        return out[keep[0]:keep[-1] + 1]
    return out


T, O, H, L, C = 0, 1, 2, 3, 4


# ------------------------------------------------------------- shared pieces
def true_ranges(bars):
    tr = [bars[0][H] - bars[0][L]]
    for i in range(1, len(bars)):
        pc = bars[i - 1][C]
        tr.append(max(bars[i][H] - bars[i][L], abs(bars[i][H] - pc), abs(bars[i][L] - pc)))
    return tr


def wilder(xs, n):
    """Wilder smoothing, returned aligned to xs with None before warmup."""
    out = [None] * len(xs)
    if len(xs) < n:
        return out
    s = sum(xs[:n])
    out[n - 1] = s / n
    for i in range(n, len(xs)):
        s = s - s / n + xs[i]
        out[i] = s / n
    return out


def atr(bars, n=14):
    return wilder(true_ranges(bars), n)


def ema_series(vals, n):
    out = [None] * len(vals)
    if len(vals) < n:
        return out
    k = 2.0 / (n + 1)
    e = sum(vals[:n]) / n
    out[n - 1] = e
    for i in range(n, len(vals)):
        e = vals[i] * k + e * (1 - k)
        out[i] = e
    return out


# ------------------------------------------------------- 1. fractal swings
def fractal_structure(bars, n=2):
    """UP needs a higher high AND a higher low; DOWN the mirror; else FLAT.

    A fractal at bar i is not KNOWN until bar i+n has printed, so the label at
    bar t is built only from swings confirmed by t. That lag is the whole
    subject of this study and is reproduced here exactly, not idealised away.
    """
    labels = [FLAT] * len(bars)
    highs, lows = [], []          # (index, price), confirmed only
    hi_i = lo_i = 0
    for t in range(len(bars)):
        # confirm any fractal whose right-hand window closes at t
        i = t - n
        if i >= n:
            win = range(i - n, i + n + 1)
            if all(bars[i][H] >= bars[j][H] for j in win) and \
               any(bars[i][H] > bars[j][H] for j in win if j != i):
                highs.append((i, bars[i][H]))
            if all(bars[i][L] <= bars[j][L] for j in win) and \
               any(bars[i][L] < bars[j][L] for j in win if j != i):
                lows.append((i, bars[i][L]))
        if len(highs) >= 2 and len(lows) >= 2:
            hh = highs[-1][1] > highs[-2][1]
            hl = lows[-1][1] > lows[-2][1]
            lh = highs[-1][1] < highs[-2][1]
            ll = lows[-1][1] < lows[-2][1]
            labels[t] = UP if (hh and hl) else (DOWN if (lh and ll) else FLAT)
    return labels


# ------------------------------------------------------- 2. ATR ZigZag, live
def atr_zigzag(bars, k=3.0, n=14):
    """Reversal when price retraces k x ATR from the running extreme.

    MEASURED AS A TRADER SEES IT LIVE. A ZigZag repaints: the leg you are in
    is provisional until the reversal threshold is crossed, and drawn
    afterwards it looks prescient. Here the label at bar t is the leg that was
    in force AT t, which is the only version anyone could have traded.

    THIS FUNCTION IS PINNED BY A RUST TEST. `crates/fd-api/tests/fixtures/
    zigzag-h1-xauusd.csv` holds the last 2,000 bars of the H1 file the study
    was measured on WITH THIS FUNCTION'S OWN OUTPUT as its label column, and
    `cargo test` compares the route's port against it. So **changing
    anything below breaks a test in another language, and that is the
    intended behaviour** - the route must not drift away from the definition
    the published numbers were measured on. If you mean to change it, change
    the fixture in the same commit and re-measure the note's table; if you do
    not mean to, the failing test is the point.

    Verified 2026-09-18: this function reproduces that fixture on all 2,000
    rows, and d1's independent Rust port matches on all 25,708 bars of the
    full file.

    FOUR CHOICES THIS PINS, because prose about "an ATR zigzag" does not, and
    each of them moves the numbers. Asked by d1 before reimplementing it in
    the route, 2026-09-18, which is the right question to have asked - and
    the asking earned its keep: three of d1's four independent choices
    differed from these (`>=` for `>`, testing down before up on a bar that
    could seed either direction, and a running high AND low before the first
    pivot rather than one wandering scalar), and **all three reproduced the
    owner's morning**. Seven bars of agreement discriminates nothing.

    1. HIGHS AND LOWS, not closes, for both the extension of a leg and the
       reversal test. A leg extends on `b[H]` while up and on `b[L]` while
       down, and reverses when the opposite extreme retraces past the
       threshold.
    2. ATR AT THE CURRENT BAR, `k * a[t]`, not the ATR at the pivot. The
       consequence is real and worth stating rather than discovering: the
       threshold MOVES under an open leg, so a leg opened in a quiet stretch
       needs a larger retracement to end it once volatility rises. Freezing
       it at the pivot is a defensible alternative and would give a different
       table; this is what every published number was measured with.
    3. THE REVERSAL IS TAKEN ON THE BAR THAT CROSSES, using that bar's high
       and low, so the label changes on that bar and is final at its close.
       Every measurement here is on closed bars. A route recomputing this on
       closed bars gets the identical series; one recomputing it INTRABAR
       would flip earlier, sometimes flip back, and would not reproduce these
       numbers.
    4. BEFORE THE FIRST PIVOT the label is FLAT, and that is the only FLAT
       this definition ever emits. Measured on the 25,708-bar H1 file: FLAT on
       the first 13 bars - ATR(14) warmup plus the wait for the first
       threshold cross - and never again, which is why the published mix
       reads 55/45/0. From the first pivot onward the leg direction alone is
       the label.
    """
    a = atr(bars, n)
    labels = [FLAT] * len(bars)
    direction = 0
    extreme = bars[0][C]
    for t in range(len(bars)):
        if a[t] is None:
            continue
        thr = k * a[t]
        if direction == 0:
            if bars[t][H] - extreme > thr:
                direction, extreme = 1, bars[t][H]
            elif extreme - bars[t][L] > thr:
                direction, extreme = -1, bars[t][L]
            else:
                extreme = max(extreme, bars[t][H]) if bars[t][C] >= extreme else min(extreme, bars[t][L])
        elif direction == 1:
            extreme = max(extreme, bars[t][H])
            if extreme - bars[t][L] > thr:
                direction, extreme = -1, bars[t][L]
        else:
            extreme = min(extreme, bars[t][L])
            if bars[t][H] - extreme > thr:
                direction, extreme = 1, bars[t][H]
        labels[t] = UP if direction == 1 else (DOWN if direction == -1 else FLAT)
    return labels


# --------------------------------------------------- 3. Dow swings on closes
def dow_closes(bars, n=2):
    """The fractal rule applied to CLOSES rather than to highs and lows.

    Closes are what a Dow-theory reading is usually drawn on, and they ignore
    the wicks that make a high/low fractal fire on a single spike.
    """
    closes = [b[C] for b in bars]
    labels = [FLAT] * len(bars)
    peaks, troughs = [], []
    for t in range(len(bars)):
        i = t - n
        if i >= n:
            win = range(i - n, i + n + 1)
            if all(closes[i] >= closes[j] for j in win) and any(closes[i] > closes[j] for j in win if j != i):
                peaks.append((i, closes[i]))
            if all(closes[i] <= closes[j] for j in win) and any(closes[i] < closes[j] for j in win if j != i):
                troughs.append((i, closes[i]))
        if len(peaks) >= 2 and len(troughs) >= 2:
            hh = peaks[-1][1] > peaks[-2][1]
            hl = troughs[-1][1] > troughs[-2][1]
            lh = peaks[-1][1] < peaks[-2][1]
            ll = troughs[-1][1] < troughs[-2][1]
            labels[t] = UP if (hh and hl) else (DOWN if (lh and ll) else FLAT)
    return labels


# ------------------------------------------------------------ 4. EMA stacks
def ema_stack(bars, spans=(21, 55)):
    closes = [b[C] for b in bars]
    es = [ema_series(closes, s) for s in spans]
    labels = [FLAT] * len(bars)
    for t in range(len(bars)):
        vals = [e[t] for e in es]
        if any(v is None for v in vals):
            continue
        px = closes[t]
        if all(vals[i] > vals[i + 1] for i in range(len(vals) - 1)) and px > vals[0]:
            labels[t] = UP
        elif all(vals[i] < vals[i + 1] for i in range(len(vals) - 1)) and px < vals[0]:
            labels[t] = DOWN
    return labels


# ------------------------------------------------------------- 5. ADX / DI
def adx_di(bars, n=14, gate=25.0):
    trs, pdm, ndm = [0.0], [0.0], [0.0]
    for i in range(1, len(bars)):
        ph, pl, pc = bars[i - 1][H], bars[i - 1][L], bars[i - 1][C]
        h, l = bars[i][H], bars[i][L]
        trs.append(max(h - l, abs(h - pc), abs(l - pc)))
        up, dn = h - ph, pl - l
        pdm.append(up if (up > dn and up > 0) else 0.0)
        ndm.append(dn if (dn > up and dn > 0) else 0.0)
    tr_s, p_s, n_s = wilder(trs[1:], n), wilder(pdm[1:], n), wilder(ndm[1:], n)
    dx = [None] * len(bars)
    pdi_a = [None] * len(bars)
    ndi_a = [None] * len(bars)
    for i in range(len(tr_s)):
        if tr_s[i] is None or tr_s[i] <= 0:
            continue
        pdi, ndi = 100 * p_s[i] / tr_s[i], 100 * n_s[i] / tr_s[i]
        pdi_a[i + 1], ndi_a[i + 1] = pdi, ndi
        if pdi + ndi > 0:
            dx[i + 1] = 100 * abs(pdi - ndi) / (pdi + ndi)
    adx = [None] * len(bars)
    run = [v for v in dx if v is not None]
    idx = [i for i, v in enumerate(dx) if v is not None]
    sm = wilder(run, n)
    for j, i in enumerate(idx):
        adx[i] = sm[j]
    labels = [FLAT] * len(bars)
    for t in range(len(bars)):
        if adx[t] is None or pdi_a[t] is None:
            continue
        if adx[t] >= gate:
            labels[t] = UP if pdi_a[t] > ndi_a[t] else DOWN
    return labels


# ----------------------------------------------------------- 6. Donchian
def donchian_state(bars, n=20):
    """UP while the most recent n-bar extreme touched was the HIGH."""
    labels = [FLAT] * len(bars)
    state = 0
    for t in range(len(bars)):
        if t < n:
            continue
        win = bars[t - n:t]
        hi = max(b[H] for b in win)
        lo = min(b[L] for b in win)
        if bars[t][H] > hi:
            state = 1
        elif bars[t][L] < lo:
            state = -1
        labels[t] = UP if state == 1 else (DOWN if state == -1 else FLAT)
    return labels


# ----------------------------------------------------------- 7. Supertrend
def supertrend(bars, n=10, mult=3.0):
    a = atr(bars, n)
    labels = [FLAT] * len(bars)
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
        labels[t] = UP if direction == 1 else DOWN
    return labels


# -------------------------------------------------- 8. anchored VWAP
def anchored_vwap(bars, period="day", epoch_anchor=False):
    """Price above/below the VWAP anchored at the session or week open.

    No volume in these files that is worth the name (tick volume only), so
    this is the typical-price mean from the anchor - a VWAP with every bar
    weighted equally. Said plainly because calling it VWAP otherwise would be
    a units claim the data cannot support.
    """
    labels = [FLAT] * len(bars)
    key = None
    total = 0.0
    count = 0
    for t, b in enumerate(bars):
        naive = b[T]
        off = 0 if epoch_anchor else shift_hours(naive)
        srv = naive + dt.timedelta(hours=off)
        k = (srv.year, srv.month, srv.day) if period == "day" else srv.isocalendar()[:2]
        if k != key:
            key, total, count = k, 0.0, 0
        total += (b[H] + b[L] + b[C]) / 3.0
        count += 1
        vw = total / count
        labels[t] = UP if b[C] > vw else (DOWN if b[C] < vw else FLAT)
    return labels


# ---------------------------------------- 9. close within the prior range
def prior_range_position(bars, period="day", epoch_anchor=False):
    """Above or below the midpoint of the PRIOR day's (week's) range.

    This is the definition that flipped sign between anchors in the step-0
    study - +0.454R on the epoch anchor, -0.811R on the broker's - so it is
    carried here mainly to be measured again rather than because anything
    recommends it.
    """
    labels = [FLAT] * len(bars)
    key = None
    cur_hi = cur_lo = None
    prev_mid = None
    for t, b in enumerate(bars):
        naive = b[T]
        off = 0 if epoch_anchor else shift_hours(naive)
        srv = naive + dt.timedelta(hours=off)
        k = (srv.year, srv.month, srv.day) if period == "day" else srv.isocalendar()[:2]
        if k != key:
            if cur_hi is not None:
                prev_mid = (cur_hi + cur_lo) / 2.0
            key, cur_hi, cur_lo = k, b[H], b[L]
        else:
            cur_hi, cur_lo = max(cur_hi, b[H]), min(cur_lo, b[L])
        if prev_mid is not None:
            labels[t] = UP if b[C] > prev_mid else (DOWN if b[C] < prev_mid else FLAT)
    return labels


def all_definitions(bars, epoch_anchor=False):
    """Every candidate, named as the note will name them."""
    return {
        "fractal(1)":        fractal_structure(bars, 1),
        "fractal(2)":        fractal_structure(bars, 2),
        "fractal(3)":        fractal_structure(bars, 3),
        "zigzag 3xATR":      atr_zigzag(bars, 3.0),
        "zigzag 1.5xATR":    atr_zigzag(bars, 1.5),
        "dow closes(2)":     dow_closes(bars, 2),
        "ema 21/55":         ema_stack(bars, (21, 55)),
        "ema 20/50/200":     ema_stack(bars, (20, 50, 200)),
        "adx20 +DI":         adx_di(bars, 14, 20.0),
        "adx25 +DI":         adx_di(bars, 14, 25.0),
        "donchian20":        donchian_state(bars, 20),
        "donchian55":        donchian_state(bars, 55),
        "supertrend(10,3)":  supertrend(bars, 10, 3.0),
        "avwap day":         anchored_vwap(bars, "day", epoch_anchor),
        "avwap week":        anchored_vwap(bars, "week", epoch_anchor),
        "pdmid day":         prior_range_position(bars, "day", epoch_anchor),
        "pdmid week":        prior_range_position(bars, "week", epoch_anchor),
    }
