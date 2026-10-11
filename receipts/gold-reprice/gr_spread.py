"""Read data/spreads/XAUUSD_sc.csv and print what the engine actually charges.

Reads only; prints only; writes nothing. Two things it establishes:

  1. The distribution of the LOGGED quantity, which is `ask - bid` -- the same
     quantity `rules.spread` is (see RESULT.md section 1).
  2. The effective round-trip cost of a ROW, which is NOT the all-hours p50:
     `engine::apply_costs` charges half the spread at the fill and half at the
     close, so a row's cost is the mean of its ENTRY hour's spread and its EXIT
     hour's spread. Hour filters are read on the NEW YORK clock
     (`fd-strategy/src/filter.rs::hours_are_read_on_the_new_york_clock`), and
     the log covers September, when New York is UTC-4.
"""
import csv, datetime as dt, statistics as st, sys
from collections import defaultdict

PATH = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/flowdesk/data/spreads/XAUUSD_sc.csv"
rows = list(csv.DictReader(open(PATH)))
ts = [dt.datetime.strptime(r["time_utc"], "%Y-%m-%dT%H:%M:%SZ") for r in rows]
sp = [float(r["spread"]) for r in rows]

def quant(v, p):
    v = sorted(v); k = (len(v) - 1) * p; f = int(k)
    return v[f] if f == len(v) - 1 else v[f] + (k - f) * (v[f + 1] - v[f])

print(f"n = {len(sp)}   span {ts[0]} -> {ts[-1]} UTC")
print("days covered:", sorted({(t.strftime('%Y-%m-%d'), t.strftime('%a')) for t in ts}))
print("min %.4f p10 %.4f p50 %.4f p90 %.4f p95 %.4f p99 %.4f max %.4f mean %.5f"
      % (min(sp), quant(sp, .10), quant(sp, .50), quant(sp, .90),
         quant(sp, .95), quant(sp, .99), max(sp), st.mean(sp)))
over = [(t, v) for t, v in zip(ts, sp) if v > 0.28]
print("samples strictly > 0.28: %d = %.2f%%, in UTC hours %s"
      % (len(over), 100 * len(over) / len(sp), sorted({t.hour for t, _ in over})))

by_ny = defaultdict(list)
for t, v in zip(ts, sp):
    by_ny[(t.hour - 4) % 24].append(v)      # September -> EDT -> UTC-4
p50 = {h: quant(v, .50) for h, v in by_ny.items()}

print("\nNY hr (UTC)      n     p50     mean     max   flat-0.220 vs this hour")
for h in sorted(by_ny):
    v = by_ny[h]
    print("%02d:00  (%02d)   %5d  %.4f  %.5f  %.4f   %+6.1f%%%s"
          % (h, (h + 4) % 24, len(v), p50[h], st.mean(v), max(v),
             100 * (0.220 - p50[h]) / p50[h], "   UNDERCHARGES" if p50[h] > 0.220 else ""))

print("\neffective round trip = (p50[entry NY hour] + p50[exit NY hour]) / 2")
for name, a, b in [("ovn1900/long: entry 19:00 NY, exit 02:00 NY", 19, 2),
                   ("16:00 -> 18:00 NY (the ADDENDUM-10 IV band)", 16, 18),
                   ("both legs at 17:00 NY (the CME rollover)", 17, 17)]:
    e = (p50[a] + p50[b]) / 2
    print("  %-44s %.4f   flat-0.220 %+.1f%%, flat-0.28 %+.1f%%"
          % (name, e, 100 * (0.220 - e) / e, 100 * (0.28 - e) / e))
print("\nclock-uniform mean = %.5f  =>  0.220 is %+.2f%% off a clock-uniform row"
      % (st.mean(sp), 100 * (0.220 - st.mean(sp)) / st.mean(sp)))
