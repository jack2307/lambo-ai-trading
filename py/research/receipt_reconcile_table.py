"""The 34-row classification table: published vs the verbatim-argv re-run vs the
pinned positive control. Read-only over run files already produced."""
import json, os, re, sys
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
V = [v for v in json.load(open(f"{S}/rr/bars3.json")) if v["indec"]]

def cells(t):
    out = {}
    hdr = None
    for ln in t.splitlines():
        tk = ln.split()
        if not tk: continue
        if "trades" in tk and ("PF" in tk or "OOS" in tk): hdr = tk; continue
        if "/" in tk[0] and len(tk) > 4 and re.fullmatch(r"\d+", tk[2]):
            out.setdefault(tk[0], []).append((int(tk[2]), tk[3], tk[4]))
    return out

CTL = {}
for fn in sorted(os.listdir(f"{S}/rr/ctl/out")):
    t = open(f"{S}/rr/ctl/out/{fn}", encoding="utf-8", errors="replace").read()
    CTL[fn[:-4]] = dict(cells=cells(t),
                        bars=re.search(r"^bars:\s*(\d+) from (\S+ \S+) to (\S+ \S+)", t, re.M),
                        bounds=re.search(r"^bounds:\s*(.*)$", t, re.M),
                        tape=re.search(r"^tape:\s*(\d+) prints, (\d+) frames", t, re.M))

# which control belongs to which row
MAP = {
 ("fix/am-after",578):"w1-london-fix", ("fix/pm-after",578):"w1-london-fix", ("fix/pm-into",578):"w1-london-fix",
 ("hivol/london",341):"w1-volcond", ("hivol/orb60",457):"w1-volcond",
 ("lowvol/london",212):"w1-volcond", ("lowvol/orb60",73):"w1-volcond",
 ("london/early",458):"w1-london-range-d", ("london/expansion",338):"w1-london-range-d",
 ("london/nyam",525):"w1-london-range-d",
 ("m15/london",501):"w1-gold-m15", ("m15/orb-60m",505):"w1-gold-m15",
 ("m15/pdhl-break",163):"w1-gold-m15", ("m15/pdhl-fade",531):"w1-gold-m15",
 ("pdhl/break-nyam",172):"w1-pdhl", ("pdhl/fade-allday",555):"w1-pdhl",
 ("orb/60m",189):"w1-orb-ny-oos", ("orb/30m",394):"w1-orb-ny-oos", ("orb/60m-expansion",153):"w1-orb-ny-oos",
 ("close/1400-1630",289):"w2-close-drift", ("close/1630-1815",282):"w2-close-drift",
 ("close/1630-2000",282):"w2-close-drift", ("close/1630-2200",282):"w2-close-drift",
 ("close/1815-2200",289):"w2-close-drift",
 ("london/early",203):"w2-london-range-u", ("london/expansion",131):"w2-london-range-u",
 ("london/nyam",257):"w2-london-range-u", ("orb/30m",164):"w2-orb-ny-is",
 ("vwap/allday",5828):"p1-vwap-nopin", ("vwap/us-hours",1392):"p1-vwap-nopin",
 ("vwap/wide",5828):"p1-vwap-nopin", ("m15/vwap-allday",3392):"p1-btc-m15-nopin",
 ("close/fri-1630-1815",336):"g1-weekend-1655", ("close/1630-1815",1621):None,
}
CLS = {
 "w1-london-fix":"W1","w1-volcond":"W1","w1-london-range-d":"W1","w1-gold-m15":"W1","w1-pdhl":"W1",
 "w1-orb-ny-oos":"W1+C1","w2-close-drift":"W2","w2-london-range-u":"W2","w2-orb-ny-is":"W2",
 "p1-vwap-nopin":"P1","p1-btc-m15-nopin":"P1","g1-weekend-1655":"G1",
}
print(f"{'label':<22}{'pubN':>6}{'pubPF':>8} | {'argvN':>7} | {'ctlN':>6}{'ctlPF':>8} {'ctl bars':>9} {'=?':>4} class  control")
rows=[]
for v in sorted(V, key=lambda x:(x["label"], x["trades"])):
    k = (v["label"], v["trades"]); c = MAP.get(k)
    cn = cpf = cb = None
    if c and c in CTL:
        got = CTL[c]["cells"].get(v["label"], [])
        if got: cn, cpf = got[0][0], got[0][1]
        cb = CTL[c]["bars"].group(1) if CTL[c]["bars"] else None
    ok = "YES" if cn == v["trades"] else ("--" if cn is None else f"{cn-v['trades']:+d}")
    cls = CLS.get(c, "E1")
    print(f"{v['label']:<22}{v['trades']:>6}{v['pf']:>8.3f} | {str(v['now'][0]) if v['now'] else '-':>7} | "
          f"{str(cn or '-'):>6}{str(cpf or '-'):>8} {str(cb or '-'):>9} {ok:>4} {cls:<6} {c or '(read only)'}")
    rows.append(dict(label=v["label"], pubN=v["trades"], pubPF=v["pf"], pubE=v["e"],
                     argvN=v["now"][0] if v["now"] else None, ctlN=cn, ctlPF=cpf,
                     ctlbars=cb, cls=cls, ctl=c,
                     docs=[p for p in v["allpaths"] if p.startswith("docs/decisions/")]))
json.dump(rows, open(f"{S}/rr/table34.json","w"), indent=1)
print()
from collections import Counter
print(Counter(r["cls"] for r in rows))
print("exact return:", sum(1 for r in rows if r["ctlN"]==r["pubN"]), "of", len(rows))
