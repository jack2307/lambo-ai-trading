# Gold rises across the New York close in every year since 2018 — by about the spread until 2023, and by a hundredth of a daily range since; the weekend leg passes seven years and cannot be confirmed on 68 Fridays

**Date:** 2026-09-13 (night)
**Question:** Held long from the last bars before the 17:00 New York close
through the reopen (and, in other rows, through the evening), does gold's
return beat sized random holds of the same length and its own flipped sides on
seven years of Dukascopy bars, and again on the broker's own bars since
2025-04 — with the Friday leg (a weekend hold) tested on its own? Registered
at `971f518`, amended once at `afb6670` (a gate minute no 15-minute bar
carries), both before any receipt (the adversary checked the timestamps).
`docs/hypotheses/2026-09-13-close-reopen-drift.md`.
**Outcome:** **Closed under its own falsifier; no paper run.** The weekday
break rows have the right sign at the 96th–100th percentile on both windows
and never pass the registered expectancy gate: the hold earns 0.6–1.2% of a
daily range per session against a gate of 5%, which for a two-hour hold is
a gate no result could pass — the registration wrote that unit down and it
stands. In money it is $132 a year on $10,000, and nothing at all before
2023. The Friday leg passes everything on 336 weekend holds (PF 2.06, 100th)
and clears neither null on the confirmation's 68 (95th of random holds, 91st
on direction); the confirmation had to agree and could not.

## What was measured

- Method: `session-hold`, long, entry signalled on the 16:15 New York bar
  (`hours:1615-1620`) and filled at the 16:30 open; exit signalled on the
  `to` bar and filled at the next open. Strategy-managed: no stop, no target,
  the clock is the exit. Sizing: 1% of $10,000 divided by one mean New
  York-day range over 20 days (`riskDailyRanges = 1`, `rangeDays = 20`), used
  for lots and R only (`crates/fd-strategy/src/session_hold.rs`,
  `tsmom::sizing_stop`). `weekdays:MoTuWeTh` for the five weekday rows;
  `weekdays:Fr` for the Friday leg, whose 18:15 exit is Sunday's.
- Primary: `xauduka` 15m, 2018-06-16 → 2025-04-10, 160,368 bars. Fixed
  replay (`fixed = true`, no grid — the test for a hold) against 300 sized
  random holds gated to the same hours, with the same length and count;
  walk-forward as the check; direction null = the same trades with sides
  permuted, 1,000 draws.
- Confirmation: `xauusd` 15m (Vantage), 2025-04-11 → 2026-09-11, 33,589 bars,
  the same instruments. Spread $0.28 flat (the reopen was measured at
  $0.26–0.27 from 4.6 M ticks, `py/ingest/mt5_spread_probe.py`); no swap
  (measured, the account is swap-free). The contract unit for both markets is
  one ounce (`config/default.toml`), so "lots" in the diagnostics are ounces.
- Receipts: `docs/research/runs/2026-09-13-close-reopen-drift/`
  (`in-sample-fixed.txt`, `in-sample.txt`, `direction-close-*.txt`,
  `out-of-sample-fixed.txt`, `out-of-sample.txt`, `direction-close-*-oos.txt`).
  Post hoc, at the review stage and at the adversary's request:
  `direction-close-1630-1815-pre-2023.txt`,
  `direction-close-fri-1630-1815-pre-screen.txt`, `diag-close-bounded.txt`.
  `diag-close-*.txt` are the trade-list diagnostic
  (`crates/fd-backtest/examples/diag_close.rs`): net dollars, concentration,
  worst holds. Not receipts, and — like every receipt — run with the guards
  off (`search` does not apply them; see the risk role).

## Evidence

Primary, fixed replay (`in-sample-fixed.txt`) with the direction percentile
from `direction-close-*.txt`:

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
close/1630-1815         1355   1.254   0.006    0.599    0.706  100%  100th       fail: expectancy 0.006R < 0.05R
close/1630-2000         1355   1.237   0.011    0.760    0.859  100%  100th       fail: expectancy 0.011R < 0.05R
close/1630-2200         1355   1.258   0.019    0.837    0.936  100%  100th       fail: expectancy 0.019R < 0.05R
close/1815-2200         1397   0.985  -0.001    0.830    0.936   99%   99th       fail: profit factor 0.985 < 1.2
close/1400-1630         1375   0.739  -0.017    0.784    0.913   21%   25th       fail: profit factor 0.739 < 1.2
close/fri-1630-1815      336   2.064   0.054    0.785    1.177  100%  100th       SURVIVES
```

Walk-forward (`in-sample.txt`): 1.288 / 1.234 / 1.292 / 1.061 / 0.826 / 2.300,
the Friday row 0.061R on 270 sessions, the same verdicts.

Confirmation, fixed replay (`out-of-sample-fixed.txt`, `direction-close-*-oos.txt`):

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
close/1630-1815          282   1.299   0.012    0.892    1.262   96%   96th       fail: expectancy 0.012R < 0.05R
close/1630-2000          282   1.637   0.039    0.934    1.250  100%  100th       fail: expectancy 0.039R < 0.05R
close/1630-2200          282   1.355   0.038    0.955    1.270   97%   98th       fail: expectancy 0.038R < 0.05R
close/1815-2200          289   1.195   0.022    0.986    1.307   88%   91st       fail: profit factor 1.195 < 1.2
close/1400-1630          289   0.955  -0.003    0.968    1.260   47%   52nd       fail: profit factor 0.955 < 1.2
close/fri-1630-1815       68   1.720   0.061    0.959    1.761   95%   91st       gate pass, inside the noise
```

