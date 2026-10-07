# Long-horizon signals, flat across every 17:00 New York rollover — result

Registration: `docs/decisions/2026-10-07-rollover-flat.md`, committed **alone**
at `4bf8457`, before the first line of strategy code and before any `xauduka`
number existed on this branch. Two dated amendments appended (section 13),
both before the gold runs.

Rows: `docs/research/designs/2026-10-07-rollover-flat.toml`, **8, frozen**.
Binary: **built on this branch** at `--release` with
`cargo +stable-x86_64-pc-windows-gnu` into this worktree's own `target/` — the
shared `fd-instr-repair` binary cannot know a strategy id that did not exist
when it was built, and a binary swallows an id it does not know in silence.
Tests: `--release -p fd-strategy` only, **127 + 2 + 3 + 1 green, 0 failures**;
`cargo test --workspace` at debug was not run.

Every run: `--mode=hypotheses --fixed --exit-mix --null-sides=exposure
--seeds=200 --interval=15m --data=/e/rust/flowdesk/data --market=xauduka`,
spread 0.28, trail off. Every receipt prints
`flags: 12 passed, every one of them read by --mode=hypotheses` (13 in arm C,
which adds `--guards`). `bounds:` `--from=2010-06-01 --to=2018-06-01 kept
190889 of 378749 bars` and `--from=2018-06-01 --to=2026-06-01 kept 187860 of
378749 bars`. `news:` identical in all six: **747 events (2010-01-08 ->
2027-12-08) from `E:/rust/flowdesk/data\news\events.parquet`**, scope USD.
`data-sealed/` was not opened, read, pointed at or counted.

Receipts: `A-duka-{is,oos}.txt`, `B-duka-{is,oos}.txt`, `C-duka-{is,oos}.txt`,
`PROBE-xag-swapB.txt`. Tables: `scripts/rollover_flat_table.py`.

---

## 0. The four falsifiers

| | fired | where |
|---|---|---|
| **F1** no flat row clears the gate on both legs in any arm | **FIRED** | **0 of 24** flat-row cells clears **either** leg, let alone both. Best anywhere: `qsf-h15` arm C IS, **PF 1.084 / +0.014R** — short of PF by 0.116 and of expectancy by 0.036R |
| **F2** the flat variant is not actually flat (`swap$/spread$ > 1.0x`, or `\|expectancy_net - expectancy\| > 0.010 R/trade`) | **did NOT fire** | `swap$/spread$` **0.123x-0.190x** on all four flat rows against **29.0x-113.2x** on their parents; the gap is **-0.001 to -0.002 R/trade**, which is `px-1s`'s own **-0.001** |
| **F3** the edge lives only on the 2018-2026 half | **fired sideways, and it is worse than the registration imagined** | no flat row is positive on both halves *in the same family*: the `quiet-swing` family is stronger on IS and negative/flat on OOS, the `tsmom` family is the exact reverse. **The window flips which family works, not just how much** |
| **F4** `WEEKEND_FLAT` > 5 per 1,000 trades on a flat row in arm C | **did NOT fire** | **0-1 in 1,157-1,944 trades = 0.0-0.8 per 1,000**, against **395.7-477.7 per 1,000** on the parents |

---

## 1. The gate, counted by hand at 40 trades, on `expectancy_net`

Gate, not moved: **PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades, on
both windows.** Counted at 40 by hand; the tool's verdict string says `need 30`.

| arm | guards | swap | parity-anchor rows clearing BOTH legs | **flat rows clearing BOTH legs** |
|---|---|---|---|---|
| A | off | 0.00 | 3 of 4 (`qs-h15`, `ts-l20`, `ts-l60`) | **0 of 4** |
| B | off | -0.83 / -0.83 per lot-night | **0 of 4** | **0 of 4** |
| C | **on** | 0.00 | 0 of 4 (`qs-h15` IS only) | **0 of 4** |

**0 of 24 flat cells.** Every flat cell in every arm, both legs:

