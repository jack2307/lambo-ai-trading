"""PC1 probe: is `underlying_price` on the gold tape ONE price series, or
several contract months interleaved? And what is the GC-1m vs XAUUSD-15m basis
on the ACTUAL overlap window, which is the thing the gate is read on.

Read-only. Nothing is written to /e/rust/flowdesk.
"""
import glob
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
import statistics as st

import pyarrow.parquet as pq

TAPE = r'E:/rust/flowdesk/data/gold/tape'
BARS = r'E:/rust/flowdesk/data/bars'


def quants(v):
    v = sorted(v)
    n = len(v)
    if n == 0:
        return None
    q = lambda p: v[min(n - 1, int(round((n - 1) * p)))]
    return dict(n=n, mean=sum(v) / n,
                sd=(st.pstdev(v) if n > 1 else 0.0),
                mn=v[0], p10=q(.10), p50=q(.50), p90=q(.90), mx=v[-1])


def show(label, d):
    if d is None:
        print(f'{label}: null (no values)')
        return
    print(f'{label}: n {d["n"]} mean {d["mean"]:+.2f} sd {d["sd"]:.2f} '
          f'min {d["mn"]:+.2f} p10 {d["p10"]:+.2f} p50 {d["p50"]:+.2f} '
          f'p90 {d["p90"]:+.2f} max {d["mx"]:+.2f}')


# ---------- 1. the tape's own underlying_price, by symbol and by minute ----
files = sorted(glob.glob(TAPE + '/date=*/*.parquet'))
cols = ['timestamp', 'symbol', 'underlying_price', 'expiration', 'strike']
rows = []
for f in files:
    t = pq.read_table(f, columns=cols)
    ts = t.column('timestamp').to_pylist()
    sy = t.column('symbol').to_pylist()
    up = t.column('underlying_price').to_pylist()
    ex = t.column('expiration').to_pylist()
    for a, b, c, d in zip(ts, sy, up, ex):
        rows.append((int(a.timestamp() * 1000), b, c, int(d.timestamp() * 1000)))
print(f'tape prints read: {len(rows)}  files {len(files)}')

symbols = {}
for t, s, u, e in rows:
    symbols.setdefault(s, []).append(u)
print(f'distinct expiry symbols: {len(symbols)}')
print()
print('--- underlying_price per expiry symbol (if these differ, `spot` is not one series) ---')
for s in sorted(symbols, key=lambda k: -len(symbols[k]))[:12]:
    show(f'  {s:<22} prints {len(symbols[s]):>6}', quants(symbols[s]))

# Spread of underlying_price WITHIN the same minute: the direct measurement.
by_min = {}
for t, s, u, e in rows:
    by_min.setdefault(t // 60000, []).append(u)
spreads = [max(v) - min(v) for v in by_min.values() if len(v) > 1]
print()
print(f'--- same-MINUTE spread of underlying_price, over {len(spreads)} minutes with >1 print ---')
show('  spread (max-min) USD', quants(spreads))
nonzero = sum(1 for s in spreads if s > 0.01)
print(f'  minutes where it is > 0.01 USD: {nonzero} of {len(spreads)} '
      f'= {100.0 * nonzero / max(1, len(spreads)):.1f}%')
big = sum(1 for s in spreads if s > 5.0)
print(f'  minutes where it is > 5.00 USD: {big} = {100.0 * big / max(1, len(spreads)):.1f}%')

# ---------- 2. GC-1m vs XAUUSD-15m on the real overlap ----------
gc = pq.read_table(BARS + '/GC-1m.parquet')
xa = pq.read_table(BARS + '/XAUUSD-15m.parquet')
print()
print('--- bar series ---')
print('GC-1m   ', gc.num_rows, 'rows, columns', gc.schema.names)


def bars_of(t):
    ts = t.column('time' if 'time' in t.schema.names else 'timestamp').to_pylist()
    cl = t.column('close').to_pylist()
    op = t.column('open').to_pylist()
    hi = t.column('high').to_pylist()
    lo = t.column('low').to_pylist()
    out = []
    for a, o, h, l, c in zip(ts, op, hi, lo, cl):
        ms = int(a.timestamp() * 1000) if hasattr(a, 'timestamp') else int(a)
        out.append((ms, o, h, l, c))
    out.sort()
    return out


gcb = bars_of(gc)
xab = bars_of(xa)
print(f'GC-1m    {len(gcb)} bars  {gcb[0][0]} -> {gcb[-1][0]}')
print(f'XAUUSD15 {len(xab)} bars  {xab[0][0]} -> {xab[-1][0]}')
rangeless = sum(1 for _, o, h, l, c in gcb if o == h == l == c)
print(f'GC-1m rangeless (o==h==l==c): {rangeless} of {len(gcb)} '
      f'= {100.0 * rangeless / len(gcb):.1f}%')

gcmap = {}
for ms, o, h, l, c in gcb:
    gcmap[ms] = c

# XAUUSD-15m bar at time T closes at T+15m. Pair it with the GC minute bar
# stamped at T+14m (the last GC minute inside the 15m bar).
pairs = []
for ms, o, h, l, c in xab:
    k = ms + 14 * 60000
    if k in gcmap:
        pairs.append((ms, gcmap[k] - c))
print()
print(f'--- GC-1m close MINUS XAUUSD-15m close, paired inside the same 15m bar ---')
print(f'  paired bars: {len(pairs)}')
vals = [v for _, v in pairs]
show('  basis USD', quants(vals))
if len(pairs) >= 4:
    n = len(pairs)
    qm = []
    for k in range(4):
        a, b = n * k // 4, n * (k + 1) // 4
        qm.append(sum(v for _, v in pairs[a:b]) / (b - a))
    print('  quartile means in TIME order: ' + ' -> '.join(f'{v:+.2f}' for v in qm))
    print(f'  first paired bar {pairs[0][0]}  last {pairs[-1][0]}')
