# pure exposure vs the gate, vs the drift null, vs the best pattern in the record — result

Registration: `docs/decisions/2026-10-07-pure-exposure.md`, committed **alone**
at `c8800e8`, before any `xauduka` number existed on this branch.
Rows: `docs/research/designs/2026-10-07-pure-exposure.toml`, 8, frozen.
Binary: `/e/rust/fd-instr-repair/target/release/search.exe` (2026-10-07 01:35,
`agent/instr-repair`, the one with `expectancy_net` / `total_r_net`).
**No build. `cargo` was never invoked on this branch.**
Every run: `--mode=hypotheses --fixed --exit-mix --null-sides=exposure
--seeds=200 --interval=15m --data=/e/rust/flowdesk/data --market=xauduka`,
spread 0.28, trail off. Every receipt prints
`flags: 12 passed, every one of them read by --mode=hypotheses` (13 in arm C,
which adds `--guards`).
`bounds:` `--from=2010-06-01 --to=2018-06-01 kept 190889 of 378749 bars` and
`--from=2018-06-01 --to=2026-06-01 kept 187860 of 378749 bars`.
`news:` identical in all six receipts: **747 events (2010-01-08 -> 2027-12-08)
from `E:/rust/flowdesk/data\news\events.parquet`**, scope USD.
`data-sealed/` was not opened, read, pointed at or counted.

## 0. The two falsifiers that fired, and the one that could not

| | fired | where |
|---|---|---|
| **F1** pure exposure clears the gate on both legs, arm A | **FIRED** | no `px-*` row clears **either** leg, let alone both |
| **F2** gate pass at swap 0 but not at the generic rate | **could not fire** | nothing cleared arm A, so nothing could be a perk |
| **F3** the sign test: drift per session positive on both legs | **FIRED** | **-0.0040 R/session** on DUKA-IS, **+0.0205 R/session** on DUKA-OOS — the sign flips |

## 1. The gate, counted by hand at 40 trades

Gate, not moved: **PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades, on
both legs.** Counted at 40 by hand; the tool's verdict string says `need 30`.

| arm | guards | swap | cells clearing BOTH legs |
|---|---|---|---|
| A | off | 0.00 | **0** |
| B | off | -0.83 / -0.83 per lot-night | **0** |
| C | on | 0.00 | **0** |

**0 of 48.** The best pure-exposure cell anywhere is `px-1s` on DUKA-OOS in
arm A: **1,982 trades, PF 1.072, expectancy +0.012R** — short of the PF leg by
0.128 and of the expectancy leg by 0.038R, on the friendliest arm that exists.
The three pattern rows reproduce `agent/n5` and clear the gate on both legs in
arm A only (section 6).

## 2. Gold's drift per session, measured from the method side with no null at all

`px-1s` and `px-short` are the **same** exposure, the same window, the same
hours, the same spread, opposite sides. So their expectancies must sum to minus
twice the spread in R, and half their difference **is** the drift. Both rows
fire their own rule on 1,981-2,013 of 1,982-2,014 exits.

| leg | long `px-1s` | short `px-short` | **drift / session** | sum | implied spread/R |
|---|---|---|---|---|---|
| DUKA-IS 2010-06 -> 2018-06 | **-0.015 R** | -0.007 R | **-0.0040 R** | -0.0220 R | **1.10% of R** |
| DUKA-OOS 2018-06 -> 2026-06 | **+0.012 R** | -0.029 R | **+0.0205 R** | -0.0170 R | **0.85% of R** |

R here is **1.5 mean New York-day ranges over 20 days**, the sizing
`quiet-swing` ships; the cost figure is `spread / stop` at that stop, per brief
section 8, and it is the number the two rows imply, not one assumed.

**The arithmetic checks against a third and fourth row.** `px-ny`
(09:30-16:00) and `px-on` (18:00-09:30) together tile the same session that
`px-1s` holds in one trade, so they must sum to it plus the one spread `px-1s`
does not pay:

| leg | `px-ny` | `px-on` | + spread | predicted | measured `px-1s` | residual |
|---|---|---|---|---|---|---|
| DUKA-IS | -0.009 R | -0.017 R | +0.0110 R | **-0.0150 R** | -0.015 R | **-0.0000 R** |
| DUKA-OOS | -0.005 R | +0.009 R | +0.0085 R | **+0.0125 R** | +0.012 R | **+0.0005 R** |

