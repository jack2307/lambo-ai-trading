# designed-1 — the cost term measured, and why it yields no method

**Agent:** designed-1, angle 1 ("attack the cost term, not the signal term") of
`docs/hypotheses/2026-09-23-designed-methods.md`.
**Design store:** `E:/rust/flowdesk/data-sealed` — every receipt below prints it.
**Design window:** XAUUSD 15m, 2022-06-16 10:30 → 2025-09-22 23:45, 77,278 bars.
Cross-checks on XAUDUKA 15m over the same dates (76,785 bars) and over 15 years
(362,803 bars from 2010-06-01).
**Methods proposed for the withheld year: none.** See "Why nothing is proposed".

---

## 1. Mechanism

Two things were true about this market before anything here was fitted.

**The spread is a fixed number of points; the move is not.** The Vantage
`XAUUSD.sc` round turn is a constant 0.28 points whatever price is doing. The
distance a trade travels before its exit is not constant: it is set by where
the exit sits. So the fraction of a trade's result that the spread takes is
`0.28 / (the distance to the exit)`, and a method controls the denominator by
choosing where it admits it is wrong. This is arithmetic, not a market claim.

**Where a break is wrong is a level, not a distance.** A range is a stretch of
price where two-sided interest has been resting. When price closes beyond the
extreme of that range, the statement "the range has been left" is falsified not
by a small move back but by price returning to the *other* edge — at which
point the range is simply intact and the break was noise. That level pre-exists
the trade, it is published by the market rather than chosen by the designer, and
it is far from the entry by construction, because it is the whole width of the
range. The registry's habit is instead a distance: `stopAtr x ATR`, which on
this data is 2.4–3.6 points and has no relationship to where the break would be
disproved.

So: take the dullest signal in the book — a close beyond the prior N-bar
channel, which `donchian-breakout` has traded in this registry for months — and
change only where it is wrong. Nothing about the entry improves. The claim is
that the same entry pays a smaller share of its result to the spread when its
invalidation is the structural level, and that the share is large enough to
matter. `crates/fd-strategy/src/far_stop_break.rs` is that instrument: one
`on_bar`, one branch, `stopMode = 0` for the far channel edge and `stopMode = 1`
for the ATR distance.

A second effect rides along and is worth naming because it is gross rather than
a cost: a stop 2.4 points from entry is inside the 15-minute noise, so it ends
trades that the market had not yet decided. In the exploratory probe of section
2 the ATR arm's *gross* profit factor — the same trades with the spread added
back on — is 0.961 while the structural arm's is 1.055, so the tight stop is
destroying about 0.09 of profit factor before any cost is charged at all.

## 2. The measured cost arithmetic

Measured on the design store, not assumed. `data-sealed`, XAUUSD 15m, all
77,278 bars. **This section's figures come from an exploratory probe over the
sealed parquet, not from the engine** — they are bar statistics and horizon
returns, used to choose the design. Section 3 is the audited path and is where
any number about a method comes from.

| quantity | points |
|---|---|
| median price over the window | 2,040 |
| true range per 15m bar, p10 / p50 / p90 | 0.95 / 2.20 / 5.52 |
| ATR(14), p10 / p50 / p90 | 1.41 / 2.49 / 4.86 |
| median absolute move over 16 bars (4h) | 3.94 |
| median absolute move over 96 bars (24h) | 11.83 |
| median distance from entry to the 20-bar channel's far edge | 9.02 |
| median distance from entry to the 80-bar channel's far edge | 21.74 |

(The last two are the channel's width plus however far the breaking close
overshot it — the invalidation distance a trade actually gets, measured on the
entries the signal took, not the width of every channel.)

At the configured 0.28 per round turn (used because every receipt in
`docs/decisions/` was measured at it; the logger's median is 0.21 and
`config/default.toml` explains why the higher figure stands):

