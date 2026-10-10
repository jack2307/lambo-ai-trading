# RESULT — no parameterisation of any registry mechanism reaches 8.7 spreads of gross per exposed session on both windows

Registration: `docs/decisions/2026-10-10-mould-target.md`, committed **alone** at
`55143bf` before the design file and before the first cell of this branch.
Design: `docs/research/designs/2026-10-10-mould-target.toml` (`16430b8`), 153
rows, frozen. Receipts: `{A,C}-{is,oos}-{s28,s00}.txt`, 8 runs, 1,224 printed
rows, plus `brk-{A,C}-{is,oos}-{s28,s00}.txt`, 8 runs, the amendment diagnostic.

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (built 2026-10-08
20:57 from `agent/stop-width` at `29c8635`, which **is** this worktree's HEAD, so
**nothing was built**). Every receipt reproduces
`flags: 12 passed, every one of them read by --mode=hypotheses` (13 in the
guards arm) and
`news: 747 events (2010-01-08 -> 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet`
— the **default** `data/`, whatever `--data=` says. Header lines reproduced in
every run: `spread: 0.28 per round trip`,
`swap: long 0.00 / short 0.00 USD per lot per night`,
`max hold: 4 h (14400000 ms) from [trading] max_hold_ms`,
`guards: on - max_open_loss_r 2.0, notional 300%, weekend flat 16:40 NY, news
flat 60/30 (impact>=3, USD), daily cap 20, loss limit $300, cooldown 30 min`.

---

## 0. The answer

