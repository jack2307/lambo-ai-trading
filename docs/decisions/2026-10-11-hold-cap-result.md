# 2026-10-11 — the hold cap, lifted and measured

**Registration:** `docs/decisions/2026-10-11-hold-cap.md` (committed alone, with
a dated amendment appended before the grid ran).
**Branch:** `agent/hold-cap`. **Binary:** built in this worktree, `target-hc/`,
`--release -p fd-backtest`.
**Store, printed by every run:** `XAUDUKA-15m`, W1 **190,889 bars**
(2010-06-01 00:00 → 2018-05-31 23:45), W2 **187,860 bars**
(2018-06-01 00:00 → 2026-05-31 23:45). Identical bar counts across all four
hold values inside each window, so every comparison below is on one bar set.
Both window ends pinned explicitly on every run; no argv in this record has an
empty `--from` or `--to`.

## 1. The gate

**0 of 96 declared cells pass.** Counted by hand, not by the tool's `need 30`.

Nothing was close. The highest `PF_r` anywhere in 96 cells is **1.0930**
(`hc/far-stop` W2 guards-ON 12 h, arm B) against the gate's 1.200; the highest
`E` is **+0.0451 R** (`hc/orb60` W2 guards-off 24 h, arm B) against +0.050 R.
Every cell carries `n >= 857`, so not one of them is an "under 40 trades, no
conclusion" cell — all 96 are measurements.

## 2. Falsifier

**F1 did NOT fire, by the letter of its amendment, on exactly one cell-pair.**
`G_session` from 4 h to 72 h, swap = 0:

| row | arm | W1 | W2 |
|---|---|---|---|
| `hc/orb60` | off | −0.6785 → −0.3140 | **+8.3046 → +3.2906 (−60.4%)** |
| `hc/orb60` | on | −1.3948 → −1.0089 | **+9.7509 → +4.2371 (−56.5%)** |
| `hc/keltner` | off | −2.1108 → +0.6056 | **+1.4868 → +0.4648 (−68.7%)** |
| `hc/keltner` | on | −3.0133 → +1.1386 | +2.0198 → +1.9553 (−3.2%) |
| `hc/far-stop` | off | +0.8873 → +1.0580 (+19.2%) | +0.0498 → +0.8162 |
| `hc/far-stop` | on | **+1.1218 → +1.3841 (+23.4%)** | **+1.6093 → +2.7844 (+73.0%)** |

`hc/far-stop` guards-ON rises ≥ 20% in **both** windows with the same sign, so
the amended escape clause is met and F1 is not fired. That is stated rather
than reinterpreted. **It does not rescue the axis, for four measured reasons.**

**F2 (carry) FIRES on every cell where `G_session` rose.** §5.
**F3 (byte identity) did not fire** — the default is byte-identical. §6.
**F4 (tool-blind)** did not fire for the method and **did fire for the
control**, in a way nobody had asked about. §7.

## 3. Why the one rise is not the cap being what was binding

**(a) `G_trip` rises almost exactly as fast as `trips` falls — `G` is flat in
hold on `Exits::Engine` too.** `G_session = G_trip × trips_per_exposed_session`,
and over 4 h → 72 h:

| row | win | arm | trips ÷ | `G_trip` × | product × |
|---|---|---|---|---|---|
| `hc/far-stop` | W1 | off | 8.30 | 9.90 | **1.19** |
| `hc/far-stop` | W1 | on | 7.87 | 9.71 | **1.23** |
| `hc/far-stop` | W2 | on | 5.13 | 8.87 | **1.73** |
| `hc/far-stop` | W2 | off | 6.52 | 106.32 | 16.32 (see note) |
| `hc/orb60` | W1 | off / on | 6.08 / 4.02 | 2.82 / 2.91 | 0.46 / 0.72 |
| `hc/orb60` | W2 | off / on | 6.22 / 4.11 | 2.47 / 1.79 | 0.40 / 0.43 |
| `hc/keltner` | W1 | off / on | 4.59 / 3.75 | −1.32 / −1.42 | −0.29 / −0.38 |
| `hc/keltner` | W2 | off / on | 2.99 / 2.56 | 0.93 / 2.48 | 0.31 / 0.97 |