Walk-forward (`out-of-sample.txt`): 1.400 (97%) / 1.800 (100%) / 1.420 (98%) /
1.254 (89%) / 0.962 (48%) / the Friday row 1.893 on 55 sessions, 0.069R, 94%.
Both files end "Nothing survived".

The two null columns are one test in two spellings, as the adversary showed:
for a fixed hold the sized random hold is gated to the same entry bar, holds
the same 120 minutes, and flips a coin for its side — the receipts' null
quantiles agree to the second decimal (0.599/0.706 against 0.603/0.698). The
record therefore counts **one null and two windows**, not two and three.

What the drift is worth (`diag-close-1630-1815.txt`, `diag-close-fri.txt`,
`diag-close-*-oos.txt`; ounces, $10,000 account, 1% per session):

```
row / window                    holds   net $   return   maxDD      top-5 share  PF w/o top-5   oz/yr   spread/yr   net/yr
1630-1815 Mon–Thu, 2018–25       1355     897    8.97%   $368 3.4%     52%          1.121        1,091    $305        $132
fri-1630-1815, 2018–25            336   1,975   19.75%   $151 1.3%     51%          1.520          281     $79        $290
1630-1815 Mon–Thu, 2025–26        282     329    3.29%   $153 1.5%     93%          1.021
fri-1630-1815, 2025–26             68     416    4.16%   $145 1.4%     91%          1.065
```

The bounded replays (`diag-close-bounded.txt`, the two post-hoc direction
files), which answer the adversary's falsifiable claim:

```
row                       window                  holds  PF     net $   direction null p95   actual → percentile
1630-1815 Mon–Thu         2018-06-16 → 2022-12-31   904  0.996    -10   0.684                0.996 → 100th
1630-1815 Mon–Thu         2023-01-01 → 2025-04-10   449  2.201    934   —                    —
fri-1630-1815             2018-06-16 → 2022-06-15   194  2.213  1,235   1.191                2.213 → 100th
fri-1630-1815             2022-06-16 → 2025-04-10   141  1.901    668   —                    —
```

Per year, the weekday break on the primary: 2018 0.82, 2019 1.10, 2020 1.31,
2021 0.90, 2022 0.64, 2023 2.57, 2024 2.24, 2025 (to April) 1.23. Before 2023
the row nets nothing on 904 sessions while its flipped self sits at 0.58: the
side is right every year and the $0.28 spread takes the whole of the move
while gold is $1,300–1,900. From 2023, at $2,000–3,000, the same move clears
the spread. The Friday row: every full year above 1.1, 2020 3.94, 2022 2.96,
2023 3.04; the 2025 stub 0.71; the years before the screen that nominated it
(2018-06 → 2022-06) 2.213 on 194 holds, 100th on direction.

The contrasts: the afternoon before the close (`1400-1630`) is negative on
both windows (25th and 52nd on direction), so the break rows are not "gold
went up"; the evening without the break (`1815-2200`) is flat on the primary
(0.985) and 1.195 on the confirmation, so what the break rows carry is the
close-to-reopen move, and the `1630-2000` row's 1.637 on the confirmation is
that move plus the year's Asian drift.

## What each role said

