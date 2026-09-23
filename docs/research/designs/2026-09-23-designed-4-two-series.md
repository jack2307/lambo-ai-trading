# designed-4: two series, not one — and **no method proposed**

**Agent:** designed-4, angle 4 of `docs/hypotheses/2026-09-23-designed-methods.md`
**Data root, every figure below:** `E:/rust/flowdesk/data-sealed` (bars end
2025-09-22 23:45; the withheld year 2025-09-23 → 2026-09-17 is absent from it
and was not read)
**Methods proposed: 0.** The reason is in *The verdict* below, and it is a
measured reason, not a shortage of ideas.
**Frozen parameters: none.** There is no
`2026-09-23-designed-4-frozen.toml`, because there is nothing to freeze. That
file is absent rather than empty: an empty parameter file would read as a
method with no parameters.

The work is still here in full, because two parts of it are worth keeping: the
**plumbing** that lets a second instrument reach `on_bar` at all (it did not
exist, it does now, and it is tested), and the **measurement** of what the
second instrument is worth — which is not nothing, and is still not enough.

---

## 1. The mechanism, stated before anything was fitted

Gold and silver share one factor: the price of precious metal. The sharing is
large and it is contemporaneous, not lagged — measured below. So a gold move
that silver did not share is one of two things:

* news about gold alone, which does not revert; or
* a liquidity event in gold alone — a stop run, a thin book, one large order
  against a shallow side — which does revert once the book refills.

The claim is that at fifteen minutes the second is the commoner of the two, so
**a large gold move the second metal did not go along with partially
retraces**, while one silver shared does not. That is a statement about two
markets' shared factor and about the microstructure of a single-venue move. It
was true or false about the market before any parameter was chosen, and it is
what makes this a two-series method rather than a one-series method wearing a
second series as decoration.

The dollar version has an even simpler basis and gets its own section: gold is
quoted in dollars, so `XAUUSD = XAUEUR x EURUSD` is an identity, and a dollar
move with no gold move is a change in the price of gold in every other
currency. If the gold leg is the slower of the two to absorb a dollar move —
FX spot being by far the deeper market — the catch-up is tradable in gold.

Both were tested. One is real and unprofitable. The other is not there at all.

---

## 2. What I found about the two series' alignment

This is reported first because a two-series method that is wrong here is wrong
everywhere, and one of the findings is a change in the vendor's own data.

### 2.1 The grids are the same, and that is measured, not assumed

Every series in the store is `timestamp[ms, tz=UTC]` and every timestamp in
`XAUUSD-15m`, `XAUDUKA-15m`, `XAGDUKA-15m` and `EURDUKA-15m` is an exact
multiple of 900,000 ms. That says the stamps are on a grid; it does not say the
two feeds mean the same fifteen minutes by it. The check that does is the
repo's own shift-scan idiom (`docs/decisions/2026-09-12-otl-timestamps.md`):
correlate fifteen-minute log returns at lags −4 … +4 bars and look at where the
peak sits.

| pair | n (paired bars) | corr at lag 0 | largest \|corr\| at any lag ±1…±4 |
|---|---|---|---|
| XAUUSD vs XAUDUKA (two vendors, same metal) | 76,707 | **+0.997** | 0.010 |
| EURUSD (Vantage) vs EURDUKA | 36,424 | **+0.992** | 0.016 |
| XAUDUKA vs XAGDUKA | 356,617 | **+0.701** | 0.008 |
| XAUUSD vs EURDUKA | 76,825 | **+0.413** | 0.010 |
| XAUDUKA vs EURDUKA | 362,080 | **+0.331** | 0.005 |
| XAUUSD vs EURUSD (Vantage) | 35,078 | **+0.294** | 0.017 |

Two readings, both load-bearing:

1. **The grids agree.** Vantage's MT5 export and Dukascopy's bid feed put the
   same metal's fifteen minutes in the same UTC bucket to a correlation of
   0.997, and the same holds for EUR at 0.992. A bar stamped `T` in one series
   covers the same interval as a bar stamped `T` in the other, so a companion
   bar stamped `T` closes at `T + 15m` — the same instant the primary's does,
   and the same instant the engine fills at. Reading it is therefore causal.
   Had the peak sat at ±1, reading the companion at `T` would have been reading
   the future, and that is the second leak a two-series method has.
