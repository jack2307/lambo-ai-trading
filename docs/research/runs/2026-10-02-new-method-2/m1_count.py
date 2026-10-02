import os, re, sys, collections
ROOT = r"E:\rust\flowdesk"
RUNS = os.path.join(ROOT, "docs", "research", "runs")
LO, HI = "2026-09-12", "2026-09-19"   # [LO, HI) the original programme

# a row line in a `--mode=hypotheses` or sweep receipt ends with a percentile field
pct_row = re.compile(r"^(?P<id>[A-Za-z0-9][A-Za-z0-9_./:-]*)\s+(?P<rest>.*?\s(?P<pct>\d{1,3})%\s)")
pairs_pct = set()      # (run, row) with a matched-null percentile printed
pairs_any = set()      # (run, row) appearing in any receipt table at all
per_run = collections.defaultdict(set)
files = 0
for d in sorted(os.listdir(RUNS)):
    if not (LO <= d[:10] < HI):
        continue
    p = os.path.join(RUNS, d)
    if not os.path.isdir(p):
        continue
    for fn in sorted(os.listdir(p)):
        if not fn.endswith((".txt",)):
            continue
        files += 1
        try:
            txt = open(os.path.join(p, fn), encoding="utf-8", errors="replace").read()
        except Exception:
            continue
        for line in txt.splitlines():
            if not line or line[0].isspace():
                continue
            m = pct_row.match(line)
            if m:
                rid = m.group("id")
                if rid in ("hypothesis", "bars:", "bounds:", "tape:", "swap:", "timeline:", "$"):
                    continue
                pairs_pct.add((d, rid))
                per_run[d].add(rid)
print("receipt files read:", files)
print("run directories in [%s, %s): %d" % (LO, HI, len(per_run)))
print("distinct (run, row) pairs with a percentile printed:", len(pairs_pct))
print("distinct row ids across those runs:", len({r for _, r in pairs_pct}))
print()
for d in sorted(per_run):
    print("%-44s %3d  %s" % (d, len(per_run[d]), " ".join(sorted(per_run[d])[:8]) + (" ..." if len(per_run[d])>8 else "")))
