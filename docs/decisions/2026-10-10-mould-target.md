# Registration: does ANY parameterisation of an existing registry mechanism reach a gross of 8.7 round-trip spreads per exposed session, on BOTH windows?

Registered **2026-10-10**, branch `agent/mould-target` (cut from
`agent/stop-width` at `29c8635`). Committed **alone**, before the design file
and before the first `xauduka` cell of this branch exists.

Briefs: `/e/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` and addenda **5, 6, 7, 8, 9**.
All six apply in full, and addendum 9 supersedes where it says so.

## 0. The question, and why it is not a grid search

`agent/rollover-flat` built a frame that works: `tsmom-flat` /
`quiet-swing-flat` + `crosses_rollover()`, with an 80-session walk test that
demands `swap_nights(in, out) == 0` on **every** closed trade. On its own
measurements that frame
**avoids the carry** (`swap$/spread$` **0.123x-0.190x** against **29.0x-113.2x**
on the continuous parents; overnight financing down to **3.4%**),
**keeps the signal** (gross per session **66-116%** of the parent on 7 of 8
pairs), **beats the continuous expression on 8 of 8 pairs** (+0.133R to
+0.731R), and **lives in the only arm the owner permits** (`WEEKEND_FLAT`
**0-1 in 1,157-1,944 trades**, against 395-478 per 1,000 on the parents).

What kills it is **cost**: it pays the spread 6-20 times for a gross of only
**+0.0036 to +0.0224 R per session**, so the spread eats **44-89%** of
everything. The frame therefore needs a signal of about **0.1 R of gross per
exposed session** — roughly **10x** what exists.

`agent/gross-ceiling` then measured that ceiling across 12 mechanisms, but
**only at DEFAULT parameters**, and said so in its own section 9.4. Its
leverage-free ceiling came out at **3.94x** against a requirement of
**~8.7x**. This job asks the question it left open, in one sentence:

> **Is there any parameterisation of any mechanism already in this registry
> whose gross per exposed session reaches 8.7 round-trip spreads on BOTH
> time-split windows?**

**It is a sieve, not a gate programme.** The screen is `gross/spread`, **not
PF**. A mechanism under the ceiling is dead by arithmetic and **needs no PF
measurement at all**, so this plan may be **wide** without being dishonest,
provided (a) the multiplicity is declared before the first run and (b) it is
said plainly that **passing the sieve proves nothing** — it only says which
cell is worth spending a gate cell on. Gate cells are spent on sieve passers
and **on nothing else**.

## 1. Hypothesis, one sentence

On this feed there exists at least one parameterisation of at least one
registry mechanism whose **gross-of-spread per exposed session, taken as the
minimum of the two time-split windows, reaches 8.7 round-trip spreads**, and
if there is one it is identifiable and nameable.

## 2. THE SIEVE, defined exactly, and why it is leverage-free AND spread-free

### 2.1 The quantity

    G  =  gross-of-spread per exposed session  /  spread of one round trip

both in the **same** R unit, so the R unit **cancels**. Written in price:

    G  =  (gross price points captured per exposed session) / 0.28

### 2.2 Where 8.7 comes from, and why the number does not depend on the spread

The frame needs **>= ~0.1 R of gross per exposed session**, quoted in `px-1s`'s
R unit (1.5 x the 20-day mean New York day range). `px-1s`'s spread per round
trip measures **1.09-1.15% of its own R** on these windows. So

    requirement  =  0.1 R / 0.0115 R  =  8.7 round-trip spreads
                 =  8.7 x 0.28        =  2.44 price points of gross per session

**The criterion is invariant to the spread assumption.** If the real round trip
is 0.22 rather than 0.28 (the code comment at `search.rs:194-196` says the
logger's p50 is 0.220 with a maximum of 0.260, and that the configured 0.28 was
a single terminal read), then every measured `G` rises by 0.28/0.22 = 1.27x
**and so does the requirement**, because the requirement is fixed in R and the
spread in R shrinks by the same factor. Addendum 9 section II is therefore
satisfied by construction here: the sieve does not rest on a spread number at
all, only on the identity `G x 0.28 = gross price points per exposed session`.

