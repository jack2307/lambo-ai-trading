"""Read the 24 stop-entry receipts into one paired table.

The falsifier of docs/decisions/2026-10-07-stop-entry.md: for each
(mechanism x guards x window) take the BEST offset within an arm FAMILY on sum
net R and compare it against the market arm of the same pairing. Read on the
TRANSLATED family (the registration's question) and again on the ANCHORED
family (a wider-stopped method, reported separately).

Three things the brief asks for by name and this prints:
  * the fill rate AND the null's fill rate, and the paired difference;
  * every row with >= 40 trades that sits BELOW its own null's median PF;
  * which rows are resolution-unsafe (small stop), and the falsifier restated
    with them dropped.
"""
from __future__ import annotations

import glob
import os
import re
import statistics
import sys

RECEIPTS = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/fd-stop-entry/receipts"
PREFIX = "stopentry-"
ARMS = ["market", "b000", "b025", "b050", "t025", "t050"]
FAMILIES = [("TRANSLATED (stop carried - the registration's question)", ["t025", "t050"]),
            ("ANCHORED (stop left at the signal bar - a WIDER-stopped method)", ["b000", "b025", "b050"])]
GATE = (1.200, 0.050, 40)
# Below this the 15m fill verdict is not evidence; n3 measured 66.0% agreement
# and 24.8% artifacts at 0.3 ATR against 100.0% at 2.7 ATR.
SAFE_STOP_ATR = 0.70

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
PREFILL_RE = re.compile(r"\*\* (\d+) of (\d+) fills \(([\d.]+)%\) were closed on their OWN fill bar")
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
                                 cost=float("nan"), no_room=0, wrong=0, prefill=0, prefill_pct=0.0)
                continue
        if cur is None:
            continue
        r = rows[cur]
        for rx, fn in ((SUMR_RE, lambda m: r.__setitem__("sumR", f(m.group(1)))),
                       (NOROOM_RE, lambda m: r.__setitem__("no_room", int(m.group(1)))),
                       (WRONG_RE, lambda m: r.__setitem__("wrong", int(m.group(1)))),
                       (PREFILL_RE, lambda m: (r.__setitem__("prefill", int(m.group(1))),
                                               r.__setitem__("prefill_pct", f(m.group(3))))),
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
    for path in sorted(glob.glob(os.path.join(RECEIPTS, PREFIX + "*.txt"))):
        stem = os.path.basename(path)[len(PREFIX):-len(".txt")]
        parts = stem.split("-")
        if len(parts) != 3:
            continue
        win, arm, g = parts
        data[(win, arm, g)] = parse(path)
    if not data:
        print("no receipts under %s" % RECEIPTS)
        return 1
    wins = sorted({k[0] for k in data})
    gs = sorted({k[2] for k in data})
    labels = []
    for v in data.values():
        for lab in v:
            if lab not in labels:
                labels.append(lab)

    # Which mechanisms are resolution-safe, judged on their MARKET arm's own
    # realised stop (the arm whose geometry the signal designed).
    stop_of = {}
    for lab in labels:
        vals = [data[(w, "market", g)][lab]["stop_atr"] for w in wins for g in gs
                if lab in data.get((w, "market", g), {})]
        vals = [v for v in vals if v == v]
        stop_of[lab] = statistics.median(vals) if vals else float("nan")
    unsafe = {lab for lab, v in stop_of.items() if not (v >= SAFE_STOP_ATR)}

    print("=" * 146)
    print("EVERY ROW. sumR = expectancy x trades over the window. fill%% is of the orders that decided.")
    print("stop = this arm's OWN realised median stop. preOpen = fills closed on their own fill bar at")
    print("that bar's OPEN (a price before the entry) - counted, not corrected.")
    print("resolution-unsafe mechanisms (market-arm stop < %.2f ATR): %s" % (
        SAFE_STOP_ATR, ", ".join("%s (%.3f ATR)" % (l, stop_of[l]) for l in sorted(unsafe)) or "none"))
    print("=" * 146)
    print("%-10s %-3s %-9s %-7s %6s %7s %8s %9s %7s %8s %7s %7s %8s %6s %5s %7s" % (
        "mech", "win", "guards", "arm", "trades", "PF", "expect", "sumR", "fill%", "nullfill",
        "null50", "null95", "stopATR", "noroom", "wrng", "preOpen"))
    for lab in labels:
        for win in wins:
            for g in gs:
                for arm in ARMS:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None:
                        continue
                    fi = r["fill"]
                    print("%-10s %-3s %-9s %-7s %6d %7.3f %8.3f %+9.2f %7s %8s %7.3f %7.3f %8.3f %6d %5d %7s%s" % (
                        lab, win, g, arm, r["trades"], r["pf"], r["expect"], r["sumR"],
                        ("%.1f" % fi["rate"]) if fi else "-", (fi["null_rate"]) if fi else "-",
                        r["null50"], r["null95"], r["stop_atr"], r["no_room"], r["wrong"],
                        ("%d/%.0f%%" % (r["prefill"], r["prefill_pct"])) if r["prefill"] else "-",
                        "  <- resolution-unsafe" if lab in unsafe and arm != "market" else ""))
        print()

    # ---------------------------------------------------------------- falsifier
    for title, fam in FAMILIES:
        for drop_unsafe in (False, True):
            tag = " (resolution-unsafe mechanisms DROPPED)" if drop_unsafe else ""
            print("=" * 146)
            print("FALSIFIER on %s%s" % (title, tag))
            print("Best offset in the family per pairing, against the market arm, on sum net R.")
            print("=" * 146)
            print("%-10s %-3s %-9s %11s %11s %-7s %11s %7s" % (
                "mech", "win", "guards", "market sumR", "best", "which", "difference", "beat?"))
            beat = failed = 0
            for lab in labels:
                if drop_unsafe and lab in unsafe:
                    continue
                for win in wins:
                    for g in gs:
                        mk = data.get((win, "market", g), {}).get(lab)
                        if mk is None:
                            continue
                        best, which = None, None
                        for arm in fam:
                            r = data.get((win, arm, g), {}).get(lab)
                            if r is None or r["sumR"] != r["sumR"]:
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
            print("pairings %d: breakout beat market in %d, FAILED to beat in %d" % (n, beat, failed))
            print("threshold: fires if it failed in >= %d of %d. FALSIFIER %s" % (
                thr, n, "FIRES" if failed >= thr else "does NOT fire"))
            print()

    # --------------------------------------------------------------------- gate
    print("=" * 146)
    print("GATE: PF >= %.3f AND expectancy >= %+.3fR AND >= %d trades, on BOTH windows." % GATE)
    print("=" * 146)
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

    # ------------------------------------------- the fill rate against the null
    print("=" * 146)
    print("DOES THE ORDER SELECT ANYTHING? method fill rate MINUS its matched null's, in points of rate.")
    print("A breakout order fills when price ran the signal's way. If the method's rate is its null's,")
    print("the signals carry NO information about whether price runs, and the arm measures only the order.")
    print("=" * 146)
    diffs, lower, rows = [], 0, 0
    per_arm = {}
    for lab in labels:
        for win in wins:
            for g in gs:
                for arm in ARMS:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None or not r["fill"]:
                        continue
                    nr = r["fill"]["null_rate"]
                    if not nr.endswith("%"):
                        continue
                    d = r["fill"]["rate"] - float(nr[:-1])
                    diffs.append(d)
                    rows += 1
                    lower += d < 0
                    per_arm.setdefault(arm, []).append(d)
    if diffs:
        print("rows compared: %d" % rows)
        print("median method-minus-null: %+.2f points of rate" % statistics.median(diffs))
        print("mean   method-minus-null: %+.2f points of rate" % statistics.fmean(diffs))
        print("the method filled LESS than its null in %d of %d rows" % (lower, rows))
        print()
        print("%-7s %6s %9s %9s %9s" % ("arm", "rows", "median", "mean", "min"))
        for arm in ARMS:
            v = per_arm.get(arm)
            if v:
                print("%-7s %6d %+9.2f %+9.2f %+9.2f" % (arm, len(v), statistics.median(v),
                                                         statistics.fmean(v), min(v)))
    else:
        print("null fill rate not printed on any row: NOT MEASURED (not 0)")
    print()

    # ----------------------------------------------- rows under their own null
    print("=" * 146)
    print("ROWS WITH >= 40 TRADES THAT SIT BELOW THEIR OWN MATCHED NULL'S MEDIAN PROFIT FACTOR.")
    print("n3 found 36 of these in the pullback family, its biggest wins among them.")
    print("=" * 146)
    print("%-10s %-3s %-9s %-7s %6s %7s %7s %8s" % (
        "mech", "win", "guards", "arm", "trades", "PF", "null50", "PF-null50"))
    under = tot = 0
    for lab in labels:
        for win in wins:
            for g in gs:
                for arm in ARMS:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None or r["trades"] < 40 or r["pf"] != r["pf"] or r["null50"] != r["null50"]:
                        continue
                    tot += 1
                    if r["pf"] < r["null50"]:
                        under += 1
                        print("%-10s %-3s %-9s %-7s %6d %7.3f %7.3f %+8.3f%s" % (
                            lab, win, g, arm, r["trades"], r["pf"], r["null50"], r["pf"] - r["null50"],
                            "  <- resolution-unsafe" if lab in unsafe and arm != "market" else ""))
    print()
    print("%d of %d rows with >= 40 trades are BELOW their own null's median PF" % (under, tot))
    print()

    # ---------------------------------------------------- breakeven fill rate
    print("=" * 146)
    print("BREAKEVEN FILL RATE on the TRANSLATED arms, where the MARKET arm has a positive expectancy.")
    print("f* = E_market / E_breakout: the share of signals the arm must fill to match the market arm")
    print("per SIGNAL. Above 100%% means no fill rate closes the gap - it is worse per fill as well.")
    print("=" * 146)
    print("%-10s %-3s %-9s %-7s %10s %10s %9s %9s" % (
        "mech", "win", "guards", "arm", "E_market", "E_brkout", "f*", "actual"))
    any_row = False
    for lab in labels:
        for win in wins:
            for g in gs:
                mk = data.get((win, "market", g), {}).get(lab)
                if mk is None or not (mk["expect"] > 0):
                    continue
                for arm in ["t025", "t050"]:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None or r["fill"] is None:
                        continue
                    any_row = True
                    em, el = mk["expect"], r["expect"]
                    star = (em / el * 100.0) if el > 0 else float("nan")
                    print("%-10s %-3s %-9s %-7s %10.4f %10.4f %8.1f%% %8.1f%%  %s" % (
                        lab, win, g, arm, em, el, star, r["fill"]["rate"],
                        "(market arm trades %d)" % mk["trades"]))
    if not any_row:
        print("no pairing has a market arm with positive expectancy: NOT MEASURED (not 0)")
    print()

    # ------------------------------------------------------------- exit mix
    print("=" * 146)
    print("EXIT MIX - brief 4.iii: a row whose own rule never fires is not a result.")
    print("=" * 146)
    for lab in labels:
        for win in wins:
            for g in gs:
                for arm in ARMS:
                    r = data.get((win, arm, g), {}).get(lab)
                    if r is None or not r["exits"]:
                        continue
                    print("%-10s %-3s %-9s %-7s %s" % (lab, win, g, arm, r["exits"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