- **adversary:** SURVIVED, for the specific claim "closed, no paper run" —
  *"the registration predates every receipt … the gate was not moved, and I
  could not find a tradeable effect the pre-registered arithmetic is hiding."*
  Three corrections to the record's wording, all taken: the two nulls are
  one (*"`permuted_sides_pf` with a different RNG"*); the screen's 2022–25
  context is a subset of this primary, so *two* windows agree, not three; and
  the 0.05R gate *"was never testable by that gate"* for a hold whose whole
  excursion is 0.07R — *"honest reading: the break rows were never testable
  … not 'failed' by it. That does not hide a trade: net is $0.12/oz/session
  regardless of R."* Its falsifiable claim — *"replay `close/1630-1815` fixed
  with `--to=2022-12-31` and net will be within ±$50 of zero on ~900
  sessions"* — came back −$10 on 904. On the Friday leg: *"Power. Point
  estimates agree: 0.054R (336) vs 0.061R (68) … nothing below PF ~1.9 could
  pass"*; *"the Friday row, unlike the weekday row, pays in flat/down years
  (2021 PF 1.63, 2022 2.96), which is the one thing here that looks unlike
  'gold went up'."* What would change its mind on the Friday leg: Dukascopy
  2010-06 → 2018-06 (~400 Fridays), registered before the run, at the 95th of
  the direction null, plus the pre-screen years at the 95th. The second half
  is now on file at the 100th; the first needs data this repository does not
  have.
- **data-integrity:** NO OBJECTION. Independent replay of the rule reproduces
  the receipts (1,359 − 4 warm-up = 1,355; 285 − 3 = 282; 337 vs 336; PF
  1.250 / 1.324 / 2.091 / 1.732 against 1.254 / 1.299 / 2.064 / 1.720). Clocks
  correct on both feeds across DST (entries at 20:30Z in summer, 21:30Z in
  winter, 100% of trades); *"both feeds have 0 bars in the 17:xx bucket"*, so
  Dukascopy and Vantage are the same instrument at these hours; coverage
  95.5% / 96.0% of weekdays. Caveats for the reading: eleven of the 1,355
  "weekday" holds are 26–74-hour holiday holds (Good Friday, New Year) and
  carry 15% of the row's R (PF 1.215 without them — still above 1.2, still
  under the gate); the confirmation's 68 is 67 weekend holds plus an
  end-of-data stub; the sizing range counts Sundays as days, so R is 9% (6.5%)
  too small and lots that much too large, uniformly, ranks unaffected; the
  confirmation is *"a time-split, not an independent instrument"*, in a regime
  with 3.7× the daily range; and *"'100th percentile on 1,355 sessions' is a
  statement about two of seven years."*
- **risk:** BLOCK on any promotion toward a paper run, at any size. The paper
  boundary holds (no order-sending code anywhere; the MT5 scripts are
  read-only). The guards are now enforced in the engine (`guards.rs`,
  `engine.rs`, tests) but only the API passes them, off by default; *"every
  receipt behind this hypothesis was produced with guards off"* — and the
  module comment that said otherwise has been corrected (`14c321e`). What the
  guards cannot see: *"the daily-loss limit sums closed trades … an open
  position's unrealised loss is invisible, and nothing can close a position"*;
  no duration cap, no weekend flag, so *"a 50-hour weekend hold is not
  representable under these guards."* Tail: worst weekday hold −0.50R; worst
  Friday −1.15R / −$119 on 2026-01-30 (a 3.0% adverse weekend, at 1% risk);
  spread 2.3× net profit on the primary. *"Raising size is the only lever — a
  fixed clock hold has nothing else"*: 10%/year from the weekday row needs
  ~7.6× (≈ $100k notional unstopped across the close, maxDD ≈ 28%, 2022 ≈
  −15%); from the Friday row ~3.45× (5× leverage every weekend; the
  2026-01-30 weekend ≈ −4%, a 6% gap ≈ −12%). *"The registered size looks
  safe only because it is too small to earn."* Before anything further: a
  paper loop that exists in code, an unrealised-loss cap and a maximum hold
  enforced for self-managed positions with a test, a weekend guard, a notional
  cap, receipts re-run with guards on, and PF without the top five ≥ 1.2
  (currently 1.02–1.12).

## Reading

The claim was right in sign and wrong in size, and the record has to be
precise about which years. The side is right in every year since 2018 — the
flipped-sides null sits at the 58th–60th of a hundred percent while the row
sits at the 100th, pre-2023 and post — but a fixed $0.28 spread took the
entire move while gold traded at $1,300–1,900, and only from 2023, at
$2,000–5,000, does the same proportional move clear it. That is a reading,
not a receipt; what the receipts say is: net zero on 904 sessions to 2022,
PF 2.2 on 449 from 2023, PF 1.30 on 282 since 2025-04. Even where it clears
the spread, a session is worth $0.10–0.20 an ounce, a year at 1% risk per
session returns 1–4% on the account, and changing the risk unit would change
the R figures and not one dollar of that. The only way the drift pays is a
larger unstopped position through a closed market, which is what the risk
role refused, with the arithmetic.

The Friday leg is the stronger effect and the smaller sample: it pays in the
flat and falling years the weekday row does not, it is at the 100th on the
years before the screen nominated it, and its confirmation miss on 67 weekend
holds is a power problem (nothing under PF 1.9 could have passed) rather than
a disagreement. The registration said the confirmation must agree; it did
not; the row is closed and the way back in is written below.

