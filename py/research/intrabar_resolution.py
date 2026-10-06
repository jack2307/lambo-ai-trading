"""The resolution map, as registered in
docs/decisions/2026-10-07-intrabar-resolution.md.

Replays the engine's fill model (engine.rs: signal on bar i-1, fill at the open
of bar i, half the spread on entry, stop = k x ATR of the SIGNAL bar, target =
risk x rr, STOP CHECKED BEFORE TARGET, gap through a level fills at the open,
4 h clock) on XAUDUKA-15m, then adjudicates the same trade on XAUDUKA-1m, whose
bars are the same tape (pre-check P2: 0 of 20,000 bars mismatched).

Nothing here is a gate. The engine is not modified.
"""
import json
import sys

import numpy as np

from intrabar_bars import load, atr_wilder

SPREAD = 0.28            # [markets.xauduka.trading] spread, price points
ATR_PERIOD = 14          # [trading] fallback_atr_period / every strategy's atrPeriod
STRIDE = 17              # declared: coprime with the 96 bars of a day
STOPS = [0.10, 0.20, 0.30, 0.378, 0.45, 0.50, 0.60, 0.714,
         0.85, 1.00, 1.20, 1.50, 2.00, 2.681, 3.310, 4.210]
RRS = [1.0, 1.5, 1.8]
HOLDS = [("4h", 14_400_000, 18), ("48h", 172_800_000, 194)]
BAR15 = 900_000

STOP, TARGET, TIMEOUT, EOD = 1, 2, 3, 4
SENTINEL = 1 << 30

t15, o15, h15, l15, c15 = load("XAUDUKA-15m")
t1, o1, h1, l1, c1 = load("XAUDUKA-1m")
atr15 = atr_wilder(h15, l15, c15, ATR_PERIOD)
n15 = len(t15)

# Fill bars: the ATR of the SIGNAL bar i-1 must be finite, so i-1 >= 13.
fills = np.arange(ATR_PERIOD, n15, STRIDE, dtype=np.int64)
fills = fills[np.isfinite(atr15[fills - 1])]
sig_atr = atr15[fills - 1]
N = len(fills)

# The 1m window of every 15m bar, precomputed once.
m_start = np.searchsorted(t1, t15, side="left")
m_end = np.searchsorted(t1, t15 + BAR15, side="left")


def walk15(side, stop, target, cap_ms, hmax):
    """First bar in [i, ...] to touch either level, stop taking precedence."""
    long = side == "LONG"
    verdict = np.zeros(N, dtype=np.int8)
    exit_h = np.full(N, -1, dtype=np.int32)
    both = np.zeros(N, dtype=bool)
    exit_raw = np.full(N, np.nan)
    alive = np.ones(N, dtype=bool)
    for h in range(hmax + 1):
        if not alive.any():
            break
        past = alive & ((fills + h) >= n15)
        if past.any():
            verdict[past] = EOD
            exit_h[past] = h - 1
            exit_raw[past] = c15[n15 - 1]
            alive &= ~past
            if not alive.any():
                break
        a = np.flatnonzero(alive)
        jj = fills[a] + h
        hi, lo, op, cl = h15[jj], l15[jj], o15[jj], c15[jj]
        s, t = stop[a], target[a]
        if long:
            s_hit, t_hit = lo <= s, hi >= t
            s_raw = np.where(op <= s, op, s)
            t_raw = np.where(op >= t, op, t)
        else:
            s_hit, t_hit = hi >= s, lo <= t
            s_raw = np.where(op >= s, op, s)
            t_raw = np.where(op <= t, op, t)
        hit = s_hit | t_hit
        timed = (~hit) & ((t15[jj] - t15[fills[a]]) > cap_ms)
        done = hit | timed
        if done.any():
            d = a[done]
            verdict[d] = np.where(s_hit[done], STOP,
                                  np.where(t_hit[done], TARGET, TIMEOUT))
            exit_h[d] = h
            both[d] = s_hit[done] & t_hit[done]
            exit_raw[d] = np.where(s_hit[done], s_raw[done],
                                   np.where(t_hit[done], t_raw[done], cl[done]))
            alive[d] = False
    assert not alive.any(), f"{int(alive.sum())} trades unresolved at hmax={hmax}"
    return verdict, exit_h, both, exit_raw


