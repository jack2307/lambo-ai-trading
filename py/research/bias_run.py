# Run the bias study and print the tables the decision note pastes.
#   python bias_run.py h1|h4|anchor|case|year <tf> <defs...>
import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bias_defs import UP, DOWN, FLAT, T, C, all_definitions, resample
from bias_measures import V, load, whipsaw, forward_map, information, by_year
from bias_ref import reference_turns, latency

ORDER = ["fractal(1)", "fractal(2)", "fractal(3)", "zigzag 1.5xATR", "zigzag 3xATR",
         "dow closes(2)", "ema 21/55", "ema 20/50/200", "adx20 +DI", "adx25 +DI",
         "donchian20", "donchian55", "supertrend(10,3)", "avwap day", "avwap week",
         "pdmid day", "pdmid week"]


def mix(lab):
    n = len(lab)
    u = sum(1 for v in lab if v == UP)
    d = sum(1 for v in lab if v == DOWN)
    return f"{u/n*100:.0f}/{d/n*100:.0f}/{(n-u-d)/n*100:.0f}"


def table(tf):
    bars = load(os.path.join(V, f"XAUUSD-{tf}.parquet"))
    hours = 1 if tf == "1h" else 4
    print(f"### {tf.upper()} — {len(bars):,} bars, {bars[0][T]} .. {bars[-1][T]}\n")
    t4 = reference_turns(bars, 4.0)
    t8 = reference_turns(bars, 8.0)
    print(f"Reference turns: {len(t4)} at 4xATR (one per {len(bars)/len(t4):.0f} bars, "
          f"median swing {median_bp(bars, t4):.0f} bp) and "
          f"{len(t8)} at 8xATR (one per {len(bars)/len(t8):.0f} bars, "
          f"median swing {median_bp(bars, t8):.0f} bp).\n")

    fwd_a = forward_map(bars, 4, hours * 0.6)
    fwd_b = forward_map(bars, 24, hours * 0.6)
    defs = all_definitions(bars)

    print("| definition | U/D/F % | flips /100 | undone <=3 | lag med (4xATR) | early | missed "
          "| lag med (8xATR) | hit 4h | vs null | hit 24h | vs null | n |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for name in ORDER:
        lab = defs[name]
        per100, frac, _ = whipsaw(lab)
        L4 = latency(lab, t4)
        L8 = latency(lab, t8)
        ia, ib = information(bars, lab, fwd_a), information(bars, lab, fwd_b)
        g = lambda r, key: "-" if not r else (f"{r['rate']*100:.1f}%" if key == "r"
                                              else f"{r['rate']*100-r['null_rate']*100:+.1f}")
        m4 = f"{L4['median']:.0f}" if L4["median"] is not None else "-"
        m8 = f"{L8['median']:.0f}" if L8["median"] is not None else "-"
        print(f"| {name} | {mix(lab)} | {per100:.1f} | {frac*100:.0f}% | {m4} "
              f"| {L4['early']/L4['turns']*100:.0f}% | {L4['missed']/L4['turns']*100:.0f}% | {m8} "
              f"| {g(ia,'r')} | {g(ia,'d')} | {g(ib,'r')} | {g(ib,'d')} | {ib['n'] if ib else 0} |")


def median_bp(bars, turns):
    mv = [abs(bars[turns[i+1][0]][C] - bars[turns[i][0]][C]) / bars[turns[i][0]][C] * 10000
          for i in range(len(turns) - 1)]
    return st.median(mv) if mv else 0


def yearly(tf, picks):
    bars = load(os.path.join(V, f"XAUUSD-{tf}.parquet"))
    hours = 1 if tf == "1h" else 4
    fwd = forward_map(bars, 24, hours * 0.6)
    defs = all_definitions(bars)
    years = sorted({b[T].year for b in bars})
    print(f"\n### {tf.upper()} — 24h hit rate by year, non-overlapping; "
          f"(points above the circular-shift null, n)\n")
    print("| definition | " + " | ".join(str(y) for y in years) + " |")
    print("|---" * (len(years) + 1) + "|")
    for name in picks:
        res = by_year(bars, defs[name], fwd)
        cells = []
        for y in years:
            r = res.get(y)
            cells.append("-" if not r else
                         f"{r['rate']*100:.0f}% ({r['rate']*100-r['null_rate']*100:+.0f}, {r['n']})")
        print(f"| {name} | " + " | ".join(cells) + " |")


def anchor():
    m15 = load(os.path.join(V, "XAUUSD-15m.parquet"))
    print(f"Both anchors aggregated from ONE 15m file: {len(m15):,} bars "
          f"{m15[0][T]} .. {m15[-1][T]}\n")
    for hours, tf in ((1, "H1"), (4, "H4")):
        b_brk = resample(m15, hours, epoch_anchor=False)
        b_epo = resample(m15, hours, epoch_anchor=True)
        # H4 on the two anchors NEVER shares a timestamp: broker candles open
        # 21/01/05/09/13/17Z and epoch ones 00/04/08/12/16/20Z. Matching on
        # the stamp therefore compares nothing. The honest question is what a
        # card would have SHOWN at one instant, so both series are sampled on
        # a common clock: at each broker bar's close, take the label each
        # series had in force from its own most recently CLOSED bar.
        import datetime as dt
        step = dt.timedelta(hours=hours)
        idx_e = {b[T]: i for i, b in enumerate(b_epo)}
        shared = [(i, idx_e[b_brk[i][T]]) for i in range(len(b_brk)) if b_brk[i][T] in idx_e]
        if shared:
            ident = all(abs(b_brk[i][C] - b_epo[j][C]) < 1e-9 and
                        abs(b_brk[i][2] - b_epo[j][2]) < 1e-9 for i, j in shared)
            print(f"### {tf}: broker {len(b_brk):,} bars, epoch {len(b_epo):,}, "
                  f"{len(shared):,} share a timestamp - OHLC on those "
                  f"{'IDENTICAL' if ident else 'DIFFER'}\n")
        else:
            print(f"### {tf}: broker {len(b_brk):,} bars, epoch {len(b_epo):,}, "
                  f"**no timestamp is shared at all** - the anchors build different "
                  f"candles, so each is sampled at the other's bar closes\n")
            et = [b[T] for b in b_epo]
            j = 0
            for i in range(len(b_brk)):
                close_t = b_brk[i][T] + step
                while j + 1 < len(et) and et[j + 1] + step <= close_t:
                    j += 1
                if et[j] + step <= close_t:
                    shared.append((i, j))
        d_brk = all_definitions(b_brk, epoch_anchor=False)
        d_epo = all_definitions(b_epo, epoch_anchor=True)
        print("| definition | labels differing |")
        print("|---|---|")
        for name in ORDER:
            diff = sum(1 for i, j in shared if d_brk[name][i] != d_epo[name][j])
            print(f"| {name} | {diff/len(shared)*100:.1f}% |")
        print()


def case():
    import datetime as dt
    bars = load(os.path.join(V, "XAUUSD-1h.parquet"))
    defs = all_definitions(bars)
    lo, hi = dt.datetime(2026, 9, 18, 0), dt.datetime(2026, 9, 18, 8)
    idx = [i for i, b in enumerate(bars) if lo <= b[T] <= hi]
    pre = [i for i, b in enumerate(bars) if dt.datetime(2026, 9, 17, 12) <= b[T] < lo]
    print(f"### 2026-09-18 on H1 — {bars[idx[0]][T]} .. {bars[idx[-1]][T]}\n")
    print("| bar | O | H | L | C |")
    print("|---|---|---|---|---|")
    for i in pre[-4:] + idx:
        mark = "" if i in idx else " *(prior day)*"
        print(f"| {bars[i][T]:%m-%d %H}Z{mark} | {bars[i][1]:.2f} | {bars[i][2]:.2f} "
              f"| {bars[i][3]:.2f} | {bars[i][C]:.2f} |")
    op, cl = bars[idx[0]][1], bars[idx[-1]][C]
    print(f"\nWindow open {op:.2f} to close {cl:.2f} = **{cl-op:+.2f}** "
          f"({(cl-op)/op*10000:+.0f} bp).\n")
    print("| definition | " + " | ".join(f"{bars[i][T]:%H}Z" for i in idx) + " |")
    print("|---" * (len(idx) + 1) + "|")
    for name in ORDER:
        cells = ["UP" if defs[name][i] == UP else ("**DOWN**" if defs[name][i] == DOWN else "flat")
                 for i in idx]
        print(f"| {name} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "h1"
    if what in ("h1", "h4"):
        table("1h" if what == "h1" else "4h")
    elif what == "year":
        yearly(sys.argv[2], sys.argv[3:])
    elif what == "anchor":
        anchor()
    elif what == "case":
        case()
