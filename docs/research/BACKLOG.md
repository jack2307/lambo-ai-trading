# Research backlog

Ideas waiting for a pre-registration. One line each: the claim and why it
would exist. The `/research next` loop takes the top unchecked item, writes
its hypothesis file, and runs the pipeline. Add reasons, not just names — an
idea without a reason is a search.

Closed ideas move to the bottom with a pointer to their decision record, so
`historian` does not let them come back under a new name.

## Open

- [x] **Gold's spread by hour is measured — and the week of logging was the
  wrong instrument for it.** (2026-09-15, receipt
  `docs/research/runs/2026-09-15-venue-residual/spread-probe-30d.txt`.) The
  168-hour logger would have produced about five observations of the 22:00 UTC
  hour. `py/ingest/mt5_spread_probe.py --days=30` was already written, already
  read-only, and reads tick **history**: 11,576,449 ticks, p50 **$0.19–0.20**
  through Asia, London and New York, **$0.25 at the daily stop, $0.26–0.27 at
  the reopen**, max 0.27 anywhere. The evening is 30% dearer than the day and
  **cheaper than the $0.28 blend this repository has been charging everywhere** —
  the thin-hour cost fear was the wrong way round. The logger runs to 2026-09-22
  as confirmation, not as the decision.
- [ ] **Re-price the 438 evening trades of `2026-09-15-pair-residual` at $0.26**,
  now that the number exists, and say whether the overnight gold–silver
  dislocation survives its own hour's cost. **The silver leg is still blocked and
  it is an operator decision, not a research one:** `XAGUSD.sc` answers
  `symbol_info` but returns no tick, and `symbol_select` is not on the approved
  read-only call list for this machine's live account — the symbol must be added
  by hand in the terminal, or that one call explicitly permitted.
- [ ] **If the evening spread does not eat it, re-register the break-crossing
  hypothesis properly**: clock-based windows rather than bar counts, a one-bar
  entry latency (the effect survives one bar at +2.298 bp / 54.55% and fails at
  two), the hour declared in advance, and the gated statistic named before the
  cell is chosen. Note it would be re-entering the territory of
  `2026-09-13-close-reopen-drift`, which failed its gate as a unit.

- [ ] **Every registration states its minimum detectable effect BEFORE its
  falsifier** (`docs/decisions/2026-09-15-intraday-frontier.md`, and it is the
  rule three dead ideas bought). The table is there: at five minutes on this
  account a mechanism needs ~0.9 bp of gross edge per trade, at thirty minutes
  ~2.0 bp, and raising a threshold makes the bar HIGHER because the sample falls
  faster than the effect can grow. Say what prior makes the required size
  plausible, or do not write the registration.
- [x] **Short-horizon reversal after a large two-sided quote move — closed by
  arithmetic.** The one interval in `2026-09-15-quote-asymmetry` that excluded
  zero: reversion is worth **+0.41 bp** and the round trip costs **0.455 bp**.
  The effect is smaller than the cost, more data cannot move that, and every
  threshold where it might exceed the cost is one where five months cannot
  measure it (4σ: needs 1.53 bp detectable, would have to be 3.7x its 2σ size).
  Not a test — a subtraction.
- [x] **Retail order flow at this dealer — not in the data.** The one variable
  with a payer story that is not speculative. `copy_ticks_range` returns `last`,
  `volume` and `volume_real` all zero on `XAUUSD.sc` and `EURUSD.sc`, and
  `TICK_FLAG_BUY/SELL/LAST/VOLUME` are never set (observed flags: 1028, 1154,
  1158 — quotes only). Probed 2026-09-15. Nothing reopens this without a
  different data source.
- [x] **The options tape question is now REGISTERED AND LOCKED**
  (`docs/hypotheses/2026-09-16-dealer-hedge-demand.md`, commit ee445ee,
  2026-09-16) — written three months before the data it runs on exists, which is
  a stronger protection than anything applied afterwards. **It may not be run
  until the tape holds 500 non-overlapping bursts**, about December. Sized from
  inventory before any return was read: gross hedge demand **502,395 oz/day =
  2.0% of COMEX volume**, **net 23,899 oz = 0.096%**, and a single $100k print
  **1,223 oz = 0.005%** — so the individual print cannot be the event and the
  registration says so in advance; the object is the signed residual of a flow
  that cancels **21:1**. MDE 1.59 bp at 500 bursts against a square-root-impact
  sizing of 2–4 bp. **The collector must not be stopped**: `fd-ingest --bin
  collect --market=gold` and `--market=btc`; history not captured as it happens
  is gone.
