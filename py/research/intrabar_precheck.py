"""P2 of docs/decisions/2026-10-07-intrabar-resolution.md: are XAUDUKA-15m and
XAUDUKA-1m the same tape? A 15m high must be the max of its 1m highs."""
import numpy as np
from intrabar_bars import load

t15, o15, h15, l15, c15 = load("XAUDUKA-15m")
t1, o1, h1, l1, c1 = load("XAUDUKA-1m")

def span(t):
    return f"{np.datetime64(int(t[0]), 'ms')} -> {np.datetime64(int(t[-1]), 'ms')}"

print(f"15m rows {len(t15)}  span {span(t15)}")
print(f"1m  rows {len(t1)}  span {span(t1)}")
assert np.all(np.diff(t15) > 0), "15m times not strictly increasing"
assert np.all(np.diff(t1) > 0), "1m times not strictly increasing"
print(f"15m bar spacing: median {int(np.median(np.diff(t15)))} ms, "
      f"{np.sum(np.diff(t15) != 900_000)} of {len(t15)-1} gaps != 15m")
print(f"1m  bar spacing: median {int(np.median(np.diff(t1)))} ms, "
      f"{np.sum(np.diff(t1) != 60_000)} of {len(t1)-1} gaps != 1m")

rng = np.random.default_rng(20261007)
idx = np.sort(rng.choice(len(t15), size=20000, replace=False))
start = np.searchsorted(t1, t15[idx], side="left")
end = np.searchsorted(t1, t15[idx] + 900_000, side="left")

missing = fail_h = fail_l = fail_o = 0
worst_h = worst_l = worst_o = 0.0
nbars = []
for s, e, hi, lo, op in zip(start, end, h15[idx], l15[idx], o15[idx]):
    if e <= s:
        missing += 1
        continue
    nbars.append(e - s)
    dh = abs(h1[s:e].max() - hi)
    dl = abs(l1[s:e].min() - lo)
    do = abs(o1[s] - op)
    worst_h = max(worst_h, dh); worst_l = max(worst_l, dl); worst_o = max(worst_o, do)
    fail_h += dh > 0.01
    fail_l += dl > 0.01
    fail_o += do > 0.01

n = len(nbars)
nb = np.array(nbars)
print(f"\nsampled 20000 15m bars: {n} had 1m bars under them, {missing} had none")
print(f"1m bars per 15m bar: min {nb.min()} median {int(np.median(nb))} max {nb.max()}")
print(f"high mismatch > 0.01 pts: {fail_h} of {n} = {100*fail_h/n:.3f}%  worst {worst_h:.4f} pts")
print(f"low  mismatch > 0.01 pts: {fail_l} of {n} = {100*fail_l/n:.3f}%  worst {worst_l:.4f} pts")
print(f"open mismatch > 0.01 pts: {fail_o} of {n} = {100*fail_o/n:.3f}%  worst {worst_o:.4f} pts")
bad = 100 * max(fail_h, fail_l) / n
print(f"\nP2 verdict: {'PASS' if bad <= 0.5 else 'FAIL'} "
      f"(threshold 0.5% of sampled bars; got {bad:.3f}%)")
