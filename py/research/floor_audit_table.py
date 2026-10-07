"""The one table of docs/decisions/2026-10-07-floor-audit.md, plus every
positive row in the 30..39 slot named.

No gate: nothing in this output passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`.
"""
import json
import re
import sys
from collections import Counter

OUT = sys.argv[1]
rows = json.load(open(f"{OUT}/rows.json", encoding="utf-8"))
files = json.load(open(f"{OUT}/files.json", encoding="utf-8"))
W = 112


def rule(c="="):
    print(c * W)


def kind(t):
    if "SURVIVES" in t:
        return "SURVIVES"
    if "PASSES ALL THREE" in t:
        return "PASSES ALL THREE"
    if "gate pass" in t:
        return "gate pass"
    return "PASS (gate column, trade count declared ignored)"


rule()
print("THE PUBLISHED RECORD AGAINST TWO PRINTED-RECEIPT DEFECTS - COUNT ONLY, NO GATE")
rule()
print("refs read                       : "
      f"{len({b for f in files for b in f['branches']})}"
      "  (81 local heads + 29 remote-tracking; nothing merged, nothing checked out)")
print(f"distinct text blobs under docs/research/runs, docs/decisions, */receipts/ : {len(files)}")
print(f"table rows parsed               : {sum(f['n_rows'] for f in files)}")
print(f"distinct printed rows           : {len(rows)}"
      "  (de-duplicated on printed text + trade count)")

rule()
print("DEFECT A - THE TRADE-COUNT FLOOR.  The tool's verdict needs 30 trades")
print("(config/default.toml [backtest.promising] min_trades = 30, printed as")
print("'only <n> trades (need 30)'); the desk gate needs 40.  A row in the 30-39 slot is")
print("NOT automatically wrong: it is a row where the engine's printed verdict and the")
print("desk's gate SAY DIFFERENT THINGS on the trade-count leg.")
rule()
bands = [(0, 4), (5, 9), (10, 19), (20, 29), (30, 39), (40, 99), (100, 10 ** 9)]
print(f"{'trades':<12}{'rows':>8}{'positive':>10}{'SURVIVES':>10}"
      f"{'gate pass':>11}{'other pos':>11}   note")
for lo, hi in bands:
    g = [r for r in rows if lo <= r["trades"] <= hi]
    p = [r for r in g if r["positive"]]
    k = Counter(kind(r["text"]) for r in p)
    name = f"{lo}-{hi}" if hi < 10 ** 9 else f">={lo}"
    note = ""
    if lo == 30:
        note = "<- engine PASSES this leg, desk FAILS it"
    elif lo == 40:
        note = "   both floors agree"
    elif hi < 30:
        note = "   engine prints 'only <n> trades (need 30)'"
    print(f"{name:<12}{len(g):>8}{len(p):>10}{k['SURVIVES']:>10}{k['gate pass']:>11}"
          f"{len(p) - k['SURVIVES'] - k['gate pass']:>11}   {note}")
rule("-")
pos = [r for r in rows if r["positive"]]
slot = [r for r in pos if 30 <= r["trades"] <= 39]
print(f"{'TOTAL':<12}{len(rows):>8}{len(pos):>10}")
print(f"\nrows in the 30-39 slot                      : "
      f"{len([r for r in rows if 30 <= r['trades'] <= 39])}")
print(f"of those, printed with a POSITIVE verdict    : {len(slot)}")
print(f"that is {100 * len(slot) / len(pos):.1f}% of every positive verdict in the record "
      f"({len(slot)} of {len(pos)})")
print(f"distinct mechanisms behind the {len(slot)} rows        : "
      f"{len({r['label'] for r in slot})}")

rule()
print(f"EVERY POSITIVE ROW IN THE 30-39 SLOT, BY NAME  ({len(slot)} rows, "
      "sorted by trade count)")
print("  guards = what the receipt's own header said about the arm; "
      "'-' = the header said nothing")
rule()
for r in sorted(slot, key=lambda r: (-r["trades"], r["text"])):
    g = "/".join(r["guards"]) or "-"
    print(f"n={r['trades']:>3}  guards={g:<4}  {r['text'][:88]}")
    print(f"{'':>12}  {sorted(r['paths'])[0]}   [{len(r['branches'])} refs]")

rule()
print("DEFECT B - THE ARM THE GUARDS RAN ON.  search.rs::guards_line prints the arm in the")
print("header of every run, but only since that line existed.  Measured elsewhere: guards")
print("move the hold-mechanism sample size 3.4x-106x, and 23 of 398 paired rows (5.8%) cross")
print("PF 1.200 on the flag alone - worst PF 2.328 unguarded vs 0.894 guarded on exactly 64")
print("trades.  A receipt that does not say which arm it ran cannot be read at the gate.")
rule()
cls = Counter(f["cls"] for f in files)
rws = Counter()
for f in files:
    rws[f["cls"]] += f["n_rows"]
