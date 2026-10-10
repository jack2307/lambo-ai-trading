"""hour-screen: the halt-boundary cross-feed check and the stretch decomposition."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hour_screen_clock import ny_offset_ms, DAY_MS, HOUR_MS
import numpy as np, pandas as pd
from scipy import stats
import hour_screen as R
OUT = r'E:/rust/fd-hour-screen/receipts/hourscreen'
SP = 0.28

def prep(sym):
    df = pd.read_parquet(f'E:/rust/flowdesk/data/bars/{sym}.parquet',
                         columns=['time','open','high','low','close'])
    utc = df['time'].values.astype('datetime64[ms]').astype(np.int64)
    loc = utc + ny_offset_ms(utc)
    df = df.drop(columns=['time']); df['utc_ms'] = utc
    df['day'] = np.floor_divide(loc, DAY_MS); df['hour'] = np.floor_divide(loc % DAY_MS, HOUR_MS)
    df['wd'] = (df['day'] + 4) % 7
    return df.sort_values('utc_ms').reset_index(drop=True)

with open(os.path.join(OUT, 'halt-and-stretches.txt'), 'w', encoding='utf-8') as f:
    pr = lambda *a: print(*a, file=f)
    pr('hour-screen | the two cross-checks that decide how to read the signed survivors')
    pr('')
    pr('=== 1. HALT-BOUNDARY MARK: provider property or market? ===')
    pr('"re-open bar" = the first 15m bar after a 60-200 min gap in the tape (the daily halt,')
    pr('not the weekend). Compared against every ordinary 15m bar (dt == 15 min) of the same feed.')
    for sym in ('XAUDUKA-15m', 'XAGDUKA-15m', 'XAUUSD-15m'):
        d = prep(sym); d['dt'] = d.utc_ms.diff()/60000
        ri = d.index[(d.dt >= 60) & (d.dt <= 200)]; ri = ri[ri > 0]
        r = d.loc[ri]; p = d.loc[ri-1]; o = d[d.dt == 15]
        pr('')
        pr(f'{sym}  {len(d)} bars  {pd.to_datetime(d.utc_ms.min(),unit="ms").date()} .. '
           f'{pd.to_datetime(d.utc_ms.max(),unit="ms").date()}   re-opens {len(r)}  '
           f'modal NY hour {int(r.hour.mode().iloc[0])}')
        pr(f'   re-open bar  mean(close-open) {(r.close-r.open).mean():+.4f} price units   '
           f'(ordinary bar {(o.close-o.open).mean():+.5f})')
        pr(f'   re-open bar  open==low {100*(r.open==r.low).mean():5.2f}%  open==high '
           f'{100*(r.open==r.high).mean():5.2f}%   | ordinary  open==low {100*(o.open==o.low).mean():5.2f}%  '
           f'open==high {100*(o.open==o.high).mean():5.2f}%')
        pr(f'   pre-halt bar mean(close-open) {(p.close-p.open).mean():+.4f}   '
           f'close==low {100*(p.close==p.low).mean():5.2f}%  close==high {100*(p.close==p.high).mean():5.2f}%'
           f'   | ordinary  close==low {100*(o.close==o.low).mean():5.2f}%  close==high {100*(o.close==o.high).mean():5.2f}%')

    pr('')
    pr('=== 2. OVERNIGHT 19:00 -> 02:00 NY, three feeds ===')
    for sym, wins in (('XAUDUKA-15m', [('W1 2010-06..2018-06','2010-06-01','2018-06-01'),
                                       ('W2 2018-06..2026-05','2018-06-01','2026-06-01')]),
                      ('XAUUSD-15m',  [('broker 2022-06..2026-09','2022-06-16','2026-09-18')]),
                      ('XAGDUKA-15m', [('W1 2010-06..2018-06','2010-06-01','2018-06-01'),
                                       ('W2 2018-06..2026-05','2018-06-01','2026-06-01')])):
        d = prep(sym)
        g = d.groupby(['day','hour']).size().rename('nb'); d = d.join(g, on=['day','hour'])
        full = d[(d.nb == 4) & (d.wd >= 1) & (d.wd <= 5)]
        day = full.groupby('day').agg(dh=('high','max'), dl=('low','min'), t0=('utc_ms','min'),
                                      nh=('hour','nunique'))
        day['dr'] = day.dh - day.dl; day = day.sort_index()
        day['dr20'] = day['dr'].shift(1).rolling(20).mean()
        cc = full.groupby(['day','hour']).agg(o=('open','first'), c=('close','last')).reset_index()
        cc['ah'] = cc.day*24 + cc.hour
        C = cc.set_index('ah')['c']; O = cc.set_index('ah')['o']
        pr(''); pr(f'{sym}')
        for name, a, b in wins:
            lo = pd.Timestamp(a+'T00:00:00Z').value//1_000_000
            hi = pd.Timestamp(b+'T00:00:00Z').value//1_000_000
            sel = day[(day.t0 >= lo) & (day.t0 < hi) & day.dr20.notna() & (day.nh >= 17)]
            base = sel.index.values*24 + 19
            okm = np.ones(len(base), bool)
            for i in range(7):
                okm &= np.isfinite(C.reindex(base+i).values)
            disp = ((C.reindex(base+6).values - O.reindex(base).values)/sel.dr20.values)[okm]
            dr = sel.dr20.values[okm].mean()
            t, pv = stats.ttest_1samp(disp, 0.0)
            pr(f'   {name:26s} n={len(disp):5d}  mean {disp.mean():+.5f} DR20  t {t:+.2f}  p {pv:.4f}'
               f'   DR20 mean {dr:.3f}  => {disp.mean()*dr:+.4f} price units'
               f'  = {disp.mean()*dr/SP:+.2f} x one round trip (0,28 USD, gold only)')

    pr('')
    pr('=== 3. WHERE THE 16-YEAR DRIFT LIVES: stretches of the NY clock (xauduka) ===')
    c, day = R.cells('ny'); ht, ok = R.hour_table(c, day)
    for w in ('W1', 'W2'):
        dr = ok[ok.win == w].dr20.mean(); g = ht[ht.win == w].set_index('hour')
        pr(''); pr(f' {w}  (mean DR20 {dr:.3f} USD, {int((ok.win==w).sum())} days)')
        for n, hs in (('overnight 19-01 (the batch\'s asia hours)', [19,20,21,22,23,0,1]),
                      ('day 02-16 (London + NY)', list(range(2,17))),
                      ('NY morning 08-11 (the batch\'s ny-morning)', [8,9,10,11]),
                      ('halt boundary 16+17+18', [16,17,18]),
                      ('every hour present', sorted(g.index.tolist()))):
            v = sum(float(g.loc[h,'sraw_mean']) for h in hs if h in g.index)
            pr(f'   {n:42s} {v:+.5f} DR20  {v*dr:+.4f} USD  {v*dr/SP:+.2f} x spread')
print(open(os.path.join(OUT,'halt-and-stretches.txt'), encoding='utf-8').read()[:400])
