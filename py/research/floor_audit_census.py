"""C1/C2 of docs/decisions/2026-10-07-floor-audit.md: parse the harvested
receipt blobs for the trade count, the verdict, and the `guards:` header line.

Defect A - `fd-backtest::sweep::verdict` compares `metrics.trades` with
`PromisingGate::min_trades`, which `config/default.toml` sets to 30 and which
the binary prints as `only <n> trades (need 30)`. The desk floor is 40, so a
row with 30..39 trades clears the engine's trade-count leg silently while it
does not reach the desk's. That is a DISAGREEMENT between two floors; it is not
by itself an error in the row.

Defect B - `search.rs::guards_line` prints `guards: on - ...` /
`guards: off (every number unguarded)` in the header of every run, but only
since that line existed. Three populations are kept apart: the arm recorded in
the header; the arm recoverable only from the echoed argv (guards are enabled
by nothing but `--guards`); and the arm not readable at all.

Counts only. Nothing is re-run and no receipt is rewritten.
"""
import os
import re
import sys
import json
from collections import defaultdict, Counter

OUT = sys.argv[1]
BLOBS = os.path.join(OUT, "blobs")

# Printed column names that are more than one word. Every one of them sits
# AFTER the `trades` column in every table in the record, which is what makes
# the index of `trades` identical in the header and in a data row.
MULTIWORD = [
    ("OOS PF", "OOS_PF"), ("null p50", "null_p50"), ("null p95", "null_p95"),
    ("PF gross", "PF_gross"), ("PF net", "PF_net"), ("total R", "total_R"),
    ("net $", "net_$"), ("top-5 share", "top5_share"),
    ("PF w/o top-5", "PF_wo_top5"), ("mean hold", "mean_hold"),
    ("per year", "per_year"), ("null fill", "null_fill"),
]
POS = re.compile(r"(SURVIVES|PASSES ALL THREE|gate pass|\bPASS\b|\bPASSES\b"
                 r"|\bpromising\b|\bsurvives\b)")
NEG = re.compile(r"^(not promising|no )", re.I)
CMD = re.compile(r"^\s*\$ .*(search|three_month|designed|slice)[._]|^\s*\$ .*\.exe")
GUARDS = re.compile(r"^\s*guards:\s*(on|off)\b")
ENGINE = re.compile(r"^(market:|bounds:|bars: |spread: |swap:|timeline:)")
INT = re.compile(r"^-?\d+$")
SOME = re.compile(r"^Some\((-?\d+)\)$")


def header_of(line):
    """(index of the trades column, tokens) if this line is a table header."""
    if not line.strip() or any(c in line for c in ":;=.,"):
        return None
    s = line
    for a, b in MULTIWORD:
        s = s.replace(a, b)
    toks = s.split()
    if len(toks) < 4 or any(INT.match(t) for t in toks):
        return None
    for name in ("trades", "wfTrades"):
        if name in toks:
            return toks.index(name), toks
    return None


def trades_of(tok):
    if INT.match(tok):
        return int(tok)
    m = SOME.match(tok)
    return int(m.group(1)) if m else None


def parse(path, sha):
    rows = []
    guards, cmds, engine = set(), [], False
    hdr_idx = hdr = pipe_hdr = None
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for ln, raw in enumerate(fh, 1):
            line = raw.rstrip("\n")
            if CMD.match(line):
                cmds.append(line.strip())
                hdr_idx = hdr = None
                continue
            m = GUARDS.match(line)
            if m:
                guards.add(m.group(1))
                continue
            if ENGINE.match(line):
                engine = True
            if line.count("|") >= 5 and not line.strip().startswith("|"):
                parts = [p.strip() for p in line.split("|")]
                if pipe_hdr is None and not any(INT.match(p) for p in parts):
                    pipe_hdr = parts
                elif pipe_hdr and "trades" in pipe_hdr:
                    j = pipe_hdr.index("trades")
                    n = trades_of(parts[j]) if j < len(parts) else None
                    if n is not None:
                        rows.append(row(sha, ln, line, n, parts[0], "pipe"))
                continue
            h = header_of(line)
            if h:
                hdr_idx, hdr = h
                continue
            if hdr_idx is None:
                continue
            toks = line.split()
            if len(toks) <= hdr_idx:
                continue
            n = trades_of(toks[hdr_idx])
            if n is not None:
                rows.append(row(sha, ln, line, n, toks[0], " ".join(hdr)))
    if guards:
        cls = "1 header"
    elif cmds:
        cls = "2 argv only"
    elif engine:
        cls = "3 not readable"
    else:
        cls = "4 not an engine receipt"
    return rows, dict(sha=sha, guards=sorted(guards), n_cmd=len(cmds),
                      n_cmd_guards=sum(1 for c in cmds if "--guards" in c),
                      cls=cls, n_rows=len(rows))


def row(sha, ln, line, n, label, hdr):
    text = re.sub(r"\s+", " ", line.strip())
    return dict(sha=sha, line_no=ln, trades=n, label=label, header=hdr,
                positive=bool(POS.search(line)) and not NEG.match(text),
                text=text)


def main():
    inv = defaultdict(lambda: {"branches": set(), "paths": set()})
    with open(os.path.join(OUT, "inventory.tsv"), encoding="utf-8") as fh:
        for line in fh:
            b, sha, p = line.rstrip("\n").split("\t", 2)
            if p.endswith((".txt", ".md", ".log")):
                inv[sha]["branches"].add(b)
                inv[sha]["paths"].add(p)

    rows, files = [], {}
    for sha in sorted(inv):
        p = os.path.join(BLOBS, sha + ".txt")
        if not os.path.exists(p):
            continue
        r, f = parse(p, sha)
        f["paths"] = sorted(inv[sha]["paths"])
        f["branches"] = sorted(inv[sha]["branches"])
        files[sha] = f
        rows.extend(r)

    # de-duplicate on the printed text and the trade count, carrying every
    # path and branch the row was found on.
    d = {}
    for r in rows:
        k = (r["text"], r["trades"])
        e = d.setdefault(k, dict(r, paths=set(), branches=set(), shas=set(),
                                 guards=set(), classes=set()))
        f = files[r["sha"]]
        e["paths"].update(f["paths"])
        e["branches"].update(f["branches"])
        e["shas"].add(r["sha"])
        e["guards"].update(f["guards"])
        e["classes"].add(f["cls"])
    uniq = [{k: (sorted(v) if isinstance(v, set) else v) for k, v in r.items()}
            for r in d.values()]

    json.dump(uniq, open(os.path.join(OUT, "rows.json"), "w"), indent=0)
    json.dump(list(files.values()), open(os.path.join(OUT, "files.json"), "w"), indent=0)
    print(f"blobs parsed            : {len(files)}")
    print(f"table rows parsed       : {len(rows)}")
    print(f"distinct printed rows   : {len(uniq)}")
    for k, v in sorted(Counter(f["cls"] for f in files.values()).items()):
        print(f"  guards class {k:<26} {v:>5} blobs")


if __name__ == "__main__":
    main()
