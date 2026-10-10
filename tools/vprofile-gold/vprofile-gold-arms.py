"""Two readings of the 72 cells that the table does not give:

1. Did the VOLUME arms beat the committed TIME control at the gate, on matched
   (window, guard arm, window length, threshold)? That is what the 24 control
   cells were declared for.
2. Which way does `PF_usd` read against `PF_r` on 72 cells that are all losing
   books? ADDENDUM-7 section A says the sign of the shift follows the sign of
   the book; ADDENDUM-8 section III.1 says it does not and `PF_usd` reads high
   in 71% of 452 cells regardless. 72 losing cells is a direct test.
"""
import re
import sys

PAT = re.compile(r"^(W\d)\s+(off|ON)\s+(\w+)/d(\d+)/t([\d.]+)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+([-+][\d.]+)")
cells = {}
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    m = PAT.match(line)
    if m:
        w, g, meas, d, t, n, pf_usd, pf_r, e = m.groups()
        cells[(w, g, d, t, meas)] = (int(n), float(pf_usd), float(pf_r), float(e))

print(f"{len(cells)} cells parsed")
print()
print("1. VOLUME arm minus TIME control, matched on (window, guards, windowDays, threshold):")
print(f"   {'win':<4} {'grd':<4} {'days':<5} {'thr':<5} {'arm':<5} {'PF_r vol':>9} {'PF_r time':>10} {'delta':>8} {'E vol':>9} {'E time':>9} {'dE':>9}")
tot = {"tvol": [], "tvsp": []}
for (w, g, d, t, meas), v in sorted(cells.items()):
    if meas == "time":
        continue
    base = cells.get((w, g, d, t, "time"))
    if not base:
        continue
    dpf, de = v[2] - base[2], v[3] - base[3]
    tot[meas].append((dpf, de))
    print(
        f"   {w:<4} {g:<4} {d:<5} {t:<5} {meas:<5} {v[2]:>9.4f} {base[2]:>10.4f} {dpf:>+8.4f} "
        f"{v[3]:>+9.4f} {base[3]:>+9.4f} {de:>+9.4f}"
    )
print()
for meas, label in [("tvol", "TICK_VOLUME / PER_TOUCHED_BUCKET"), ("tvsp", "TICK_VOLUME / SPREAD")]:
    xs = tot[meas]
    better = sum(1 for dpf, _ in xs if dpf > 0)
    mean_dpf = sum(dpf for dpf, _ in xs) / len(xs)
    mean_de = sum(de for _, de in xs) / len(xs)
    print(
        f"   {label}: beat the time control on PF_r in {better}/{len(xs)} matched pairs; "
        f"mean dPF_r {mean_dpf:+.4f}, mean dE {mean_de:+.4f} R"
    )
    # Split by regime, because that is this job's whole question.
    for w, lab in [("W1", "W1 = regime A"), ("W2", "W2 = regime B")]:
        sub = [
            (v[2] - cells[(w, g, d, t, "time")][2], v[3] - cells[(w, g, d, t, "time")][3])
            for (ww, g, d, t, mm), v in cells.items()
            if mm == meas and ww == w and (w, g, d, t, "time") in cells
        ]
        b = sum(1 for dpf, _ in sub if dpf > 0)
        print(
            f"      {lab}: beat control in {b}/{len(sub)}; mean dPF_r "
            f"{sum(x for x, _ in sub)/len(sub):+.4f}, mean dE {sum(y for _, y in sub)/len(sub):+.4f} R"
        )

print()
print("2. PF_usd vs PF_r on 72 cells that are ALL losing books (every E < 0):")
higher = sum(1 for v in cells.values() if v[1] > v[2])
gaps = sorted(v[1] - v[2] for v in cells.values())
print(f"   PF_usd reads HIGHER than PF_r in {higher}/{len(cells)} cells ({100*higher/len(cells):.1f}%)")
print(f"   gap PF_usd - PF_r: min {gaps[0]:+.4f}, median {gaps[len(gaps)//2]:+.4f}, max {gaps[-1]:+.4f}")
for g, lab in [("off", "guards off"), ("ON", "guards ON")]:
    sub = [v for k, v in cells.items() if k[1] == g]
    h = sum(1 for v in sub if v[1] > v[2])
    print(
        f"   {lab}: PF_usd higher in {h}/{len(sub)}; mean gap "
        f"{sum(v[1]-v[2] for v in sub)/len(sub):+.4f}"
    )
print("   every one of these 72 books LOSES money (max PF_r = "
      f"{max(v[2] for v in cells.values()):.4f}), so a rule that ties the gap's sign to the")
print("   book's sign predicts ONE direction for all 72.")
