# The reference turns, rewritten after the first version was found broken.
#
# WHAT WAS WRONG, recorded because the symptom was a plausible-looking table.
# The first version initialised `direction = 0` and then ran two unguarded
# `if`s - one tracking highs while direction >= 0, one tracking lows while
# direction <= 0. At direction 0 BOTH fired every bar, so the running extreme
# became that bar's own low and the only reversal that could ever trigger was
# a single bar whose range exceeded the threshold. The first pivot it found
# was bar 21,918 of 25,708 - the 2026-01-28 crash - and the 21,917 bars
# before it produced nothing. It also let a bar that EXTENDED the extreme
# also reverse it, which produced pivots at the same index in both directions
# and legs of zero bars.
#
# The table it printed looked ordinary: latency 0 almost everywhere, 60% of
# turns "missed". That reads as "the definitions are instant but unreliable"
# and was entirely an artefact.
import statistics as st

from bias_defs import UP, DOWN, FLAT, H, L, C, atr


def reference_turns(bars, k=8.0, n=14):
    """Hindsight zigzag. Returns (pivot_index, direction_AFTER_the_pivot).

    THE THRESHOLD IS k x ATR AT THE BAR, not k x a fixed median. Gold ran
    from 1,620 to 5,600 across this sample; a constant point threshold is
    7.5% of price at the start and 2.2% at the end, so it would mark a
    different kind of event in 2022 than in 2026 and every per-year column
    would be measuring the threshold rather than the market.

    Acausal on purpose: this is the ground truth a chart reader points at
    afterwards. Nothing causal is graded against its own output.
    """
    a = atr(bars, n)
    direction = 0
    start_i = next((i for i, v in enumerate(a) if v is not None), 0)
    ext_i, ext_p = start_i, bars[start_i][C]
    piv = []
    for i in range(start_i, len(bars)):
        if a[i] is None:
            continue
        thr = k * a[i]
        b = bars[i]
        if direction == 0:
            if b[H] - ext_p > thr:
                direction, ext_i, ext_p = +1, i, b[H]
            elif ext_p - b[L] > thr:
                direction, ext_i, ext_p = -1, i, b[L]
            continue
        if direction == +1:
            if b[H] > ext_p:                      # elif below: extending the
                ext_i, ext_p = i, b[H]            # extreme cannot also
            elif ext_p - b[L] > thr:              # reverse it on one bar
                piv.append((ext_i, -1))
                direction, ext_i, ext_p = -1, i, b[L]
        else:
            if b[L] < ext_p:
                ext_i, ext_p = i, b[L]
            elif b[H] - ext_p > thr:
                piv.append((ext_i, +1))
                direction, ext_i, ext_p = +1, i, b[H]
    return piv


def latency(labels, turns):
    """How long each definition takes to agree with a turn, decomposed.

    Three outcomes per turn, and all three are reported, because collapsing
    them hides the trade-off the owner is actually choosing between:

      early   the label ALREADY read the new direction at the pivot. It did
              not need to flip. Not free - it means the definition was on
              that side while the market was still going the other way,
              which is the same property that makes it whipsaw.
      lag     bars from the pivot until the label first reads the new
              direction. Only these go into the median.
      missed  never agreed before the NEXT turn. Counted, never dropped:
              dropping them flatters a slow definition by deleting the turns
              it slept through entirely.
    """
    want = {+1: UP, -1: DOWN}
    lags, early, missed = [], 0, 0
    for j, (i, d) in enumerate(turns):
        limit = turns[j + 1][0] if j + 1 < len(turns) else len(labels)
        if i >= len(labels):
            continue
        if labels[i] == want[d]:
            early += 1
            continue
        hit = None
        for t in range(i + 1, min(limit, len(labels))):
            if labels[t] == want[d]:
                hit = t - i
                break
        if hit is None:
            missed += 1
        else:
            lags.append(hit)
    return {"lags": lags, "early": early, "missed": missed, "turns": len(turns),
            "median": st.median(lags) if lags else None,
            "p90": sorted(lags)[int(0.9 * len(lags))] if lags else None}
