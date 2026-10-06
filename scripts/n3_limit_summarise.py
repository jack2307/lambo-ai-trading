"""Read the 24 receipts of the limit-entry registration into one paired table.

The falsifier of docs/decisions/2026-10-06-limit-entry.md, as amended: for each
(mechanism x guards x window) take the BEST offset within an arm FAMILY on sum
net R and compare it against the market arm of the same pairing. Read twice -
on the TRANSLATED family (the registration's real question) and on the ANCHORED
family (a different method, reported separately).
"""
from __future__ import annotations

import glob
import os
import re
import sys

RECEIPTS = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/fd-b3/receipts"
ARMS = ["market", "a000", "a025", "a050", "c025", "c050"]
FAMILIES = [("TRANSLATED (stop carried - the registration's question)", ["c025", "c050"]),
            ("ANCHORED (stop left at the signal bar - a tighter method)", ["a000", "a025", "a050"])]
GATE = (1.200, 0.050, 40)

ROW_RE = re.compile(
    r"^(\S+)\s+(\S+)\s+(\d+)\s+(-?[\d.]+|NaN|inf)\s+([-+]?[\d.]+|NaN)\s+(-?[\d.]+|NaN|inf)\s+"
    r"(-?[\d.]+|NaN|inf)\s+(-?[\d.]+)\s+(\S+)\s\s(.*)$")
SUMR_RE = re.compile(r"sum net R over the window: ([-+][\d.]+) across (\d+) trades")
FILL_RE = re.compile(
    r"resting entry: (\d+) orders placed, (\d+) FILLED, (\d+) expired unfilled, (\d+) replaced"
    r".*?fill rate ([\d.]+)% of the orders that decided \(([\d.]+)% of all placed\)"
    r" vs the null.s median (\S+)")
NOROOM_RE = re.compile(r"(\d+) REFUSED for no room")
WRONG_RE = re.compile(r"\*\* (\d+) of (\d+) positions opened with their own stop on the WRONG SIDE")
EXIT_RE = re.compile(r"^\s+exits: (.*?); mean hold")
STOP_RE = re.compile(r"the method's own realised stop: median ([\d.]+) ATR = ([\d.]+) points")
COST_RE = re.compile(r"cost ([\d.]+)% of R")


def f(x):
    try:
        return float(x)
    except ValueError:
        return float("nan")


def parse(path):
    rows, cur = {}, None
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        if not line.startswith(" ") and not line.startswith("="):
            m = ROW_RE.match(line)
            if m and m.group(1) != "hypothesis":
                cur = m.group(1)
                rows[cur] = dict(base=m.group(2), trades=int(m.group(3)), pf=f(m.group(4)),
                                 expect=f(m.group(5)), null50=f(m.group(6)), null95=f(m.group(7)),
                                 pct=m.group(9), verdict=m.group(10).strip(), sumR=float("nan"),
                                 fill=None, exits="", stop_atr=float("nan"), stop_pts=float("nan"),
                                 cost=float("nan"), no_room=0, wrong=0)
                continue
        if cur is None:
            continue
        r = rows[cur]
        for rx, fn in ((SUMR_RE, lambda m: r.__setitem__("sumR", f(m.group(1)))),
                       (NOROOM_RE, lambda m: r.__setitem__("no_room", int(m.group(1)))),
                       (WRONG_RE, lambda m: r.__setitem__("wrong", int(m.group(1)))),
                       (STOP_RE, lambda m: (r.__setitem__("stop_atr", f(m.group(1))),
                                            r.__setitem__("stop_pts", f(m.group(2))))),
                       (COST_RE, lambda m: r.__setitem__("cost", f(m.group(1)))),
                       (EXIT_RE, lambda m: r.__setitem__("exits", m.group(1).strip()))):
            m = rx.search(line)
            if m:
                fn(m)
        m = FILL_RE.search(line)
        if m:
            r["fill"] = dict(placed=int(m.group(1)), filled=int(m.group(2)), expired=int(m.group(3)),
                             replaced=int(m.group(4)), rate=f(m.group(5)), rate_placed=f(m.group(6)),
                             null_rate=m.group(7))
    return rows