## What would reopen this

- **The Friday leg**: a window it did not choose and that exists now —
  Dukascopy XAUUSD 2010-06 → 2018-06, about 400 Fridays, fetched and
  registered before any run, the registered parameters replayed, at or above
  the 95th percentile of the direction null. That, with the pre-screen years
  already at the 100th, reopens it for a paper-run proposal — which then
  meets the risk role's list (weekend guard, unrealised-loss cap, maximum
  hold, notional cap, all enforced and tested) before a proposal is written.
  Nothing else: not a wider window, not a different exit, not the next two
  years of Vantage Fridays (68 could not decide; 100 more will not either).
- **The weekday break**: nothing within this program. A mechanism that turns
  a $0.10–0.20-an-ounce drift into a trade with a stop and a size would be a
  new registration, and it would have to beat the spread that took all of it
  for five of the seven years.

## What this does not say

- It does not say the close-to-reopen drift is noise. Its sign is at the
  96th–100th percentile on both windows and in the pre-2023 years alone; the
  record closes the *trade*, not the observation.
- It does not say the 0.05R gate was wrong, or that it should be re-tuned. R
  was one daily range by the registration's choice; a two-hour hold cannot
  earn 5% of a day, and the record says so instead of moving the number.
- It does not say the Friday leg failed. It says 67 weekend holds could not
  confirm 336, and the rule is that the confirmation must.
- It does not say the drift is proportional to price; that is the reading
  that fits net-zero-then-positive, and no receipt tests it.
- It does not price a weekend gap for an unstopped position; the fill model
  exits at the Sunday 18:30 open, and the worst observed Sunday was −1.15R at
  1% risk. A larger size makes that number larger in proportion, with no stop
  between Friday and Sunday.
- Every number above is unguarded: the research runner does not apply the
  configured guards, and this record is the first to say so.

## Amendment (2026-09-13, later the same night): the spread reading is wrong

`2026-09-13-friday-weekend-hold` put the weekday break row on Dukascopy
2010-06 → 2018-06 as a prediction of the Reading above: side right, net near
zero. The side came in right (100th on direction, 1,621 sessions) and the
net did not: −$1,459, PF 0.733, negative in every year but 2013. The mean
move per ounce was $0.13 there against about $0.28 on 2018–22 (net −$10 on
904 sessions) — 2.2× smaller at a price 1.15× lower — and the Friday hold's
$0.25 an ounce on 2010–18 against $1.31 on 2018–25, 5.2× smaller at 1.4×
lower. The drift is not proportional to price; "a fixed spread eating a
proportional move" does not describe it. What the receipts support is only
what they said: the side is right in every year since 2010, the size of
the move varies by period and is below the spread in most of them, and
2023–2025 is the period it was not. "In every year since 2018" in the title
stands; the sentence in Reading beginning "The side is right in every year
since 2018 … only from 2023, at $2,000–5,000, does the same proportional
move clear it" is withdrawn as an explanation. Nothing in the verdict
changes: closed, no paper run.

## Amendment (2026-09-14): the sized-null column was computed with a late-exiting control

The random-hold control in every receipt above signalled its exit one bar
after the method's (`2026-09-13-instrument-faults.md`, addendum item 3):
on the 16:30 → 18:30 rows the null held to 18:45, fifteen minutes longer;
on the Friday row it held to the Sunday 18:15 fill, fifteen minutes less.
The direction null — the method's own trades with sides permuted — is
unaffected and put every row where the sized column did (100th / 100th /
100th / 99th / 25th / 100th on the primary). Nothing in the verdict moves.
Fixed at `4eaef94`.

## Addendum (2026-09-14, afternoon): the same rows with the guards on — the first guarded receipts

The guards the risk role asked for now exist in the engine and apply to
self-managed holds (`675b5ef`); `runs/2026-09-14-close-reopen-guarded/`
is this record's two rows re-run with them on (context, decides nothing):

```
row                    trades  OOS PF  expect   pct  direction   guards
close/1630-1815          1355   1.274   0.006  100%   100th      nothing acted
close/fri-1630-1815       336   0.827  -0.004  100%    99th      closed WEEKEND_FLAT 336
```

The weekday break row is untouched — no release falls in 16:30–18:30, the
open-loss cap at 2R is never reached by a two-hour hold. The Friday leg
cannot exist under the guards: every one of its 336 holds is flattened at
the 16:45 bar's close (17:00 New York) by the weekend guard, and what is
left is a thirty-minute hold at PF 0.83. That is the guard doing what it
was written to do, and the honest sentence for the paper phase: the row
this record could not confirm on 68 Fridays is one the paper bot is not
allowed to hold.
