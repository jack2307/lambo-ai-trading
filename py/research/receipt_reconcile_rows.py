"""rr = receipt-reconcile. Reconstruct the 51 vanished band keys (34 of them in
docs/decisions/) from the artifacts agent/pfr-audit left behind:

    pfr-audit/gate_legs_rows.json   every printed row carrying PF+expectancy
    pfr-audit/cmd_of.json           blob sha -> echoed argv found on that blob
    pfr-audit/repro_band.json       band key -> the commands that reproduce it
    pfr-audit/runs/*.txt            the 88 completed re-runs (exit 0)

Nothing is harvested here and search.exe is NOT invoked. Read-only.
"""
import json, os, re, sys
from collections import defaultdict

P = sys.argv[1] if len(sys.argv) > 1 else "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/pfr-audit"
LO, HI = 0.700, 1.700

rows = json.load(open(f"{P}/gate_legs_rows.json", encoding="utf-8"))
cmd_of = json.load(open(f"{P}/cmd_of.json", encoding="utf-8"))
repro = json.load(open(f"{P}/repro_band.json", encoding="utf-8"))

def key(r): return (r["label"], r["trades"], round(r["pf"],3), round(r["e"],3))

# ---- re-run cells, per command file -----------------------------------------
# A method row in a hypotheses receipt: "<label> <base> <trades> <PF> <exp> ..."
runs = {}
for fn in sorted(os.listdir(f"{P}/runs")):
    if not fn.endswith(".txt"): continue
    t = open(f"{P}/runs/{fn}", encoding="utf-8", errors="replace").read()
    if "# exit 0" not in t: continue
    pub = re.search(r"^# PUBLISHED COMMAND: (.*)$", t, re.M)
    cells = []   # (label, trades, pf_usd)
    hdr = None
    for ln in t.splitlines():
        tk = ln.split()
        if not tk: continue
        if "trades" in tk and ("PF" in tk or "OOS_PF" in tk):
            hdr = tk; continue
        if hdr and len(tk) > len(hdr) - 4:
            try:
                i = hdr.index("trades")
                if i < len(tk) and re.fullmatch(r"\d+", tk[i]):
                    j = hdr.index("PF") if "PF" in hdr else hdr.index("OOS_PF")
                    pf = float(tk[j]) if j < len(tk) and re.fullmatch(r"[\d.]+", tk[j]) else None
                    cells.append((tk[0], int(tk[i]), pf))
            except (ValueError, IndexError):
                pass
    runs[fn[:-4]] = {
        "pub": pub.group(1) if pub else None,
        "rerun_argv": t.splitlines()[0][2:],
        "cells": cells,
        "bars": re.search(r"kept (\d+) of (\d+) bars", t),
        "text": t,
    }

import hashlib
by_cmd = {}
for h, v in runs.items():
    if v["pub"]: by_cmd[v["pub"]] = h

# ---- which band keys never found a cell at their trade count ----------------
rowkey = {}
for r in rows:
    if LO <= r["pf"] <= HI: rowkey.setdefault(key(r), []).append(r)

vanished = []
for ks, cmds in repro.items():
    label, trades, pf, e = ks.split("|")
    trades, pf, e = int(trades), float(pf), float(e)
    hit = False
    seen = set(); seen_lbl = set()
    for c in cmds:
        h = by_cmd.get(c)
        if not h: continue
        for (lb, n, p) in runs[h]["cells"]:
            if lb == label:
                seen_lbl.add(n)
                if n == trades: hit = True
        seen.add(h)
    if not hit:
        src = rowkey.get((label, trades, pf, e), [])
        paths = sorted({p for r in src for p in r["paths"]})
        vanished.append(dict(label=label, trades=trades, pf=pf, e=e,
                             cmds=sorted(cmds), runs=sorted(seen),
                             now=sorted(seen_lbl), paths=paths,
                             positive=any(r.get("positive") for r in src)))

dec = [v for v in vanished if any(p.startswith("docs/decisions/") for p in v["paths"])]
print(f"vanished band keys        : {len(vanished)}")
print(f"  of them in docs/decisions/: {len(dec)}")
json.dump(vanished, open(f"{os.path.dirname(P)}/rr/vanished.json","w"), indent=1)
for v in sorted(dec, key=lambda x:(x["label"], x["trades"])):
    print(f"{v['label']:<26} n={v['trades']:<6} PF={v['pf']:.3f} E={v['e']:+.3f} "
          f"now={v['now']} runs={len(v['runs'])} docs={[p for p in v['paths'] if p.startswith('docs/decisions/')]}")
