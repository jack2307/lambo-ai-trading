# agent/flat-native: pull the declared columns out of the 8 receipts and apply
# the registered ruler (section 3 of docs/decisions/2026-10-09-flat-native.md).
import re, os, json, sys

R = r'E:/rust/fd-flat-native/receipts/flat-native'
C = 0.05          # commission per lot per side in the calibration arm
SPREAD = 0.28     # price points per round trip, flat on this feed
CS = 1.0          # contract size, xauduka

ROWS = ["px-1s", "gap-fade", "gap/s05", "gap/s4", "gap/2atr", "gap/daily",
        "gap/daily-a05", "gap/daily-a2", "gap/daily-s4", "intraday-mom",
        "im/f05", "im/f2"]


def parse(path):
    out = {}
    cur = None
    for line in open(path, encoding="utf-8", errors="replace"):
        m = re.match(r"^(\S+)\s+(session-hold|gap-fade|intraday-momentum)\s+(\d+)\s+"
                     r"([\d.]+|nan|inf)\s+(-?[\d.]+)\s+", line)
        if m and m.group(1) in ROWS:
            cur = m.group(1)
            out[cur] = {"trades": int(m.group(3)), "pf_usd": m.group(4),
                        "expect": float(m.group(5)), "verdict": line.split("  ")[-1].strip()}
            continue
        if cur is None:
            continue
        s = line.strip()
        m = re.search(r"max drawdown ([\d.]+) USD = ([\d.]+)% of the book's peak", s)
        if m:
            out[cur]["dd_usd"] = float(m.group(1)); out[cur]["dd_pct"] = float(m.group(2))
        m = re.search(r"avg_mae (-?[\d.]+) R", s)
        if m:
            out[cur]["avg_mae"] = float(m.group(1))
        m = re.search(r"net (-?[\d.]+) USD over (\d+) trades", s)
        if m:
            out[cur]["net_usd"] = float(m.group(1))
        m = re.search(r"max drawdown: NOT MEASURED", s)
        if m:
            out[cur]["dd_usd"] = None
        m = re.search(r"Lbar ([\d.]+) R .*? PF_r ([\d.]+) \(R\) vs PF_usd ([\d.]+) \(USD, gap ([+-][\d.]+)\)", s)
        if m:
            out[cur]["lbar"] = float(m.group(1)); out[cur]["pf_r"] = float(m.group(2))
            out[cur]["pf_usd_f"] = float(m.group(3)); out[cur]["pf_gap"] = float(m.group(4))
        m = re.search(r"count match ([\d.]+)", s)
        if m:
            out[cur]["count_match"] = float(m.group(1))
        m = re.search(r"long share: method ([\d.]+)", s)
        if m:
            out[cur]["long_share"] = float(m.group(1))
        m = re.search(r"spread paid: method ([\d.]+) USD", s)
        if m:
            out[cur]["spread_usd"] = float(m.group(1))
        m = re.search(r"signed share of time method ([+-][\d.]+).*?time in market method ([\d.]+) min", s)
        if m:
            out[cur]["signed"] = float(m.group(1)); out[cur]["tim"] = float(m.group(2))
        m = re.search(r"the method's own realised stop: median ([\d.]+) ATR = ([\d.]+) points over (\d+)", s)
        if m:
            out[cur]["stop_atr"] = float(m.group(1)); out[cur]["stop_pts"] = float(m.group(2))
        m = re.search(r"cost ([\d.]+)% of R", s)
        if m:
            out[cur]["cost_pct_median"] = float(m.group(1))
        m = re.search(r"expectancy_net ([+-][\d.]+) R and total_r_net ([+-][\d.]+) R .*?vs expectancy ([+-][\d.]+) R and total_r ([+-][\d.]+) R", s)
        if m:
            out[cur]["e_net"] = float(m.group(1)); out[cur]["tr_net"] = float(m.group(2))
            out[cur]["e"] = float(m.group(3)); out[cur]["tr"] = float(m.group(4))
        m = re.match(r"^exits: (.*); mean hold ([\d.]+) min", s)
        if m:
            out[cur]["exits"] = m.group(1); out[cur]["hold"] = float(m.group(2))
        m = re.search(r"wrong_side_stop[^\d]*(\d+)", s)
        if m:
            out[cur]["wrong_side_stop"] = int(m.group(1))
        m = re.search(r"guards: refused ([^;]+); closed ([^;]+); sized down (\d+)", s)
        if m:
            out[cur]["g_refused"] = m.group(1).strip()
            out[cur]["g_closed"] = m.group(2).strip()
            out[cur]["sized_down"] = int(m.group(3))
    return out


R_ = {}
for f in sorted(os.listdir(R)):
    if f.endswith(".txt"):
        R_[f[:-4]] = parse(os.path.join(R, f))