- [ ] **Four ideas killed by inventory on 2026-09-15/16, recorded so they are not
  re-proposed.** (a) **Quote-revision runs**: the distribution is geometric with
  median 2 and **max 8** — there is no "the dealer repeated a decision eight
  times", so the persistence story has no events. (b) **Quote freezes**: the
  longest silence in five sampled days is **10.8 s**, the resume spread is only
  **1.05x** baseline (so not defensive), and ≥5 s gaps in busy hours are **2 of
  96** — a silence here is an empty market, not a decision. (c) **Retail order
  flow from the IB book**: the Vantage rebate report carries `totalLots`,
  `totalrebate` and per-category volume — **volume, no direction, no per-trade
  time**. It cannot supply the missing variable and the client data was
  therefore not touched. (d) **Dealer positioning before a release** — ~50 events
  in eight months of ticks needs an edge a third the size of the release.
- [ ] **The options tape instrument work, if it is wanted before the lock lifts.** Dealer hedging after a
  large print is mechanical, price-insensitive flow — the only payer story on
  the shelf that does not require someone to be wrong. Gold holds 9 days, BTC 5,
  both live. Build and validate the measurement (large print -> hedging flow in
  the following N minutes) against the days already on disk, **declare nothing**,
  and register when the tape reaches three months.
- [x] **Two of them now do, and both live findings survived it**
  (`docs/decisions/2026-09-15-selftest-audit.md`, 2026-09-15). `news_drift.py`
  recovers a planted drift to the penny with a monotone percentile
  (0.00/0.90/**2.70**/8.70/17.60 at -1.0/-0.5/**0**/+0.5/+1.0 dollars), and on
  twenty **fake** release-date sets reads min 15.70 / median 50.35 / mean 48.86
  with **zero below the 5th percentile** — so the pre-NFP drift is the release,
  not the instrument. `friday_cells.py` on forty fake release assignments reads
  median 40.62 with 1 below the 5th (expected 2.0) and 1 below the 1st
  (expected 0.4): within noise at both tails. **Fault 9 measured and immaterial
  to the live number** — 0 of the 25 later-Friday releases carry a second
  high-impact event, so applying the screen to both arms moves the percentile by
  **0.00**. The fault stays open below because it will bite another cell.
  Also found: the as-filed cell-C percentile wanders 4.7 / 6.1 / 6.55 across
  seeds and draw counts, two points on a figure quoted to one decimal.
- [ ] **The remaining three still have no self-test**: `pair_residual.py`,
  `fx_window_drift.py`, `venue_residual.py`. They hold closures rather than live
  findings, so a fault there buries rather than invents — but the gold-silver
  overnight dislocation is an open observation resting on `pair_residual.py`,
  and a buried effect is still a wrong answer. Pattern to copy:
  `scripts/news_drift_selftest.py` (plant a known size, then feed fake dates).
- [ ] **A measurement script must find an edge you plant in it** (the strongest
  thing to come out of 2026-09-15, and no script but one has it).
  `scripts/quote_asymmetry_selftest.py` keeps every real thing — minutes, quotes,
  broker-clock hours, volatility — and replaces only the signal column with
  `noise + β × forward return`, then runs the registered machinery unchanged. It
  showed the harness maps realised gross onto percentile monotonically
  (+0.129→66.06, +0.197→89.23, +0.341→98.53, +0.418→99.33), finds a planted
  +0.34 bp edge above the gate, and never scores pure noise above it across six
  seeds. That converts "my null result is not a bug" from an argument into a
  receipt. **`pair_residual.py`, `news_drift.py`, `friday_cells.py`,
  `fx_window_drift.py` and `venue_residual.py` have no such test**, and three
  faults shipped inside one file in one day says they need one.
- [ ] **Size every intraday registration against the noise floor before writing
  it** (2026-09-15-quote-asymmetry). At ~5,000 non-overlapping five-minute trades
  on this account the per-trade dispersion is 15.5 bp, so the standard error of
  the mean is **0.22 bp** and the 97.5 gate sits at about **+0.30 bp of realised
  gross** — against a measured round trip of 0.455 bp. A registration whose
  plausible edge is below that floor is not a test, and the sample it would need
  should be computed in the hypothesis file rather than discovered in the record.
- [ ] **A direction gate belongs on gross, a tradability gate on net; say which
  is which** (design fault, same record). The registration promised gate 4 the
  role of deciding "direction exists, not a trade", then wrote it on net returns,
  so it could not play that role: clearing gates 2+3+4 together needed a gross
  edge of ~2.4x the round trip (joint power ~14% at the observed effect size),
  while gate 3 alone had ~85%. Every future falsifier states the two separately.
