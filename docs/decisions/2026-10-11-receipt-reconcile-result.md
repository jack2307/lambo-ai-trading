# RESULT — 28 of the 34 unreproducible decision-record rows reproduce EXACTLY once the thing that moved is pinned back, and not one of them is a unit mistake

**Date:** 2026-10-11
**Registration:** `docs/decisions/2026-10-11-receipt-reconcile.md`, committed
alone at `3097e8a` before the first engine run. The classification table was
declared there.
**Branch:** `agent/receipt-reconcile` (cut from `agent/stop-width`, `29c8635`).
**Binary:** `/e/rust/fd-stop-width/target-sw/release/search.exe`, built
2026-10-08 20:57 from `agent/stop-width` with the `lbar_line` patch. **Nothing
was built on this axis.**
**Receipt:** `receipts/receipt-reconcile/TABLE-34-rows.txt`, plus the raw
control runs in `receipts/receipt-reconcile/controls/`.
**NO GATE.** Nothing here passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`. No
gate cell was spent.

## The count, re-derived and not inherited

    vanished band keys rebuilt from agent/pfr-audit's own artifacts      51
    of them sitting in docs/decisions/                                  34
    distinct decision docs they sit in                                   9

**34, independently.** `py/research/receipt_reconcile_rows.py` rebuilt the set
from `gate_legs_rows.json` (6,589 rows carrying both a PF and an expectancy),
`cmd_of.json`, `repro_band.json` and all 88 completed `pfr-audit` runs, pairing
on **the same label AND the same trade count**. The number did not need to be
forced; it came out 51/34 on the criterion stated in the registration.

## THE ANSWER

    rows whose PUBLISHED TRADE COUNT returns EXACTLY once the
      thing that moved is pinned back                            28 of 34
    of those, PF also returns to printed width                   20 of 28
    rows that are a PF_usd-published-as-PF_r unit mistake          0 of 34
    rows that are UNREADABLE (`KHÔNG ĐỌC ĐƯỢC`)                    0 of 34
    rows not fully adjudicated, recorded as OPEN                   6 of 34

**The record is not contaminated at the measurement layer and it is not
contaminated at the reporting layer either. It is contaminated at the
SPECIFICATION layer: an argv with an open-ended window is not a reproducible
specification of a run, and 25 of the 34 rows were published from one.**

## Where the 34 rows sit, and where the correction was appended

    decision record                                 rows   note appended
    2026-09-13-close-reopen-drift.md                   6   yes (+91 lines, 0 deleted)
    2026-09-13-london-range.md                         6   yes (+41, 0)
    2026-09-13-m15-check.md                            5   yes (+50, 0)
    2026-09-13-orb-ny.md                               4   yes (+49, 0)
    2026-09-13-volcond-breakout.md                     4   yes (+36, 0)
    2026-09-13-london-fix.md                           3   yes (+29, 0)
    2026-09-13-vwap-fade.md                            3   yes (+47, 0)
    2026-09-13-pdhl.md                                 2   yes (+34, 0)
    2026-09-13-friday-weekend-hold.md                  1   yes (+36, 0)
                                                      --
                                                      34

**Every one of the nine diffs is insertions with ZERO deletions**, and no file
under `docs/research/runs/` was touched. The correction is an appended dated
note in each record, as brief 5 requires; nothing above a note was rewritten.

## The classes, with the count and the positive control each one passed

    class  cause                                                rows  exact
    W1     the store was BACKFILLED under an omitted `--from`      16   16
    W1+C1  W1, plus the Dukascopy flat-bar cleaning in-window       3    2
    W2     the store GREW AT THE LIVE END under an omitted `--to`   9    5
    P1     a preset that did not pin a swept grid axis              4    4
    G1     the weekend guard's cut-off moved, 1655 -> 1640          1    1
    E1     one trade at a pinned window edge                        1    0
                                                                   --   --
                                                                   34   28

### W1 — 16 rows. The store grew a 12-year tail under them, and the argv never said when to start.

Every one of these 16 commands omits `--from=`. The published receipts say what
the store held at the time; the re-runs say what it holds now:

    XAUDUKA-5m   published  198,716 bars from 2022-06-16  ->  now 1,135,389 from 2010-06-01
    XAUDUKA-15m  published   66,243 bars from 2022-06-16  ->  now   352,261 from 2010-06-01

That is 5.31x and 5.32x more bars, and the trade counts moved by the same
factor (`fix/pm-after` 578 -> 3,078 = 5.33x; `m15/pdhl-fade` 531 -> 2,655 =
5.00x). **Pin `--from=2022-06-16` and all 16 come back exactly**, 14 of them
with the published `PF_usd` to printed width as well:

    fix/am-after      578  0.811 -> 578  0.811      m15/london        501  0.886 -> 501  0.886
    fix/pm-after      578  0.904 -> 578  0.904      m15/orb-60m       505  0.996 -> 505  0.996
    fix/pm-into       578  0.961 -> 578  0.961      m15/pdhl-break    163  0.770 -> 163  0.770
    hivol/london      341  0.950 -> 341  0.950      m15/pdhl-fade     531  0.726 -> 531  0.728
    hivol/orb60       457  1.004 -> 457  1.004      pdhl/break-nyam   172  0.758 -> 172  0.758
    lowvol/london     212  0.962 -> 212  0.962      pdhl/fade-allday  555  0.739 -> 555  0.739
    lowvol/orb60       73  1.074 ->  73  1.074      london/early      458  0.947 -> 458  0.947
    london/nyam       525  0.953 -> 525  0.953      london/expansion  338  0.945 -> 338  0.945

**Which desk conclusion depends on these 16: none that moves.** Four of the
nine docs — `london-fix`, `volcond-breakout`, `london-range`, `pdhl`,
`m15-check` — state their window **in their own prose** ("2022-06 →
2025-04-10", "2022-06-16 → 2025-04-10 (198,716 bars)"). The prose was right;
only the argv was under-specified. Each of those rows is a FAIL that is still
a fail at the same number.

### W1+C1 — 3 rows. The same, plus a data repair the record itself asked for.

`orb/60m`, `orb/30m` and `orb/60m-expansion` on `xauduka:5m` published from a
run with **344,744 bars** over 2022-06-16 → 2026-05-31. Pinning that exact span
today gives **278,169** — 66,575 fewer bars in the same dates. That is the
Dukascopy flat-filled zero-volume cleaning that
`docs/decisions/2026-09-13-orb-ny.md` itself listed as process change #3 ("the
Dukascopy converter should drop flat-filled zero-volume bars") and whose
result its own addendum quotes ("198,716 bars, flat bars removed").

    orb/60m             189  0.945  ->  189  0.966     exact count, PF +0.021
    orb/30m             394  1.088  ->  394  1.092     exact count, PF +0.004
    orb/60m-expansion   153  1.115  ->  137  1.102     -16 trades: DOES NOT return

**Which desk conclusion depends on these 3: none.** The doc's own
data-integrity role had already ruled the window contaminated — *"Any future
xauduka OOS must be cut at 2025-04-10"* — and its addendum re-ran all three on
the cleaned, cut window, where they fail again. The closing sentence quotes
"PF 0.95–1.12", which is these three rows; two of the three still print inside
that range and the third (1.102) stays inside it too. `orb/60m-expansion` is
the only row in the whole set of 34 whose **trade set** genuinely changed, and
the direction is stated: 16 of its 153 entries were on flat-filled bars that no
longer exist.

### W2 — 9 rows. The store grew at the live end, under an omitted `--to`.

    XAUUSD-15m   published  33,589 of 100,249 bars, last bar 2026-09-11 20:45
                 now        33,589 of 100,586 bars, last bar 2026-09-17 13:00
    XAUUSD-5m    published 100,749 bars, last 2026-09-11 20:55
                 now       101,242 bars, last 2026-09-15 16:00

Four of the nine come back exactly, with the bar input byte-matched
(100,749 bars, same first and last timestamp):

    london/early      203  1.309 -> 203  1.292      london/nyam  257  1.317 -> 257  1.323
    london/expansion  131  1.209 -> 131  1.207      orb/30m      164  0.897 -> 164  0.874

The five `close/*` rows on `xauusd:15m` come back to **33,589 bars with the
identical first and last bar**, and the trade counts to within one:

    close/1815-2200   289  1.195 -> 289  1.209     EXACT
    close/1630-1815   282  1.299 -> 281  1.275     -1
    close/1630-2000   282  1.637 -> 281  1.702     -1
    close/1630-2200   282  1.355 -> 281  1.359     -1
    close/1400-1630   289  0.955 -> 288  0.889     -1

**And the record already carries the 281.** `2026-09-13-close-reopen-drift.md`
line 323, in its own 2026-09-15 amendment: *"Second window, `xauusd` (Vantage)
2025-04-11 → 2026-09-12, **281 holds**"* — the same bound I pinned, the same
number I measured. The doc prints 282 for the unbounded confirmation run and
281 for the bounded re-pricing of *"the same trades"*. So the one-trade
difference is the trade at the window edge that an explicit `--to` drops, it is
**internal to the record and already recorded there**, and nothing in the
conclusion moves: the doc closed these rows on the expectancy leg
(0.012R/0.039R/0.038R against +0.050R), which one trade in 282 cannot reach.

### P1 — 4 rows. A preset that never took effect, and the engine source already named the file.

`vwap-fade`'s grid is `{stretchAtr, stopAtr}`. The batch file overrides
`stretchAtr` — **the same key the walk-forward sweeps.** Before `f4c8e7c` a
preset did not remove the axis from the grid, so the grid overwrote it. The
proof is in the published receipt itself: `vwap/allday` (declared
`stretchAtr = 1.5`) and `vwap/wide` (declared `2.5`) printed **byte-identical**
trades, PF, expectancy, null p50, null p95 and percentile — two different
declared parameters cannot produce the same 5,828 trades.

`crates/fd-backtest/src/hypotheses.rs:1347-1351` says it in the source:

> A preset on a parameter the method also sweeps **pins** it: the grid loses
> that axis. Otherwise the walk-forward would replace the registered value with
> the grid's and the row would test something else — which it did, once
> (`2026-09-13-vwap-fade.md`).

**Positive control: delete the colliding override from a copy of the TOML, and
all four published numbers return exactly.**

    vwap/allday     5828  0.951  -0.029  ->  5828  0.951  -0.029   EXACT
    vwap/wide       5828  0.951  -0.029  ->  5828  0.951  -0.029   EXACT (identical to allday, as published)
    vwap/us-hours   1392  0.917  -0.054  ->  1392  0.918  -0.054   exact count
    m15/vwap-allday 3392  0.917  -0.037  ->  3392  0.917  -0.037   EXACT

So these four published rows are **the unpinned-grid numbers**, and the
parameter each row says it tested was never tested on it.

**Which desk conclusion depends on these 4, precisely.**
`2026-09-13-vwap-fade.md` already carries the note for `vwap/wide` and already
says the only receipt that tested 2.5 ATR was the direction null (77th,
inside). What its note does **not** say, and what this measurement adds: the
same defect applies to **`vwap/allday` and `vwap/us-hours` and
`m15/vwap-allday` too** — their rows are also grid-selected rather than
preset-at-1.5. The conclusion ("a 1.5-ATR stretch continues; mean reversion to
the session average at this timeframe and cost has no edge") rests on the
**direction nulls** — allday 13th, us-hours 15th, m15/vwap-allday 0th — which
are computed from the preset directly and are therefore untouched. **The
conclusion stands; three more of its evidence rows than the record admits are
mislabelled as to which parameter they tested.** Under the fixed engine the
honest 1.5-ATR numbers are `vwap/allday` 5,700 / PF_usd 0.934 and
`m15/vwap-allday` 2,557, and the honest 2.5-ATR number is `vwap/wide` 4,966 /
PF_usd 0.978 — all still failing both legs.

### G1 — 1 row. The weekend guard's cut-off moved after the receipt was written.

    close/fri-1630-1815   336 trades, PF_usd 0.827   ->   0 trades today

Both window ends are pinned (`--from=2018-06-16 --to=2025-04-10`), the bar
input is identical (**160,368 bars**, same first and last), and the sibling row
in the same run reproduces exactly (`close/1630-1815` 1,355 -> 1,355). Only the
Friday row changed, and it went to **zero entries**.

The cause is a recorded config amendment, not an engine defect.
`config/default.toml:204` moved `flat_before_weekend_hhmm` from **1655 to
1640** on 2026-09-19 (`docs/decisions/2026-09-19-weekend-flat-never-fires.md`:
the 1655 cut-off could never fire, because the last 15-minute bar the desk
receives closes at 16:45 NY = minute 1005 against a cut-off of 1015). The entry
is signalled on the 16:15 NY bar and filled on the 16:30 bar, whose close is
minute 1005. `Guards::calendar_refusal` (`guards.rs:223-226`) refuses an entry
whose **signal bar or fill bar** closes past the cut-off. At 1655 neither did,
so the entry was taken and then flattened — the published receipt says exactly
that: *"guards: refused none; closed WEEKEND_FLAT 336"*. At 1640 the fill bar
is past the cut-off, so the entry is **refused** and the row has no trades.

**POSITIVE CONTROL, and it is exact.** With `flat_before_weekend_hhmm = 1655`
restored in a scratchpad copy of `config/` and nothing else changed, the row
comes back **byte-identical to the published one**:

    close/fri-1630-1815   336 trades   PF_usd 0.827   expectancy -0.004 R
    and the patched binary adds: PF_r 0.8308, Lbar 0.0241 R, gap +0.0037,
    identity residual -0.00002 R, max drawdown 242.29 USD = 2.42% of peak,
    net -137.92 USD over 336 trades
    guards line: "weekend flat 16:55 NY"   (the published receipt's own value)

Both PF readings are far below 1.200, so there is no unit flip hiding here
either. **The config amendment is the whole cause; nothing else moved.**

**Which desk conclusion depends on it: none, and it is strengthened.**
`2026-09-13-close-reopen-drift.md`'s guarded addendum concluded *"the row this
record could not confirm on 68 Fridays is one the paper bot is not allowed to
hold."* The guard now forbids it one step earlier — at the entry rather than at
the exit. The row's only positive reading (PF 2.064, 100th percentile,
`SURVIVES`) is the **guards-off** row, and addendum 5 D already rules the
guards-off arm one the owner has forbidden.

### E1 — 1 row. One trade at a pinned edge, on a byte-identical window.

    close/1630-1815 (friday-weekend-hold)   1621  PF_usd 0.733  ->  1620 today

`--from=2010-06-01 --to=2018-06-15` both pinned, **191,809 of 378,749 bars,
identical first and last bar**, no guards, no grid collision. The drift is one
trade in 1,621 — the same signature as the four `close/*` rows above, on the
same `session-hold` base at 15 minutes. **Which desk conclusion depends on it:
none.** `2026-09-13-friday-weekend-hold.md` reads this row as *"nets −$1,459 on
1,621 sessions"* — one trade in 1,621 does not move a −$1,459 verdict, and the
doc's conclusion is about the 400 weekend holds, not this row.

## FALSIFIER — it did NOT fire, and the registration said in advance why

Declared: *if the majority of the 34 turn out to be a `PF_usd` published as if
it were `PF_r`, the record is clean at the measurement layer and the cure is a
writing rule.* **Measured: 0 of 34.** A unit cannot move a trade count, and all
34 are unpairable because the trade count moved. The registration predicted
this and stated the price: every row landing in **P1** or **G1** is a desk
conclusion standing on air, and it must be named. Named above — and the honest
result is that **in all five cases the conclusion survives**, because each of
those docs either already carried the defect's note (`vwap-fade`, the guarded
addendum) or closed on a leg the defect does not touch (the direction nulls,
the expectancy leg).

**But the cure is still a writing rule, a different one than the falsifier
guessed:**

> **An argv with an open-ended `--from` or `--to` is not a reproducible
> specification of a run.** 25 of these 34 rows are unreproducible for that
> reason alone, and all 25 come back when the window the decision's own prose
> already states is pinned on the command line. A receipt must echo the window
> it actually ran, not the flag that happened to be typed.

## SPREAD PROVENANCE, per row — and a config sentence that is now false

The coordinator is right that "measured or assumed" has more than two states.
Measured by me today, read-only, on `E:/rust/flowdesk/data/spreads/`:

    market    rows  charged           provenance
    xauusd       9  0.28 /round trip  MEASURED, large sample: XAUUSD_sc.csv n=8,967 over
                                      49.8 h wall, ALL 24 hours covered
    xauduka     21  0.28 /round trip  ASSUMED BY TRANSFER - there is NO XAUDUKA log on
                                      disk; the 0.28 is the Vantage XAUUSD.sc terminal
                                      read applied to a Dukascopy BID series
    btc          4  5.00 /round trip  ASSUMED (config says so). No Binance BTCUSDT log
                                      exists. The nearest measurement is Vantage
                                      BTCUSD_sc.csv p50 17.03 (n=6,650), which the config
                                      itself labels "MEASURED: 3.4x the 5.00 assumed"

So **21 of the 34 rows run on a spread transferred from a different venue's
instrument**, which is a fourth state: not assumed, not measured, *measured
somewhere else*.

**XAUUSD, measured, as a RANGE and by hour — not as a point:**

    n = 8,967 samples, 2026-09-15 02:01Z .. 2026-09-17 03:50Z, 24 of 24 hours
    min 0.210   p50 0.220   p95 0.260   p99 = max 0.290        range 38% of the p50

    samples ABOVE the 0.28 charged : 251 = 2.80%, every one of them at 0.290
    and 100% of those sit in hours 20, 21, 22 UTC:
        hour 20 UTC  p50 0.260  max 0.290   13.6% above 0.28
        hour 21 UTC  p50 0.290  max 0.290   50.1% above 0.28   <- 17:00 NY, the CME rollover
        hour 22 UTC  p50 0.260  max 0.290    6.1% above 0.28
    outside 20-22 UTC the maximum observed anywhere is 0.260.

**`config/default.toml:313-317` says: *"p10 0.210, p50 0.220, p90 0.220, max
0.260 — the configured 0.28 is above every spread ever observed"*. On the
larger sample that sentence is FALSE** (max 0.290), and the config's own note
says why it could not know: *"Two things are still missing before that
amendment can be written: the 21:00 UTC rollover hour and a weekend, neither of
which the 12.8-hour sample covers."* **The rollover hour is now covered, and it
is the one hour where 0.28 is too LOW.**

**This lands on exactly one family in these 34, and it is the family defined by
that hour.** The five `close/*` rows on `xauusd` are a hold *across the 17:00
New York close*: signal on the 16:15 NY bar, fill on the 16:30 bar, held through
17:00 NY = **21:00 UTC**. They are the only rows here whose entry and hold sit
inside hours 20–22 UTC. Direction of the error, stated: those rows were charged
**less** than the hour's measured spread, so their published FAIL is
*optimistic*, and repricing them upward makes the fail more conservative, not
less. The one reading that repricing would weaken is the Friday leg's
guards-off `PF 2.064 / 100th / SURVIVES`, which addendum 5 D already rules
untradable.

**NOT CHANGED HERE, and it is the owner's call.** `config/default.toml:319-323`
states the rule: *"every receipt in docs/decisions/ was measured at 0.28, and
moving the number globally would make all of them unreproducible without
editing a single receipt. Repricing is an explicit, recorded amendment … not a
silent edit."* Per addendum 9 §IX and 10 §IX this axis stops at the measurement
and hands the decision over. **OPEN FOR THE OWNER:** write the spread
amendment, now that the rollover hour exists in the log — and decide the
round-trip convention first, because the log records a one-way bid-ask (0.210 →
0.290) while the engine charges 0.28 "per round trip", and the two are not
comparable until that is written down.

## COST/R per row, with the stop size on every row

The coordinator's rule is right and this set measures it again, independently.
From the engine's own `cost-matched null: control stop X ATR = Y points, cost
Z% of R` line, on my own control runs:

    cost/R measured on 23 of 34 rows   min 0.77%   max 17.13%   ratio 22.2x
    on xauduka ALONE (one instrument)  min 1.93%   max 17.13%   ratio  8.9x

    widest, same instrument, same feed, same spread, only the stop differs:
        hivol/london       stop 6.534 ATR(5m) = 14.51 pts   cost  1.93% of R
        pdhl/break-nyam    stop 0.778 ATR(5m) =  1.63 pts   cost 17.13% of R

Every row's stop is in the receipt table. **11 of 34 have NO cost/R and it is
printed as `n/m`, not 0** — the `session-hold` rows (`close/*`, `fix/*`) are
**L1 denominator stops** (`Exits::Strategy`, self-managed; addendum 7 §B): the
"stop" is `riskDailyRanges = 1 x rangeDays = 20` New York daily ranges and the
engine never enforces it, so there is no enforced stop to divide a spread by.
Deriving one would need a `--spread=` scan (addendum 10 §III's clean method),
which is a new program — **recorded as OPEN, not guessed.**

## Multiple-testing ledger: declared vs seen

    declared                                seen
    W1 controls                   7           7   (all exit 0)
    W2 controls                   3           3
    P1 controls                   2           2
    G1 controls                   0           1   <- OVERRUN, declared here
    ----------------------------------------------
    CEILING                      12          13

**The overrun is stated rather than hidden.** The registration budgeted 0 runs
for G1 because it expected the cause to be engine code, which cannot be
controlled for without a bisect. It turned out to be a **config value** with a
recorded amendment date, which makes a control one copied `config/` directory
away. One run, with `flat_before_weekend_hhmm = 1655` restored in a scratchpad
copy of `config/`, `config/` in this worktree untouched.

No gate cell was spent by any of the 13. None of them tests a hypothesis; each
pins back one thing and asks whether a published number returns.

## Store state at the time of my runs — because the store is written while it is read

    XAUDUKA-5m    1,135,389 bars   2010-06-01 00:00 .. 2026-05-31 23:55
    XAUDUKA-15m     378,749 bars   2010-06-01 00:00 .. 2026-05-31 23:45
    XAUUSD-5m       101,242 bars   2025-04-11 01:05 .. 2026-09-15 16:00
    XAUUSD-15m      100,586 bars   2025-04-11 00:00 .. 2026-09-17 13:00
    BTCUSDT-5m      218,880 bars   2024-08-13 19:00 .. 2026-09-12 18:55
    BTCUSDT-15m      70,080 bars   2024-09-12 10:15 .. 2026-09-12 10:00
    news            747 events     from data/news/events.parquet
    tape (btc runs) 8,711 / 8,713 frames    gold/xauusd runs: 0 prints, 0 frames

`data-sealed/` was not opened.

## What was NOT measured, and why (null, not 0)

- **0 of the 34 are unreadable, but 6 of the 34 are not fully adjudicated**
  and the registration's rule applies to them: a row that needs a new program
  is an OPEN item, not a verdict.
  - **5 rows carry a one-trade residual at a pinned window edge** (the four
    `close/*` on `xauusd:15m` and `close/1630-1815` n=1621 on `xauduka:15m`).
    The bar input is byte-matched and the direction is known — an explicit
    `--to` drops one trade. The record's own 2026-09-15 amendment already
    prints the bounded number (281). **Naming the commit that owns it needs a
    bisect across ~4 weeks of engine history: OPEN.**
  - **1 row does not return its trade set at all**
    (`orb/60m-expansion` 153 -> 137). The cause is named (66,575 flat-filled
    bars removed from the window) but which 16 entries were lost is not read.
    **OPEN**, and cheap: `--exit-mix` plus a bar-level diff.
  - One further caveat on that control: the same config also moved
    `max_trades_per_day` from **4 to 20** (a separately recorded amendment in
    `config/default.toml:155-169`). It cannot touch these rows — they take at
    most one trade a day — and the exact return of 1,355 and 336 trades proves
    it did not. It is named so the next reader does not have to re-derive it.
- **8 of the 28 exact-count rows return a PF that drifted** (largest:
  `orb/60m` 0.945 -> 0.966, `orb/30m` 0.897 -> 0.874). On an identical trade
  count with a changed bar series (W1+C1) that is expected; on a byte-matched
  series (`london/*` on xauusd, ±0.025) it is not, and it is **OPEN** for the
  same bisect.
- **No percentile and no verdict word is re-derived.** `SURVIVES` depends on
  the seat in the TOML and on null width as `1/sqrt(n)`; this axis reads trade
  sets. Every verdict quoted above is the **published** one.
- **The other 17 of the 51 vanished keys** sit only in receipts, never in
  `docs/decisions/`, and were out of scope by the registration's criterion (e).
  They are **unmeasured**, not shown to be fine.
- **The 4,342 band rows with no echoed argv** remain `pfr-audit`'s ceiling and
  this axis did not move it.
- **No drawdown criterion** (addendum 5 §G), although this binary prints it.
- **No spread was repriced.** The measurement is above; the amendment is the
  owner's.

## What would change this answer

- A bisect of the engine between 2026-09-13 and 2026-10-08 would close the
  one-trade residual and the PF drift on byte-matched windows. That is the only
  thing standing between 28/34 and 33/34.
- Making `search` echo the **resolved** window (first and last bar) into the
  argv line of every receipt would make this whole class impossible to create
  again. That is the one-line infrastructure change this axis argues for.


---

## Note added 2026-10-11, later — three things the coordinator sent after this result was written, each measured against MY OWN control runs (appended; nothing above is rewritten)

All three land on the cost/unit table above. Two of them correct it. The first
one does **not** replicate on this set, and under brief §8 the measurement is
reported rather than the claim.

### A. The sign of `PF_usd - PF_r` is NOT a window constant on this set

Measured on the **43 cells** my own 13 control runs printed a
`Lbar ... PF_r ... PF_usd ... gap` line for — no new engine run, the lines were
already in the receipts:

    window                market   tf   cells  USD higher  share   |gap| min  |gap| max
    W1 (2018-06-17 ->)    xauduka  15m      2           1   50.0%     0.0037     0.0049
    W1 (2022-06-16 ->)    xauduka  15m      4           3   75.0%     0.0051     0.0095
    W1 (2022-06-16 ->)    xauduka   5m     20          11   55.0%     0.0020     0.0122
    W1 (2024-08-13 ->)    btc       5m      3           2   66.7%     0.0091     0.0096
    W1 (2024-09-12 ->)    btc      15m      2           2  100.0%     0.0045     0.0115
    W2 (2025-04-11 ->)    xauusd   15m      6           2   33.3%     0.0114     0.2188
    W2 (2025-04-11 ->)    xauusd    5m      6           4   66.7%     0.0014     0.0483

    pooled by window:  W1 2022-06-16  14/24 = 58.3%      W2 2025-04-11  6/12 = 50.0%
    pooled across all: 25/43 = 58.1%   <- the number a pooled ratio would print

**The coordinator measured W1 USD-higher 24/24 (100%) and W2 0/24 (0%) on 48
cells of ONE mechanism, one parameterisation, one arm. This set reads 55% and
33–67%, mixed inside BOTH windows.** The two are not in conflict and the
reconciliation is the useful part: the coordinator's set held the mechanism
fixed and varied the window; this set holds the window fixed and varies the
mechanism across nine of them. So the sign is **not** determined by the window
alone — on one window with nine mechanisms it splits almost evenly. If it is
determined by anything it is **mechanism × window**, which is a two-factor
statement neither set can settle on its own.

**The operational rule is therefore the one addendum 9 §I and 10 §V already
give, and this is now the eighth measurement of it: measure the gap on your own
cells, print both PFs on every row, and do not quote anyone's ratio — including
a per-window one.** The honest caveat on my own table: the per-bucket counts are
2–20 cells, so this set cannot *refute* a 100%/0% split, it can only show that
the split is not reproduced when the mechanism is allowed to vary.

**And it found one flip the result above did not report.** Two of the 43 cells
have `PF_usd` and `PF_r` on **opposite sides of the gate's 1.200**:

    m15/eq-open-60m   n=316  btc     PF_usd 1.1919  PF_r 1.2034   reads TOO LOW
    close/1815-2200   n=289  xauusd  PF_usd 1.2088  PF_r 1.1960   reads TOO HIGH

The first independently reproduces `agent/pfr-audit`'s only decision-record
flip, from a different run and a different batch file — worth having as a second
sighting. **The second is new.** It is the pinned W2 control for
`close/1815-2200` in `2026-09-13-close-reopen-drift.md`: read in USD that
control clears the PF leg (1.2088), read in R it misses it (1.1960). The
**published** row was PF 1.195 and already below, so no published verdict moves
— but anyone re-running that row with a pinned `--to` and reading `PF_usd` would
now see it clear a leg it does not clear in R. Recorded here so the next reader
does not have to find it twice.

Two more free reads off the same 43 cells, both confirming the record's own
arithmetic: the identity `E = Lbar x (PF_r - 1)` holds on every cell printed
(largest residual seen in any line of my runs, 0.00002 R), and **35 of 43 cells
have `Lbar >= 0.250 R`**, i.e. the expectancy leg is REDUNDANT on them and
binds on the other 8 — the split addendum 6 §I predicted.

### B. CORRECTION to the cost/R column above: it names ONE ruler, and there are two

Every `cost/R` figure in this result and in the nine appended notes is read off
the engine's own line

    cost-matched null: control stop X ATR = Y points, cost Z% of R

which uses the **control's NOMINAL stop size**. `agent/range-sizing` measures
the same quantity from the **p50 of the per-trade distance `d` on the signal
bar**, and the two rulers disagree by roughly 1.25x on the same cells (its
15.03% / 7.52% against the engine line's 18.42–18.79% / 9.30–9.41%).

**Both are correct and they answer different questions.** Nothing above is
withdrawn, but every number in that column must be read as
*engine-nominal-stop ruler*, and it is **not comparable** to a
`range-sizing`-style number without converting. The table's claim that survives
either ruler is the **ratio**, because a constant factor cancels: cost/R spans
**22.2x across the 34 rows and 8.9x within `xauduka` alone**, driven by stop
size on one instrument, one feed, one spread.

And the coordinator is right that a single cost/R per instrument is an
undeclared choice. This result already refuses to print one: the column is
per-row, with the stop size on the row. The further variation *within* a row —
`range-sizing` measures f=1.0 -> 15.03% against f=2.0 -> 7.52% on one window,
and 10.68% / 5.34% on the other — is **not measured here and is not claimed**;
these 34 rows each ran at one stop, so this table reads one point of that curve
per row. **OPEN**, and the clean method is the `--spread=` scan of addendum
10 §III.

### C. The component of the R denominator nobody declares: `delta = spread/2`

Defect 16 (addendum 7 §C): `risk = f x range + delta`, because `sizing_stop`
reads the signal bar's `close` while `entry` has already been through
`apply_costs`, so `d risk / d s = +1/2` **exactly, from the code** — not from a
fit. Expressed against each row's own printed stop:

    row                 mkt      stop(pts)  spread   delta   delta/R   cost/R
    pdhl/break-nyam     xauduka       1.63    0.28  0.1400    8.59%   17.13%
    pdhl/fade-allday    xauduka       1.79    0.28  0.1400    7.82%   15.69%
    m15/pdhl-break      xauduka       2.08    0.28  0.1400    6.73%   13.46%
    m15/pdhl-fade       xauduka       2.37    0.28  0.1400    5.91%   11.81%
    lowvol/orb60        xauduka       5.32    0.28  0.1400    2.63%    5.26%
    ...
    london/expansion    xauusd       36.45    0.28  0.1400    0.38%    0.77%
    vwap/allday         btc         113.39    5.00  2.5000    2.20%    4.41%
    m15/vwap-allday     btc         228.91    5.00  2.5000    1.09%    2.18%

    delta/R across the 23 rows carrying an enforced stop: 0.38% -> 8.59% = 22.4x

**An identity worth writing down, because it makes defect 16 free to read off
any receipt.** `delta` is `spread/2` by construction and the engine's cost line
is `spread/stop`, so

    delta/R  =  (the printed cost/R)  /  2      exactly

and it holds on **24 of 24** rows measured here at 49.8%–50.1% (rounding only).
So **any receipt that prints `cost Z% of R` is also telling you that defect 16
is worth `Z/2`% of that row's R**, with no extra run.

**Why it matters on this set specifically:** the four `pdhl` rows are the
narrowest stops in the 34 and the highest cost/R, and on them `delta` alone is
**5.91% to 8.59% of R** — i.e. the half-spread that leaked into the denominator
is on its own **larger than the gate's entire expectancy leg of +0.050 R**, by
up to 1.7x. Those rows were closed at the 1st–6th percentile of their nulls
(`2026-09-13-pdhl.md`), so the direction of the verdict is not in doubt; but the
*size* of their failure is partly a mis-measured denominator rather than money
paid to a venue, and the record nowhere says so.

Note this is a different quantity from the coordinator's 14.34% -> 3.38%
(4.2x): that is `delta` varying **per trade inside one cell** (p10 to p90 of
`d`); mine is `delta` varying **across rows** at each row's median stop. Both
are real, they are not the same number, and neither should be quoted for the
other.

### D. The spread log, restated because it is half this job's value

Unchanged from the section above and re-measured, read-only, today:
`data/spreads/XAUUSD_sc.csv` holds **8,967 samples over 49.8 hours covering all
24 hours** against the **2,315 samples / 12.8 hours** `config/default.toml`
describes. p50 **0.220**, p95 **0.260**, p99 = max **0.290**, range
0.210 -> 0.290 = **38% of the p50**. **2.80% of samples exceed the 0.28
charged**, and **100% of those sit in hours 20-22 UTC** (hour 21 = 17:00 NY, the
rollover, p50 0.290; outside 20-22 UTC the maximum observed anywhere is 0.260).
The config sentence *"the configured 0.28 is above every spread ever observed"*
is **false on the larger sample**, and the config's own note names the gap it
could not close: *"the 21:00 UTC rollover hour and a weekend."* The rollover
hour is now in the log.

`XAGUSD_sc.csv` (n=3,434) and `BTCUSD_sc.csv` (n=6,650) are the only other logs
on disk, so of the eight `markets.*` entries in the config exactly **three** have
a spread log and **five do not** — including `xauduka`, which carries 21 of
these 34 rows on a number transferred from a different venue's instrument.

**Still not repriced here, and still the owner's call** for the reason
`config/default.toml:319-323` gives, with the round-trip convention to be
settled first (the log records a one-way bid-ask; the engine charges 0.28 "per
round trip"). **OPEN FOR THE OWNER.**

Receipt for this note: `receipts/receipt-reconcile/TABLE-gap-delta.txt`,
produced by `py/research/receipt_reconcile_gap.py` and
`py/research/receipt_reconcile_delta.py` over the control runs already in
`receipts/receipt-reconcile/controls/`. No engine run was added; the ledger
stays at 13.
