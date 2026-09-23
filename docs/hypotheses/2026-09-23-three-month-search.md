# 2026-09-23-three-month-search: what the best three-month rule does on the nine months it did not see

**Registered:** (commit time is authoritative) — before any run, and before
any in-sample winner is known
**Status:** decided -> docs/decisions/2026-09-23-three-month-search.md (161 cells across 21 mechanisms, 25 out-of-sample tests, 0 survivors; the claim is refuted -- and this registration's own prediction that the search would clear 50% in sample was falsified too, the best of 161 cells reaching +20.22%, so the named "nothing clears 50% even in sample" branch is the one that happened; the cited "+0.011R best ever" ceiling was also wrong, an unfiltered macd-cross cell measuring +0.049R over 862 trades)

## What the owner asked

"Dựa vào dữ liệu 3 tháng gần đây tìm ra cho tôi phương pháp trade tốt nhất
với lãi suất ít nhất 50%" — search the last three months and find the best
method, returning at least 50%.

That search will succeed. Over ~6,000 fifteen-minute bars, sweeping the
registry's mechanisms and their parameter grids produces hundreds of cells;
the best of hundreds on a window that short clears 50% by arithmetic, not by
edge. So the search is run **and the thing that decides whether it means
anything is declared here first**.

## The arithmetic the target implies

50% over 63 trading days is **0.646% a day compounded**. At 1% risk per
trade and two trades a day that is an expectancy of **+0.322R per trade**.

The best expectancy this desk has ever measured is **+0.011R**
(`macd-cross`, 923 trades, closed). Most are negative. The target therefore
asks for **about 29× the best number in the record**, on a window a fifth
as long as the one the owner made primary on 2026-09-13.

This is written down before the search so that a cell which does clear 50%
is read against it.

## Claim

The best-performing rule on `xauusd:15m` over the last three months
(2026-06-23 → 2026-09-23) also returns a positive expectancy on the nine
months before it (2025-09-23 → 2026-06-22), the data it was not selected on.

## Falsifier, declared before the winner is known

The winner is whichever cell has the highest walk-forward return on the
three-month window, chosen by the existing `select_by` rule, with guards on
and the configured spread of 0.28.

It is then run **unchanged — same strategy, same parameters, no re-fit** on
the nine months. It survives only if:

- **expectancy on the nine months > 0**, and
- **profit factor ≥ 1.2** there, the registry's standing gate, and
- **≥ 95th percentile of its matched null** on the nine months, that null
  count-matched by the repair landed earlier today.

Anything less and the claim is refuted for that cell.

**The top five by three-month return are all carried forward, not just the
first.** A single winner that fails is one draw; five failing together is
the finding.

## What is pre-committed about the reporting

1. **The in-sample number is published whatever it is**, including if it is
   spectacular. A 200% three-month return is reported as a 200% three-month
   return, beside what the same rule did on the nine months.
2. **No cell is dropped for being embarrassing in either direction.**
3. **Nothing from this search goes on the funded account**, whatever it
   shows. A rule selected on three months and confirmed on nine has been
   measured twice on one year; the desk's own standing criterion is the
   recent year, and a fresh registration would be needed to promote
   anything.
4. **If a cell does survive all three legs on the nine months**, that is
   genuinely interesting and gets its own registration and a paper book —
   not money.

## Multiplicity

The sweep is hundreds of cells on one short window. At the 95th percentile
alone, dozens clear by luck before the return filter is applied. The record
will state how many cells were searched, because "the best of N" means
nothing without N.

## What each outcome means

- **The winners collapse on the nine months.** The expected result. It
  answers the owner's question with his own data: a three-month search buys
  a number, not an edge.
- **One survives.** Worth a paper book and a new registration. Not money.
- **Several survive.** Would contradict forty closed registrations on this
  desk and should be disbelieved until re-run on a second instrument.
- **Nothing clears 50% even in sample.** Also possible, and would say the
  window is too short to produce even a lucky number at this risk level.