### 2.3 How G is measured: a SPREAD SWEEP, not a swap arm

Each cell is run **twice**, identical in every respect except `--spread=`:

    run X:  --spread=0.28   (the configured round trip)
    run Y:  --spread=0.0

From the `lbar_line` patch on this branch each run prints `E` to **four**
decimals (`identity E = Lbar(PF_r-1) = <x> R vs expectancy <x> R`), which
addendum 8 section IV requires because the printed 3-dp `expectancy` is
**coarser than the quantity** when `gross` is a difference of two near-equal
numbers. Then

    spreadR_per_trip  =  E_Y - E_X
    gross_per_trip    =  E_Y                       (spread is INSIDE expectancy,
                                                    swap and commission are not)
    trips_per_session =  trades_X x S / time_in_market_X
    S                 =  px-1s mean hold in the SAME run and arm
    gross_per_session =  gross_per_trip x trips_per_session

    G  =  gross_per_session / spreadR_per_trip
       =  (trades_X x S / time_in_market_X) x E_Y / (E_Y - E_X)

### 2.4 Why the swap-derived route is NOT used, measured before the first cell

`agent/gross-ceiling` derived its R unit in dollars as
`risk_usd = |swap$| / |total_r_net - total_r|` from a swap arm, then read
`spreadR = (spread_usd/trades)/risk_usd`. That identity holds **only if
`risk_usd` is constant across trades**: `engine.rs:958-963` gives
`risk_usd_i = risk_i x lots_i x contract_size` per trade, so
`|swap$|/|dtotal_r|` is a **swap-weighted harmonic mean** of `risk_usd_i` while
`spread_usd/trades` is a **plain mean** of `0.28 x lots_i x cs`. On a row whose
equity path collapses, the two weightings diverge.

**Measured, 0 gate cells, on the record's own ceiling holder.** `rsi2-pullback`
IS, guards off: the swap route gives `spreadR = 17.75% of R` (implied R = 1.58
price points), while the engine's own printed
`the method's own realised stop: median 2.096 ATR = 3.10 points` gives
**9.03%**, and my spread sweep gives **9.66%**. `engine.rs:717-727` settles it:
`risk = |entry - stop|` in price and `r = points/risk`, so **R in price IS the
realised stop distance** and the swap route is wrong **by a factor of 1.84** on
that row. That row has a 99.45% drawdown, which is exactly where the weighting
diverges.

**Consequence, declared now:** the record's published ceiling of **3.94x** is
expected to come out **near 2.1-2.3x** on this ruler, i.e. **further from 8.7,
not closer**. Brief section 8 applies — my measured number wins and is reported
as beating the brief's.

### 2.5 Why the spread sweep is accurate even at a narrow stop

Defect 16 (`risk = f x range + delta`, `delta = 0.138 USD` = exactly half the
round trip) makes `risk_i` depend on the spread for any strategy that supplies
an **explicit** stop computed from the signal bar's close. At `--spread=0`,
`delta = 0`. To first order in `delta/rho`:

    E_Y - E_X  ~=  0.28 x mean(1/rho_i)  +  delta x mean(p_i/rho_i^2)

and the second term is about `0.5 x E` of the first. With `|E| < 0.3` the
contamination is **under 2%** at any stop width, which is why the sweep is the
primary ruler and the realised-stop line is only a cross-check. Verified
already on the probe: `rsi2-pullback` printed realised stop **2.096 ATR** at
spread 0.28 and **2.000 ATR** at spread 0, a 4.8% shift, while the three
estimates of `spreadR` span 9.03-9.66%.

### 2.6 The shape G cannot see, declared now

    trips_per_session  =  S / mean_hold

