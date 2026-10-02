"""The stricter test registered in docs/hypotheses/2026-10-02-new-method-2.md
(amendment, 2026-10-02 afternoon). Read-only.

Gold long 16:30 -> 18:30 New York, Monday to Thursday, on three windows, measured
in dollars an ounce and in basis points of the hold's own entry price.
"""
import math
import numpy as np
import pandas as pd
import pyarrow.parquet as pq

ROOT = "E:/rust/flowdesk"
SPREAD = 0.28  # price units a round turn, measured over 2,315 samples


def load(sym):
    t = pq.read_table(ROOT + "/data/bars/%s-15m.parquet" % sym)
    df = t.to_pandas().sort_values("time").reset_index(drop=True)
    ny = df["time"].dt.tz_convert("America/New_York")
    df["m"] = ny.dt.hour * 60 + ny.dt.minute
    df["ny"] = ny
    df["date"] = pd.to_datetime(ny.dt.date)
    df["wd"] = ny.dt.weekday
    return df


def holds(df, lo, hi):
    out = []
    for d, sub in df.groupby("date"):
        e = sub[sub["m"] >= lo]
        if e.empty:
            continue
        ei = e.index[0]
        x = sub[(sub["m"] >= hi) & (sub.index > ei)]
        if x.empty:
            continue
        xi = x.index[0]
        hours = (df["ny"].iloc[xi] - df["ny"].iloc[ei]).total_seconds() / 3600.0
        out.append((d, df["open"].iloc[ei], df["open"].iloc[xi], hours))
    return pd.DataFrame(out, columns=["date", "entry", "exit", "hours"])


WINDOWS = [
    ("xauduka 2010-06-01 -> 2018-06-15", "XAUDUKA", "2010-06-01", "2018-06-15"),
    ("xauduka 2018-06-16 -> 2025-04-10", "XAUDUKA", "2018-06-16", "2025-04-10"),
    ("xauusd  2025-04-11 -> 2026-09-11", "XAUUSD", "2025-04-11", "2026-09-11"),
]

res = []
print("=" * 104)
print("STRICTER TEST - gold long 16:30 -> 18:30 New York, Mon-Thu, 100%% long by construction")
print("drift control: each hold's raw move minus the window's own unconditional drift x the hold's hours")
print("cost: %.2f price units a round turn, converted to basis points of the hold's own entry price" % SPREAD)
print("=" * 104)
for lab, sym, a, b in WINDOWS:
    df = load(sym)
    df = df[(df["date"] >= a) & (df["date"] < b)].reset_index(drop=True)
    per_h = (df["open"].iloc[-1] - df["open"].iloc[0]) / (
        (df["ny"].iloc[-1] - df["ny"].iloc[0]).total_seconds() / 3600.0)
    h = holds(df[df["wd"] <= 3].reset_index(drop=True), 16 * 60 + 30, 18 * 60 + 15)
    h["raw_usd"] = h["exit"] - h["entry"]
    h["ex_usd"] = h["raw_usd"] - per_h * h["hours"]
    h["raw_bp"] = h["raw_usd"] / h["entry"] * 1e4
    h["ex_bp"] = h["ex_usd"] / h["entry"] * 1e4
    h["cost_bp"] = SPREAD / h["entry"] * 1e4
    h["net_bp"] = h["ex_bp"] - h["cost_bp"]
    h["net_usd"] = h["ex_usd"] - SPREAD
    n = len(h)

    def ms(c):
        x = h[c].values
        return x.mean(), x.std(ddof=1) / math.sqrt(n)

    m_eu, s_eu = ms("ex_usd")
    m_eb, s_eb = ms("ex_bp")
    m_nb, s_nb = ms("net_bp")
    m_nu, s_nu = ms("net_usd")
    print()
    print("%s   %d holds, mean hold %.2f h, mean entry price %.2f $/oz" % (lab, n, h["hours"].mean(), h["entry"].mean()))
    print("   unconditional drift of the window            %+.5f $/oz per hour" % per_h)
    print("   raw move                                     %+8.4f $/oz   %+8.4f bp" % (h["raw_usd"].mean(), h["raw_bp"].mean()))
    print("   DETRENDED excess                             %+8.4f $/oz (SE %.4f, t %+.2f)   %+8.4f bp (SE %.4f, t %+.2f)"
          % (m_eu, s_eu, m_eu / s_eu, m_eb, s_eb, m_eb / s_eb))
    print("   cost                                         %+8.4f $/oz   %+8.4f bp" % (SPREAD, h["cost_bp"].mean()))
    print("   NET of cost  (F2)                            %+8.4f $/oz (t %+.2f)        %+8.4f bp (t %+.2f)"
          % (m_nu, m_nu / s_nu, m_nb, m_nb / s_nb))
    print("   sides: long %d, short 0.  SIDES FLIPPED detrended excess %+8.4f bp (t %+.2f)  (F3)"
          % (n, -m_eb, -m_eb / s_eb))
    print("   F2 verdict on this window: %s" % ("PASS (net bp > 0 and t >= 2.0)" if (m_nb > 0 and m_nb / s_nb >= 2.0) else "FAIL"))
    res.append((lab, n, m_eu, s_eu, m_eb, s_eb, m_nb, s_nb))


def homog(vals, ses, name, unit):
    w = np.array([1.0 / s ** 2 for s in ses])
    v = np.array(vals)
    mu = (w * v).sum() / w.sum()
    chi2 = (w * (v - mu) ** 2).sum()
    print()
    print("  %s, inverse-variance weighted mean %+.4f %s; chi-square across 3 windows, 2 d.f. = %.3f  (5.99 is the 5%% point)"
          % (name, mu, unit, chi2))
    for (lab, n, *_), x, s in zip(res, vals, ses):
        print("     %-34s %+8.4f %s  SE %.4f  -> %+5.2f SE from the weighted mean %s"
              % (lab, x, unit, s, (x - mu) / s, "INSIDE 2 SE" if abs((x - mu) / s) <= 2 else "OUTSIDE 2 SE"))
    return chi2


print()
print("=" * 104)
print("F1 - is the effect fixed in dollars an ounce, or fixed in basis points of price?")
c_usd = homog([r[2] for r in res], [r[3] for r in res], "detrended excess in dollars an ounce", "$/oz")
c_bp = homog([r[4] for r in res], [r[5] for r in res], "detrended excess in basis points", "bp")
print()
print("  dollars-an-ounce chi-square %.3f ; basis-point chi-square %.3f" % (c_usd, c_bp))
if c_bp < 5.99 <= c_usd:
    print("  F1 CONFIRMED: the basis-point effect is one number across all three windows and the dollar one is not.")
elif c_bp >= 5.99:
    print("  F1 FAILS: the basis-point effect is itself heterogeneous across the windows.")
else:
    print("  F1 UNDECIDED: neither unit is rejected at the 5%% point.")
