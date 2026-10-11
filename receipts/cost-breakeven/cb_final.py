# -*- coding: utf-8 -*-
import sys, os, re, json
SC = r'C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/cb/'
sys.path.insert(0, SC)
from cb_parse import parse

RC = r'E:/rust/fd-cost-breakeven/receipts/cost-breakeven/'
POS = ['0.28', '0.24', '0.20', '0.16', '0.12', '0.08', '0.04', '0.00']
NEG = ['-0.04', '-0.08', '-0.12', '-0.16', '-0.20', '-0.28']
SP = POS + NEG
ROWS = ['qs-lb3', 'ts-l5', 'ts-l20', 'ts-l10', 'qs-lb30', 'tsmom',
        'qs-h15', 'qs-qp020', 'qs-h30', 'qs-h2', 'ovn1900/long']
CTRL = ['px-1s', 'ovn1900/short', 'ema-cross']


def parse_plus(path):
    rows = parse(path)
    cur = None
    for line in open(path, encoding='utf-8', errors='replace'):
        m = re.match(r'^(\S+)\s+\S+\s+\d+\s', line)
        if m and not line.startswith(' '):
            cur = m.group(1)
        if cur and 'spread paid: method' in line and cur in rows and 'spread_paid' not in rows[cur]:
            rows[cur]['spread_paid'] = float(re.search(r'spread paid: method ([\-\d.]+) USD', line).group(1))
    return rows


D = {}
missing = []
for w, pre in [('IS', 'is'), ('OOS', 'oos')]:
    for s in SP:
        p = RC + pre + '-s' + s + '.txt'
        if os.path.exists(p) and 'hypothesis   base' in open(p, encoding='utf-8', errors='replace').read():
            D[(w, s)] = parse_plus(p)
        else:
            missing.append((w, s))
print('receipts: %d of 28; missing: %s' % (len(D), missing))


def fit(xs, ys, slope=None):
    n = len(xs)
    my = sum(ys) / n
    if slope is None:
        mx = sum(xs) / n
        sxx = sum((x - mx) ** 2 for x in xs)
        sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
        k = sxy / sxx
        a = my - k * mx
    else:
        k = slope
        a = sum(y - k * x for x, y in zip(xs, ys)) / n
    sst = sum((y - my) ** 2 for y in ys)
    ssr = sum((y - (a + k * x)) ** 2 for x, y in zip(xs, ys))
    return a, k, (1 - ssr / sst if sst > 0 else float('nan')), ssr


T = {}
for r in ROWS + CTRL:
    T[r] = {}
    for w in ['IS', 'OOS']:
        tab = []
        for s in SP:
            d = D.get((w, s), {}).get(r)
            if not d or 'pf_r' not in d:
                continue
            fs = float(s)
            sp = d.get('spread_paid')
            tr = d['E'] * d['n']
            risk = None
            if sp not in (None, 0.0) and abs(fs) > 1e-9 and abs(tr) > 1e-12:
                risk = (d['net_usd'] / tr) * d['n'] * fs / sp
            tab.append(dict(s=fs, n=d['n'], pf_r=d['pf_r'], pf_usd=d['pf_usd'], E=d['E'],
                            Lbar=d['Lbar'], sd=d.get('sized_down'), dd=d.get('dd_usd'),
                            ddp=d.get('dd_pct'), net=d.get('net_usd'), sp=sp, risk=risk,
                            mae=d.get('avg_mae'), leg=d.get('leg')))
        T[r][w] = tab
json.dump(T, open(SC + 'final.json', 'w'))


def pick(tab, s):
    for x in tab:
        if abs(x['s'] - s) < 1e-9:
            return x
    return None


