"""C2 of docs/decisions/2026-10-07-intrabar-resolution.md: the published record,
binned by the stop it actually ran at.

Every receipt that carries the printed line

    the method's own realised stop: median <x> ATR = <y> points over <n> trades

is parsed with the row it belongs to. Complete census, no selection.
"""
import os
import re
import sys
import json
from collections import defaultdict

ROW = re.compile(
    r"^(?P<label>\S+)\s+(?P<base>\S+)\s+(?P<trades>-?\d+)\s+"
    r"(?P<pf>[-\d.]+|nan|inf)\s+(?P<exp>[-\d.]+|nan)\s+"
    r"(?P<p50>[-\d.]+|nan)\s+(?P<p95>[-\d.]+|nan)\s+"
    r"(?P<swap>-?[\d.]+)\s+(?P<pct>-?\d+|null)%\s+(?P<verdict>.*)$"
)
STOPLINE = re.compile(
    r"the method's own realised stop: median\s+(?P<atr>[-\d.]+) ATR = "
    r"(?P<pts>[-\d.]+) points over (?P<n>\d+) trades"
)
CTRL = re.compile(
    r"cost-matched null: control stop\s+([-\d.]+) ATR = ([-\d.]+) points, "
    r"cost\s+([-\d.]+)% of R \(([^)]*)\)"
)
MARKET = re.compile(r"^market:\s+(\S+)\s+(\S+)")
DATA = re.compile(r"^data:\s+(\S+)")
EXITS = re.compile(r"^\s+exits: (.*?); mean hold")


def parse(path, origin):
    out = []
    market = tf = data = None
    cur = None
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            m = MARKET.match(line)
            if m:
                market, tf = m.group(1), m.group(2)
                continue
            m = DATA.match(line)
            if m:
                data = m.group(1)
                continue
            if line and not line[0].isspace():
                m = ROW.match(line)
                cur = None
                if m and m.group("label") != "hypothesis":
                    cur = dict(origin=origin, file=os.path.basename(path),
                               path=path, market=market, tf=tf, data=data,
                               label=m.group("label"), base=m.group("base"),
                               trades=int(m.group("trades")),
                               pf=m.group("pf"), exp=m.group("exp"),
                               p50=m.group("p50"), p95=m.group("p95"),
                               pct=m.group("pct"),
                               verdict=m.group("verdict").strip(),
                               stop_atr=None, stop_pts=None, stop_n=None,
                               ctrl_src=None, cost_pct=None, exits=None)
                    out.append(cur)
                continue
            if cur is None:
                continue
            m = STOPLINE.search(line)
            if m:
                cur["stop_atr"] = float(m.group("atr"))
                cur["stop_pts"] = float(m.group("pts"))
                cur["stop_n"] = int(m.group("n"))
                continue
            m = CTRL.search(line)
            if m:
                cur["ctrl_src"] = m.group(4)
                cur["cost_pct"] = float(m.group(3))
                continue
            m = EXITS.match(line)
            if m:
                cur["exits"] = m.group(1)
    return out


def collect(roots):
    rows = []
    for origin, root in roots:
        for dirpath, _dirs, files in os.walk(root):
            for f in files:
                if not f.endswith((".txt", ".log")):
                    continue
                p = os.path.join(dirpath, f)
                try:
                    rows.extend(parse(p, origin))
                except Exception as e:  # noqa: BLE001
                    print(f"  ! {p}: {e}", file=sys.stderr)
    return rows


BANDS = [(0.0, 0.30, "NOT EVIDENCE  (>5% of verdicts are resolution artifacts)"),
         (0.30, 0.60, "caveat        (1-5%)"),
         (0.60, 99.0, "readable      (<=1%)")]


def band(atr):
    for lo, hi, name in BANDS:
        if lo <= atr < hi or (hi == 99.0 and atr >= lo):
            return name
    return "?"


