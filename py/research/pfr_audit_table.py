"""The tables of docs/decisions/2026-10-09-pf-unit-audit.md.

Pairs every cell of the re-runs written by `pfr_audit_run.py` with the
published band row it reproduces, and counts the verdicts that cross PF 1.200
when the leg is read in R instead of USD.

NO GATE: nothing in this output passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`.
This axis checks the UNIT of the PF leg, not a mechanism.

    argv: OUT [subdir]
"""
import json
import os
import re
import statistics as st
import sys
from collections import Counter

OUT = sys.argv[1]
SUB = sys.argv[2] if len(sys.argv) > 2 else "runs1"

# Both modules below are REUSED BYTE-IDENTICAL (floor_audit_census.py from
# `agent/floor-audit`, gate_legs_check.py from `agent/gate-legs`) and both read
# sys.argv at import time, so argv is masked across the import rather than
# editing either file.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
_argv, sys.argv = sys.argv, [sys.argv[0], OUT]
from floor_audit_census import header_of, trades_of          # reused, unchanged
from gate_legs_check import num                              # reused, unchanged
sys.argv = _argv
PF_MIN = 1.200
W = 112
LBAR = re.compile(
    r"Lbar ([-\d.]+) R = \|avg_loss_r\| ([-\d.]+) x loss share ([-\d.]+) over (\d+) trades; "
    r"PF_r ([-\d.]+) \(R\) vs PF_usd ([-\d.]+) \(USD, gap ([-+\d.]+)\); "
    r"identity E = Lbar\(PF_r-1\) = ([-+\d.]+) R vs expectancy ([-+\d.]+) R, residual ([-+\d.]+) R")
NOTMEAS = re.compile(r"Lbar: NOT MEASURED[^\n]*")
DD = re.compile(r"max drawdown ([-\d.]+) USD = ([-\d.]+)% of the book")
# starting_equity_usd = 100.0 in config/default.toml -> a cent book, defect 17
CENT = {"xauusd", "eurusd", "btcusd"}


def rule(c="="):
    print(c * W)


def parse_run(path):
    cells, cur, hdr_idx, hdr, market, interval = [], None, None, None, None, None
    for raw in open(path, encoding="utf-8", errors="replace"):
        line = raw.rstrip("\n")
        m = re.match(r"^market:\s+(\S+)\s+(\S+)", line)
        if m:
            market, interval = m.group(1), m.group(2)
            continue
        m = NOTMEAS.search(line)
        if m and cur is not None:
            cur["lbar_absent"] = m.group(0)[:150]
            continue
        m = LBAR.search(line)
        if m and cur is not None:
            g = [float(x) for x in m.groups()]
            cur.update(lbar=g[0], avg_loss_r=g[1], loss_share=g[2],
                       lbar_trades=int(g[3]), pf_r=g[4], pf_usd=g[5],
                       gap=g[6], implied_e=g[7], e_lbar=g[8], resid=g[9])
            continue
        m = DD.search(line)
        if m and cur is not None:
            cur["dd_usd"], cur["dd_pct"] = float(m.group(1)), float(m.group(2))
            continue
        h = header_of(line)
        if h:
            hdr_idx, hdr = h
            continue
        s = line.strip()
        if hdr_idx is None or not s or line[:1].isspace() or line.startswith("#"):
            continue
        toks = line.split()
        if len(toks) <= hdr_idx:
            continue
        n = trades_of(toks[hdr_idx])
        if n is None:
            continue
        cell = dict(label=toks[0], trades=n, market=market, interval=interval,
                    text=re.sub(r"\s+", " ", s)[:150], lbar=None,
                    pf_tbl=None, e_tbl=None)
        for pc, ec in (("OOS_PF", "expect"), ("PF", "expect"), ("PF", "expR"),
                       ("PF", "expectancy")):
            if pc in hdr and ec in hdr:
                i, j = hdr.index(pc), hdr.index(ec)
                if max(i, j) < len(toks):
                    cell["pf_tbl"], cell["e_tbl"] = num(toks[i]), num(toks[j])
                break
        cells.append(cell)
        cur = cell
    return cells


