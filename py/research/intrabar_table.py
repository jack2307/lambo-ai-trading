"""The single table of docs/decisions/2026-10-07-intrabar-resolution.md.

Reads the cells and prints the resolution map: per stop size, how often the 15m
verdict and the 1m sequence disagree, in which direction, and what it costs in
R. No gate, no selection: every cell measured is printed.
"""
import json
import sys

PATH = sys.argv[1] if len(sys.argv) > 1 else "../../receipts/intrabar-cells.json"
rows = json.load(open(PATH))

MECH = {
    0.378: "trend-pullback (realised)",
    0.714: "pdhl (realised)",
    0.85: "doji-reversal (realised)",
    1.00: "vwap-fade (declared+realised)",
    1.20: "the [trading] default",
    1.50: "ema-cross / bb-fade / macd-cross / stoch-reversal (declared)",
    2.00: "donchian-breakout / rsi2-pullback (declared)",
    2.681: "orb (realised)",
    3.310: "rsi-reversal-vol (realised)",
    4.210: "volume-thrust (realised)",
}


def band(pct):
    if pct <= 1.0:
        return "readable"
    if pct <= 5.0:
        return "caveat"
    return "NOT EVIDENCE"


def agg(hold, stop, rr=None):
    sel = [r for r in rows if r["hold"] == hold and r["stop_atr"] == stop
           and (rr is None or r["rr"] == rr)]
    t = sum(r["trades"] for r in sel)
    res = sum(r["resolved"] for r in sel)
    fl = sum(r["fake_loss"] for r in sel)
    fw = sum(r["fake_win"] for r in sel)
    un = sum(r["unresolved_1m"] for r in sel)
    both = sum(r["both"] for r in sel)
    s15 = sum(r["sumR_15"] for r in sel)
    s1m = sum(r["sumR_1m"] for r in sel)
    return dict(trades=t, resolved=res, both=both, fake_loss=fl, fake_win=fw,
                unresolved=un, sumR_15=s15, sumR_1m=s1m, cells=len(sel))


def stop_table(hold):
    print(f"\n{'='*118}")
    print(f"RESOLUTION MAP - XAUDUKA 15m adjudication vs XAUDUKA 1m sequence, "
          f"hold cap {hold}, both sides, rr 1.0/1.5/1.8 pooled")
    print(f"{'='*118}")
    print(f"{'stop':>6} {'trades':>7} {'resolv':>7} {'both':>7} {'both%':>6} "
          f"{'fakeLOSS':>8} {'fL%all':>7} {'fL%res':>7} {'fakeWIN':>7} "
          f"{'unres1m':>7} {'disagr%':>8} {'band':>12}  {'meanR 15m':>9} {'meanR 1m':>9} {'dR':>7}")
    out = []
    for stop in sorted({r["stop_atr"] for r in rows}):
        a = agg(hold, stop)
        d = 100.0 * (a["fake_loss"] + a["fake_win"]) / a["trades"]
        mr15 = a["sumR_15"] / a["trades"]
        mr1 = a["sumR_1m"] / a["trades"]
        print(f"{stop:>6.3f} {a['trades']:>7} {a['resolved']:>7} {a['both']:>7} "
              f"{100*a['both']/a['trades']:>5.1f}% {a['fake_loss']:>8} "
              f"{100*a['fake_loss']/a['trades']:>6.2f}% "
              f"{100*a['fake_loss']/max(a['resolved'],1):>6.2f}% {a['fake_win']:>7} "
              f"{a['unresolved']:>7} {d:>7.2f}% {band(d):>12}  "
              f"{mr15:>9.4f} {mr1:>9.4f} {mr1-mr15:>7.4f}")
        out.append((stop, d))
    return out


def threshold(per_hold):
    """Smallest stop whose disagreement stays <= 1.0% on BOTH hold arms, and
    the largest stop still above 5.0%."""
    stops = sorted({r["stop_atr"] for r in rows})
    readable = []
    for s in stops:
        if all(dict(v)[s] <= 1.0 for v in per_hold.values()):
            readable.append(s)
    not_evidence = [s for s in stops
                    if any(dict(v)[s] > 5.0 for v in per_hold.values())]
    return readable, not_evidence


def side_rr_split():
    print(f"\n{'='*118}")
    print("THE SAME TABLE SPLIT - does the answer move with side or with "
          "reward-to-risk? (4h cap, disagreement % of all trades)")
    print(f"{'='*118}")
    print(f"{'stop':>6} | {'LONG rr1.0':>10} {'LONG rr1.5':>10} {'LONG rr1.8':>10} "
          f"| {'SHORT rr1.0':>11} {'SHORT rr1.5':>11} {'SHORT rr1.8':>11}")
    for stop in sorted({r["stop_atr"] for r in rows}):
        cells = []
        for side in ("LONG", "SHORT"):
            for rr in (1.0, 1.5, 1.8):
                r = next(x for x in rows if x["hold"] == "4h" and x["stop_atr"] == stop
                         and x["rr"] == rr and x["side"] == side)
                cells.append(100.0 * (r["fake_loss"] + r["fake_win"]) / r["trades"])
        print(f"{stop:>6.3f} | {cells[0]:>9.2f}% {cells[1]:>9.2f}% {cells[2]:>9.2f}% "
              f"| {cells[3]:>10.2f}% {cells[4]:>10.2f}% {cells[5]:>10.2f}%")


def fake_win_check():
    fw = sum(r["fake_win"] for r in rows)
    tr = sum(r["trades"] for r in rows)
    worst = max(100.0 * r["fake_win"] / r["trades"] for r in rows)
    print(f"\nF2 (fake wins on a market-at-next-open entry): {fw} of {tr} trades "
          f"across all 192 cells; worst single cell {worst:.4f}% "
          f"(fires above 0.10%) -> {'FIRES' if worst > 0.10 else 'DOES NOT FIRE'}")


def mech_map():
    print(f"\n{'='*118}")
    print("EACH STOP SIZE, AND WHICH MECHANISM SITS ON IT (4h cap, the engine "
          "default every published row ran under)")
    print(f"{'='*118}")
    for stop in sorted({r["stop_atr"] for r in rows}):
        a = agg("4h", stop)
        d = 100.0 * (a["fake_loss"] + a["fake_win"]) / a["trades"]
        name = MECH.get(stop, "- (fill-in point, no mechanism)")
        print(f"{stop:>6.3f} ATR  disagreement {d:>6.2f}%  {band(d):>12}   {name}")


if __name__ == "__main__":
    per_hold = {h: stop_table(h) for h in ("4h", "48h")}
    side_rr_split()
    fake_win_check()
    readable, not_evidence = threshold(per_hold)
    print(f"\nreadable (<=1.0% on BOTH hold arms): stops {readable}")
    print(f"  -> THRESHOLD = {min(readable):.3f} ATR is the smallest stop whose "
          f"15m verdict is readable")
    print(f"NOT EVIDENCE (>5.0% on either arm): stops {not_evidence}")
    mech_map()
