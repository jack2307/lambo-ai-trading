"""Pull the per-row figures out of the bar-interval receipts.

Every number is lifted verbatim from a receipt line; nothing is recomputed.
0 trades prints as NaN PF / `null` percentile and is carried as null, never 0.
"""
import glob, io, os, re, statistics as st

NUM = r"(?:NaN|-?inf|[-+]?[\d.]+)"
ROW = re.compile(
    r"^(?P<label>\S+)\s+(?P<base>\S+)\s+(?P<trades>\d+)\s+(?P<pf>%s)\s+"
    r"(?P<exp>%s)\s+(?P<p50>%s)\s+(?P<p95>%s)\s+(?P<swap>%s)\s+"
    r"(?P<pct>\d+%%|null)\s+(?P<verdict>.*)$" % (NUM, NUM, NUM, NUM, NUM))
COST = re.compile(r"cost-matched null: control stop (\S+) ATR = (\S+) points, cost (\S+)% of R")
EXITS = re.compile(r"^\s+exits: (?P<mix>.*?); mean hold (\S+) min")

def f(x):
    try:
        v = float(x)
    except ValueError:
        return None
    return None if v != v else v

def parse(path):
    rows, cur = [], None
    for line in io.open(path, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        if line.startswith("hypothesis   base") or line.startswith("max hold:"):
            continue
        m = ROW.match(line) if not line.startswith(" ") else None
        if m:
            cur = dict(m.groupdict())
            cur["trades"] = int(cur["trades"])
            for k in ("pf", "exp", "p50", "p95"):
                cur[k] = f(cur[k])
            cur.update(cost_r=None, stop_pts=None, exits=None, hold=None)
            rows.append(cur)
            continue
        if cur is None:
            continue
        c = COST.search(line)
        if c:
            cur["stop_atr"], cur["stop_pts"], cur["cost_r"] = f(c.group(1)), f(c.group(2)), f(c.group(3))
        e = EXITS.match(line)
        if e:
            cur["exits"], cur["hold"] = e.group("mix"), f(e.group(2))
    return rows

def gate(r):
    if r["pf"] is None or r["exp"] is None:
        return "null"
    return "PASS" if (r["pf"] >= 1.200 and r["exp"] >= 0.050) else "fail"

def to_share(rows):
    to = tot = 0
    for r in rows:
        if not r["exits"] or r["exits"].startswith("no exits"):
            continue
        for part in r["exits"].split(", "):
            name, n = part.rsplit(" ", 1)
            tot += int(n)
            if name.strip() == "TIMEOUT":
                to += int(n)
    return (100.0 * to / tot) if tot else None

files = sorted(glob.glob("E:/rust/fd-a3/receipts/A*-window*.txt"))
allrows = {}
print("THE TABLE — one line per run (13 rows each)")
print("%-18s %5s %8s %8s %11s %9s %9s %7s %7s" % (
    "run", "rows", "med_trd", "max_trd", "med_cost%R", "min_cost%R", "med_PF", "gatePASS", "TIMEOUT%"))
for path in files:
    rows = parse(path)
    key = os.path.basename(path)[:-4]
    allrows[key] = rows
    trd = [r["trades"] for r in rows]
    cost = [r["cost_r"] for r in rows if r["cost_r"] is not None]
    pf = [r["pf"] for r in rows if r["pf"] is not None]
    ts = to_share(rows)
    print("%-18s %5d %8.1f %8d %11.2f %9.2f %9.3f %7d %7s" % (
        key, len(rows), st.median(trd), max(trd),
        st.median(cost), min(cost), st.median(pf),
        sum(1 for r in rows if gate(r) == "PASS"),
        ("%.0f%%" % ts) if ts is not None else "null"))

print()
print("BOTH WINDOWS — a cell is (arm, interval, label/base); it counts only if it PASSES the gate in A and in B")
cells = {}
for key, rows in allrows.items():
    arm, iv, win = key.split("-")
    for r in rows:
        cells.setdefault((arm, iv, r["label"] + "/" + r["base"]), {})[win[-1]] = r
both = []
for (arm, iv, name), byw in sorted(cells.items()):
    a, b = byw.get("A"), byw.get("B")
    if a is None or b is None:
        continue
    if gate(a) == "PASS" and gate(b) == "PASS":
        both.append((arm, iv, name, a, b))
        print("  %s %s %-32s A: %d trd PF %.3f exp %+.3f (nullp50 %.3f) | B: %d trd PF %.3f exp %+.3f (nullp50 %.3f)" % (
            arm, iv, name, a["trades"], a["pf"], a["exp"], a["p50"], b["trades"], b["pf"], b["exp"], b["p50"]))
if not both:
    print("  none")
print("  cells passing both windows:", len(both))
print("  of those with >=40 trades in BOTH windows:",
      sum(1 for (_, _, _, a, b) in both if a["trades"] >= 40 and b["trades"] >= 40))
print("  of those with >=30 trades (the engine's own floor) in BOTH windows:",
      sum(1 for (_, _, _, a, b) in both if a["trades"] >= 30 and b["trades"] >= 30))

print()
print("ENGINE'S OWN SURVIVOR LINE per receipt (gate AND outside its null AND its trade floor)")
for path in files:
    txt = io.open(path, encoding="utf-8", errors="replace").read()
    last = [l for l in txt.splitlines() if l.startswith("Survivors:") or l.startswith("Nothing survived")]
    print("  %-18s %s" % (os.path.basename(path)[:-4], last[0] if last else "???"))

print()
print("TRADE DENSITY — total trades across the 13 rows, and rows with >=40 / >=30 trades")
for key in sorted(allrows):
    rows = allrows[key]
    print("  %-18s total %6d   rows>=40 %2d/13   rows>=30 %2d/13   rows==0 %2d/13" % (
        key, sum(r["trades"] for r in rows),
        sum(1 for r in rows if r["trades"] >= 40),
        sum(1 for r in rows if r["trades"] >= 30),
        sum(1 for r in rows if r["trades"] == 0)))

print()
print("cells parsed:", sum(len(v) for v in allrows.values()))

print()
print("SAME RULE ACROSS INTERVALS, Arm 1 (standing ceiling) — cost falls, the verdict does not travel")
flat = {}
for key, rs in allrows.items():
    arm, iv, win = key.split("-")
    for r in rs:
        flat[(arm, iv, win[-1], r["label"] + "/" + r["base"])] = r
names = ["intraday/donchian-breakout", "intraday/bb-fade", "intraday/rsi-reversion",
         "ny-morning/donchian-breakout", "intraday/ema-cross", "asia/bb-fade"]
print("%-30s %-3s %-4s %7s %9s %8s %8s %9s %8s %5s" % (
    "rule", "win", "iv", "trades", "stop_pts", "cost%R", "PF", "expect", "nullp50", "gate"))
for name in names:
    for win in "AB":
        for iv in ("15m", "1h", "4h"):
            r = flat.get(("A1", iv, win, name))
            if r is None:
                print("%-30s %-3s %-4s  MISSING" % (name, win, iv))
                continue
            q = lambda v, fmt: (fmt % v) if v is not None else "null"
            print("%-30s %-3s %-4s %7d %9s %8s %8s %9s %8s %5s" % (
                name, win, iv, r["trades"], q(r["stop_pts"], "%.2f"), q(r["cost_r"], "%.2f"),
                q(r["pf"], "%.3f"), q(r["exp"], "%+.3f"), q(r["p50"], "%.3f"), gate(r)))
    print()
