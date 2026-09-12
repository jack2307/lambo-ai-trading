# One night of the loop: five hypotheses, zero survivors, one finding about the sample

**Date:** 2026-09-13 (01:15 → 04:00, unattended)
**Question:** Is there, in this project's registry, an intraday method on gold
whose edge at Vantage's cost generalises beyond the window it was found on?
**Outcome:** No. Five pre-registered hypotheses ran end to end under the
`/research` discipline — ORB-NY, London-range, previous-day levels, the
volatility-conditional breakout, plus the ICT chain from the evening — and
none survives a multi-year window. The night's real finding is about the
sample: the 2025–26 Vantage window (annualised volatility 24%) passes almost
anything that trades the New York morning, and 2022–25 (13%) passes nothing.

## The ledger

| hypothesis | in-sample (window) | out-of-sample (window) | direction null | record |
|---|---|---|---|---|
| ICT sweep→MSS→FVG | B preset survives, 100th pct (Vantage M1, 3 mo) | PF 0.75–0.77 (Dukascopy M1, 4 y) | 79th in, — | `2026-09-13-ict-sweep-mss-fvg.md` |
| ORB, New York 60m | survives, 100th → 92nd once the null was window-gated (Vantage 5m, 1.4 y) | PF 0.95 (Dukascopy 5m, 4 y, 28% overlap) | 34th / 54th | `2026-09-13-orb-ny.md` |
| London range | survives 98th, direction 96th (Vantage 5m) | PF 0.95 on 525 trades (Dukascopy 5m, disjoint 2022–25); replay 0.957 | 96th / 46th | `2026-09-13-london-range.md` |
| Previous-day H/L, fade and break | 1st–6th pct on the long window (primary) | not opened | 47th–82nd | `2026-09-13-pdhl.md` |
| Volatility-conditional break | 70th–84th on the long window; contrast rows identical | not opened | 32nd–91st | `2026-09-13-volcond-breakout.md` |

Earlier the same day: the four indicator baselines and their session and
regime gates (`2026-09-12-gold-intraday-batch-1.md`), inside the noise on
4.2 years of Vantage 15m.

## What was learned about the method, not the market

Each pass changed the loop, and the changes are in the skill and the code:

1. **The null must trade the same hours as the method** (ORB pass: 100th
   percentiles that were 72nd–96th once gated). `Filter::parse`, batch
   files, `null-dir --filters`.
2. **An out-of-sample feed that overlaps the in-sample one is only out of
   sample where it does not overlap** (ORB pass: 28% of the "out-of-sample"
   days were the in-sample price series). `search --from/--to`.
3. **The long window is the primary test** (London pass). Three unrelated
   mechanisms survived one year and failed three; the year was the variable.
4. **An amendment is its own commit** (London pass: a zero-trade preset was
   fixed and committed with the result).
5. **A direction null on the method's defaults says nothing about the
   preset tested** (London pass: two receipts were byte-identical).
   `null-dir --params`.
6. **A walk-forward on the out-of-sample window re-selects on it; replay the
   registered parameters too** (`--fixed`, `out-of-sample-fixed.txt`).
7. Flat-filled bars in a free feed (334,406 of them) depress every ATR;
   drop them at conversion.
8. Config keys nobody reads are intentions; four were deleted.

None of these was known at 01:15. All of them would have produced a false
survivor if left alone, and one did for about ninety minutes.

## What each role said, across the night

- **adversary** (three passes): broke every positive with the same two
  instruments — the gated null and the direction null — and never had to
  argue about a number. *"The side of the trade only carried information
  in the trending window."*
- **data-integrity** (two passes): found the overlap, the flat bars, the
  clock held; *"the ORB fails on a four-year sample tilted in its favour."*
- **risk** (one pass): nothing was ever proposed; found the unread limits.
- **historian** (one pass): predicted the London result from the ORB record
  and set the condition under which it was still worth running.

## What is left with a reason

- **Options flow at levels** — the founding thesis, untested because the
  tape is days long; both collectors are running and the tape grows at
  ~3.5k gold prints and ~5k BTC prints a day. Months, not hours.
- **BTC at the US open** — a different market with its own reason (the
  equity open moves BTC); registered next, on Binance 5m (two years) as
  primary and Vantage BTCUSD 5m as confirmation.
- **Session VWAP reversion** — needs a volume feed for the long window;
  Dukascopy's minutes have none.

Everything else in the backlog that was a range, a level or a gate on gold is
closed, and the skill now says so.

## What this does not say

- It does not say intraday gold cannot be traded; it says nothing in this
  registry does it at $0.28/oz over 2022–25, and the 2025–26 year should not
  be believed alone by anyone, about anything.
- It does not say the loop is finished tuning itself: eight process changes
  in one night means the ninth is likely, and the next reviewer should look
  for it.
