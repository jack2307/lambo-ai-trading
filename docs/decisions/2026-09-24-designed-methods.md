# 2026-09-24-designed-methods: four agents designed, four refused, and the withheld year was never spent

**Registration:** `docs/hypotheses/2026-09-23-designed-methods.md`, commit `253e94a`,
written and committed before any agent was briefed.
**Status:** decided. **0 of 8 available hypotheses were proposed.** No method reached
the withheld year, so the seal is intact and unspent. The programme closes, as
pre-commitment 5 requires, and the weight moves to explanation 2.

## What the owner asked

*"Bạn giao cho Agent tự nghiên cứu phương pháp chứ đừng dùng mấy cái cũ nữa"* — have
agents research methods themselves rather than sweep the registry's 21 textbook
mechanisms again, after the three-month search closed with 161 cells and no
survivors.

## The short answer

Four agents, four disjoint angles, **nobody proposed a method.** Not one of the eight
hypotheses the registration made available was spent.

That is not four agents giving up. Three of the four **found the effect they were
sent to look for**, measured it on three to fifteen years and two vendors, and then
refused it on its own design window. The fourth measured the angle away entirely. In
every case the reason was the same and it was arithmetic, not luck:

| angle | the effect, as measured | why it was refused |
|---|---|---|
| 1 · attack the cost term | a structural stop beats an ATR stop by **+0.14 to +0.20 profit factor**, on two vendors, over 3 years and over 15 | gross profit factor on offer from any dull signal at any horizon is **1.01–1.16**; net 1.20 needs **1.21–1.32**. The columns never meet |
| 2 · when not to trade | against a correct null, **no slice of this market is directional** — 0th–1st percentile in every hour, every volatility band, four instruments | the one first-moment consequence is worth **+0.017 ATR** net over 15 years and **inverts sign above ATR 4 points** — the band the withheld year sits in |
| 3 · lower frequency | the cost bar really does fall **21×**, from 14.0% of R at an intraday stop to **0.67%**; and `quiet-swing` reached **PF 1.218, +0.0434 R, 136 trades** — above the gate | 2025's nine months are 26% of trades and carry **more than 100%** of total R; 2022–2024 is **−0.65 R over 101 trades**. The withheld year begins the day after those nine months end |
| 4 · two series | fading a gold move silver did not share earns **+0.4 to +1.3 gold points/trade gross**, against **+0.00 to +0.03** for the single-series control over 30,000 trades, sign holding in three disjoint five-year eras | bracketed it is PF **1.072**; with the bracket removed the gross profit factor is **0.999**. The whole positive mean is a handful of large retracements that a stop inside four hours of gold noise converts into losses |

## The seal was built, verified, and never used

The programme's one methodological innovation was removing the hold-out from the data
the designers were given, rather than asking them to test out of sample afterwards.
`seal-store` wrote `data-sealed/` ending 2025-09-23, re-read all 21 files and verified
that no bar at or after the cutoff survived: 20,837,886 bars kept, 1,598,723 sealed
away. Every receipt from all four agents prints that root.

**Nothing reached the test, so the withheld year was never read.** XAUUSD 15m
2025-09-23 → 2026-09-17, 23,308 bars, remains a genuinely untouched hold-out. That is
an asset the desk did not have yesterday and should not be spent casually: it is worth
exactly one honest programme.

The registration's pre-commitment 5 says that if all eight fail, the programme closes.
What happened is not identical — **nothing was submitted, so nothing failed a withheld
test** — and the difference is worth stating rather than glossing:

- It is **more informative** than eight withheld-year failures would have been. Eight
  draws at the 95th percentile have an expected yield of 0.4, so 0 of 8 would have
  been indistinguishable from the null. What actually happened is four mechanisms
  measured on long windows, three of them real, all four falling short on the same
  quantity by roughly the same factor. The convergence is the finding, not the count.
- It is **weaker in one respect that must be said**: four agents are four instances of
  one model, briefed by one lead, sharing priors and reading the same record. Four
  correlated judgements are not four independent tests, and the confidence this record
  carries should be read with that discount.

## The quantity everything converged on

Angle 3 put it most exactly, from the cost side:

> There was about 4% of R to save, and the edge underneath is the same 4% of R.
> **Lowering the cost bar stops an edge being destroyed; it does not create one.**

Angle 1 put the same thing from the horizon side. Because `[trading] max_hold_ms` is
four hours, the gross profit factor needed for net 1.20 falls only from **1.32 at four
hours to 1.21 at a week** — and the gross on offer measured 1.01–1.16 everywhere, with
the single best figure anywhere at **1.164 against the 1.221 it needed**.

Angle 4 reached it from the other end: its rule sits at PF 1.072 with a null median of
0.872, so the second series genuinely carries direction the first does not, and **the
entire remaining gap is the cost term**.

Angle 3 also closed the underlying question by measurement rather than by another
failed backtest: **Lo–MacKinlay variance ratios on 362,802 fifteen-minute returns and
3,957 session returns sit within 1–7% of 1.0 at every horizon from 30 minutes to 60
sessions.** The only deviations past |z|>2 are at 30 and 60 minutes and amount to 1–2%
of variance — uncollectable against 0.28 at any intraday stop.

