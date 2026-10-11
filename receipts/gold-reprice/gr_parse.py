"""Parse the four gold-reprice receipts into one table. Reads only; prints only."""
import re, sys, os
D = os.path.dirname(os.path.abspath(__file__))
ROWS = ["px-1s","ovn1900/short","ema-cross","qs-lb3","ts-l5","ts-l20","ts-l10",
        "qs-lb30","tsmom","qs-h15","qs-qp020","qs-h30","qs-h2","ovn1900/long"]

def parse(path):
    txt = open(path, encoding="utf-8", errors="replace").read().splitlines()
    out, cur = {}, None
    for ln in txt:
        m = re.match(r"^(\S+)\s+(\S+)\s+(\d+)\s+([+-]?[\d.]+)\s+([+-]?[\d.]+)\s", ln)
        if m and m.group(1) in ROWS:
            cur = m.group(1)
            out[cur] = {"n": int(m.group(3)), "pf_tbl": float(m.group(4)), "exp": float(m.group(5))}
        if cur is None:
            continue
        m = re.search(r"Lbar ([+-]?[\d.]+) R .*PF_r ([+-]?[\d.]+) \(R\) vs PF_usd ([+-]?[\d.]+) \(USD, gap ([+-]?[\d.]+)\).*identity E = Lbar\(PF_r-1\) = ([+-]?[\d.]+) R", ln)
        if m:
            out[cur].update(lbar=float(m.group(1)), pf_r=float(m.group(2)),
                            pf_usd=float(m.group(3)), gap=float(m.group(4)), e=float(m.group(5)))
        m = re.search(r"max drawdown ([\d.]+) USD = ([\d.]+)% .*avg_mae ([+-]?[\d.]+) R.*net ([+-]?[\d.]+) USD", ln)
        if m:
            out[cur].update(dd=float(m.group(1)), ddpct=float(m.group(2)),
                            mae=float(m.group(3)), net=float(m.group(4)))
    return out

for w in ("is", "oos"):
    a = parse(os.path.join(D, f"{w}-s0.28.txt"))
    b = parse(os.path.join(D, f"{w}-s0.220.txt"))
    print(f"\n===== {w.upper()} window =====")
    print(f"{'row':<14} {'n@.28':>6} {'n@.22':>6} | {'PF_r@.28':>9} {'PF_r@.22':>9} {'dPF_r':>8} | "
          f"{'E@.28':>8} {'E@.22':>8} {'dE':>8} | {'Lbar@.28':>9} {'Lbar@.22':>9} {'dLbar':>8} | "
          f"{'DD$@.28':>9} {'DD$@.22':>9} | {'net$@.28':>10} {'net$@.22':>10}")
    for r in ROWS:
        x, y = a.get(r), b.get(r)
        if not x or not y: print(f"{r:<14} MISSING"); continue
        print(f"{r:<14} {x['n']:>6} {y['n']:>6} | {x['pf_r']:>9.4f} {y['pf_r']:>9.4f} {y['pf_r']-x['pf_r']:>+8.4f} | "
              f"{x['e']:>+8.4f} {y['e']:>+8.4f} {y['e']-x['e']:>+8.4f} | {x['lbar']:>9.4f} {y['lbar']:>9.4f} "
              f"{y['lbar']-x['lbar']:>+8.4f} | {x['dd']:>9.2f} {y['dd']:>9.2f} | {x['net']:>+10.2f} {y['net']:>+10.2f}")