so `G` rises when the hold shortens **if gross per trip does not shorten with
it**. A 15-minute-hold row needs only 0.027 price points of gross per trade to
reach `G = 8.7`, and it would pay **91 round trips per session** doing it. The
`rollover-flat` frame pays **one**. So the ranking is published **twice**, both
orderings declared now and neither chosen after the fact:

* **unrestricted** — every readable cell;
* **frame-compatible** — only cells with `trips_per_exposed_session <= 1.5`,
  the same subset `agent/gross-ceiling` declared, for the same reason.

`trips_per_session` and `spread/gross` are printed **beside every row, always**.

### 2.7 A structural fact about which rows can be frame-compatible at all

`[trading] max_hold_ms = 14_400_000` (4 h) force-closes every `Exits::Engine`
position, so `mean_hold <= 240 min` and `trips_per_session >= S/240 ~= 5.7` for
every engine-exit row in this registry. **No `Exits::Engine` parameterisation
can be frame-compatible under this cap, whatever its parameters.** The cap is
an engine setting with **no CLI flag**, and raising it would make every number
in `docs/decisions/` incomparable (the receipt line exists precisely to say
so). So it is **not changed and not measured here**, it is declared as the
owner's decision, and the prediction it implies is tested the cheap way
instead: by the hold sweeps on the three **self-managed** families, where hold
is a real parameter.

## 3. Two unit traps, checked in the CODE before any cell is spent

**Trap 1 — which "stop" is a stop.** `engine.rs:772-777` opens `check_exit`
with `if position.self_managed { return None }`. Read from
`grep -n 'Exits::' crates/fd-strategy/src/`:

* `Exits::Strategy` (stop is a **DENOMINATOR**, never enforced):
  `buy-and-hold`, `intraday-momentum`, `quiet-swing`, `session-hold`, `tsmom`.
* `Exits::Engine` (stop is **ENFORCED**): everything else.

On the first group, widening or narrowing the stop changes **0 signals, 0
trades, 0 USD** — only the unit — and because gross per trade **and** cost per
trade in R both scale as `1/f`, **`G` is exactly invariant**. So no row sweeps
`riskDailyRanges` as a strategy question. Three rows sweep it **as a
pre-check** (`px-1s`, `px-1s-r05`, `px-1s-r40`) so the invariance is
demonstrated rather than assumed. For `tsmom` and `quiet-swing` the real lever
is **hold** (`lookbackDays`, `holdSessions`), and for `session-hold` it is the
**window**.

**Trap 2 — defect 16, half the spread inside the risk unit.** Handled in
section 2.5. Any row whose realised stop is under 2.0 price points has
`delta` over 7% of an R; such rows are flagged and their `spreadR` is reported
by **both** routes.

## 4. DRIFT, subtracted the way the record does it

Addendum 5 section B: gold's one-session drift is **-0.0040 R (2010-2018)** and
**+0.0205 R (2018-2026)** in `px-1s`'s R unit, and the sign **flips** between
the windows. In this plan `px-1s`'s own `E_Y` **is** that number, measured in my
own run (the probe already printed **-0.0040** on IS, to the published digit),
so the drift anchor is not carried in from a brief.

    drift_attributed_per_session(row)
        = signed_share_of_exposure(row) x G(px-1s, same run)
    residual_G(row) = G(row) - drift_attributed_G(row)

`G` is already in spread units, which are the **same** units for every row on
this feed (a flat 0.28 at every hour), so **no R-unit conversion is needed for
the drift subtraction** — this is the one place the sieve is strictly cleaner
than the record's ruler, which had to convert through `spreadR` ratios and said
it carried a few percent of unbounded error doing so.

## 5. Rows, windows, arms — frozen

**153 rows**, in `docs/research/designs/2026-10-10-mould-target.toml`, committed
after this file and never edited afterwards:

