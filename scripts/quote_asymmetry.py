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
trade, and no spread constant appears anywhere in this file. `gross` is the same
trade measured mid to mid, which is what the direction gates are really about.

**The clock is the BROKER's, not UTC.** `copy_ticks_range` stamps `time_msc` on
the server clock (Vantage: UTC+3 in New York summer, UTC+2 in winter) and the
ingest stores it unconverted; the parquet's `tz="UTC"` label is wrong and the
sessions running 01:00-23:58 are what give it away. For matching it is the
better variable — phase of the trading day, with the daily stop at the day
boundary — but it must not be read as UTC. Fault found by data-integrity.

Three faults this file has already paid for:

  * The first run stored `ret = sign * raw` and asked the null for
    `mean(sign * ret)`, which is `mean(raw)`: the permutation permuted nothing.
    Both sides of every fire are stored now, each paying its own spread, and the
    null selects between them. Receipt kept as `in-sample-VOID-permutation-bug.txt`.
  * The guards' thirty-minute cooldown counted rows of the trade list, not
    minutes of the clock.
  * `rolling(W)` counts rows, so the trailing sum spanned session gaps on 0.37%
    of minutes. A W-sum is now invalid unless its whole window is contiguous.
    (The five-day scale window spans sessions on purpose and is left alone.)
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
CELLS = ((1, 5), (1, 30), (5, 5), (5, 30))


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


def zscore(raw: pd.Series, contig: pd.Series, w: int) -> pd.Series:
    """Trailing sum over w CONTIGUOUS minutes, scaled by its own dispersion.

    The sum may include minute t — it is made of ticks that have printed. The
    scale may not: `.shift(1)` is what keeps the denominator in the past. A sum
    whose window straddles a session gap is dropped rather than used.
    """
    s = raw.rolling(w).sum()
    if w > 1:
        # `.astype(float)` BEFORE the rolling: a rolling min over a bool Series
        # returns NaN for every row.
        whole = contig.astype(float).rolling(w - 1).min()
        s = s.where(whole > 0)
    # The scale is built only from valid sums, and `min_periods` has to be
    # below the window or it can never be met: once the gap-straddling sums are
    # NaN, a 6,890-row window always contains one, and requiring 6,890 non-NaN
    # observations emptied both W=5 cells silently. 95% of the window is the
    # declared tolerance.
    denom = s.rolling(Z_WINDOW, min_periods=int(0.95 * Z_WINDOW)).std().shift(1)
    return s / denom


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

    out = {key: [] for key in ("sign", "long", "short", "gross", "vol", "hour", "month", "day", "when")}
    i, blocked = 0, -1
    while i < n:
        x = i + k
        if x >= n:
            break
        if i > blocked and np.isfinite(zz[i]) and abs(zz[i]) >= threshold:
            if contig[i + 1:x + 1].all() and np.isfinite(rng[i]):
                s = 1.0 if zz[i] > 0 else -1.0
                # BOTH sides are stored, each paying its own spread, because the
                # null permutes the side and a permuted trade must be charged the
                # quotes it would really have been given.
                r_long = 1e4 * (bid[x] - ask[i]) / ask[i] if ask[i] > 0 else np.nan
                r_short = 1e4 * (bid[i] - ask[x]) / bid[i] if bid[i] > 0 else np.nan
                m_in, m_out = (bid[i] + ask[i]) / 2, (bid[x] + ask[x]) / 2
                g = s * 1e4 * (m_out - m_in) / m_in if m_in > 0 else np.nan
                if np.isfinite(r_long) and np.isfinite(r_short) and np.isfinite(g):
                    out["sign"].append(s)
                    out["long"].append(r_long)
                    out["short"].append(r_short)
                    out["gross"].append(g)
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
    res["ret"] = np.where(res["sign"] > 0, res["long"], res["short"])
    return res


def cluster_t(values: np.ndarray, days: pd.DatetimeIndex) -> float:
    """t with the calendar day as the cluster, because same-day trades are one bet."""
    n = len(values)
    if n < 3:
        return float("nan")
    mu = values.mean()
    dev = values - mu
    s = pd.Series(dev).groupby(pd.Series(days.values)).sum().to_numpy()
    se = np.sqrt((s ** 2).sum()) / n
    return float(mu / se) if se > 0 else float("nan")


def matched_null(sign, r_long, r_short, vol, hour, draws: int, seed: int) -> tuple:
    """Percentile of the observed mean among sign permutations matched on BOTH
    the broker-clock hour and the volatility decile.

    Hour-matching is required, not optional: `2026-09-15-venue-residual` showed
    a volatility decile is really a liquidity proxy. The diagnostic that matters
    is not how many trades sit ALONE in a bucket but how many sit in a bucket
    whose members all carry the same side — those permute to themselves.
    """
    n = len(sign)
    if n < 30:
        return float("nan"), float("nan"), float("nan")
    dec = pd.qcut(vol, 10, labels=False, duplicates="drop")
    key = hour.astype(np.int64) * 100 + dec.astype(np.int64)
    groups = [np.nonzero(key == k)[0] for k in np.unique(key)]
    frozen = sum(len(g) for g in groups if len(g) < 2 or len(set(sign[g])) < 2)
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
    return observed, pct, 100.0 * frozen / n