A residual of **0.0005 R** over four independently-run rows. So for pure
exposure the whole result is **price drift over the hours held, minus one
round-trip spread per trade, and nothing else** — the chop-invariance claim of
registration section 7.1, confirmed rather than assumed.

**Drift density, gross of spread:**

| leg | 09:30-16:00 NY | 18:00-09:30 NY | whole session |
|---|---|---|---|
| DUKA-IS | +0.294 R / 1,000 h | **-0.385 R / 1,000 h** | -0.175 R / 1,000 h |
| DUKA-OOS | +0.509 R / 1,000 h | **+1.115 R / 1,000 h** | +0.880 R / 1,000 h |

**What that buys the gate.** At the OOS density, a pure long position needs
`(0.050 + 0.0085) / 0.00088 = ` **66 exposure-hours — about 2.9 sessions — per
trade** to reach the gate's `+0.050R` leg with no pattern at all. On the IS half
the density is negative, so **no horizon reaches it.** The gate's expectancy
leg is therefore, for a one-sided book, a statement about **hold length**, not
about method.

## 3. The number worth keeping: buy-and-hold, in dollars an ounce

`buy-and-hold` had never been run in this record. One trade a leg, `END_OF_DATA`,
long share 1.000, exposure share of time +1.000, mean hold **4,207,455 min**
(8.00 years) on both legs. Its R unit is the 15-minute ATR at a single instant
(**$1.8555/oz** on IS, **$1.1271/oz** on OOS) so its R figures are not
comparable to anything, exactly as the registration declared. In dollars, all
of it derived from numbers the receipts print (`risk_usd = |swap| /
|expectancy_net - expectancy|` = **$100.00** on both legs — the 1% risk budget;
`lots = spread$ / 0.28`):

| leg | position | price captured | financing at the generic rate | nights | NET |
|---|---|---|---|---|---|
| DUKA-IS | 53.89 oz | **+$78.73 / oz** | **-$3,118.97 / oz** | 3,758 | **-$3,040.24 / oz** |
| DUKA-OOS | 88.71 oz | **+$3,240.73 / oz** | **-$3,117.68 / oz** | 3,756 | **+$123.04 / oz** |

Nights are **1.286 per calendar day** on both legs — days plus two extra per
Wednesday, which is `clock.rs::swap_nights` exactly, so the model is doing what
it says.

**Read it in one line: across the largest gold bull market in this feed — a
+$3,240.73/oz move in eight years — the generic swap rate takes 96.2% of it. In
the other eight years it takes 3,962% of it. Pure gold exposure held
continuously for the whole sixteen years, charged at the generic rate, loses
-$2,917/oz.** That is not a parameter failure and no horizon fixes it: the
financing is **11,135x-11,139x** the single round-trip spread the position paid.

## 4. The finding that is new and actionable: financing is charged per rollover crossed, not per hour held

`swap$ / spread$` — both printed, receipt-internal, no outside number — across
eight rows that are **all** gold exposure on the same instrument:

| row | hold | DUKA-IS | DUKA-OOS | arm-B `expectancy_net - expectancy` |
|---|---|---|---|---|
| `px-on` 18:00-09:30 | 15.6 h | **0.014x** | **0.030x** | +0.000 / +0.000 R/trade |
| `px-ny` 09:30-16:00 | 6.8 h | 0.106x | 0.153x | -0.001 / -0.002 R/trade |
| `px-1s` 18:00-16:00 | 22.9 h | 0.116x | 0.188x | **-0.001 / -0.001 R/trade** |
| `px-short` 18:00-16:00 | 22.9 h | 0.123x | 0.186x | -0.001 / -0.001 R/trade |
| `qs-h15` | 215 h | 35.0x | 29.0x | -0.395 / -0.259 R/trade |
| `ts-l20` | 334 h | 55.2x | 53.5x | -0.440 / -0.321 R/trade |
| `ts-l60` | 651 h | 111.8x | 113.2x | **-0.882 / -0.641 R/trade** |
| `bh` | 70,124 h | **11,139x** | **11,135x** | -1,680.907 / -2,766.035 R/trade |

Five orders of magnitude, same instrument, same currency, same direction of
exposure. The ordering is not by hours: `px-1s` holds **65.7% of the calendar
clock** (2,761,575 of 4,207,680 min on IS) and pays **~0.04 rollovers per
exposure-day** against `bh`'s **1.286** — about **3% on IS and 5% on OOS** —
because
`swap_nights` charges a crossing of 17:00 New York and a book flat from 16:00
to 18:00 crosses none. The residual it does pay is holidays, where the feed
prints no bar in the flat window.