2. **There is no lead-lag to trade.** At every lag other than zero, every pair
   is at or below 0.017 — indistinguishable from nothing, on up to 362,080
   paired bars. **"One instrument leads the other by minutes" is refused
   outright for gold/silver and gold/EUR at fifteen minutes.** That is a whole
   sub-family of the assigned angle closed by one measurement, and it is
   reported as a result rather than as a step.

### 2.2 Missing bars: counted, never filled

The join is on an exact timestamp. A primary bar whose timestamp the companion
does not carry is **missing** — `NaN`, not zero, not the previous value, and
the strategy takes no trade on it. The counts:

| pair | timestamps in both | only in the first | only in the second |
|---|---|---|---|
| XAUDUKA & XAGDUKA (2010-06-01 → 2025-09-22) | 356,618 | 6,185 | 1,447 |
| XAUDUKA & EURDUKA (same span) | 362,081 | 722 | 19,118 |
| XAUUSD & XAUDUKA (2022-06-16 → 2025-09-22) | 76,708 | 570 | 35 |
| XAUUSD & XAGDUKA (same span) | 74,633 | 2,645 | 30 |
| XAUUSD & EURDUKA (same span) | 76,826 | 452 | 4,187 |
| XAUUSD & EURUSD Vantage (2024-03-29 → 2025-09-22) | 35,078 | 0 | 1,817 |

So pairing Vantage gold with Dukascopy silver costs **2,645 of 77,278 XAUUSD
bars (3.4%)** as no-decision bars. That is the price of the alignment rule and
it is paid in signals, not in wrong numbers.

### 2.3 A finding to report rather than paper over: the Dukascopy metals bar
rule changed in 2013

Longest run of consecutive fifteen-minute bars, per year, in the
XAUDUKA ∩ XAGDUKA join:

| years | longest run | median run | share of bars in a run > 97 |
|---|---|---|---|
| 2010, 2011, 2012 | **476 bars (≈5 days)** | 227 / 179 / 129 | 0.78 / 0.67 / 0.58 |
| 2013 … 2025 (every year) | **92 bars (23 h)** | 44–46 | **0.000** |

Before 2013 the feed carries bars straight through the daily break; from 2013
on it does not, and 92 bars = 23 hours is exactly one session minus a one-hour
break. **The same vendor's same instrument changes its bar-existence rule
inside the design window.**

That is not cosmetic. My first long-horizon scan required the forward holding
window to be contiguous, and for horizons of 96 bars and up **that condition is
satisfiable only in 2010–2012** — so those cells silently became a
three-year measurement dressed as a fifteen-year one, and they changed sign.
I caught it by printing run lengths per year; a horizon scan that trusted
`n` would have reported a reversal that was a calendar artefact. Anything on
this desk that measures a multi-day holding period on `XAU/XAGDUKA` 15m bars
needs to know this.

---

## 3. The plumbing: the trait *did* block a second series. Exactly where.

**`Strategy` as written cannot see a second instrument.** Three places say so
at once, and each would have had to change:

1. `fd_indicators::compute_indicators(bars: &[Bar], specs: &[IndicatorSpec])`
   — every series a strategy can read is a pure function of **one** bar slice.
2. `fd_strategy::registry::BarContext` carries `bar`, `i`, `bars`, `ind`,
   `series`, `options`, `position`, `params`. The `series` hook is not an escape
   hatch: `run_backtest_guarded` resolves it with
   `ind.get(k)` against the set those same `bars` produced.
   `needs_options()` reaches the one non-price source that exists, and
   `OptionsView` has no price series in it.
3. `run_backtest_guarded`'s `shared: Option<&IndicatorSet>` looks like a door —
   a caller could inject a series computed anywhere — but **every production
   caller passes `None`** (`sweep.rs` ×3, `hypotheses.rs` ×5, `search.rs` ×3,
   `routes.rs`, the `three_month*` binaries). Using it would have meant editing
   all of them, and a path that forgot would have run the same strategy against
   an all-`NaN` companion.

**The smallest extension, and the one I implemented.** Not a new `BarContext`
field: it is a public struct with public fields, constructed at **28 sites
across three crates** — 3 in production code (`engine.rs`, `fd-api/paper.rs`
×2) and 25 in test modules — and every one of the 28 would have to name the new
field for a capability 27 of them do not use. Instead, the extension follows the
precedent this codebase already set for exactly this problem —
`fd_strategy::news`, where the scheduled-news calendar is a process-wide
`OnceLock` installed by the binary that owns the data directory, because a
`Filter` is a value with no IO and every reader must see the same calendar.

