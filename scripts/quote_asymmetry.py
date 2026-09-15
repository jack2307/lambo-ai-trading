"""Does a one-sided quote revision say where the price goes next?

    python scripts/quote_asymmetry.py --from=2026-01-20 --to=2026-06-16

Registered as `docs/hypotheses/2026-09-15-quote-asymmetry.md` at commit 13178b5,
before this file existed. Features come from `py/ingest/mt5_quote_features.py`,
one row a minute, from the broker's own tick history on one clock.

Two arms, identical machinery, and the second is the control the hypothesis
declared rather than one added afterwards:

    signal   zOS = (os_sum over the trailing W minutes) / trailing sd, shift(1)
             os_sum = signed points over revisions where ONE side moved
    control  zTS = the same construction on two-sided revisions, which are price
             moving rather than a dealer deciding

Declared direction, with no mirror available: zOS > 0 precedes a RISE.

Fills are quotes, not assumptions. A long is bought at the ASK of the first tick
at least L seconds after the signal minute closes and sold at the BID of the
first tick at least L seconds after the exit minute closes; a short is the
reverse. The spread is therefore paid at what it actually was, twice, on every
trade, and no spread constant appears anywhere in this file.

The seven gates are in the registration. Six are computed here; the seventh
(the project's configured guards) is the `guarded` line under each table.
"""

from __future__ import annotations

import argparse
import glob
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]
MINUTES_PER_DAY = 1378          # what the tape actually holds, measured
Z_WINDOW = 5 * MINUTES_PER_DAY  # the trailing scale, five trading days


def load(symbol: str, lo: pd.Timestamp, hi: pd.Timestamp) -> pd.DataFrame:
    d = ROOT / "data" / "ticks" / symbol.replace(".", "_")
    files = sorted(glob.glob(str(d / "date=*.parquet")))
    if not files:
        sys.exit(f"no features in {d}; run py/ingest/mt5_quote_features.py first")
    keep = [f for f in files if lo.strftime("%Y-%m-%d") <= Path(f).stem[5:] < hi.strftime("%Y-%m-%d")]
    if not keep:
        sys.exit(f"no feature days inside {lo.date()} -> {hi.date()}")
    df = pd.concat([pq.read_table(f).to_pandas() for f in keep], ignore_index=True)
    df = df.sort_values("minute").reset_index(drop=True)
    step = df["minute"].diff().dt.total_seconds().div(60).fillna(1e9)
    df["contig"] = step <= 1.0
    return df


def zscore(raw: pd.Series, w: int) -> pd.Series:
    """Trailing sum over w minutes, scaled by its own trailing dispersion.

    The sum may include minute t — it is made of ticks that have printed. The
    scale may not: `.shift(1)` is what keeps the denominator in the past.
    """
    s = raw.rolling(w).sum()
    return s / s.rolling(Z_WINDOW).std().shift(1)