def main() -> int:
    data = {}
    for path in sorted(glob.glob(os.path.join(RECEIPTS, "n3-limit-*.txt"))):
        win, arm, g = os.path.basename(path)[len("n3-limit-"):-len(".txt")].split("-")
        data[(win, arm, g)] = parse(path)
    wins = sorted({k[0] for k in data})
    gs = sorted({k[2] for k in data})
    labels = []
    for v in data.values():
        for lab in v:
            if lab not in labels:
                labels.append(lab)

    print("=" * 132)
    print("EVERY ROW.  sumR = expectancy x trades over the window.  fill% is of the orders that decided.")
    print("stop = the method's OWN realised median stop in this arm (how much the risk unit moved).")
    print("=" * 132)
    print("%-10s %-3s %-9s %-7s %6s %7s %8s %9s %7s %8s %7s %8s %6s %5s" % (
        "mech", "win", "guards", "arm", "trades", "PF", "expect", "sumR", "fill%", "nullfill",
        "null50", "stopATR", "noroom", "wrng"))
    for lab in labels:
        for win in wins:
            for g in gs:
                for arm in ARMS:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None:
                        continue
                    fi = r["fill"]
                    print("%-10s %-3s %-9s %-7s %6d %7.3f %8.3f %+9.2f %7s %8s %7.3f %8.3f %6d %5d" % (
                        lab, win, g, arm, r["trades"], r["pf"], r["expect"], r["sumR"],
                        ("%.1f" % fi["rate"]) if fi else "-", (fi["null_rate"]) if fi else "-",
                        r["null50"], r["stop_atr"], r["no_room"], r["wrong"]))
        print()

    for title, fam in FAMILIES:
        print("=" * 132)
        print("FALSIFIER on %s" % title)
        print("Best offset in the family per pairing, against the market arm, on sum net R.")
        print("=" * 132)
        print("%-10s %-3s %-9s %11s %11s %-7s %11s %7s" % (
            "mech", "win", "guards", "market sumR", "best", "which", "difference", "beat?"))
        beat = failed = 0
        for lab in labels:
            for win in wins:
                for g in gs:
                    mk = data.get((win, "market", g), {}).get(lab)
                    if mk is None:
                        continue
                    best, which = None, None
                    for arm in fam:
                        r = data.get((win, arm, g), {}).get(lab)
                        if r is None:
                            continue
                        if best is None or r["sumR"] > best:
                            best, which = r["sumR"], arm
                    if best is None:
                        continue
                    won = best > mk["sumR"]
                    beat += won
                    failed += not won
                    print("%-10s %-3s %-9s %+11.2f %+11.2f %-7s %+11.2f %7s" % (
                        lab, win, g, mk["sumR"], best, which, best - mk["sumR"],
                        "yes" if won else "NO"))
        n = beat + failed
        thr = -(-2 * n // 3)
        print()
        print("pairings %d: limit beat market in %d, FAILED to beat in %d" % (n, beat, failed))
        print("threshold: fires if it failed in >= %d of %d. FALSIFIER %s" % (
            thr, n, "FIRES" if failed >= thr else "does NOT fire"))
        print()

    print("=" * 132)
    print("GATE: PF >= %.3f AND expectancy >= %+.3fR AND >= %d trades, on BOTH windows." % GATE)
    print("=" * 132)
    passed = []
    for lab in labels:
        for g in gs:
            for arm in ARMS:
                ok = []
                for win in wins:
                    r = data.get((win, arm, g), {}).get(lab)
                    ok.append(r is not None and r["pf"] >= GATE[0] and r["expect"] >= GATE[1]
                              and r["trades"] >= GATE[2] and r["wrong"] == 0)
                if ok and all(ok):
                    passed.append((lab, g, arm))
                    for win in wins:
                        r = data[(win, arm, g)][lab]
                        print("  PASS %-10s %-9s %-7s %s  trades %d PF %.3f exp %+.3f null50 %.3f pct %s" % (
                            lab, g, arm, win, r["trades"], r["pf"], r["expect"], r["null50"], r["pct"]))
    print("cells through the gate on BOTH windows: %d" % len(passed))

    print()
    print("=" * 132)
    print("BREAKEVEN FILL RATE on the TRANSLATED arms, where the market arm has a positive expectancy.")
    print("f* = E_market / E_limit: the share of signals a limit must fill to match the market arm per signal.")
    print("=" * 132)
    print("%-10s %-3s %-9s %-7s %10s %10s %9s %9s" % (
        "mech", "win", "guards", "arm", "E_market", "E_limit", "f*", "actual"))
    for lab in labels:
        for win in wins:
            for g in gs:
                mk = data.get((win, "market", g), {}).get(lab)
                if mk is None:
                    continue
                for arm in ["c025", "c050"]:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None or r["fill"] is None:
                        continue
                    em, el = mk["expect"], r["expect"]
                    star = (em / el * 100.0) if el > 0 else float("nan")
                    print("%-10s %-3s %-9s %-7s %10.4f %10.4f %8.1f%% %8.1f%%" % (
                        lab, win, g, arm, em, el, star, r["fill"]["rate"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
