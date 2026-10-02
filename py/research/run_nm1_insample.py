"""In-sample run of the three registered mechanisms. Design window only.

Registration: docs/hypotheses/2026-10-02-new-method-1.md
Design window: XAUDUKA 15m 2010-06-01 -> 2021-12-31. Nothing at or after 2025-09-23 is
read from any file (asserted in load_bars).
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pandas as pd
from new_method_1 import (load_bars, sessionise, events, method_a, method_b_leg,
                          method_b_control, method_c, method_c_control_deep,
                          report, gate, stats, fmt, COST, CUT)

DATA = sys.argv[1] if len(sys.argv) > 1 else "."
LO = pd.Timestamp("2010-06-01", tz="UTC")
HI = pd.Timestamp("2022-01-01", tz="UTC")
ERAS = [(pd.Timestamp(a, tz="UTC"), pd.Timestamp(b, tz="UTC")) for a, b in
        [("2010-06-01", "2013-03-01"), ("2013-03-01", "2016-01-01"),
         ("2016-01-01", "2018-11-01"), ("2018-11-01", "2022-01-01")]]

print("=" * 100)
print("IN SAMPLE  XAUDUKA 15m  design window %s -> %s" % (LO.date(), HI.date()))
print("hold-out cut: no bar at or after %s is read, from any file" % CUT.date())
print("cost: %.2f price units per ounce round trip (config [markets.xauusd] spread)" % COST)
print("=" * 100)

df = load_bars(os.path.join(DATA, "data/bars/XAUDUKA-15m.parquet"))
df = df[(df["time"] >= LO) & (df["time"] < HI)].reset_index(drop=True)
df, first, atr, nsess = sessionise(df)
print("design window: %d bars, %d sessions, %s -> %s"
      % (len(df), nsess, df["time"].iloc[0], df["time"].iloc[-1]))
import numpy as np
print("ATR20d over the design window: mean %.2f price units, median %.2f"
      % (np.nanmean(atr), np.nanmedian(atr)))

ev_all = events(os.path.join(DATA, "data/news/events-extended.csv"))
ev_all = ev_all[(ev_all["time_utc"] >= LO) & (ev_all["time_utc"] < HI)].reset_index(drop=True)
print("USD impact-3 events in window: %d" % len(ev_all))
print(ev_all["name"].value_counts().to_string())
ev_fomc = ev_all[ev_all["name"].str.startswith("FOMC")].reset_index(drop=True)

# ---------------------------------------------------------------- A
print("\n" + "-" * 100)
print("METHOD A  slow-moving capital after a scheduled macro repricing")
print("  PRIMARY cell: release window 1 hour, hold 3 sessions, all four US impact-3 series")
print("-" * 100)
tr = method_a(df, first, atr, nsess, ev_all, rel_bars=4, hold_sess=3)
sA = report(tr, "A PRIMARY rel=1h hold=3sess all-US", ERAS)
okA = gate(sA, tr, ERAS, "A primary")

print("\n  robustness cells (declared in advance; may break the primary, never rescue it)")
for subset, ev in (("all-US", ev_all), ("FOMC", ev_fomc)):
    for rb, rl in ((4, "1h"), (16, "4h")):
        for h in (1, 3, 5):
            t2 = method_a(df, first, atr, nsess, ev, rel_bars=rb, hold_sess=h)
            if len(t2) == 0:
                print("    A rel=%s hold=%dsess %-6s  n=0 null" % (rl, h, subset)); continue
            s2 = stats(t2["raw"], t2["atr"], "    A rel=%s hold=%dsess %-6s" % (rl, h, subset))
            print(fmt(s2))

print("\n  mechanism diagnostic: per series, primary parameters (FOMC must not be weakest)")
for name in sorted(ev_all["name"].unique()):
    e = ev_all[ev_all["name"] == name].reset_index(drop=True)
    t2 = method_a(df, first, atr, nsess, e, rel_bars=4, hold_sess=3)
    if len(t2) < 2:
        print("    %-42s n=%d null" % (name, len(t2))); continue
    print(fmt(stats(t2["raw"], t2["atr"], "    " + name[:40])))

# ---------------------------------------------------------------- B
print("\n" + "-" * 100)
print("METHOD B  the premium for carrying risk that cannot be hedged across a known instant")
print("  PRIMARY cell: pre leg 4h short, post leg 4h long starting 1h after the event")
print("-" * 100)
hours_pre = sorted(set((ev_all["time_utc"] - pd.Timedelta(hours=4)).dt.hour))
hours_post = sorted(set((ev_all["time_utc"] + pd.Timedelta(hours=1)).dt.hour))
print("  pre-leg start hours (UTC): %s ; post-leg start hours (UTC): %s" % (hours_pre, hours_post))

pre = method_b_leg(df, first, atr, nsess, ev_all, -4, 0, -1)
sPre = report(pre, "B pre leg  4h SHORT into the event", ERAS)
cpre = method_b_control(df, first, atr, nsess, ev_all, 4, -1, hours_pre)
sCpre = stats(cpre["raw"], cpre["atr"], "  CONTROL pre: same length, same hours, no-event days")
print(fmt(sCpre))
if sPre and sCpre["mean_atr"] is not None:
    print("      EXCESS pre leg over the hour-matched control: %+.4f ATR20d (%+.3f px)"
          % (sPre["mean_atr"] - sCpre["mean_atr"], sPre["mean_px"] - sCpre["mean_px"]))

post = method_b_leg(df, first, atr, nsess, ev_all, 1, 5, 1)
sPost = report(post, "B post leg 4h LONG from 1h after the event", ERAS)
cpost = method_b_control(df, first, atr, nsess, ev_all, 4, 1, hours_post)
sCpost = stats(cpost["raw"], cpost["atr"], "  CONTROL post: same length, same hours, no-event days")
print(fmt(sCpost))
if sPost and sCpost["mean_atr"] is not None:
    print("      EXCESS post leg over the hour-matched control: %+.4f ATR20d (%+.3f px)"
          % (sPost["mean_atr"] - sCpost["mean_atr"], sPost["mean_px"] - sCpost["mean_px"]))

print("\n  GATE B pre leg (on the raw leg; the excess is printed above)")
okBpre = gate(sPre, pre, ERAS, "B pre")
print("  GATE B post leg")
okBpost = gate(sPost, post, ERAS, "B post")

comb = pd.concat([pre, post], ignore_index=True)
print("\n  drift-neutral combination (short pre + long post, equal lengths, net side 0)")
sComb = report(comb, "B combined (drift-neutral by construction)", ERAS)

print("\n  robustness cells")
for ph in (2, 4, 8):
    t2 = method_b_leg(df, first, atr, nsess, ev_all, -ph, 0, -1)
    print(fmt(stats(t2["raw"], t2["atr"], "    B pre %dh SHORT" % ph)))
for lab, lo, hi in (("4h", 1, 5), ("1sess", 1, 25)):
    t2 = method_b_leg(df, first, atr, nsess, ev_all, lo, hi, 1)
    print(fmt(stats(t2["raw"], t2["atr"], "    B post %s LONG" % lab)))

# ---------------------------------------------------------------- C
print("\n" + "-" * 100)
print("METHOD C  a reopen priced into the thinnest book of the week")
print("  PRIMARY cell: |gap| >= 0.5 ATR20d, fade it, hold 1 session")
print("-" * 100)
tc = method_c(df, first, atr, nsess, thresh=0.5, hold_sess=1)
sC = report(tc, "C PRIMARY gap>=0.5 ATR20d hold=1sess", ERAS)
okC = gate(sC, tc, ERAS, "C primary")

print("\n  robustness cells")
for th in (0.25, 0.5, 1.0):
    for lab, kw in (("4h", dict(hold_bars=16)), ("1sess", dict(hold_sess=1))):
        t2 = method_c(df, first, atr, nsess, thresh=th, **kw)
        if len(t2) < 2:
            print("    C gap>=%.2f hold=%-5s n=%d null" % (th, lab, len(t2))); continue
        print(fmt(stats(t2["raw"], t2["atr"], "    C gap>=%.2f hold=%-5s" % (th, lab))))

print("\n  MECHANISM FALSIFIER for C: the same fade on a same-size move in the DEEP")
print("  London hours (09-11 UTC). If this is as large, the overshoot is not liquidity.")
cd = method_c_control_deep(df, first, atr, nsess, thresh=0.5, hold_sess=1)
print(fmt(stats(cd["raw"], cd["atr"], "    C deep-hour control gap>=0.5 hold=1sess")))

print("\n" + "=" * 100)
print("IN-SAMPLE VERDICT  A=%s  B(pre)=%s  B(post)=%s  C=%s"
      % tuple("clears" if x else "FAILS" for x in (okA, okBpre, okBpost, okC)))
print("=" * 100)