**8 of 12 combinations have product ≤ 1.00** and two of those are negative
(`hc/keltner` W1, where gross is below zero at the short end). A six- to
eight-fold longer hold buys **×1.19 to ×1.73** on gross per exposed session.
The ×16.32 is a small-denominator artifact: its 4 h baseline is `G_trip`
**+0.0109**, essentially zero, and the same row's guards-ON arm reads ×1.73.

This is the second measurement of the same shape on a second exit kind.
`mould-target` measured `G` flat in hold on `Exits::Strategy`; this measures it
flat in hold on `Exits::Engine`. **The "hold it longer" push is now closed on
both exit kinds.** And the mould needs **8.7×** gross/spread per exposed
session; the best cell here is **5.59** (`hc/far-stop` W2 guards-ON at 12 h) and
the best 72 h cell is **2.78**.

**(b) What the extra gross per trip IS: a bigger bet, not a better one.**
`Lbar` — gross loss per trade in R — rises with the cap on **12 of 12**
combinations, with no exception:

    hc/far-stop  W1 off  0.1706 -> 0.2545 -> 0.3602 -> 0.4660   (x2.73)
    hc/far-stop  W2 off  0.1885 -> 0.3101 -> 0.4255 -> 0.4714   (x2.50)
    hc/orb60     W2 off  0.3557 -> 0.4336 -> 0.5174 -> 0.6127   (x1.72)
    hc/keltner   W1 off  0.4037 -> 0.5036 -> 0.5485 -> 0.6406   (x1.59)
    ... x1.37 to x2.73 across all twelve

Through `E = Lbar × (PF_r − 1)` — which held on **96/96** cells here, largest
residual **0.00009 R** — a bigger `Lbar` with `PF_r` pinned near 1.000 only
magnifies whatever sign `PF_r − 1` already had. **`PF_r` is what would have to
move and it does not: 0.8937 → 1.0652 is the largest move any row makes across
the whole 18× range of hold, and it never reaches 1.200.** The cap was holding
down trade SIZE in R, not holding back an edge.

**(c) TIMEOUT turns mostly into STOP, not TARGET.** This is the sharpest
discriminator available and it answers against the axis. Guards-off,
4 h → 72 h, in percentage points of the exit mix:

| row | win | TIMEOUT | ΔSTOP | ΔTARGET | which won |
|---|---|---|---|---|---|
| `hc/orb60` | W1 | 63.6% → 9.5% | +33.6 | +20.2 | STOP |
| `hc/orb60` | W2 | 66.3% → 9.6% | +32.7 | +23.9 | STOP |
| `hc/keltner` | W1 | 48.4% → 8.4% | +30.6 | +9.5 | STOP |
| `hc/keltner` | W2 | 58.3% → 3.5% | +23.1 | +31.6 | TARGET |
| `hc/far-stop` | W1 | 89.0% → 38.8% | +35.3 | +14.6 | STOP |
| `hc/far-stop` | W2 | 86.2% → 23.2% | +35.7 | +27.0 | STOP |

**5 of 6 window-pairs convert the clock's exits predominantly into STOP.** The
one that goes the other way, `hc/keltner` W2, reverses in W1 on the same
mechanism — which is artifact #13 by the rule, not a mechanism. Lifting the cap
is, mostly, letting losing trades run longer.

**(d) In the only arm the owner allows, the guards take over the long hold.**
`WEEKEND_FLAT` grows from **1.5–2.9%** of exits at 4 h to **6.9–26.8%** at 72 h,
and `NEWS_FLAT` from ~2% to **5.1–18.9%**. At `hc/far-stop` W1 guards-ON 72 h the
two guards close **45.7%** of all trades and the row's own clock only 14.3%.
A 72-hour horizon is not really expressible inside the arm that forbids holding
over a weekend; it is partly being cut by the arm.

## 4. The cap was real, though — this is not a null instrument

