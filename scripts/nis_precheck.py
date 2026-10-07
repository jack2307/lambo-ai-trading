import numpy as np, pyarrow.parquet as pq, pyarrow.compute as pc, datetime as dt

DATA = 'E:/rust/flowdesk/data'

def ms(s):
    return int(dt.datetime.fromisoformat(s).replace(tzinfo=dt.timezone.utc).timestamp()*1000)

WIN = {"A'": (ms('2022-01-01'), ms('2026-01-01')), "B'": (ms('2018-01-01'), ms('2022-01-01'))}

ev = pq.read_table(f'{DATA}/news/events.parquet')
names = ev.column_names
print('news columns:', names, 'rows:', ev.num_rows)
et = ev.column('time').to_pylist()
et = [int(x.timestamp()*1000) if hasattr(x,'timestamp') else int(x) for x in et]
ec = ev.column('currency').to_pylist()
ei = ev.column('impact').to_pylist()
usd3 = sorted(t for t,c,i in zip(et,ec,ei) if c and c.upper()=='USD' and i is not None and i>=3)
print('USD impact>=3 events total:', len(usd3),
      dt.datetime.utcfromtimestamp(usd3[0]/1000), '->', dt.datetime.utcfromtimestamp(usd3[-1]/1000))
for w,(a,b) in WIN.items():
    print(f'  {w}: {sum(1 for t in usd3 if a<=t<b)} events')

def rma(x, n):
    out = np.full(len(x), np.nan)
    if len(x) < n: return out
    out[n-1] = np.mean(x[:n])
    for i in range(n, len(x)):
        out[i] = (out[i-1]*(n-1) + x[i]) / n
    return out

for interval, bar_ms in (('1m', 60_000), ('5m', 300_000)):
    t = pq.read_table(f'{DATA}/bars/XAUDUKA-{interval}.parquet')
    time = np.array([int(x.timestamp()*1000) for x in t.column('time').to_pylist()], dtype=np.int64)
    o = np.asarray(t.column('open').to_pylist(), dtype=float)
    h = np.asarray(t.column('high').to_pylist(), dtype=float)
    l = np.asarray(t.column('low').to_pylist(), dtype=float)
    c = np.asarray(t.column('close').to_pylist(), dtype=float)
    tr = np.empty(len(c)); tr[0] = h[0]-l[0]
    pc_ = c[:-1]
    tr[1:] = np.maximum(h[1:]-l[1:], np.maximum(np.abs(h[1:]-pc_), np.abs(l[1:]-pc_)))
    atr = rma(tr, 14)
    print(f'\n=== XAUDUKA-{interval}: {len(c)} bars ===')
    for w,(a,b) in WIN.items():
        anchored = 0; skipped_lag = 0; no_bar = 0
        rows = {1: [], 2: []}
        for e in usd3:
            if not (a <= e < b): continue
            j = int(np.searchsorted(time, e, side='left'))
            if j <= 0 or j >= len(c): no_bar += 1; continue
            if time[j-1] >= e: no_bar += 1; continue
            if time[j] - e > 15*60_000: skipped_lag += 1; continue
            if not np.isfinite(atr[j-1]) or atr[j-1] <= 0: continue
            anchored += 1
            for probe in (1, 2):
                i = j + probe - 1
                if i >= len(c): continue
                # contiguity of the probe window
                if any(time[k+1]-time[k] != bar_ms for k in range(j, i)): continue
                rng = h[j:i+1].max() - l[j:i+1].min()
                impulse = c[i] - o[j]
                rows[probe].append((rng, atr[j-1], abs(impulse)))
        print(f' {w}: anchored {anchored}, lag>15min {skipped_lag}, no first-bar {no_bar}')
        for probe in (1, 2):
            r = np.array([x[0] for x in rows[probe]]); av = np.array([x[1] for x in rows[probe]])
            imp = np.array([x[2] for x in rows[probe]])
            if len(r) == 0: print('   probe', probe, 'null'); continue
            ratio = r/av
            gate_pass = int((imp >= 0.5*av).sum())
            print(f'   probe {probe}: n={len(r)}  range median {np.median(r):.2f} pts'
                  f'  = {np.median(ratio):.3f} ATR({interval})   ATR median {np.median(av):.3f} pts'
                  f'  | pass 0.5ATR gate: {gate_pass}/{len(r)}')
            for f in (0.25, 0.50, 0.75, 1.00):
                stop_pts = np.median(r)*f
                stop_atr = np.median(ratio)*f
                print(f'      f={f:.2f}: stop {stop_pts:6.2f} pts = {stop_atr:6.3f} ATR({interval})'
                      f'   cost/R = {0.28/stop_pts*100:5.2f}%   target(1.8R) = {stop_pts*1.8:6.2f} pts'
                      f' = {stop_pts*1.8/np.median(r):.2f}x the range')
