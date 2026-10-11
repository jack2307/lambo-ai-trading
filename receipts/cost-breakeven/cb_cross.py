# -*- coding: utf-8 -*-
# Where each leg CROSSES its gate line, by linear interpolation on the measured
# grid (labelled as interpolation; the grid step is 0.04). Also the linearity of
# E(s) and PF_r(s) in s, so the interpolation is not taken on faith.
import json
SC = r'C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/cb/'
T = json.load(open(SC + 'final.json'))
ROWS = ['qs-lb3', 'ts-l5', 'ts-l20', 'ts-l10', 'qs-lb30', 'tsmom',
        'qs-h15', 'qs-qp020', 'qs-h30', 'qs-h2', 'ovn1900/long',
        'px-1s', 'ovn1900/short', 'ema-cross']


def r2lin(xs, ys):
    n = len(xs)
    mx = sum(xs) / n
    my = sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    k = sxy / sxx
    a = my - k * mx
    sst = sum((y - my) ** 2 for y in ys)
    ssr = sum((y - (a + k * x)) ** 2 for x, y in zip(xs, ys))
    return a, k, (1 - ssr / sst if sst > 0 else float('nan'))


def crossing(tab, key, line):
    # tab is ordered s descending; find s where value crosses `line` upward as s falls
    pts = sorted(tab, key=lambda x: -x['s'])
    for i in range(len(pts) - 1):
        a, b = pts[i], pts[i + 1]
        if (a[key] - line) * (b[key] - line) <= 0 and a[key] != b[key]:
            t = (line - a[key]) / (b[key] - a[key])
            return a['s'] + t * (b['s'] - a['s'])
    return None


def f(v, fmt='%+.4f'):
    return (fmt % v) if v is not None else 'null'


print('LINEARITY of E and PF_r in s over the 14 measured points, and the INTERPOLATED crossings')
print('(grid step 0.04 USD per round trip; crossings between grid points are interpolation, labelled as such)')
print('')
print('%-14s %-4s | %8s %8s | %10s %10s | %9s %9s | %8s' % (
    'row', 'win', 'R2 E~s', 'R2 PF~s', 'dE/ds(R)', 'cost/R@.28', 's: E=0.050', 's: PFr=1.2', 's*both'))
for r in ROWS:
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        if not tab:
            continue
        xs = [x['s'] for x in tab]
        aE, kE, r2E = r2lin(xs, [x['E'] for x in tab])
        aP, kP, r2P = r2lin(xs, [x['pf_r'] for x in tab])
        z = [x for x in tab if abs(x['s']) < 1e-9][0]
        c28 = [x for x in tab if abs(x['s'] - 0.28) < 1e-9][0]
        cost = z['E'] - c28['E']
        sE = crossing(tab, 'E', 0.050)
        sP = crossing(tab, 'pf_r', 1.200)
        both = None
        if sE is not None and sP is not None:
            both = min(sE, sP)
        elif sE is None and sP is not None and z['E'] >= 0.050:
            both = sP
        elif sP is None and sE is not None and z['pf_r'] >= 1.200:
            both = sE
        print('%-14s %-4s | %8.5f %8.5f | %+10.5f %10.4f | %9s %9s | %8s' % (
            r, w, r2E, r2P, kE, cost, f(sE, '%+.4f'), f(sP, '%+.4f'), f(both, '%+.4f')))

print('')
print('BOTH-WINDOW INTERPOLATED spread* = min over the two windows of the both-leg crossing')
print('%-14s %12s %12s %10s %14s' % ('row', 's*_IS', 's*_OOS', 's*', 'as % of 0.28'))
for r in ROWS:
    vals = {}
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        if not tab:
            continue
        z = [x for x in tab if abs(x['s']) < 1e-9][0]
        sE = crossing(tab, 'E', 0.050)
        sP = crossing(tab, 'pf_r', 1.200)
        both = None
        if sE is not None and sP is not None:
            both = min(sE, sP)
        elif sE is None and sP is not None and z['E'] >= 0.050:
            both = sP
        elif sP is None and sE is not None and z['pf_r'] >= 1.200:
            both = sE
        vals[w] = both
    if 'IS' in vals and 'OOS' in vals:
        if vals['IS'] is None or vals['OOS'] is None:
            st = None
        else:
            st = min(vals['IS'], vals['OOS'])
        print('%-14s %12s %12s %10s %14s' % (
            r, f(vals['IS'], '%+.4f'), f(vals['OOS'], '%+.4f'), f(st, '%+.4f'),
            ('%.1f%%' % (100 * st / 0.28)) if st is not None else 'null'))


def leg_star(tab, key, line):
    """Three cases: passes already at the config spread (+0.28) -> '>=+0.28';
    crosses inside the measured grid -> the interpolated s; never passes even at
    -0.28 -> '<-0.28'. No null is printed for a case that is a known inequality."""
    top = [x for x in tab if abs(x['s'] - 0.28) < 1e-9][0]
    bot = [x for x in tab if abs(x['s'] + 0.28) < 1e-9][0]
    if top[key] >= line:
        return ('ge', 0.28)
    if bot[key] < line:
        return ('lt', -0.28)
    c = crossing(tab, key, line)
    return ('x', c)


def both_star(tab):
    a = leg_star(tab, 'E', 0.050)
    b = leg_star(tab, 'pf_r', 1.200)
    if a[0] == 'lt' or b[0] == 'lt':
        return ('lt', -0.28)
    if a[0] == 'ge' and b[0] == 'ge':
        return ('ge', 0.28)
    return ('x', min(a[1], b[1]))


def show(t):
    if t[0] == 'ge':
        return '>= +0.28'
    if t[0] == 'lt':
        return '<  -0.28'
    return '%+.4f' % t[1]


print('')
print('THREE-CASE spread* PER LEG AND PER WINDOW (>= +0.28 means the leg already passes at the config spread)')
print('%-14s %-4s | %10s %10s %10s' % ('row', 'win', 'E leg', 'PF leg', 'both legs'))
for r in ROWS:
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        if not tab:
            continue
        print('%-14s %-4s | %10s %10s %10s' % (r, w, show(leg_star(tab, 'E', 0.050)),
                                               show(leg_star(tab, 'pf_r', 1.200)), show(both_star(tab))))

print('')
print('ANSWER: spread* on BOTH windows = the tighter of the two windows')
print('%-14s %12s %12s %14s %16s' % ('row', 's*_IS', 's*_OOS', 's*', 'as share of 0.28'))
for r in ROWS:
    if not T[r]['IS'] or not T[r]['OOS']:
        continue
    a = both_star(T[r]['IS'])
    b = both_star(T[r]['OOS'])
    if a[0] == 'lt' or b[0] == 'lt':
        st = ('lt', -0.28)
    elif a[0] == 'ge' and b[0] == 'ge':
        st = ('ge', 0.28)
    else:
        st = ('x', min(a[1], b[1]))
    sh = ('%+.1f%%' % (100 * st[1] / 0.28)) if st[0] == 'x' else ('>= 100%' if st[0] == 'ge' else '< -100%')
    print('%-14s %12s %12s %14s %16s' % (r, show(a), show(b), show(st), sh))
