# RESULT — the gross-per-exposed-session ceiling of this registry on `xauduka` 15m

Registration: `docs/decisions/2026-10-08-gross-ceiling.md`, committed alone at
`bf9bbee` before the design file and before the first cell of this branch.
Design: `docs/research/designs/2026-10-08-gross-ceiling.toml`, 30 rows, frozen.
Receipts: `{A,Aswap,C,Cswap}-duka-{is,oos}.txt`, 8 runs, 240 printed rows.

Binary `/e/rust/fd-drawdown/target/release/search.exe` (built 2026-10-07 18:08
from `agent/drawdown` `eac9812`; `git diff eac9812 fd0e718 -- crates/ src/
Cargo.toml Cargo.lock` is empty, so nothing was built on this branch).
Every receipt reproduces `flags: 12 passed, every one of them read by
--mode=hypotheses` and `news: 747 events (2010-01-08 → 2027-12-08) from
E:/rust/flowdesk/data\news\events.parquet`, which is the **default `data/`**
regardless of `--data=`.

---

## 0. The one number

**The largest gross-of-spread R per exposed session reached by any registry
mechanism on this data, on the minimum of the two time-split windows, is
`+0.4745 R` — `rsi2-pullback` at its registered defaults, guards off, OOS leg
(IS leg `+0.7642 R`).**

**F1 does NOT fire on that reading.** The declared line was +0.050 R and the
frame needs ~0.1 R; the unrestricted ceiling is **4.7x** the frame's
requirement.

**F1 DOES fire on the frame-compatible reading, which is the one the desk can
act on.** Restricted to rows that take at most 1.5 round trips per exposed
session — the declared subset, because the `rollover-flat` frame pays **one**
round trip per session, not seven — and after voiding the one row whose own rule
never fires, the ceiling is **`+0.0126 R` per exposed session** (`quiet-swing`
at defaults, guards off). That is **4.0x below the +0.050 R line** and **8x
below what the frame needs**, and it is within `0.004 R` of what
`agent/rollover-flat` already measured on a frame it had actually built.

So the answer to the brief's question has two halves and both are load-bearing:

* **There IS 0.47 R per exposed session of gross signal in this data.** It is not
  drift (the residual after the section 4 subtraction is `+0.4426 R` of a
  `+0.4745 R` gross — **93%** of it is not exposure), and it survives both
  windows.
* **It is not reachable.** It arrives as **6.9 round trips per exposed session**
  at a spread of **12.1% of R per trip**, so the spread is **176% of the whole
  gross** before anything else happens, net per exposed session is
  **`-0.3599 R`**, and the row's drawdown is **10,117 USD on a 10,000 USD
  book — 98.6% of peak equity**, 1.4 points under the burned line.

**The desk is not short of a better mechanism. At one round trip per session it
is short of a factor of 8 in signal; at the trip rate where the signal exists it
is short of a factor of 1.8 in cost.** Those are two different purchases, and
the second one is the smaller number.

---

## 1. Pre-checks: the ruler, and the binary. Neither falsifier fired.

**F2 — the ruler (0 gate cells).** Steps 1-3 of the registration, applied to
`agent/rollover-flat`'s own receipts, reproduce its published gross per trip on
**6 of 6** rows to the printed digit:

| row | leg | registration section 2, steps 1-3 | `rollover-flat` printed |
|---|---|---|---|
| `qsf-h15` | IS | `0.011 + 1807.22/110.553/1435` = **+0.02239** | **+0.0224** |
| `qsf-h15` | OOS | `-0.005 + 1028.82/91.089/1319` = **+0.00356** | **+0.0036** |
| `tsf-l60` | IS | `0.001 + 1553.52/103.015/1823` = **+0.00927** | **+0.0093** |
| `tsf-l60` | OOS | `0.007 + 972.34/98.824/1790` = **+0.01250** | **+0.0125** |
| `qsf-h5` | IS | `0.009 + 1597.30/106.011/1305` = **+0.02055** | **+0.0205** |
| `tsf-l20` | IS | `0.003 + 1622.68/101.508/1944` = **+0.01122** | **+0.0112** |

