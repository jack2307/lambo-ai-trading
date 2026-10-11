# -*- coding: utf-8 -*-
"""Is the sign of PF_usd - PF_r a WINDOW constant? Measured on MY OWN control
runs: every `Lbar ... PF_r ... PF_usd ... gap` line the patched binary printed,
split by the window the run was bounded to. Read-only; no engine run."""
import os, re, collections
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
D = f"{S}/rr/ctl/out"
rows = []
for fn in sorted(os.listdir(D)):
    if not fn.endswith(".txt"): continue
    t = open(f"{D}/{fn}", encoding="utf-8", errors="replace").read()
    mk = re.search(r"^market:\s*(\S+)\s+(\S+)", t, re.M)
    bd = re.search(r"^bars:\s*(\d+) from (\S+) \S+ to (\S+) \S+", t, re.M)
    cur = None
    for ln in t.splitlines():
        tk = ln.split()
        if tk and "/" in tk[0] and len(tk) > 4 and re.fullmatch(r"\d+", tk[2]):
            cur = (tk[0], int(tk[2]))
        m = re.search(r"Lbar ([\d.-]+) R .* PF_r ([\d.]+) \(R\) vs PF_usd ([\d.]+) \(USD, gap ([+-][\d.]+)\)", ln)
        if m and cur:
            rows.append(dict(file=fn[:-4], label=cur[0], n=cur[1],
                             market=mk.group(1) if mk else "?",
                             tf=mk.group(2) if mk else "?",
                             span=(bd.group(2), bd.group(3)) if bd else ("?", "?"),
                             lbar=float(m.group(1)), pf_r=float(m.group(2)),
                             pf_usd=float(m.group(3)), gap=float(m.group(4))))
print(f"cells with a printed Lbar line in my own control runs: {len(rows)}")
print()
# the desk's two windows: W1 = the 2010/2022 -> 2025 history, W2 = the 2025-04-11 -> now broker window
def win(r):
    return "W2 (2025-04-11 ->)" if r["span"][0] >= "2025-04-11" else f"W1 ({r['span'][0]} ->)"
by = collections.defaultdict(list)
for r in rows: by[(win(r), r["market"], r["tf"])].append(r)
print(f"{'window':<22}{'market':<9}{'tf':<5}{'cells':>6}{'USD higher':>12}{'share':>8}{'|gap| min':>11}{'|gap| max':>11}")
for k in sorted(by):
    v = by[k]; hi = sum(1 for r in v if r["gap"] > 0)
    ag = [abs(r["gap"]) for r in v]
    print(f"{k[0]:<22}{k[1]:<9}{k[2]:<5}{len(v):>6}{hi:>12}{100*hi/len(v):>7.1f}%{min(ag):>11.4f}{max(ag):>11.4f}")
print()
agg = collections.defaultdict(list)
for r in rows: agg[win(r)].append(r)
print("POOLED BY WINDOW ONLY:")
for k in sorted(agg):
    v = agg[k]; hi = sum(1 for r in v if r["gap"] > 0)
    print(f"  {k:<22} cells {len(v):>3}   PF_usd higher {hi:>3} = {100*hi/len(v):5.1f}%")
v = rows; hi = sum(1 for r in v if r["gap"] > 0)
print(f"  {'POOLED ACROSS BOTH':<22} cells {len(v):>3}   PF_usd higher {hi:>3} = {100*hi/len(v):5.1f}%   <- the number a pooled ratio would print")
print()
# identity residual check on my own cells, and the Lbar >= 0.250R split
print("identity / gate-leg read on my own cells:")
print(f"  cells with Lbar >= 0.250 R (expectancy leg REDUNDANT): {sum(1 for r in rows if r['lbar']>=0.250)} of {len(rows)}")
print(f"  cells where PF_usd and PF_r sit on OPPOSITE sides of 1.200: "
      f"{sum(1 for r in rows if (r['pf_usd']>=1.2) != (r['pf_r']>=1.2))}")
for r in rows:
    if (r["pf_usd"]>=1.2) != (r["pf_r"]>=1.2):
        print(f"    {r['label']:<22} n={r['n']:<6} {r['market']:<8} PF_usd {r['pf_usd']:.4f} vs PF_r {r['pf_r']:.4f}  [{r['file']}]")