- [ ] **A construction that does not cancel opposing revisions inside the
  minute** (the only reopening path for the tick family). 91.2% of one-sided
  point movement nets away before `os_sum` is formed. Counts, run lengths, or
  the largest revision of the minute are all different mechanisms rather than a
  new threshold — and the out-of-sample window 2026-06-16 → 2026-09-12 has never
  been opened, so a re-registration has a clean market to be judged on.
- [ ] **Commit the code that produced a receipt before replacing it** (process
  fault, same record). The VOID run is preserved but the code that made it never
  entered git, so the amendment cannot be audited against the thing it amends.
  `git log --follow` shows both files entering at the amendment commit.
- [ ] **`mt5_quote_features.py` stamps `tz="UTC"` on broker-clock times.**
  `copy_ticks_range` returns the server clock (Vantage: UTC+3 summer, UTC+2
  winter) and the ingest stores it unconverted — the sessions running 01:00–23:58
  are the tell. Matching on it is fine and arguably better (phase of the trading
  day, daily stop at the day boundary), but the label is a lie and the next
  reader will believe it. Either convert or rename the column.
- [ ] **No trade may span a hole, in any measurement script** (risk,
  data-integrity and execution-realist, independently, on
  `2026-09-15-venue-residual`). `fires()` checked freshness on the **signal** bar
  and exited `k` rows later regardless: 27 of 985 one-hour holds crossed the
  daily stop, **eleven of them held over a weekend, the longest 51 hours**,
  carrying 16% of the net — while `config/default.toml` sets
  `flat_before_weekend_hhmm = 1655` precisely to forbid it. Fixed in
  `scripts/venue_verify.py` (`clean_exit`); **`scripts/pair_residual.py`,
  `news_drift.py`, `friday_cells.py` and `fx_window_drift.py` have not been
  audited for it.** This is the same break-crossing fault that closed
  `2026-09-15-pair-residual`, now found in a second instrument.
- [ ] **A thin-hour mechanism needs an hour-matched null, not a volatility one**
  (data-integrity, same record). The vol-decile control was chosen because
  |skew| grows with volatility — but `sd(resid)` varies **6.43x** across UTC
  hours while the ECN bar's range varies only **1.46x**, so the deciles are a
  liquidity proxy: decile 0 is 73.6% evening bars at mean tick volume 485,
  decile 9 is 20.7% at 2,621. Permuting signs *inside* the thin-hour bucket
  cannot break a thin-hour mechanism. The repository's standing rule already
  says a session gate needs a session-matched null; this extends it to any
  control variable that is really a proxy for the hour.
- [ ] **Print the null's centre, and a day-block interval, next to every
  percentile** (adversary and risk, same record). A book 73% long in a year gold
  rose 37% earns a drift the permutation is already centred on — the percentile
  stays honest, the headline basis points do not, and +1.000 bp read as edge was
  +0.587 bp of bull market. Separately, a sample **94% of whose trades share a
  calendar day with another** is not n independent bets: the day-block bootstrap
  put 95% intervals of `[-0.48, +4.63]` and `[-0.41, +1.16]` around percentiles
  of 99.05 and 98.10. Both belong in the receipt, beside the percentile, always.
- [ ] **A concentration gate, declared before the run** (risk, same record).
  When the best 5% of trades are 271% of the net, one month is 98% of it and one
  day 75%, four days have been measured rather than an effect. Risk will not
  clear anything whose single best month exceeds ~40% of its total; write that
  into `TEMPLATE.md` so it is declared rather than discovered.
- [ ] **The Monte-Carlo error is printed at the wrong p** (data-integrity and
  adversary, same record). `venue_residual.py` hardcodes `0.5/sqrt(draws)`, the
  SE at p=0.5, printing 0.16 pp where the registration declared 0.07 and the
  true binomial SE at p=0.975 with 100k draws is 0.047. Nothing turned on it
  here; a run contradicting its own registration in print is still a fault.
  Fixed in `venue_verify.py`; not in the scripts it was copied from.
- [ ] **Automate the demo lock** (risk, same record, and it is the only safety
  item on this list). `py/live/mt5_executor.py:139` refuses to run against a
  non-demo account, and that refusal is the single thing between this repository
  and a **live** Vantage terminal on this machine. It has no automated test —
  `docs/paper/DESIGN.md` describes a manual procedure. A guard that is
  remembered rather than tested is not a guard.
- [ ] **Make `--guards` the default in `search`, not a flag** (risk, same record;
  the existing "every receipt is unguarded" item, now with a number).
  `search.rs:110` defaults it off. Applying the repository's **own configured**
  guards to the venue study's trade list turned its five-minute row from +0.33 bp
  to **−0.24 bp** — a 30-minute cooldown is structurally incompatible with a
  five-minute hold, and `max_trades_per_day = 4` deletes a 39-trade day. A
  measurement whose sign flips under the project's own limits is not a candidate
  for anything, and the Python measurement scripts have no guard concept at all.
