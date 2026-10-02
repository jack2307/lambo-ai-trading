import os, re, statistics as st
ROOT=r"E:\rust\flowdesk"; RUNS=os.path.join(ROOT,"docs","research","runs")
LO,HI="2026-09-12","2026-09-19"
rx=re.compile(r"^(?P<id>[A-Za-z0-9][A-Za-z0-9_./:-]*)\s+\S+\s+(?P<tr>\d+)\s+(?P<pf>-?[\d.]+)\s+(?P<ex>-?[\d.]+)\s+(?P<p50>-?[\d.]+)\s+(?P<p95>-?[\d.]+)\s+(?P<sw>-?[\d.]+)\s+(?P<pct>\d{1,3})%")
best={}
for d in sorted(os.listdir(RUNS)):
    if not (LO<=d[:10]<HI): continue
    p=os.path.join(RUNS,d)
    if not os.path.isdir(p): continue
    for fn in sorted(os.listdir(p)):
        if not fn.endswith(".txt"): continue
        for line in open(os.path.join(p,fn),encoding="utf-8",errors="replace"):
            if not line or line[0].isspace(): continue
            m=rx.match(line)
            if not m: continue
            k=(d,m.group("id")); v=dict(tr=int(m.group("tr")),pf=float(m.group("pf")),
               p50=float(m.group("p50")),p95=float(m.group("p95")),pct=int(m.group("pct")))
            if k not in best or v["pct"]>best[k]["pct"]: best[k]=v
V=list(best.values())
p50s=[v["p50"] for v in V]
print("cells:",len(V))
print("null median profit factor (the control's own p50): min %.3f  q1 %.3f  median %.3f  q3 %.3f  max %.3f"%(
  min(p50s),st.quantiles(p50s,n=4)[0],st.median(p50s),st.quantiles(p50s,n=4)[2],max(p50s)))
print("cells whose null median is BELOW 1.000 (the control loses money): %d of %d (%.1f%%)"%(
  sum(1 for x in p50s if x<1.0),len(p50s),100.0*sum(1 for x in p50s if x<1.0)/len(p50s)))
# cells at >=95th that are themselves LOSING (PF<1.0)
lose95=[(k,v) for k,v in best.items() if v["pct"]>=95 and v["pf"]<1.0]
print("cells at >= 95th of their null that LOSE money (PF < 1.000): %d"%len(lose95))
for k,v in sorted(lose95): print("   %-38s %-26s PF %.3f  null p50 %.3f  pct %d"%(k[0],k[1],v["pf"],v["p50"],v["pct"]))
# sigma scale
sig=[(v["p95"]-v["p50"])/1.645 for v in V if v["p95"]>v["p50"]]
print("\nsigma of the null in PF units: median %.4f over %d cells"%(st.median(sig),len(sig)))
