# -*- coding: utf-8 -*-
"""What a BOS and a CHoCH marker cost: frequency, whipsaw, lag and what follows.

    python py/research/smc_structure_measure.py

Prints every table in `docs/decisions/2026-09-19-smc-structure-measured.md`.
Deterministic: no sampling, no seeds, no permutations - every number here is a
count or a quantile over a fixed bar series.

WHY THIS EXISTS. The levels route is about to draw two new markers. The twelve
raw indicators on this desk carry the verdict of the registration that killed
them and the three method indicators carry the 2026-09-18 bias study's latency
and whipsaw table; BOS and CHoCH would be the first lines shipped with nothing
attached. This measures the EVENT - how often it fires, how often it is undone,
how late it is at a turn a chart reader would point at - in the same shape and
with the same functions as that study, so the two tables can be read side by
side.

WHAT IT IS NOT. It is not a test of the SMC method. That was done: the full ICT
chain was ported from the expert's manual, passed in-sample on three months of
broker minutes and lost on four years of a second feed - 1,428 trades, PF
0.746, expectancy -0.190R (`docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md`).
Nothing below rebuts that and nothing below may be read as reopening it. A
distribution of what follows an event is a description; the moment it acquires
an entry and a stop it needs its own registration and its own falsifier.

THE DEFINITIONS ARE PINNED HERE, because prose about "a break of structure"
does not pin them and each choice below moves the numbers - the same lesson the
zigzag port paid for on 2026-09-18, where three of four independent choices
differed from the Python and all three still reproduced the owner's morning.

  1. Swings are `fractal(2,2)`, walked by the same loop as
     `bias_defs.fractal_structure(bars, 2)`. `check_walk()` asserts the labels
     this file derives equal that function's on every bar, so the swing state
     used for the events is provably the swing state behind the label.
  2. A swing is usable only once CONFIRMED - two bars must print past it. The
     level a break is measured against at bar t is therefore never a level a
     live reader could not already see.
  3. The prevailing structure is that same `fractal(2)` label at the bar,
     which is what `htf.rs` serves and what the bias study measured.
  4. A BREAK is a bar CLOSING beyond the level. Never a wick. A wick-based
     rule fires on every spike and would make the frequency column a measure
     of the tape's noise.
  5. A LEVEL IS SPENT ONCE BROKEN. Without this, price holding above an old
     swing high for forty bars prints forty BOS and the frequency column
     becomes a measure of how long trends last. One swing point, one break.
  6. BOS = break in the direction of the prevailing structure. CHoCH = break
     against it.
  7. FLAT structure yields NO event, because a rule with no direction has no
     "with" and no "against". `fractal(2)` is FLAT on a large share of bars,
     so this choice is expensive and is reported both ways: the carry-forward
     variant (use the last non-FLAT label) is measured beside it rather than
     argued about.
"""
import datetime as dt
import hashlib
import os
import statistics as st
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

from bias_defs import (UP, DOWN, FLAT, T, O, H, L, C, atr, atr_zigzag,
                       fractal_structure)
from bias_measures import forward_map
from bias_ref import reference_turns, latency
from bars_fingerprint import NO_VOLUME

FIXTURE = os.path.join(ROOT, "crates", "fd-api", "tests", "fixtures", "zigzag-h1-xauusd.csv")