def walk1m(side, which, stop, target):
    """Adjudicate the 15m exit bar on its own 1m bars, in time order.

    A trade whose stop and target fall inside the SAME 1m bar is reported in its
    own column (`unresolved at 1m`) and never folded into either side.
    """
    long = side == "LONG"
    M = len(which)
    s0, e0 = m_start[which], m_end[which]
    first_s = np.full(M, SENTINEL, dtype=np.int64)
    first_t = np.full(M, SENTINEL, dtype=np.int64)
    raw_s = np.full(M, np.nan)
    raw_t = np.full(M, np.nan)
    width = int((e0 - s0).max()) if M else 0
    for hm in range(width):
        a = np.flatnonzero((s0 + hm) < e0)
        if a.size == 0:
            continue
        mm = s0[a] + hm
        hi, lo, op = h1[mm], l1[mm], o1[mm]
        s, t = stop[a], target[a]
        if long:
            sh, th = lo <= s, hi >= t
            sr = np.where(op <= s, op, s)
            tr = np.where(op >= t, op, t)
        else:
            sh, th = hi >= s, lo <= t
            sr = np.where(op >= s, op, s)
            tr = np.where(op <= t, op, t)
        pos = np.flatnonzero(sh & (first_s[a] == SENTINEL))
        if pos.size:
            first_s[a[pos]] = hm
            raw_s[a[pos]] = sr[pos]
        pos = np.flatnonzero(th & (first_t[a] == SENTINEL))
        if pos.size:
            first_t[a[pos]] = hm
            raw_t[a[pos]] = tr[pos]
    v = np.zeros(M, dtype=np.int8)
    raw = np.full(M, np.nan)
    neither = (first_s == SENTINEL) & (first_t == SENTINEL)
    same = (first_s == first_t) & ~neither
    s_first = first_s < first_t
    t_first = first_t < first_s
    v[s_first] = STOP
    raw[s_first] = raw_s[s_first]
    v[t_first] = TARGET
    raw[t_first] = raw_t[t_first]
    return v, raw, same, neither


def r_of(side, entry, risk, raw):
    """The exit price pays half the spread, then R = points / risk."""
    half = SPREAD / 2.0
    if side == "LONG":
        return (raw - half - entry) / risk
    return (entry - (raw + half)) / risk


def signal_range_over_atr():
    """The signal bar's true range divided by its ATR — the quantity that says
    whether a mechanism sits on expansion bars or quiet ones."""
    s = fills - 1
    pc = c15[s - 1]
    tr = np.maximum(h15[s] - l15[s],
                    np.maximum(np.abs(h15[s] - pc), np.abs(l15[s] - pc)))
    return tr / sig_atr


def run_cell(side, k, rr, cap_name, cap_ms, hmax):
    half = SPREAD / 2.0
    entry = o15[fills] + (half if side == "LONG" else -half)
    risk = k * sig_atr
    if side == "LONG":
        stop, target = entry - risk, entry + risk * rr
    else:
        stop, target = entry + risk, entry - risk * rr

    v15, exit_h, both, raw15 = walk15(side, stop, target, cap_ms, hmax)
    r15 = r_of(side, entry, risk, raw15)

    resolved = np.isin(v15, (STOP, TARGET))
    idx = np.flatnonzero(resolved)
    which = fills[idx] + exit_h[idx]
    v1, raw1, same, neither = walk1m(side, which, stop[idx], target[idx])
    r1 = r_of(side, entry[idx], risk[idx], raw1)

    a = v15[idx]
    ok1 = ~same & ~neither
    agree = (a == v1) & ok1
    fake_loss = (a == STOP) & (v1 == TARGET)
    fake_win = (a == TARGET) & (v1 == STOP)

    # R under the 1m adjudication: the 1m exit price where the 1m bars decided,
    # the 15m one where no 1m call was needed (timeout, end of data) or where
    # 1m could not resolve it either.
    r1_full = r15.copy()
    r1_full[idx[ok1]] = r1[ok1]

    return dict(
        side=side, stop_atr=k, rr=rr, hold=cap_name, trades=N,
        resolved=int(resolved.sum()),
        stop15=int((v15 == STOP).sum()), tgt15=int((v15 == TARGET).sum()),
        timeout15=int((v15 == TIMEOUT).sum()), eod15=int((v15 == EOD).sum()),
        both=int(both.sum()),
        agree=int(agree.sum()),
        fake_loss=int(fake_loss.sum()),
        fake_win=int(fake_win.sum()),
        unresolved_1m=int(same.sum()),
        neither_1m=int(neither.sum()),
        price_diff_trades=int((np.abs(r1_full - r15) > 1e-9).sum()),
        meanR_15=float(np.nanmean(r15)),
        meanR_1m=float(np.nanmean(r1_full)),
        sumR_15=float(np.nansum(r15)),
        sumR_1m=float(np.nansum(r1_full)),
    )


def main():
    rows = []
    for cap_name, cap_ms, hmax in HOLDS:
        for k in STOPS:
            for rr in RRS:
                for side in ("LONG", "SHORT"):
                    rows.append(run_cell(side, k, rr, cap_name, cap_ms, hmax))
                    print(".", end="", file=sys.stderr, flush=True)
        print(f" {cap_name} done", file=sys.stderr, flush=True)
    out = sys.argv[1] if len(sys.argv) > 1 else "intrabar-cells.json"
    with open(out, "w") as f:
        json.dump(rows, f, indent=1)
    print(f"entries per cell: {N}   cells: {len(rows)}   -> {out}")


if __name__ == "__main__":
    main()