def fires(df: pd.DataFrame, z: pd.Series, threshold: float, k: int, lag: int) -> dict:
    """Non-overlapping trades, filled at quotes, never spanning a gap."""
    zz = z.to_numpy()
    contig = df["contig"].to_numpy()
    rng = df["mid_range"].to_numpy()
    hour = pd.DatetimeIndex(df["minute"]).hour.to_numpy()
    day = pd.DatetimeIndex(df["minute"]).normalize()
    month = pd.DatetimeIndex(df["minute"]).to_period("M").astype(str).to_numpy()
    bid = df[f"bid_{lag}s"].to_numpy()
    ask = df[f"ask_{lag}s"].to_numpy()
    minute = pd.DatetimeIndex(df["minute"])
    n = len(df)

    out = {key: [] for key in ("sign", "long", "short", "vol", "hour", "month", "day", "when")}
    i, blocked = 0, -1
    while i < n:
        x = i + k
        if x >= n:
            break
        if i > blocked and np.isfinite(zz[i]) and abs(zz[i]) >= threshold:
            # Every minute from the signal to the exit must be contiguous, so a
            # trade cannot be held across the daily stop or a weekend.
            if contig[i + 1:x + 1].all() and np.isfinite(rng[i]):
                s = 1.0 if zz[i] > 0 else -1.0
                # BOTH sides are stored, each paying its own spread, because the
                # null permutes the side and a permuted trade must be charged
                # the quotes it would really have been given. Storing one signed
                # return instead is how the first run of this file measured
                # `sign * (sign * raw)` and permuted nothing.
                r_long = 1e4 * (bid[x] - ask[i]) / ask[i] if ask[i] > 0 else np.nan
                r_short = 1e4 * (bid[i] - ask[x]) / bid[i] if bid[i] > 0 else np.nan
                if np.isfinite(r_long) and np.isfinite(r_short):
                    out["sign"].append(s)
                    out["long"].append(r_long)
                    out["short"].append(r_short)
                    out["vol"].append(rng[i])
                    out["hour"].append(hour[i])
                    out["month"].append(month[i])
                    out["day"].append(day[i])
                    out["when"].append(minute[i])
                    blocked = x
        i += 1
    res = {key: np.array(val) for key, val in out.items() if key not in ("day", "when")}
    res["day"] = pd.DatetimeIndex(out["day"])
    res["when"] = pd.DatetimeIndex(out["when"])
    # The book as actually traded: the declared side, paying its own spread.
    res["ret"] = np.where(res["sign"] > 0, res["long"], res["short"])
    return res


def matched_null(sign, r_long, r_short, vol, hour, draws: int, seed: int) -> tuple:
    """Percentile of the observed mean among sign permutations matched on BOTH
    the UTC hour and the volatility decile.

    Hour-matching is required, not optional: `2026-09-15-venue-residual` showed
    a volatility decile is really a liquidity proxy, so permuting inside it
    alone cannot break a mechanism that lives in one part of the day.
    """
    n = len(sign)
    if n < 30:
        return float("nan"), float("nan"), float("nan")
    dec = pd.qcut(vol, 10, labels=False, duplicates="drop")
    key = hour.astype(np.int64) * 100 + dec.astype(np.int64)
    groups = [np.nonzero(key == k)[0] for k in np.unique(key)]
    singles = sum(len(g) for g in groups if len(g) < 2)
    # Only the sides are permuted; each outcome stays on the minute it happened
    # and is re-priced at the quotes that side would have been filled at.
    observed = float(np.where(sign > 0, r_long, r_short).mean())
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
        stats[done:done + m] = np.where(perm > 0, r_long[None, :], r_short[None, :]).mean(axis=1)
        done += m
    pct = 100.0 * float((stats <= observed).mean())
    return observed, pct, 100.0 * singles / n


def day_block_ci(ret_signed, days, resamples: int, seed: int) -> tuple:
    """95% interval resampling whole calendar days, because same-day trades are
    one bet (risk, `2026-09-15-venue-residual`: 94% of trades shared a day)."""
    uniq = np.unique(days)
    idx = {d: np.nonzero(days == d)[0] for d in uniq}
    rs = np.random.default_rng(seed)
    means = np.empty(resamples)
    for i in range(resamples):
        pick = rs.choice(len(uniq), size=len(uniq), replace=True)
        take = np.concatenate([idx[uniq[j]] for j in pick])
        means[i] = ret_signed[take].mean()
    return float(np.percentile(means, 2.5)), float(np.percentile(means, 97.5))


def guarded(f: dict, signed: np.ndarray, k: int) -> tuple:
    """The same book under the project's configured limits: at most four trades
    a day and a thirty-minute cooldown between them."""
    # The cooldown is thirty MINUTES of wall clock, not thirty rows of the trade
    # list: the first version of this counted index distance and let a burst of
    # trades inside one minute through.
    when = f["when"]
    keep, last_day, count, last_t = [], None, 0, None
    for i in range(len(signed)):
        d = f["day"][i]
        if d != last_day:
            last_day, count, last_t = d, 0, None
        if count >= 4:
            continue
        if last_t is not None and (when[i] - last_t).total_seconds() < 30 * 60:
            continue
        keep.append(i)
        count += 1
        last_t = when[i]
    keep = np.array(keep, dtype=int)
    return (len(keep), float(signed[keep].mean()) if len(keep) else float("nan"))