Stated because a negative needs its positive control. The clock went from
**58.3–89.0% of all exits at 4 h to 0.4–38.8% at 72 h**, mean hold from
182–328 min to 519–2,722 min, and trade counts fell by up to **2.6×**
(`hc/far-stop` W1: 2,597 → 1,058). The flag moves a great deal. It just does not
move `PF_r`.

## 5. Carry: F2 fires everywhere, and it does more than erase the gain

Arm B is `config-swap/` (in-tree since `74a49a9`): the generic gold rate
**−82.76 points = −0.83 USD per lot per night** on `xauusd` and `xauduka`. Arm A
is the desk's **measured** swap-free account (340 closed positions 2026-04-21 →
2026-09-08, 87 held overnight, swap 0.00). **Neither arm is a `null`** — which
corrects the brief's warning that `markets.xauduka`'s `swap = 0.00/0.00` leaves
this receipt blind to financing. What IS `null` here is a third thing: the rate
Dukascopy-era gold would have paid in 2010–2018. That is not known and is not
printed as a zero.

**Every 72 h cell in arm B has negative `expectancy_net`.** The two best rises
of the whole job:

    hc/far-stop W1 guards-off 72h:  E +0.0318 R  ->  expectancy_net -0.123 R   (swap -$14,134)
    hc/far-stop W2 guards-on  72h:  E +0.0253 R  ->  expectancy_net -0.021 R   (swap  -$8,180)
    hc/orb60    W2 guards-off 72h:  E +0.0433 R  ->  expectancy_net -0.066 R   (swap  -$9,732)

And **18 cells whose book survived in arm A blow up in arm B** —
`max_drawdown_pct` crossing 100%, which by the record's own rule means their PF
must not be read at all:

    hc/far-stop W1 guards-off 12h:  19.90% -> 100.71%
    hc/far-stop W1 guards-off 72h:  16.08% -> 111.13%
    hc/orb60    W1 guards-off 72h:  58.94% -> 166.53%
    hc/keltner  W1 guards-off 72h:  65.24% -> 170.00%
    ... 18 in all, every one of them at 12h or longer, none at 4h

So the honest line on the one surviving rise is the one the registration
reserved: **an account perk, not a strategy** — and on this desk it is not even
that, because 0/96 cells clear the gate in either arm.

## 6. The instrument, and the four things it had to prove

**`--max-hold=<milliseconds>` in `hypotheses` and `rescore` — in every mode, in
fact**, since it mutates the shared `TradingRules` before dispatch. Default
**unchanged at 14,400,000 ms**.

1. **DEFAULT BYTE-IDENTICAL — proved, not asserted.** Five receipts captured
   with the pre-change binary (two `hypotheses` arms on the full W2, a
   `rescore`, a `costs`, and a `config-swap` run), then reproduced by the
   post-change binary **passing no flag**: `diff -r` clean, **all five byte for
   byte**. Output was first shown deterministic run-to-run, so the comparison
   means something. A free extra control: **two independent builds of `29c8635`,
   in two different target directories, produce byte-identical output on all
   five** — so the baseline is the commit, not one machine's binary.
   In-suite: `the_default_horizon_is_bit_identical_to_a_four_hour_literal`
   compares **every `f64` field of every trade by `to_bits()`** plus four gate
   metrics, and `with_no_flag_the_line_is_the_one_the_record_already_carries`
   pins the header string as a whole literal. Nothing was relaxed to
   "approximately equal" at any point.
2. **NOT SWALLOWED IN SILENCE — and the scanner was watched catching it.**
   `search.rs` holds a test that scans its own source for flag literals. The
   flag's reading code went in **first**, the suite was run, and that test
   **FAILED** with `["max-hold"]`. Only then was it declared in `ALWAYS`.
   A probe that was not seen to fail proves nothing about the probe.
   New assertions: all 10 modes read it; `--maxhold=`, `--max_hold=` and
   `--max-hold-ms=` are each denounced in the header rather than ignored; a
   non-integer value is a hard error. Verified end to end against the real
   binary. **199 tests pass, 0 fail**, `--release -p fd-backtest`.
