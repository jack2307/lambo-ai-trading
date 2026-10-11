"""Resolve each vanished key's PUBLISHED RUN RECEIPT (preferring
docs/research/runs/*) from whatever ref carries it, and put its own header next
to the re-run's. Read-only; search.exe is NOT invoked."""
import json, os, re, subprocess
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
REPO = "E:/rust/fd-receipt-reconcile"
V = json.load(open(f"{S}/rr/vanished.json"))
rows = json.load(open(f"{S}/pfr-audit/gate_legs_rows.json", encoding="utf-8"))
runs = {fn[:-4]: open(f"{S}/pfr-audit/runs/{fn}", encoding="utf-8", errors="replace").read()
        for fn in os.listdir(f"{S}/pfr-audit/runs") if fn.endswith(".txt")}
cache = {}
def git(ref, path):
    k = (ref, path)
    if k in cache: return cache[k]
    p = subprocess.run(["git", "show", f"{ref}:{path}"], cwd=REPO,
                       capture_output=True, text=True, errors="replace")
    cache[k] = p.stdout if p.returncode == 0 else None
    return cache[k]

def grab(t):
    k = re.search(r"kept (\d+) of (\d+) bars", t)
    b = re.search(r"^bars:\s*(\d+) from (\S+ \S+) to (\S+ \S+)", t, re.M)
    fo = re.search(r"walk-forward \((\d+) folds?\)", t)
    de = re.search(r": (\d+) declared", t)
    tp = re.search(r"^tape:\s*(.*)$", t, re.M)
    cmd = t.splitlines()[0] if t else ""
    return dict(kept=int(k.group(1)) if k else None,
                total=int(k.group(2)) if k else None,
                n=int(b.group(1)) if b else None,
                t0=b.group(2) if b else None, t1=b.group(3) if b else None,
                folds=int(fo.group(1)) if fo else None,
                declared=int(de.group(1)) if de else None,
                tape=tp.group(1) if tp else None,
                fixed="--fixed" in cmd, guards="--guards" in cmd, cmd=cmd)

out = []
for v in V:
    cand = [r for r in rows
            if (r["label"], r["trades"], round(r["pf"],3), round(r["e"],3)) ==
               (v["label"], v["trades"], round(v["pf"],3), round(v["e"],3))]
    allp = sorted({p for r in cand for p in r["paths"]})
    indec = any(p.startswith("docs/decisions/") for p in allp)
    # prefer a run receipt over a decision/summary doc
    order = ([p for p in allp if p.startswith("docs/research/runs/")] +
             [p for p in allp if p.startswith("receipts/")] +
             [p for p in allp if p not in ()])
    got = gotp = gotr = None
    for p in order:
        if os.path.exists(f"{REPO}/{p}"):
            t = open(f"{REPO}/{p}", encoding="utf-8", errors="replace").read()
            if "kept" in t: got, gotp, gotr = t, p, "WORKTREE"; break
        for r in cand:
            if p not in r["paths"]: continue
            for br in r.get("branches", []):
                t = git(br, p)
                if t and "kept" in t: got, gotp, gotr = t, p, br; break
            if got: break
        if got: break
    out.append(dict(v, indec=indec, allpaths=allp, pubpath=gotp, pubref=gotr,
                    pub=grab(got) if got else None,
                    rerun=[dict(grab(runs[h]), h=h) for h in v["runs"] if h in runs]))
json.dump(out, open(f"{S}/rr/bars3.json","w"), indent=1)
d = [v for v in out if v["indec"]]
print(f"in docs/decisions/: {len(d)}   published header resolved: {sum(1 for v in d if v['pub'])}")
print()
hdr = f"{'label':<22}{'pubN':>6}{'now':>9} {'PUB kept(span end)':<34}{'RERUN kept(span end)':<34}{'fold p/r':<10}{'fix p/r':<8}"
print(hdr)
for v in sorted(d, key=lambda x:(x["label"], x["trades"])):
    p = v["pub"]; r = v["rerun"][0] if v["rerun"] else None
    f = lambda z: (f"{z['kept']} ({(z['t0'] or '?')[:10]}..{(z['t1'] or '?')[:10]})" if z and z['kept'] else "?")
    print(f"{v['label']:<22}{v['trades']:>6}{str(v['now']):>9} {f(p):<34}{f(r):<34}"
          f"{str(p and p['folds'])+'/'+str(r and r['folds']):<10}"
          f"{str(p and p['fixed'])[0]+'/'+str(r and r['fixed'])[0]:<8}")