| row | arm A IS | arm A OOS | arm B IS | arm B OOS | arm C IS | arm C OOS |
|---|---|---|---|---|---|---|
| `qsf-h15` | 1.057 / +0.011 | 0.962 / -0.005 | 1.049 / +0.010 | 0.954 / -0.007 | **1.084 / +0.014** | 0.956 / -0.006 |
| `qsf-h5` | 1.046 / +0.009 | 1.013 / +0.003 | 1.038 / +0.007 | 1.004 / +0.002 | 1.067 / +0.011 | 1.006 / +0.002 |
| `tsf-l20` | 1.022 / +0.003 | 1.035 / +0.005 | 1.013 / +0.002 | 1.027 / +0.004 | 1.034 / +0.004 | 1.032 / +0.004 |
| `tsf-l60` | 1.006 / +0.001 | 1.049 / +0.007 | 0.998 / +0.000 | 1.042 / +0.006 | 0.990 / -0.001 | 1.055 / +0.007 |

(PF / `expectancy_net` in R. Trade counts 1,157-1,944 a cell, so the 40-trade
leg is never the one that fails.)

**Which leg binds — and addendum section A does NOT hold at this exit mix.**
Addendum A measured, at `reward_risk = 1.8` with pure STOP/TARGET exits, that
PF 1.200 implies a 40.0% win rate and +0.050R only 37.5%, so expectancy was the
**redundant** leg (4 cells passed expectancy while missing PF, 0 the reverse).
On these rows the ratio to threshold is the other way round: `tsf-l60` arm A OOS
reaches **87.4% of the PF threshold** and **14.0% of the expectancy threshold**.
On all 24 flat cells **expectancy is the far more binding leg**, by a factor of
4 to 50. These mechanisms never exit on STOP/TARGET — `reward_risk` derives no
target when exits are the strategy's — so addendum A's equivalence is a
statement about a stop-and-target exit mix and is not transferable. **My numbers
are the ones measured here; the addendum's are the ones measured there.**

**And a tool-reading defect worth recording, which is brief requirement 1 in
action.** In arm B the verdict string on all **six** parent cells reads
`fail: profit factor 0.594 < 1.2` and **names only the PF leg** — because the
verdict tests `expectancy` (+0.130 / +0.095 / +0.151 / +0.080 / +0.122 /
+0.370, all of which pass) while `expectancy_net` is **-0.265 / -0.345 /
-0.731 / -0.179 / -0.199 / -0.271**, all of which fail. Anyone reading the
verdict alone would conclude those six cells "only missed PF". They miss both.
Counted, not repaired.

---

## 2. Parity: the new binary reproduces the record exactly

Four anchors, from a **different binary in a different worktree** than
`agent/n5` and `agent/pure-drift`:

| row | leg | trades | PF | expectancy | null p50 | pct | matches |
|---|---|---|---|---|---|---|---|
| `px-1s` | OOS | **1,982** | **1.072** | **+0.012** | **1.153** | **0%** | `pure-drift` s.1 to the digit |
| `qs-h15` | IS | **190** | **1.262** | **+0.130** | 0.954 | 91% | `n5` / `pure-drift` s.9 |
| `qs-h15` | OOS | **199** | **1.201** | **+0.080** | **1.110** | **62%** | idem |
| `ts-l20` | IS / OOS | **183 / 185** | **1.315 / 1.427** | **+0.095 / +0.122** | 0.982 / **1.188** | 92% / **76%** | idem |
| `ts-l60` | IS / OOS | **93 / 90** | **1.383 / 2.183** | **+0.151 / +0.370** | 0.966 / **1.211** | 81% / **82%** | idem |

Arm B's financing multiples reproduce too: **34.988 / 55.195 / 111.761** (IS)
and **29.031 / 53.456 / 113.203** (OOS) against `pure-drift`'s 35.0 / 55.2 /
111.8 and 29.0 / 53.5 / 113.2. Arm C reproduces to the trade: `qs-h15` IS
**PF 1.255, +0.066R, 393 trades, 98%, own rule 88 of 393**; `px-1s` IS own rule
**1,761 of 2,014**. `wrong_side_stop` is reported **0 times in all seven
receipts**, and no row uses a pending order, so neither counted-not-repaired
defect of brief section 7 is exercised.