| group | rows |
|---|---|
| anchors and pre-checks (`px-1s`, `px-1s-r05`, `px-1s-r40`, `qs-h15`, `ts-l20`) | **5** |
| registry mechanisms at DEFAULTS | **26** |
| `rsi2-pullback` parameterisations | **14** |
| `stoch-reversal` | 7 |
| `macd-cross` | 6 |
| `rsi-reversion` | 7 |
| `bb-fade` | 6 |
| `donchian-breakout` | 4 |
| `keltner-break` | 5 |
| `ema-cross` | 4 |
| `squeeze-break` | 4 |
| `crt` | 5 |
| `pdhl` | 5 |
| `orb` | 6 |
| `vwap-fade` | 4 |
| `trend-pullback` | 4 |
| `doji-reversal` | 5 |
| `far-stop-break` | 5 |
| `gap-fade` | 5 |
| `volman-box` | 3 |
| `ict-sweep-mss-fvg` | 2 |
| `tsmom` (hold) | 5 |
| `quiet-swing` (hold and selectivity) | 8 |
| `session-hold` (windows) | 6 |
| `intraday-momentum` | 2 |

Sweeps are **one parameter at a time** around each mechanism's registered
default, plus **four** declared two-parameter corners (`r2-lv5rr4`,
`r2-lv5sa05`, `pd-m1rr30`, `crt-t2rr30`, `qs-qp020h1`, `vb-loose` — six, and
they are named here so none can be added later).

**Excluded, with the reason:**

* `level-reversion`, `maxpain-magnet`, `flow-momentum`, `flow-at-level` —
  `needs_options()`, and the header prints
  `timeline: none - options strategies will be skipped`. **4 mechanisms.**
* `external` — reads a signal file that does not exist for this question.
* `companion-unconfirmed` — takes no trade without `--companion=`.
* **every `volMult` parameterisation of `volume-thrust` and
  `rsi-reversal-vol`** — dead for **every** value on this feed, by code read,
  **0 cells spent**: `volume_thrust.rs:78` refuses on `mean <= 0.0` and
  `rsi_reversal_vol.rs:326` refuses on `vol <= mean x volMult`, while addendum
  9 section III measures that Duka publishes `null` and the pipeline
  manufactures a single `0.0`. With `mean = 0` the first gate can never open
  and the second is `0 <= 0` for every multiplier, **including negative ones**.
  Both are kept at defaults as **one row each**, to print the 0 and show it is
  a 0 trade count and not a 0 result. **This family is NOT MEASURED on this
  feed, not rejected.**

**Windows, the record's own two halves, split by time:**

    IS   --from=2010-06-01 --to=2018-06-01
    OOS  --from=2018-06-01 --to=2026-06-01

**Arms:**

| arm | guards | what it is for |
|---|---|---|
| **A** | off | the sieve, the unguarded gross |
| **C** | **on** | the sieve in the **only** arm the owner permits (addendum 5 section D) |

Both are run and both are published. Addendum 9 section I measures that the
`PF_usd` - `PF_r` gap is itself decided by the arm, and brief section 0 item 4
measured that 5.8% of paired rows cross PF 1.200 on the guards flag alone.

**No swap arm is run**, because section 2.3 shows the sieve does not need
`risk_usd` and section 2.4 shows the swap-derived route is wrong where it
matters most. Carry is **not** part of this question: the frame already solves
it, and these runs are at `swap = 0.00/0.00` (the `xauduka` config value).

## 6. Multiplicity ledger — declared NOW, counted BEFORE the first run

| | declared |
|---|---|
| rows | **153** |
| windows | 2 |
| guards arms | 2 |
| **sieve cells** (row x window x arm) | **612** |
| spread legs per cell (0.28 and 0.0) | 2 |
| **printed rows in total** | **1,224**, in **8 runs** |
| pre-check cells already spent (2-row timing and parity probe, IS, arm A, both spread legs, seeds 2 and 4) | **4** |
| **gate cells** | **0 now**; spent only on sieve passers, and only on those |