def load():
    rows = json.load(open(f"{OUT}/gate_legs_rows.json", encoding="utf-8"))
    repro = json.load(open(f"{OUT}/repro_band.json", encoding="utf-8"))
    index = json.load(open(f"{OUT}/{SUB}/index.json", encoding="utf-8"))
    by_cmd, bad = {}, []
    for h, c in index.items():
        p = f"{OUT}/{SUB}/{h}.txt"
        if not os.path.exists(p):
            continue
        # the two echoed command lines can be long, so the `# exit` line can
        # sit well past the first few hundred characters.
        t = open(p, encoding="utf-8", errors="replace").read(4000)
        if "# exit 0" not in t:
            bad.append((h, c))
            continue
        by_cmd[c] = parse_run(p)

    pub = {}
    for r in rows:
        if not (0.700 <= r["pf"] <= 1.700):
            continue
        k = f'{r["label"]}|{r["trades"]}|{round(r["pf"],3)}|{round(r["e"],3)}'
        e = pub.setdefault(k, dict(label=r["label"], trades=r["trades"],
                                   pf=r["pf"], e=r["e"], paths=set(),
                                   positive=False))
        e["paths"].update(r["paths"])
        e["positive"] |= r["positive"]

    paired, unrun, nomatch, misparse = [], [], [], []
    for k, cmds in repro.items():
        p = pub.get(k)
        if p is None:
            continue
        got = None
        for c in cmds:
            for cell in by_cmd.get(c, []):
                if cell["label"] == p["label"] and cell["trades"] == p["trades"]:
                    got = (c, cell)
                    break
            if got:
                break
        if got is None:
            (unrun if not any(c in by_cmd for c in cmds) else nomatch).append(dict(p, key=k))
            continue
        c, cell = got
        if cell.get("lbar") is not None and cell["lbar_trades"] != cell["trades"]:
            misparse.append(dict(p, key=k))
            continue
        paired.append(dict(p, key=k, cmd=c, cell=cell))
    return rows, repro, index, pub, by_cmd, bad, paired, unrun, nomatch, misparse


def dec(p):
    return [x for x in p if x.startswith("docs/decisions/")]


def side(pf):
    return "pass" if pf >= PF_MIN else "fail"