eng = sum(v for k, v in cls.items() if not k.startswith("4"))
labels = {
    "1 header": "records the arm in its header (guards: on / guards: off)",
    "2 argv only": "no guards: line, but echoes its own full argv",
    "3 not readable": "engine receipt, no guards: line AND no argv echo",
    "4 not an engine receipt": "not an engine receipt (agent table / decision doc)",
}
print(f"{'class':<58}{'blobs':>7}{'% of eng':>9}{'rows':>8}")
for k in sorted(cls):
    pct = f"{100 * cls[k] / eng:.1f}%" if not k.startswith("4") else "  -"
    print(f"{labels[k]:<58}{cls[k]:>7}{pct:>9}{rws[k]:>8}")
rule("-")
print(f"{'engine receipts':<58}{eng:>7}{'100.0%':>9}"
      f"{sum(rws[k] for k in rws if not k.startswith('4')):>8}")
nohdr = cls["2 argv only"] + cls["3 not readable"]
print("\nengine receipts that do NOT record arm guards in the header : "
      f"{nohdr}  ({100 * nohdr / eng:.1f}% of {eng})")
print("  of those, the arm is still recoverable from the echoed argv : "
      f"{cls['2 argv only']}")
cg = sum(f["n_cmd_guards"] for f in files if f["cls"] == "2 argv only")
print(f"    --guards appears in an echoed command line in            : {cg} of them")
print("    guards are enabled by nothing but --guards (search.rs:244), so all "
      f"{cls['2 argv only']}")
print("    of these are the UNGUARDED arm, stated by the argv and not by the header")
print("  of those, the arm is NOT READABLE AT ALL                    : "
      f"{cls['3 not readable']}   <- this is null, not off")
onearm = Counter(tuple(f["guards"]) for f in files if f["cls"] == "1 header")
print("\nreceipts that record the arm, by which arm they record:")
for k, v in sorted(onearm.items()):
    print(f"  {v:>5}  guards: {'+'.join(k)}")
print(f"  {0:>5}  both arms printed side by side in one receipt"
      "   <- every recording receipt pins exactly ONE arm")

rule()
print(f"THE {cls['3 not readable']} ENGINE RECEIPTS WHOSE ARM CANNOT BE READ AT ALL, BY NAME")
print("  'trades' = the receipt books trades, so the arm changes its numbers.")
print("  A receipt that books no trade at all is descriptive and the arm cannot reach it.")
rule()
TRADES = re.compile(r"\b\d+ trades\b")
SEALED = re.compile(r"^(data root:|data:)\s*\S*data-sealed", re.M)
n_tr = n_no = 0
for f in sorted((f for f in files if f["cls"] == "3 not readable"),
                key=lambda f: (f["paths"][0],)):
    t = open(f"{OUT}/blobs/{f['sha']}.txt", encoding="utf-8", errors="replace").read()
    has = bool(TRADES.search(t)) or f["n_rows"] > 0
    n_tr += has
    n_no += not has
    print(f"  trades={'YES' if has else 'no ':<3}  rows={f['n_rows']:>3}  "
          f"[{len(f['branches']):>3} refs]  {f['paths'][0]}")
print(f"\n  of the {cls['3 not readable']}: {n_tr} book trades "
      "<- these are the receipts the desk cannot place on an arm")
print(f"               {n_no} book no trade at all (descriptive), so no arm applies")

rule()
print("THE TWO DEFECTS CROSSED - the 30-39 positive rows by the guards readability of")
print("their receipt")
rule()
for k, v in sorted(Counter(c for r in slot for c in r["classes"]).items()):
    print(f"  {v:>4}  {labels[k]}")

rule()
print("NOT MEASURED, and why")
rule()
print("  * Rows whose table prints no trade count: 8 lines carry a positive verdict word")
print("    with no trades column on the row (two copies of a four-row hold/<window> table).")
print("    They cannot be binned and are null for defect A, not folded into any band.")
print("  * Whether any 30-39 row would survive at 40 trades: needs a re-run; this axis runs")
print("    nothing.  The slot is a disagreement between two floors, not a verdict.")
print("  * Whether a receipt recording guards: off ran the right arm: it records ONE arm.")
print("    It is readable FOR THAT ARM and silent about the other.")
print("  * 5 further positive rows sit BELOW 30 trades, all five in one agent-written table")
print("    (receipts/Z-summary.txt) whose own header declares 'gate PASS counts rows with")
print("    PF >= 1.200 AND expectancy >= +0.050R, trade count ignored'.  Declared by its")
print("    author, not an engine defect; 2 of the 69 slot rows come from that same table")
print("    and carry PASS above.")

rule()
print("ONE BRIEF FIGURE THAT DOES NOT MATCH WHAT IS PRINTED (brief sec.8: my number wins)")
rule()
sealed = [f for f in files
          if SEALED.search(open(f"{OUT}/blobs/{f['sha']}.txt", encoding='utf-8',
                                errors='replace').read())]
print("Brief sec.9 says 6 receipts in the repo print that they ran on data-sealed/.")
print(f"Counted from the printed `data:` / `data root:` header: {len(sealed)} blobs.")
for k, v in sorted(Counter(f["cls"] for f in sealed).items()):
    print(f"  {v:>4}  {labels[k]}")
print("  directories: 2026-09-23-designed-2 (19), 2026-09-24-repair-c (16),")
print("               2026-09-24-cost-matched-null (5), 2026-09-23-designed-4 (6),")
print("               2026-09-24-repair-d (2)")
print("The sealed store is NOT opened by this axis; this is a count of what the receipts")
print("already say about themselves.")