**No. F1 fires on every reading that is a measurement.**

    requirement, the brief's line                              8.7 x
    requirement on MY OWN measurement of px-1s's spread in R   9.2 x (IS) / 12.0 x (OOS)

    max over 594 readable cells, n >= 40 in BOTH windows       6.061 x   sh-roll, guards ON
      ... and that row's gross is a PROVIDER PRICE STAMP (section 4)
    max whose exposure excludes 16:00-18:15 New York entirely  4.212 x   sh-asia, guards off
    max in the DECLARED frame-compatible subset (trips <= 1.5) 1.901 x   qs-lb3, guards ON
    max of the 26 DEFAULTS (the record's own question, re-ruled) 2.454 x rsi2-pull, guards ON

**The `rollover-flat` frame is dead for want of a signal.** 612 declared cells
over 153 parameterisations of 24 runnable mechanisms, two windows, both arms,
and the frame-compatible ceiling is **4.6x** short of what the frame needs. The
desk is not short of a better mechanism to put in the frame; **there is no
mechanism in this registry, at any parameterisation measured here, large enough
to feed it.**

Three readings decide it, and each is independent of the other two:

1. **The parameter sweep bought a factor of 2.5, not a factor of 4.** Defaults
   top out at `2.454 x`; opening 122 parameterisations reached `6.061 x`. Even
   taking that number at face value it is **1.44x short**, and it is not face
   value (point 2).
2. **The highest reading is the feed, not the market.** `sh-roll` is
   `session-hold` over New York **16:00-18:00**, i.e. it holds the CME break and
   the reopen bar on every one of its 2,012 trades. The same rule on the two
   hours **immediately before** the break reads **`0.101 x`** — a **60-fold**
   collapse — and the 1 h 45 that contains **only** the break and the reopen bar
   reads **`5.079 x` / `17.476 x`**. `agent/hour-screen` measured the mechanism
   from the other side on the same feed (`open == low` on **19.02%** of
   XAUDUKA reopen bars against **3.20%** of ordinary bars, and near-absent on
   the broker's own XAUUSD).
3. **The frame-shaped rows are 4.6x short and they are the whole long-horizon
   family.** `quiet-swing` and `tsmom` at every hold and every selectivity
   measured land at `0.07 x` to `1.901 x`, which brackets `agent/rollover-flat`'s
   own `0.4-2.3 x` on the frame it had actually built.

**And the gap is now measured from the other end too.** `tsmom` at defaults,
guards off, earns **W = 17.778 spreads of gross per round trip (IS)** and
**53.155 (OOS)** — a real, large, two-window signal — at **0.04 / 0.03 round
trips per exposed session**, i.e. **25 to 33 sessions per trade**. Expressed
flat, one round trip per session, that becomes **G = 0.624 / 1.848**.
**The flattening does not lose the signal; it pays 25 to 33 spreads to collect
17.8 of them.** That is `rollover-flat`'s death certificate in two numbers.

---

## 1. The ruler, and the pre-checks. F2 and F3 did not fire; F2b did.

**The sieve.** `G = gross-of-spread per exposed session / spread of one round
trip`, both in the row's own R unit so the unit cancels; equivalently
`G x 0.28 = gross price points per exposed session`. Measured by running every
cell **twice**, identical but for `--spread=`:

    spreadR_per_trip = E(spread 0) - E(spread 0.28)
    gross_per_trip   = E(spread 0)
    trips_per_session = trades x S / time_in_market = S / mean_hold
    G = gross_per_trip x trips_per_session / spreadR_per_trip

`E` is read to **four** decimals from the `lbar_line` identity, not from the
3-dp `expectancy` column (addendum 8 section IV). `S` is `px-1s`'s own mean hold
in the same run and arm: **1371.2 / 1398.5 min** (guards off, IS / OOS) and
**1313.8 / 1340.4 min** (guards on).

**F2 — the ruler. Did not fire, and it calibrates on both windows.** `px-1s`'s
own `gross_per_trip` at one round trip per session **is** gold's one-session
drift, and it reproduces addendum 5 section B **to the published digit on both
halves**:

| | measured here | addendum 5 section B |
|---|---|---|
| IS 2010-06 -> 2018-06 | **-0.0040 R / session** | **-0.0040** |
| OOS 2018-06 -> 2026-06 | **+0.0205 R / session** | **+0.0205** |

Its own spread per round trip measures **1.09% of R (IS)** and **0.83% (OOS)**
against `agent/rollover-flat`'s published **1.15% / 0.83%**. The 5% gap on IS is
**against** the hypothesis: the frame needs `0.1 R` of gross, so on my number the
requirement is **9.2 x (IS)** and **12.0 x (OOS)**, not 8.7.

**F3 — parity with `agent/gross-ceiling`. Did not fire, on five anchors.**
Addendum 8 section II warns that this record often does not reproduce its own
numbers; here it did.

    px-1s   IS  2014 trades  PF_usd 0.912  expect -0.015  net -2753.66 USD  hold 1371.2 min
    qs-h15  IS   190         PF_usd 1.262  expect +0.130  dd 1085.94 USD = 8.38%
    ts-l20  IS   183         PF_usd 1.315  expect +0.095
    tsmom   IS    93         PF_usd 1.383  expect +0.151    OOS 90  2.183  +0.370
    rsi2-pull IS 7743        PF_usd 0.908  expect -0.063  dd 10436.45 USD = 99.45%
    gap-fade OOS  130        dd 28014.34 USD = 272.41%   (gross-ceiling's worst burned row, to the cent)

**F2b — trap 1, the stop-is-a-denominator invariance. FIRES, and the way it
fires is a finding.** `px-1s` at `riskDailyRanges` 0.5 / 1.5 / 4.0 is the same
rule at three R units spanning **7.8x** (R = **8.72 / 25.69 / 68.29 price
points**):

| arm | window | `px-1s-r05` | `px-1s` | `px-1s-r40` | spread of G | trades |
|---|---|---|---|---|---|---|
| guards **off** | IS | -0.3801 | -0.3670 | -0.3659 | **3.9%** | 2014 / 2014 / 2014 |
| guards **off** | OOS | +2.4462 | +2.4699 | +2.4839 | **1.5%** | 1982 / 1982 / 1982 |
| guards **on** | IS | -0.5658 | -0.8716 | -1.0233 | **81%** | 2013 / 2014 / 2014 |
| guards **on** | OOS | +2.9348 | +1.8929 | +1.9028 | **55%** | **1962** / 1982 / 1982 |

**In the no-guards arm the stop is a pure denominator, exactly as
`engine.rs:772-777` says: 0 trades change across a 7.8x stop and `G` moves
1.5-3.9%** (the residual is defect 16's `delta = 0.138` price points, which at
R = 8.72 is 1.6% of an R). **In the guards arm it is NOT a denominator.** The
guard line prints why: **`max_open_loss_r = 2.0`** is stated **in R**, so the
stop sets the loss cap, and **`notional 300%`** caps `lots`, which
`engine.rs:728-729` computes as `risk_usd / (risk x contract_size)`, so a
narrower stop buys more lots and runs into the cap. Measured consequence: the
trade count moves (**1982 -> 1962**) and `G` moves **up to 81%** on a change
that alters **no signal**.

⇒ **Addendum 7 section B is correct only in the arm it was measured in.** In the
only arm the owner permits, a self-managed "denominator" stop is behavioural,
and any cross-stop comparison there carries up to **81%** of unit artefact. This
is why no row in this plan sweeps `riskDailyRanges` as a strategy question.

**The record's own R unit, corrected.** `agent/gross-ceiling` derived R in
dollars as `|swap$| / |total_r_net - total_r|`. That is a swap-weighted harmonic
mean of a per-trade quantity (`engine.rs:958-963`), and on a row whose equity
collapses it diverges from the plain per-trade mean that `spread_usd/trades`
is. Measured on its own ceiling holder, `rsi2-pullback` IS guards off:

    swap route      spreadR 17.75% of R   implied R 1.58 price points
    printed realised stop                           3.10 price points
    MY SPREAD SWEEP          9.66% of R             2.90 price points

`engine.rs:717-727` settles it — `risk = |entry - stop|` in price and
`r = points / risk`. **The record's published leverage-free ceiling of `3.94 x`
is `1.774 x` on the same row and arm: it was 2.22x too high**, and the whole
default table moves the same way (`sess-hold` `1.77-2.47 x` -> **1.036 x**;
`quiet-swing` `0.98 x` -> `1.236 x`). Per brief section 8, these are my measured
numbers and they beat the brief's.

---

## 2. The ranking, as declared: `min(G_IS, G_OOS)`

`W` = gross per **round trip** in spreads = `G / trips`. `res` = `G` minus the
drift attributed from `px-1s` in the same run (section 3). `F7` marks a cell
where `--spread=0` moved the trade set.

### 2.1 Unrestricted, cells with >= 40 trades in both windows

| row | arm | Gmin | G_is | G_oos | W_min | trips is/oos | n is/oos | res is/oos | dd% oos | F7 |
|---|---|---|---|---|---|---|---|---|---|---|
| **`sh-roll`** | **on** | **6.061** | 6.061 | 11.085 | 0.537 | 11.29 / 10.82 | 2012 / 1982 | 6.93 / 9.19 | 4.6 | |
| `r2-rr05` | on | 5.990 | 6.965 | 5.990 | 0.352 | 16.61 / 17.04 | 8406 / 8071 | 6.96 / 5.95 | 90.6 | **Y** |
| `r2-sa10` | on | 4.736 | 7.202 | 4.736 | 0.160 | 28.37 / 29.56 | 10046 / 9745 | 7.18 / 4.69 | 97.6 | **Y** |
| `r2-sa10` | off | 4.320 | 5.233 | 4.320 | 0.221 | 18.18 / 19.54 | 11075 / 10729 | 5.20 / 4.12 | 100.1 | |
| `sh-asia` | off | **4.212** | 4.509 | 4.212 | 1.084 | 3.81 / 3.88 | 2056 / 2045 | 4.88 / 1.74 | 11.2 | |
| `r2-rr05` | off | 4.075 | 4.558 | 4.075 | 0.363 | 10.65 / 11.23 | 8663 / 8319 | 4.53 / 3.89 | 98.5 | |
| `sh-asia` | **on** | 4.037 | 4.085 | 4.037 | 1.084 | 3.65 / 3.72 | 2056 / 2045 | 4.96 / 2.14 | 11.2 | |
| `r2-lv2` | on | 3.984 | 9.541 | 3.984 | 0.372 | 9.95 / 10.71 | 1621 / 1579 | 9.57 / 3.97 | 33.5 | **Y** |
| `r2-tr800` | on | 3.106 | 3.542 | 3.106 | 0.301 | 10.10 / 10.32 | 8656 / 8416 | 3.55 / 2.83 | 93.6 | **Y** |
| `orb-rm30` | off | 2.944 | 2.944 | 5.823 | 0.312 | 9.44 / 9.65 | 1679 / 1694 | 2.94 / 5.76 | 45.5 | |
| `rsi2-pull` | on | 2.454 | 2.938 | 2.454 | 0.248 | 9.76 / 9.89 | 7537 / 7286 | 2.95 / 2.33 | 89.5 | **Y** |
| `rsi2-pull` | off | 1.774 | 2.303 | 1.774 | 0.256 | 6.68 / 6.92 | 7743 / 7477 | 2.95 / 2.33 | 98.6 | |
| `qs-lb3` | on | 1.901 | 1.901 | 2.386 | 4.365 | 0.44 / 0.45 | 494 / 452 | 1.95 / 2.08 | 8.7 | |

**The first thing in this table is not the order. It is that `Gmin` and `W` point
in opposite directions and no row has both.** Every row above `4 x` buys it with
**3.6 to 29.6 round trips per exposed session** and therefore has
`W <= 1.1`, i.e. **one round trip of that mechanism does not produce one round
trip of spread in gross**. `sh-roll`'s `W = 0.537` means **0.150 price points of
gross against a 0.280 point round trip**: it loses **0.130 points every time it
trades**, and its printed `expectancy` says so — **-0.0050 R (IS)** and
**+0.0002 R (OOS)**, `PF_r` **0.7479 / 1.0135**.

Conversely every row with `W > 4` holds for **2.2 to 33 sessions**, so making it
flat multiplies its spread bill by exactly that factor.

### 2.2 The DECLARED frame-compatible subset: `trips <= 1.5` in both windows

| row | arm | Gmin | W_min | trips is/oos | n is/oos | dd USD is/oos | `PF_r` is/oos | `Lbar` |
|---|---|---|---|---|---|---|---|---|
| `qs-lb3` | **on** | **1.901** | 4.365 | 0.44 / 0.45 | 494 / 452 | 880.24 / 1036.10 | 1.1865 / 1.1730 | 0.208 |
| `ts-l5` | on | 1.603 | 3.964 | 0.40 / 0.41 | 749 / 744 | | | |
| `ts-l20` | on | 1.402 | 4.470 | 0.31 / 0.32 | 612 / 618 | | | |
| `ts-l10` | on | 1.366 | 4.048 | 0.32 / 0.34 | 628 / 649 | | | |
| `qs-h2` | on | 1.271 | 2.284 | 0.54 / 0.56 | 521 / 480 | | | |
| `quiet-swing` | off | 1.236 | 6.067 | 0.20 / 0.22 | 314 / 296 | | | |
| `qs-h15` | off | 1.218 | 9.900 | 0.11 / 0.12 | 190 / 199 | 1085.94 / 1001.45 | 1.2991 / 1.2078 | 0.433 |
| `qs-qp020` | off | 1.151 | 5.521 | 0.21 / 0.23 | 183 / 170 | | | |
| `tsmom` | off | 0.624 | **17.778** | 0.04 / 0.03 | 93 / 90 | 687.08 / 673.49 | 1.4213 / 2.1760 | 0.358 |

**Nothing in this subset reaches a quarter of the 8.7 line, in either arm, at
any hold from 1 to 30 sessions or any selectivity from `quietPct` 0.20 to
0.85.** And the subset is the whole long-horizon family of the record. The best
of it, `qs-lb3`, is **4.6x** short.

**The hold lever is dead, and now it is measured rather than inferred.**
`quiet-swing` at `holdSessions` 1 / 2 / 5 / 15 / 30, guards off:
`G = 0.073 / 1.258 / 1.236 / 1.218 / 0.738`. `tsmom` at `lookbackDays`
5 / 10 / 20 / 60 / 120 / 240: `0.773 / 0.252 / 0.838 / 0.624 / -0.382 / (n<40)`.
**Gross is proportional to time held, so `G` is flat in hold** — exactly the
prediction `agent/gross-ceiling` implied when it measured `quiet-swing` h5 and
h15 at the same ratio, now tested across a 30x range of holds and confirmed.
`W` is the quantity that moves with hold (**2.3 -> 11.5** from h2 to h15) and
`W` is the quantity the frame cannot keep.

### 2.3 The 26 defaults, re-ruled

`Gmin`, guards off / guards on: `rsi2-pull` **1.774 / 2.454** · `quiet-swing`
1.236 / 0.881 · `sess-hold` 1.036 / 0.889 · `tsmom` 0.624 / 0.915 · `buy-hold`
(n=1) / 0.526 · `macd-cross` -0.298 / -0.076 · `intraday-mom` -0.423 / -0.337
· `stoch-rev` -0.550 / -0.827 · `donchian` -1.001 / -1.894 · `vwap-fade`
-1.021 / -1.124 · `crt` -1.072 / -2.292 · `far-stop` -1.101 / -1.070 ·
`rsi-rev` -1.149 / -1.648 · `bb-fade` -1.265 / -1.467 · `ema-cross` -1.957 /
-3.091 · `gap-fade` -2.052 / -1.966 · `keltner` -2.183 / -2.907 · `pdhl`
-2.648 / -4.218 · `squeeze` -2.731 / -2.883 · `orb` -3.147 / -3.257 ·
`doji-rev` -17.609 / -18.629 · `trend-pull` -16.408 / -24.105 · `ict-smf`
-10.471 / -12.181. **Four of 24 are positive on both windows.**

`vol-thrust` **0 trades**, `rsi-rev-vol` **0 trades**, `volman` **1 trade**:
**NOT MEASURED, not 0** (section 6).

---

## 3. Drift, subtracted

`px-1s`'s own `G` is the all-hours drift rate in the same spread units:

| arm | IS | OOS |
|---|---|---|
| guards off | **-0.367** | **+2.470** |
| guards on | **-0.872** | **+1.893** |

So **holding gold long through the whole session is worth +2.47 spreads of
gross per session on 2018-2026 and negative on 2010-2018** — 28% of the frame's
requirement on one half, and the wrong sign on the other. Addendum 5 section
B's drift sign flip is reproduced a third independent way.

**F5 — exposure, not signal.** Does not fire on the ceiling holder: `sh-roll`'s
residual is **+6.93 / +9.19** against a `G` of 6.06 / 11.09, i.e. the late New
York window carries far more gross per unit of long time than the average hour
— which is the true statement, and section 4 says where that gross comes from.
It **does** fire on the default `sess-hold` guards off OOS: `G` **+2.254**,
drift **+2.470**, residual **-0.216**, i.e. **110% of its OOS gross is the
all-hours drift** and the 09:30-16:00 window **under-collects** it. That
reproduces `agent/gross-ceiling`'s own F5 fire on the same row from the other
ruler. `sh-asia` OOS is **41% drift** (`G` 4.212, drift
+2.470, residual +1.742) and its IS residual is +4.876 against a negative
drift.

---

## 4. AMENDMENT DIAGNOSTIC: the ceiling holder's gross is a provider price stamp

`agent/hour-screen` measured, mid-run, that on **XAUDUKA** the reopen bar after
the 17:00 New York CME break has `open == low` on **19.02%** of bars against
**3.20%** of ordinary bars, and the pre-break bar has `close == low` on
**13.25%** against **1.88%**; the same on XAGDUKA (24.16% / 26.44%) and
**near-absent on the broker's own XAUUSD** (6.75% / 1.42%). That is a
**provider price stamp**, not a market.

`sh-roll` — the unrestricted ceiling holder — is `session-hold` over New York
**16:00 to 18:00**, so it holds the break and the reopen bar on **every** trade.
**None** of my 153 rows carries a `flat 16:30-18:15` filter (`filters = []`
throughout), so no row is immune by construction. Four diagnostic rows,
registered as amendment note 2 and **excluded from the ranking whatever they
printed**:

| row | New York window | relation to the break | G_is | G_oos | W_is | W_oos | n |
|---|---|---|---|---|---|---|---|
| `sh-roll` | 16:00-18:00 | holds break + reopen bar | **6.061** | **11.085** | 0.537 | 1.024 | 2012 / 1982 |
| `brk-only` | 16:30-18:15 | **only** the break + reopen bar | **5.079** | **17.476** | 0.477 | 1.707 | 1655 / 1655 |
| `brk-1800` | 18:00-20:00 | **enters at** the reopen bar | 2.215 | 3.946 | 0.211 | 0.390 | 2054 / 2045 |
| `brk-post` | 18:15-20:15 | entirely **after** the reopen bar | 2.526 | 2.101 | 0.241 | 0.207 | 2056 / 2049 |
| `brk-pre` | 14:00-16:00 | entirely **before** the break | **0.101** | **1.180** | 0.009 | 0.110 | 1949 / 1949 |

(guards arm; the no-guards arm reads the same shape — `brk-pre` 0.418 / 1.780
against `brk-only` 1.062 / 4.524.)

**The two hours immediately before the break, same rule, same side, same
sizing, same instrument, read `0.101 x` against `6.061 x`: a 60-fold collapse.
The 1 h 45 that contains nothing but the break and the reopen bar keeps
`5.079 x / 17.476 x` — more gross per round trip than `sh-roll` itself.** The
gross of the highest reading in this programme is **located in the one bar two
independent measurements call a provider artefact**, and on the broker's own
feed that artefact is 3-9x smaller.

⇒ **`6.061 x` is not read as a market number.** The highest reading whose
exposure excludes 16:00-18:15 entirely is **`sh-asia`** (New York 20:00-02:00
long) at **4.212 x / 4.037 x**, which is itself the nearest thing in this
programme to `agent/hour-screen`'s independent reading of **1.43-1.58 x** for
19:00-02:00 — their number is a per-round-trip one and my `sh-asia`
`W = 1.084-1.183`, the same quantity, the same order, two rulers, two agents.

**Not measured, and it matters:** what share of the `rsi2-pullback` family's
gross sits on the reopen bar. Those rows take 8,000-10,000 trades across all
hours, so decomposing them is a new programme, not a note. **Their `4.7-6.0 x`
is therefore an upper bound as well**, and in the direction against the
hypothesis.

---

## 5. Falsifiers

**F1 — FIRES.** Section 0. Under 8.7 on the declared frame-compatible subset
(**1.901**), under 8.7 on the unrestricted n>=40 reading (**6.061**), and under
8.7 even before section 4 removes that reading. Because a max over 612 cells is
biased **upward**, the negative conclusion is safe in exactly the direction the
registration said it would be.

**F1b — FIRES.** The frame-compatible ceiling is **1.901 x**, **4.6x** short.
Section 2.7 of the registration predicted this subset would contain **only**
self-managed rows, and it does: all **42** cells in it (n >= 40) are
`quiet-swing`, `tsmom` or `buy-and-hold` rows, **0** of them `Exits::Engine`. Measured confirmation
of the structural claim: under `max_hold_ms = 4 h` the **minimum** trips per
exposed session over all 466 readable `Exits::Engine` cells is **4.444**
(`r2-sa40`, guards off), so no engine-exit
parameterisation can be frame-compatible whatever its parameters.

**F2 — did not fire.** Section 1, 2 of 2 windows to the published digit.

**F2b — FIRES, in the guards arm only, and the mechanism is identified.**
Section 1.

**F3 — did not fire.** Section 1, 5 anchors.

**F4 — FIRES, hard.** **98 of 297 readable row-arms (33.0%) flip the sign of
`G` between the two halves**, against `agent/gross-ceiling`'s 34.7% measured
with a different ruler — an independent reproduction. And the single most
expensive instance in this record: **`r2-lv5sa05` guards on reads `+18.973` on
IS and `-22.981` on OOS** at 5,428 / 5,272 trades. **That cell clears 8.7 by
more than a factor of two on one window and is minus twenty-three on the
other.** Ranking on one window would have produced a passer. `min(IS, OOS)` was
declared in section 2 of the registration, before any number.

**F5 — fires on two rows, not on the maximum.** Section 3.

**F6 — FIRES on 86 of 594 cells, 49 row-arms, every one in the guards-off
arm.** Worst: `gf-fp50` OOS **273.06% of peak = 27,828.06 USD** on a 10,000 USD
book, net -27,636.05 USD over 130 trades; `gap-fade` OOS **272.41% =
28,014.34 USD**, which reproduces `agent/gross-ceiling`'s worst burned row to
the cent. `fs-sm1` OOS **203.77% = 20,659.04 USD** over 15,041 trades.
As declared, burned rows are excluded from the gate and **not** from the sieve,
because `G` is built from `E` in R and from price rather than from the equity
path — and the declared check on that holds: `rsi2-pull` guards off (dd 98.6%)
and `qs-lb3` guards on (dd 8.7%) differ in `G` by 0.9x, not by the 10x their
drawdowns differ by.

**F7 — FIRES on 168 of 594 cells, and the split is the finding.**

    guards OFF:   17 of 296 cells (5.7%)
    guards ON:   151 of 298 cells (50.7%)

The sweep is near-exact unguarded and path-dependent guarded, for the same
reason F2b breaks there: the open-loss cap and the notional cap read the equity
path, which the spread moves. Worst drift `bb-fade` guards on IS, **12,716 ->
10,053 trades (21%)**. **Every guards-arm `G` in section 2 marked `Y` is an
approximation with that mismatch stated**, and the two rows this report leans
on — `sh-roll` (2012 / 2012 and 1982 / 1982) and `sh-asia` (2056 / 2056 and
2045 / 2045) — are **clean on both windows**.

---

## 6. NOT measured

1. **Percentiles.** None published, none read. `--seeds=2` was passed only
   because the mode requires a number.
2. **The volume family, for ANY parameterisation.** By code read, 0 cells:
   `volume_thrust.rs:78` refuses on `mean <= 0.0`, `rsi_reversal_vol.rs:326`
   refuses on `vol <= mean x volMult`, and addendum 9 section III measures that
   Duka publishes `null` while the pipeline manufactures a single `0.0`. With
   `mean = 0` the first gate can never open and the second is `0 <= 0` for
   **every** multiplier including negative ones. Confirmed by the receipts:
   `vol-thrust` **0 trades**, `rsi-rev-vol` **0 trades**, both windows, both
   arms. **NOT MEASURED on this feed, not rejected.**
3. **`volman-box` at defaults and at `boxes = 1`: 1 trade.** Not a measurement.
   Loosened on all three gates (`vb-loose`) it takes 5,422 / 5,570 and reads
   `Gmin = -1.630` (guards off) and `-1.849` (guards on).
4. **18 of the 612 declared cells carry no reading**: `vol-thrust` (0),
   `rsi-rev-vol` (0), `volman` (1), `vb-b1` (1) — 16 cells — and `buy-hold`
   guards off (1 trade, a single 16-year hold) — 2 cells. **594 readable.**
5. **`im-mm10` is not a measurement.** `intraday-momentum` with `minMove = 1.0`
   prints the largest `min(G)` in the whole programme — **46.921 x** guards on,
   **45.407 x** guards off — on **9 and 6 trades**. That is 4.4 to 6.7x under
   the record's 40-trade floor. It is reported here, in full, as **the number
   the sieve would have returned if the floor were not enforced**, and as
   nothing else.
6. **`max_hold_ms`.** Not changed; no CLI flag exists. The consequence is
   reported (F1b), not engineered around.
7. **Carry.** Swap is **0.00 / 0.00** in the `xauduka` config, so no receipt
   here charges financing, and `sh-roll` crosses the 17:00 New York rollover on
   **every** trade — the one thing the frame exists to forbid. Independently,
   `agent/hour-screen` measured `swap$ = 0` on all 32 cells held wholly between
   two 17:00 marks, which is the record's third confirmation that **carry is
   genuinely avoidable and the frame closes on SIGNAL**.
8. **Intrabar excursion.** Every drawdown is the closed-trade curve. `avg_mae`
   is printed: `sh-roll` **-0.048 / -0.043 R**, `sh-asia` **-0.121 / -0.139 R**,
   `qs-lb3` **-0.397 / -0.385 R**, `rsi2-pull` **-0.648 / -0.649 R**.
9. **Any instrument or bar size but `xauduka` 15m.** Every number is a 15m
   number, and section 4 is a reason to doubt the feed itself.
10. **`wrong_side_stop`** — no counter exists on this tree (addendum 8 section
    III.2). No 0 is read from it.
11. **`data-sealed/`.** Not opened, read, pointed at or counted.
12. **The live spread at the session break.** Flat 0.28 at every hour here. A
    real venue widens it most at exactly the break `sh-roll` lives in, so that
    row's economics are worse than printed, not better.

---

## 7. `cap_lots` — and a 0 this report printed before it was caught

The registration promised the notional-cap share on any quoted row. The first
pass of my parser looked for a string **the engine does not print** and
returned **`0 cells ride the notional cap`**. The engine prints
`guards: refused ...; closed ...; sized down N`. Corrected:

    486 of 618 guarded prints have `sized down` > 0

and it bites hardest on exactly the high-`G` engine rows, at the
**trade-count share**, guards on, IS / OOS:

    vw-sa04      10186/10188 = 100.0%        r2-lv5sa05  5422/5428 = 99.9% / 99.7%
    r2-sa10       9907/10046 =  98.6% / 97.2%  sess-hold  1979/2062 = 96.0% / 94.3%
    r2-rr05       6790/ 8406 =  80.8% / 77.3%  rsi2-pull  6044/7537 = 80.2% / 77.1%

**Both gate legs are blind to this** (`r = points/risk` does not read `lots`).
The rows this report leans on are clean: `sh-roll`, `sh-asia`, `qs-lb3`,
`qs-h15`, `tsmom`, `px-1s` and every diagnostic row print **sized down 0**.
Also worth the line: guards-on `sess-hold` is closed by **`OPEN_LOSS_CAP` on
886 of 2,062 trades (43%)** and `r2-rr05` has **`COOLDOWN` refusing 175-185
entries** — guard activity, not mechanism.

**Reporting the parse bug rather than the 0 is the point.** Brief section 8:
`null` is not `0`.

---

## 8. The gate: 0 cells

**No cell passed the sieve, so no gate cell was spent — as registered.** The
verdict strings the tool printed are in the receipts and are read as context
only; no percentile is published anywhere in this report.

For context, counted **by hand at 40** and not at the tool's `need 30`: the
nearest thing to a gate pass in the frame-compatible subset is **`qs-lb3`
guards on**, which misses **both** legs on **both** windows —
`PF_r 1.1865 / 1.1730` against 1.200 and `E +0.0387 / +0.0361 R` against
+0.050 R, at 494 / 452 trades, `Lbar 0.208 / 0.209` so **the expectancy leg
binds** (addendum 9 section V.1: redundant only when `Lbar >= 0.250 R`).
`PF_usd` reads `1.190 / 1.171` beside `PF_r 1.1865 / 1.1730` — USD **higher**
by **+0.0035** on IS and **lower** by **-0.002** on OOS. Same row, same method,
and **the gap changes sign between the two windows**: one more reading against
quoting any fixed direction for it (addendum 9 section I).

---

## 9. Multiplicity ledger: declared vs viewed

| | declared | viewed |
|---|---|---|
| rows | 153 | **153** |
| windows | 2 | **2** |
| guards arms | 2 | **2** |
| sieve cells | 612 | **612 run, 594 readable, 18 with <= 1 trade** |
| spread legs per cell | 2 | **2** |
| printed rows | 1,224 in 8 runs | **1,224 in 8 runs** |
| pre-check cells | 4 | **4** |
| **gate cells** | 0 now, passers only | **0** |
| amendment diagnostic (section 4), EXCLUDED from the ranking | not declared in advance | **16 new cells + 8 re-measurements of declared rows, in 8 runs** |

No row was added to the ranking, no window changed, no arm added, no parameter
tuned, no threshold moved, no sample floor lowered. **Two things are introduced
after the numbers were seen and are labelled where they appear:** `W = G/trips`
(a ratio of two columns section 8 of the registration already declared), and
the four diagnostic rows of section 4, which were prompted by another agent's
measurement arriving mid-run and are excluded from the ranking whatever they
printed.

---

## 10. If the owner wants the one paragraph

**There is no signal in this registry big enough for the `rollover-flat` frame,
at any of the 153 parameterisations measured over 612 declared cells.** The
frame needs **8.7** round-trip spreads of gross per exposed session — **9.2 to
12.0** on my own measurement of the same quantity — and the frame-shaped
ceiling is **1.901**, which is where the whole long-horizon family sits at
every hold from 1 to 30 sessions. The one reading that got to **6.061** is
`session-hold` across the CME break, whose gross collapses **60-fold** two
hours earlier and which two independent measurements locate in a Dukascopy
price stamp that is 3-9x smaller on the broker's own feed. The gap is also now
measured from the other side: `tsmom` really does earn **17.8 spreads of gross
per round trip**, but it takes **25 sessions** to do it, and flattening it pays
**25 spreads to collect 17.8**. **Carry was never the problem and the frame was
never wrong. The desk should stop paying to fix carry on gold 15m, and spend on
cost or on another market instead.**