So the sentence `agent/n5` was forced to write — *an account perk, not a
strategy* — is true of **long-horizon** mechanisms and is **not** a property of
gold exposure as such. It is a property of **sleeping through the rollover**.
`px-1s` survives the generic rate intact (`expectancy_net +0.011R` vs
`expectancy +0.012R`); `ts-l60` does not (`-0.271R` vs `+0.370R`).
**It just has no edge to protect.**

**NOT MEASURED, and not claimed:** whether `qs-h15` or `ts-l60` would keep any
of their arm-A edge if re-expressed as flat-across-17:00 variants. That is a
different mechanism and it is not in this plan. Also not modelled: a real venue
widens the spread around its daily break, and this feed's spread is a flat 0.28
at every hour.

## 5. The guards arm deletes the mechanism, measured at maximum contrast

Arm C guards as the receipt prints them: `max_open_loss_r 2.0, notional 300%,
weekend flat 16:40 NY, news flat 60/30 (impact>=3, USD), daily cap 20, loss
limit $300, cooldown 30 min`.

**`buy-and-hold` — a strategy whose `on_bar` issues one `Intent::Enter` and
never an `Intent::Exit` — books 2,329 trades on DUKA-IS and 2,091 on
DUKA-OOS.** Mean hold falls from **4,207,455 min to 1,181.8 min**, a factor of
**3,560**. Its own rule fires **0 times**: every exit is
`OPEN_LOSS_CAP 1,713 / WEEKEND_FLAT 385 / NEWS_FLAT 230 / END_OF_DATA 1`.
On DUKA-OOS that row prints **PF 1.195, expectancy +0.258R, 2,091 trades,
percentile 100%** and misses the gate **on the PF leg alone, by 0.005**. It is
the `tsmom/120d` trap in its purest available form, and it is **void** under the
`agent/n5` rule-3 convention. In arm C the label `buy-and-hold` is false: the
mechanism is "re-enter long whenever the guards have flattened you".

`qs-h15` prints the only **`SURVIVES`** of the 48 cells — arm C, DUKA-IS, PF
1.255, +0.066R, 393 trades, percentile 98%, own rule firing **88 of 393** — and
fails its own OOS leg (PF 1.067, +0.020R). That reproduces `agent/n5` section 4
to the trade count.

**The one exception, and it matters.** `px-1s` is the only row here whose arm-A
and arm-C numbers are the **same mechanism**: PF 0.912 -> 0.875 (IS) and
1.072 -> 1.046 (OOS), mean hold 1,371.2 -> 1,313.8 min (a 4% change), own rule
still firing 1,761 of 2,014 and 1,723 of 1,982. A one-session book is flat
before 16:40 New York **by construction**, so `WEEKEND_FLAT` fires **once** in
2,014 trades. It therefore also complies with the owner's standing rule of
2026-09-19 (*"nguyên tắc là không bao giờ giữ qua tuần"*) without a guard.

**Which raises the governance point this plan did not set out to make: every
long-horizon result in this record, `agent/n5`'s four gate-passers included,
exists only in an arm the owner has already forbidden.** `WEEKEND_FLAT` fires
148-385 times on the long-hold rows in arm C, so the 1640 cut-off repaired in
`2026-09-19-weekend-flat-never-fires.md` does work on this feed — and it is
exactly what deletes them.

## 6. Does the pattern add anything over exposure? Two readings, and they disagree

**M1 = net R per 1,000 hours of position time**, declared before the run. Arm A
(`expectancy_net == expectancy` at swap 0, by construction and by assertion).
Starred column rescales `tsmom`'s 2.0-daily-range R unit onto the
1.5-daily-range unit the other rows use (x 1.333), as the registration required.

