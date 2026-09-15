"""Does the gold-silver residual revert, and by more than it costs to touch?

    python scripts/pair_residual.py 2010-06-01 2018-06-15

Every family this loop has closed asks one instrument about itself: a level, a
range, a session, a trend, a release. Twenty-nine registrations, no survivor
out of sample. This asks a different question, and it is the only mechanism on
the shelf that needs two instruments to exist at all.

Gold and silver share their macro drivers, so silver's move is mostly gold's
move times a beta. `2026-09-15-nfp-cross-asset` measured that beta at 1.66 in
basis points on quiet Fridays with an R-squared of 0.723 — which the adversary
used to prove silver was not independent evidence. The same number read the
other way is a signal: what is left over when gold's move is removed is
silver's own, and if that leftover is noise rather than news it should come
back.

The spread is built from RETURNS, not from log levels, and the difference is
not cosmetic. A level spread `log(s) - beta*log(g)` carries `log(g) ~ 7.2`, so
a beta that moves by 0.01 moves the spread by 72 basis points on its own: the
first version of this script measured beta's drift and called it divergence,
and printed reversions of -400 bp. Accumulating the residual return removes the
level entirely.

All of it strictly causal:

    beta_t = cov(silver, gold) / var(gold) over the trailing BETA_BARS,
             from returns that had all printed by t-1
    dev_t  = [log s_t - log s_{t-W}] - beta_t * [log g_t - log g_{t-W}]
             the residual return accumulated over the trailing W bars
    z_t    = dev_t / (sd of dev over the trailing Z_BARS)

The measurement is the residual return over the holding period, signed so that
positive means it came back, and priced the way the engine fills — the signal
is read on the close of t, the pair is on at the open of t+1 and off at the
open of t+1+k, with beta held at its entry value:

    reversion = -sign(z_t) * ( [log s_exit - log s_entry]
                             - beta_t * [log g_exit - log g_entry] )   in bp

**The cost this has to clear is stated before any number is read, and it is a
range because the true one is unknowable.** A pair is two legs, each paying its
own spread to get in and out. The only spreads this repository has are the ones
measured read-only against the live Vantage terminal on 2026-09-13 — gold $0.28
and silver $0.021 — and there is no history of what either was in 2012. Two
readings bracket it:

  * **proportional** (the working assumption): a dollar spread scales with the
    price, so today's quote over today's level holds throughout. At the span's
    end, gold $4,539 and silver $75.56, that is 0.62 + 2.78 = **3.40 bp** a
    round trip.
  * **dollar-constant** (the pessimistic bound): today's dollar spread over the
    span's own mean level, $1,731 and $24.29, is 1.62 + 8.64 = **10.26 bp**.

Both are printed for every cell. Neither is the truth and the gap between them
is larger than anything this measurement is likely to find, which is itself the
most useful sentence in this docstring.

**Trades do not overlap.** A signal fires on a large fraction of bars and a
position is held for k of them, so an overlapping sample counts the same four
hours of market up to sixteen times and inflates every t-statistic by roughly
the square root of that. Only non-overlapping trades are counted: a fire is
taken, and nothing fires again until the position is off.

This is a measurement, not a receipt. Run it on the exploratory half, decide
what is worth pre-registering, and test that on the half it never touched.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]

#: One bar is fifteen minutes; a Dukascopy trading day holds about 68 of them.
BARS_PER_DAY = 68

#: Half-spreads are paid four times in a round trip of two legs. These are the
#: measured Vantage quotes over the mean price level of each instrument, and
#: they are the number every cell below is judged against.
#: (proportional, dollar-constant) basis points per leg per round trip.
COST_BP = {"xauduka": (0.62, 1.62), "xagduka": (2.78, 8.64)}
COST_PROP = COST_BP["xauduka"][0] + COST_BP["xagduka"][0]      # 3.40 bp
COST_FIXED = COST_BP["xauduka"][1] + COST_BP["xagduka"][1]     # 10.26 bp


def load(market: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    import re
    text = (ROOT / "config" / "default.toml").read_text(encoding="utf-8")
    table = re.search(r"\[markets\." + re.escape(market) + r"\]\s*(.*?)(?=\n\[)", text, re.S)
    if table is None:
        sys.exit(f"no [markets.{market}] in config/default.toml")
    symbol = re.search(r'bar_symbol\s*=\s*"([^"]+)"', table.group(1)).group(1)
    t = pq.read_table(ROOT / "data" / "bars" / f"{symbol}-15m.parquet")
    t = t.filter(pc.and_(pc.greater_equal(t["time"], pa.scalar(lo, type=t["time"].type)),
                         pc.less(t["time"], pa.scalar(hi, type=t["time"].type))))
    df = t.to_pandas()[["time", "open", "close"]].sort_values("time").reset_index(drop=True)
    return df.rename(columns={"open": f"{market}_open", "close": f"{market}_close"})


def build(lo: pd.Timestamp, hi: pd.Timestamp, beta_days: int, dev_hours: int, z_days: int) -> pd.DataFrame:
    """The aligned pair, its causal beta, its accumulated residual and that residual's z-score.

    Both instruments must have a bar at the same stamp or the row is dropped:
    an inner join is the only honest alignment, because a residual computed
    against a stale leg is a residual against a price nobody could trade.
    """
    g = load("xauduka", lo, hi)
    s = load("xagduka", lo, hi)
    df = g.merge(s, on="time", how="inner").reset_index(drop=True)

    # Returns on closes; the spread is read on closes and traded at the NEXT
    # open, which is what the engine does and what the forward measure uses.
    df["g_ret"] = np.log(df["xauduka_close"]).diff()
    df["s_ret"] = np.log(df["xagduka_close"]).diff()

    beta_bars = beta_days * BARS_PER_DAY
    dev_bars = dev_hours * 4          # 15-minute bars
    z_bars = z_days * BARS_PER_DAY
    # `.shift(1)` on the rolling moments is what makes beta causal: the value
    # standing at t is built from returns that had all printed by t-1.
    cov = df["s_ret"].rolling(beta_bars).cov(df["g_ret"]).shift(1)
    var = df["g_ret"].rolling(beta_bars).var().shift(1)
    df["beta"] = cov / var

    # The accumulated residual RETURN over the trailing window. No log level
    # appears, so a drifting beta cannot masquerade as a divergence.
    ls, lg = np.log(df["xagduka_close"]), np.log(df["xauduka_close"])
    df["dev"] = (ls - ls.shift(dev_bars)) - df["beta"] * (lg - lg.shift(dev_bars))
    # Scale by the dev's own recent dispersion, again using only the past. The
    # mean is NOT subtracted: a residual return has no level to centre, and
    # subtracting a trailing mean would smuggle a second signal in.
    df["z"] = df["dev"] / df["dev"].rolling(z_bars).std().shift(1)
    return df


def cell(df: pd.DataFrame, threshold: float, k: int) -> dict:
    """One (threshold, horizon) reading, entered on the bar AFTER the signal.

    The signal is read on a close; the trade is on at the next open and off at
    the open k bars later. Both legs move, so the spread is measured on opens
    for the forward change and the entry is never the bar that produced the
    signal.
    """
    ls, lg = np.log(df["xagduka_open"]).to_numpy(), np.log(df["xauduka_open"]).to_numpy()
    beta = df["beta"].to_numpy()
    z = df["z"].to_numpy()
    n = len(df)
    rev = []
    i, blocked_until = 0, -1
    while i < n:
        # Entry at the next open, exit k bars later, beta frozen at the signal.
        e, x = i + 1, i + 1 + k
        if x >= n:
            break
        if i > blocked_until and np.isfinite(z[i]) and abs(z[i]) >= threshold and np.isfinite(beta[i]):
            fwd = (ls[x] - ls[e]) - beta[i] * (lg[x] - lg[e])
            if np.isfinite(fwd):
                rev.append(-np.sign(z[i]) * fwd * 10_000.0)
                blocked_until = x     # nothing fires again until the pair is off
        i += 1
    rev = np.array(rev)
    if len(rev) < 3:
        return {"n": int(len(rev))}
    sd = rev.std(ddof=1)
    return {
        "n": int(len(rev)),
        "gross": float(rev.mean()),
        "median": float(np.median(rev)),
        "win": 100.0 * float((rev > 0).mean()),
        "net_prop": float(rev.mean() - COST_PROP),
        "net_fixed": float(rev.mean() - COST_FIXED),
        "t": float(rev.mean() / sd * np.sqrt(len(rev))) if sd > 0 else 0.0,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("start", nargs="?", default="2010-06-01")
    ap.add_argument("end", nargs="?", default="2018-06-15")
    ap.add_argument("--beta-days", type=int, default=20)
    ap.add_argument("--dev-hours", type=int, nargs="+", default=[4, 24],
                    help="hours the residual is accumulated over before it counts as a divergence")
    ap.add_argument("--z-days", type=int, default=20)
    ap.add_argument("--threshold", type=float, nargs="+", default=[1.5, 2.0])
    ap.add_argument("--horizon", type=int, nargs="+", default=[4, 16, 68],
                    help="bars held: 4 = one hour, 16 = four hours, 68 = one day")
    args = ap.parse_args()

    lo, hi = pd.Timestamp(args.start, tz="UTC"), pd.Timestamp(args.end, tz="UTC")
    print(f"gold/silver residual, 15m, {args.start} -> {args.end}")
    print(f"beta over {args.beta_days} trading days, causal; non-overlapping trades only")
    print(f"a round trip costs {COST_PROP:.2f} bp if spreads scale with price, "
          f"{COST_FIXED:.2f} bp if they are constant in dollars")
    cells = len(args.dev_hours) * len(args.threshold) * len(args.horizon)
    print(f"{cells} cells are printed below and that is the multiplicity: the best of "
          f"{cells} looks is not evidence, it is a search\n")

    for dev_hours in args.dev_hours:
        df = build(lo, hi, args.beta_days, dev_hours, args.z_days)
        usable = int(np.isfinite(df["z"]).sum())
        print(f"residual accumulated over {dev_hours}h: {len(df)} aligned bars, {usable} with a z-score, "
              f"mean beta {df['beta'].mean():.3f}, mean |dev| {1e4*df['dev'].abs().mean():.1f} bp")
        print(f"   {'thr':>4s} {'k':>4s} {'trades':>7s} {'gross bp':>9s} {'median':>8s} {'win':>7s} "
              f"{'net@3.40':>9s} {'net@10.26':>10s} {'t':>7s}")
        for thr in args.threshold:
            for k in args.horizon:
                c = cell(df, thr, k)
                if c["n"] < 3:
                    print(f"   {thr:4.1f} {k:4d} {c['n']:7d}   too few")
                    continue
                print(f"   {thr:4.1f} {k:4d} {c['n']:7d} {c['gross']:+9.3f} {c['median']:+8.3f} "
                      f"{c['win']:6.1f}% {c['net_prop']:+9.3f} {c['net_fixed']:+10.3f} {c['t']:+7.2f}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