def main():
    sp = os.environ.get("INTRABAR_RECEIPTS")
    roots = [("branch-receipts", sp)] if sp else []
    roots.append(("docs/research/runs (HEAD)", "E:/rust/fd-intrabar/docs/research/runs"))
    rows = collect(roots)
    with_stop = [r for r in rows if r["stop_atr"] is not None]
    print(f"rows parsed: {len(rows)};  rows carrying a realised stop: {len(with_stop)}")

    # de-duplicate: the same (file basename, label, trades, stop) appears on
    # several branches because branches copy each other's receipts.
    seen = {}
    for r in with_stop:
        key = (r["file"], r["label"], r["trades"], r["stop_atr"], r["market"], r["tf"])
        seen.setdefault(key, r)
    uniq = list(seen.values())
    print(f"distinct published rows (file+label+trades+stop+market+tf): {len(uniq)}")

    print(f"\n{'='*104}")
    print("THE PUBLISHED RECORD, BINNED BY THE STOP IT ACTUALLY RAN AT")
    print(f"{'='*104}")
    bins = defaultdict(list)
    for r in uniq:
        bins[band(r["stop_atr"])].append(r)
    for _lo, _hi, name in BANDS:
        g = bins.get(name, [])
        n40 = [r for r in g if r["trades"] >= 40]
        print(f"{name:<58} {len(g):>5} rows  ({len(n40)} with >= 40 trades)")
    tot = len(uniq)
    below = len(bins.get(BANDS[0][2], [])) + len(bins.get(BANDS[1][2], []))
    print(f"{'-'*104}")
    print(f"{'TOTAL':<58} {tot:>5} rows;  {below} ({100*below/tot:.1f}%) sit below "
          f"the 0.600 ATR threshold")

    print(f"\n{'='*104}")
    print("EVERY ROW BELOW 0.600 ATR, BY NAME  (stop | ATR | pts | trades | PF | "
          "expect | pct | market tf | label / base)")
    print(f"{'='*104}")
    low = sorted([r for r in uniq if r["stop_atr"] < 0.60], key=lambda r: r["stop_atr"])
    for r in low:
        print(f"{r['stop_atr']:>6.3f} {r['stop_pts']:>7.2f}pt {r['trades']:>6} "
              f"PF {r['pf']:>6} exp {r['exp']:>7} pct {r['pct']:>4}% "
              f"{r['market']:>8} {r['tf']:>4}  {r['label']} / {r['base']}  "
              f"[{r['origin']}:{r['file']}]")
    if not low:
        print("  none")

    print(f"\n{'='*104}")
    print("DISTINCT STOP SIZES IN THE RECORD, with the row count at each "
          "(rounded to 0.05 ATR)")
    print(f"{'='*104}")
    hist = defaultdict(int)
    for r in uniq:
        hist[round(r["stop_atr"] / 0.05) * 0.05] += 1
    for k in sorted(hist):
        print(f"{k:>6.2f} ATR  {hist[k]:>5} rows  {'#' * min(60, hist[k])}")

    print(f"\n{'='*104}")
    print("BY BASE STRATEGY: the realised stop range the record actually used")
    print(f"{'='*104}")
    bybase = defaultdict(list)
    for r in uniq:
        bybase[r["base"]].append(r["stop_atr"])
    for b in sorted(bybase, key=lambda b: min(bybase[b])):
        v = sorted(bybase[b])
        mid = v[len(v) // 2]
        print(f"{b:<24} {len(v):>5} rows  min {v[0]:>6.3f}  median {mid:>6.3f}  "
              f"max {v[-1]:>6.3f}  -> {band(mid)}")

    # the sealed tape, found while doing this and reported because the brief
    # says the sealed data has never been consumed
    sealed = sorted({r["path"] for r in uniq if r["data"] and "data-sealed" in r["data"]})
    print(f"\nreceipts whose own `data:` header names data-sealed: {len(sealed)}")
    for p in sealed[:10]:
        print(f"  {p}")

    with open("../../receipts/intrabar-published-rows.json", "w") as f:
        json.dump(uniq, f, indent=1)
    print(f"\n{len(uniq)} rows -> receipts/intrabar-published-rows.json")


if __name__ == "__main__":
    main()