**No row is added, no window changed, no arm added, no parameter tuned, no
threshold moved and no sample floor lowered after a number is seen.**
Amendments are dated notes appended to section 12; no line above is rewritten.

**The selection bias of a maximum, stated now.** The headline is a **max over
612 cells**, so it is biased **upward**. That asymmetry is in the falsifier's
favour: if the max comes out **below** 8.7 the negative conclusion is **safe**,
because the bias could only have pushed it up. If it comes out **above**, it is
reported as *"the largest of 612 cells over 153 rows"* and **is not a
candidate**; it earns a gate measurement, nothing more.

## 7. Falsifiers — specific and firable

**F1 — THE ONE THIS JOB EXISTS TO FIRE.** If
`max over all 612 cells of min(G_IS, G_OOS)` is **< 8.7**, then **no
parameterisation of any existing mechanism can feed the `rollover-flat`
frame**, and the frame is dead — not because it is wrong, but because there is
**no signal large enough to put in it**. The desk stops paying to fix carry on
this instrument at this bar size and goes and asks about **cost** or about **a
different market**. *Fires on:* that max being under 8.7. **This is the most
valuable reading available, including when it is negative**, and it closes a
direction the record has spent three nights on.

**F1b — the frame-compatible branch.** The same statement restricted to
`trips_per_exposed_session <= 1.5`. Section 2.7 predicts this subset contains
**only** self-managed rows. Both numbers are published whatever they say.

**F2 — the ruler. A PRE-CHECK, partly already run.** If `px-1s`'s `E_Y` does
not reproduce addendum 5 section B's published one-session drift
(**-0.0040 R** on IS, **+0.0205 R** on OOS) to the printed digit, the ruler is
not calibrated and **no new number is read**. *Already run on IS:* the probe
printed `E_Y = -0.0040 R`, and `px-1s` reproduced `trades 2014 / expect -0.015 /
net -2753.66 USD / mean hold 1371.2 min` against the record. OOS is checked in
the first full run.

**F2b — trap 1, the invariance.** If `px-1s`, `px-1s-r05` and `px-1s-r40` do
**not** print the same `G` to within 2%, then either `Exits::Strategy` does not
mean what `engine.rs:772-777` says, or defect 16 is larger than section 2.5
bounds it — and in either case every stop-related reading in this report is
void. *Fires per window.*

**F3 — parity with the record.** If `qs-h15`, `ts-l20` and `rsi2-pull` do not
reproduce `agent/gross-ceiling`'s printed `trades` / `expect` / `spread paid` /
`mean hold`, the binary or the settings differ. *Already checked on two rows
(`px-1s`, `rsi2-pull`), both exact.* Addendum 8 section II says a mismatch is
**normal in this record** — if one appears it is **reported as the record not
reproducing itself**, not silently absorbed.

**F4 — artefact #10, the signal family; and artefact #13.** Addendum 6 section
V and addendum 9 section IV: a high `G` in **one** window is worthless, and
`agent/gross-ceiling` measured that **17 of 49 readable row-arms (34.7%) flip
the sign** of this very quantity between the halves. *Fires on:* the row with
the highest **single-window** `G` having a **negative** `G` in the other. If it
fires, that row is written up as a further instance of artefact #10 and the
ranking stands on `min(IS, OOS)`, which is why `min` is declared here and not
after.

**F5 — exposure, not signal.** If a sieve passer has `residual_G` within
**20%** of zero in **either** window after the section 4 drift subtraction, it
is reported as **drift exposure re-expressed**, not as a mechanism.
`agent/gross-ceiling` fired this on `sess-hold` (101.4% drift, OOS) and
`buy-hold` (under-collects its own drift), so it is a live, not a decorative,
falsifier.