3. **IT TOUCHES `Exits::Engine` AND NOTHING ELSE — on the method side.**
   Measured on real bars: four registered `Exits::Strategy` mechanisms
   (`tsmom`, `quiet-swing`, `session-hold`, `intraday-momentum`, 3,548 trades)
   at 4 h / 72 h / no cap — **identical trade counts, PF, expectancy, spread
   paid and time in market at all three values**, with the receipts differing
   in nothing but the header line and the null block. In-suite the same claim is
   pinned by trade fingerprint with its positive control in the same test:
   engine-managed, the identical two values give 4 h **> 10×** the trades of
   72 h and `--max-hold=0` gives one hold to the end of data.
4. **WHERE IT CAN BE SET FROM, measured.** CLI only. `[run] max_hold_ms` and
   `[run] max_hold` in a batch file are **accepted without complaint** — the
   file parses, the 3 hypotheses declare, the run proceeds — and the horizon
   stays at 4 h. Silently inert, the same class as `[run] null_sides`. The
   registered batch file says so in a comment at its top.

**One more thing the flag taught the instrument.** The cap is tested on **bar
boundaries**, so a data gap carries a hold straight past it: the 4 h setting
produced a mean hold of **313.8 min** on `hc/far-stop` W2, and the first reading
of that was "the instrument is broken". It is not — pinned by
`a_gap_carries_a_hold_past_the_cap_and_that_is_the_rule_not_a_bug`. A receipt
that says "72 h" means "72 h plus whatever gap followed".

## 7. The finding nobody asked for: the flag moves the NULL on rows where it
moves 0 method trades

`check_exit` exempts `self_managed` positions, so `--max-hold=` cannot touch a
self-managed method. **The matched null is engine-held on every row.** So on
`sm/tsmom` — mean hold 26 days, `Exits::Strategy`, method bit-identical at all
three values — the control changed completely:

| cap | null trades | count match | null spread | cost match | null time in mkt | exposure ratio | null p50 | null p95 | percentile |
|---|---|---|---|---|---|---|---|---|---|
| 4 h | 91 | **1.08 (in band)** | $34.69 | 0.77 | 33,772 min | **0.01** | 0.875 | 1.425 | 100% |
| 72 h | 230 | 2.74 (out) | $98.96 | 2.20 (out) | 1,094,542 min | 0.34 | 0.978 | 1.311 | 100% |
| none | 43 | 0.51 (out) | $13.35 | 0.30 (out) | 3,345,398 min | **1.05** | **1.033** | **1.659** | 98% |

Read it twice. **At 4 h the control held 1% of the method's time in market while
`count match` read 1.08 — comfortably "in band".** The record's rule for whether
a percentile may be published reads `count match`; on this row `count match` was
satisfied in exactly the arm where exposure was wrong by a factor of 100.

And **the only arm where the control actually collected the method's exposure
(ratio 1.05) is the arm where the control stops losing money** — `null p50`
crosses 1.000 to **1.033**, `p95` rises to **1.659**, and the method's
percentile falls from 100% to 98%.

⇒ **Every percentile the record holds for a long-horizon self-managed row was
read against a control the four-hour cap had crippled.** This bears directly on
`agent/n5`'s four passing cells. It does **not** touch their gate legs — PF and
expectancy are unchanged at every cap value — so what is affected is the
percentile and nothing else. The three other control rows, whose holds fit
inside 4 h, have **identical nulls at all three values**: the null moves only
where the method's hold exceeds the cap, which is the mechanism, measured.

## 8. Numbers worth keeping, including the inconvenient ones

* **`PF_usd` − `PF_r` in my own 96 cells, and nobody's ratio quoted.**
  W1: `PF_r` higher in **42/48**, `PF_usd` higher in 5, equal in 1.
  W2: `PF_r` higher in **26/48**, `PF_usd` higher in 22.
  My W1 is the **opposite direction** to a "W1 100% `PF_usd` higher" reading, so
  the gap's direction is not a constant of the window either. Both PFs are
  printed on every row above.
