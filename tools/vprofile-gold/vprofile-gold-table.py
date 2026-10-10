"""Build the 72-cell table for agent/vprofile-gold from the four receipts.

Reads only what the receipts print. `E` is taken as the receipt's own
`expectancy` AND recomputed as `Lbar x (PF_r - 1)` from the Lbar line, which is
the identity the `lbar_line` patch exists to make checkable; `expectancy` at
three decimals is not enough to read a gross (ADDENDUM-8 section IV), and both
are printed so the pair can be compared.
"""
import glob
import os
import re
import sys

ROWS = {}
HEAD = re.compile(r"^(\S+/d\d+/t[\d.]+) vprofile-reversion\s+(\d+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)%\s+(.*)$")
DD = re.compile(r"max drawdown ([\d.]+) USD = ([\d.]+)% of the book's peak.*?net ([-+\d.]+) USD over (\d+) trades")
LB = re.compile(r"Lbar ([\d.]+) R .*?PF_r ([\d.]+) \(R\) vs PF_usd ([\d.]+) \(USD, gap ([-+\d.]+)\).*?identity E = Lbar\(PF_r-1\) = ([-+\d.]+) R vs expectancy ([-+\d.]+) R, residual ([-+\d.]+) R")
COST = re.compile(r"cost-matched null: control stop ([\d.]+) ATR = ([\d.]+) points, cost ([\d.]+)% of R")
STOP = re.compile(r"the method's own realised stop: median ([\d.]+) ATR = ([\d.]+) points")
EX = re.compile(r"^\s+exits: (.*?); mean hold (.*)$")
CM = re.compile(r"count match ([\d.]+)")

for path in sorted(glob.glob(sys.argv[1] + "/*.txt")):
    run = os.path.basename(path)[:-4]
    cur = None
    for line in open(path, encoding="utf-8", errors="replace"):
        m = HEAD.match(line)
        if m:
            cur = {
                "run": run,
                "label": m.group(1),
                "n": int(m.group(2)),
                "pf_oos": m.group(3),
                "expect": m.group(4),
                "null_p50": m.group(5),
                "null_p95": m.group(6),
                "verdict": m.group(9).strip(),
            }
            ROWS[(run, m.group(1))] = cur
            continue
        if cur is None:
            continue
        for rx, keys in [
            (DD, ("dd_usd", "dd_pct", "net_usd", "n_dd")),
            (LB, ("lbar", "pf_r", "pf_usd", "gap", "identity_e", "expect_r", "resid")),
            (COST, ("null_stop_atr", "null_stop_pts", "cost_pct_r")),
            (STOP, ("stop_atr", "stop_pts")),
            (CM, ("count_match",)),
        ]:
            mm = rx.search(line)
            if mm:
                for k, v in zip(keys, mm.groups()):
                    cur[k] = v
        mm = EX.match(line)
        if mm:
            cur["exits"] = mm.group(1)
            cur["hold"] = mm.group(2)

print(f"{len(ROWS)} cells read")
print()

GATE_PF, GATE_E, GATE_N = 1.200, 0.050, 40
hdr = (
    f"{'window':<6} {'guards':<4} {'label':<16} {'n':>5} {'PF_usd':>7} {'PF_r':>7} {'E(R)':>8} {'Lbar':>7} "
    f"{'resid':>9} {'netUSD':>9} {'ddUSD':>8} {'dd%':>7} {'stopATR':>7} {'cost/R%':>8} {'np50':>6} {'np95':>6} {'cm':>5} {'exits':<34}"
)
print(hdr)
print("-" * len(hdr))
for run in ["w1-guards-off", "w1-guards-on", "w2-guards-off", "w2-guards-on"]:
    w = "W1" if run.startswith("w1") else "W2"
    g = "off" if run.endswith("off") else "ON"
    for key, r in ROWS.items():
        if key[0] != run:
            continue
        print(
            f"{w:<6} {g:<4} {r['label']:<16} {r['n']:>5} {r.get('pf_usd','-'):>7} {r.get('pf_r','-'):>7} "
            f"{r.get('expect_r','-'):>8} {r.get('lbar','-'):>7} {r.get('resid','-'):>9} "
            f"{r.get('net_usd','-'):>9} {r.get('dd_usd','-'):>8} {r.get('dd_pct','-'):>7} "
            f"{r.get('stop_atr','-'):>7} {r.get('cost_pct_r','-'):>8} {r.get('null_p50','-'):>6} "
            f"{r.get('null_p95','-'):>6} {r.get('count_match','-'):>5} {r.get('exits','-'):<34}"
        )
    print()

# The gate, counted by hand on BOTH windows, per guard arm.
print("GATE, counted by hand (PF_r >= 1.200 AND E >= +0.050R AND n >= 40 on BOTH windows):")
labels = sorted({k[1] for k in ROWS})
for g, a, b in [("off", "w1-guards-off", "w2-guards-off"), ("ON", "w1-guards-on", "w2-guards-on")]:
    passed = []
    for lab in labels:
        ra, rb = ROWS.get((a, lab)), ROWS.get((b, lab))
        if not ra or not rb:
            continue
        ok = all(
            float(r["pf_r"]) >= GATE_PF and float(r["expect_r"]) >= GATE_E and r["n"] >= GATE_N
            for r in (ra, rb)
        )
        if ok:
            passed.append(lab)
    print(f"  guards {g}: {len(passed)}/{len(labels)} rows pass -> {passed if passed else 'NONE'}")

print()
print("best PF_r on each window, per guard arm (none of these is a pass):")
for run in ["w1-guards-off", "w1-guards-on", "w2-guards-off", "w2-guards-on"]:
    rs = [r for k, r in ROWS.items() if k[0] == run]
    best = max(rs, key=lambda r: float(r["pf_r"]))
    print(
        f"  {run:<15} {best['label']:<16} PF_r {best['pf_r']}  E {best['expect_r']} R  n {best['n']}  "
        f"dd {best.get('dd_usd','-')} USD = {best.get('dd_pct','-')}% of peak"
    )

print()
print("F4 (does the rule fire?) - TARGET is the mechanism's own rule, exit at the POC:")
for run in ["w1-guards-off", "w2-guards-off"]:
    for k, r in sorted(ROWS.items()):
        if k[0] != run:
            continue
        ex = r.get("exits", "")
        tgt = re.search(r"TARGET (\d+)", ex)
        tot = sum(int(x) for x in re.findall(r"(\d+)", ex)) or 1
        share = 100 * int(tgt.group(1)) / tot if tgt else 0.0
        print(f"  {run:<15} {r['label']:<16} TARGET {tgt.group(1) if tgt else 0:>4} of {tot:>4} exits = {share:5.1f}%")

print()
print("F6 (burned book) - any cell printing max_drawdown_pct > 100%:")
burned = [(k, r["dd_pct"]) for k, r in ROWS.items() if float(r.get("dd_pct", 0)) > 100.0]
print(f"  {len(burned)} of {len(ROWS)} cells -> {burned if burned else 'none'}")
hi = sorted(ROWS.items(), key=lambda kv: -float(kv[1].get("dd_pct", 0)))[:3]
for k, r in hi:
    print(f"  worst: {k[0]} {k[1]} dd {r['dd_usd']} USD = {r['dd_pct']}% of peak, net {r['net_usd']} USD")