def concentration(signed: np.ndarray, month: np.ndarray) -> tuple:
    net = signed.sum()
    if abs(net) < 1e-12:
        return "n/a", float("nan")
    by = pd.Series(signed).groupby(pd.Series(month)).sum()
    worst = by.idxmax() if net > 0 else by.idxmin()
    return worst, 100.0 * by.loc[worst] / net


def run_arm(df: pd.DataFrame, raw: pd.Series, label: str, args) -> None:
    print(f"\n=== {label} ===")
    print(f"   {'W':>2s} {'k':>3s} {'n':>6s} {'net bp':>8s} {'up%':>6s} {'pctile':>7s} "
          f"{'CI low':>8s} {'CI high':>8s} {'worst mo':>9s} {'share':>7s} "
          f"{'guard n':>8s} {'guard bp':>9s}")
    for w in (1, 5):
        z = zscore(raw, w)
        for k in (5, 30):
            f = fires(df, z, 2.0, k, args.lag)
            n = len(f["sign"])
            if n < 30:
                print(f"   {w:2d} {k:3d} {n:6d}   too few")
                continue
            signed = f["ret"]
            _, pct, singles = matched_null(f["sign"], f["long"], f["short"], f["vol"], f["hour"],
                                           args.draws, args.seed)
            lo_ci, hi_ci = day_block_ci(signed, f["day"], args.resamples, args.seed)
            mo, share = concentration(signed, f["month"])
            gn, gbp = guarded(f, signed, k)
            print(f"   {w:2d} {k:3d} {n:6d} {signed.mean():+8.4f} "
                  f"{100 * (signed > 0).mean():5.1f}% {pct:7.2f} {lo_ci:+8.4f} {hi_ci:+8.4f} "
                  f"{mo:>9s} {share:6.1f}% {gn:8d} {gbp:+9.4f}")
        print(f"      (W={w}: {singles:.1f}% of trades sit alone in their hour x decile bucket)")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--from", dest="start", default="2026-01-20")
    ap.add_argument("--to", dest="end", default="2026-06-16")
    ap.add_argument("--lag", type=int, default=1, choices=[0, 1, 5, 30],
                    help="seconds after the minute close before a fill is taken")
    ap.add_argument("--draws", type=int, default=100_000)
    ap.add_argument("--resamples", type=int, default=20_000)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")
    df = load(a.symbol, lo, hi)

    one, two = df["n_one"].sum(), df["n_two"].sum()
    print(f"quote asymmetry, {a.symbol}, {lo.date()} -> {hi.date()} (exclusive), "
          f"fill lag {a.lag}s")
    print(f"{len(df):,} minutes over {df['minute'].dt.normalize().nunique()} days; "
          f"{one:,.0f} one-sided revisions ({100 * one / (one + two):.1f}%), {two:,.0f} two-sided")
    print(f"price-derived classification disagrees with the tick's own flags on "
          f"{df['flag_disagree'].sum():,.0f} of {df['n_ticks'].sum():,.0f} revisions "
          f"({100 * df['flag_disagree'].sum() / df['n_ticks'].sum():.3f}%)")
    print(f"declared: zOS > 0 precedes a RISE, no mirror. Gate 97.5 on the matched null "
          f"({a.draws:,} draws), the day-block 95% interval must exclude zero,")
    print(f"no month over 40% of the net, at least 200 trades, and the control arm must NOT clear the gate.")
    print(f"4 cells per arm and that is the multiplicity.")

    run_arm(df, df["os_sum"], "SIGNAL — one-sided revisions (the dealer deciding)", a)
    run_arm(df, df["ts_sum"], "CONTROL — two-sided revisions (price moving)", a)
    return 0


if __name__ == "__main__":
    sys.exit(main())
