import os, re, collections
ROOT = r"E:\rust\flowdesk"
RUNS = os.path.join(ROOT, "docs", "research", "runs")
LO, HI = "2026-09-12", "2026-09-19"
pct_row = re.compile(r"^(?P<id>[A-Za-z0-9][A-Za-z0-9_./:-]*)\s+\S+\s+(?P<tr>\d+)\s+(?P<pf>-?[\d.]+)\s+(?P<ex>-?[\d.]+)\s+(?P<p50>-?[\d.]+)\s+(?P<p95>-?[\d.]+)\s+(?P<sw>-?[\d.]+)\s+(?P<pct>\d{1,3})%")
rows = {}
for d in sorted(os.listdir(RUNS)):
    if not (LO <= d[:10] < HI): continue
    p = os.path.join(RUNS, d)
    if not os.path.isdir(p): continue
    for fn in sorted(os.listdir(p)):
        if not fn.endswith(".txt"): continue
        for line in open(os.path.join(p, fn), encoding="utf-8", errors="replace"):
            if not line or line[0].isspace(): continue
            m = pct_row.match(line)
            if not m: continue
            key = (d, m.group("id"), fn)
            rows[key] = dict(trades=int(m.group("tr")), pf=float(m.group("pf")),
                             p50=float(m.group("p50")), p95=float(m.group("p95")),
                             pct=int(m.group("pct")))
# collapse to (run,row): keep the max percentile seen for that pair
best = {}
for (d, rid, fn), v in rows.items():
    k = (d, rid)
    if k not in best or v["pct"] > best[k]["pct"]:
        best[k] = v
print("parsed cell lines:", len(rows))
print("distinct (run,row) pairs fully parsed:", len(best))
ge95 = [k for k,v in best.items() if v["pct"] >= 95]
ge99 = [k for k,v in best.items() if v["pct"] >= 99]
print("pairs at >= 95th of their own null:", len(ge95), " (%.1f%%)" % (100.0*len(ge95)/len(best)))
print("pairs at >= 99th:", len(ge99), " (%.1f%%)" % (100.0*len(ge99)/len(best)))
print("pairs with >= 30 trades:", sum(1 for v in best.values() if v["trades"]>=30))
b30 = {k:v for k,v in best.items() if v["trades"]>=30}
g = [k for k,v in b30.items() if v["pct"]>=95]
print("of those, >= 95th:", len(g), " (%.1f%%)" % (100.0*len(g)/len(b30)))
print()
print("the >=95th pairs with >=30 trades:")
for k in sorted(g):
    v=b30[k]; print("  %-40s %-24s trades %5d PF %6.3f p50 %5.3f p95 %5.3f pct %3d" % (k[0],k[1],v["trades"],v["pf"],v["p50"],v["p95"],v["pct"]))