---

## 3. The mechanism IS flat — F2 answered with the number the brief asked for

Requirement 2 asked: if the flat variant's `expectancy_net` is near-unchanged
between the two swap arms, **that is the result**, said with `swap$/spread$`.
It is, and here it is:

| row | hold | `swap$/spread$` IS | OOS | `expectancy_net - expectancy` IS / OOS |
|---|---|---|---|---|
| `px-1s` (reference) | 22.9 h | 0.116x | 0.188x | **-0.001 / -0.001 R** |
| **`qsf-h15`** | **22.9 h** | **0.123x** | **0.181x** | **-0.001 / -0.002 R** |
| **`qsf-h5`** | **22.9 h** | **0.123x** | **0.190x** | **-0.002 / -0.001 R** |
| **`tsf-l20`** | **22.9 h** | **0.126x** | **0.176x** | **-0.001 / -0.001 R** |
| **`tsf-l60`** | **22.9 h** | **0.133x** | **0.174x** | **-0.001 / -0.001 R** |
| `qs-h15` | 215 / 189 h | 34.988x | 29.031x | -0.395 / -0.259 R |
| `ts-l20` | 334 / 330 h | 55.195x | 53.456x | -0.440 / -0.321 R |
| `ts-l60` | 651 / 670 h | 111.761x | 113.203x | **-0.882 / -0.641 R** |

In dollars: `tsf-l60` pays **-$205 (IS) / -$168 (OOS)** of financing where
`ts-l60` pays **-$6,091 / -$4,647** over the same eight years — **3.4% / 3.6%**
of it. The residual is holidays, where the feed prints no bar in the flat
16:00-18:00 window. **The carry is gone, measured, on the row the whole job was
built around.**

Checked a second way, against the charging function rather than a restatement
of it: `causality_rollover_flat.rs` walks both strategies over an 80-session
15-minute fixture and asserts `fd_core::clock::swap_nights(entry, exit) == 0`
on **every** closed position, with the longest hold asserted at ~22 h so the
zero is not the zero of a position that never opened. `crosses_rollover()`
refuses any window containing 17:00 New York outright, so the invariant cannot
be lost to a bad override either.

---

## 4. The brief owner's arithmetic: right in direction, wrong in both numbers, and conservative

The brief said: *~27 round trips instead of 1, at 0.85-1.10% of R, ~23-30% of R,
against 0.641R of carry.* Measured:

| the brief's number | measured | why it differs |
|---|---|---|
| **~27 round trips** per `ts-l60` hold | **19.6x (IS) / 19.9x (OOS)** | 651 h is **27.1 calendar days**, but a flat book only trades sessions the feed opens: 27.1 x 5/7 = **19.4 trading sessions**. The 27 counted calendar days |
| **0.85-1.10% of R** per round trip | **0.55-0.60% (OOS) / 0.82-0.83% (IS)** on the `tsmom` family; **0.86-0.87% / 1.14-1.15%** on the `quiet-swing` family | the ruler. `tsmom` sizes on **2.0** mean New York-day ranges over 20 days, `quiet-swing` on **1.5**. 1.5/2.0 x 1.15% = 0.86%, which is what the `tsmom` rows read. On the 1.5-range ruler the addendum quoted, the measurement **confirms addendum F**: my own `px-1s` reads **1.15% (IS) / 0.83% (OOS)** against its 1.10% / 0.85% |
| **23-30% of R** spent to avoid the carry | **16.3% (IS) / 10.9% (OOS)** for `tsf-l60` | 19.9 trips, not 27, at 0.55% of R, not 0.85% |
| **0.641R** of carry bought back | **0.641R**, confirmed exactly | — |

So the owner's arithmetic was **conservative by about 2.5x**: the flat
expression of `ts-l60` spends **0.109R (OOS)** to avoid **0.641R**.

**And the measured net per original-trade-equivalent, arm B (financing charged),
is positive in 7 of 8 comparisons and better than the continuous expression in
all 8:**