| row | mean hold | IS M1 | OOS M1 | IS M1* | OOS M1* |
|---|---|---|---|---|---|
| `px-1s` | 22.9 h | **-0.656** | **+0.515** | -0.656 | +0.515 |
| `px-ny` | 6.8 h | -1.323 | -0.727 | -1.323 | -0.727 |
| `px-on` | 15.6 h | -1.092 | **+0.573** | -1.092 | +0.573 |
| `px-short` | 22.9 h | -0.306 | -1.244 | -0.306 | -1.244 |
| `qs-h15` | 215 / 189 h | +0.603 | +0.422 | +0.603 | +0.422 |
| `ts-l20` | 334 / 330 h | +0.284 | +0.370 | +0.379 | +0.494 |
| `ts-l60` | 651 / 670 h | +0.232 | +0.552 | +0.309 | **+0.736** |
| `bh` | 70,124 h | +0.605 | +41.001 | — | — (R unit not comparable, section 3) |

**Reading 1, per unit of exposure, on the OOS leg only:** pure exposure
(+0.515, +0.573) sits inside the pattern band (+0.422 to +0.736) and above two
of the three pattern rows. On that leg the pattern buys little per exposure-hour.

**Reading 2, across both legs:** every pattern row is **positive on both**;
every pure-exposure row is negative on IS. **What the pattern buys is not more R
per exposure-hour — it is surviving the half where exposure loses money.** That
is the honest answer and it goes against the premise this job started from.

**A first-order attribution, with its assumption stated.** Taking the drift
density of section 2 as constant across hours, and the row's own printed long
share as its net exposure to it, the drift accounts for:

| row / leg | net long fraction | hold | drift explains | gross expectancy | residual |
|---|---|---|---|---|---|
| `qs-h15` DUKA-IS | +0.032 | 215 h | **-0.001 R** | +0.143 R | +0.144 R |
| `qs-h15` DUKA-OOS | +0.126 | 189 h | **+0.021 R** | +0.089 R | +0.068 R |
| `ts-l60` DUKA-OOS | -0.066 | 670 h | **-0.039 R** | +0.501 R* | +0.540 R* |

So **75-100% of the pattern rows' gross expectancy is not drift exposure** —
which is a different statement from "they do not beat their null", and the two
have been conflated. The residual is nevertheless **entirely destroyed by
financing** (`qs-h15` `expectancy_net -0.179R`, `ts-l60` `-0.271R`), while pure
exposure's is not (section 4).

## 7. An eighth way a positive result turns out to be a property of the measurement

The brief lists seven. Here is the eighth, pre-declared in registration section
3.5 as a read limit and then measured:

**The `RandomHold` control is handed more time in the market than the row it is
matched to — on 14 of the 15 measurable arm-A rows.** Ratios: 0.97, 1.00, 1.07,
1.10, 1.14, 1.15, 1.21, 1.28, 1.28, 1.30, 1.30, 1.32, 1.32, 1.36, 1.39. On the
`px-*` rows, where the side match is **exact** (long share 1.000 method vs
1.000 null — the match `agent/n5`'s direction-switching rows kept failing on
5 of 8), the mismatch is largest: **1.28-1.39**.

Consequence, on the row where it is cleanest: **`px-1s` DUKA-OOS, percentile
0%**. Method PF 1.072, null p50 **1.153**, null p95 1.194 — the deterministic
daily long hold lost to **all 200** random long holds, with an identical side
ratio and 1.11x the spread, because the control was given **28% more exposure
hours in a half where the drift is +0.880 R / 1,000 h**. 0.28 x 0.0205 R is
+0.0057 R per trade of free drift, against a method expectancy of +0.012 R.

So `agent/n5`'s headline — *the drift null stops losing money at >= 5
sessions, 16 of 23 rows with `null p50 > 1.000`* — is, in part, **the control
being over-allocated exposure in a positive-drift half.** The ordering supports
it: where the exposure ratio is near 1 (`ts-l60` OOS, 0.97) the method beats
its null 2.183 to 1.211; where it is 1.28 the null wins. **This is an
instrument defect, it is counted and not repaired, and it is the reason this
report does not read any percentile as a result.**

Also counted, not repaired: `bh`'s null does not calibrate at all on DUKA-IS
(`matched null NaN trades median`, `null p50 NaN`, percentile printed `null`,
not `0.000` — the engine got this right), and on DUKA-OOS it calibrates to
**2 trades against the method's 1** with **cost match 4.85**, so that
percentile is unreadable too. Declared in registration section 3.2 before the
gold run.

## 8. What this report does NOT measure

1. **Drawdown.** `max_drawdown_usd` / `max_drawdown_pct` exist in
   `engine.rs::Metrics` but `--mode=hypotheses` does not print them, so the
   registration's offer of "drawdown over total R" as an alternative measure
   **could not be honoured** and M1 carried the whole load. A `bh` row with
   +$3,240/oz of price and an unprinted peak-to-trough is a return without a
   risk figure, and it is reported as such.
