"""hour-screen pre-check: hour-of-day on xauduka, 16 years, two time-split windows.

Pre-registered: docs/decisions/2026-10-10-hour-screen.md
Writes receipts to receipts/hourscreen/.
"""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, pandas as pd
from scipy import stats
from hour_screen_clock import load, DAY_MS, HOUR_MS

OUT = r'E:/rust/fd-hour-screen/receipts/hourscreen'
os.makedirs(OUT, exist_ok=True)

# window split: W1 2010-06-01 -> 2018-06-01, W2 2018-06-01 -> 2026-05-31
SPLIT_MS = pd.Timestamp('2018-06-01T00:00:00Z').value // 1_000_000

HANDPICKED = {
    'ny-morning  08:00-12:00': (8, 4),
    'london-open 02:00-06:00': (2, 4),
    'asia        19:00-02:00': (19, 7),
}
# Discovered in coverage, not assumed: the CME daily halt. On the NY clock it is
# one hour (17:00). On a FIXED UTC clock the same halt lands on 21:00 UTC in
# summer and 22:00 UTC in winter, so TWO UTC hours are each only half-covered -
# that is the DST artifact, printed as a fact rather than a choice.
BREAKS = {'ny': {17}, 'utc': {21, 22}}
BREAK_HOUR = 17


def cells(clock):
    """One row per (day, hour) with 4 complete 15m bars; NY/UTC weekdays only."""
    d = load(clock)
    d = d[(d.wd >= 1) & (d.wd <= 5)]
    g = d.groupby(['day', 'hour'])
    c = pd.DataFrame({
        'nb': g.size(),
        'o': g['open'].first(),
        'h': g['high'].max(),
        'l': g['low'].min(),
        'c': g['close'].last(),
        't0': g['utc_ms'].first(),
    }).reset_index()
    c = c[c.nb == 4].copy()
    c['disp'] = c['c'] - c['o']
    c['rng'] = c['h'] - c['l']
    # daily range in USD, from the same complete cells of that day
    day = c.groupby('day').agg(dh=('h', 'max'), dl=('l', 'min'), nh=('hour', 'size'),
                               t0=('t0', 'min'))
    day['dr'] = day.dh - day.dl
    day = day.sort_index()
    # DR20 = mean of the PREVIOUS 20 trading days' ranges (no look-ahead, not self-inclusive)
    day['dr20'] = day['dr'].shift(1).rolling(20).mean()
    day['win'] = np.where(day['t0'] < SPLIT_MS, 'W1', 'W2')
    return c, day


def hour_table(c, day):
    """Per (window, hour): signed + range, raw and day-demeaned, normalised by DR20."""
    ok = day[(day.nh >= 17) & day.dr20.notna()]
    m = c.merge(ok[['dr20', 'win']], left_on='day', right_index=True, how='inner')
    m['s'] = m.disp / m.dr20
    m['r'] = m.rng / m.dr20
    # day mean over the hours present that day
    dm = m.groupby('day')[['s', 'r']].transform('mean')
    m['sd_'] = m.s - dm['s']
    m['rd_'] = m.r - dm['r']
    rows = []
    for win, gw in m.groupby('win'):
        for hr, g in gw.groupby('hour'):
            row = {'win': win, 'hour': int(hr), 'n': len(g)}
            for name, col in (('sdem', 'sd_'), ('rdem', 'rd_'), ('sraw', 's'), ('rraw', 'r')):
                x = g[col].values
                t, p = stats.ttest_1samp(x, 0.0)
                row[name + '_mean'] = float(x.mean())
                row[name + '_t'] = float(t)
                row[name + '_p'] = float(p)
            rows.append(row)
    return pd.DataFrame(rows), ok


def win_hours(start, length):
    return [(start + i) % 24 for i in range(length)]