**And the same arithmetic reproduces addendum 5 section B's drift from the
method side, which was not asked for and is the stronger check**: `px-1s`'s own
gross per trip reads **`-0.00350 R` (IS)** against the published **-0.0040** and
**`+0.02026 R` (OOS)** against the published **+0.0205**, and its spread per
trip reads **1.150% / 0.826% of R** against `rollover-flat`'s published
**1.15% / 0.83%**. The ruler and the drift anchor are therefore **measured in my
own runs**, not carried in from a brief.

**F3 — binary and settings parity.** My `Aswap-duka-is.txt` reproduces
`rollover-flat`'s `B-duka-is.txt` on all four anchors, every printed figure:

    px-1s    2014  PF 0.905  expect -0.015  total_r -30.06  total_r_net -32.71  swap -212   spread 1832.80  mean hold 1371.2 min
    qs-h15    190  PF 0.594  expect +0.130  total_r +24.61  total_r_net -50.42  swap -6166  spread  176.23
    ts-l20    183  PF 0.309  expect +0.095  total_r +17.29  total_r_net -63.15  swap -5960  spread  107.98
    ts-l60     93  PF 0.109  expect +0.151  total_r +14.04  total_r_net -68.00  swap -6091  spread   54.50

`tsmom` at defaults (`lookbackDays 60`) prints **identically** to `ts-l60`, as
the registration said it must. **Did not fire.**

**`wrong_side_stop` is 0 in all 8 receipts**, so brief section 7's first defect
is not exercised on any of the 240 rows, consistent with the ~580 rows the
record had already counted.