| invalidation distance | cost as a share of R |
|---|---|
| 1.2 x ATR(14) median = 2.99 points | 9.4% |
| 1.5 x ATR(14) median = 3.73 points — *what the null uses* | 7.5% |
| 20-bar channel far edge, median 9.02 points | 3.1% |
| 80-bar channel far edge, median 21.74 points | 1.3% |

So the denominator really is available: the structural level is three to seven
times the ATR distance on the same bars, and the cost share falls by the same
factor. That is the whole of what angle 1 has to sell.

**What it is worth, and the ceiling it runs into.** The spread hits a trade
twice — it shrinks every winner and enlarges every loser — so its effect on
profit factor is roughly `(1 - c/W) / (1 + c/L)` for a winner of `W` points and
a loser of `L` points. Two consequences, both measured:

- The best case is `W` and `L` both large and roughly equal. Making one large
  and the other small does not help: at `W` = 20, `L` = 2 points the multiplier
  is 0.865, at `W` = 2, `L` = 20 it is 0.848, and at `W` = `L` = 6 it is 0.911.
- `W` and `L` are capped by how long the engine lets a position live.
  `[trading] max_hold_ms = 14_400_000` is **four hours** and
  `trading_rules_for` reads it from the shared table for every market, so every
  `Exits::Engine` method in this record — all but the drift claims — has been
  measured on holds of at most 16 bars. Over 4 hours the median absolute move
  on this data is 3.94 points. **The cost multiplier on profit factor therefore
  cannot get better than about 0.93 for any engine-exit method on 15-minute
  gold, however the stop is placed.**

Measured multipliers (net profit factor over gross profit factor, barrier-free,
same signals, design store):

| horizon | median absolute move | multiplier | gross profit factor needed for net 1.2 |
|---|---|---|---|
| 16 bars (4h) — the engine's limit | 3.94 points | 0.913–0.933 | 1.29–1.32 |
| 96 bars (24h) | 11.84 points | 0.967 | 1.24 |
| 288 bars (72h) | 22.2 points | 0.982 | 1.22 |
| 672 bars (168h) | 35.1 points | 0.988 | 1.21 |

And the gross profit factor actually on offer from dull signals, on the same
bars: **1.01 to 1.16**, at every horizon, on both vendors, over 3 years and over
15. The single best figure anywhere in the exploration was 1.164 (an 80-bar
break held 72 hours, XAUUSD) against the 1.221 it would have needed.

The two columns never meet. That is the finding.

## 3. Design-window numbers, through the audited path

`far-stop-break` run by `search --mode=hypotheses --fixed --seeds=200`, so these
are `run_hypothesis_fixed_guarded` numbers against the count-matched
random-entry null, comparable to every receipt in `docs/decisions/`. Eight rows
declared in `2026-09-23-designed-1-contrast.toml` before the run; all eight
reported. Receipts: `docs/research/runs/2026-09-23-designed-1-cost-term/`.

**XAUUSD 15m, 2022-06-16 → 2025-09-23, guards on, spread 0.28:**

| row | invalidation | trades | profit factor | expectancy | null pct | count match |
|---|---|---|---|---|---|---|
| atr-20 | 1.2 x ATR | 5,577 | 0.786 | −0.104 R | 2% | 0.67 (unmatched) |
| struct-20 | 20-bar far edge | 3,404 | 0.871 | −0.029 R | 61% | 0.80 |
| atr-40 | 1.2 x ATR | 3,917 | 0.801 | −0.096 R | 6% | 0.76 |
| struct-40 | 40-bar far edge | 2,266 | 0.903 | −0.014 R | 79% | 0.88 |
| atr-80 | 1.2 x ATR | 2,536 | 0.833 | −0.077 R | 26% | 0.86 |
| struct-80 | 80-bar far edge | 1,437 | 0.973 | −0.001 R | 98% | 0.95 |
| struct-80-f14 | far edge, ≥ 14 points | 1,289 | **1.065** | +0.009 R | 100% | 0.97 |
| atr-20-f14 | 1.2 x ATR, ≥ 14 points | 17 | 0.505 | −0.307 R | 10% | 2.24 (unmatched) |

