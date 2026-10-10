"""PC5: WHERE IN THE DAY the 786 overlapping bars fall, in New York time, and
what the basis looks like inside vs outside the CME maintenance break.

`agent/hour-screen` measured that the reopening bar after the 17:00 NY break
carries a provider print artefact (on Dukascopy gold the reopen `open` IS the
bar low 19.02% of the time against 3.20% for an ordinary bar; on the broker
feed 6.75%). The gold option tape covers only 786 of 100,586 bars = 0.8%, so if
those bars sit around the break the basis is being estimated across a bar one
of the two feeds prints badly. Read-only.
"""
import glob
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
import statistics as st

import pyarrow.parquet as pq

BARS = r'E:/rust/flowdesk/data/bars'


def bars_of(t):
    ts = t.column('time').to_pylist()
    o = t.column('open').to_pylist()
    h = t.column('high').to_pylist()
    l = t.column('low').to_pylist()
    c = t.column('close').to_pylist()
    out = []
    for a, oo, hh, ll, cc in zip(ts, o, h, l, c):
        ms = int(a.timestamp() * 1000) if hasattr(a, 'timestamp') else int(a)
        out.append((ms, oo, hh, ll, cc))
    out.sort()
    return out


gcb = bars_of(pq.read_table(BARS + '/GC-1m.parquet'))
xab = bars_of(pq.read_table(BARS + '/XAUUSD-15m.parquet'))
gcmap = {ms: c for ms, o, h, l, c in gcb}

# NY offset: the record's receipts print NY wall clock; gold's weekend-flat is
# 16:40 NY and the break is 17:00-18:00 NY. September 2026 is EDT = UTC-4.
NY_OFFSET_H = -4

pairs = []
for ms, o, h, l, c in xab:
    k = ms + 14 * 60000
    if k in gcmap:
        ny_min = (ms // 60000 + NY_OFFSET_H * 60) % 1440
        pairs.append((ms, ny_min, gcmap[k] - c, o, h, l, c))

print(f'paired (overlapping) bars: {len(pairs)} of {len(xab)} XAUUSD-15m bars '
      f'= {100.0 * len(pairs) / len(xab):.1f}%')
print()
print('--- NY hour distribution of the overlapping bars ---')
print('  hour   bars   share   basis mean   basis sd')
byh = {}
for ms, nym, b, o, h, l, c in pairs:
    byh.setdefault(nym // 60, []).append(b)
for hh in range(24):
    v = byh.get(hh, [])
    if not v:
        print(f'  {hh:02d}:00   null   —        null         null   (no overlapping bar in this hour)')
        continue
    sd = st.pstdev(v) if len(v) > 1 else 0.0
    print(f'  {hh:02d}:00  {len(v):5d}  {100.0 * len(v) / len(pairs):5.1f}%   '
          f'{sum(v) / len(v):+10.2f}   {sd:8.2f}')

# The break bars specifically: 17:00-17:15 NY (last before) and 18:00-18:15 NY
# (the reopen `hour-screen` found the artefact in).
print()
print('--- the CME maintenance break, bar by bar ---')
for label, lo, hi in [("17:00-17:15 NY (into the break)", 17 * 60, 17 * 60 + 15),
                      ("17:15-18:00 NY (inside the break)", 17 * 60 + 15, 18 * 60),
                      ("18:00-18:15 NY (the REOPEN bar)", 18 * 60, 18 * 60 + 15),
                      ("everything else", -1, -1)]:
    if lo < 0:
        sel = [p for p in pairs if not (17 * 60 <= p[1] < 18 * 60 + 15)]
    else:
        sel = [p for p in pairs if lo <= p[1] < hi]
    if not sel:
        print(f'  {label:<34} null bars — nothing measured here (not 0)')
        continue
    b = [p[2] for p in sel]
    # open == low, the artefact `hour-screen` counted
    openlow = sum(1 for p in sel if p[3] == p[5])
    sd = st.pstdev(b) if len(b) > 1 else 0.0
    print(f'  {label:<34} bars {len(sel):4d}  basis mean {sum(b) / len(b):+7.2f} sd {sd:5.2f}  '
          f'| open==low on {openlow} = {100.0 * openlow / len(sel):.2f}%')

# And the same count over ALL XAUUSD-15m bars, as the baseline.
allopenlow = sum(1 for ms, o, h, l, c in xab if o == l)
print(f'\n  baseline, ALL {len(xab)} XAUUSD-15m bars: open==low on {allopenlow} '
      f'= {100.0 * allopenlow / len(xab):.2f}%')
reopen_all = [(ms, o, h, l, c) for ms, o, h, l, c in xab
              if 18 * 60 <= ((ms // 60000 + NY_OFFSET_H * 60) % 1440) < 18 * 60 + 15]
if reopen_all:
    ol = sum(1 for ms, o, h, l, c in reopen_all if o == l)
    print(f'  baseline, the {len(reopen_all)} REOPEN bars across all 4 years: '
          f'open==low on {ol} = {100.0 * ol / len(reopen_all):.2f}%')

# Calendar days covered
days = sorted({p[0] // 86400000 for p in pairs})
print(f'\n  distinct UTC days with overlap: {len(days)}')
print(f'  market-hours of overlap: {len(pairs) * 15 / 60:.1f} h')
