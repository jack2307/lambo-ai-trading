#!/usr/bin/env python3
"""One line per (run, row) off this job's receipts, and the declared-cell count.

Reads only `receipts/nis_*.txt`. Prints the gate verdict counted BY HAND against
the desk's 40-trade floor rather than the tool's 30 (brief section 4), plus the
stop in ATR and points, the cost % of R, the count match and the exit mix, so no
figure in the record is quoted without the line it came from.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REC = ROOT / 'receipts'

GATE_PF, GATE_EXP, GATE_N = 1.200, 0.050, 40

ROW = re.compile(
    r'^(nis\d+m-\w+-p\d-f\d+)\s+news-pulse\s+(\d+)\s+'
    r'(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+(\d+)%'
)
STOP = re.compile(r"the method's own realised stop: median ([\d.]+) ATR = ([\d.]+) points")
COST = re.compile(r'cost-matched null: control stop ([\d.]+) ATR = ([\d.]+) points, cost ([\d.]+)% of R')
MATCH = re.compile(r'count match ([\d.]+)')
EXITS = re.compile(r'exits: (.+?); mean hold')
REFUSED = re.compile(r'refused (\w+) (\d+)')
LONGSH = re.compile(r'long share: method ([\d.]+)')


def parse(path):
    text = path.read_text(encoding='utf-8', errors='replace')
    lines = text.splitlines()
    news = next((l for l in lines if l.startswith('news: ')), 'news: MISSING')
    scope = next((l for l in lines if l.startswith('news scope:')), 'news scope: MISSING')
    guards = next((l for l in lines if l.startswith('guards:')), 'guards: MISSING')
    spread = next((l for l in lines if l.startswith('spread:')), 'spread: MISSING')
    flags = next((l for l in lines if l.startswith('flags:')), 'flags: MISSING')
    out = []
    i = 0
    while i < len(lines):
        m = ROW.match(lines[i])
        if not m:
            i += 1
            continue
        block = '\n'.join(lines[i + 1:i + 14])
        s, c, mt, ex = STOP.search(block), COST.search(block), MATCH.search(block), EXITS.search(block)
        ls = LONGSH.search(block)
        refused = dict((k, int(v)) for k, v in REFUSED.findall(block))
        exits = {}
        if ex:
            for part in ex.group(1).split(','):
                bits = part.strip().split()
                if len(bits) == 2:
                    exits[bits[0]] = int(bits[1])
        out.append(dict(
            label=m.group(1), trades=int(m.group(2)), pf=float(m.group(3)),
            exp=float(m.group(4)), null_p50=float(m.group(5)), null_p95=float(m.group(6)),
            pct=int(m.group(8)),
            stop_atr=float(s.group(1)) if s else None, stop_pts=float(s.group(2)) if s else None,
            cost=float(c.group(3)) if c else None,
            match=float(mt.group(1)) if mt else None,
            long_share=float(ls.group(1)) if ls else None,
            exits=exits, refused=refused,
        ))
        i += 1
    return dict(news=news, scope=scope, guards=guards, spread=spread, flags=flags, rows=out)


def verdict(r):
    if r['trades'] < GATE_N:
        return 'VOID <40'
    legs = []
    if r['pf'] < GATE_PF:
        legs.append('PF')
    if r['exp'] < GATE_EXP:
        legs.append('exp')
    return 'PASS' if not legs else 'fail:' + '+'.join(legs)


def main():
    runs = sorted(REC.glob('nis_*.txt'))
    runs = [p for p in runs if p.name not in ('nis_runlog.txt',)]
    total = 0
    for p in runs:
        d = parse(p)
        print(f'\n===== {p.name}  ({len(d["rows"])} rows) =====')
        print('  ' + d['news'])
        print('  ' + d['scope'] + ' | ' + d['guards'] + ' | ' + d['spread'])
        print('  ' + d['flags'])
        if not d['rows']:
            # a run that took no trade on any row still prints no row line
            print('  NO ROW LINE PRINTED — see the receipt (a guarded arm takes 0 trades)')
        for r in d['rows']:
            total += 1
            mix = '/'.join(f'{k} {v}' for k, v in r['exits'].items()) or 'null'
            own = sum(v for k, v in r['exits'].items() if k in ('STOP', 'TARGET'))
            share = f'{100*own/r["trades"]:.0f}%' if r['trades'] else 'null'
            print(f'  {r["label"]:<22} n={r["trades"]:>4} PF={r["pf"]:>6.3f} exp={r["exp"]:>+7.3f}R '
                  f'p50={r["null_p50"]:>5.3f} pct={r["pct"]:>3}% match={r["match"] if r["match"] is not None else "null"} '
                  f'stop={r["stop_atr"] if r["stop_atr"] is not None else "null"}ATR'
                  f'/{r["stop_pts"] if r["stop_pts"] is not None else "null"}pts '
                  f'cost={r["cost"] if r["cost"] is not None else "null"}% own={share} [{mix}] '
                  f'-> {verdict(r)}')
            if r['refused']:
                print(f'       refused: {r["refused"]}')
    print(f'\n(run, row) lines printed: {total}')
    print('declared in docs/decisions/2026-10-07-news-inner-stop.md section 8: 176')
    print('  plumbing 16 + 1m gate 64 + 5m gate 64 + zero-spread diagnostic 32')
    return 0


if __name__ == '__main__':
    sys.exit(main())
