"""Per-row stop size and cost/R, read off the engine's own cost-matched-null
line in the positive-control runs, plus each row's spread PROVENANCE."""
import json, os, re
S = "C:/Users/Admin/AppData/Local/Temp/claude/E--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad"
R = json.load(open(f"{S}/rr/table34.json"))
MKT = {}   # (label, pubN) -> market, from the published argv
V = {(v["label"], v["trades"]): v for v in json.load(open(f"{S}/rr/bars3.json")) if v["indec"]}
for k, v in V.items():
    m = re.search(r"--market=(\S+)", v["cmds"][0]); MKT[k] = m.group(1) if m else "?"

# the engine's own stop/cost line, attributed to the row above it
COST = {}
for fn in os.listdir(f"{S}/rr/ctl/out"):
    t = open(f"{S}/rr/ctl/out/{fn}", encoding="utf-8", errors="replace").read()
    cur = None
    for ln in t.splitlines():
        tk = ln.split()
        if tk and "/" in tk[0] and len(tk) > 4 and re.fullmatch(r"\d+", tk[2]):
            cur = (tk[0], int(tk[2]))
        m = re.search(r"cost-matched null: control stop ([\d.]+) ATR = ([\d.]+) points, cost ([\d.]+)% of R \(([^)]*)\)", ln)
        if m and cur:
            COST[(fn[:-4],) + cur] = m.groups()
            COST.setdefault((fn[:-4], cur[0]), m.groups())

SPREAD = {   # market -> (charged, provenance measured 2026-10-11 by me)
 "xauusd":  ("0.28 /round trip", "MEASURED log data/spreads/XAUUSD_sc.csv n=8,967 over 49.8 h, 24/24 hours: p50 0.220 p95 0.260 p99=max 0.290; 2.80% of samples EXCEED the 0.28 charged, 100% of those in 20-22 UTC (hour 21 = 17:00 NY, median 0.290)"),
 "xauduka": ("0.28 /round trip", "ASSUMED BY TRANSFER - no XAUDUKA spread log exists on disk; the 0.28 is the Vantage XAUUSD.sc terminal read applied to a Dukascopy BID series"),
 "btc":     ("5.00 /round trip", "ASSUMED - config/default.toml says so; no Binance BTCUSDT log on disk. The only measured BTC log is Vantage BTCUSD_sc.csv, p50 17.03 (n=6,650), which config labels 'MEASURED: 3.4x the 5.00 assumed'"),
}
print(f"{'label':<22}{'pubN':>6} {'mkt':<8}{'stop(ATR)':>10}{'stop(pts)':>11}{'cost/R':>8}  stop kind")
from collections import Counter
cnt = Counter()
for r in R:
    k = (r["label"], r["pubN"]); mk = MKT.get(k, "?"); cnt[mk] += 1
    c = (COST.get((r["ctl"],) + k) or COST.get((r["ctl"], r["label"]))) if r["ctl"] else None
    r["market"] = mk
    r["stop_atr"], r["stop_pts"], r["costR"], r["stop_kind"] = c if c else (None,)*4
    print(f"{r['label']:<22}{r['pubN']:>6} {mk:<8}"
          f"{(c[0] if c else '-'):>10}{(c[1] if c else '-'):>11}{((c[2]+'%') if c else '-'):>8}  {c[3] if c else '(no control run)'}")
json.dump(R, open(f"{S}/rr/table34c.json","w"), indent=1)
print()
print("rows per market:", dict(cnt))
print()
for m, (ch, pv) in SPREAD.items():
    print(f"{m:<8} charged {ch:<18} {pv}")