So: **explanation 1 is answered. The old mechanisms were not the problem.** The
registration said this programme tests only that, and that if designed methods also
failed the weight moves to explanation 2 — a 15-minute single-instrument edge net of a
0.28 spread is thin to nonexistent at this cost level — which no further designing
fixes. It has moved.

## The candidate that cleared the gate and was refused anyway

`quiet-swing` deserves its own section, because it is the closest thing this desk has
produced to a pass and it was refused by its designer.

On the recent Vantage window it measured **PF 1.218 over 136 trades at +0.0434 R** —
above the standing 1.2 gate — holding genuinely multi-day (geometric-mean 2.2 days,
longest 5–7). Its mechanism is not fitted: volatility clustering means "the tape has
been quiet for two weeks" is a statement about the next two weeks, and signal
extraction says the same observed move implies more drift when noise is lower. On 3,958
Dukascopy sessions that prediction lands where it should: **+0.1649 ATR20 over five
sessions in the quiet third (n=1,323, t=+3.48) against −0.0027 in the top third**, with
a 53% long share so it is not drift, and three five-year blocks at +0.136 / +0.177 /
+0.137.

Four reasons it was refused, and the first exists only because the lead asked every
agent to check for era concentration after a sibling's 15-year figure turned out to be
a three-year figure:

1. **Per year on the broker's bars: 2022 −2.82 R (PF 0.238), 2023 +0.81 R, 2024
   +1.36 R, 2025 nine months +6.55 R (PF 2.196).** 2025 is 26% of the trades and
   carries more than 100% of the +5.90 R total; 2022–2024 is −0.65 R over 101 trades.
   **The withheld year starts the day after that stub ends.** On 15 years, 2010–2012 is
   14% of trades carrying 48% of R, and the twelve full years between yield +0.0128 R
   per trade with six of twelve under PF 1.0.
2. It fails a falsifier leg on its own design window, and a **different** leg on each
   window — profit factor on the 15 years, both nulls on the recent one.
3. It does not transfer to silver: **PF 1.012 over 783 trades and 15 years**, same
   vendor, same parameters.
4. 70 variants behind it, and the survivor is at the noise floor.

Its frozen parameters are committed and reproducible
(`docs/research/designs/2026-09-23-designed-3-frozen.toml`, header `proposed = false`)
so a later programme can spend a slot on it deliberately. **This one did not.** A
method whose only good window is the one immediately before the hold-out is the single
most likely thing to pass a hold-out for the wrong reason.

## Defects found, and the one that matters most

Eight, all published rather than smoothed over. Three are repairs to the machinery that
decides what this desk believes.

**1. Neither null controls for the instrument's unconditional drift — and this is the
serious one.** Both the matched null and the direction null take random or flipped
sides, so both carry roughly zero drift exposure. Gold's unconditional drift is
**+0.3946 ATR20 per five sessions (t=+6.74)** on the recent window, and gold went from
1,200 to 3,700 over the data on file. **So a long-biased multi-day gold method clears
both nulls on its side ratio alone.** Angle 3 measured exactly such a method — long-only
weekly hold, **+0.179 ATR8 per week, t=+3.26** — noted that it *would clear the gate*,
and refused it because it is buy-and-hold wearing weekend flats. That is why its
submitted design is two-sided and why its side split and per-leg P&L print with every
result. Nothing in the existing machinery would have caught this.

**2. The matched null is count-matched but not cost-matched.** `control_for` overrides
only `seed` and `entryRate`, so the control's stop stays at 1.5 ATR whatever the method
uses; a method that merely widens its stop clears the 95th percentile without
predicting anything. Measured: PF **0.973 — losing money — at the 98th percentile**,
count match 0.95 and inside the band; the same signal with an ATR stop at the 2nd.
Registered for repair with its direction of bias stated in advance
(`docs/hypotheses/2026-09-24-cost-matched-null.md`). Scope is bounded: the registry runs
1.0–3.0 ATR, so at most twofold, and **both funded books sit at 1.5 and are
cost-matched by coincidence** — the 76th and 14th percentiles reported to the owner on
2026-09-23 do not move on this axis.

**3. The drift null is hold-matched but never count-matched.**
`run_hypothesis_fixed_guarded` has `let rate = (!drift).then(|| matched_rate(...))`, so
on the drift branch `RandomHold` keeps `entryRate = 1.0` and re-enters on every flat
bar — 3.1–3.7× the method's trades. `count_matched()` honestly reports out of band, but
the printed percentile reads **100**, and calibrating the rate to a count match of 1.03
moves it to **82**. An 18-point error in the number a reader takes away. This is the
third separate count-matching defect found in two days, after the walk-forward repair
of 2026-09-23 and the hold-null ratio measured the same day.

**4. `[trading] max_hold_ms` is four hours and `trading_rules_for` applies it to every
market** with no override path (`trading_rules.rs:49` asserts BTC equals gold). **Every
`Exits::Engine` method in this entire record has been measured on holds of at most 16
bars, whatever its logic intended.**

