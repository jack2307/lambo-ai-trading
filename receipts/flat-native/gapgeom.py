# agent/flat-native: an INDEPENDENT re-implementation of gap-fade's entry
# geometry, straight off the parquet, to measure the distribution of
# risk = |entry - stop| that the engine sizes on. Read-only; no gate cell.
#
# gap_fade.rs: on the first bar after a break >= minGapHours, gap = bar.open -
# prev.close; if |gap| >= minGapAtr * ATR(14) at the bar BEFORE the break,
# side = short if gap > 0 else long, target = bar.open - gap*fill,
# stop = bar.open + gap*stopGapMult.  The engine fills the pending order at the
# NEXT bar's open (plus half the spread) and sizes on risk = |entry - stop|.
import pyarrow.parquet as pq, numpy as np, datetime as dt, sys

MIN_GAP_H = float(sys.argv[1]) if len(sys.argv) > 1 else 24.0
MIN_GAP_ATR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
STOP_MULT = float(sys.argv[3]) if len(sys.argv) > 3 else 1.0
SPREAD = 0.28

t = pq.read_table(r'E:/rust/flowdesk/data/bars/XAUDUKA-15m.parquet')
tm = t.column('time').to_pylist()
o = np.array(t.column('open').to_pylist(), float)
h = np.array(t.column('high').to_pylist(), float)
l = np.array(t.column('low').to_pylist(), float)
c = np.array(t.column('close').to_pylist(), float)
ms = np.array([int(x.replace(tzinfo=dt.timezone.utc).timestamp() * 1000) for x in tm], dtype='int64')

# Wilder ATR(14), the engine's atr_14
tr = np.empty(len(c)); tr[0] = h[0] - l[0]
pc = c[:-1]
tr[1:] = np.maximum(h[1:] - l[1:], np.maximum(np.abs(h[1:] - pc), np.abs(l[1:] - pc)))
atr = np.full(len(c), np.nan); P = 14
atr[P - 1] = tr[:P].mean()
for i in range(P, len(c)):
    atr[i] = (atr[i - 1] * (P - 1) + tr[i]) / P

WINS = {"IS": ("2010-06-01", "2018-06-01"), "OOS": ("2018-06-01", "2026-06-01")}
for name, (a, b) in WINS.items():
    lo = int(dt.datetime.fromisoformat(a).replace(tzinfo=dt.timezone.utc).timestamp() * 1000)
    hi = int(dt.datetime.fromisoformat(b).replace(tzinfo=dt.timezone.utc).timestamp() * 1000)
    risks, gaps, ratio = [], [], []
    for i in range(1, len(c) - 1):
        if not (lo <= ms[i] < hi):
            continue
        if ms[i] - ms[i - 1] < MIN_GAP_H * 3600000:
            continue
        a14 = atr[i - 1]
        if not np.isfinite(a14) or a14 <= 0:
            continue
        gap = o[i] - c[i - 1]
        if abs(gap) < MIN_GAP_ATR * a14:
            continue
        short = gap > 0
        stop = o[i] + gap * STOP_MULT
        # the engine fills at the next bar's open, moved by half the spread
        entry = o[i + 1] - SPREAD / 2 if short else o[i + 1] + SPREAD / 2
        risk = abs(entry - stop)
        risks.append(risk); gaps.append(abs(gap)); ratio.append(risk / abs(gap))
    r = np.array(risks)
    if len(r) == 0:
        print(name, "no signal"); continue
    r_ok = r[r > 0]
    print(f"\n### {name}  minGapHours={MIN_GAP_H} minGapAtr={MIN_GAP_ATR} stopGapMult={STOP_MULT}: {len(r)} signals")
    print(f"  |gap| median {np.median(gaps):.2f} pts | risk=|entry-stop| median {np.median(r):.3f} pts, "
          f"mean {r.mean():.3f}, min {r.min():.4f}, max {r.max():.2f}")
    print(f"  HARMONIC mean of risk = 1/mean(1/risk) = {1.0/np.mean(1.0/r_ok):.4f} pts   "
          f"(spread/risk harmonic = {100*SPREAD*np.mean(1.0/r_ok):.1f}% of R)")
    for q in (0.01, 0.05, 0.10, 0.25, 0.50):
        print(f"  q{q:<5} risk = {np.quantile(r,q):8.4f} pts   lots at 1% of 10,000 USD = {100/np.quantile(r,q):10.1f}")
    for thr in (0.01, 0.05, 0.117, 0.28, 1.0):
        print(f"  risk < {thr:5} pts: {int((r<thr).sum()):4d} of {len(r)}  "
              f"({100*(r<thr).mean():.1f}%)   lots > {100/thr:.0f}")