| flat row | leg | trips | flat `expectancy_net`/trip | **x trips = per original hold** | parent `expectancy_net` | **improvement** |
|---|---|---|---|---|---|---|
| `qsf-h15` | IS | 7.6 | +0.010 R | **+0.076 R** | -0.265 R | **+0.341 R** |
| `qsf-h5` | IS | 6.9 | +0.007 R | **+0.048 R** | -0.265 R | **+0.313 R** |
| `tsf-l20` | IS | 10.6 | +0.002 R | **+0.021 R** | -0.345 R | **+0.366 R** |
| `tsf-l60` | IS | 19.6 | +0.000 R | **+0.000 R** | -0.731 R | **+0.731 R** |
| `qsf-h15` | OOS | 6.6 | -0.007 R | **-0.046 R** | -0.179 R | **+0.133 R** |
| `qsf-h5` | OOS | 5.8 | +0.002 R | **+0.012 R** | -0.179 R | **+0.191 R** |
| `tsf-l20` | OOS | 10.3 | +0.004 R | **+0.041 R** | -0.199 R | **+0.240 R** |
| `tsf-l60` | OOS | 19.9 | +0.006 R | **+0.119 R** | -0.271 R | **+0.390 R** |

Assumption stated: multiplying a per-trade R by the trip count treats the R
unit as equal across a horizon's trips. It is the same sizing rule re-measured
each session on a 20-day mean range, so the unit moves slowly — but it moves,
and this is an approximation, not an identity.

---

## 5. The number worth keeping: the SIGNAL survives the in-session expression; the SPREAD eats it

Gross of spread, per session, all from arm A (swap 0, so `expectancy` is gross
of financing and net of spread; `spread/trip` in R uses the `risk_usd`
recovered as `|swap$| / |total_r_net - total_r|` from the same row's arm-B pair,
which lands at **$91-$111** against the configured 1% of $10,000):

| flat row | leg | flat gross / trip | parent gross / hold | parent gross / **session** | **retained** | spread eats |
|---|---|---|---|---|---|---|
| `qsf-h15` | IS | **+0.0224 R** | +0.1462 R | +0.0194 R | **116%** | 51% |
| `qsf-h5` | IS | +0.0205 R | +0.1462 R | +0.0213 R | **97%** | 56% |
| `tsf-l20` | IS | +0.0112 R | +0.1063 R | +0.0100 R | **112%** | 73% |
| `tsf-l60` | IS | +0.0093 R | +0.1633 R | +0.0083 R | **111%** | 89% |
| `qsf-h15` | OOS | +0.0036 R | +0.0912 R | +0.0138 R | **26%** | 240% |
| `qsf-h5` | OOS | +0.0117 R | +0.0912 R | +0.0157 R | **75%** | 74% |
| `tsf-l20` | OOS | +0.0110 R | +0.1299 R | +0.0126 R | **87%** | 54% |
| `tsf-l60` | OOS | **+0.0125 R** | +0.3775 R | +0.0190 R | **66%** | **44%** |

**Read it in one line: expressing a long-horizon gold signal only inside the
session, flat across the rollover, keeps 66-116% of its gross edge per session
in 7 of 8 comparisons — and then pays 44-89% of what is left to the spread, 6
to 20 times over.** The signal is not destroyed by the in-session expression.
It is destroyed by paying for the expression.

So the answer to the brief's question — *is there anything left of a
long-horizon signal expressed in-session and flat across every rollover?* — is
**yes, there is something, and it is about 0.01 R per session gross, which is
not enough.**

---

## 6. Why it fails the gate although the trade-off is favourable: the gate is PER TRADE

This is the finding that matters most for what the desk does next.

| flat row | leg | trips per original horizon | gate's expectancy leg, per horizon | measured net, per horizon |
|---|---|---|---|---|
| `qsf-h15` | IS | 7.6 | **+0.378 R** | +0.083 R |
| `tsf-l20` | OOS | 10.3 | **+0.516 R** | +0.052 R |
| `tsf-l60` | IS | 19.6 | **+0.980 R** | +0.020 R |
| `tsf-l60` | OOS | 19.9 | **+0.994 R** | **+0.139 R** |