- [ ] **The residual gate on an unread instrument** (adversary,
  `2026-09-15-nfp-cross-asset`), and it is the only experiment left that could
  settle the pre-NFP thread. Silver's fall could not be distinguished from
  "gold times its beta", because the registration gated the instrument rather
  than its excess over gold. Fix that: on platinum, copper or a dollar-side
  pair — none of which is on disk, so this starts as a data task — fit beta on
  **quiet Fridays only**, where no release information exists, then gate the
  sign and the up-rate of the **residual** of (instrument_bp − β·gold_bp).
  Declare the median, and run the permutation at **100,000 draws**, where the
  standard error is 0.07 percentage points rather than 0.74. A residual that
  falls is a second factor; a residual at 50% is what silver gave and closes
  the thread.
- [ ] **Every percentile gate in this repository states the precision it
  requires** (fault 11, `2026-09-13-instrument-faults.md`). `news_drift.py`
  defaults to 1,000 draws, whose standard error is 0.74 percentage points, and
  a gate written at "the 5th percentile" cannot be judged by it — silver's run
  turned on which seed the default happened to be. Make `--draws` a declared
  quantity in every registration, and consider raising the default.
- [ ] **Bitcoin has never been read for a news question either** (noted while
  checking what feeds exist). `BTCUSD` and `BTCUSDT` are on disk. A
  24/7 dollar-denominated asset with no metals exposure separates "the dollar
  is bought before the print" from "the metals complex is sold" better than
  silver could. **Register it before reading it**, with the residual design
  above rather than the instrument-level design that failed here.

- [ ] **Does the hour before 08:30 New York fall before ANY macro release,
  rather than before this one?** (the successor to
  `2026-09-14-nfp-vs-first-friday`, named independently by the adversary and
  by news-desk). Reason: it is the only hypothesis that fits all three
  findings at once. Gold's 07:30 → 08:30 hour falls on US employment Fridays;
  it falls on "quiet" first Fridays, eleven of which carry Canada's Labour
  Force Survey or US Personal Income/PCE at 08:30; and it does **not** fall on
  quiet Monday-to-Thursday turn-of-month days (median −0.005 over 274 days) —
  a calendar cannot tell a Friday from a Tuesday, an 08:30 release can. This
  is a mechanism rather than a window, it has five to ten times the sample,
  and it is the first idea in this loop that was proposed by a review rather
  than by a search. **Prerequisite, and it is a data task not a research one:**
  extend `data/news/events.csv` with Canada's Labour Force Survey and US
  Personal Income/PCE release dates (`docs/news/README.md` records that the
  calendar holds five event names and two currencies). **Declare before
  reading:** the median as the statistic, not the mean (the mean was the
  statistic that misled the parent); a day-of-month-matched control; and the
  baseline restricted to mid-month days.
- [ ] **A screen is a property of a comparison, not of a cell** (fault 9,
  `2026-09-13-instrument-faults.md`): `scripts/friday_cells.py` excludes days
  carrying a second high-impact release from its control cells and not from
  its treatment cells. Fix it there, and check every other instrument in
  `scripts/` for a filter applied to one arm only.

- [ ] ~~**Asian-range breakout at London open.**~~ Withdrawn 2026-09-13: the
  breakout family is closed (`2026-09-13-volcond-breakout.md`); a fourth
  range on the same mechanism needs a new reason, not a new window.
- [ ] **ICT variant: order block instead of FVG.** Reason: the manual's own
  alternative entry zone; the chain is already ported.
- [ ] **The sized random-hold null holds the method's mean hold, not its
  distribution** (adversary, `2026-09-14-tsmom-eurusd`; the mean was fixed
  at `f65a061`): a fixed 23-day random hold cannot produce a 356-day +23R
  interval, so the null's PF is bounded where the method's is not and a
  `tsmom` row sits at the 100th on its exit rule, not its sign. Draw each
  control hold from the method's realised hold distribution (a list, not a
  scalar). Also: the scan-back matches the lookback by day, so the
  reference close is 23:45 New York, not the rebalance minute
  (data-integrity, same record).
- [ ] **A direction test that three trades cannot saturate** (adversary,
  `2026-09-14-tsmom-eurusd`): when three trades are the net, permuting
  sides has about one answer in eight and cannot reach 0.05. Candidates,
  each its own instrument change and not a re-tune: a sign test on R; a
  stale-signal null (same intervals, the side from one lookback earlier);
  the literature's pooling across a diversified set.