* **`crates/fd-indicators/src/companion.rs`** (new): a `Companion`
  (`symbol`, `interval`, `bars`) in a `OnceLock`, installed once, idempotent for
  the same series and refused for a different one. `summary()` is the receipt
  line. Nothing in it touches a file.
* **one new `IndicatorDef`, `cmp`** (`period`, `atrPeriod` → `change`, `atr`,
  `close`): the companion's close-to-close change over `period` of **its own**
  bars ending at the same timestamp, its own ATR there, and its close. Because
  it is an ordinary indicator, it reaches **every** path — sweep cell, matched
  null, direction null, Workbench, paper loop — with no change to any of them.
  With nothing installed all three outputs are `NaN` at every bar.
* **`search.rs`**: `--companion=<SYMBOL>` reads
  `<data>/bars/<SYMBOL>-<interval>.parquet` from the **same store and the same
  interval** as the primary and installs it, printing
  `companion: XAGDUKA-15m 358,065 bars 2010-06-01 → 2025-09-22 (modal interval
  900s) from <path>` or `companion: none loaded — every cmp series is NaN`.
* **`routes.rs`**: `cmp` is offered in the chart's indicator menu only when the
  process has a companion installed. `fd-api` installs none, so without this it
  would offer a line that is `NaN` at every bar.

**The honest costs of doing it this way**, so the next agent does not discover
them:

* `compute_indicators` is no longer a pure function of its arguments for one
  of its 16 definitions. The module says so in its first paragraph. The
  alternative was worse, and `news` already made this trade for the same
  reason.
* **Nothing checks that the installed companion is the one the strategy meant.**
  `--companion=EURDUKA` with a method written for silver produces numbers, not
  an error. The receipt line is the only guard, and a receipt without it cannot
  be read. If this is ever used for a method that matters, the strategy should
  declare its intended symbol and the binary should refuse a mismatch.
* **A companion that stops before the primary does is invisible in the result.**
  It shows up as a thinning trade count, which looks like a period with fewer
  signals. Anyone running a companion method on a window must check the
  companion's own coverage over that window first. This one bit me in the
  planning: I could not verify that any companion series covers the withheld
  year without reading the withheld store, and I did not read it.
* **Two truths about causality**, both tested in
  `companion.rs::a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion`:
  truncating the primary changes no earlier value, **and** truncating the
  companion at the same wall-clock instant changes no value at or before it.
  The second is the two-series leak and it is the one that needed a test.
* A change measured across a break is refused rather than reported: the change
  over `period` bars is `NaN` unless the companion's own bar `period` places
  earlier is exactly `period` modal intervals older. The strategy applies the
  same rule to gold's own leg, checked against the window's own first step so
  no bar length is hard-coded.

### A second, separate expressiveness limit, found on the way

**`Exits::Engine` cannot express "a stop and no target."**
`Intent::Enter { target: None }` does not mean "no target" — `open_position`
reads it as "derive one from `reward_risk`" (1.8R). Only `Exits::Strategy`
escapes, and that removes the engine's stop and clock as well. It matters here
because the cost argument of angle 1 wants a wide stop and no target, and that
shape is unreachable without also giving up the engine's exit. I did **not**
work around it: the strategy as committed supplies a stop and lets the engine
place the target, so its geometry is the same as every other method's on the
same leaderboard. Reported as a limit, not patched.

---

## 4. The relationship, measured, with counts

Design window: `XAUDUKA-15m` × `XAGDUKA-15m`, 2010-06-01 → 2025-09-22,
**356,618 paired bars**, 15.31 years. Quantities: gold's change over `k` bars
divided by its own ATR(14) (`ug`, in gold ATRs); silver's change over the same
`k` of its own bars ending at the same timestamp, divided by its own ATR(14),
signed by gold's direction (`us`, in silver ATRs). Forward figures are
open-to-open from the fill bar, in **gold points**, with the 0.28 round trip
charged where stated.

### 4.1 The second series is doing the work. That part is unambiguous.

At the same gold threshold and the same horizon, the single-series control —
fade **any** gold move — against the two-series condition:

| k (bars) | h (bars) | \|ug\| > | silver condition | n | gross, gold points/trade |
|---|---|---|---|---|---|
| 2 | 4 | 1.5 | **none (control)** | 32,212 | **+0.002** |
| 2 | 4 | 1.5 | signed \|us\| < 0.3 | 2,041 | **+0.143** |
| 2 | 8 | 1.5 | none (control) | 31,679 | **+0.010** |
| 2 | 8 | 1.5 | signed us < 0.3 | 2,026 | **+0.434** |
| 2 | 8 | 1.5 | signed us < 0.0 | 787 | **+0.842** |
| 2 | 8 | 1.5 | signed us < −0.3 | 319 | **+0.973** |
| 2 | 16 | 1.5 | none (control) | 30,458 | +0.022 |
| 2 | 16 | 1.5 | signed us < 0.0 | 766 | **+1.256** |
| 4 | 4 | 2.0 | none (control) | 36,801 | −0.004 |
| 4 | 4 | 2.0 | signed us < 0.3 | 1,746 | +0.131 |

The control is at zero everywhere and the conditioned slice is not, and it
grows monotonically as the silver condition tightens. The complementary slice —
gold moved and silver **went along** (signed `us` > 0.7·A) — is +0.005 to
+0.118 points gross at `h` = 4…16 over 9,100–54,300 trades: a confirmed move
neither continues nor retraces. So the partition behaves the way the mechanism
says it should, in all three directions at once.

Sign stability, in gold ATRs so that eras with ATR(14) medians of 1.706 /
1.196 / 2.412 points are comparable: across the 184 cells I printed with at
least 40 signals a year, roughly 70 are positive in **all three** five-year
eras 2010–2015 / 2015–2020 / 2020–2025. The cells are nested, so that is not 70
independent tests — but a sign that survives a whole parameter block and three
disjoint five-year windows is not one cell's luck.

### 4.2 And then the bracket takes all of it

Every number above is a **mean forward move** over a fixed horizon. A trade is
not that. Simulating the engine's own exit — fill at the next bar's open, a
stop, a target, stop first when one bar spans both, the configured four-hour
maximum hold, one position at a time, 0.28 charged — over 45 (k, A, B, stop,
target) cells: **every one has profit factor ≤ 1.02 and negative expectancy.**
Widening the stop and dropping the target, the best of a further 20 cells
reaches profit factor 1.174 at expectancy +0.053 R, or 1.203 at +0.031 R —
never both legs at once.

The reason is visible with the bracket removed. With no stop and no target
reachable, the same cell nets +0.139 points a trade at **profit factor
0.999**: the positive mean is a few large retracements, not an edge in most
trades. A stop set inside four hours of gold noise converts exactly the
drew-down-then-reverted paths — which is what the mechanism predicts — into
losses. A profit factor of 1.0 gross cannot be turned into 1.2 net by any
choice of stop and target.

Year by year, the strongest fixed-horizon cell (k=2, A=1.5, B=0.0, h=16, one
position at a time) is negative in **7 of 16 calendar years**, and 2011, 2024
and 2025 carry the total. That is a fat tail in high-volatility years, not a
thing that pays every year.

### 4.3 The dollar version is not there at all

`XAUDUKA-15m` × `EURDUKA-15m`, same window, the tradable test directly (stop,
target, four-hour hold, 0.28 charged), four decisions — fade or follow a gold
move the dollar did or did not confirm — over **96 cells** with 2,218 to 8,598
trades each:

**Every one of the 96 has profit factor below 1.00 and negative expectancy.**
Best seen: 0.984. The earlier fixed-horizon scan over 50 cells says the same
thing more mildly (gross ≈ 0 to +1.3 points on the 3-year Vantage sub-window,
negative net on all 25 fifteen-year cells). Combined with the shift scan's flat
±1…±4 correlations, the conclusion is not "the parameters were wrong":
**at fifteen minutes there is no gold–dollar decision here in either
direction.** The `XAUUSD = XAUEUR x EURUSD` identity is real; the timing
mismatch it would create is either absent at this resolution or smaller than
0.28 points.

---

## 5. Design-window numbers from the audited path