def day_block_ci(values, days, resamples: int, seed: int) -> tuple:
    uniq = np.unique(days)
    idx = {d: np.nonzero(days == d)[0] for d in uniq}
    rs = np.random.default_rng(seed)
    means = np.empty(resamples)
    for i in range(resamples):
        pick = rs.choice(len(uniq), size=len(uniq), replace=True)
        take = np.concatenate([idx[uniq[j]] for j in pick])
        means[i] = values[take].mean()
    return float(np.percentile(means, 2.5)), float(np.percentile(means, 97.5))


def guarded(f: dict, signed: np.ndarray) -> tuple:
    """The same book under the project's configured limits: at most four trades
    a day and thirty MINUTES of clock between them."""
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


def concentration(values: np.ndarray, month: np.ndarray) -> tuple:
    """Share of the net carried by its biggest month — meaningless when the net
    is small next to the spread between months, and it says so rather than
    printing 513%."""
    net = values.sum()
    by = pd.Series(values).groupby(pd.Series(month)).sum()
    if abs(net) < by.abs().max():
        return "n/a", float("nan")
    worst = by.idxmax() if net > 0 else by.idxmin()
    return worst, 100.0 * by.loc[worst] / net


def run_arm(df: pd.DataFrame, raw: pd.Series, label: str, args) -> dict:
    print(f"\n=== {label} ===")
    print(f"   {'W':>2s} {'k':>3s} {'n':>6s} {'net bp':>8s} {'gross':>8s} {'up%':>6s} "
          f"{'pctile':>7s} {'CI low':>8s} {'CI high':>8s} {'g.month':>9s} {'share':>7s} "
          f"{'guard n':>8s} {'guard bp':>9s} {'g7':>4s} {'frozen':>7s}")
    kept = {}
    for w, k in CELLS:
        z = zscore(raw, df["contig"], w)
        f = fires(df, z, 2.0, k, args.lag)
        n = len(f["sign"])
        if n < 30:
            print(f"   {w:2d} {k:3d} {n:6d}   too few")
            continue
        signed, gross = f["ret"], f["gross"]
        _, pct, frozen = matched_null(f["sign"], f["long"], f["short"], f["vol"], f["hour"],
                                      args.draws, args.seed)
        lo_ci, hi_ci = day_block_ci(signed, f["day"], args.resamples, args.seed)
        mo, share = concentration(gross, f["month"])
        gn, gbp = guarded(f, signed)
        g7 = "ok" if (np.sign(gbp) == np.sign(signed.mean()) and np.isfinite(gbp)) else "FAIL"
        print(f"   {w:2d} {k:3d} {n:6d} {signed.mean():+8.4f} {gross.mean():+8.4f} "
              f"{100 * (gross > 0).mean():5.1f}% {pct:7.2f} {lo_ci:+8.4f} {hi_ci:+8.4f} "
              f"{mo:>9s} {share:6.1f}% {gn:8d} {gbp:+9.4f} {g7:>4s} {frozen:6.1f}%")
        kept[(w, k)] = f
    print("   share is of GROSS, the quantity the direction gates test; the registration's")
    print("   ceiling is 40%. frozen = trades in an hour x decile bucket that permutes to itself.")
    return kept


