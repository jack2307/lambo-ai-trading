"""Build the 48-cell table from the receipts, reading only what the tool printed.

Gate counted by hand against 40 trades, not the tool's `need 30`.
No percentile is published.
"""
import glob
import os
import re
import sys

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

OUT = r'E:/rust/fd-options-basis/docs/research/runs/2026-10-10-options-rolling-basis'

LABELS = {'opt-lvl': 'level-reversion', 'opt-mpn': 'maxpain-magnet',
          'opt-flw': 'flow-momentum', 'opt-fal': 'flow-at-level'}

ROW = re.compile(r'^(opt-\w+)\s+([\w-]+)\s+(\d+)\s+([\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(\d+)%\s+(\S+)')


def parse(path):
    txt = open(path, encoding='utf-8', errors='replace').read()
    head = {}
    m = re.search(r'^basis:\s+ROLLING median over a trailing (\d+) min window, >= (\d+)', txt, re.M)
    if m:
        head['kind'] = 'roll'
        head['param'] = f'W={m.group(1)}m'
        head['min_obs'] = m.group(2)
    else:
        m = re.search(r'^basis:\s+option levels shifted ([+-][\d.]+) price units', txt, re.M)
        head['kind'] = 'const'
        head['param'] = m.group(1) if m else '?'
    m = re.search(r'^basis:\s+BARS \(the engine.s own denominator\): (\d+) of the bar series carry a frame, and (\d+) of those still carry LEVELS after the basis . (\d+) bars REFUSED', txt, re.M)
    if m:
        head['bars_covered'], head['bars_levels'], head['bars_refused'] = (int(m.group(i)) for i in (1, 2, 3))
    m = re.search(r'^basis:\s+APPLIED rolling estimates: n (\d+) mean ([+-][\d.]+) sd ([\d.]+).*?p10 ([+-][\d.]+) p50 ([+-][\d.]+) p90 ([+-][\d.]+)', txt, re.M)
    if m:
        head['applied'] = dict(n=int(m.group(1)), mean=float(m.group(2)), sd=float(m.group(3)),
                               p10=float(m.group(4)), p50=float(m.group(5)), p90=float(m.group(6)))
    m = re.search(r'^tape:\s+(\d+) prints, (\d+) frames', txt, re.M)
    if m:
        head['prints'], head['frames'] = int(m.group(1)), int(m.group(2))
    m = re.search(r'^bars:\s+(\d+) from', txt, re.M)
    if m:
        head['bars'] = int(m.group(1))
    m = re.search(r'^basis:\s+raw GC-spot basis[^:]*: n (\d+) mean ([+-][\d.]+) sd ([\d.]+) min ([+-][\d.]+) p10 ([+-][\d.]+) p50 ([+-][\d.]+) p90 ([+-][\d.]+) max ([+-][\d.]+) . quartile means in TIME order ([+-][\d.]+) -> ([+-][\d.]+) -> ([+-][\d.]+) -> ([+-][\d.]+)', txt, re.M)
    if m:
        head['raw'] = m.group(0).split(': ', 1)[1]

    cells = []
    lines = txt.splitlines()
    for i, ln in enumerate(lines):
        m = ROW.match(ln)
        if not m:
            continue
        c = dict(label=m.group(1), base=m.group(2), trades=int(m.group(3)),
                 pf_usd_col=float(m.group(4)), expect=float(m.group(5)),
                 null_p50=float(m.group(6)), null_p95=float(m.group(7)),
                 pct=int(m.group(9)), verdict=m.group(10))
        blk = '\n'.join(lines[i + 1:i + 14])
        g = re.search(r'Lbar ([\d.-]+) R .*?PF_r ([\d.-]+) \(R\) vs PF_usd ([\d.-]+) \(USD, gap ([+-][\d.]+)\); identity E = Lbar\(PF_r-1\) = ([+-][\d.]+) R vs expectancy ([+-][\d.]+) R, residual ([+-][\d.e-]+) R', blk)
        if g:
            c.update(lbar=float(g.group(1)), pf_r=float(g.group(2)), pf_usd=float(g.group(3)),
                     gap=float(g.group(4)), E=float(g.group(5)), resid=g.group(7))
        g = re.search(r'max drawdown ([\d.]+) USD = ([\d.]+)% of the book.s peak equity.*?net ([+-][\d.]+) USD over (\d+) trades', blk, re.S)
        if g:
            c.update(dd_usd=float(g.group(1)), dd_pct=float(g.group(2)), net_usd=float(g.group(3)))
        g = re.search(r'count match ([\d.]+)', blk)
        if g:
            c['count_match'] = float(g.group(1))
        g = re.search(r'cost match ([\d.]+)', blk)
        if g:
            c['cost_match'] = float(g.group(1))
        g = re.search(r"the method's own realised stop: median ([\d.]+) ATR = ([\d.]+) points", blk)
        if g:
            c.update(stop_atr=float(g.group(1)), stop_pts=float(g.group(2)))
        g = re.search(r'cost ([\d.]+)% of R', blk)
        if g:
            c['cost_r'] = float(g.group(1))
        g = re.search(r'sized down (\d+)', blk)
        c['sized_down'] = int(g.group(1)) if g else None
        g = re.search(r'^\s+exits: (.+?); mean hold ([\d.]+) min', blk, re.M)
        if g:
            c['exits'] = g.group(1)
            c['hold'] = float(g.group(2))
        cells.append(c)
    return head, cells


