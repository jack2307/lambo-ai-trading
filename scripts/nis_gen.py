import io, os
HEAD = """# XAUDUKA {iv}: a stop INSIDE the release range
#
# Registered in docs/decisions/2026-10-07-news-inner-stop.md, written and
# committed before this file existed. The row shape is agent/n1's
# (receipts/news-entry.toml) with ONE thing varied: `stopImpulse`, the fraction
# of the impulse window's own range the stop sits at. All 12 of n1's rows and
# all 32 of agent/news-tf's set it to 1.0; nobody moved it.
#
# f = 1.00 is the published control, re-run here in THIS binary so the
# comparison is like-for-like (F3 of the registration).
#
# `minMoveAtr` is pinned at 0.5 on every row and is NOT swept: see section 3.
#
# Each row carries the `newsonly:` gate that admits EXACTLY the one bar its own
# `probeBars` fires on at this interval, so the matched null in hypotheses.rs
# draws from the same single bar after the same release as the method.
#
# `news-pulse` declares no grid. Run with --fixed.
"""
ROW = """
[[hypothesis]]
label = "{label}"
base = "news-pulse"
filters = ["newsonly:{a}/{b}:3:USD"]
overrides = {{ probeBars = {p}, mode = {m}, minMoveAtr = 0.5, stopImpulse = {f}, minImpact = 3, atrPeriod = 14, maxLagMin = 15 }}
why = "{why}"
"""
out_dir = 'E:/rust/fd-news-inner-stop/receipts'
os.makedirs(out_dir, exist_ok=True)
for iv, step in (('1m', 1), ('5m', 5)):
    buf = io.StringIO()
    buf.write(HEAD.format(iv=iv))
    for f in (0.25, 0.50, 0.75, 1.00):
        for p in (1, 2):
            for m, tag in ((0, 'brk'), (1, 'rev')):
                a, b = (p - 1) * step, p * step
                ftag = f'f{int(f*100):03d}'
                label = f'nis{iv}-{tag}-p{p}-{ftag}'
                sense = ('the first move after a scheduled release persists'
                         if m == 0 else
                         'the first move after a scheduled release overshoots and gives it back')
                inner = ('the WHOLE release range (the published control, f = 1.00)'
                         if f == 1.00 else
                         f'a fraction f = {f:.2f} of the release range, so the stop sits INSIDE it '
                         f'and the target is {1.8*f:.2f}x the range away instead of 1.80x')
                why = (f'{sense}; probe {p} bar(s) of {iv} = the signal bar opens +{a} min from the '
                       f'release, gate 0.5 pre-release ATR(14) of {iv} bars. Stop = {inner}. '
                       f'The filter admits exactly that one bar, so the matched null draws from the '
                       f'same bar after the same release.')
                buf.write(ROW.format(label=label, a=a, b=b, p=p, m=m, f=f'{f:.2f}', why=why))
    path = f'{out_dir}/nis-{iv}.toml'
    open(path, 'w', encoding='utf-8').write(buf.getvalue())
    n = buf.getvalue().count('[[hypothesis]]')
    print(path, n, 'rows')