`crates/fd-strategy/src/companion_unconfirmed.rs`, id `companion-unconfirmed`,
at its declared defaults — `moveBars` 2, `moveAtr` 1.5, `companionAtr` 0.0,
`stopAtr` 4.0, `atrPeriod` 14, `companionAtrPeriod` 14 — through
`run_backtest_guarded`, spread 0.28, binary built in this worktree
(`E:/rust/fd-wt-dm4/target/release/search.exe`, and the receipts name the
strategy, so it is not a sibling's build answering):

| run | bars | trades | win% | profit factor | expectancy |
|---|---|---|---|---|---|
| XAUDUKA 15m, 2010-06-01 → 2025-09-22, guards **off** | 362,803 | 716 | 52.7% | **1.126** | **+0.042 R** |
| XAUDUKA 15m, same, guards **on** | 362,803 | 685 | 52.6% | **1.072** | **+0.025 R** |
| XAUUSD 15m, 2022-06-16 → 2025-09-22, guards on, companion XAGDUKA | 77,278 | 216 | 49.5% | **1.087** | **+0.033 R** |
| XAUDUKA 15m, guards on, **no companion installed** | 362,803 | **0** | — | — | — |

**The gate is 30 trades / profit factor 1.2 / 0.05 R. Every row fails two of
the three legs, on the window the parameters were chosen on.**

The last row is the safety property working: no companion means no trades, not
trades at a fabricated zero. A run of zero trades from a missing companion and
a run of zero trades from no signals are told apart by the `companion:` receipt
line and by nothing else.

**Stop distance, as the coordinator asked, in both units.** `stopAtr` = **4.0
ATR(14)**. At the per-era median gold ATR(14) on 15m bars — 1.706 / 1.196 /
2.412 points for 2010-2015 / 2015-2020 / 2020-2025 — that is a stop of
**6.8 / 4.8 / 9.6 points**, so the 0.28 round trip is **4.1% / 5.8% / 2.9% of
R**. It is a wide stop, and it is wide because §4.2 measured that a tight one
destroys the effect, not because a percentile wanted it wide.

### The two nulls, and what I do and do not claim from them

Guards on, XAUDUKA, 200 assignments, `--mode=null-dir`:

| arm | trades | profit factor | direction-null percentile |
|---|---|---|---|
| the rule as written | 685 | 1.072 | **100th** (null p05 0.799, p50 0.872, p95 0.954, **max 1.012**) |
| the same rule with the companion condition removed (`companionAtr=999`) | 12,659 | 0.924 | 88th, p = 0.115 (null p50 0.873, p95 0.938, max 0.950) |

Read carefully, because this is the one place a number here looks better than
it is.

* The **direction** null holds entries, stops and targets fixed and flips only
  the side, so it *is* cost-matched by construction — the coordinator's warning
  about `hypotheses::control_for` pinning `RandomEntry`'s `stopAtr` at 1.5
  applies to the count-matched random-entry null, which I have not run and am
  not quoting. I have not verified that claim; I have avoided depending on
  either null.
* What the table shows is a real thing: the two-series direction rule is
  outside its own cost-matched null, above the maximum of 200 coin flips, while
  **the same rule with the second series taken out is inside its own null.**
  The second series carries direction information the first does not.
* **And it still does not matter.** The level is 1.072 against a gate of 1.2.
  The direction null's median sits at 0.872 because the cost is charged in every
  arm, so beating it says the sign is right, not that the trade pays. **I claim
  nothing from a percentile, and if this method were being proposed on one it
  should be refused.** It is not being proposed.

### The private harness agrees with the audited path

Everything in §4 came from a numpy harness, which is comparable to nothing on
its own. Predicting the engine's exact geometry (stop 4 ATR off the signal
close, target 1.8 R, 4-hour hold, 0.28, one position at a time) it said **718
trades, profit factor 1.127, +0.0382 R**; the engine said **716, 1.126,
+0.042 R**. Two trades and 0.001 of profit factor apart. The scans in §4 can be
read as approximations of what the engine would have said.

---

## 6. Variants tried — my own multiplicity

**About 700 parameter cells on the design window**, in six families. The
design-window numbers in §5 cannot be read without this figure, which is why it
is a section and not a footnote.

| family | cells | what varied |
|---|---|---|
| gold-vs-dollar residual, fixed horizon | 50 | 2 windows × 5 lookbacks × 5 horizons |
| gold-vs-silver confirmed/unconfirmed, fixed horizon | 108 | 3 × 3 × 3 × (control + 2 unconfirmed + confirmed) |
| era-stability and long-horizon rescans | 94 | lookback, horizon, both thresholds |
| the wide scan with a ≥40-signals-a-year floor | 184 | 5 lookbacks × 6 horizons × 3 × 3 |
| bracketed-exit and wide-stop simulation | 78 | stop, target, max hold, plus a gross-PF table |
| gold-vs-dollar, tradable test | 96 | 4 decisions × 2 × 2 × 2 × 3 stops |
| audited engine runs | 5 | 3 leaderboards, 2 direction nulls |