* **`PF_r` is CARRY-BLIND and that is dangerous at exactly this horizon.** In
  arm B, **13 cells read `PF_r` above 1.000 on a book that lost money**:
  `hc/far-stop` W1 guards-off 72 h reads `PF_r 1.0695` against
  `PF_usd 0.7750` and `expectancy_net −0.123 R`. `PF_r` is built on
  `r = points/risk`, which `apply_costs` puts the spread into and swap never
  reaches. ⇒ **reading the gate on `PF_r` is correct only in the swap-free arm.**
  On any row that pays carry, `PF_r` alone will pass a dead book. No cell of
  mine landed in the 1.19–1.21 band where the two legs straddle the gate.
* **δ/R (defect 16) read free off my own receipts**, as half the printed
  `cost Z% of R`: **1.095% to 4.700% of R**, largest on `hc/keltner` W1 at 4 h
  (`cost 9.40% of R`, control stop 1.931 ATR ⇒ δ/R **4.70%**). Note the ruler:
  that `Z` is the **null's nominal stop** basis, which the engine prints as
  `(realised median, structural)` — not a per-trade p50 ruler, and the two are
  not directly comparable.
* **cost/R falls as the cap rises, because the realised stop widens**:
  `hc/far-stop` W2 goes `3.00% → 2.78% → 2.41% → 2.19%` of R as the control
  stop widens `4.187 → 4.524 → 4.726 → 4.969` ATR. Cost per R is a property of
  the horizon, measured again here.
* **`sized_down_by_guard` moves with the horizon**, read from the printed
  `guards:` line (which formats the field) and not grepped:
  `hc/far-stop` W2 guards-ON **25 → 3 → 222 → 140** across 4/12/24/72 h —
  non-monotonic; `hc/keltner` W1 guards-ON **937 → 863 → 738 → 644`;
  `hc/orb60` W1 guards-ON **285 → 235 → 220 → 230**. So part of any change in
  a guards-ON cell is a shifted lot distribution, exactly as warned. On
  guards-off cells this is **`null`, not 0** — no guard ran.
* **`wrong_side_stop`: not measurable.** `grep -rn wrong_side_stop crates/`
  returns nothing on this tree. No number is reported for it.
* **None of my rows carries a `flat:1630-1815` filter** (`filters =
  ["weekdays"]`), so every row here trades through the 17:00 New York rollover,
  where `XAUDUKA`'s reopening `open` is the bar's own low **19.02%** of the time
  against 3.20% on an ordinary bar. My rows are **not** immune to the provider
  price artifact, and this record does not claim they are.

## 9. Bookkeeping — declared versus spent

    declared gate cells                      96   (48 arm A + 48 arm B)
    spent gate cells                         96   (32 runs x 3 rows; none skipped, none added)
    pre-check screen, before registration     10   (0 gate cells, per brief §5)
    self-managed controls                     12   (4 rows x 3 flag values)
    byte-identity fixtures                 5 x 3   (pre binary, independent pre build, post binary)
    unit / integration tests                 199   (--release -p fd-backtest, 0 failures)
    cells with n < 40 ("no conclusion")        0   (minimum n = 857)

## 10. Two things left for the owner, not decided here

1. **A receipt prints its resolved window but never its argv.** `bars:` already
   carries first bar / last bar / bar count on every run, and `bounds:` carries
   the flags when bounds were passed — so the resolved window IS in every
   receipt. What is missing is the argv itself, and **adding a line to the
   output is exactly what the byte-identity standard forbids**: it would
   restate every receipt in `docs/decisions/`. Those two requirements are in
   direct conflict and the trade is not mine to make. Recorded, not done.
2. **`reward_risk = 1.8` is the real ceiling on an engine-managed long hold.**
   `Exits::Engine` cannot say "a stop and no target"
   (`max_hold_horizon.rs::a_no_target_entry_is_given_one`, defect 5 of
   `2026-09-24-designed-methods.md`), and `reward_risk` lives in the shared
   `[trading]` table with no per-market override. So every 72 h cell above was
   still bounded by a 1.8 R target — which is visible in §3(c), where TARGET
   grows by 9.5 to 31.6 percentage points as the clock stops firing. **A clean
   "lift the horizon" measurement needs a `--reward-risk=` flag of the same
   shape as this one.** Whether to build it is a decision, not a task, and the
   horizon result above is what it is **only for `reward_risk = 1.8`**.
