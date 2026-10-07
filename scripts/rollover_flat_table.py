#!/usr/bin/env python3
"""Turn the six rollover-flat receipts into the tables the report needs.

Reads nothing but the receipts. Every number it prints is a number the engine
printed, or an arithmetic combination of two printed numbers whose formula is
named in the output. Nothing is assumed about a row that the receipt did not
say.

Usage: python scripts/rollover_flat_table.py receipts/rollover-flat
"""
import os
import re
import sys
from collections import OrderedDict

ROWS = ["px-1s", "qs-h15", "ts-l20", "ts-l60", "qsf-h15", "qsf-h5", "tsf-l20", "tsf-l60"]
FLAT = {"qsf-h15", "qsf-h5", "tsf-l20", "tsf-l60"}
# label -> the baseline it is the variant of
PARENT = {"qsf-h15": "qs-h15", "qsf-h5": "qs-h15", "tsf-l20": "ts-l20", "tsf-l60": "ts-l60"}

MAIN = re.compile(
    r"^(?P<label>\S+)\s+(?P<base>\S+)\s+(?P<trades>\d+)\s+(?P<pf>[-\d.]+|inf|NaN)\s+"
    r"(?P<exp>[-+\d.]+|NaN)\s+(?P<p50>[-\d.]+|NaN)\s+(?P<p95>[-\d.]+|NaN)\s+"
    r"(?P<swap>[-\d.]+)\s+(?P<pct>\d+%|null)\s+(?P<verdict>.*)$"
)


def parse(path):
    out = OrderedDict()
    cur = None
    header = {}
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("guards:"):
                header["guards"] = line.split(":", 1)[1].strip()
            if line.startswith("swap:") and "swap" not in header:
                header["swap"] = line.split(":", 1)[1].strip()
            if line.startswith("bounds:"):
                header["bounds"] = line.split(":", 1)[1].strip()
            if line.startswith("flags:"):
                header["flags"] = line.split(":", 1)[1].strip()
            if line.startswith("news:") and "news" not in header:
                header["news"] = line.split(":", 1)[1].strip()
            m = MAIN.match(line)
            if m and m.group("label") in ROWS:
                cur = dict(m.groupdict())
                cur["trades"] = int(cur["trades"])
                for k in ("pf", "exp", "p50", "p95", "swap"):
                    try:
                        cur[k] = float(cur[k])
                    except ValueError:
                        cur[k] = float("nan")
                out[cur["label"]] = cur
                continue
            if cur is None:
                continue
            s = line.strip()
            m2 = re.search(
                r"expectancy_net ([-+\d.]+) R and total_r_net ([-+\d.]+) R .*?"
                r"vs expectancy ([-+\d.]+) R and total_r ([-+\d.]+) R",
                s,
            )
            if m2:
                cur["exp_net"] = float(m2.group(1))
                cur["total_r_net"] = float(m2.group(2))
                cur["total_r"] = float(m2.group(4))
            if s.startswith("expectancy_net: NOT MEASURED"):
                cur["exp_net"] = None
            m2 = re.search(r"spread paid: method ([\d.]+) USD vs the null's median ([\d.]+) USD — cost match ([\d.]+)", s)
            if m2:
                cur["spread"] = float(m2.group(1))
                cur["cost_match"] = float(m2.group(3))
                cur["cost_band"] = "outside the band" in s
            m2 = re.search(r"matched null ([\d.]+|NaN) trades median vs the method's (\d+) — count match ([\d.]+|NaN)", s)
            if m2:
                cur["count_match"] = m2.group(3)
                cur["count_band"] = "outside the band" in s
            m2 = re.search(r"long share: method ([\d.]+) vs the null's median ([\d.]+)", s)
            if m2:
                cur["long_share"] = float(m2.group(1))
                cur["null_long_share"] = float(m2.group(2))
            m2 = re.search(r"time in market method (\d+) min vs the null's median ([\d.]+) min — ratio ([\d.]+)", s)
            if m2:
                cur["time_in_market"] = int(m2.group(1))
                cur["exposure_ratio"] = float(m2.group(3))
            m2 = re.search(r"signed share of time method ([-+\d.]+)", s)
            if m2:
                cur["signed_share"] = float(m2.group(1))
            m2 = re.search(r"^exits: (.*); mean hold ([\d.]+) min$", s)
            if m2:
                cur["exits_raw"] = m2.group(1)
                cur["mean_hold"] = float(m2.group(2))
    return header, out