**F6 — a burned row cannot be read.** Defect 14: a row printing
`max_drawdown_pct > 100%` has blown the account and kept trading at `min_lot`,
so its `PF_usd` is flattered. Such rows are **excluded from the gate** and
listed separately with their percentage. They are **not** excluded from the
sieve, because `G` is built from `E` in R and from price, not from the equity
path — and saying so is itself a claim this plan tests: if the sieve's `G` for a
burned row disagrees with a non-burned sibling of the same mechanism by more
than the parameter change can explain, that is reported. **Declared now so
neither the inclusion nor the exclusion is a choice made after seeing which
rows it touches.**

**F7 — the sweep moved the trade set.** The sweep is exact only if `--spread=0`
does not change which trades happen. *Fires per row:* `trades_X != trades_Y`.
On the probe, `px-1s` (2,014) and `rsi2-pullback` (7,743) were **identical**,
with identical exit mixes (`STOP 2778 / TARGET 3022 / TIMEOUT 1942`) and
identical mean holds. Any row where the counts differ is reported with both
counts and its `G` is quoted as an **approximation with the mismatch stated**.

**F1, F1b, F2b, F4, F5, F6 and F7 can all fire. Every reading is written
whatever it says.**

## 8. What is reported, declared now

Per row, per window, per arm: `trades` (both spread legs), `E_X`, `E_Y`,
`spreadR_per_trip`, `gross_per_trip`, `trips_per_session`, **`G`**,
**`spread/gross` as a percent**, `net_per_session`, `drift_attributed_G`,
`residual_G`, `max drawdown USD` and `_pct`, `avg_mae`, `long share`,
`signed share of exposure`, `mean hold`, the **exit mix**, and the realised
stop in points where the engine prints one.