def gate(c):
    """The desk gate, counted by hand: PF_r >= 1.200 AND E >= +0.050R AND >= 40 trades."""
    if c['trades'] < 40:
        return 'NO VERDICT (floor)'
    legs = []
    if c.get('pf_r', 0) < 1.200:
        legs.append('PF_r')
    if c.get('E', -9) < 0.050:
        legs.append('E')
    return 'CLEARS' if not legs else 'MISS (' + '+'.join(legs) + ')'


files = sorted(glob.glob(OUT + '/*.txt'))
files = [f for f in files if not f.endswith('SUMMARY.txt')]
order = {'roll': 0, 'const': 1}
rows = []
heads = {}
for f in files:
    name = os.path.basename(f)[:-4]
    h, cs = parse(f)
    heads[name] = h
    arm = 'guards' if name.endswith('-guards') else 'noguards'
    for c in cs:
        rows.append((name, arm, h, c))

print('=' * 118)
print('RUN HEADERS - this run\'s own print and bar counts (the store is written while it is read)')
print('=' * 118)
seen = set()
for n, h in heads.items():
    k = (h.get('prints'), h.get('frames'), h.get('bars'))
    if k not in seen:
        seen.add(k)
        print(f'  prints {h.get("prints")}  frames {h.get("frames")}  bars {h.get("bars")}   (first seen in {n})')
raws = {h['raw'] for h in heads.values() if 'raw' in h}
for r in raws:
    print(f'  raw basis: {r}')
print()

print('=' * 118)
print('BAR COVERAGE after the basis (the engine iterates BARS, never frames)')
print('=' * 118)
for n in sorted(heads, key=lambda x: (order.get(heads[x]['kind'], 9), x)):
    h = heads[n]
    if 'bars_covered' in h:
        a = h.get('applied')
        ap = (f'  applied: n {a["n"]} mean {a["mean"]:+.2f} sd {a["sd"]:.2f} '
              f'p10 {a["p10"]:+.2f} p50 {a["p50"]:+.2f} p90 {a["p90"]:+.2f}') if a else ''
        print(f'  {n:<24} bars with frame {h["bars_covered"]:4d}  with LEVELS {h["bars_levels"]:4d}  '
              f'REFUSED {h["bars_refused"]:3d}{ap}')
    else:
        print(f'  {n:<24} (constant offset {h["param"]}: no bar line, every covered bar keeps its levels)')
print()

print('=' * 118)
print('THE CELLS. Gate counted by hand against 40 trades. NO PERCENTILE IS PUBLISHED.')
print('=' * 118)
hdr = (f'{"basis":<12} {"arm":<9} {"mechanism":<17} {"n":>4} {"PF_r(R)":>8} {"PF_usd":>8} {"gap":>8} '
       f'{"E(R)":>9} {"Lbar(R)":>8} {"DD$":>8} {"DD%":>7} {"cm":>5} {"down":>5}  verdict')
for arm in ('guards', 'noguards'):
    print(f'\n---- arm: {arm}{" (THE ONLY ARM THE OWNER PERMITS)" if arm == "guards" else " (NOT A TRADING ARM)"} ----')
    print(hdr)
    for base in ('level-reversion', 'flow-at-level', 'maxpain-magnet', 'flow-momentum'):
        for n, a, h, c in sorted(rows, key=lambda r: (order.get(r[2]['kind'], 9), r[2]['param'])):
            if a != arm or c['base'] != base:
                continue
            p = h['param']
            print(f'{p:<12} {a:<9} {c["base"]:<17} {c["trades"]:>4} '
                  f'{c.get("pf_r", float("nan")):>8.4f} {c.get("pf_usd", float("nan")):>8.4f} '
                  f'{c.get("gap", float("nan")):>+8.4f} {c.get("E", float("nan")):>+9.4f} '
                  f'{c.get("lbar", float("nan")):>8.4f} {c.get("dd_usd", float("nan")):>8.2f} '
                  f'{c.get("dd_pct", float("nan")):>7.2f} {c.get("count_match", float("nan")):>5.2f} '
                  f'{str(c.get("sized_down")):>5}  {gate(c)}')