**5. `Exits::Engine` cannot express "a stop and no target."** `Intent::Enter { target:
None }` means "derive one from `reward_risk`", not "none". Only `Exits::Strategy`
escapes, and that surrenders the engine's stop and clock too.

**6. Five binaries printed the news calendar's provenance from a hardcoded literal**
while loading it from the `--data` root, so every sealed-store receipt named a path it
had not read, and named it as the full store — the one line in the receipt that prints
a data path, wrong, in a programme whose entire audit rests on receipts naming their
store. Fixed; the resolved path now lives in a `OnceLock` because the two deeper call
sites have no `--data` in scope.

**7. `search` printed no data root at all.** Added independently by two agents on the
same day, one naming the market and one the root; both kept.

**8. A data trap, and an agent correcting another agent on it.** Angle 4 found that the
longest run of consecutive Dukascopy 15m bars is 476 in 2010–2012 and **exactly 92 from
2013 on**, so a contiguity-gated scan of ≥96 bars is satisfiable only in 2010–2012 — its
own scan had silently become a three-year measurement wearing a fifteen-year count, and
correcting it flipped the sign. Angle 3 then narrowed that: the 44–46 median came from
the gold∩silver **join**, which drops bars only one feed printed; on XAUDUKA alone the
median run of 92 **is a whole session**, sessions per year stay 257–261, and no session
interior is gapped. The 2013 change is the one-hour daily break. Both statements are in
the record; the second is the more precise one.

## Multiplicity, declared

Eight hypotheses were available and **none was spent**. What was spent is each agent's
own private search on its design window, which it declared as instructed:

| angle | variants tried | reached the engine | frozen |
|---|---:|---:|---:|
| 1 | 116 exploratory cells + 8 declared configurations | 8 | 0 |
| 2 | of the order of 1,000 cells inspected, 7 candidate specifications | 1 carried to a full test | 0 |
| 3 | 70 (47 structure screens + 23 trading configurations) | 1 | 0 |
| 4 | about 700 looks, heavily nested | 5 audited runs | 0 |

Angle 1 also voided 16 of its own 48 horizon cells and named them rather than deleting
them: it had charged silver the gold spread of 0.28 instead of its configured 0.021.
Angle 2 caught its own halt filter dropping 100% of bars, from a `dropped` column it had
put there for that purpose. Both are recorded in their notes.

## What was left in the tree, and what it is not

Three strategies are registered — `far-stop-break`, `companion-unconfirmed`,
`quiet-swing` — plus a new `cmp` companion indicator and a `--companion=<SYMBOL>` flag
that let a method read a second instrument for the first time in this codebase. **None
of them is a candidate.** All three are registered at parameters their own design
window refuses, and `builtin.rs` says so where they are registered. They exist so the
next agent starts from working plumbing rather than rebuilding it.

The companion plumbing followed the precedent `fd_strategy::news` already set — a
process-wide `OnceLock` installed by the binary that owns the data directory, plus one
ordinary `IndicatorDef` — rather than adding a `BarContext` field, which is constructed
at 28 sites across three crates of which 27 would not use it. Its causality test checks
**both** directions: truncating the companion at the same wall-clock instant must change
no value at or before it, which is the leak a two-series method adds.

## Operational

`E:` filled from 301 GB to **640 KB free** mid-programme, because `CLAUDE.md` claimed
agent worktrees share `CARGO_TARGET_DIR` "so three of them cannot fill the disk" and
**nothing sets that variable** — the note meant to prevent exactly this was recording an
intention as a fact. Four worktrees built 12–32 GB targets each. Corrected in
`CLAUDE.md`, together with the diagnostic that cost one agent a whole test run: **cargo
failing on a full disk reports `rustc-LLVM ERROR: No space left on device` with no
compile error and no failing assertion**, so a build that dies with LLVM errors and no
test output is a disk check, not a code review. Also recorded there: a binary in another
tree's target can answer your command, which is how one agent got "unknown strategy" for
a strategy that existed only on its own branch.

`cargo test --workspace` on the fully merged tree: **62 suites, 649 tests, 0 failures.**

## What this record does not claim

- That gold cannot be traded profitably. It says this desk has not found a 15-minute
  single-instrument edge that survives 0.28 of spread, and now has four measured
  reasons why rather than only a count of failures.
- That the withheld year would have refuted these methods. It was never read. Nothing
  here is evidence about it.
- That any funded position should change. `ai-xau-ds-ctx` and `xau-macd-asia` on
  account 33708517 are untouched by this programme, and whether `xau-macd-asia` keeps
  running after failing its gate on the most recent three months
  (`docs/decisions/2026-09-23-three-month-search.md`) is a standing decision for the
  owner, twice put to him and not yet answered. It is not taken here.

## Where a next programme should start, if there is one

Not a fifth angle — the registration forbids that and the evidence does not support it.
The three repairs above are worth more than another search: a null that controls for
the instrument's own drift would change what this desk is able to believe, and the
absence of one means every long-biased result in the record is unread. That, and the
`max_hold_ms` cap, which has silently bounded every engine-exit measurement ever taken
here.
