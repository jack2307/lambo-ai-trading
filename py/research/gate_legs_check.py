"""Q1/Q3/Q4 of docs/decisions/2026-10-07-gate-legs.md.

Reads `rows.json` produced by `floor_audit_census.py` (itself fed by
`floor_audit_harvest.py` - BOTH REUSED UNCHANGED from `agent/floor-audit`;
this file adds no harvesting of its own) and checks every distinct printed row
that carries BOTH a profit factor and an expectancy against the pure
STOP/TARGET identity derived in Q1:

    PF = rr * (1 + E) / (rr - E)          E in R, rr = reward_risk

A row that matches exited only at its stop or its target. A row that does NOT
match exited some other way (TIMEOUT / WEEKEND_FLAT / NEWS_FLAT / a
strategy-issued exit), which is what makes the mismatch the finding rather
than an error.

No gate: nothing here passes or fails anything. `search.exe` is not invoked.
"""
import json
import re
import sys
from collections import Counter

OUT = sys.argv[1]
RR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.8
PF_MIN, E_MIN = 1.200, 0.050

# Printed PF column -> printed expectancy column, in the order a row is read.
# `wholePF` has no expectancy column beside it and is therefore never paired.
PAIRS = [("PF", "expect"), ("PF", "expR"), ("PF", "expectancy"), ("PF", "exp"),
         ("OOS_PF", "expect"), ("OOS_PF", "expectancy"),
         ("PF_net", "expect"), ("PF_gross", "expect"),
         ("wfPF", "wfExpect")]
NUM = re.compile(r"^[+-]?(?:\d+\.?\d*|\.\d+)$")
INT = re.compile(r"^-?\d+$")
SOME = re.compile(r"^Some\((-?[\d.]+)\)$")


def num(tok):
    """A printed cell as a float, or None when the cell is not a number."""
    tok = tok.strip().rstrip(",;")
    if NUM.match(tok):
        return float(tok)
    m = SOME.match(tok)
    if m:
        return float(m.group(1))
    if tok.endswith("R") and NUM.match(tok[:-1]):      # `+0.098R`
        return float(tok[:-1])
    if tok.endswith("%") and NUM.match(tok[:-1]):      # a percent is not an R
        return None
    return None


def predict_pf(e, rr=RR):
    """The PF a pure STOP/TARGET book with this expectancy must print."""
    if e >= rr:
        return None            # every trade a winner: no losing side, PF = inf
    return rr * (1.0 + e) / (rr - e)


def read(row):
    """(pf, expectancy, pf_col, e_col) read off one printed row, or None."""
    hdr = row["header"].split()
    if "trades" not in hdr and "wfTrades" not in hdr:
        return None
    anchor = hdr.index("trades") if "trades" in hdr else hdr.index("wfTrades")
    toks = row["text"].split()
    # Alignment check: the token at the header's own `trades` index must be
    # the trade count the census already read off this row. Without it a row
    # whose label contains a space would read the wrong columns in silence.
    if len(toks) <= anchor:
        return None
    a = num(toks[anchor])
    if a is None or int(a) != row["trades"]:
        return None
    for pc, ec in PAIRS:
        if pc in hdr and ec in hdr:
            i, j = hdr.index(pc), hdr.index(ec)
            if max(i, j) >= len(toks):
                continue
            pf, e = num(toks[i]), num(toks[j])
            if pf is None or e is None:
                continue
            return pf, e, pc, ec
    return None


def main():
    rows = json.load(open(f"{OUT}/rows.json", encoding="utf-8"))
    read_rows = []
    for r in rows:
        got = read(r)
        if got:
            pf, e, pc, ec = got
            pred = predict_pf(e)
            read_rows.append(dict(r, pf=pf, e=e, pf_col=pc, e_col=ec,
                                  pf_pred=pred,
                                  rel=None if not pred else (pf - pred) / pred))
    json.dump(read_rows, open(f"{OUT}/gate_legs_rows.json", "w"), indent=0)
    print(f"distinct printed rows in the harvest        : {len(rows)}")
    print(f"rows carrying BOTH a PF and an expectancy   : {len(read_rows)}")
    print(f"  by printed column pair:")
    for k, v in Counter((r["pf_col"], r["e_col"]) for r in read_rows).most_common():
        print(f"    {k[0]:<10} + {k[1]:<12} {v:>6}")


if __name__ == "__main__":
    main()


# ---------------------------------------------------------------- Q3 and Q4
def bands(read_rows):
    """The residual distribution of the Q1 identity over the record."""
    out = Counter()
    for r in read_rows:
        rel = r["rel"]
        if rel is None:
            out["E >= rr (no losing side; PF undefined)"] += 1
        else:
            a = abs(rel)
            out["<=0.5% of predicted PF (exact to print width)" if a <= 0.005
                else "<=1%" if a <= 0.01
                else "<=5%" if a <= 0.05
                else "<=20%" if a <= 0.20
                else ">20%"] += 1
    return out


def cells(read_rows):
    """Q4: the two legs crossed, as four cells."""
    out = {("fail", "fail"): [], ("fail", "pass"): [],
           ("pass", "fail"): [], ("pass", "pass"): []}
    for r in read_rows:
        k = ("pass" if r["pf"] >= PF_MIN else "fail",
             "pass" if r["e"] >= E_MIN else "fail")
        out[k].append(r)
    return out
