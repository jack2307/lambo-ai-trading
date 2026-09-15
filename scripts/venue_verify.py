"""Re-read 2026-09-15-venue-residual the way its reviews demanded.

    python scripts/venue_verify.py --start=2025-06-01 --end=2026-06-01

Nothing here is a new cell or a new parameter. Every number below is the SAME
six registered cells, read again under the corrections three review roles asked
for in writing. A gate is never re-tuned to pass; these changes can only make it
harder, and two of them did.

What changed and who asked for it:

  * **The percentile is read on the leg that would be traded.** (adversary)
    The registration chose the ECN leg because a dealer's own quote cannot
    manufacture it by converging back — true when trading AGAINST the skew, the
    direction that was declared. The data went the other way, and under the
    mirror convergence flatters the ECN leg instead. The artefact shows it: the
    ECN-minus-Vantage wedge is +0.54 bp at k=1, k=6 AND k=12 alike, and absent
    from every control cell. A forecast grows with the horizon; a constant is
    paid in the first bar and is the residual closing.
  * **The null's centre is printed next to its percentile.** (adversary) A book
    that is 73% long in a year gold rose 37% earns a drift the permutation null
    is already centred on, so the percentile is honest but the headline bp is
    not, and a reader cannot tell how much of it is the bull market.
  * **A trade may not cross a hole.** (risk) `fires()` checked freshness on the
    signal bar only, so a fire in the last hour before the daily stop exited on
    the far side of it: 27 trades at k=12, eleven of them held over a WEEKEND,
    the longest 51 hours, carrying 16% of the net — while the project's own
    config sets `flat_before_weekend_hhmm = 1655` precisely to forbid that.
  * **The fill is delayed.** (adversary, risk) Entry at the first tick after the
    signal's own closing tick is not a fill, it is the signal's own input: on
    31% of Vantage fire bars `open[t+1]` is bit-identical to `close[t]`.
  * **Each cell is split in halves.** (adversary) A mechanism is present in both;
    a regime is present in one.
  * **The cost is charged by the hour the trade enters**, from 11.5M ticks of
    read-only history (`py/ingest/mt5_spread_probe.py --days=30`, receipt in
    this run directory): $0.26 in 20:00-23:59 UTC where the daily rollover sits,
    $0.20 elsewhere, and a flat $0.28 column for comparison with what the
    registration assumed. The evening quote is CHEAPER than the blend the study
    charged, which is the opposite of what the study feared.
  * **The Monte-Carlo error is computed at the percentile it is read at**, not
    at p=0.5: sqrt(p(1-p)/draws), which at 97.5% and 50,000 draws is 0.07 pp.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))
from venue_residual import build, permute, tf_minutes  # noqa: E402

#: Measured, not assumed. `mt5_spread_probe.py --symbol=XAUUSD.sc --days=30`,
#: 11,576,449 ticks 2026-08-16 -> 2026-09-15, p50 by server-clock window.
SPREAD_EVENING_USD = 0.26   # 20:00-23:59 UTC: the stop, the reopen, the hour after
SPREAD_DAY_USD = 0.20       # Asia, London, New York
SPREAD_FLAT_USD = 0.28      # what the registration charged everywhere


def trades(df: pd.DataFrame, threshold: float, k: int, delay: int, clean_exit: bool) -> dict:
    """Non-overlapping fires, in the MIRROR direction, with an optional delay.

    `clean_exit` requires every bar from the entry to the exit to be one
    interval after its predecessor, so a trade cannot span the daily stop or a
    weekend. That is a correctness fix, not a filter: the design never said a
    one-hour hold could last 51 hours.
    """
    z = df["z"].to_numpy()
    fresh = df["fresh"].to_numpy()
    rng = df["range_bp"].to_numpy()
    hour = pd.DatetimeIndex(df["time"]).hour.to_numpy()
    e_open, e_close = df["e_open"].to_numpy(), df["e_close"].to_numpy()
    v_open, v_close = df["v_open"].to_numpy(), df["v_close"].to_numpy()
    t = pd.DatetimeIndex(df["time"])
    n = len(df)
    out = {key: [] for key in ("sign", "ecn", "van", "vol", "hour", "when")}
    i, blocked = 0, -1
    while i < n:
        e = i + 1 + delay
        x = e + k - 1
        if x >= n:
            break
        if i > blocked and fresh[i] and np.isfinite(z[i]) and abs(z[i]) >= threshold:
            ok = True
            if clean_exit and not fresh[i + 1:x + 1].all():
                ok = False
            if ok:
                fe = 1e4 * (np.log(e_close[x]) - np.log(e_open[e]))
                fv = 1e4 * (np.log(v_close[x]) - np.log(v_open[e]))
                if np.isfinite(fe) and np.isfinite(fv) and np.isfinite(rng[i]):
                    out["sign"].append(np.sign(z[i]))  # the mirror: WITH the skew
                    out["ecn"].append(fe)
                    out["van"].append(fv)
                    out["vol"].append(rng[i])
                    out["hour"].append(hour[e])
                    out["when"].append(t[e])
                    blocked = x
        i += 1
    res = {key: np.array(val) for key, val in out.items() if key != "when"}
    res["when"] = pd.DatetimeIndex(out["when"])
    return res


def cost_bp(hours: np.ndarray, level: float, flat: bool) -> np.ndarray:
    usd = np.full(len(hours), SPREAD_FLAT_USD) if flat else np.where(
        (hours >= 20) & (hours <= 23), SPREAD_EVENING_USD, SPREAD_DAY_USD)
    return 1e4 * usd / level


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--start", default="2025-06-01")
    ap.add_argument("--end", default="2026-06-01")
    ap.add_argument("--tf", default="5m")
    ap.add_argument("--draws", type=int, default=50_000)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")
    minutes = tf_minutes(a.tf)
    per_day = int(23 * 60 / minutes)
    z_bars = 5 * per_day
    se = 100 * (0.975 * 0.025 / a.draws) ** 0.5

    print(f"venue-residual, re-read as the reviews required, gold {a.tf}, "
          f"{lo.date()} -> {hi.date()}")
    print(f"mirror direction throughout (WITH the skew); {a.draws:,} draws, "
          f"Monte-Carlo error at the 97.5 gate {se:.3f} pp")
    print(f"spread by entry hour, measured from 11.5M ticks: "
          f"${SPREAD_EVENING_USD:.2f} in 20-23 UTC, ${SPREAD_DAY_USD:.2f} elsewhere; "
          f"flat ${SPREAD_FLAT_USD:.2f} shown for comparison")
    print("clean exits only: no trade spans the daily stop or a weekend\n")

    for name, mb in (("4h", int(4 * 60 / minutes)), ("24h", per_day)):
        df = build("gold", a.tf, lo, hi, mb, z_bars)
        level = float(df["v_close"].mean())
        print(f"--- markup removed over {name} ---")
        print(f"   {'k':>3s} {'n':>5s} {'ECN':>7s} {'VANT':>7s} {'null mu':>8s} {'edge':>7s} "
              f"{'cost':>6s} {'net':>7s} {'net@flat':>9s} {'pctile':>7s} {'H1':>7s} {'H2':>7s}")
        for k in (1, 6, 12):
            t0 = trades(df, 2.0, k, 0, True)
            n = len(t0["sign"])
            if n < 30:
                print(f"   {k:3d} {n:5d}   too few")
                continue
            van = t0["sign"] * t0["van"]
            ecn = t0["sign"] * t0["ecn"]
            _, pct, _ = permute(t0["sign"], t0["van"], t0["vol"], a.draws, a.seed)
            # The null's centre: what a book with this long-share earns from drift.
            ranks = pd.qcut(t0["vol"], 10, labels=False, duplicates="drop")
            null_mu = float(sum(
                (ranks == b).sum() * t0["sign"][ranks == b].mean() * t0["van"][ranks == b].mean()
                for b in np.unique(ranks)) / n)
            c = cost_bp(t0["hour"], level, flat=False)
            c_flat = cost_bp(t0["hour"], level, flat=True)
            mid = n // 2
            _, p1, _ = permute(t0["sign"][:mid], t0["van"][:mid], t0["vol"][:mid], a.draws, a.seed)
            _, p2, _ = permute(t0["sign"][mid:], t0["van"][mid:], t0["vol"][mid:], a.draws, a.seed)
            print(f"   {k:3d} {n:5d} {ecn.mean():+7.3f} {van.mean():+7.3f} {null_mu:+8.3f} "
                  f"{van.mean() - null_mu:+7.3f} {c.mean():6.3f} {(van - c).mean():+7.3f} "
                  f"{(van - c_flat).mean():+9.3f} {pct:7.2f} {p1:7.2f} {p2:7.2f}")
        print(f"   entry delayed, percentile on the Vantage leg:")
        for k in (1, 6, 12):
            row = []
            for d in (0, 1, 2):
                td = trades(df, 2.0, k, d, True)
                if len(td["sign"]) < 30:
                    row.append("  n/a")
                    continue
                _, p, _ = permute(td["sign"], td["van"], td["vol"], a.draws, a.seed)
                v = (td["sign"] * td["van"]).mean()
                row.append(f"{p:6.2f} ({v:+.3f})")
            print(f"      k={k:<3d} delay 0/1/2: " + "   ".join(row))
        print()

    print("Sidak over the six registered cells: a two-sided p of 0.019 becomes 0.109.")
    print("The gate is 97.5 per cell as registered; the family correction is stated")
    print("here because the registration printed the multiplicity and never charged it.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