def window_table(c, day, ok, breaks):
    """Per (time-window, candidate session window): net displacement and range.

    Windows live on the ABSOLUTE hour axis, so a window that wraps midnight
    (19:00 -> 02:00) is one contiguous stretch of clock time, not two ends of
    the same calendar day.
    """
    cc = c.copy()
    cc['ah'] = cc.day * 24 + cc.hour          # absolute hour index on this clock
    cc = cc.set_index('ah').sort_index()
    lo, hi = int(cc.index.min()), int(cc.index.max())
    full = pd.RangeIndex(lo, hi + 1)
    O, H, L, C = (cc[k].reindex(full) for k in ('o', 'h', 'l', 'c'))
    dr20 = ok['dr20']; win = ok['win']
    rows = []
    for length in (4, 7):
        for start in range(24):
            hs = win_hours(start, length)
            offs = [i for i in range(length) if hs[i] not in breaks]
            days = ok.index.values
            base = days * 24 + start        # anchor on the DAY, not on a cell
            ok_mask = np.ones(len(base), dtype=bool)
            his, los = [], []
            for i in offs:
                idx = base + i
                inrange = (idx >= lo) & (idx <= hi)
                h_ = np.where(inrange, H.reindex(np.clip(idx, lo, hi)).values, np.nan)
                l_ = np.where(inrange, L.reindex(np.clip(idx, lo, hi)).values, np.nan)
                ok_mask &= np.isfinite(h_)
                his.append(h_); los.append(l_)
            o_ = O.reindex(base + offs[0]).values
            c_ = C.reindex(base + offs[-1]).values
            ok_mask &= np.isfinite(o_) & np.isfinite(c_)
            d20 = dr20.reindex(days).values
            wv = win.reindex(days).values
            ok_mask &= np.isfinite(d20)
            disp = (c_ - o_) / d20
            rng = (np.nanmax(np.vstack(his), axis=0) - np.nanmin(np.vstack(los), axis=0)) / d20
            for w in ('W1', 'W2'):
                sel = ok_mask & (wv == w)
                n = int(sel.sum())
                dv = disp[sel]; rv = rng[sel]
                td, pd_ = stats.ttest_1samp(dv, 0.0) if n > 2 else (np.nan, np.nan)
                rows.append({'len': length, 'start': start, 'win': w, 'n': n,
                             'hours': ' '.join(f'{h:02d}' for h in hs),
                             'disp_mean': float(dv.mean()) if n else np.nan,
                             'disp_t': float(td), 'disp_p': float(pd_),
                             'rng_mean': float(rv.mean()) if n else np.nan,
                             'spans_break': bool(set(hs) & breaks)})
    return pd.DataFrame(rows)


def fmt(x, nd=4):
    if x is None or (isinstance(x, float) and not np.isfinite(x)):
        return 'null'
    return f'{x:.{nd}f}'.replace('.', ',')