- [ ] **Filters gate entries only; a self-managed method exits on the Sunday
  reopen** (data-integrity, `2026-09-13-tsmom-silver`): `weekdays` does not
  stop `tsmom` from acting on a sign flip at Sunday 18:00 as an exit. Decide
  whether an exit gate is wanted and say so in the filter's doc either way.
- [ ] **The receipts' profit factor is a compounded-dollar number**
  (adversary, `2026-09-14-volman-box`): with lots at 1% of compounding
  equity, a −0.14R/trade row's PF is weighted 47% on its first seven
  months, and its direction percentile reads 1st where the R-weighted one
  is 38th. Print PF and the direction null on R (or on fixed lots) next to
  the dollar figures in `search`'s tables and in `permuted_sides_pf`; then
  re-read every thousands-of-trades negative in the loop against the R
  column (the verdicts will not move — a loser is a loser in both units —
  but the percentiles quoted for them will).
- [ ] **`session-hold` fires at the reopen on a feed-gap day** (news-desk,
  `2026-09-14-fx-local-hours` addendum): on Christmas-week days with no
  bars until 17:00 New York the "first bar of the window" was the 17:00
  bar, giving fifteen-minute holds and one Sunday hold under `weekdays`.
  Require the window's first bar to be at most one bar-interval after
  `from`, or skip days whose first bar is after `to`. Immaterial to every
  record so far (five holds, −$26); a correctness item.
- [ ] **Blackout that flattens** (owner's intent for the paper phase): the
  `news:` filter blocks entries; a hold that spans a release is avoided by
  `before = hold length`. A live bot also needs a guard that closes an open
  position before a release (and does not re-enter until `after`), which
  is the unrealised-loss / weekend guard family — the live loop does not
  exist yet. news-desk publishes the schedule; the runner must enforce it.
- [ ] **Intraday reversal on gold, as a sign claim only** (adversary,
  `2026-09-14-intraday-momentum`): the day-so-far sign (08:30 → 15:00)
  predicted the opposite of the last hour (15:30 → 16:45) on 2010–18 at the
  1st percentile of its own sides — one extreme of six two-sided draws, 64%
  of it 2011, PF 0.765 where it was seen. Register the mirror (side = −sign,
  every parameter frozen) on gold 2018-06 → 2026-05 and silver 2010–2026;
  falsifier ≥ 95th of the direction null on both windows or ≥ 99th on one;
  state before the run that the PF gate is unreachable at $0.28 on a $1.27
  window, so the outcome is "direction exists / does not", never a paper
  run. Low priority.
- [ ] **Trend pullback on BTC with a time exit**: the side beat its mirror
  (99th) while the swing-stop structure lost 0.38R a trade; a re-registration
  needs an exit with a reason, not a wider stop. `2026-09-13-trend-pullback.md`.
- [ ] **Guards for open positions, and guards in the research runner**
  (risk, `2026-09-13-close-reopen-drift`): the daily-loss limit sums closed
  trades and nothing can close a self-managed position; no maximum hold, no
  weekend flag, no notional cap; `search` does not pass the configured guards,
  so every receipt is unguarded. Instrument work: enforce with tests, then
  say in the receipts whether guards were on.
- [ ] **The sizing range counts Sundays as days** (data-integrity,
  `2026-09-13-close-reopen-drift`): `average_day_range` uses the New York
  calendar day, so a Sunday evening's six hours enter the 20-day mean as a
  full day — R 6–9% too small, uniformly. Instrument: use the 17:00 session
  day, or skip days with fewer than N bars. Ranks unaffected; sizes are.
- [ ] **Direction null on the walk-forward's selected cell** (adversary,
  recent-year screen): `null-dir` runs the defaults over the whole window,
  so for a gridded row it scores different trades than the table. An
  instrument change.
- [ ] **Options: cluster-at-level with the accumulated tape** — *blocked until
  the collector has months of tape*; check `search --market=btc` tape line.

## Infrastructure (found by reviews; not hypotheses)

- [x] `search --from/--to` and `[run] *_from/*_to` in the runner (2026-09-13).
- [x] `dukascopy_to_parquet.py` drops flat zero-volume bars — 334,406 of them
  (weekends and breaks), not the 9,732 first counted; XAUDUKA-1m/5m
  regenerated (2026-09-13). Records before this date ran on the filled series.
- [x] The four unread `[trading]` keys deleted; they return with the code
  that enforces them (2026-09-13).
- [x] `null-dir` applies the batch's filters via `--filters=` (2026-09-13).
- [x] Out-of-sample stage also replays the registered parameters with no
  re-selection (`search --mode=hypotheses --fixed`, receipt
  `out-of-sample-fixed.txt`) (2026-09-13).