print()

# ---- the control: flow-momentum must be identical everywhere -------------
print('=' * 118)
print('CONTROL - flow-momentum reads NO price level, so it must print IDENTICALLY at every basis')
print('=' * 118)
for arm in ('guards', 'noguards'):
    sig = {}
    for n, a, h, c in rows:
        if a != arm or c['base'] != 'flow-momentum':
            continue
        key = (c['trades'], c.get('pf_r'), c.get('E'), c.get('lbar'), c.get('stop_atr'), c.get('exits'))
        sig.setdefault(key, []).append(h['param'])
    print(f'  arm {arm}: {len(sig)} distinct print(s) across {sum(len(v) for v in sig.values())} basis settings')
    for k, v in sig.items():
        print(f'    n={k[0]} PF_r={k[1]} E={k[2]} Lbar={k[3]} stop={k[4]} exits="{k[5]}"')
        print(f'      at: {", ".join(sorted(v))}')
print()

print('=' * 118)
print('IDENTITY E = Lbar x (PF_r - 1), and the PF_usd - PF_r gap direction ON THIS SET')
print('=' * 118)
res = [abs(float(c['resid'])) for _, _, _, c in rows if 'resid' in c]
print(f'  identity held on {len(res)}/{len(rows)} cells, largest residual {max(res):.5f} R' if res else '  null')
gaps = [c['gap'] for _, _, _, c in rows if 'gap' in c]
hi = sum(1 for g in gaps if g > 0)
print(f'  PF_r read HIGHER than PF_usd on {hi}/{len(gaps)} cells = {100.0 * hi / len(gaps):.1f}%  '
      f'(gap = PF_r - PF_usd; measured on THIS set, not quoted from another)')
print(f'  widest gaps: ' + '; '.join(
    f'{h["param"]}/{a}/{c["base"]} {c["gap"]:+.4f}'
    for _, a, h, c in sorted(rows, key=lambda r: -abs(r[3].get('gap', 0)))[:4] for h in [_ and h]) if gaps else '')
straddle = [(h['param'], a, c['base'], c['pf_r'], c['pf_usd'])
            for _, a, h, c in rows if 'pf_r' in c and (c['pf_r'] - 1.2) * (c['pf_usd'] - 1.2) < 0]
print(f'  cells STRADDLING the gate\'s own 1.200 line between the two units: {len(straddle)}')
for s in straddle:
    print(f'    {s[0]}/{s[1]}/{s[2]}: PF_r {s[3]:.4f} (R) vs PF_usd {s[4]:.4f} (USD)')
print()

print('=' * 118)
print('REALISED STOP and cost/R, from each receipt\'s own line')
print('=' * 118)
for base in ('level-reversion', 'flow-at-level', 'maxpain-magnet', 'flow-momentum'):
    v = [(c.get('stop_atr'), c.get('stop_pts'), c.get('cost_r')) for _, a, h, c in rows
         if c['base'] == base and a == 'guards' and c.get('stop_atr')]
    if not v:
        print(f'  {base:<17} null')
        continue
    print(f'  {base:<17} stop {min(x[0] for x in v):.3f}-{max(x[0] for x in v):.3f} ATR14(15m) = '
          f'{min(x[1] for x in v):.2f}-{max(x[1] for x in v):.2f} points, cost/R '
          f'{min(x[2] for x in v):.2f}-{max(x[2] for x in v):.2f}% (spread 0.28 CONFIGURED, one terminal read)')
print()

print('=' * 118)
print('EXIT MIX, guards arm — the mechanism\'s own rule must have fired')
print('=' * 118)
for _, a, h, c in sorted(rows, key=lambda r: (r[3]['base'], order.get(r[2]['kind'], 9), r[2]['param'])):
    if a != 'guards':
        continue
    print(f'  {c["base"]:<17} {h["param"]:<12} n {c["trades"]:>4}  hold {c.get("hold", float("nan")):>6.1f} min  '
          f'exits: {c.get("exits")}')
