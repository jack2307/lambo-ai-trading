# -*- coding: utf-8 -*-
# Per-leg, per-window breakeven: the widest spread on the grid at which each
# gate leg passes on its own. Reads final.json written by cb_final.py.
import json
SC = r'C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/cb/'
T = json.load(open(SC + 'final.json'))
POS = [0.28, 0.24, 0.20, 0.16, 0.12, 0.08, 0.04, 0.00]
ALL = POS + [-0.04, -0.08, -0.12, -0.16, -0.20, -0.28]
ROWS = ['qs-lb3', 'ts-l5', 'ts-l20', 'ts-l10', 'qs-lb30', 'tsmom',
        'qs-h15', 'qs-qp020', 'qs-h30', 'qs-h2', 'ovn1900/long',
        'px-1s', 'ovn1900/short', 'ema-cross']


def pick(tab, s):
    for x in tab:
        if abs(x['s'] - s) < 1e-9:
            return x
    return None


def widest(tab, test, grid):
    best = None
    for s in grid:
        x = pick(tab, s)
        if x and x['n'] >= 40 and test(x):
            best = s if best is None else max(best, s)
    return best


def fmt(v):
    return ('%+.2f' % v) if v is not None else 'null'


print('WIDEST SPREAD AT WHICH EACH LEG PASSES ON ITS OWN (n>=40 enforced), guards ON')
print('  "pos" = over the 8 non-negative points only; "cf" = including the 6 COUNTERFACTUAL credit points')
print('')
print('%-14s %-4s | %9s %9s | %9s %9s | %9s %9s' % ('row', 'win', 'PFleg pos', 'PFleg cf', 'Eleg pos', 'Eleg cf', 'both pos', 'both cf'))
for r in ROWS:
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        if not tab:
            continue
        pf = lambda x: x['pf_r'] >= 1.200
        ee = lambda x: x['E'] >= 0.050
        bo = lambda x: x['pf_r'] >= 1.200 and x['E'] >= 0.050
        print('%-14s %-4s | %9s %9s | %9s %9s | %9s %9s' % (
            r, w, fmt(widest(tab, pf, POS)), fmt(widest(tab, pf, ALL)),
            fmt(widest(tab, ee, POS)), fmt(widest(tab, ee, ALL)),
            fmt(widest(tab, bo, POS)), fmt(widest(tab, bo, ALL))))

print('')
print('DEFICIT AT ZERO SPREAD (free execution), and what the counterfactual grid says it costs to close')
print('%-14s %-4s | %9s %9s | %9s %9s | %12s' % ('row', 'win', 'E(0)', 'need E', 'PF_r(0)', 'need PF', 'E-gap in R'))
for r in ROWS:
    for w in ['IS', 'OOS']:
        x = pick(T[r][w], 0.0)
        if not x:
            continue
        print('%-14s %-4s | %+9.4f %9.3f | %9.4f %9.3f | %+12.4f' % (
            r, w, x['E'], 0.050, x['pf_r'], 1.200, 0.050 - x['E']))

print('')
print('SPREAD PAID IN R PER TRADE, measured as E(0) - E(s) over the grid (the sweep, not a division)')
print('%-14s %-4s | %10s %10s %10s %10s %10s %10s %10s' % ('row', 'win', 'dE@0.28', 'dE@0.24', 'dE@0.20', 'dE@0.12', 'dE@0.08', 'dE@0.04', 'per 0.01'))
for r in ROWS:
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        z = pick(tab, 0.0)
        if not z:
            continue
        vals = []
        for s in [0.28, 0.24, 0.20, 0.12, 0.08, 0.04]:
            x = pick(tab, s)
            vals.append((z['E'] - x['E']) if x else float('nan'))
        print('%-14s %-4s | %10.5f %10.5f %10.5f %10.5f %10.5f %10.5f %10.6f' % (
            r, w, vals[0], vals[1], vals[2], vals[3], vals[4], vals[5], vals[0] / 28.0))