# --------------------------------------------------------------- 0. THE BARS
#
# Same resolution order as the rest of the study: FD_BARS, else the repo's
# data/bars. A worktree checkout has neither - data/ is git-ignored and is
# fetched, not committed - so this falls back to the 2,000 real H1 bars that
# ARE in the repository, the ones the zigzag and method fixtures are pinned
# against. That is a smaller sample than the study's and the note says so in
# its own table rather than here; inventing bars was the alternative and would
# have measured an arithmetic exercise instead of a market.
def load_bars(tf="1h"):
    """Returns (bars, source dict). Prefers the exported parquet if present."""
    root = os.environ.get("FD_BARS") or os.path.join(ROOT, "data", "bars")
    path = os.path.join(root, "XAUUSD-%s.parquet" % tf)
    if os.path.exists(path):
        from bias_measures import load
        bars = load(path)
        return bars, {"kind": "parquet", "path": rel(path)}
    if tf != "1h":
        return None, {"kind": "missing", "path": rel(path)}
    bars = []
    with open(FIXTURE, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            f = line.split(",")
            when = dt.datetime(1970, 1, 1) + dt.timedelta(milliseconds=int(f[0]))
            bars.append((when, float(f[1]), float(f[2]), float(f[3]), float(f[4])))
    return bars, {"kind": "fixture", "path": rel(FIXTURE), "abs": FIXTURE}


def rel(path):
    """Repo-relative where possible: the note's table has to read the same on
    the VPS, on the desktop and in a worktree, and an absolute path in a
    published table is a path that was true once on one machine."""
    try:
        r = os.path.relpath(path, ROOT)
    except ValueError:                        # different drive
        return path.replace("\\", "/")
    return path.replace("\\", "/") if r.startswith("..") else r.replace("\\", "/")


def fingerprint_rows(bars):
    """The bar digest under `bars_fingerprint`'s recipe, so it is comparable
    with the study's table rather than merely similar to it.

    Bit patterns, big-endian, time as int64 ms, then OHLC as float64, then the
    absent-volume sentinel imported from that module - the fixture carries no
    volume column and "no volume" must not collide with "volume 0"."""
    d = hashlib.sha256()
    for b in bars:
        ms = int((b[T] - dt.datetime(1970, 1, 1)).total_seconds() * 1000)
        d.update(struct.pack(">q", ms))
        for k in (O, H, L, C):
            d.update(struct.pack(">d", float(b[k])))
        d.update(NO_VOLUME)
    return d.hexdigest()


def file_sha256(path):
    d = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            d.update(chunk)
    return d.hexdigest()


# ----------------------------------------------- 1. THE SWING STATE, WALKED
def structure_walk(bars, n=2, strict=False):
    """Per bar: the fractal(2) label and the last CONFIRMED swing on each side.

    This is `bias_defs.fractal_structure`'s loop with the swing lists exposed,
    because the events need the levels and that function returns only the
    label. `check_walk` below asserts the two agree on every bar of whatever
    data this run read - not on a synthetic prefix, on the real series - so a
    later edit to either cannot silently split them.

    `strict` switches the tie rule to `htf.rs::fractal_swings`'s - strictly
    greater than every neighbour, so a flat top is not a swing - while the
    default is `bias_defs`'s: not beaten by any neighbour and strictly
    beating at least one, which DOES admit a flat top. The two files really do
    disagree and the difference is measured below rather than assumed small.
    """
    labels = [FLAT] * len(bars)
    hi_lvl = [None] * len(bars)     # (bar_index, price) of the newest confirmed high
    lo_lvl = [None] * len(bars)
    highs, lows = [], []
    for t in range(len(bars)):
        i = t - n
        if i >= n:
            win = range(i - n, i + n + 1)
            if strict:
                is_hi = all(bars[i][H] > bars[j][H] for j in win if j != i)
                is_lo = all(bars[i][L] < bars[j][L] for j in win if j != i)
            else:
                is_hi = (all(bars[i][H] >= bars[j][H] for j in win) and
                         any(bars[i][H] > bars[j][H] for j in win if j != i))
                is_lo = (all(bars[i][L] <= bars[j][L] for j in win) and
                         any(bars[i][L] < bars[j][L] for j in win if j != i))
            if is_hi:
                highs.append((i, bars[i][H]))
            if is_lo:
                lows.append((i, bars[i][L]))
        if highs:
            hi_lvl[t] = highs[-1]
        if lows:
            lo_lvl[t] = lows[-1]
        if len(highs) >= 2 and len(lows) >= 2:
            hh = highs[-1][1] > highs[-2][1]
            hl = lows[-1][1] > lows[-2][1]
            lh = highs[-1][1] < highs[-2][1]
            ll = lows[-1][1] < lows[-2][1]
            labels[t] = UP if (hh and hl) else (DOWN if (lh and ll) else FLAT)
    return labels, hi_lvl, lo_lvl


def check_walk(bars, n=2):
    a, _, _ = structure_walk(bars, n)
    b = fractal_structure(bars, n)
    bad = sum(1 for x, y in zip(a, b) if x != y)
    return bad, len(bars)


# ------------------------------------------------------------- 2. THE EVENTS
def structure_events(bars, n=2, carry_flat=False, strict=False):
    """Breaks, classified. Returns (events, breaks).

    `breaks[t]` is a set of directions broken at t, +1 for a close above the
    last confirmed swing high and -1 for a close below the last confirmed
    swing low, each level counted once (rule 5). It is kept separately from
    the classified events because the reversal measure asks whether the OLD
    direction broke again, which is a question about the break and not about
    what the label called it at that later bar.

    `events` are the classified ones: (index, side, kind, structure), with
    kind BOS when the side matches the prevailing structure and CHoCH when it
    opposes. With `carry_flat` the prevailing structure is the last non-FLAT
    label instead of FLAT itself - measured as a sensitivity, never as the
    primary, because it invents a direction the rule declined to give.
    """
    labels, hi_lvl, lo_lvl = structure_walk(bars, n, strict)
    events, breaks = [], [set() for _ in bars]
    spent_hi = spent_lo = None      # the swing bar index already broken
    last_dir = FLAT
    for t in range(len(bars)):
        c = bars[t][C]
        hi, lo = hi_lvl[t], lo_lvl[t]
        if hi is not None and hi[0] != spent_hi and c > hi[1]:
            breaks[t].add(+1)
            spent_hi = hi[0]
        if lo is not None and lo[0] != spent_lo and c < lo[1]:
            breaks[t].add(-1)
            spent_lo = lo[0]
        lab = labels[t]
        if lab != FLAT:
            last_dir = lab
        prevailing = lab if not carry_flat else (lab if lab != FLAT else last_dir)
        if prevailing == FLAT:
            continue
        want = +1 if prevailing == UP else -1
        for side in sorted(breaks[t]):
            kind = "BOS" if side == want else "CHoCH"
            events.append((t, side, kind, prevailing))
    return events, breaks, labels


# ------------------------------------------------------- 3. REVERSAL WITHIN N
def reversal_within(events, breaks, horizons=(3, 5, 10)):
    """Share of CHoCHs followed by a break in the OLD direction within N bars.

    This is the SMC claim's own failure mode written as a number: a change of
    character that changes nothing. The old direction is the prevailing
    structure AT the CHoCH, and the look-forward uses the raw break stream, so
    a break back through the old side counts whether or not the slow fractal
    label had caught up by then.
    """
    ch = [e for e in events if e[2] == "CHoCH"]
    out = {}
    for n in horizons:
        hit = 0
        for (t, side, _, prevailing) in ch:
            old = +1 if prevailing == UP else -1
            if any(old in breaks[u] for u in range(t + 1, min(t + 1 + n, len(breaks)))):
                hit += 1
        out[n] = (hit, len(ch))
    return out


# ------------------------------------------------------------ 4. LAG, MISSED
def choch_label_series(events, nbars):
    """An instantaneous series for `bias_ref.latency` to grade.

    latency() wants a per-bar label. A CHoCH is an instant, not a state, so
    the series reads UP or DOWN only on the bar the CHoCH printed and FLAT
    everywhere else. That keeps latency()'s arithmetic untouched and untouched
    is the point - the lag column below is produced by the same function that
    produced the bias study's lag column, so the two are comparable numbers
    and not two things with the same name. One consequence to read correctly:
    its `early` bucket here means the CHoCH printed on the pivot bar itself,
    not that some state was already leaning that way.
    """
    s = [FLAT] * nbars
    for (t, side, kind, _) in events:
        if kind != "CHoCH":
            continue
        s[t] = UP if side > 0 else DOWN
    return s


# ---------------------------------------------------------- 5. WHAT FOLLOWS
#
# Forward moves are taken over WALL-CLOCK hours, not a bar count, and the map
# comes from `bias_measures.forward_map`. This tape stops an hour a day and
# all weekend; "10 bars ahead" on H1 crosses the break and silently becomes
# thirteen hours. That is fault 11 on this desk's own list and the bias study
# already refused to pay it.
def follow_through(bars, idxs_sides, a, fwd):
    """Quartiles of the forward move in ATR units, SIGNED IN THE EVENT'S
    DIRECTION: positive means the market continued the way the break pointed.

    Median and quartiles, never a win rate and never a profit factor. A win
    rate needs a stop and a target and those are a strategy, which this is
    not.
    """
    xs = []
    for (i, side) in idxs_sides:
        j = fwd[i]
        if j is None or a[i] is None or a[i] <= 0:
            continue
        xs.append(side * (bars[j][C] - bars[i][C]) / a[i])
    if len(xs) < 20:
        return None
    xs.sort()
    q = lambda p: xs[min(len(xs) - 1, int(p * len(xs)))]
    # The standard error of a MEDIAN, 1.2533 * sd / sqrt(n), reported because
    # without it a reader compares "+0.31 after a BOS" against "-0.07 on any
    # bar" and cannot see that the gap is 1.4 SE in a table of eight medians -
    # under what the largest of thirty noise draws is expected to reach.
    # Closed form rather than a bootstrap, so this file stays deterministic
    # and needs no seed.
    se = 1.2533 * st.pstdev(xs) / (len(xs) ** 0.5)
    return {"n": len(xs), "p25": q(0.25), "med": st.median(xs), "p75": q(0.75), "se": se}


def baseline_moves(bars, a, fwd):
    """The same quantiles on every bar, unsigned by any event.

    Without it the event rows say nothing: "+0.31 ATR after a BOS" is only
    readable beside the -0.07 an arbitrary bar was followed by over the same
    window. A figure never appears on this desk without what qualifies it.
    """
    return follow_through(bars, [(i, +1) for i in range(len(bars))], a, fwd)


# ------------------------------------------------------------------- REPORT
def pct(x, n):
    return "-" if not n else "%.0f%%" % (100.0 * x / n)


def main():
    bars, src = load_bars("1h")
    a = atr(bars, 14)
    n_bars = len(bars)

    print("## Data\n")
    if src["kind"] == "fixture":
        print("`data/bars` is empty in this worktree (git-ignored, fetched not")
        print("committed), so this ran on the committed fixture slice.\n")
    print("| file | bars | first | last | bar sha256 (first 16) | file sha256 (first 16) |")
    print("|---|---|---|---|---|---|")
    print("| `%s` | %s | %sZ | %sZ | `%s` | `%s` |" % (
        src["path"], "{:,}".format(n_bars), bars[0][T].strftime("%Y-%m-%dT%H:%M:%S"),
        bars[-1][T].strftime("%Y-%m-%dT%H:%M:%S"), fingerprint_rows(bars)[:16],
        file_sha256(src["abs"])[:16] if src["kind"] == "fixture" else "n/a"))
    bad, tot = check_walk(bars, 2)
    print("\nSwing walk vs `bias_defs.fractal_structure(bars, 2)`: "
          "**%d label mismatches on %s bars**.\n" % (bad, "{:,}".format(tot)))
    assert bad == 0, "the swing walk is not the study's fractal(2) rule"

    events, breaks, labels = structure_events(bars, 2, carry_flat=False)
    ev_c, br_c, _ = structure_events(bars, 2, carry_flat=True)
    flat = sum(1 for v in labels if v == FLAT)

    # ------------------------------------------------------------- frequency
    print("## 1. Frequency\n")
    print("`fractal(2)` is FLAT on %s of %s bars (%s), and a FLAT bar yields no"
          % ("{:,}".format(flat), "{:,}".format(n_bars), pct(flat, n_bars)))
    print("event under the primary rule.\n")
    print("| rule | BOS | /100 bars | CHoCH | /100 bars | all breaks | /100 bars |")
    print("|---|---|---|---|---|---|---|")
    for name, evs in (("prevailing = fractal(2), FLAT yields nothing", events),
                      ("sensitivity: FLAT carries the last direction", ev_c)):
        b = sum(1 for e in evs if e[2] == "BOS")
        c = sum(1 for e in evs if e[2] == "CHoCH")
        allb = sum(len(s) for s in breaks)
        print("| %s | %d | %.1f | %d | %.1f | %d | %.1f |" % (
            name, b, 100.0 * b / n_bars, c, 100.0 * c / n_bars,
            allb, 100.0 * allb / n_bars))
    print()

    # -------------------------------------------------------- reversal within
    print("## 2. Reversal within N: the CHoCH that changed nothing\n")
    print("| rule | CHoCHs | old direction breaks again <=3 | <=5 | <=10 |")
    print("|---|---|---|---|---|")
    for name, evs, brs in (("primary", events, breaks),
                           ("carry-forward", ev_c, br_c)):
        r = reversal_within(evs, brs)
        cells = " | ".join("%s (%d)" % (pct(r[k][0], r[k][1]), r[k][0]) for k in (3, 5, 10))
        print("| %s | %d | %s |" % (name, r[3][1], cells))
    print()

    # ------------------------------------------------------------ lag, missed
    print("## 3. Lag at a 4xATR turn, and 4. the turns with no CHoCH at all\n")
    series = choch_label_series(events, n_bars)
    # The two STATE definitions are re-measured on this same slice rather than
    # quoted from the 2026-09-18 table, which was computed on 25,708 bars. A
    # lag of 5 against a lag of 12 read off two different samples is not a
    # comparison; read off the same 2,000 bars it is.
    rows = [("CHoCH (this note)", series),
            ("fractal(2) label", fractal_structure(bars, 2)),
            ("zigzag 3xATR label", atr_zigzag(bars, 3.0))]
    print("| k | definition | turns | one per N bars | already there | lag med | lag p90 | missed |")
    print("|---|---|---|---|---|---|---|---|")
    for k in (4.0, 8.0):
        turns = reference_turns(bars, k)
        for name, ser in rows:
            if not turns:
                print("| %gxATR | %s | 0 | - | - | - | - | - |" % (k, name))
                continue
            r = latency(ser, turns)
            print("| %gxATR | %s | %d | %.0f | %s | %s | %s | %s |" % (
                k, name, r["turns"], 1.0 * n_bars / len(turns),
                pct(r["early"], r["turns"]),
                "-" if r["median"] is None else "%.0f" % r["median"],
                "-" if r["p90"] is None else "%.0f" % r["p90"],
                pct(r["missed"], r["turns"])))
    print("\n*`lag med` is over the turns where the definition did agree before the")
    print("next turn; `missed` are the rest and are counted, never dropped. `already")
    print("there` means the definition read the new direction ON the pivot bar - for")
    print("a state label that is a lean it already had, for an instantaneous event it")
    print("is the event firing exactly on the pivot.*\n")

    # ------------------------------------------------------------ what follows
    print("## 5. What follows, in ATR units, signed in the event's direction\n")
    print("| horizon | after BOS (n) | p25 | med | p75 | after CHoCH (n) | p25 | med | p75 "
          "| any bar (n) | p25 | med | p75 |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for hours in (3, 5, 10, 24):
        fwd = forward_map(bars, hours, 0.6)
        b = follow_through(bars, [(e[0], e[1]) for e in events if e[2] == "BOS"], a, fwd)
        c = follow_through(bars, [(e[0], e[1]) for e in events if e[2] == "CHoCH"], a, fwd)
        z = baseline_moves(bars, a, fwd)
        cell = lambda r: ("- | - | - | -" if not r else
                          "%d | %+.2f | %+.2f | %+.2f" % (r["n"], r["p25"], r["med"], r["p75"]))
        print("| %dh | %s | %s | %s |" % (hours, cell(b), cell(c), cell(z)))
    print()
    print("| horizon | SE of the BOS median | SE of the CHoCH median | BOS med - any-bar med |")
    print("|---|---|---|---|")
    for hours in (3, 5, 10, 24):
        fwd = forward_map(bars, hours, 0.6)
        b = follow_through(bars, [(e[0], e[1]) for e in events if e[2] == "BOS"], a, fwd)
        c = follow_through(bars, [(e[0], e[1]) for e in events if e[2] == "CHoCH"], a, fwd)
        z = baseline_moves(bars, a, fwd)
        gap = "-" if not (b and z) else "%+.2f (%.1f SE)" % (
            b["med"] - z["med"], abs(b["med"] - z["med"]) / b["se"])
        print("| %dh | %s | %s | %s |" % (
            hours, "-" if not b else "%.2f" % b["se"],
            "-" if not c else "%.2f" % c["se"], gap))
    print()
    print("*`any bar` is the unqualified forward move on every bar, always long-signed:")
    print("the drift this tape had over the window, which is what the event columns")
    print("have to beat to mean anything. The SE columns are the closed form for a")
    print("median and they are the reason none of the gaps above is written up as a")
    print("finding.*\n")

    # ------------------------------------------------------ 6. THE TIE RULE
    #
    # NOT a sensitivity anyone asked for - a disagreement found while writing
    # this. `bias_defs.fractal_structure` admits a flat top as a swing (not
    # beaten by any neighbour, strictly beating at least one);
    # `htf.rs::fractal_swings` requires strictly greater than every
    # neighbour and says so in its doc comment. Every number above is the
    # bias_defs rule, because that is what the study measured. If the route
    # ships the htf.rs rule it ships these numbers about a slightly different
    # object, so the size of the gap belongs in the record.
    print("## 6. The two fractal tie rules this repository already contains\n")
    ev_s, br_s, lab_s = structure_events(bars, 2, carry_flat=False, strict=True)
    hs, hh, hl = structure_walk(bars, 2, strict=False)
    ss, sh, sl = structure_walk(bars, 2, strict=True)
    diff = sum(1 for x, y in zip(hs, ss) if x != y)
    print("| tie rule | source | swing highs | swing lows | labels differing | BOS | CHoCH |")
    print("|---|---|---|---|---|---|---|")
    for name, src_name, walk, evs, is_base in (
            ("not beaten, beats one", "`bias_defs` (every number above)", (hh, hl), events, True),
            ("strictly beats all", "`htf.rs::fractal_swings`", (sh, sl), ev_s, False)):
        nh = len(set(v[0] for v in walk[0] if v is not None))
        nl = len(set(v[0] for v in walk[1] if v is not None))
        print("| %s | %s | %d | %d | %s | %d | %d |" % (
            name, src_name, nh, nl,
            "-" if is_base else "%s (%s)" % (pct(diff, n_bars), "{:,}".format(diff)),
            sum(1 for e in evs if e[2] == "BOS"),
            sum(1 for e in evs if e[2] == "CHoCH")))
    print("\n*On THIS slice the two rules are the same object: an exact tie between a")
    print("bar's extreme and a neighbour's never occurs in 2,000 H1 bars of gold at")
    print("two decimals, so every count matches. That is a fact about these bars, not")
    print("a proof that the rules are equivalent - a tie is possible and the wider")
    print("file has twelve times as many chances to contain one. Re-run this section")
    print("with FD_BARS set before concluding the choice does not matter.*")


if __name__ == "__main__":
    main()
