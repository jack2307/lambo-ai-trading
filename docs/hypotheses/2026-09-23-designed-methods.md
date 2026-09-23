# 2026-09-23-designed-methods: do methods designed from this data's own structure survive a year that was physically withheld

**Registered:** (commit time is authoritative) — before any agent is briefed, before
any method is designed, and before any sealed-window number exists
**Status:** decided -> docs/decisions/2026-09-24-designed-methods.md (0 of the 8 available hypotheses were proposed: four agents, four angles, each measured its own mechanism on 3-15 years and refused it. Three of the four found the effect they were sent for and all four fell short on the same quantity -- gross profit factor 1.01-1.22 against the 1.21-1.32 that net 1.20 needs. Explanation 1 is answered: the old mechanisms were not the problem. THE SEAL WAS NEVER SPENT -- no method reached the withheld year, so XAUUSD 15m 2025-09-23 -> 2026-09-17 remains a genuinely untouched hold-out. Eight defects published, the worst being that neither null controls for the instrument's own drift, so a long-biased gold method clears both on its side ratio alone)

## Where this came from

The owner's instruction, after the three-month search closed with 161 cells and no
survivors: *"Bạn giao cho Agent tự nghiên cứu phương pháp chứ đừng dùng mấy cái cũ
nữa"* — have agents research methods themselves, stop using the old ones.

The registry's 21 mechanisms are textbook: moving-average crossings, RSI, MACD,
Donchian and Keltner channels, VWAP fades, opening ranges, ICT sweeps. Every one has
been published for decades and traded by everybody. Widening the search to methods
designed from this data's measured structure is a real widening, not a gesture.

**What it can and cannot buy, stated before the work.** Two explanations compete for
why 25 closed registrations and 161 cells have produced nothing:

1. the mechanisms tried were too well known to still pay, or
2. a 15-minute single-instrument edge net of a 0.28 spread is thin to nonexistent at
   this cost level.

This program tests **explanation 1 only.** If designed methods also fail, that is
evidence for explanation 2, which no amount of further designing will fix. Saying so
now prevents this program being re-run forever on the belief that the next design
will be the one.

## The thing this program does differently: the hold-out is physically absent

Every previous registration on this desk tested out of sample *after* the fact, on
data the searching process could have read. That is a discipline, not a guarantee.

Here the hold-out is removed from the data the designers are given.
`crates/fd-store/src/bin/seal-store.rs` wrote `data-sealed/`, which holds:

- **bars**, every instrument truncated at **2025-09-23 00:00Z, exclusive**. XAUUSD
  15m ends 2025-09-22 23:45 with 77,278 bars from 2022-06-16. Dukascopy gold, silver
  and EUR end the same day with 362,803 / 358,065 / 381,199 fifteen-minute bars from
  2010-06-01, and roughly 5.4M / 5.0M / 5.6M one-minute bars.
- **the news calendar, whole and untruncated.** An economic calendar is published
  ahead of time and the live desk holds next year's events today, so a method that
  knows when news is scheduled knows nothing the desk does not already know.
- **nothing else.** No `paper/`, no `live/`, no `ticks/`, no `equity/`, no
  `spreads/` — the desk's own trade history through today would reveal the hold-out
  as surely as the bars would.

The sealer re-reads all 21 files after writing and fails rather than reports success
if one bar at or after the cutoff survived. It verified clean: 20,837,886 bars kept,
1,598,723 sealed away.

**The withheld year is XAUUSD 15m, 2025-09-23 → 2026-09-17: 23,308 bars.** It is the
recent Vantage year the owner made the primary criterion on 2026-09-13, so it is both
the hold-out and the window that decides.

**What the seal does not do, stated plainly.** The design agents run as the same
operating-system user as the session that briefs them, so the full store at
`E:/rust/flowdesk/data` is readable by them if they type that path. The seal stops
accident and stops the lazy shortcut; it cannot stop a determined peek. Three things
carry the rest of the weight, and none of them is an assurance of good faith:

- every run's receipt prints its data root and window bounds, and every receipt is
  read;
- the sealed-window test is run by the lead, not by the designer, on frozen
  parameters, once per method;
- **a method must come with a stated reason it should work.** A design justified only
  by its own fit is refused whatever its numbers say. A method reverse-engineered
  from the hold-out rarely survives having to explain itself in terms of a mechanism
  that existed before the fit.

## The options flow cannot be used, and that is worth recording

The desk's most distinctive data source is the OTL gold options tape, and it holds
**11 day-partitions, every one of them inside the hold-out window.** There is no
options history before 2025-09-23 at all. No options-based method can be designed on
a design window and tested on a withheld one, because there is no design window for
it. `data-sealed/gold/tape/` is therefore empty, and options-flow design is out of
scope for this program by absence of data, not by choice.

## Claim

At least one method designed from the structure of the design window, by an agent
that has not seen the withheld year, clears all four legs on that withheld year at
frozen parameters.

## Falsifier

Per method, on **XAUUSD 15m, 2025-09-23 → 2026-09-17**, guards on, the configured
spread of **0.28**, parameters **frozen before the run**, no folds, no selection, no
re-fit, run once:

- **at least 30 trades** — the standing floor; fewer is unscoreable, not a pass;
- **expectancy > 0**;
- **profit factor ≥ 1.2** — the registry's standing `min_profit_factor`;
- **≥ 95th percentile of a count-matched null**, 200 seeds, the 2026-09-23 repair in
  force, with the achieved count match reported rather than assumed;
- **≥ 95th of the direction null**, likewise.

All of them, or the method is refuted. A method that clears them is a **candidate**
with one measurement, not a result, and it goes to a paper book and a fresh
registration — never to money on this evidence.

## Multiplicity, fixed before any method exists

**Four agents, at most two methods each: eight hypotheses, and eight is the cap.**
At the 95th percentile, **0.4 of 8 pass by luck**, so:

- one pass is at the edge of what noise produces and is treated as a candidate, not a
  finding;
- two passes is interesting;
- three or more would contradict the whole record and should be disbelieved until
  re-run on Dukascopy gold over the same dates — a different vendor for the same
  metal — and on silver.

**A null submission is a valid and preferred outcome.** An agent that cannot design a
method it believes in reports that and proposes nothing. Padding the slate to two
costs multiplicity for nothing, and an agent that submits filler has made the program
worse. This is stated in every brief.

Each agent must also declare **how many variants it tried on the design window**.
That is its own private multiplicity, and its design-window number cannot be read
without it.

## The four angles, assigned so the agents do not converge

1. **Attack the cost term, not the signal term.** Every failure in the record sits at
   a profit factor near 1.0 — gross edges that costs eat. `cost/R = spread/stop`, so
   an entry chosen to make the stop large relative to the spread improves the bar it
   has to clear, measurably and without needing a better signal.
2. **Conditional structure: when not to trade.** Find measured conditions under which
   price behaviour differs, and act only in the thinnest slice where the difference is
   measurable. The record says real clocks existed at the 95th on a first window and
   vanished on a second, so this angle must state how its design differs from that
   one rather than rediscovering it.
3. **Lower frequency.** Everything on this desk is 15-minute intraday. A multi-day
   holding period has a far larger stop for the same spread, which is the same
   arithmetic as angle 1 approached from the other side. 15 years of Dukascopy gold
   are available for design.
4. **Two series, not one.** Every mechanism in the registry reads a single series.
   Gold, silver and EUR are on disk from 2010 from one vendor. A relationship between
   two instruments is absent from the registry entirely.

## Standing constraints, unchanged

- Methods implement the existing `Strategy` trait (`crates/fd-strategy/src/registry.rs`)
  so they run through the audited guarded backtest path with the same guards, the same
  spread and the same null machinery. A method scored by its own private harness is
  not comparable to anything in the record.
- **Causality test required per method**: the decision on bar *i* may read no bar
  after *i*, tested the way `fd-indicators` tests it — a truncated series must be a
  prefix of the full one.
- No threshold moves. The gate stays 30 / 1.2 / 0.05R and both nulls stay at the 95th.
- `null` is not `0` is not `[]`. A method that took no trades has no return, not a
  return of zero, and sorts below every method that traded.
- The six banned words stay banned: score, composite, confluence, bias, rank,
  strength. They name arithmetic without justifying it.
- Units carried on every figure.

## What is pre-committed about the reporting

1. **Every proposed method's sealed-window number is published**, including the ones
   that fail and including a method whose design-window number was spectacular.
2. **Nothing from this program goes on the funded account**, whatever it shows. The
   funded books are `ai-xau-ds-ctx` and `xau-macd-asia` on account 33708517 and this
   program does not touch them.
3. **The design-window number is reported beside the sealed-window number for every
   method**, so the gap between them is visible rather than inferable.
4. **If an agent's receipts show it read the full store**, that is recorded and its
   methods are reported as contaminated rather than quietly re-run.
5. **If all eight fail**, the record says that explanation 2 above has gained the
   evidence, and the program closes rather than spawning a fifth angle.

## What each outcome means

- **All eight fail.** The likeliest result. It moves the weight onto the cost
  explanation and closes "the old mechanisms were the problem" as a hypothesis.
- **One clears.** A candidate with one measurement, at the edge of what eight tests
  produce by luck. Paper book, new registration, no money.
- **Two clear.** Interesting, and worth the second-vendor and silver re-runs before
  anything else.
- **Something clears with fewer than 30 trades.** Unscoreable. It does not become a
  pass by being interesting.