# the ruler
def ruler(arm, comm, win):
    main = R_[f"{arm}-duka-{win}"]
    cal = R_[f"{comm}-duka-{win}"]
    S = main["px-1s"]["hold"]
    px_sess = main["px-1s"]["tim"] / S
    res = {}
    for row in ROWS:
        a, k = main.get(row), cal.get(row)
        if not a or not k:
            continue
        n = a["trades"]
        d = {"n": n, "pf_usd": a.get("pf_usd_f"), "pf_r": a.get("pf_r"),
             "lbar": a.get("lbar"), "E": a["expect"], "dd_usd": a.get("dd_usd"),
             "dd_pct": a.get("dd_pct"), "avg_mae": a.get("avg_mae"),
             "exits": a.get("exits"), "hold": a.get("hold"),
             "stop_pts_median": a.get("stop_pts"),
             "cost_pct_median": a.get("cost_pct_median"),
             "long_share": a.get("long_share"), "signed": a.get("signed"),
             "count_match": a.get("count_match"), "verdict": a.get("verdict"),
             "net_usd": a.get("net_usd"), "sized_down": a.get("sized_down"),
             "g_closed": a.get("g_closed"), "g_refused": a.get("g_refused")}
        # parity: the calibration arm must not move the trade set
        d["cal_n"] = k["trades"]
        d["cal_parity"] = (k["trades"] == n)
        if "tr" in k and "tr_net" in k:
            # (B1) mean(1/risk) = (total_r - total_r_net)/(2 c n)
            mean_inv_risk = (k["tr"] - k["tr_net"]) / (2.0 * C * n)
            # `expectancy` prints at 3 dp, which cannot resolve a gross of
            # ~0.001 R: gross = E + spreadR is a difference of two nearly
            # equal numbers. `total_r` prints at 2 dp over n trades, so
            # E = total_r / n is ~n/5 times more precise. The two must agree
            # to the printed digit of `expectancy`, which is the parity check.
            d["tr"] = k["tr"]
            d["E_precise"] = k["tr"] / n
            d["tr_parity"] = abs(d["E_precise"] - a["expect"]) <= 0.00051
            d["mean_inv_risk"] = mean_inv_risk
            d["risk_harm_pts"] = 1.0 / mean_inv_risk if mean_inv_risk else None
            d["spreadR"] = SPREAD * mean_inv_risk
            d["gross_trip"] = d["E_precise"] + d["spreadR"]
            sess = a["tim"] / S if S else None
            d["sess"] = sess
            d["trips_sess"] = n / sess if sess else None
            d["gross_sess"] = d["gross_trip"] * n / sess if sess else None
            # spread/gross is only a question where gross is positive; on a
            # negative gross the spread is not "a share of" anything.
            d["spread_share"] = ((d["spreadR"] * n / sess) / d["gross_sess"]
                                 if d["gross_sess"] and d["gross_sess"] > 0 else None)
            d["spreads_of_gross"] = d["gross_sess"] / d["spreadR"] if d["spreadR"] else None
            # net of spread per exposed session: `expectancy` already has the
            # spread in it, so this is just E x trips.
            d["net_sess"] = d["E_precise"] * d["trips_sess"] if sess else None
            d["lots_mean"] = a["spread_usd"] / (n * SPREAD * CS) if a.get("spread_usd") else None
        res[row] = d
    # drift anchor from px-1s in the SAME run
    px = res.get("px-1s", {})
    for row, d in res.items():
        if px.get("gross_sess") is not None and d.get("spreadR") and px.get("spreadR"):
            d["drift_attr"] = d["signed"] * px["gross_sess"] * (d["spreadR"] / px["spreadR"])
            d["residual"] = d["gross_sess"] - d["drift_attr"]
    res["_S"] = S
    res["_px_sess"] = px_sess
    return res


OUT = {}
for arm, comm in (("A", "Acomm"), ("C", "Ccomm")):
    for win in ("is", "oos"):
        OUT[f"{arm}-{win}"] = ruler(arm, comm, win)

json.dump(OUT, open(os.path.join(R, "derived.json"), "w"), indent=1, default=str)

def f(v, p=4):
    return "—" if v is None else (f"{v:+.{p}f}" if isinstance(v, float) else str(v))

for key in ("A-is", "A-oos", "C-is", "C-oos"):
    r = OUT[key]
    print(f"\n######## {key}   S = {r['_S']:.1f} min, px-1s sessions = {r['_px_sess']:.1f}")
    print(f"{'row':<14}{'n':>5} {'PF_r':>7} {'PF_usd':>7} {'Lbar':>7} {'E':>8} {'dd USD':>9} {'dd%':>7} "
          f"{'stopMed':>8} {'riskHar':>8} {'sprR%':>7} {'gr/trip':>9} {'trip/ses':>9} {'gr/sess':>9} {'spr/gr':>8} {'xSPR':>7} {'resid':>9} {'lots':>8}")
    for row in ROWS:
        d = r.get(row)
        if not d:
            continue
        print(f"{row:<14}{d['n']:>5} {f(d['pf_r'],3):>7} {f(d['pf_usd'],3):>7} {f(d['lbar'],3):>7} {f(d['E']):>8} "
              f"{'' if d['dd_usd'] is None else format(d['dd_usd'],'.2f'):>9} {'' if d['dd_pct'] is None else format(d['dd_pct'],'.1f'):>7} "
              f"{f(d.get('stop_pts_median'),2):>8} {f(d.get('risk_harm_pts'),3):>8} "
              f"{'' if d.get('spreadR') is None else format(100*d['spreadR'],'.2f'):>7} {f(d.get('gross_trip')):>9} "
              f"{f(d.get('trips_sess'),2):>9} {f(d.get('gross_sess')):>9} "
              f"{'' if d.get('spread_share') is None else format(100*d['spread_share'],'.0f')+'%':>8} "
              f"{f(d.get('spreads_of_gross'),2):>7} {f(d.get('residual')):>9} {f(d.get('lots_mean'),1):>8}")
    print("  parity cal n/total_r:", {k: (v['cal_parity'], v.get('tr_parity')) for k, v in r.items() if isinstance(v, dict)})