- [x] Dukascopy converter docstring: the CSV has no volume column; every
  O=H=L=C bar is dropped (data-integrity, London pass).

- [x] A null for drift claims: `null-hold`, random holds of the row's window
  length, chosen automatically for `Exits::Strategy` methods with no grid
  (2026-09-13).
- [ ] `run_null_control` (`search --mode=null`) skips options strategies;
  when the tape covers months this is the control the founding thesis needs
  (adversary, 2026-09-12).

- [x] **Volatility-targeted sizing for strategy-managed exits**: a sizing
  stop of N daily ranges carried by the entry, used for lots and R, never
  enforced; the random-hold control sized the same way (2026-09-13,
  `edf7d96`).
- [x] Three engine faults fixed with pinning tests — sizing stop enforced on
  a self-managed hold, presets at the default value not pinned, a hold open
  at a fold's end running to the end of data — and the direction null for
  self-managed methods rebuilt as a permutation of the method's own trades.
  `docs/decisions/2026-09-13-instrument-faults.md`.
- [x] `research-run.py --stage dir` re-runs only the in-sample direction
  nulls (after a null amendment).
- [x] **Time-series momentum on gold** — re-registered as `2026-09-13-tsmom-2`
  with the sizing above and the 2018–25 window; see its record.

## Closed

- [x] Does a one-sided quote revision say where the price goes next? (the second
  mechanism invented rather than read, and the first study in this loop that
  assumes **no spread anywhere** — a long is filled at the ask and exits at the
  bid, from the tick that would have filled it) → **no.** 56.8M ticks over 104
  sessions, 23.8% of revisions moving one side only, classified correctly
  (0.071% disagreement with the broker's own flags, all of it one systematic
  re-send) and rebuilding byte-for-byte from a fresh fetch. No cell reaches the
  97.5 gate (best **94.08**), every day-block interval contains zero, and the
  40% concentration ceiling the registration set for itself fails in all four
  cells — **2026-01-30, the session gold fell 9.5%, carries 62% of the best
  cell**. Three diagnostics the registration forced into print each contradict
  the mechanism: the quantity is **strongest at a thirty-second fill delay**,
  only the **narrowing** half carries anything (the widening half — the clearer
  inventory story — is nothing or negative), and days with **more** one-sided
  revisions carry **less** signal. The instrument was tested rather than
  defended (see the self-test item above) and is sound: the effect is **one
  standard error wide**. Out-of-sample and the euro confirmation stay unopened.
  `2026-09-15-quote-asymmetry.md`.
- [x] Does a dealer's quote leave the consensus before the price moves? (the
  owner's instruction of 2026-09-15: stop testing what is published, work the
  data nobody else has) → **the question cannot be asked at bar resolution.**
  The declared direction failed at the 1.90th / 12.75th / 0.95th percentile; its
  mirror survived the momentum and staleness controls that were written to kill
  it (plain momentum 75.76 / 13.69 where the residual read 98.10 / 99.05; a
  deliberately **staler** ECN was *weaker*; correlation with the dealer's own
  last-bar return −0.031) and then reached **no cell's gate** on the traded leg
  once a trade was forbidden from spanning a weekend — best 93.48, and that is
  one half of the window (H1 41.95, H2 96.46). The deciding objection is not a
  percentile: the residual is **0.14 s of tape on an average bar and 6.2 s at a
  fire**, one minute of misalignment is 21x it, and both feeds are stamped to the
  minute — so "inventory" cannot be separated from two vendors' clock latency
  with anything on disk. **No venue-comparison hypothesis should be registered at
  bar resolution again**; it needs tick data with venue timestamps on both legs.
  What the study did buy: an alignment proof that is reusable (52 weeks, 0
  disagreements, both feeds shifting on exactly the two NY DST weeks), 30 days of
  measured spread by hour, and six instrument faults above.
  `2026-09-15-venue-residual.md`.
- [x] Audit every horizon in the repository for fault 12 → **it lives in one
  instrument and that registration is already closed.** `news_drift.py` and
  `friday_cells.py` offset in clock minutes and resolve through a tolerance;
  `fx_window_drift.py` selects by minute-of-day; the engine holds in
  milliseconds throughout and its filters are clock windows. Indicator periods
  are bar counts and are meant to be. One lesser asymmetry named in
  `fx_window_drift.py` (a partial day's drift netted against a full span's trend
  share) moves no verdict. Written up in `2026-09-13-instrument-faults.md`.