**Anchors, per run** (`S` = `px-1s`'s own mean hold, the session unit):

| arm | window | `S` | `risk_usd` | `px-1s` spread/trip | drift anchor |
|---|---|---|---|---|---|
| guards off | IS | 1371.2 min | 80.00 USD | 1.150% of R | **-0.00350 R/session** |
| guards off | OOS | 1398.5 min | 96.04 USD | 0.826% of R | **+0.02026 R/session** |
| guards on | IS | 1313.8 min | 75.39 USD | 1.164% of R | **-0.00836 R/session** |
| guards on | OOS | 1340.4 min | 92.74 USD | 0.823% of R | **+0.01623 R/session** |

`px-1s`'s trade count is also the count of sessions this feed opens:
**2,014 (IS) / 1,982 (OOS)**, used as the secondary calendar denominator in
section 5.

---

## 2. The ranking, as declared: `min(IS, OOS)` gross R per exposed session

Guards off. Every number from the receipts; `**burned**` = `max_drawdown_pct`
over 100%, which F6 excludes from the ranking.

| row | min(IS,OOS) | gross/sess IS | gross/sess OOS | gross/trip IS | gross/trip OOS | trips/sess | spread/gross | net/sess OOS | dd USD IS / OOS | dd% | avg_mae |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **`rsi2-pull`** | **+0.4745** | +0.7642 | +0.4745 | +0.1144 | +0.0686 | 6.68 / 6.92 | 155% / **176%** | **-0.3599** | 10,436 / 10,117 | 99.5 / 98.6 | -0.649 |
| `rsi-rev` | +0.2693 | +0.5250 | +0.2693 | +0.0628 | +0.0303 | 8.36 / 8.89 | 285% / 384% | -0.7647 | 11,152 / 10,508 | 99.7 / 98.7 | -0.746 |
| `sess-hold` | +0.2539 | +0.3229 | +0.2539 | +0.0961 | +0.0750 | 3.36 / 3.39 | 190% / 140% | -0.1016 | 14,555 / 8,875 | 97.0 / 88.8 | -2.241 |
| `crt` | +0.1370 | +0.6736 | +0.1370 | +0.1153 | +0.0250 | 5.84 / 5.48 | 175% / 500% | -0.5485 | 8,966 / 9,239 | 89.2 / 92.4 | -0.703 |
| `stoch-rev` | +0.2259 | +0.5554 | +0.2259 | +0.0769 | +0.0288 | 7.22 / 7.83 | 260% / 433% | -0.7519 | 11,347 / 10,818 | **100.1 / 100.2** | -0.754 |
| `macd-cross` | +0.1803 | +0.5068 | +0.1803 | +0.0623 | +0.0229 | 8.14 / 7.87 | 296% / 506% | -0.7320 | 10,009 / 10,408 | **100.1 / 100.1** | -0.756 |
| `bb-fade` | +0.0396 | +1.3150 | +0.0396 | +0.1038 | +0.0029 | 12.67 / 13.58 | 196% / **3498%** | -1.3447 | 10,525 / 10,383 | **100.1 / 100.3** | -0.749 |
| `far-stop` | +0.0035 | +0.1082 | +0.0035 | +0.0204 | +0.0007 | 5.30 / 5.27 | 360% / 6557% | -0.2267 | 10,307 / 10,010 | 99.0 / 98.4 | -0.508 |
| `quiet-swing` | +0.0126 | +0.0154 | +0.0126 | +0.0755 | +0.0567 | 0.20 / 0.22 | **21% / 19%** | **+0.0102** | 1,084 / 1,169 | 8.6 / 9.9 | -0.534 |
| `qs-h15` | +0.0112 | +0.0155 | +0.0112 | +0.1462 | +0.0912 | 0.11 / 0.12 | **11% / 12%** | **+0.0098** | 1,086 / 1,001 | 8.4 / 8.5 | -0.639 |
| `ts-l20` | +0.0073 | +0.0073 | +0.0092 | +0.1063 | +0.1299 | 0.07 / 0.07 | **11% / 6%** | **+0.0086** | 728 / 702 | 6.0 / 5.7 | -0.468 |
| `tsmom` = `ts-l60` | +0.0057 | +0.0057 | +0.0131 | +0.1633 | **+0.3775** | 0.04 / 0.03 | **8% / 2%** | **+0.0129** | 687 / 673 | 5.7 / 4.9 | -0.583 |
| `px-1s` | -0.0035 | -0.0035 | +0.0203 | -0.0035 | +0.0203 | 1.00 / 1.00 | — | +0.0120 | 4,599 / 2,063 | 40.4 / 15.4 | -0.355 |
| `donchian` | -0.0137 | +0.1229 | -0.0137 | | | 7.75 / 7.67 | | -0.6445 | 10,053 / 10,355 | **100.0 / 100.0** | -0.684 |
| `keltner` | -0.0245 | -0.0245 | +0.0889 | | | 7.78 / 7.92 | | -0.6572 | 10,002 / 10,128 | **100.0** / 99.9 | -0.736 |
| `pdhl` | -0.0766 | +1.6203 | -0.0766 | | | 17.40 / 22.00 | | -3.3220 | 9,549 / 10,184 | 95.1 / 96.4 | -0.777 |
| `ema-cross` | -0.0939 | +0.0995 | -0.0939 | | | 8.34 / 8.15 | | -0.9531 | 11,135 / 10,057 | 99.2 / 98.2 | -0.776 |
| `orb` | -0.1249 | -0.1249 | +0.2725 | | | 6.94 / 7.03 | | -0.0141 | 6,746 / 2,292 | 67.0 / 21.4 | -0.596 |
| `ict-smf` | -0.5323 | -0.1417 | -0.5323 | | | 6.47 / 6.37 | | -0.8914 | 4,329 / 4,999 | 43.2 / 50.0 | -0.656 |
| `squeeze` | -0.5653 | -0.5653 | +0.5383 | | | 11.79 / 12.12 | | -1.6002 | 10,800 / 9,274 | **103.6** / 92.7 | -0.781 |
| `vwap-fade` | -0.6329 | +1.3778 | -0.6329 | | | 14.36 / 15.28 | | -2.1694 | 10,143 / 10,018 | **100.0 / 100.2** | -0.837 |
| `doji-rev` | -5.0827 | +2.6389 | -5.0827 | | | 23.77 / 36.66 | | -8.5775 | 9,966 / 9,958 | 97.9 / 99.4 | -0.797 |
| `trend-pull` | -7.9415 | +3.0535 | -7.9415 | | | 37.30 / 45.54 | | -17.6246 | 10,263 / 10,024 | **100.2 / 100.2** | -0.823 |

Guards on (the only tradeable arm, addendum 5 section D) gives the same shape
with every number smaller: `rsi2-pull` **+0.3751**, `sess-hold` **+0.3313**,
`buy-hold` +0.3237 (**void**, section 4), `stoch-rev` +0.0894, `orb` +0.0255,
then the long-horizon family at +0.0080 to +0.0106. **No row in the guards-on
arm reaches +0.050 R on `min(IS, OOS)` except the three that are either
frame-incompatible at 6.7-9.9 trips per exposed session or void.**

**The first thing to read in this table is not the ordering. It is that the
`spread/gross` column and the `min(IS,OOS)` column point in opposite
directions.** The top eight rows all have `spread/gross` above 100% — the spread
already exceeds the entire gross — while the four rows with `spread/gross` under
25% are the four at the bottom of the gross ranking. There is no row in this
registry with both.

---

## 3. Falsifiers

**F1 — the ceiling. FIRES on the frame-compatible reading, does NOT fire
unrestricted.** Both numbers are stated in section 0 and neither is suppressed.
The frame-compatible number is the one that answers the brief's question,
because the brief's question is *"what does the `rollover-flat` frame need and
is it there"*, and that frame takes one position per session.

**F4 — artefact #10, the signal family. FIRES, and hard.** The row with the
**highest single-window** gross per exposed session is `trend-pull` guards off
IS at **+3.0535 R**, whose other window is **-7.9415 R**. The next three are the
same story:

| row | best window | other window |
|---|---|---|
| `trend-pull` OFF | IS **+3.0535** | OOS **-7.9415** |
| `doji-rev` OFF | IS **+2.6389** | OOS **-5.0827** |
| `pdhl` OFF | IS **+1.6203** | OOS **-0.0766** |
| `vwap-fade` OFF | IS **+1.3778** | OOS **-0.6329** |
| `bb-fade` OFF | IS **+1.3150** | OOS **+0.0396** (33x collapse) |

**17 of 49 readable row-arms (34.7%) flip the SIGN of their gross per exposed
session between the two halves**, `px-1s` itself among them (-0.0035 -> +0.0203,
which is the drift sign flip of addendum 5 section B measured a second,
independent way). Ranking on a single window would have produced a **+3.05 R**
headline that is **-7.94 R** on the other half. This is why `min(IS, OOS)` was
declared in section 2 of the registration and not chosen afterwards.

**F5 — exposure, not signal. Does NOT fire on the maximum.** `rsi2-pull`'s
drift attribution is **+0.0004 R (IS)** and **+0.0319 R (OOS)** against a gross
of +0.7642 and +0.4745, so the residual is **+0.7639** and **+0.4426** — i.e.
**100.0% and 93.3% of its gross is not drift**. The signal is real. (Addendum 5
section B's "75-100% of the sample rows' edge is not drift" reproduces here.)

**F5 fires on two other rows, and they are the two that matter most for the
frame**, because they are the ones that look frame-shaped:

* **`sess-hold` at defaults, guards off, OOS**: gross per exposed session
  **+0.2539 R**, drift attribution **+0.2575 R**, residual **-0.0035 R**.
  **101.4% of its OOS gross is the gold drift.** Its IS residual is +0.3783 R
  against an IS drift of -0.0555 R — i.e. the row is *drift plus something* in
  the first half and *drift minus a little* in the second. Reported as
  **exposure**, not as a mechanism.
* **`buy-hold`, guards on, OOS**: gross **+0.7800 R**, drift attribution
  **+0.9435 R**, residual **-0.1635 R**. It collects **less** than its own
  exposure would.

**F6 — burned rows. FIRES on 15 of 120 cells, 9 row-arms**, all in the guards-off
arm, and all excluded from the ranking:

    bb-fade/OFF   IS 100.1%  OOS 100.3%        keltner/OFF   IS 100.0%
    donchian/OFF  IS 100.0%  OOS 100.0%        squeeze/OFF   IS 103.6%
    macd-cross/OFF IS 100.1% OOS 100.1%        stoch-rev/OFF IS 100.1%  OOS 100.2%
    trend-pull/OFF IS 100.2% OOS 100.2%        vwap-fade/OFF IS 100.0%  OOS 100.2%
    gap-fade/OFF  OOS 272.4%  (28,014.34 USD on a 10,000 USD book)

None of them held the maximum, so the ceiling of section 0 is unchanged by the
exclusion. **`gap-fade` guards-off OOS is the worst instance of defect 14 in
this record so far**: `max drawdown 28,014.34 USD = 272.41% of peak`, net
**-27,729.87 USD**, `avg_mae -2.699 R`, and its spread bill is **28,346.05 USD
over 130 trades = 218 USD per trade**. Back out the R unit and its **sizing stop
is 0.117 price points — 11.7 cents of gold** — which at 1% of equity is about
**7.8 lots per trade on a 10,000 USD book.** The row is a degenerate
sizing-stop, not a strategy result, and the engine traded it for eight years
past the point the account was gone.

---

## 4. The void row, caught by the declared exits check

**`buy-and-hold` in the guards-on arm reads `+0.3237 R` on `min(IS, OOS)` at
1.06-1.11 round trips per exposed session — the only row in the whole table
that is both above +0.050 R and at the frame's trip rate.** Its exits:

    guards ON  IS   END_OF_DATA 1, NEWS_FLAT 230, OPEN_LOSS_CAP 1713, WEEKEND_FLAT 385
    guards ON  OOS  END_OF_DATA 1, NEWS_FLAT 228, OPEN_LOSS_CAP 1497, WEEKEND_FLAT 365

**Its own rule fires 0 times out of 2,329 and 2,091 trades.** This is exactly
brief section 6a's trap (`tsmom/120d`, PF 2.236, `SURVIVES`, own rule 0 times),
and it is **void** by the registration's section 8. What the row actually
measures is *a buy-and-hold chopped into daily pieces by the guards*, and its
drawdown says so: **21,052 USD (IS) and 32,286 USD (OOS)** on a 10,000 USD book,
with `avg_mae -1.75 R`. Its F5 residual is **-0.1635 R** — it under-collects its
own drift. Removing it is what leaves `quiet-swing` at **+0.0126 R** as the
frame-compatible ceiling.

Frame-compatible subset, after F6 and the void, `min(IS, OOS)`:

| row | arm | min(IS,OOS) | gross/trip IS / OOS | trips/exposed-sess | trips/calendar-sess | residual IS / OOS | dd USD IS / OOS |
|---|---|---|---|---|---|---|---|
| `quiet-swing` | off | **+0.0126** | +0.0755 / +0.0567 | 0.20 / 0.22 | 0.16 / 0.15 | +0.0157 / +0.0056 | 1,084 / 1,169 |
| `qs-h15` | off | +0.0112 | +0.1462 / +0.0912 | 0.11 / 0.12 | 0.09 / 0.10 | +0.0159 / +0.0037 | 1,086 / 1,001 |
| `qs-h15` | **on** | +0.0106 | +0.0801 / +0.0299 | 0.32 / 0.36 | 0.20 / 0.19 | +0.0272 / +0.0057 | 795 / 1,338 |
| `ts-l20` | **on** | +0.0090 | +0.0393 / +0.0281 | 0.31 / 0.32 | 0.30 / 0.31 | +0.0126 / +0.0059 | 839 / 1,166 |
| `tsmom`=`ts-l60` | **on** | +0.0081 | +0.0293 / +0.0473 | 0.28 / 0.29 | 0.26 / 0.27 | +0.0076 / +0.0101 | 1,124 / 1,150 |
| `quiet-swing` | **on** | +0.0080 | +0.0782 / +0.0219 | 0.33 / 0.37 | 0.20 / 0.19 | +0.0272 / +0.0031 | 859 / 1,527 |
| `ts-l20` | off | +0.0073 | +0.1063 / +0.1299 | 0.07 / 0.07 | 0.09 / 0.09 | +0.0074 / +0.0048 | 728 / 702 |
| `tsmom`=`ts-l60` | off | +0.0057 | +0.1633 / +0.3775 | 0.04 / 0.03 | 0.05 / 0.05 | +0.0055 / +0.0083 | 687 / 673 |
| `px-1s` | off | -0.0035 | -0.0035 / +0.0203 | 1.00 / 1.00 | 1.00 / 1.00 | 0 / 0 | 4,599 / 2,063 |
| `px-1s` | **on** | -0.0084 | -0.0084 / +0.0162 | 1.00 / 1.00 | 1.00 / 1.00 | 0 / 0 | 4,292 / 2,480 |

**Nothing in this subset reaches a tenth of the +0.050 R line, in either arm.**
And the subset is the whole long-horizon family of the record — `tsmom`,
`quiet-swing` and their anchors. `agent/rollover-flat`'s flat variants measured
**+0.0036 to +0.0224**; the continuous parents measured on the same ruler
measure **+0.0057 to +0.0155** on `min(IS,OOS)`. **The frame did not lose the
signal. There was +0.013 R of it.**

---

## 5. The reading that changes the question: gross per session is a LEVERAGE number, and `spread/gross` is not

The rows at the top of section 2 and the rows at the bottom are not separated by
how much price they capture. They are separated by **how small their stop is**,
because R *is* the stop, and the engine risks 1% of equity per R.
`spreadR_per_trip` measures the stop in price directly, since the spread is a
flat 0.28 points at every hour on this feed:

| row | spread/trip | implied R | gross/trip IS | gross per session, own R | gross per session, in price points |
|---|---|---|---|---|---|
| `rsi2-pull` | 17.74% of R | **1.58 points** | +0.1144 | **+0.7642 R** | +1.21 |
| `sess-hold` | 18.21% of R | **1.54 points** | +0.0961 | +0.3229 R | +0.50 |
| `qs-h15` | 1.62% of R | **17.3 points** | +0.1462 | +0.0155 R | +0.27 |
| `px-1s` | 1.150% of R | **24.3 points** | -0.0035 | -0.0035 R | -0.09 |

`rsi2-pull`'s 0.76 R per session and `qs-h15`'s 0.016 R per session are **1.21
and 0.27 price points** of the same metal. The 49x gap in R is a **15x gap in
leverage** and a **4.5x gap in captured price.** Both readings are true and the
record's own rule says which to quote: *"cost/R = spread/stop, goes by the
HORIZON, and always say the ATR of which bar size"* — a bare R number is not
comparable across stop widths.

**The quantity that does not move with leverage is `spread/gross`.** Lever a row
up and gross and spread scale together. So the frame's requirement, stated
leverage-free:

    frame needs:  gross per session >= ~8.7 x the spread of one round trip
                  (0.1 R of gross against 1.15% of R of spread)

    rollover-flat measured:  0.4x - 2.3x       (spread ate 44-240%)
    best in this table:      3.94x   rsi2-pull guards off, min(IS,OOS)
                             4.12x   rsi2-pull guards on
                             2.47x   sess-hold guards on
                             1.77x   sess-hold guards off
                             0.98x   quiet-swing / qs-h15 guards off

(This ratio is `trips_per_session / (spread/gross)`, a ratio of two columns the
registration declared. It is **derived after the numbers were seen** and is
labelled as such; it is not the headline and the ranking of section 2 is
unchanged by it.)

**So the measured ceiling, leverage-free, is 3.94 spreads of gross per exposed
session against a requirement of ~8.7. The gap is a factor of 2.2 — not the
factor of 8 the R-denominated reading suggests, and not reachable by levering,
because levering moves neither side of this ratio.** The one purchase that does
move it is **a lower spread**: at a round-trip cost of 0.13 points instead of
0.28, `rsi2-pull`'s ratio crosses 8.7. That is a 2.2x cost reduction on a
feed whose spread is already a flat 0.28 with no session widening — so it is a
**venue and instrument question**, which is exactly what F1's negative branch
said the desk should go and ask.

---

## 6. The gate, since the tool prints it: 0 in the tradeable arm, 3 in the forbidden one

Not a finding of this job, reported because brief section 10 asks for it and
because the verdict strings are in the receipts. Counted **by hand at 40**, not
at the tool's `need 30`.

* **Guards ON (the only arm the owner permits): 0 of 30 rows clear
  `PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades` on BOTH windows.**
  `quiet-swing` and `qs-h15` clear it on IS (PF 1.248 / 1.255, expectancy
  +0.064 / +0.066, 400 / 393 trades, both printed `SURVIVES`) and miss on OOS
  (PF 1.037 / 1.067).
* **Guards OFF: 3 distinct methods clear it on both windows**, and all three are
  the long-horizon family already in the record as `agent/n5`'s gate-passers:

| row | IS PF / expectancy / n | OOS PF / expectancy / n | carry measured by `agent/pure-drift` |
|---|---|---|---|
| `tsmom` = `ts-l60` | 1.383 / +0.151 R / 93 | 2.183 / +0.370 R / 90 | **-0.641 R/trade** -> `expectancy_net -0.731 R` |
| `ts-l20` | 1.315 / +0.095 R / 183 | 1.427 / +0.122 R / 185 | -0.440 R/trade -> `expectancy_net -0.345 R` |
| `qs-h15` | 1.262 / +0.130 R / 190 | 1.201 / +0.080 R / 199 | -0.395 R/trade -> `expectancy_net -0.265 R` |

  All three live in the arm addendum 5 section D says is **not tradeable**, and
  all three are wiped by carry in the swap arm. Their gross per exposed session
  is **+0.0057 to +0.0155 R** — the bottom of section 2's ranking. **The three
  rows in this registry that pass the desk gate on both windows are the three
  with the LOWEST gross per exposed session in it.** That is the clearest
  single statement of what addendum 6 section II proved by arithmetic: the gate
  rewards few long trades, and the frame needs many short ones.

**Which leg binds, at the realised exit mix: addendum 5 section A's reading
HOLDS here, and `agent/rollover-flat`'s opposite reading also holds on its own
rows — addendum 6 section I's identity reconciles them.** Measured on my 98
readable cells: **10 cells clear PF >= 1.200 and every one of them also clears
expectancy >= +0.050 R; 2 cells clear expectancy and miss PF** (`quiet-swing`
guards-off IS at PF 1.181 / +0.060 R; `buy-hold` guards-on OOS at PF 1.195 /
+0.258 R); **0 cells clear PF and miss expectancy.** So **expectancy is
redundant and PF binds** — 2 one-way, 0 the other, the same direction and almost
the same count as addendum 5's 4-and-0. `rollover-flat` measured the reverse on
its flat rows, and addendum 6 section I's identity `E = Lbar x (PF - 1)` says
exactly why — expectancy is redundant iff `Lbar >= 0.250 R`, and `Lbar` is
recoverable from the printed pair as `E / (PF - 1)`:

    tsmom  IS   0.151 / 0.383 = Lbar 0.394 R   >= 0.250  -> expectancy redundant
    qs-h15 IS   0.130 / 0.262 = Lbar 0.496 R   >= 0.250  -> expectancy redundant
    ts-l20 IS   0.095 / 0.315 = Lbar 0.302 R   >= 0.250  -> expectancy redundant
    tsf-l60 IS  0.001 / 0.022 = Lbar 0.045 R   <  0.250  -> expectancy BINDS   (rollover-flat's row)

**Two agents read opposite answers off the same tool and both were right. The
discriminator is `Lbar` — gross loss per trade — not the gate**, and a
flat-across-the-rollover expression crushes `Lbar` by construction because it
books a fraction of an R per session.

---

## 7. An eleventh artefact, measured by accident and worth more than the ranking

The design has `tsmom` at defaults and `ts-l60` as an override to
`lookbackDays = 60`, which **is** the default — so the two rows are the **same
method on the same bars in the same arm**, and they print identical `trades`,
`PF`, `expect`, `total_r`, `total_r_net`, `swap$` and `mean hold`. They do
**not** print the same null:

| receipt | row | trades | PF | expect | null p50 | null p95 | pct | verdict |
|---|---|---|---|---|---|---|---|---|
| `A-duka-is` | `tsmom` | 93 | 1.383 | +0.151 | 0.872 | **1.297** | **96%** | **`SURVIVES`** |
| `A-duka-is` | `ts-l60` | 93 | 1.383 | +0.151 | 0.966 | **2.036** | **81%** | `gate pass, inside the noise` |
| `A-duka-oos` | `tsmom` | 90 | 2.183 | +0.370 | 0.927 | **1.422** | **100%** | **`SURVIVES`** |
| `A-duka-oos` | `ts-l60` | 90 | 2.183 | +0.370 | 1.211 | **3.190** | **82%** | `gate pass, inside the noise` |

**Same method, 200 matched null runs each, and the null's 95th percentile moves
1.297 -> 2.036 and 1.422 -> 3.190 while the percentile moves 96% -> 81% and
100% -> 82% and the printed VERDICT STRING changes.** The only difference
between the two rows is **their position in the batch file**, which feeds the
seed. Four of the record's receipts quote `SURVIVES` as if it were a property of
a method; on this evidence it is partly a property of **where the method sat in
a TOML file**. Addendum 6 section VI already measured that the null's width
moves with `1/sqrt(n)`; this measures that it also moves with **nothing at all**.

Consistent with the registration's section 9.1, **no percentile is published or
read anywhere in this report**, and this pair is the reason that was the right
call rather than a cautious one.

---

## 8. NOT measured

1. **`gap-fade` (both windows) and `intraday-momentum` (OOS) cannot be put
   through the calibrated ruler at all**, because their `swap$` prints **0** in
   the swap arm: they cross **zero** 17:00 New York rollovers by construction,
   so `|total_r_net - total_r|` is 0 and step 1 has nothing to divide. **This is
   NOT MEASURED, not 0.** It is also a positive finding the frame should keep:
   `gap-fade` is **already** rollover-flat, on the one window — 16:00-18:00 New
   York — that `agent/rollover-flat` explicitly listed as unmeasured because its
   own frame forgoes it. An uncalibrated side-estimate from the arm-A
   `net USD / (expectancy x trades)` route (**declared here as not the
   registered ruler**) puts `gap-fade` at **-0.42 R** and `intraday-momentum` at
   **-0.0103 R** on `min(IS,OOS)` gross per exposed session, so neither changes
   the ceiling; and `gap-fade` OOS is burned at 272.4% regardless.
2. **Three of the 26 registry mechanisms take essentially no trade at defaults on
   this feed**: `rsi-reversal-vol` **0 / 0**, `volume-thrust` **0 / 0**,
   `volman-box` **1 / 0**. The drawdown line correctly prints
   `max drawdown: NOT MEASURED — this cell took no position` rather than 0.00
   USD. `volman-box`'s single IS trade prints `expect 1.806` and `PF inf`; at
   n = 1 that is **not a measurement** and it is excluded, as is `buy-and-hold`
   guards-off at n = 1 (`gross/trip +42.58 R` IS and **+2875.45 R** OOS over one
   16-year hold, which is a unit statement about a 4,207,455-minute position and
   nothing else).
3. **Percentiles**, for the reasons in sections 1 and 7.
4. **Intrabar excursion.** Every drawdown here is the closed-trade curve;
   `avg_mae` is printed beside it and is the only field that sees an open
   position go against the book. On `sess-hold` that gap is the story:
   `avg_mae -2.241 R` on a row whose stop is **never enforced** (exits are
   `window closed` 2,062 of 2,062 in the guards-off arm), so its R is a sizing
   unit the position routinely exceeds twofold.
5. **Parameters.** Defaults only, 30 rows. A mechanism that would reach the
   ceiling at some other parameter is not measured here, and sweeping parameters
   is the ~6,000-cell shape that has returned 0 eight times.
6. **Any instrument or bar size but `xauduka` 15m.** Brief section 0 item 3
   measured that the **sign** of an edge can be a property of the 15m ruler.
   Every number in this report is a 15m number.
7. **`data-sealed/`.** Not opened, read, pointed at or counted.
8. **The live spread at the session break.** A flat 0.28 at every hour here; a
   real venue widens it exactly where a per-session book re-enters. Every
   `spread/gross` above is a **lower bound** and section 5's "2.2x cost
   reduction" is therefore **optimistic**, against the hypothesis.

---

## 9. Multiplicity ledger: declared vs viewed

| | declared | viewed |
|---|---|---|
| rows | 30 | **30** |
| windows | 2 | **2** |
| guards arms | 2 | **2** |
| ranking cells | 120 | **120** |
| swap-arm reads for `risk_usd` | 120 | **120** |
| printed rows | 240, in 8 runs | **240, in 8 runs** |
| pre-check cells | 2 | **2** |
| of the 120 ranking cells: readable | — | **98** (49 row-arms) |
| excluded by F6 as burned | — | **15** |
| not measurable (swap 0, or n < 40) | — | **22** |

No row was added, no window changed, no arm added, no parameter tuned, no
threshold moved, no sample floor lowered. One derived ratio (section 5) is
introduced after the numbers and is labelled as such; the declared ranking is
unchanged by it.

**Selection bias of the maximum, as declared:** `+0.4745 R` is the largest of
120 cells over 30 rows and 2 arms, so it is biased upward. That matters only for
the branch that did not fire. **The frame-compatible ceiling of `+0.0126 R` is
also a max over its subset, and it is 4x below the line — so F1's negative
branch is safe in exactly the direction the registration said it would be.**

---

## 10. If the owner wants the one sentence

**There is `+0.47 R` per exposed session of real, non-drift, two-window-stable
gross signal in this data, and it is reachable only at 6.9 round trips per
session where the spread is 176% of it; at the one-trip-per-session rate the
`rollover-flat` frame runs, the whole registry's ceiling is `+0.0126 R`, which
is 8x short. Leverage moves neither conclusion. A 2.2x cheaper round trip moves
both.**
