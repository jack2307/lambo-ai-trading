# -*- coding: utf-8 -*-
"""delta = spread/2 as a share of R, per row, on the stop the engine printed.
Defect 16 (addendum 7 C): risk = f x range + delta, because sizing_stop reads
the signal bar's close while entry has already been through apply_costs, so
d risk/d s = +1/2 exactly (from the CODE, not a regression)."""
import json
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
R = json.load(open(f"{S}/rr/table34c.json"))
SPREAD = {"xauusd": 0.28, "xauduka": 0.28, "btc": 5.00}
print(f"{'label':<22}{'pubN':>6} {'mkt':<8}{'stop(pts)':>10}{'spread':>8}{'delta':>8}{'delta/R':>9}{'cost/R':>8}  delta as a share of the printed cost/R")
out = []
for r in sorted(R, key=lambda x: (float(x["stop_pts"]) if x["stop_pts"] else 9e9)):
    if not r["stop_pts"]:
        print(f"{r['label']:<22}{r['pubN']:>6} {r['market']:<8}{'L1':>10}{SPREAD[r['market']]:>8.2f}{'n/m':>8}{'n/m':>9}{'n/m':>8}  self-managed denominator: no enforced stop")
        continue
    sp = SPREAD[r["market"]]; d = sp / 2.0; pts = float(r["stop_pts"])
    dR = 100.0 * d / pts; c = float(r["costR"])
    out.append((r["label"], r["pubN"], dR, c))
    print(f"{r['label']:<22}{r['pubN']:>6} {r['market']:<8}{pts:>10.2f}{sp:>8.2f}{d:>8.4f}{dR:>8.2f}%{c:>7.2f}%  {100*dR/c:>5.1f}% of it")
print()
print(f"delta/R across the 23 rows with an enforced stop: min {min(o[2] for o in out):.2f}%  max {max(o[2] for o in out):.2f}%  ratio {max(o[2] for o in out)/min(o[2] for o in out):.1f}x")