The cells are heavily nested — a `|ug| > 2.0` slice is inside its own
`|ug| > 1.5` slice — so this is nowhere near 700 independent tests. It is still
700 looks, and the best cell of 700 looks on one window is worth roughly
nothing by itself. **The only reading of §4 I would defend is the one that does
not depend on picking a cell**: the control sits at zero and the conditioned
slice does not, in every cell and in all three eras. That is the finding. The
best cell's profit factor is not.

The parameters in §5 are a grid *centre*, not a maximum — `moveBars` 2 of
{1,2,3,4}, `moveAtr` 1.5 of {1.0,1.5,2.0}, `companionAtr` 0.0 of
{+0.3,0.0,−0.3} — chosen that way before the engine was run so that the number
reported would not be the best of anything. `stopAtr` 4.0 is the exception and
it was chosen with sight of §4.2, which measured that tight stops kill the
effect; it is the widest of {2,4,6} that still leaves expectancy measurable in
R, and it should be read as fitted.

---

## 7. The verdict: zero methods

The registration asks for methods that clear four legs on a withheld year at
frozen parameters. What I have is:

1. a real relationship — the second series moves gross return from 0.00 to
   +0.4…+1.3 points a trade, consistently, across three disjoint five-year
   windows and a whole parameter block;
2. which is a **fat-tailed mean, not a profit factor**: gross profit factor
   0.999 with the bracket removed, 1.07–1.13 through the engine;
3. which therefore **fails two of the three gate legs on the window it was
   designed on**, at a grid centre and at the best cell of ~700 alike;
4. and whose one flattering figure is a percentile I have just argued nobody
   should accept.

A method that fails the gate in sample has no business spending one of the
programme's eight hypotheses. Proposing it would be filler, the brief says so,
and the brief is right.

**What this contributes to the programme's own question.** The registration
sets up two explanations for 25 closed registrations with no survivors: (1) the
mechanisms tried were too well known, or (2) a 15-minute edge net of 0.28 is
thin to nonexistent. Angle 4 is as far from the textbook as this desk's data
reaches — a relationship between two instruments, absent from all 21 registry
mechanisms — and the result is a *found* relationship that is unambiguously
real and unambiguously unprofitable, with the entire gap sitting in the cost
term: the direction rule beats a cost-matched coin flip at the 100th percentile
and still returns a profit factor of 1.07, because the null's own median is
0.872. **That is evidence for explanation 2, produced by an angle that cannot
be accused of being a tired mechanism.** It is worth more to the record than a
ninth hypothesis would have been.

## 8. What is committed

* `crates/fd-indicators/src/companion.rs` — the second-series plumbing, with
  the two-sided causality test.
* `crates/fd-indicators/src/lib.rs` — the `cmp` definition and its dispatch.
* `crates/fd-strategy/src/companion_unconfirmed.rs` — the method, registered,
  running through the guarded path, with the unit tests including
  *a missing companion bar takes no trade rather than treating it as flat* and
  *a move straddling a break is not a thirty-minute move*.
* `crates/fd-backtest/src/bin/search.rs` — `--companion=<SYMBOL>` and a
  `data:` line naming the store the run read.
* `crates/fd-api/src/routes.rs` — `cmp` off the chart menu unless this process
  can compute it.

The strategy is committed **because the plumbing is only demonstrated by
something using it**, and because a later agent that wants to test a two-series
idea should start from a working example rather than from this note. It is
registered at parameters that fail the gate, and its module docs point here. It
is not a candidate and it is not proposed.

---

## 9. Two operational notes for whoever merges this

**The workspace test ran out of disk, not out of correctness.** `E:` reached
**301 GB used of 301 GB, 640 KB free** during `cargo test --workspace`, and
every error in that run was `rustc-LLVM ERROR: IO failure on output stream: No
space left on device` — no compile error and no failing assertion. The cause is
worth recording: `CARGO_TARGET_DIR` is **unset**, so each agent worktree builds
its own `target/`, and at the time of the failure `flowdesk/target` held 32.3 GB
and `fd-wt-dm1/target` 11.4 GB against 18 worktrees on the drive.
`crates/CLAUDE.md` says agent worktrees share a target dir "so three of them
cannot fill the disk"; they do not share it, and the disk filled. I deleted
nothing outside my own worktree. The suite was re-run once space came back.

**This branch is based on `253e94a`, before `main` merged designed-1 and
designed-2.** My `search.rs` touches two places — a `data:` receipt line near
the top of `main` and a new `load_companion`. If designed-1's own data-root line
lands in the same spot the conflict is one line and either copy will do; the
`load_companion` function and its call site are independent.