- [x] Does silver's move against gold come back? → **not as registered.** All
  three declared conditions passed and neither review could break the code
  (causality proven bitwise; a deliberate look-ahead flips the sign to −21.3 bp)
  — but the windows are counted in bars, so on the 1,257 of 1,706 trades where
  four hours really means four hours the cell reads +0.287 bp at 52.51% won,
  p 0.0803, failing two of three. The registered cell was also the **argmax of
  the win rate** across all 27 exploratory cells, the exact statistic both gates
  are functions of. What is left is real and is not this: an overnight,
  break-crossing gold–silver dislocation, 100th percentile of two nulls, not
  reducible to either leg, decayed to nothing since 2022.
  `2026-09-15-pair-residual.md`.
- [x] Does the pre-NFP hour exist on instruments never read for a news
  question? → **no**. The euro refuses outright (the release adds +0.312 pips
  at t +0.28; the drift is the already-closed European-hours effect). Silver's
  hour falls hard — 29.9% up on 184 releases, sign p 4.9e-08, robust to units,
  trimming and leave-one-year-out — and **fails the percentile gate** at
  6.02 ± 0.04 against 5.0 over 400,000 draws. And it was never independent: in
  a world where silver is only beta times gold plus noise, all three conditions
  pass 79.4% of the time. The loop's one survivor did not replicate.
  `2026-09-15-nfp-cross-asset.md`.
- [x] Is the pre-NFP hour the release or the first Friday of the month? → the
  question cannot be asked of this calendar. Both declared conditions passed
  and neither measured what it was declared for: the falsifier compared one
  treatment cell to another instead of to the control, and **one** of the 32
  "quiet first Fridays" is an ordinary print-free trading day — eleven carry
  Canada's jobs report or US PCE at the same 08:30 minute, four are US market
  holidays, six exist only because a shutdown removed the release. What
  survived: 25 releases on a **later** Friday, away from the turn of the month,
  at the **0.2nd percentile** of a day-of-month-matched control with 20% of
  hours up against 53%. `2026-09-14-nfp-vs-first-friday.md`.
- [x] News trading, the one family in the sarwa.co survey this loop had never
  tested (the owner's request) → gold's hour before the US employment report
  falls at the **3.2nd percentile** of a New-York-clocked Friday null on the
  eight years the question had not been asked of, passing all three numbers
  declared before the window was opened. **The first registration in this loop
  to survive its falsifier** — and the exploratory cell that nominated it does
  not survive its own control (7.8th, not 0.4th; the number is retracted). Not
  a strategy: about $46 a year at 1% risk, and the loop's own news guard
  forbids the hour. `2026-09-14-pre-nfp-drift.md`.
- [x] Volman's three boxes on 5m gold (the owner's request) → the tight-box
  break is a coin flip before costs (38th on direction in R, flat gross on
  3,317 trades) and loses a seventh of its risk a trade after $0.28;
  0.92–0.97 on the last Vantage year. The breakout family's verdict, with
  box-height exits. `2026-09-14-volman-box.md`.
- [x] The euro's European-hours drift on 2018–2026, sign only → 67th on
  direction, excess −0.10 pips a hold (t = −0.16) against −2.84 on 2010–18;
  the clock stopped; the flip year is 2025. `2026-09-14-fx-local-hours-sign.md`.
- [x] The spread print (`search` header showed 0.00 on EURUSD) → fixed at
  `49b5139`.
- [x] The euro falls through European hours (Breedon–Ranaldo) → the sign
  at the 96th–100th of both nulls on ~2,000 holds a row, 6,150 pips in
  eight years, eight of nine years relative to trend; PF 1.07 / 0.98 /
  0.98 at 1.4 pips, and the gate needed −0.7 pips. Closed on the gate.
  `2026-09-14-fx-local-hours.md`.
- [x] Intraday momentum on gold (Gao–Han–Li–Zhou) → the first half hour's
  sign is a coin flip for the last hour (40th), the window loses the spread
  (a coin flip earns 0.62), the day-so-far sign reverses at the 1st on the
  window that produced it. Closed. `2026-09-14-intraday-momentum.md`.
- [x] TSMOM on EURUSD, the adversary's terms → gate and sized null pass on
  2010–18 (PF 1.81, 100th), the row's own sides permuted 85th; one 356-day
  trade is 71% of net, PF 0.62 without the top five. Three assets, one
  shape; the family on intraday bars is closed. `2026-09-14-tsmom-eurusd.md`.
- [x] TSMOM on silver, two eight-year windows → the 20-day row passes
  2010–18 (one of three lookbacks, five trades of 2010–14) and is a coin
  flip on 2018–26; 60d the gold shape on the same rally; 120d inverted then
  nothing. With gold, daily-rebalanced TSMOM on intraday bars is closed on two
  assets. `2026-09-13-tsmom-silver.md`.