**The claim I was given to test is true, and it is paired.** At every lookback,
the same signal with the structural invalidation beats itself with the ATR one:
+0.085, +0.102 and +0.140 of profit factor at 20, 40 and 80 bars. It holds with
guards off (+0.074, +0.150, +0.167 — `xauusd-unguarded.txt`, run because the
notional cap sized down 5,192 of the ATR arm's 5,577 entries and none of the
structural arm's, so the guard had to be ruled out as the cause). It holds on a
second vendor over the same dates (+0.075, +0.113, +0.154) and over 15 years
(+0.159, +0.181, +0.198). A dull entry with a structurally wide stop does beat a
sharp entry with a tight one, by up to a fifth of a profit factor.

**And every row still fails.** The best is 1.065 against a gate of 1.20, with
+0.009 R against 0.05 R. On 15 years of Dukascopy gold the same row is 0.916 —
it loses. Nothing here is near the gate, in sample, on the window it was
designed on.

## 4. Variants tried

My own multiplicity, stated because the numbers above cannot be read without it:

- **116 exploratory cells** in Python on the sealed store before any Rust ran:
  24 barrier geometries (3 lookbacks x 4 stop rules x 2 reward-risk settings),
  44 signal-and-horizon cells (11 dull signals x 2 horizons x all bars / the
  top volatility quartile), and 48 horizon cells (4 signals x 4 horizons x 3
  series). 16 of the 48 were silver and are void — I charged silver the gold
  spread of 0.28 instead of its configured 0.021, which makes those 16 numbers
  meaningless. They are excluded from every claim above and named here rather
  than deleted.
- **8 declared configurations** in the audited engine, measured four times each
  (XAUUSD guarded, XAUUSD unguarded, XAUDUKA same dates, XAUDUKA 15 years).
  Nothing was carried, selected or re-fitted between runs; the eight rows are
  the same eight in all four receipts.

No parameter was moved to improve a number. `minStopPoints = 14` was set once,
as 50 x the 0.28 spread, because that is the cost condition in cost units; it
was not tuned.

## 5. Why nothing is proposed

The registration fixes multiplicity at eight methods across four agents and says
a null submission is preferred to filler. Mine is a null submission, for a
reason that is arithmetic rather than discouragement:

1. To clear a net profit factor of 1.20, a method needs a **gross** profit
   factor of 1.21 to 1.32 depending on how long it holds — and 1.29 or worse for
   anything using the engine's exits, because of the four-hour cap.
2. Every dull signal on this data supplies a gross profit factor between 1.01
   and 1.16, on two vendors, over 3 years and over 15.
3. The cost term is worth at most about 0.19 of profit factor, measured: from
   0.786 to 0.973 at an 80-bar channel, or from 0.758 to 0.934 unguarded. The
   shortfall to the gate is 0.23 to 0.41.

The angle works and is not enough. Submitting `struct-80-f14` would be
submitting a method whose own design-window profit factor is 1.065 against a
gate of 1.20, in the hope that a withheld year is kinder than the window it was
built on. That spends an eighth of the program's multiplicity on a coin flip,
and I would not defend it. There is therefore no
`2026-09-23-designed-1-frozen.toml` content to run: the file exists, says so,
and refuses to parse as a batch if anyone tries.

**What this is evidence for.** The registration sets up two explanations for 25
closed registrations and 161 cells with no survivors: the mechanisms were too
well known, or a 15-minute single-instrument edge net of 0.28 is thin to
nonexistent. This work says something sharper than either: **the cost is not
what is standing in the way.** Take the cost almost entirely off the table — a
one-week hold pays 1.2% of the move to the spread — and the required gross
profit factor barely moves, from 1.32 to 1.21, while the available gross profit
factor stays near 1.05. The gap is in the signal term by a factor of two, and
the designing of better exits cannot close it.

## 6. Two defects found

**(a) The matched null is count-matched but not cost-matched, and the ≥95th
percentile leg can be cleared by stop width alone.**
`hypotheses::control_for` builds the non-drift control from
`RandomEntry.default_params()` and overrides only `seed` — and `entryRate`,
which `matched_rate` calibrates. `stopAtr` stays at its default **1.5**,
whatever the method under test uses. So the null's cost share is fixed at
0.28 / (1.5 x ATR) ≈ 7.5% of R, and its profit-factor distribution sits where
that puts it: median 0.86–0.87 and 95th percentile 0.92–0.95 on this window.
A method that merely widens its stop pays 1.3% instead of 7.5% and clears that
bar **without predicting anything**. The evidence is in the table above and is
not subtle:

- `struct-80`, profit factor **0.973** — it loses money — sits at the **98th
  percentile** of its own count-matched null, count match 0.95, inside the band.
- `struct-80-f14` on 15 years of Dukascopy gold, profit factor **0.916** over
  4,441 trades — it loses 8% of gross — sits at the **100th percentile**.
- The same signal with an ATR stop sits at the 2nd percentile.

The 2026-09-23 repair matched the null's trade *count*, which was the right fix
for the defect it was aimed at. Nobody matched its *stop*, and for this
program's angle 1 that is the difference between a yardstick and a thermometer
held to the wrong thing. Consequence for the record: the ≥95th-percentile legs
of the falsifier can be passed by exit geometry, so the profit-factor leg is
doing all the work; and any past receipt whose method carried an unusually wide
stop has a percentile flattered by it. I have not changed the null — that is a
threshold move and not mine to make — but a percentile in this program should
be read beside the method's stop distance in ATRs.

**(b) `search` never printed the store it read.** `three_month_2` prints
`market: … from <path>`; `search` printed only `bars:` and a count, so a
`search` receipt could not be checked against the store it was supposed to
read. Cosmetic until a program withholds part of the history, which this one
does. Fixed in `crates/fd-backtest/src/bin/search.rs` with one line, above
`read_bars`; every receipt in this note carries it.

**A third thing, not a defect but a trap worth recording.** With the cent
account's 100 USD book, 1% risk per trade and the 300% notional cap, the cap
binds whenever `price / stop > 300` — that is, for **any stop under about 8.3
points at 2,500 gold**, which is most of this registry. When it binds, lots stop
being `risk_usd / (risk x contract_size)` and become a constant notional, so
`profit_factor` — computed from `pnl_usd` — stops being risk-weighted and
becomes price-weighted. It did not cause the effect measured here (the
unguarded run is larger, not smaller) but it means guarded and unguarded profit
factors for tight-stop methods are not the same quantity.

## 7. Judgement calls another agent might have made differently

- **`Exits::Engine` rather than `Exits::Strategy`.** Self-managed exits escape
  the four-hour cap, which is where the cost angle has room. I refused them
  because a self-managed method with a pinned preset is routed to the
  `RandomHold` null, which enters at `entryRate = 1.0` and re-enters as soon as
  it is flat; a selective method holding a day would be read against a control
  taking several times its trades, the count match would leave the 0.25 band,
  and the percentile would be unreadable. An agent willing to make its method
  always-in-market could have taken the longer horizon. It would have been
  angle 3's territory.
- **Designing on XAUUSD 2022–2025 rather than 15 years of Dukascopy.** The
  longer series is 15 years, but gold was 1,200 in 2010 and 3,700 in 2025, so
  0.28 points is a different cost against a different ATR in each era — the
  15-year ATR(14) median is smaller and the cost share correspondingly larger.
  Designing on the era the withheld year belongs to seemed right given the
  owner's 2026-09-13 criterion; the 15-year run is reported as a cross-check,
  and it agrees.
- **Not proposing `struct-80-f14`.** Another agent could reasonably submit it to
  put a number on the record. I think a design-window profit factor of 1.065
  against a 1.20 gate makes that a coin flip, and the registration asked me not
  to spend multiplicity on one.