def exit_counts(raw):
    """`reason N, reason N, ...` -> {reason: N}, with the long-tailed
    per-percentage `return flipped` reasons folded into one key."""
    counts = {}
    if not raw:
        return counts
    for part in raw.split(", "):
        m = re.match(r"^(.*?) (\d+)$", part)
        if not m:
            continue
        reason, n = m.group(1), int(m.group(2))
        if "return flipped" in reason:
            reason = "<own rule> return flipped"
        elif re.match(r"^held \d+ sessions of \d+$", reason):
            reason = "<own rule> hold clock"
        elif reason == "window closed":
            reason = "<own rule> window closed"
        counts[reason] = counts.get(reason, 0) + n
    return counts


def gate(r, use_net=True):
    """PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades, counted by hand.
    Returns (passes, which legs failed)."""
    exp = r.get("exp_net", r["exp"]) if use_net else r["exp"]
    if exp is None:
        return False, ["expectancy_net NOT MEASURED"]
    bad = []
    if not (r["pf"] >= 1.200):
        bad.append("PF %.3f" % r["pf"])
    if not (exp >= 0.050):
        bad.append("exp %+.3fR" % exp)
    if r["trades"] < 40:
        bad.append("%d trades" % r["trades"])
    return (not bad), bad


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else "receipts/rollover-flat"
    legs = ["is", "oos"]
    data = {}
    for arm in "ABC":
        for leg in legs:
            p = os.path.join(d, "%s-duka-%s.txt" % (arm, leg))
            if os.path.exists(p):
                data[(arm, leg)] = parse(p)

    print("=" * 100)
    print("HEADERS")
    for k, (h, _) in sorted(data.items()):
        print(" arm %s %-4s guards=%-4s | swap=%s" % (k[0], k[1], h.get("guards", "?")[:3], h.get("swap", "?")))
        print("              bounds=%s" % h.get("bounds", "?"))
        print("              flags=%s" % h.get("flags", "?"))
        print("              news=%s" % h.get("news", "?"))

    for arm in "ABC":
        if (arm, "is") not in data:
            continue
        print()
        print("=" * 100)
        print("ARM %s   %s" % (arm, data[(arm, "is")][0].get("swap", "")))
        print("%-9s %-17s %6s %7s %8s %9s %8s %8s %7s %6s %7s %s"
              % ("row", "base", "trades", "PF", "exp", "exp_net", "swap$", "spread$", "hold_h", "pct", "p50", "gate"))
        for leg in legs:
            if (arm, leg) not in data:
                continue
            _, rows = data[(arm, leg)]
            print("-- %s" % leg.upper())
            for label in ROWS:
                r = rows.get(label)
                if not r:
                    print(" %-9s MISSING" % label)
                    continue
                en = r.get("exp_net", r["exp"])
                ok, bad = gate(r)
                print(" %-9s %-17s %6d %7.3f %+8.3f %9s %8.0f %8.1f %7.1f %6s %7.3f %s"
                      % (label, r["base"], r["trades"], r["pf"], r["exp"],
                         ("%+.3f" % en) if en is not None else "NOTMEAS",
                         r["swap"], r.get("spread", float("nan")),
                         r.get("mean_hold", float("nan")) / 60.0,
                         r["pct"], r["p50"],
                         "PASS" if ok else "fail: " + ", ".join(bad)))

    # --- the gate, both legs, counted by hand at 40
    print()
    print("=" * 100)
    print("GATE ON BOTH LEGS (expectancy_net, 40 trades by hand)")
    for arm in "ABC":
        if (arm, "is") not in data or (arm, "oos") not in data:
            continue
        for label in ROWS:
            ri = data[(arm, "is")][1].get(label)
            ro = data[(arm, "oos")][1].get(label)
            if not ri or not ro:
                continue
            oi, bi = gate(ri)
            oo, bo = gate(ro)
            mark = "**BOTH**" if (oi and oo) else ("IS only" if oi else ("OOS only" if oo else "neither"))
            print(" arm %s %-9s %-8s  IS[%s]  OOS[%s]"
                  % (arm, label, mark, "pass" if oi else ",".join(bi), "pass" if oo else ",".join(bo)))

    # --- financing: is the flat variant actually flat?
    print()
    print("=" * 100)
    print("F2 — FINANCING, arm B only (swap$/spread$, and exp_net - exp)")
    print("%-9s %5s %10s %10s %12s %14s" % ("row", "leg", "swap$", "spread$", "swap$/spread$", "exp_net-exp"))
    for leg in legs:
        if ("B", leg) not in data:
            continue
        _, rows = data[("B", leg)]
        for label in ROWS:
            r = rows.get(label)
            if not r:
                continue
            sp = r.get("spread", float("nan"))
            ratio = abs(r["swap"]) / sp if sp else float("nan")
            en = r.get("exp_net")
            print(" %-9s %5s %10.0f %10.1f %12.3f %14s"
                  % (label, leg, r["swap"], sp, ratio,
                     ("%+.3f" % (en - r["exp"])) if en is not None else "NOTMEAS"))

    # --- the trade-off: round trips and spread paid, flat vs parent
    print()
    print("=" * 100)
    print("THE TRADE-OFF, arm A: round trips and spread, flat row vs the baseline it varies")
    print("%-9s %5s %7s %9s %10s %9s %9s %10s %10s"
          % ("row", "leg", "trades", "x trades", "spread$", "x spread", "$/trade", "R/trip*", "carry saved"))
    for leg in legs:
        if ("A", leg) not in data or ("B", leg) not in data:
            continue
        _, a = data[("A", leg)]
        _, b = data[("B", leg)]
        for label in ROWS:
            r = a.get(label)
            if not r:
                continue
            parent = PARENT.get(label)
            pr = a.get(parent) if parent else None
            sp = r.get("spread", float("nan"))
            # risk_usd recovered from the arm-B pair: swap$ / (trades * delta)
            rb = b.get(label)
            risk = float("nan")
            if rb and rb.get("total_r") is not None and rb.get("total_r_net") is not None:
                dr = abs(rb["total_r_net"] - rb["total_r"])
                if dr:
                    risk = abs(rb["swap"]) / dr
            # carry the parent pays, in R per trade
            carry = float("nan")
            if parent and b.get(parent) and b[parent].get("exp_net") is not None:
                carry = b[parent]["exp_net"] - b[parent]["exp"]
            print(" %-9s %5s %7d %9s %10.1f %9s %9.3f %10s %10s"
                  % (label, leg, r["trades"],
                     ("%.1fx" % (r["trades"] / pr["trades"])) if pr else "-",
                     sp,
                     ("%.1fx" % (sp / pr["spread"])) if pr and pr.get("spread") else "-",
                     sp / r["trades"],
                     ("%.2f%%" % (100.0 * sp / r["trades"] / risk)) if risk == risk else "-",
                     ("%+.3fR" % carry) if carry == carry else "-"))
    print(" * R/trip = (spread$ / trades) / risk_usd, risk_usd recovered as")
    print("   |swap$| / |total_r_net - total_r| from the SAME row's arm-B pair (both printed to 2 dp,")
    print("   which is 100x the precision of the per-trade delta). Approximate: lots are re-sized")
    print("   every trade, so this is a mean risk unit, not a constant one.")

    # --- exits
    print()
    print("=" * 100)
    print("EXIT MIX — does the mechanism's OWN rule fire? (brief 6a)")
    for arm in "ABC":
        for leg in legs:
            if (arm, leg) not in data:
                continue
            _, rows = data[(arm, leg)]
            for label in ROWS:
                r = rows.get(label)
                if not r:
                    continue
                c = exit_counts(r.get("exits_raw", ""))
                own = sum(v for k, v in c.items() if k.startswith("<own rule>"))
                tot = sum(c.values())
                wf = c.get("WEEKEND_FLAT", 0)
                nf = c.get("NEWS_FLAT", 0)
                oc = c.get("OPEN_LOSS_CAP", 0)
                per_k = (1000.0 * wf / tot) if tot else float("nan")
                print(" arm %s %-4s %-9s own %5d/%5d (%5.1f%%)  WEEKEND_FLAT %4d (%6.1f /1k)  NEWS_FLAT %4d  OPEN_LOSS_CAP %4d"
                      % (arm, leg, label, own, tot, 100.0 * own / tot if tot else float("nan"), wf, per_k, nf, oc))

    # --- reading limits
    print()
    print("=" * 100)
    print("READING LIMITS: count match / cost match / exposure ratio (percentile publishable?)")
    for arm in "ABC":
        for leg in legs:
            if (arm, leg) not in data:
                continue
            _, rows = data[(arm, leg)]
            for label in ROWS:
                r = rows.get(label)
                if not r:
                    continue
                pub = not (r.get("cost_band") or r.get("count_band"))
                print(" arm %s %-4s %-9s count %5s%s cost %5s%s expo %5s  long %5s vs null %5s  -> %s"
                      % (arm, leg, label,
                         r.get("count_match", "?"), "!" if r.get("count_band") else " ",
                         ("%.2f" % r["cost_match"]) if "cost_match" in r else "?",
                         "!" if r.get("cost_band") else " ",
                         ("%.2f" % r["exposure_ratio"]) if "exposure_ratio" in r else "?",
                         ("%.3f" % r["long_share"]) if "long_share" in r else "?",
                         ("%.3f" % r["null_long_share"]) if "null_long_share" in r else "?",
                         "percentile publishable" if pub else "PERCENTILE NOT PUBLISHED"))


if __name__ == "__main__":
    main()
