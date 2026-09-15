"""Does a dealer's quote leave the consensus before the price moves?

    python scripts/venue_residual.py align   --tf=5m  --start=2025-06-01 --end=2026-06-01
    python scripts/venue_residual.py measure --tf=5m  --start=2025-06-01 --end=2026-06-01

Registered as `docs/hypotheses/2026-09-15-venue-residual.md` at commit dc7d7e5,
which is what makes this file a measurement rather than a search.

Every family this loop has closed reads one venue's bars about themselves.
This reads two venues about each other. Vantage is a dealer running its own
book; Dukascopy is an ECN aggregating LP quotes. Both files here are the BID
side (`dukascopy-node -t m1 -f csv` on its bid feed; MT5 `copy_rates` returns
bid bars), so the comparison is like for like and whatever constant markup
separates them is a level, removed causally below.

The claim is inventory: a dealer marks away from consensus to discourage the
flow it already has too much of. If that skew knows anything, it must show up
in the **ECN leg's** next move — a number the dealer's own quote cannot
manufacture by converging back.

    skew_t   = 1e4 * [log(v_close_t) - log(e_close_t)]        raw, in bp
    markup_t = median(skew) over the trailing M bars, shift(1)  causal
    resid_t  = skew_t - markup_t
    z_t      = resid_t / sd(resid over the trailing Z bars, shift(1))

and the declared direction, written down before the first run: a POSITIVE skew
precedes a FALL. So the signed outcome of a fire at t is

    r = -sign(z_t) * 1e4 * [log(close_{t+1+k}) - log(open_{t+1})]

read on the ECN leg as the statistic and on the Vantage leg for the cost gate,
entered at the open AFTER the signal bar, and non-overlapping: nothing fires
again until the position is off. An overlapping sample counts the same hour up
to k times and inflates every t by about sqrt(k).

Three things can fake this effect, and each has a control here:

  * **The clocks.** The Vantage side came off the broker's clock (UTC+3 in New
    York summer, UTC+2 in winter) through our own exporter, and this repository
    has been bitten by a timezone twice. One bar of misalignment manufactures a
    skew exactly the size of one bar's return. `align` proves the clocks agree
    WITHOUT using a single return: gold stops for an hour a day, and the two
    feeds must open and close that break on the same UTC minute and shift it on
    the same DST dates. It runs first and its failure stops the study.
  * **The last tick.** A bar close on two venues is two different final ticks,
    so |skew| grows with volatility for a reason that is not inventory. The
    null therefore permutes the signal's SIGN within volatility deciles: a skew
    that is only volatility cannot clear it.
  * **The search.** Six cells are printed and six is the multiplicity. The best
    of six is not evidence.

The permutation runs at 100,000 draws because the gate is a percentile and a
gate must state the precision it requires: at 1,000 draws the standard error is
0.74 percentage points and a 97.5 gate cannot be judged by it; at 100,000 it is
0.07.
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

#: (dealer feed, ECN feed) by asset. Both are bid.
PAIRS = {
    "gold": ("XAUUSD", "XAUDUKA"),
    "btc": ("BTCUSD", "BTCUSDT"),
}

#: The Vantage round trip, measured read-only against the live terminal on
#: 2026-09-13 and the only spread history this repository owns. Read two ways
#: because the true one over a past window is unknowable: proportional to the
#: price level, or constant in dollars over the window's own mean level.
GOLD_SPREAD_USD = 0.28
BTC_SPREAD_USD = 12.0  # order of magnitude only; the BTC leg is out of sample


def tf_minutes(tf: str) -> int:
    return {"1m": 1, "5m": 5, "15m": 15}[tf]


def load(symbol: str, tf: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    path = ROOT / "data" / "bars" / f"{symbol}-{tf}.parquet"
    if not path.exists():
        sys.exit(f"missing {path}")
    t = pq.read_table(path)
    t = t.filter(
        pc.and_(
            pc.greater_equal(t["time"], pa.scalar(lo, type=t["time"].type)),
            pc.less(t["time"], pa.scalar(hi, type=t["time"].type)),
        )
    )
    df = t.to_pandas()[["time", "open", "high", "low", "close"]]
    return df.sort_values("time").reset_index(drop=True)


# ---------------------------------------------------------------- alignment


def breaks(df: pd.DataFrame, minutes: int) -> pd.DataFrame:
    """Every gap in the series, as (start of the hole, reopen, hours).

    A gap is a missing run, not a return. Dukascopy's converter drops flat
    zero-volume bars, so its holes include the daily break (48,660 minutes of
    the 17:00 New York hour over four years, counted 2026-09-13); MT5 simply
    has no bars there. Both are holes, which is why this test can compare them.
    """
    t = pd.DatetimeIndex(df["time"])
    # Minutes between consecutive stamps. Computed through `total_seconds` and
    # not from `asi8`, because these files are datetime64[ms] and the first
    # version of this line divided milliseconds by a nanosecond constant and
    # found no gaps at all in a series that is closed every weekend.
    step = t.to_series().diff().dt.total_seconds().to_numpy()[1:] / 60.0
    idx = np.nonzero(step > minutes)[0]
    return pd.DataFrame(
        {
            "hole_from": t[idx] + pd.Timedelta(minutes=minutes),
            "reopen": t[idx + 1],
            "hours": step[idx] / 60.0,
        }
    )


def daily_break_minutes(df: pd.DataFrame, minutes: int) -> pd.DataFrame:
    """The one-hour-ish daily stop, as minute-of-day UTC, per ISO week.

    Kept deliberately loose on duration (0.5h to 4h) and tight on placement
    (the hole must start between 19:00 and 23:59 UTC, where 17:00 New York
    falls in both halves of the year), so that a feed whose clock is an hour
    out is still caught rather than filtered away.
    """
    b = breaks(df, minutes)
    if b.empty:
        return pd.DataFrame(columns=["week", "close_mod", "open_mod", "n"])
    start = pd.DatetimeIndex(b["hole_from"])
    keep = (b["hours"] >= 0.5) & (b["hours"] <= 4.0) & (start.hour >= 19)
    b = b.loc[keep].copy()
    if b.empty:
        return pd.DataFrame(columns=["week", "close_mod", "open_mod", "n"])
    s, r = pd.DatetimeIndex(b["hole_from"]), pd.DatetimeIndex(b["reopen"])
    b["week"] = s.tz_localize(None).to_period("W").astype(str)
    b["close_mod"] = s.hour * 60 + s.minute
    b["open_mod"] = r.hour * 60 + r.minute

    def mode(x):
        m = x.mode()
        return int(m.iloc[0]) if len(m) else -1

    g = b.groupby("week").agg(close_mod=("close_mod", mode), open_mod=("open_mod", mode), n=("hours", "size"))
    return g.reset_index()


def hhmm(m: int) -> str:
    return "--:--" if m < 0 else f"{m // 60:02d}:{m % 60:02d}"


def stage_align(asset: str, tf: str, lo: pd.Timestamp, hi: pd.Timestamp) -> int:
    dealer, ecn = PAIRS[asset]
    minutes = tf_minutes(tf)
    dv, de = load(dealer, tf, lo, hi), load(ecn, tf, lo, hi)
    print(f"alignment gate, {asset} {tf}, {lo.date()} -> {hi.date()}")
    print(f"  {dealer}: {len(dv):,} bars, {dv['time'].iloc[0]} -> {dv['time'].iloc[-1]}")
    print(f"  {ecn}: {len(de):,} bars, {de['time'].iloc[0]} -> {de['time'].iloc[-1]}")
    print("  the daily one-hour stop, by ISO week, as UTC minute-of-day.")
    print("  no return is read anywhere in this test.\n")

    a = daily_break_minutes(dv, minutes).set_index("week")
    b = daily_break_minutes(de, minutes).set_index("week")
    weeks = sorted(set(a.index) & set(b.index))
    if not weeks:
        print("  FAIL: no week has a daily break on both feeds")
        return 1

    rows, bad_close, bad_open = [], 0, 0
    for w in weeks:
        ca, cb = int(a.loc[w, "close_mod"]), int(b.loc[w, "close_mod"])
        oa, ob = int(a.loc[w, "open_mod"]), int(b.loc[w, "open_mod"])
        dc, do = ca - cb, oa - ob
        bad_close += int(dc != 0)
        bad_open += int(do != 0)
        rows.append((w, ca, cb, dc, oa, ob, do))

    # The DST shift is a CHANGE, so it survives the dropped-flat-bar bias that
    # can move a close-side edge: both feeds must move the break in the same week.
    def shifts(series):
        out = []
        prev = None
        for w in weeks:
            v = int(series.loc[w])
            if prev is not None and v != prev:
                out.append((w, prev, v))
            prev = v
        return out

    sa_o, sb_o = shifts(a["open_mod"]), shifts(b["open_mod"])

    print(f"  {'week':<12s} {'reopen ' + dealer:>14s} {'reopen ' + ecn:>14s} {'diff':>6s}   "
          f"{'close ' + dealer:>13s} {'close ' + ecn:>13s} {'diff':>6s}")
    for w, ca, cb, dc, oa, ob, do in rows:
        flag = "" if (do == 0 and dc == 0) else "   <-- disagrees"
        print(f"  {w:<12s} {hhmm(oa):>14s} {hhmm(ob):>14s} {do:>6d}   "
              f"{hhmm(ca):>13s} {hhmm(cb):>13s} {dc:>6d}{flag}")

    print(f"\n  weeks compared: {len(weeks)}")
    print(f"  reopen minute disagrees in {bad_open} week(s); close minute in {bad_close} week(s)")
    print(f"  {dealer} moved its reopen in weeks: {[w for w, _, _ in sa_o] or 'never'}")
    print(f"  {ecn} moved its reopen in weeks: {[w for w, _, _ in sb_o] or 'never'}")

    same_shifts = [w for w, _, _ in sa_o] == [w for w, _, _ in sb_o]
    ok = bad_open == 0 and same_shifts
    print()
    if ok:
        print("  PASS: the two clocks agree to the minute on the reopen and shift together.")
        if bad_close:
            print(f"        {bad_close} close-side week(s) differ, which is the known "
                  f"dropped-flat-bar bias in the Dukascopy converter, not a clock.")
    else:
        print("  FAIL: the clocks cannot be shown to agree. The study stops here and the")
        print("        finding is an instrument fault, not a measurement.")
    return 0 if ok else 1


# ---------------------------------------------------------------- measurement


def build(asset: str, tf: str, lo: pd.Timestamp, hi: pd.Timestamp, markup_bars: int, z_bars: int) -> pd.DataFrame:
    dealer, ecn = PAIRS[asset]
    minutes = tf_minutes(tf)
    dv = load(dealer, tf, lo, hi).rename(columns={c: f"v_{c}" for c in ("open", "high", "low", "close")})
    de = load(ecn, tf, lo, hi).rename(columns={c: f"e_{c}" for c in ("open", "high", "low", "close")})
    df = dv.merge(de, on="time", how="inner").reset_index(drop=True)

    # A bar whose predecessor is more than two intervals behind it sits on the
    # far side of a hole; a reopen is not a skew.
    step = df["time"].diff().dt.total_seconds().div(60).fillna(1e9)
    df["fresh"] = step <= 2 * minutes

    df["skew"] = 1e4 * (np.log(df["v_close"]) - np.log(df["e_close"]))
    df["markup"] = df["skew"].rolling(markup_bars).median().shift(1)
    df["resid"] = df["skew"] - df["markup"]
    df["z"] = df["resid"] / df["resid"].rolling(z_bars).std().shift(1)
    # The control variable: the bar's own range on the ECN leg, in bp.
    df["range_bp"] = 1e4 * (df["e_high"] - df["e_low"]) / df["e_close"]
    return df


def fires(df: pd.DataFrame, threshold: float, k: int) -> dict:
    """Non-overlapping fires, entered at the open after the signal bar."""
    z = df["z"].to_numpy()
    fresh = df["fresh"].to_numpy()
    rng = df["range_bp"].to_numpy()
    e_open, e_close = df["e_open"].to_numpy(), df["e_close"].to_numpy()
    v_open, v_close = df["v_open"].to_numpy(), df["v_close"].to_numpy()
    n = len(df)
    sign, r_ecn, r_van, vol = [], [], [], []
    i, blocked = 0, -1
    while i < n:
        e, x = i + 1, i + k
        if x >= n:
            break
        if i > blocked and fresh[i] and np.isfinite(z[i]) and abs(z[i]) >= threshold:
            fe = 1e4 * (np.log(e_close[x]) - np.log(e_open[e]))
            fv = 1e4 * (np.log(v_close[x]) - np.log(v_open[e]))
            if np.isfinite(fe) and np.isfinite(fv) and np.isfinite(rng[i]):
                sign.append(-np.sign(z[i]))  # declared: positive skew precedes a fall
                r_ecn.append(fe)
                r_van.append(fv)
                vol.append(rng[i])
                blocked = x
        i += 1
    return {
        "sign": np.array(sign),
        "ecn": np.array(r_ecn),
        "van": np.array(r_van),
        "vol": np.array(vol),
    }


def permute(sign: np.ndarray, outcome: np.ndarray, vol: np.ndarray, draws: int, seed: int) -> tuple:
    """Percentile of the observed mean among `draws` sign-permutations.

    The signs are shuffled WITHIN volatility deciles, so the control keeps the
    relationship between |skew| and volatility that the two feeds'
    non-synchronous last ticks create on their own.
    """
    n = len(sign)
    if n < 3:
        return float("nan"), float("nan"), float("nan")
    ranks = pd.qcut(vol, 10, labels=False, duplicates="drop")
    groups = [np.nonzero(ranks == b)[0] for b in np.unique(ranks)]
    actual = float((sign * outcome).mean())
    rs = np.random.default_rng(seed)
    stats = np.empty(draws)
    chunk = max(1, int(2e7 // max(n, 1)))
    done = 0
    while done < draws:
        m = min(chunk, draws - done)
        perm = np.repeat(sign[None, :], m, axis=0)
        for g in groups:
            if len(g) < 2:
                continue
            order = np.argsort(rs.random((m, len(g))), axis=1)
            perm[:, g] = sign[g][order]
        stats[done:done + m] = (perm * outcome[None, :]).mean(axis=1)
        done += m
    pct = 100.0 * float((stats <= actual).mean())
    return actual, pct, float(stats.std(ddof=1))


def stage_measure(asset: str, tf: str, lo: pd.Timestamp, hi: pd.Timestamp, draws: int, seed: int) -> int:
    dealer, ecn = PAIRS[asset]
    minutes = tf_minutes(tf)
    per_day = int(23 * 60 / minutes)
    markups = {"4h": int(4 * 60 / minutes), "24h": per_day}
    z_bars = 5 * per_day
    thresholds = [2.0]
    horizons = [1, 6, 12]
    cells = len(markups) * len(thresholds) * len(horizons)

    spread = GOLD_SPREAD_USD if asset == "gold" else BTC_SPREAD_USD
    print(f"venue residual, {asset} {tf}, {dealer} (dealer) vs {ecn} (ECN), {lo.date()} -> {hi.date()}")
    print(f"declared before the run: a POSITIVE skew precedes a FALL; the mirror is a")
    print(f"second look, so the gate is the TWO-SIDED 97.5th, not the 95th.")
    print(f"null: sign permuted within volatility deciles, {draws:,} draws "
          f"(standard error {100 * (0.5 / draws ** 0.5):.2f} pp on a percentile)")
    print(f"{cells} cells below and that is the multiplicity: the best of {cells} is a search\n")

    for name, mb in markups.items():
        df = build(asset, tf, lo, hi, mb, z_bars)
        lvl = float(df["v_close"].mean())
        cost_prop = 1e4 * spread / lvl
        usable = int(np.isfinite(df["z"]).sum())
        print(f"markup removed over {name} ({mb} bars), z over {z_bars} bars: "
              f"{len(df):,} aligned bars, {usable:,} with a z")
        print(f"   mean raw skew {df['skew'].mean():+.2f} bp, "
              f"mean |residual| {df['resid'].abs().mean():.2f} bp, "
              f"round trip on {dealer} {cost_prop:.2f} bp at ${spread:.2f} / ${lvl:,.0f}")
        print(f"   {'thr':>4s} {'k':>3s} {'fires':>6s} {'ECN bp':>8s} {'up%':>6s} {'pctile':>7s} "
              f"{'Vantage bp':>11s} {'net':>8s}")
        for thr in thresholds:
            for k in horizons:
                f = fires(df, thr, k)
                n = len(f["sign"])
                if n < 3:
                    print(f"   {thr:4.1f} {k:3d} {n:6d}   too few")
                    continue
                signed_e = f["sign"] * f["ecn"]
                signed_v = f["sign"] * f["van"]
                _, pct, _ = permute(f["sign"], f["ecn"], f["vol"], draws, seed)
                print(f"   {thr:4.1f} {k:3d} {n:6d} {signed_e.mean():+8.3f} "
                      f"{100 * (signed_e > 0).mean():5.1f}% {pct:6.2f} "
                      f"{signed_v.mean():+11.3f} {signed_v.mean() - cost_prop:+8.3f}")
        print()
    print("gate: pctile >= 97.5 AND fires >= 200 AND net > 0. A pass on the percentile")
    print("with a failure on the net is 'direction exists, not a trade'.")
    return 0


# ---------------------------------------------------------------- the control


def stage_control(asset: str, tf: str, lo, hi, draws: int, seed: int) -> int:
    """Is the venue residual anything more than the dealer's own last return?

    The registered signal fired in the direction opposite to the declared one,
    which makes the trivial explanation the important one: Dukascopy's bar
    close may simply be an older tick than Vantage's. If it is, `resid` is just
    the Vantage return that has already printed, and a two-venue costume adds
    nothing to plain momentum.

    Three signals, identical machinery, identical cells:

      resid   the registered one, z of (Vantage - ECN) with the markup removed
      mom     z of Vantage's OWN last-bar return, no ECN anywhere
      stale   the same residual against a DELIBERATELY staler ECN close
              (the ECN leg lagged one more bar). If staleness is the driver
              this is the strongest of the three; if inventory is, it is the
              weakest.
    """
    dealer, ecn = PAIRS[asset]
    minutes = tf_minutes(tf)
    per_day = int(23 * 60 / minutes)
    z_bars = 5 * per_day
    df = build(asset, tf, lo, hi, int(4 * 60 / minutes), z_bars)

    v_ret = 1e4 * (np.log(df["v_close"]) - np.log(df["v_close"].shift(1)))
    df_mom = df.copy()
    df_mom["z"] = v_ret / v_ret.rolling(z_bars).std().shift(1)

    df_stale = df.copy()
    skew_stale = 1e4 * (np.log(df["v_close"]) - np.log(df["e_close"].shift(1)))
    resid_stale = skew_stale - skew_stale.rolling(int(4 * 60 / minutes)).median().shift(1)
    df_stale["z"] = resid_stale / resid_stale.rolling(z_bars).std().shift(1)

    spread = GOLD_SPREAD_USD if asset == "gold" else BTC_SPREAD_USD
    cost = 1e4 * spread / float(df["v_close"].mean())
    print(f"control, {asset} {tf}, {lo.date()} -> {hi.date()}, markup 4h, threshold 2.0")
    print("signs are reported in the direction the data took, not the one declared:")
    print("a POSITIVE number means the price moved WITH the skew (the mirror of the")
    print("registration), which is what the in-sample run found.")
    print(f"round trip on {dealer} {cost:.2f} bp; null {draws:,} draws, "
          f"sign permuted in volatility deciles\n")
    print(f"   {'signal':>7s} {'k':>3s} {'fires':>6s} {'ECN bp':>8s} {'Vantage bp':>11s} {'net':>8s} "
          f"{'up%':>6s} {'pctile':>7s}")
    corr = float(pd.concat([df["resid"], v_ret], axis=1).corr().iloc[0, 1])
    for label, d in (("resid", df), ("mom", df_mom), ("stale", df_stale)):
        for k in (1, 6, 12):
            f = fires(d, 2.0, k)
            n = len(f["sign"])
            if n < 3:
                print(f"   {label:>7s} {k:3d} {n:6d}   too few")
                continue
            e = -f["sign"] * f["ecn"]   # the mirror: with the skew, not against it
            v = -f["sign"] * f["van"]
            _, pct, _ = permute(-f["sign"], f["ecn"], f["vol"], draws, seed)
            print(f"   {label:>7s} {k:3d} {n:6d} {e.mean():+8.3f} {v.mean():+11.3f} "
                  f"{v.mean() - cost:+8.3f} {100 * (e > 0).mean():5.1f}% {pct:6.2f}")
    print()
    print(f"correlation between the registered residual and the dealer's own last-bar return: {corr:+.3f}")
    fh = fires(df, 2.0, 1)
    hrs = pd.DatetimeIndex(df["time"]).hour
    z = df["z"].to_numpy()
    mask = np.isfinite(z) & (np.abs(z) >= 2.0) & df["fresh"].to_numpy()
    counts = pd.Series(hrs[mask]).value_counts().sort_index()
    print("fires by UTC hour (all fires, before the non-overlap rule):")
    print("   " + "  ".join(f"{h:02d}:{c}" for h, c in counts.items()))
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("stage", choices=["align", "measure", "control"])
    ap.add_argument("--asset", default="gold", choices=sorted(PAIRS))
    ap.add_argument("--tf", default="5m", choices=["1m", "5m", "15m"])
    ap.add_argument("--start", default="2025-06-01")
    ap.add_argument("--end", default="2026-06-01")
    ap.add_argument("--draws", type=int, default=100_000,
                    help="declared, not defaulted: a percentile gate states the precision it requires")
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")
    if a.stage == "align":
        return stage_align(a.asset, a.tf, lo, hi)
    if a.stage == "control":
        return stage_control(a.asset, a.tf, lo, hi, a.draws, a.seed)
    return stage_measure(a.asset, a.tf, lo, hi, a.draws, a.seed)


if __name__ == "__main__":
    sys.exit(main())