- [x] The Friday weekend hold on 2010–2018 Dukascopy → the window the
  adversary named before the run: 400 holds PF 0.983, 92nd on direction,
  2014–2018 negative every year; the 2018–2025 pass was the decade. Closed
  for good; nothing reopens it. `2026-09-13-friday-weekend-hold.md`.
- [x] Close-to-reopen drift on gold → the sign is at the 96th–100th of the
  null on both windows and in every year, worth $0.10–0.20 an ounce a
  session: net zero before 2023 (the spread), 1–4% a year at 1% risk since;
  fails the registered gate as a unit and is not a trade. The Friday leg
  passes seven years and cannot be confirmed on 67 Fridays. No paper run
  (risk BLOCK). `2026-09-13-close-reopen-drift.md`.
- [x] Technical baselines (ema-cross, rsi-reversion, donchian, bb-fade) on GC,
  Binance, Vantage → inside the noise. `docs/decisions/2026-09-12-technical-baselines.md`,
  `2026-09-12-vantage-bars-baselines.md`.
- [x] Session and regime gates on the baselines → nothing.
  `2026-09-12-gold-intraday-batch-1.md`.
- [x] ICT sweep → MSS → FVG (three presets, kill zones) → survives three
  months, fails four years. `2026-09-13-ict-sweep-mss-fvg.md`.
- [x] Logistic model on the 24 features → does not generalise.
  `2026-09-12-logistic-does-not-generalise.md`.
- [x] Opening-range breakout, New York → gate pass in-sample against a
  mis-matched null, inside the corrected one, fails four years out of sample,
  direction a coin flip. `2026-09-13-orb-ny.md`.
- [x] London-range breakout → passes everything in-sample on 2025–26 (gated
  null, direction null), fails the disjoint 2022–25 window; the window is a
  volatility regime. `2026-09-13-london-range.md`.
- [x] Previous-day high/low (fade and break) → 1st–6th percentile of the gated
  null on the long window; wick-tight stops lose to the spread.
  `2026-09-13-pdhl.md`.
- [x] Volatility-conditional breakout → high- and low-volatility halves of
  2022–25 indistinguishable, both inside; the breakout family is closed.
  `2026-09-13-volcond-breakout.md`.
- [x] BTC US equity-open range break → best row 84th percentile, direction
  73rd, on two years of Binance 5m; not opened at Vantage cost.
  `2026-09-13-btc-us-open.md`.
- [x] BTC US-hours drift (hold 09:30–16:00) → PF 0.78 over 435 sessions,
  complement 0.95; no drift by hour in 2024–26. `2026-09-13-btc-us-hours.md`.
- [x] London fix drift (into / out of the PM fix, AM contrast) → 74th–84th
  percentile of random holds, PF < 1 on every row, 2022–25.
  `2026-09-13-london-fix.md`.
- [x] VWAP fade (BTC, real volume) → PF 0.95 on 5,828 trades, direction null
  13th: a stretch continues rather than reverts. `2026-09-13-vwap-fade.md`.
- [x] The closed mechanisms on 15-minute bars (gold ×4, BTC ×2) → every row
  fails; the timeframe is not the variable. `2026-09-13-m15-check.md`.
- [x] Doji reversal (gold 15m, BTC 15m; three gold windows) → inside the
  count-matched null on 2022–25, loses on 2018–22 and 2025–26; BTC fails.
  `2026-09-13-doji.md`.
- [x] Weekend gap fade (gold 5m, 2018–25) → PF 0.42 on 207 gaps, 1st
  percentile; large gaps 0.77; direction 33rd/66th. `2026-09-13-gap-fade.md`.
- [x] Volume thrust (BTC Binance volume, gold Vantage tick volume) → PF 0.96
  / 0.93, 50th / 78th, direction 30th / 78th; the best walk-forward row fails
  the gate. `2026-09-13-volume-thrust.md`.
- [x] Time-series momentum, sized on daily ranges (gold 15m, 2018–25) → 60d
  PF 2.03 on 75 trades, 98th of the sized random-hold null, 86th of its own
  trades with sides permuted; three trades are the profit; the confirmation
  window cannot produce 30 trades. `2026-09-13-tsmom-2.md`.
- [x] Trend pullback (EMA20 in an EMA200 trend, swing stop; gold 2018–25,
  BTC 2024–26) → PF 0.38 / 0.51, 0th percentile on both; trend-following on
  intraday bars closed as a family. `2026-09-13-trend-pullback.md`.
- [x] The recent-year screen (owner's criterion: the last twelve months at
  Vantage; 17 mechanisms × 4 sessions × 2 timeframes + 24 hour-holds) → one
  row of ~170 passes, noise by the registration's own count; five indicator
  mechanisms added to the registry. `2026-09-13-recent-year-screen.md`.