2. **R in dollars for the `px-*` and pattern rows.** The receipt prints no mean
   daily range, so their R unit ($36/oz implied on DUKA-OOS from
   `risk_usd / lots`) cannot be pinned the way `bh`'s can. Cross-row R
   comparisons are therefore stated only inside the 1.5-daily-range family plus
   the declared x1.333 rescale.
3. **Fixed-notional versus risk-constant exposure.** `bh` holds a fixed 88.71 oz
   for eight years; `px-1s` re-sizes daily to a constant $100 of risk, so it
   holds fewer ounces exactly when gold is most volatile. That is why +24.16 R
   of `px-1s` does not integrate to `bh`'s +$3,240.73/oz. Which of the two is
   the right exposure definition is a sizing question this plan did not ask.
4. **`xauusd`.** Not run: four years cannot carry a sixteen-year exposure
   question, declared in registration section 4.
5. The two counted-not-repaired defects of brief section 7 (`wrong_side_stop`,
   `check_exit` gap pricing at `bar.open`): `wrong_side_stop` is not reported on
   any row here, and no row uses a pending order, so neither is exercised.

## 9. Multiplicity ledger — declared vs viewed

| | declared | viewed |
|---|---|---|
| gold cells (window x arm x row) | **48** | **48** (6 runs x 8 rows) |
| pre-registration probe cells | 3 on `xagduka` + 1 header-only `xauduka` call | 3 + 1 |
| grand total | 48 gold + 4 probe | 48 gold + 4 probe |

No row added, no threshold moved, no window changed after a number was seen,
no sample floor lowered. M1 and M2 were defined in registration section 7
before the first gold run and were not redefined. The only amendment is the
dated note in section 11 of the registration.

**Cross-check worth recording:** the repaired binary reproduces `agent/n5`'s
arm-A `xauduka` figures **exactly** — `qs-h15` 199 / 1.201 / +0.080 / null p50
1.110 / 62%; `ts-l20` 185 / 1.427 / +0.122 / 1.188 / 76%; `ts-l60` 90 / 2.183 /
+0.370 / 1.211 / 82% on DUKA-OOS, and 190 / 1.262 / +0.130, 183 / 1.315 /
+0.095, 93 / 1.383 / +0.151 on DUKA-IS — from a different binary in a different
worktree. The `expectancy_net` patch moved no existing number, and the arm-B
financing-to-spread multiples (35.0, 29.0, 55.2, 53.5, 111.8, 113.2) match
`agent/n5` section 3 to the decimal.

## 10. What the desk should take from this

- **Do not move to exposure sizing.** F1 fired on the friendliest arm that
  exists. Pure long gold exposure, chopped any way, clears nothing.
- **"The drift is real" is a fact about the null, and partly about the null's
  construction.** Measured from the method side, gold's one-session drift is
  **-0.0040 R on 2010-2018 and +0.0205 R on 2018-2026** — the same sign flip
  across windows that killed six other programmes, now visible with **no
  pattern, no parameter and no control involved.** The drift is the eighth
  window artefact, not the exception to them.
- **The one thing here that is worth engineering is the rollover, not the
  signal.** A book flat 16:00-18:00 New York pays 1.4-3.0% of the financing of
  a continuous one for 65.7% of its clock. Every long-horizon mechanism in this
  record dies of carry; the carry is avoidable; the mechanisms are not.

## 11. How the rollover count was derived, and its error bar

`swap_nights` is not printed; it is reconstructed per row as
`(swap$ / 0.83) / (spread$ / 0.28 / trades)` — lot-nights divided by the mean
lots the spread implies. For `bh` this is exact (one trade, one lot size:
3,758 and 3,756 nights, 1.286 per calendar day, which is days + 2 per
Wednesday as `clock.rs` specifies). For the `px-*` rows it is **approximate**,
because lots are re-sized every trade and the few trades that do carry a
rollover are holiday trades whose lots are not the mean: `px-1s` comes out at
**~79 of 2,014 trades on IS (3.9%)** and **~126 of 1,982 on OOS (6.3%)**.
The exact figures are `swap$` itself — **-$212 and -$291** — and
`expectancy_net - expectancy` — **-0.001 R/trade on both legs** — both printed,
and neither depends on this reconstruction.