def report(clock, f):
    c, day = cells(clock)
    ht, ok = hour_table(c, day)
    breaks = BREAKS[clock]
    wt = window_table(c, day, ok, breaks)
    p = lambda *a: print(*a, file=f)
    p(f'\n{"="*96}\nCLOCK = {clock.upper()}   (NY = exact fd_core::clock DST rule; UTC = fixed offset)')
    p(f'{"="*96}')
    p(f'valid trading days (>=17 complete hour cells, DR20 available): '
      f'W1 {int((ok.win=="W1").sum())}  W2 {int((ok.win=="W2").sum())}')
    p(f'first/last day W1: {pd.to_datetime(ok[ok.win=="W1"].t0.min(), unit="ms", utc=True).date()}'
      f' .. {pd.to_datetime(ok[ok.win=="W1"].t0.max(), unit="ms", utc=True).date()}')
    p(f'first/last day W2: {pd.to_datetime(ok[ok.win=="W2"].t0.min(), unit="ms", utc=True).date()}'
      f' .. {pd.to_datetime(ok[ok.win=="W2"].t0.max(), unit="ms", utc=True).date()}')

    # ---- per-hour table
    for w in ('W1', 'W2'):
        g = ht[ht.win == w].sort_values('hour')
        p(f'\n-- {w}: per NY hour, units = fraction of DR20 (20-day mean NY daily range, USD) --')
        p(f'{"hr":>3} {"n":>5} | {"sgn-dem":>9} {"t":>7} {"p":>7} | {"rng-dem":>9} {"t":>7} {"p":>7} '
          f'| {"sgn-raw":>9} {"t":>7} {"p":>7} | {"rng-raw":>8}')
        for _, r in g.iterrows():
            flag = '  <- CME daily halt falls here (partial coverage)' if r.hour in breaks else ''
            floor = '  BELOW 200-day FLOOR => null' if r.n < 200 else ''
            p(f'{int(r.hour):>3} {int(r.n):>5} | {fmt(r.sdem_mean):>9} {fmt(r.sdem_t,2):>7} {fmt(r.sdem_p,3):>7} '
              f'| {fmt(r.rdem_mean):>9} {fmt(r.rdem_t,2):>7} {fmt(r.rdem_p,3):>7} '
              f'| {fmt(r.sraw_mean):>9} {fmt(r.sraw_t,2):>7} {fmt(r.sraw_p,3):>7} | {fmt(r.rraw_mean):>8}'
              f'{flag}{floor}')

    # ---- the screen: same sign AND p<0.05 in BOTH windows
    p('\n-- THE SCREEN: same sign AND p<0,05 in BOTH windows (per hour, per quantity) --')
    a = ht[ht.win == 'W1'].set_index('hour')
    b = ht[ht.win == 'W2'].set_index('hour')
    survivors = []
    counts = {}
    for q, label in (('sdem', 'signed (day-demeaned)'), ('rdem', 'range (day-demeaned)'),
                     ('sraw', 'signed (raw vs 0)')):
        hits1 = hits2 = samesign = both = 0
        for h in range(24):
            if h not in a.index or h not in b.index:
                continue
            if a.loc[h, 'n'] < 200 or b.loc[h, 'n'] < 200:
                continue
            m1, p1 = a.loc[h, q + '_mean'], a.loc[h, q + '_p']
            m2, p2 = b.loc[h, q + '_mean'], b.loc[h, q + '_p']
            hits1 += p1 < 0.05
            hits2 += p2 < 0.05
            ss = np.sign(m1) == np.sign(m2)
            samesign += ss
            if ss and p1 < 0.05 and p2 < 0.05:
                both += 1
                survivors.append((clock, q, h, m1, p1, m2, p2))
        counts[q] = dict(hits_W1=int(hits1), hits_W2=int(hits2), same_sign=int(samesign),
                         survivors=int(both))
        p(f'   {label:<26}: p<0,05 in W1 {hits1}/24, in W2 {hits2}/24, same sign {samesign}/24, '
          f'SURVIVE BOTH {both}/24')
    if survivors:
        p('   survivors:')
        for s in survivors:
            p(f'     hour {s[2]:02d} {s[1]}: W1 mean {fmt(s[3])} p {fmt(s[4],4)} | '
              f'W2 mean {fmt(s[5])} p {fmt(s[6],4)}')
    else:
        p('   survivors: NONE')

    # ---- rankings of the 24 hours
    p('\n-- RANKING of the 24 hours (1 = largest). Hour 17 kept but flagged. --')
    for w in ('W1', 'W2'):
        g = ht[ht.win == w].copy()
        g['rk_rng'] = g.rraw_mean.rank(ascending=False).astype(int)
        g['rk_absS'] = g.sraw_mean.abs().rank(ascending=False).astype(int)
        g['rk_sdem'] = g.sdem_mean.rank(ascending=False).astype(int)
        p(f'   {w} widest hours (rng-raw):  ' +
          '  '.join(f'{int(r.hour):02d}({fmt(r.rraw_mean,3)})'
                    for _, r in g.sort_values('rraw_mean', ascending=False).head(6).iterrows()))
        p(f'   {w} narrowest hours:         ' +
          '  '.join(f'{int(r.hour):02d}({fmt(r.rraw_mean,3)})'
                    for _, r in g.sort_values('rraw_mean').head(6).iterrows()))
        p(f'   {w} largest |signed| hours:  ' +
          '  '.join(f'{int(r.hour):02d}({fmt(r.sraw_mean,4)})'
                    for _, r in g.reindex(g.sraw_mean.abs().sort_values(ascending=False).index).head(6).iterrows()))

    # per-hour ranks of the hand-picked constituents
    p('\n-- rank of each HAND-PICKED window\'s constituent hours, out of 24 --')
    for name, (start, length) in HANDPICKED.items():
        hs = win_hours(start, length)
        for w in ('W1', 'W2'):
            g = ht[ht.win == w].copy().set_index('hour')
            rr = g.rraw_mean.rank(ascending=False)
            rs = g.sraw_mean.abs().rank(ascending=False)
            p(f'   {name}  {w}: range rank ' +
              ' '.join(f'{h:02d}->{int(rr[h])}' for h in hs if h in rr.index) +
              '   | |signed| rank ' +
              ' '.join(f'{h:02d}->{int(rs[h])}' for h in hs if h in rs.index))

    # ---- window-level ranking: 24 contiguous candidates of the same length
    p('\n-- WINDOW-LEVEL: each hand-picked window against ALL 24 contiguous windows of its length --')
    for name, (start, length) in HANDPICKED.items():
        for w in ('W1', 'W2'):
            g = wt[(wt.len == length) & (wt.win == w)].copy()
            g['rk_rng'] = g.rng_mean.rank(ascending=False).astype(int)
            g['rk_disp'] = g.disp_mean.abs().rank(ascending=False).astype(int)
            me = g[g.start == start].iloc[0]
            top_r = g.sort_values('rng_mean', ascending=False).head(3)
            top_d = g.reindex(g.disp_mean.abs().sort_values(ascending=False).index).head(3)
            p(f'   {name}  {w}  n={int(me.n)}')
            p(f'      range  mean {fmt(me.rng_mean,3)} DR20  -> rank {int(me.rk_rng)}/24   '
              f'(top3: ' + ', '.join(f'{int(r.start):02d}h+{length} {fmt(r.rng_mean,3)}' for _, r in top_r.iterrows()) + ')')
            p(f'      |disp| mean {fmt(me.disp_mean,4)} DR20 (t {fmt(me.disp_t,2)}, p {fmt(me.disp_p,3)}) -> rank {int(me.rk_disp)}/24   '
              f'(top3: ' + ', '.join(f'{int(r.start):02d}h+{length} {fmt(r.disp_mean,4)}' for _, r in top_d.iterrows()) + ')')

    # full window tables
    for length in (4, 7):
        p(f'\n-- all 24 contiguous {length}-hour windows (start hour, NY) --')
        p(f'{"start":>5} {"n W1":>6} {"rngW1":>7} {"dispW1":>8} {"rkR1":>5} {"rkD1":>5} | '
          f'{"n W2":>6} {"rngW2":>7} {"dispW2":>8} {"rkR2":>5} {"rkD2":>5}  break')
        g1 = wt[(wt.len == length) & (wt.win == 'W1')].set_index('start')
        g2 = wt[(wt.len == length) & (wt.win == 'W2')].set_index('start')
        r1 = g1.rng_mean.rank(ascending=False); d1 = g1.disp_mean.abs().rank(ascending=False)
        r2 = g2.rng_mean.rank(ascending=False); d2 = g2.disp_mean.abs().rank(ascending=False)
        for s in range(24):
            p(f'{s:>5} {int(g1.loc[s,"n"]):>6} {fmt(g1.loc[s,"rng_mean"],3):>7} {fmt(g1.loc[s,"disp_mean"],4):>8} '
              f'{int(r1[s]):>5} {int(d1[s]):>5} | {int(g2.loc[s,"n"]):>6} {fmt(g2.loc[s,"rng_mean"],3):>7} '
              f'{fmt(g2.loc[s,"disp_mean"],4):>8} {int(r2[s]):>5} {int(d2[s]):>5}  '
              f'{"YES" if g1.loc[s,"spans_break"] else ""}')
    return ht, wt, counts, ok


if __name__ == '__main__':
    with open(os.path.join(OUT, 'hour-screen.txt'), 'w', encoding='utf-8') as f:
        print('hour-screen pre-check | XAUDUKA-15m 378.749 bars 2010-06-01 -> 2026-05-31', file=f)
        print('normaliser: DR20 = mean NY daily range (USD) of the PREVIOUS 20 trading days', file=f)
        print('W1 = 2010-06-01 -> 2018-06-01 | W2 = 2018-06-01 -> 2026-05-31', file=f)
        print('weekdays only (Mon-Fri on the stated clock); an hour cell needs all 4 of its 15m bars', file=f)
        allc = {}
        for clock in ('ny', 'utc'):
            ht, wt, counts, ok = report(clock, f)
            ht.to_csv(os.path.join(OUT, f'hours-{clock}.csv'), index=False)
            wt.to_csv(os.path.join(OUT, f'windows-{clock}.csv'), index=False)
            allc[clock] = counts
        print('\n' + '=' * 96, file=f)
        print('MULTIPLE-TEST LEDGER (declared 288 p-values before the first line of code)', file=f)
        print(json.dumps(allc, indent=2), file=f)
    print(open(os.path.join(OUT, 'hour-screen.txt'), encoding='utf-8').read())