print('')
print('############ PER-ROW SPREAD SWEEP, guards ON ############')
summary = []
for r in ROWS + CTRL:
    print('')
    print('=' * 138)
    print('ROW %s %s' % (r, '  <<< CONTROL' if r in CTRL else ''))
    base = {}
    for w in ['IS', 'OOS']:
        tab = T[r][w]
        if not tab:
            continue
        r28 = pick(tab, 0.28)
        base[w] = r28
        print('  %s  %6s %6s %8s %8s %10s %8s %6s %10s %7s %8s %9s %5s %5s' % (
            w, 's', 'n', 'PF_r', 'PF_usd', 'E(R)', 'Lbar', 'sd', 'ddUSD', 'dd%', 'R_pts', 'E_pin(R)', 'PFok', 'Eok'))
        rp28 = r28['risk'] if (r28 and r28['risk']) else None
        for x in tab:
            riskhat = (rp28 + (x['s'] - 0.28) / 2.0) if rp28 else None
            epin = x['E'] * (riskhat / rp28) if riskhat else float('nan')
            flag = '  [COUNTERFACTUAL]' if x['s'] < 0 else ''
            print('     %6.2f %6d %8.4f %8.4f %+10.4f %8.4f %6s %10.2f %7.2f %8.2f %+9.4f %5s %5s%s' % (
                x['s'], x['n'], x['pf_r'], x['pf_usd'], x['E'], x['Lbar'], str(x['sd']),
                (x['dd'] or 0), (x['ddp'] or 0),
                (x['risk'] if x['risk'] else float('nan')), epin,
                'Y' if x['pf_r'] >= 1.2 else 'n', 'Y' if x['E'] >= 0.05 else 'n', flag))
        ns = sorted(set(x['n'] for x in tab))
        pos = [x for x in tab if x['s'] >= 0]
        nspos = sorted(set(x['n'] for x in pos))
        mono = all(pos[i]['E'] <= pos[i + 1]['E'] for i in range(len(pos) - 1))
        monoall = all(tab[i]['E'] <= tab[i + 1]['E'] for i in range(len(tab) - 1))
        print('     n over the 8 non-negative points: %s  (n over all 14: %s)   E monotone as s falls: non-neg %s / all %s' % (
            nspos, ns, mono, monoall))
        print('     sized_down_by_guard over all points: %s' % sorted(set(str(x['sd']) for x in tab)))
        pts = [(x['s'], x['risk']) for x in tab if x['risk'] is not None]
        if len(pts) >= 4 and rp28:
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            a, k, r2, ssr = fit(xs, ys)
            a0, k0, r20, ssr0 = fit(xs, ys, slope=0.5)
            az, kz, r2z, ssrz = fit(xs, ys, slope=0.0)
            print('     delta FIT free slope: risk(s)=%.4f%+.4f*s R2=%.5f | ONE-PARAM k=0.5: A=%.4f R2=%.5f SSR=%.6f | k=0 (no delta): A=%.4f R2=%.5f SSR=%.6f' % (
                a, k, r2, a0, r20, ssr0, az, r2z, ssrz))
            print('     code-exact delta(0.28)=+0.1400 pts = %.3f%% of R(0.28)=%.2f pts => E_pin(0)/E(0)=%.5f' % (
                100 * 0.14 / rp28, rp28, 1 - 0.14 / rp28))
    row = {'row': r}
    for key, mode in [('spread_star', 'E'), ('spread_star_pin', 'Epin')]:
        best = None
        for s in [float(x) for x in POS]:
            ok = True
            for w in ['IS', 'OOS']:
                x = pick(T[r][w], s)
                r28 = base.get(w)
                if not x or x['n'] < 40:
                    ok = False
                    break
                rp = r28['risk'] if (r28 and r28['risk']) else None
                e = x['E'] if mode == 'E' else (x['E'] * ((rp + (s - 0.28) / 2.0) / rp) if rp else x['E'])
                if not (x['pf_r'] >= 1.200 and e >= 0.050):
                    ok = False
                    break
            if ok:
                best = s if best is None else max(best, s)
        row[key] = best
    for w in ['IS', 'OOS']:
        x = pick(T[r][w], 0.0)
        if x:
            row['E0_' + w] = x['E']
            row['PFr0_' + w] = x['pf_r']
    cf = None
    for s in [float(y) for y in SP]:
        ok = True
        for w in ['IS', 'OOS']:
            x = pick(T[r][w], s)
            if not x or x['n'] < 40 or not (x['pf_r'] >= 1.200 and x['E'] >= 0.050):
                ok = False
                break
        if ok:
            cf = s if cf is None else max(cf, s)
    row['cf_star'] = cf
    summary.append(row)
    print('  => spread* (non-negative grid, engine E): %s | spread* (R unit pinned at 0.28): %s | widest passing point incl COUNTERFACTUAL: %s' % (
        row['spread_star'], row['spread_star_pin'], row['cf_star']))

print('')
print('############ ANSWER TABLE ############')
print('%-14s %10s %12s %10s %10s %10s %10s %10s' % ('row', 'spread*', 'spread*_pin', 'cf_star', 'E0_IS', 'E0_OOS', 'PFr0_IS', 'PFr0_OOS'))
for row in summary:
    def f(k, fmt):
        v = row.get(k)
        return (fmt % v) if v is not None else 'null'
    print('%-14s %10s %12s %10s %10s %10s %10s %10s' % (
        row['row'], f('spread_star', '%.2f'), f('spread_star_pin', '%.2f'), f('cf_star', '%.2f'),
        f('E0_IS', '%+.4f'), f('E0_OOS', '%+.4f'), f('PFr0_IS', '%.4f'), f('PFr0_OOS', '%.4f')))
json.dump(summary, open(SC + 'summary.json', 'w'))