* **Drawdown in USD beside every quoted return**, with `_pct` quoted as a
  **floor** (it divides by the curve's highest equity).
* **`PF_r` beside `PF_usd` with the unit named**, from the `lbar_line` patch,
  on every gate row. Addendum 9 section I: the direction of the gap is **not**
  a constant of this engine, so it is measured per row and no ratio is quoted
  from another agent.
* **Exit mix read on every quoted row.** A row whose own rule fires **0
  times** — exits all `WEEKEND_FLAT` / `NEWS_FLAT` / `END_OF_DATA` /
  `OPEN_LOSS_CAP` / `window closed` — is reported **void** whatever its `G`
  (brief section 6a; `agent/gross-ceiling` voided `buy-hold` guards-on at
  `+0.3237 R` on exactly this check).
* **`cap_lots`**: if a row rides the notional ceiling, the share of trades is
  stated, because **both gate legs are blind to it**.
* **The gate, on sieve passers only**, both windows, both arms, `PF_r` beside
  `PF_usd`, `E = total_r/n`, drawdown USD, `--exit-mix` with the rule-fires
  check, and the **40-trade** floor **counted by hand** (the tool's verdict
  says `need 30`).

## 9. NOT measured, declared now so it cannot be claimed later

1. **Percentiles.** None is published and none is read.
   `agent/gross-ceiling` measured that the **same method in two batch
   positions** printed null `p95` **1.297 vs 2.036** and a **different verdict
   string**; addendum 6 section VI measured that the null's width moves with
   `1/sqrt(n)`. `--seeds=2` is passed only because the mode requires a number;
   `null p50 = 0.000` is read as **did not calibrate**, never as "the median
   does not profit".
2. **Carry and the swap arm.** Not run. Out of scope: the frame has solved it.
3. **Drawdown as a criterion.** Reported beside every quoted row, **ranked on
   by nothing**, no threshold offered.
4. **Intrabar excursion.** Every drawdown here is the **closed-trade** curve;
   `avg_mae` is the only field that sees an open position go against the book
   and is printed unconverted.
5. **Any instrument or bar size but `xauduka` 15m.** Brief section 0 item 3
   measured that the **sign** of an edge can be a property of the 15m ruler
   (+0.114R -> -0.065R -> -0.043R on the same entries at three bar sizes).
   Every number here is a **15m** number.
6. **`max_hold_ms`.** Section 2.7. Not changed, no flag exists, and the
   consequence is reported rather than engineered around.
7. **The volume family.** Section 5. **NOT MEASURED on this feed, not
   rejected.**
8. **`wrong_side_stop`.** Addendum 8 section III.2: `grep -rn wrong_side_stop
   crates/` returns **0** on this tree, so there is **no counter** and no 0 will
   be read from one.
9. **`data-sealed/`.** Not opened, read, pointed at or counted.
10. **The live spread at the session break.** A flat 0.28 at every hour here; a
    real venue widens it exactly where a per-session book re-enters. Section
    2.2 makes the sieve spread-invariant, but any **net** number quoted in the
    report is therefore a **lower bound on the real cost share**, and that
    asymmetry is against the hypothesis.
11. **New mechanisms.** None is written. The question is about the registry
    that exists.

## 10. Settings, frozen

    --mode=hypotheses --fixed --exit-mix --seeds=2
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --config=/e/rust/fd-mould-target/config
    --batch-file=/e/rust/fd-mould-target/docs/research/designs/2026-10-10-mould-target.toml
    --from=<window> --to=<window>
    --spread=0.28  or  --spread=0.0
    --guards                                   (arm C only)

Binary: **`/e/rust/fd-stop-width/target-sw/release/search.exe`**, built
2026-10-08 20:57 from `agent/stop-width` at `29c8635`, which is **this
worktree's own HEAD** — so the binary and the code are the same program and
**nothing is built** (brief section 1). It is the only shared binary carrying
`expectancy_net`, the drawdown line **and** `lbar_line`, and this plan needs
all three. `/e/rust/fd-drawdown/` and `/e/rust/fd-rollover-flat/` no longer
exist on disk; `/e/rust/fd-gross-ceiling/` does not either, so its receipts are
read from the `agent/gross-ceiling` branch.

Every receipt must reproduce the
`flags: N passed, every one of them read by --mode=hypotheses` line, and the
`news:` source line is reproduced in the report because the engine reads
`data/news/events.parquet` from the **default** `data/` whatever `--data=` says.

`--samples=`, `--direction-samples=`, `--rebate-share=`, `--trail=`,
`--params=`, `--filters=`, `--strategy=`, `--batch=`, `--companion=`,
`--null-sides=` and `--null-registered-stop` are **not passed**: none is read
by `--mode=hypotheses` for this plan, `--null-sides=` is inert outside
`hypotheses`-with-a-null-we-publish, and no percentile is read anyway.

Not touched: `config/accounts.toml` outside this worktree, `config/local.toml`,
the VPS 103.19.29.194, `main`, `data/gold/`, `data/btc/`,
`/e/rust/flowdesk/target/`, and the two `collect.exe` processes (pids 5044 and
38720), which are not signalled. No `target/` is deleted.
`df -h /e` before the first run: **16 GB available**.

## 11. How to read the result

One number is the result: **`max over 612 cells of min(G_IS, G_OOS)`**, with
the parameterisation that holds it named, and beside it its
`trips_per_session`, its `spread/gross`, its drawdown in USD, its exit mix and
its post-drift residual.

* **under 8.7** — F1 fires. **The `rollover-flat` frame is dead for want of a
  signal**, and the ledger of 612 declared cells says how hard it was looked
  for. The desk stops here and spends on cost or on another market.
* **8.7 or over, with `trips_per_session > 1.5`** — the signal exists but not
  in a shape the frame can hold. Reported with the trip rate and the net per
  session, which will be deeply negative, and **not** proposed.
* **8.7 or over, with `trips_per_session <= 1.5`** — the first cell in this
  record worth a gate measurement for this purpose. Gate measured on **that
  cell and nothing else**, two windows, both arms, and **handed to the owner**.
  It is not proposed, and whether it goes into `rollover-flat` is the owner's
  call.

## 12. Amendments
