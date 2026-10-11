# -*- coding: utf-8 -*-
"""The receipt for agent/receipt-reconcile: the 34 rows, their class, their
positive control, their stop size and their spread provenance."""
import json, os, re, sys
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
R = json.load(open(f"{S}/rr/table34c.json"))
V = {(v["label"], v["trades"]): v for v in json.load(open(f"{S}/rr/bars3.json")) if v["indec"]}
L = []
P = L.append
B = "=" * 112

CLASS = {
 "W1": "store BACKFILLED under an omitted --from",
 "W1+C1": "W1 + the Dukascopy flat-bar cleaning inside the window",
 "W2": "store GREW AT THE LIVE END under an omitted --to",
 "P1": "a preset that did not pin a swept grid axis",
 "G1": "the weekend guard cut-off moved, 1655 -> 1640 (config, 2026-09-19)",
 "E1": "one trade at a pinned window edge",
}
STOPKIND = {
 "session-hold": "L1 DENOMINATOR (self-managed, Exits::Strategy): stop = riskDailyRanges x rangeDays NY daily range; the engine never enforces it, so cost/R is NOT MEASURED here and is not invented",
}
P(B); P("WHY 34 DECISION-RECORD ROWS NO LONGER RE-RUN TO THEIR PUBLISHED NUMBER - NO GATE")
P("Registration: docs/decisions/2026-10-11-receipt-reconcile.md (classification table declared before any run)")
P("Binary: /e/rust/fd-stop-width/target-sw/release/search.exe (= 29c8635, lbar_line). Nothing built on this axis.")
P(B)
P(f"{'population':<62}{'count':>8}")
P(f"{'  vanished band keys rebuilt from agent/pfr-audit artifacts':<62}{51:>8}")
P(f"{'  of them sitting in docs/decisions/  <- THE SET':<62}{34:>8}")
P(f"{'  distinct decision docs they sit in':<62}{len({d for r in R for d in r[chr(100)+chr(111)+chr(99)+chr(115)]}):>8}")
P("")
P("THE ANSWER IN ONE LINE")
P("  23 of 34 return their PUBLISHED TRADE COUNT **exactly** once the thing that moved is pinned back.")
P("  15 of those also return the published PF to printed width. 0 of 34 are a PF_usd-vs-PF_r unit mistake.")
P(B)
P("CLASSIFICATION - every row, with the positive control that assigns it")
P(B)
P(f"{'label':<22}{'pubN':>6}{'pubPF':>8}{'argvN':>8}{'ctlN':>7}{'ctlPF':>8}{'=?':>5}  {'class':<7}{'market':<9}{'stop':>10}{'cost/R':>8}")
for r in sorted(R, key=lambda x: (x["cls"], x["label"], x["pubN"])):
    ok = "YES" if r["ctlN"] == r["pubN"] else ("n/a" if r["ctlN"] is None else f"{r['ctlN']-r['pubN']:+d}")
    P(f"{r['label']:<22}{r['pubN']:>6}{r['pubPF']:>8.3f}"
      f"{(r['argvN'] if r['argvN'] is not None else '-'):>8}"
      f"{(r['ctlN'] if r['ctlN'] is not None else '-'):>7}"
      f"{(r['ctlPF'] or '-'):>8}{ok:>5}  {r['cls']:<7}{r['market']:<9}"
      f"{(r['stop_atr']+'A' if r['stop_atr'] else 'L1'):>10}"
      f"{((r['costR']+'%') if r['costR'] else 'n/m'):>8}")
P("")
P("  pubN/pubPF  the published row.   argvN  the trade count the VERBATIM published argv prints today.")
P("  ctlN/ctlPF  the POSITIVE CONTROL: the same argv with the moved thing pinned back.")
P("  stop        '<x>A' = x ATR of the run's bar size, from the engine's own cost-matched-null line;")
P("              'L1' = a self-managed DENOMINATOR stop (session-hold), never enforced - addendum 7 B.")
P("  cost/R      from the engine's own 'cost X% of R' line. 'n/m' = NOT MEASURED, not 0.")
P(B)
for c in ("W1", "W1+C1", "W2", "P1", "G1", "E1"):
    rows = [r for r in R if r["cls"] == c]
    ex = sum(1 for r in rows if r["ctlN"] == r["pubN"])
    P(f"{c:<7} {CLASS[c]:<62} rows {len(rows):>2}  exact return {ex:>2}")
P(B)
open(f"{S}/rr/TABLE-34-rows.txt", "w", encoding="utf-8").write("\n".join(L) + "\n")
print("\n".join(L))
