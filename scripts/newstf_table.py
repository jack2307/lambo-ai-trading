# Pull one line per (run,row) out of every receipt this job wrote.
import re, glob, os, io
rows=[]
for p in sorted(glob.glob(r"E:/rust/fd-news-tf/receipts/newstf_*.txt")):
    name=os.path.basename(p)[len("newstf_"):-4]
    if name in ("runlog",): continue
    txt=io.open(p,encoding="utf-8",errors="replace").read().splitlines()
    cur=None
    for ln in txt:
        m=re.match(r"^(np\S+) news-pulse\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(.*)$", ln)
        if m:
            cur=dict(run=name,row=m.group(1),trades=m.group(2),pf=m.group(3),exp=m.group(4),
                     np50=m.group(5),np95=m.group(6),pct=m.group(8),verdict=m.group(9).strip(),
                     cm="", cost="", stop="", exits="", longsh="")
            rows.append(cur); continue
        if cur is None: continue
        if "count match" in ln:
            cur["cm"]=re.search(r"count match (\S+)",ln).group(1)+(" UNMATCHED" if "outside the band" in ln else "")
        elif "cost-matched null" in ln:
            g=re.search(r"control stop (\S+) ATR = (\S+) points, cost (\S+) of R",ln)
            if g: cur["stop"]=g.group(2); cur["cost"]=g.group(3)
        elif ln.strip().startswith("exits:"):
            cur["exits"]=ln.strip()[6:].split(";")[0].strip()
        elif "long share: method" in ln:
            cur["longsh"]=re.search(r"long share: method (\S+)",ln).group(1)
print("run|row|trades|PF|exp|null_p50|null_p95|pct|count_match|stop_pts|cost%R|long|exits|verdict")
for r in rows:
    print("|".join([r["run"],r["row"],r["trades"],r["pf"],r["exp"],r["np50"],r["np95"],r["pct"],
                    r["cm"],r["stop"],r["cost"],r["longsh"],r["exits"],r["verdict"]]))
print("\nTOTAL (run,row) cells seen:", len(rows))