def main():
    (rows, repro, index, pub, by_cmd, bad, paired, unrun, nomatch,
     misparse) = load()
    nb = len([r for r in rows if 0.700 <= r["pf"] <= 1.700])

    rule()
    print("THE UNIT OF THE PF LEG, READ ACROSS THE PUBLISHED RECORD - NO GATE")
    print("Registration: docs/decisions/2026-10-09-pf-unit-audit.md (band declared before any run)")
    rule()
    print(f"{'refs scanned':<52}{131:>8}")
    print(f"{'distinct printed rows harvested':<52}{10802:>8}")
    print(f"{'rows carrying BOTH a PF and an expectancy':<52}{len(rows):>8}")
    print(f"{'IN THE DECLARED BAND PF_usd in [0.700, 1.700]':<52}{nb:>8}  rows")
    print(f"{'  distinct (label, trades, PF, E) keys':<52}{len(pub):>8}")
    print(f"{'  keys reproducible from an echoed command':<52}{len(repro):>8}   <- declared re-run set")
    print(f"{'  commands declared':<52}{len(index):>8}")
    print(f"{'  commands that completed (exit 0)':<52}{len(by_cmd):>8}")
    print(f"{'  commands that did not':<52}{len(bad):>8}")
    print(f"{'PAIRED CELLS (same label AND same trade count)':<52}{len(paired):>8}")
    print(f"{'  of those printing a finite Lbar':<52}"
          f"{len([r for r in paired if r['cell'].get('lbar') is not None]):>8}")
    print(f"{'  Lbar NOT MEASURED (no losing trade / no position)':<52}"
          f"{len([r for r in paired if r['cell'].get('lbar') is None]):>8}   <- null, not 0.000 R")
    print(f"{'not paired: command not run':<52}{len(unrun):>8}")
    print(f"{'not paired: ran, no cell with that label+trades':<52}{len(nomatch):>8}")
    print(f"{'not paired: Lbar trade count disagreed with row':<52}{len(misparse):>8}")

    lb = [r for r in paired if r["cell"].get("lbar") is not None]
    if not lb:
        print("\nno cell with a finite Lbar yet - nothing to read")
        return

    rule()
    print("REQUIRED CHECK - THE IDENTITY  E = Lbar x (PF_r - 1)  ON MY OWN RE-RUNS")
    print("The first place the identity is known to break is engine.rs:1070-1076 (USD mixed")
    print("with R). A cell of mine outside ~1e-4 R would be a SECOND place, and that is worth")
    print("more than the result.")
    rule()
    res = sorted(lb, key=lambda r: -abs(r["cell"]["resid"]))
    ok = [r for r in lb if abs(r["cell"]["resid"]) <= 1e-4]
    print(f"cells checked                                   : {len(lb)}")
    print(f"within 1e-4 R                                   : {len(ok)}  "
          f"({100*len(ok)/len(lb):.1f}%)")
    print(f"largest |residual| of E = Lbar(PF_r - 1)         : "
          f"{abs(res[0]['cell']['resid']):.5f} R")
    print("the ten largest residuals:")
    for r in res[:10]:
        c = r["cell"]
        print(f"   {c['resid']:+.5f} R  {r['label']:<28} n={r['trades']:<5} "
              f"Lbar {c['lbar']:.4f} R  PF_r {c['pf_r']:.4f}  PF_usd {c['pf_usd']:.4f}")
    off = [r for r in lb if abs(r["cell"]["resid"]) > 1e-4]
    if off:
        print(f"\nCELLS OUTSIDE 1e-4 R ({len(off)}) - A SECOND PLACE THE IDENTITY BREAKS:")
        for r in off:
            c = r["cell"]
            print(f"   {c['resid']:+.5f} R  {r['label']:<28} n={r['trades']:<5} "
                  f"{c['market']} {c['interval']}  Lbar {c['lbar']:.4f}  PF_r {c['pf_r']:.4f}")
    else:
        print("\nNo cell is outside 1e-4 R: the identity holds in R on every cell I ran.")

    rule()
    print("THE GAP  PF_usd - PF_r  ON THE SAME TRADE SET (same run, same cell, same trades)")
    rule()
    g = [abs(r["cell"]["gap"]) for r in lb]
    gs = sorted(g)
    print(f"cells                : {len(lb)}")
    print(f"|gap| median         : {st.median(g):.4f}")
    print(f"|gap| p90            : {gs[int(0.9*(len(gs)-1))]:.4f}")
    print(f"|gap| MAX            : {max(g):.4f}   (agent/stop-width measured 0.3640 on 200 cells)")
    print(f"|gap| > 0.3640       : {len([x for x in g if x > 0.3640])} cells  "
          "<- wider than anything stop-width measured")
    print(f"|gap| > 0.5000       : {len([x for x in g if x > 0.5000])} cells  "
          "<- outside the half-width of my declared band")
    print("\nsign of the gap against the sign of the book (PF_usd >= 1 = a winning book):")
    sc = Counter()
    for r in lb:
        c = r["cell"]
        sc[("book up  " if c["pf_usd"] >= 1.0 else "book down",
            "PF_usd flattered" if c["gap"] > 0 else
            "PF_usd penalised" if c["gap"] < 0 else "equal")] += 1
    for k, v in sorted(sc.items()):
        print(f"   {k[0]:<10} {k[1]:<18} {v:>6}")
    print("\nthe twelve widest gaps:")
    for r in sorted(lb, key=lambda r: -abs(r["cell"]["gap"]))[:12]:
        c = r["cell"]
        print(f"   gap {c['gap']:+.4f}  PF_usd {c['pf_usd']:.4f} -> PF_r {c['pf_r']:.4f}  "
              f"{r['label']:<26} n={r['trades']:<5} {c['market']} {c['interval']}")

    rule()
    print("REQUIRED COUNT 1 - VERDICTS THAT CROSS PF 1.200 WHEN THE LEG IS READ IN R")
    print("Two readings, reported separately and never mixed:")
    print("  (A) PUBLISHED PF_usd  vs  re-run PF_r  - the published verdict's own side")
    print("  (B) re-run  PF_usd    vs  re-run PF_r  - the pure unit effect inside one run")
    rule()
    flips = {}
    for tag, getpf in (("A", lambda r: r["pf"]),
                       ("B", lambda r: r["cell"]["pf_usd"])):
        hi = [r for r in lb if getpf(r) >= PF_MIN and r["cell"]["pf_r"] < PF_MIN]
        lo = [r for r in lb if getpf(r) < PF_MIN and r["cell"]["pf_r"] >= PF_MIN]
        flips[tag] = (hi, lo)
        nm = ("published PF_usd vs re-run PF_r" if tag == "A"
              else "re-run PF_usd vs re-run PF_r")
        print(f"\n({tag}) {nm}   cells {len(lb)}")
        print(f"   FLIP, the record reads TOO HIGH (PF_usd >= 1.200, PF_r <  1.200) : {len(hi):>4}")
        print(f"   FLIP, the record reads TOO LOW  (PF_usd <  1.200, PF_r >= 1.200) : {len(lo):>4}")
        print(f"   TOTAL FLIPS                                                      : "
              f"{len(hi)+len(lo):>4}   ({100*(len(hi)+len(lo))/len(lb):.1f}% of cells read)")

    rule()
    print("REQUIRED COUNT 1 (cont.) - EVERY FLIPPING ROW BY NAME, reading A (the published verdict)")
    print("  DECISION = the row sits in docs/decisions/; RECEIPT = it only ever sat in a receipt.")
    rule()
    for name, sel in (
            ("THE RECORD READS TOO HIGH (published PF_usd >= 1.200, PF_r < 1.200)", flips["A"][0]),
            ("THE RECORD READS TOO LOW  (published PF_usd <  1.200, PF_r >= 1.200)", flips["A"][1])):
        print(f"\n{name}   -   {len(sel)} rows")
        if not sel:
            print("   none")
            continue
        for r in sorted(sel, key=lambda r: -abs(r["cell"]["gap"])):
            c = r["cell"]
            d = dec(r["paths"])
            book = ("CENT 100 USD book, min-lot clamped (defect 17)"
                    if c["market"] in CENT else "10,000 USD book")
            print(f"   {'DECISION' if d else 'RECEIPT '} {r['label']:<26} n={r['trades']:<5} "
                  f"published PF_usd {r['pf']:.3f} | re-run PF_usd {c['pf_usd']:.4f} -> "
                  f"PF_r {c['pf_r']:.4f}")
            print(f"{'':>12} Lbar {c['lbar']:.4f} R, E {r['e']:+.3f} R, "
                  f"{c['market']} {c['interval']}, {book}"
                  f"{'  [POSITIVE verdict printed]' if r['positive'] else ''}")
            for x in sorted(d):
                print(f"{'':>12} {x}")

    rule()
    print("REQUIRED COUNT 2 - DECISION RECORD vs RECEIPT ONLY")
    print("A flip inside docs/decisions/ weighs more than a flip that only ever sat in a receipt.")
    rule()
    print(f"{'population':<44}{'cells':>7}{'flips A':>9}{'flips B':>9}{'too high':>10}{'too low':>9}")
    for nm, sel in (("paired cells, ALL", lb),
                    ("  in docs/decisions/ (DECISION RECORD)", [r for r in lb if dec(r["paths"])]),
                    ("  receipt only", [r for r in lb if not dec(r["paths"])]),
                    ("  printed a POSITIVE verdict", [r for r in lb if r["positive"]]),
                    ("  >= 40 trades (the desk floor)", [r for r in lb if r["trades"] >= 40])):
        a_hi = [r for r in sel if r["pf"] >= PF_MIN and r["cell"]["pf_r"] < PF_MIN]
        a_lo = [r for r in sel if r["pf"] < PF_MIN and r["cell"]["pf_r"] >= PF_MIN]
        b = len([r for r in sel if side(r["cell"]["pf_usd"]) != side(r["cell"]["pf_r"])])
        print(f"{nm:<44}{len(sel):>7}{len(a_hi)+len(a_lo):>9}{b:>9}{len(a_hi):>10}{len(a_lo):>9}")

    rule()
    print("REQUIRED COUNT 3 - SPLIT BY INSTRUMENT AND BOOK (defect 17)")
    print("config/default.toml: starting_equity_usd = 100.0 for xauusd/eurusd/btcusd (a CENT")
    print("book), so raw_lots < min_lot 0.01 is clamped and PF_usd there is the PF of a")
    print("0.01-lot book, not of a 1%-risk book. The other markets take the 10,000 USD")
    print("default. PF_usd on the two is NOT the same quantity.")
    rule()
    print(f"{'market':<12}{'book USD':>10}{'cells':>7}{'|gap| med':>11}{'|gap| max':>11}"
          f"{'flips A':>9}{'flips B':>9}")
    for mkt in sorted({r["cell"]["market"] for r in lb if r["cell"]["market"]}):
        sel = [r for r in lb if r["cell"]["market"] == mkt]
        gg = sorted(abs(r["cell"]["gap"]) for r in sel)
        a = len([r for r in sel if side(r["pf"]) != side(r["cell"]["pf_r"])])
        b = len([r for r in sel if side(r["cell"]["pf_usd"]) != side(r["cell"]["pf_r"])])
        print(f"{mkt:<12}{'100' if mkt in CENT else '10,000':>10}{len(sel):>7}"
              f"{gg[len(gg)//2]:>11.4f}{gg[-1]:>11.4f}{a:>9}{b:>9}")

    rule()
    print("WHAT I COULD NOT HOLD FIXED - THE RECORD DOES NOT RE-RUN TO ITS OWN PF_usd")
    print("Same command, same batch file, same window, same trade count - but the binary has")
    print("moved since publication. Reported SEPARATELY, never folded into a flip count.")
    rule()
    dr = [(r, r["cell"]["pf_usd"] - r["pf"]) for r in lb]
    dd = sorted(abs(x) for _, x in dr)
    exact = len([x for x in dd if x <= 0.0005])
    print(f"paired cells (trade count identical, so the TRADE SET is the same) : {len(lb)}")
    print(f"  re-run PF_usd equal to published to print width (<= 0.0005)      : {exact}"
          f"  ({100*exact/len(lb):.1f}%)")
    print(f"  |published PF_usd - re-run PF_usd| median                        : {dd[len(dd)//2]:.4f}")
    print(f"  |published PF_usd - re-run PF_usd| max                           : {dd[-1]:.4f}")
    print("  the eight widest drifts:")
    for r, x in sorted(dr, key=lambda t: -abs(t[1]))[:8]:
        print(f"    {x:+.4f}  {r['label']:<26} n={r['trades']:<5} published {r['pf']:.3f} "
              f"-> re-run {r['cell']['pf_usd']:.4f}  {r['cell']['market']}")

    rule()
    print("IS THE DECLARED BAND WIDE ENOUGH?  measured over EVERY cell of EVERY completed run,")
    print("not only the paired ones - the widest |PF_usd - PF_r| anywhere bounds how far from")
    print("1.200 a row can sit and still cross it.")
    rule()
    allc = [c for cells in by_cmd.values() for c in cells
            if c.get("lbar") is not None]
    ag = sorted(abs(c["gap"]) for c in allc)
    print(f"cells with a finite Lbar in the re-runs          : {len(allc)}")
    if ag:
        print(f"|PF_usd - PF_r| median                          : {ag[len(ag)//2]:.4f}")
        print(f"|PF_usd - PF_r| p99                             : {ag[int(0.99*(len(ag)-1))]:.4f}")
        print(f"|PF_usd - PF_r| MAX                             : {ag[-1]:.4f}")
        print(f"declared band half-width                        : 0.5000")
        print(f"cells whose gap exceeds the band half-width     : "
              f"{len([x for x in ag if x > 0.5])}")
        worst = max(allc, key=lambda c: abs(c["gap"]))
        print(f"the widest gap anywhere: {worst['label']} n={worst['trades']} "
              f"{worst['market']} {worst['interval']}  "
              f"PF_usd {worst['pf_usd']:.4f} -> PF_r {worst['pf_r']:.4f}")
        if ag[-1] <= 0.5:
            print("\n=> No cell anywhere in the re-runs has a gap as wide as the band's half-width,")
            print("   so on this evidence a published row OUTSIDE [0.700, 1.700] could not have")
            print("   crossed 1.200 either. The band was declared wide enough and measured so.")
        else:
            print("\n=> At least one cell has a gap wider than the band half-width: the band was")
            print("   NOT wide enough and rows outside it are not covered by this evidence.")

    rule()
    print("PUBLISHED ROWS WHOSE COMMAND RAN BUT WHOSE CELL NO LONGER EXISTS, BY NAME")
    print("The label is still printed by the same batch file, but the TRADE COUNT has moved,")
    print("so there is no cell with the published trade count to read a PF_r off. This is a")
    print("reproducibility defect of the record, not a flip; it is counted, never folded in.")
    rule()
    if not nomatch:
        print("  none")
    else:
        # SENSITIVITY, so these rows are not an unquantified hole: for each one
        # take the re-run cell of the SAME label whose trade count is nearest,
        # and ask whether THAT cell would flip. It is a different trade set, so
        # it can never enter a flip count - it only bounds what was missed.
        cells_by_label = {}
        for cells in by_cmd.values():
            for c in cells:
                if c.get("lbar") is not None:
                    cells_by_label.setdefault(c["label"], []).append(c)
        near, nearflip, close5 = 0, [], 0
        for r in nomatch:
            cs = cells_by_label.get(r["label"], [])
            if not cs:
                continue
            c = min(cs, key=lambda c: abs(c["trades"] - r["trades"]))
            d = abs(c["trades"] - r["trades"])
            if d <= max(5, 0.02 * r["trades"]):
                near += 1
                if d <= 5:
                    close5 += 1
                if side(r["pf"]) != side(c["pf_r"]):
                    nearflip.append((r, c))
        print(f"  rows whose nearest surviving cell is within 2% or 5 trades : {near} of {len(nomatch)}")
        print(f"    of those, within 5 trades                               : {close5}")
        print(f"    of those NEAREST cells that would sit on the other side  : {len(nearflip)}")
        for r, c in nearflip:
            print(f"      {'DECISION' if dec(r['paths']) else 'RECEIPT '} {r['label']:<24} "
                  f"published n={r['trades']} PF_usd {r['pf']:.3f}  vs nearest cell "
                  f"n={c['trades']} PF_r {c['pf_r']:.4f} (PF_usd {c['pf_usd']:.4f})")
        print("    (a different trade set, so NOT a flip - a bound on what the 51 could hide)")
        print()
        bylabel = {}
        for cells in by_cmd.values():
            for c in cells:
                bylabel.setdefault(c["label"], []).append(c["trades"])
        shown = 0
        for r in sorted(nomatch, key=lambda r: r["label"]):
            now = sorted(set(bylabel.get(r["label"], [])))
            where = "DECISION" if dec(r["paths"]) else "RECEIPT "
            print(f"  {where} {r['label']:<28} published n={r['trades']:<6} PF_usd {r['pf']:.3f}"
                  f"   now prints n={now if now else 'the label is gone'}")
            shown += 1
            if shown >= 60:
                print(f"  ... and {len(nomatch) - shown} more")
                break

    rule()
    print("NOT MEASURED, and why (null, not 0)")
    rule()
    print(f"  * {nb - len(repro)} band rows of {nb} have NO echoed command in any blob they were")
    print("    found on - most are agent-written summary tables, which print a row but not the")
    print("    argv that made it. UNMEASURED on this axis, NOT shown to be unflippable.")
    print(f"  * {len(rows) - nb} rows carrying a PF sit OUTSIDE the declared band. Declared")
    print("    unmeasured in advance, on the stated ground that reaching 1.200 from there needs")
    print("    a USD-vs-R gap wider than 1.37x the largest ever measured.")
    print(f"  * {len(unrun)} reproducible keys whose command has not completed; {len(nomatch)} whose")
    print("    command ran but printed no cell with that label and trade count (the batch file")
    print("    or the engine has moved since publication).")
    print("  * No percentile and no verdict WORD is read from a re-run: --seeds=1 was used after")
    print("    MEASURING that it leaves the method row and the whole Lbar line byte-identical.")
    print("    The verdict side is read from the PUBLISHED row.")
    print("  * Drawdown is printed by this binary but is not a criterion of this axis")
    print("    (addendum 5 G: do not promise a drawdown figure on a non-drawdown job).")
    rule()


if __name__ == "__main__":
    main()