def diagnostics(df: pd.DataFrame, kept: dict, args) -> None:
    """Everything the registration declared and the first receipt did not carry."""
    print("\n--- declared diagnostics ---")

    print("\nlatency curve (registration: 0/1/5/30 s), signal arm, net bp then gross bp:")
    for w, k in CELLS:
        row = []
        for lag in (0, 1, 5, 30):
            z = zscore(df["os_sum"], df["contig"], w)
            f = fires(df, z, 2.0, k, lag)
            if len(f["sign"]) < 30:
                row.append("   n/a")
                continue
            row.append(f"{f['ret'].mean():+.3f}/{f['gross'].mean():+.3f}")
        print(f"   W={w} k={k:<3d} " + "   ".join(row))
    print("   a dealer-reaction signal decays with latency. Read which way these go.")

    print("\nwidening vs narrowing (registration: a diagnostic, never a cell):")
    print("   widening = ask_up + bid_dn (the spread opens); narrowing = ask_dn + bid_up")
    for name, series in (("widening", df["ask_up"] + df["bid_dn"]),
                         ("narrowing", df["ask_dn"] + df["bid_up"]),
                         ("ask side", df["ask_up"] + df["ask_dn"]),
                         ("bid side", df["bid_up"] + df["bid_dn"])):
        row = []
        for w, k in CELLS:
            z = zscore(series, df["contig"], w)
            f = fires(df, z, 2.0, k, args.lag)
            if len(f["sign"]) < 30:
                row.append("     n/a")
                continue
            row.append(f"{f['gross'].mean():+6.3f} (t{cluster_t(f['gross'], f['day']):+5.2f})")
        print(f"   {name:>9s}  " + "  ".join(row))
    print("   the claim was that both readings point one way. The columns are W1k5 W1k30 W5k5 W5k30.")

    print("\nregime (registration: the one-sided share is not stationary):")
    day_share = df.groupby(pd.DatetimeIndex(df["minute"]).normalize()).apply(
        lambda g: g["n_one"].sum() / max(g["n_one"].sum() + g["n_two"].sum(), 1))
    terc = pd.qcut(day_share, 3, labels=["low", "mid", "high"])
    print(f"   one-sided share by day: min {day_share.min():.3f} median "
          f"{day_share.median():.3f} max {day_share.max():.3f}")
    for w, k in CELLS:
        f = kept.get((w, k))
        if f is None:
            continue
        lab = terc.reindex(f["day"]).to_numpy()
        parts = []
        for t in ("low", "mid", "high"):
            m = lab == t
            parts.append(f"{t} {f['gross'][m].mean():+6.3f} (n{int(m.sum())})" if m.sum() > 30 else f"{t} n/a")
        print(f"   W={w} k={k:<3d} " + "  ".join(parts))

    print("\nthe concentration the adversary found, stated in full:")
    for w, k in CELLS:
        f = kept.get((w, k))
        if f is None:
            continue
        g, days = f["gross"], f["day"]
        by_day = pd.Series(g).groupby(pd.Series(days.values)).sum().sort_values(ascending=False)
        total = g.sum()
        crash = pd.Timestamp("2026-01-30")
        share_crash = 100.0 * by_day.get(crash, 0.0) / total if abs(total) > 1e-9 else float("nan")
        top5 = 100.0 * by_day.head(5).sum() / total if abs(total) > 1e-9 else float("nan")
        no_jan = g[pd.DatetimeIndex(days).month != 1]
        print(f"   W={w} k={k:<3d} gross {g.mean():+6.3f} (t{cluster_t(g, days):+5.2f}) | "
              f"2026-01-30 alone {share_crash:6.1f}% of net | top 5 days {top5:6.1f}% | "
              f"without January {no_jan.mean():+6.3f} (n{len(no_jan)})")
    print("   2026-01-30 is the day gold fell 9.5%.")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--symbol", default="XAUUSD.sc")
    ap.add_argument("--from", dest="start", default="2026-01-20")
    ap.add_argument("--to", dest="end", default="2026-06-16")
    ap.add_argument("--lag", type=int, default=1, choices=[0, 1, 5, 30])
    ap.add_argument("--draws", type=int, default=100_000)
    ap.add_argument("--resamples", type=int, default=20_000)
    ap.add_argument("--seed", type=int, default=20260915)
    a = ap.parse_args()
    lo, hi = pd.Timestamp(a.start, tz="UTC"), pd.Timestamp(a.end, tz="UTC")
    df = load(a.symbol, lo, hi)

    one, two = df["n_one"].sum(), df["n_two"].sum()
    days = pd.DatetimeIndex(df["minute"]).normalize().nunique()
    print(f"quote asymmetry, {a.symbol}, {lo.date()} -> {hi.date()} (exclusive), fill lag {a.lag}s")
    print(f"{len(df):,} minutes over {days} sessions, stamps on the BROKER clock (not UTC)")
    print(f"{one:,.0f} one-sided revisions ({100 * one / (one + two):.1f}%), {two:,.0f} two-sided; "
          f"the registration guessed 'about a third' and the tape says {100 * one / (one + two):.1f}%")
    print(f"classification disagrees with the tick's own flags on {df['flag_disagree'].sum():,.0f} "
          f"of {df['n_ticks'].sum():,.0f} revisions ({100 * df['flag_disagree'].sum() / df['n_ticks'].sum():.3f}%)")
    print(f"declared: zOS > 0 precedes a RISE, no mirror. Gate 97.5 on the matched null "
          f"({a.draws:,} draws, MC SE 0.06 pp), the day-block 95% interval must exclude zero,")
    print(f"no month over 40% of the net, at least 200 trades, the control arm must NOT clear")
    print(f"the gate, and the sign must survive the configured guards. 4 cells per arm.")

    kept = run_arm(df, df["os_sum"], "SIGNAL - one-sided revisions (the dealer deciding)", a)
    run_arm(df, df["ts_sum"], "CONTROL - two-sided revisions (price moving)", a)
    diagnostics(df, kept, a)
    return 0


if __name__ == "__main__":
    sys.exit(main())