The gate's `+0.050R` leg is a **per-trade** threshold. Chop one horizon into
19.9 trades and the same total R has to clear **19.9 x 0.050R = +0.994R per
horizon** instead of +0.050R. `ts-l60`'s *entire* gross edge over that horizon
is **+0.3775R**. **No flat expression of this signal can pass the gate's
expectancy leg at any cost**, including zero: it would need a gross edge
**2.6x** the parent's measured one.

That is the same structure `agent/pure-drift` section 2 found from the other
side (*"the gate's expectancy leg is, for a one-sided book, a statement about
hold length, not about method"*), now measured on a two-sided patterned book.
**It is a property of the gate, not a result about the market, and it is not an
argument for moving the gate** — the gate is the desk's, not this plan's. It is
an argument for saying clearly what the gate can and cannot express.

---

## 7. The one genuinely new, genuinely positive finding: these are the first long-horizon mechanisms that survive the TRADEABLE arm as themselves

Addendum section D: the owner has settled that nothing is held over a weekend,
so **every long-horizon result in this record — `agent/n5`'s four gate-passers
included — exists only in an arm the owner has forbidden.** `--exit-mix`, arm C
(guards on: `max_open_loss_r 2.0, notional 300%, weekend flat 16:40 NY, news
flat 60/30 (impact>=3, USD), daily cap 20, loss limit $300, cooldown 30 min`):

| row | trades A -> C | mean hold A -> C | own rule fires (arm C) | `WEEKEND_FLAT` | per 1,000 |
|---|---|---|---|---|---|
| `qs-h15` IS | 190 -> **393** | 215.5 h -> **67.5 h** | **88 / 393 = 22.4%** | **159** | **404.6** |
| `ts-l20` IS | 183 -> **612** | 334.0 h -> **69.8 h** | 146 / 612 = 23.9% | **243** | **397.1** |
| `ts-l60` IS | 93 -> **527** | 651.4 h -> **79.6 h** | **65 / 527 = 12.3%** | **247** | **468.7** |
| `ts-l60` OOS | 90 -> **538** | 670.4 h -> **78.1 h** | **55 / 538 = 10.2%** | **257** | **477.7** |
| **`qsf-h15` IS** | 1,435 -> **1,435** | 22.9 h -> **21.9 h** | **1,256 / 1,435 = 87.5%** | **1** | **0.7** |
| **`qsf-h5` IS** | 1,305 -> **1,305** | 22.9 h -> **21.9 h** | 1,140 / 1,305 = 87.4% | **1** | **0.8** |
| **`tsf-l20` IS** | 1,944 -> **1,944** | 22.9 h -> **21.9 h** | 1,703 / 1,944 = 87.6% | **1** | **0.5** |
| **`tsf-l60` OOS** | 1,790 -> **1,790** | 23.3 h -> **22.3 h** | 1,561 / 1,790 = 87.2% | **0** | **0.0** |
| `px-1s` IS (ref) | 2,014 -> 2,014 | 22.9 h -> 21.9 h | 1,761 / 2,014 = 87.4% | 1 | 0.5 |

**The guards do not touch the flat variants.** Trade count unchanged to the
unit, mean hold down 4%, own rule still the exit on 87% of trades, and
`WEEKEND_FLAT` **0 or 1 time in 1,157-1,944 trades**. The 13% the guards do take
is `NEWS_FLAT` (143-240 a cell), the same share it takes from `px-1s` (245 of
2,014 = 12.2%).

**The parents are deleted and replaced.** `ts-l60` in arm C books **527-538
trades** instead of 90-93 and holds **78-80 h** instead of 651-670 — a factor of
**5.8x** on count and **8.4x** on hold — and its **own rule fires on 10.2-12.3%
of exits**. In arm C the label `ts-l60` is false: the mechanism is "re-enter on
the trailing 60-day sign whenever the guards have flattened you". That is the
`tsmom/120d` trap of brief section 6a, and those cells are **void** under the
`agent/n5` rule-3 convention — which is why the table above reports them and
does not read them.

**So the governance claim of registration section 8 F4 stands, and it is the
strongest thing this job produced:** `tsmom-flat` and `quiet-swing-flat` are the
first mechanisms in this record that carry a long-horizon signal and **survive
the only arm the owner permits as the same mechanism they are in the
unguarded arm.** Stated with its limit in the same breath: **surviving the
tradeable arm is not passing the gate, and they do not pass it.**

---

## 8. F3: the window flips WHICH FAMILY works, which is worse than a sign flip

Addendum section B: gold's one-session drift is **-0.0040 R (2010-2018)** and
**+0.0205 R (2018-2026)** — the sign flips, and nothing may be built on "gold
has positive drift". Nothing here is: all four flat rows are two-sided, with
long shares **0.474-0.534 (IS)** and **0.600-0.633 (OOS)**.

The registration asked whether a gate-passing flat row would live only on the
2018-2026 half. None passes, so that clause cannot fire as written. What the
numbers say instead is worse:

| family | IS (2010-2018) | OOS (2018-2026) |
|---|---|---|
| `quiet-swing` flat (`qsf-h15`) | **+0.011 R**, PF 1.057, retained **116%** | **-0.005 R**, PF 0.962, retained **26%** |
| `tsmom` flat (`tsf-l60`) | **+0.001 R**, PF 1.006, retained 111% | **+0.007 R**, PF 1.049, retained 66% |
| `tsmom` flat (`tsf-l20`) | +0.003 R, PF 1.022 | +0.005 R, PF 1.035 |

`qsf-h15` is the **best** flat row on IS and the **only negative** flat row on
OOS. `tsf-l60` is the **worst** on IS and the **best** on OOS. **Which of the
two signal families carries the in-session edge is itself a function of the
window** — the same disease as the nine artefacts of addendum B, now visible in
the choice of signal rather than in the choice of parameter. Had this plan
declared one family and not both, it would have produced a clean-looking
window artefact.

---

## 9. Percentiles — reported, and not read as a result

Reading limits (brief section 4), as the receipts print them. `count match` is
**0.89-1.09** on all 48 cells. `cost match` is outside the band on **9 cells**,
all in arm B (`px-1s` 0.69 / 0.75 and the four flat rows 0.57-0.64 on IS) — for
those nine **no percentile is published**. For the rest:

* The `RandomHold` control is handed **1.25x-1.31x** the flat rows' time in the
  market in arms A and B, which is `agent/pure-drift` section 7's eighth window
  artefact (the control over-allocated exposure); in arm C it is handed
  **0.87-0.90x**, and on the parents **0.53-0.60x**. **The defect is counted and
  not repaired, and no percentile below is read as a result.**
* With that said: the flat rows sit at **87%-96% (arm A IS)**, **34%-82% (arm A
  OOS)**, **86%-98% (arm C IS)** and **45%-92% (arm C OOS)** against null p50s
  of **0.924-0.997**. A `null p50` under 1.000 means the control's own median
  loses money, so a high percentile there says "loses less than random", not
  "makes money" — the receipts print that warning themselves, and the gate is
  what says the other thing.
* Side matching is **excellent** on the flat rows and poor on the parents: long
  share method vs null **0.516 / 0.508**, **0.633 / 0.630** on the flat rows,
  against **0.467 / 0.627** on `ts-l60` arm A OOS. That is the match
  `agent/n5`'s direction-switching rows kept failing, and the flat rows pass it
  — which makes their percentiles the more readable ones even though none is
  read as a result.

---

## 10. What this report does NOT measure

1. **Drawdown.** `max_drawdown_usd` / `max_drawdown_pct` exist in
   `engine.rs::Metrics`; `--mode=hypotheses` does not print them (addendum G).
   The registration therefore **offered no drawdown criterion**, and every
   return figure above — including the **+0.390R per original horizon** of
   section 4 — is a return with **no risk number beside it**. A mechanism that
   trades 1,790 times instead of 90 has a different path even at the same total
   R, and this plan cannot say how different.
2. **The 16:00-18:00 New York gap**, which the flat variant forgoes: 2 of the
   session's 24 hours, containing the daily gap. Not measured; a
   `session-hold` row across it would cross the rollover and is a different
   question.
3. **Venue spread around the daily break.** This feed's spread is a flat 0.28 at
   every hour, and a real venue widens it exactly where this mechanism
   re-enters — 6 to 20 times per original horizon. **Every spread figure here is
   a lower bound on the real cost of the flat expression**, and the asymmetry
   runs against the hypothesis. Given that section 5 measures the spread eating
   44-89% of the flat gross, a realistic break spread plausibly closes the
   remaining edge entirely. **Not measured.**
4. **Bit-identity with the parents.** `quiet-swing-flat`'s state rule is the
   stateless reconstruction declared in registration section 4, not a port: it
   restarts the hold clock whenever the quiet condition fires again inside a
   run. Measured consequence: `qsf-h15` takes **1,319-1,435** trades where
   `7.6 x 190 = 1,444` sessions of pure hold-clock replication would be the
   ceiling, so the reconstruction is ON for **91-99%** of the sessions the
   parent would have held — close, and not identical.
5. **`xauusd`** (four years cannot carry a sixteen-year question) and
   **`xagduka` beyond the pre-check** (8 probe cells, arm B; the second declared
   probe run was not made because `config/` and `config-swap/` are identical for
   silver and the receipt would have been byte-identical).
6. **Why `qsf-h15` is the only negative flat row.** Its long share rises from
   0.516 (IS) to 0.625 (OOS) while its edge goes from best to negative; whether
   that is the quiet filter selecting differently in a higher-volatility half is
   not measured here.
7. **`data-sealed/`** — not opened, read, pointed at or counted.

---

## 11. Multiplicity ledger — declared vs viewed

| | declared | viewed |
|---|---|---|
| gold cells (2 windows x 3 arms x 8 rows) | **48**, in 6 runs | **48**, in 6 runs |
| pre-check probe cells, `xagduka` | **16**, in 2 runs | **8**, in **1** run (amendment of 2026-10-07: the arm-A probe would have been byte-identical, silver's swap rate being 0.00 in both config trees) |
| grand total | 48 gold + 16 probe | **48 gold + 8 probe** |

No row was added, no threshold moved, no window changed, no sample floor
lowered, after a number was seen. The two amendments are dated notes appended to
registration section 13; no line above them was rewritten. Both were written
**before** the first gold run.

---

## 12. What the desk should take from this

1. **F1 fired, on the friendliest arm that exists and on two more.** 0 of 24.
   The gate is not reached by any flat expression of these signals.
2. **But the falsifier's stated reading — "the residual edge is not carry, it
   does not exist" — is NOT what the numbers say, and the brief owner asked for
   this to be said if it happened.** The carry is real and avoidable
   (0.641R -> 0.001R of financing per trip, `swap$/spread$` 113.203x -> 0.174x),
   the signal does survive the in-session expression (66-116% of gross per
   session retained in 7 of 8 comparisons), and the flat expression is better
   net than the continuous one in **8 of 8** comparisons, by **+0.133R to
   +0.731R** per original horizon. What kills it is the **spread, paid 6 to 20
   times**, which takes 44-89% of a gross edge of **+0.0036 to +0.0224 R per
   session**. The edge exists; it is about **one hundredth of an R a session**;
   it is too small to pay for its own expression.
3. **The gate cannot express a chopped horizon at all.** +0.050R per trade is
   +0.994R per original `ts-l60` horizon, against a measured gross of +0.3775R.
   This is arithmetic, not a market fact, and it applies to every future
   proposal of this shape. **A desk that wants to test slow signals in a fast
   expression needs a per-unit-time criterion, or it is testing the chopping.**
4. **The one thing here that is tradeable-arm-compatible is the shape, not the
   edge.** `WEEKEND_FLAT` 0.0-0.8 per 1,000 against the parents' 395.7-477.7;
   own rule firing 87% in arm C against the parents' 10-24%; trade count and
   hold unchanged by the guards. If the desk ever finds a signal with ten times
   this gross edge per session, **this is the expression to put it in** — the
   plumbing is built, registered, tested against `swap_nights` itself, and it
   lives in the arm the owner permits.
5. **Do not report any of these four rows as a candidate.** None passes the
   gate on either leg, and section 10.3 says the spread figures are a lower
   bound.
