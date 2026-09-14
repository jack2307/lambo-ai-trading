# Intraday momentum on gold: the first half hour of New York carries no direction for the last hour; the last hour itself loses the spread

**Date:** 2026-09-14 (small hours)
**Question:** Does the sign of gold's 08:30–09:00 New York return predict the
sign of its 15:30–16:45 return (Gao, Han, Li and Zhou 2018, on SPY), on
Dukascopy 2010-06 → 2018-06, and again on 2018-06 → 2026-05 — with the
day-so-far sign as the paper's second spelling and a mid-day half hour as
the contrast that must fail? Registered at `bea54fa` after the method was
implemented and tested (`bfc2475`), before any run.
`docs/hypotheses/2026-09-14-intraday-momentum.md`.
**Outcome:** **Closed on the primary; the confirmation was not opened.**
The first-half-hour row is PF 0.606 on 2,022 sessions, 35th percentile of
the sized random-hold null and 40th of its own sides permuted — the sign
carries nothing, and the window itself loses the spread (a coin flip on it
earns 0.62). The day-so-far row is at the
1st percentile of its own sides: on this window the morning's direction
predicted the *opposite* of the last hour. That is an observation made
after the fact on the window that produced it; it is not this claim, and
if it is ever tested it is tested on bars no run has read. No paper run.

## What was measured

- Method: `intraday-momentum`, new (`crates/fd-strategy/src/intraday_momentum.rs`,
  six tests including no-lookahead and a hole in the predictor window).
  Predictor = close of the last bar before `firstTo` over the open of the
  first bar at or after `firstFrom`, same New York day, both at or before
  the signal bar; side = its sign; entry signalled on the first bar ≥ 15:15
  and filled at the 15:30 open; exit signalled on the first bar ≥ 16:30 and
  filled at the 16:45 open; 1% of $10,000 per one mean daily range, sizing
  only; `Exits::Strategy`; `weekdays`.
- Rows: `im/first` 08:30 → 09:00; `im/day` 08:30 → 15:00; `im/mid`
  12:00 → 12:30 (the contrast).
- Data: `xauduka` 15m, 2010-06-01 → 2018-06-14; 2,023 of 2,090 weekdays
  carry the 08:30, 08:45, 15:15 and 16:30 bars (counted before the run).
  Before 2013 the feed had no 17:00 halt. Costs: Vantage's, $0.28 spread,
  swap-free, one ounce a unit.
- Fixed replay (the test) against 300 random holds of the same window with
  a coin-flip side and the same sizing; walk-forward as the check; 1,000
  side permutations. Receipts `docs/research/runs/2026-09-14-intraday-momentum/`.
  The batch was run twice: the first receipts (kept under `null-late-exit/`)
  used a random-hold control that signalled its exit one bar late — 18:00,
  or Sunday on Fridays, for a hold the method closed at 16:45 — which the
  adversary found by noticing the two null columns disagreed (sized p50
  0.677 against direction 0.619) where on the close-reopen record they had
  agreed to two decimals. Fixed at `4eaef94` and re-run; the sized null's
  median is now 0.619, the direction null's 0.619, and for a fixed window
  the two are one test in two spellings, as they should be. Guards off, as
  in every receipt.

## Evidence

```
hypothesis   trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
im/first       2022   0.606  -0.017    0.619    0.684   35%   40th       fail: profit factor 0.606 < 1.2
im/day         2021   0.538  -0.021    0.619    0.684    1%    1st       fail: profit factor 0.538 < 1.2
im/mid         2021   0.593  -0.018    0.619    0.684   24%   22nd       fail: profit factor 0.593 < 1.2
```

(`in-sample-fixed.txt`, `direction-im-*.txt`, the corrected null.)
Walk-forward (`in-sample.txt`): 0.603 (66%) / 0.540 (11%) / 0.528 (5%).
"Nothing survived". With the late-exiting control the sized column read
5% / 0% / 1% (`null-late-exit/`); the direction column, the method's own
trades, is the same in both runs.

The null's median of 0.62 is what a coin flip on the 15:30–16:45 window
earns after $0.28 an ounce: the window's mean absolute move is $1.27, its
signed mean $0.16, and a sign rule has to be right over 60% of the time to
clear the spread (data-integrity's arithmetic). The first-half-hour sign is
right 50.5% of the time (sign correlation +0.011 over 2,019 days); the
day-so-far sign is wrong more often than a coin (48.4%, −0.031, six of
nine years negative, none individually significant).

## What each role said

- **adversary:** SURVIVED, for "closed on the primary; the first half hour
  carries no direction on gold" — *"the negative is not an artefact and
  hides no pass."* Order of commits clean (implement → register → run).
  Implementation verified from the code and an independent equal-lots
  replay (PF 0.616 / 0.541 / 0.629 against the receipts' 0.606 / 0.538 /
  0.593; the sizing explains the gap); the 16:45 fill bar exists in every
  year. On the window: *"gross signed mean per hold: first −$0.028, day
  −$0.109, mid −$0.016; net of $0.28: −0.31 / −0.39 / −0.30 … break-even
  needs ≥ 61% directional accuracy on a $1.27 move; `first` is right 40.0%
  of the time after cost. It is the spread, nothing worse."* Its strongest
  objection was an instrument fault, stated as a falsifiable claim — *"a
  RandomHold null with the exit intent on the bar before `held ≥ 75 min`
  will report a sized-null p50 within 0.01 of the direction null's 0.619,
  not 0.677"* — which the corrected re-run confirmed at 0.619. On the
  reversal: *"1st percentile = 7 of 1,000 below; two-sided p ≈ 0.014, times
  three rows ≈ 0.04. It is fragile: 2011 carries 64% of the gross reversal;
  ex-2011 the mirror sits at the 88th percentile of its own null … the
  mirror's PF on the window that produced it is 0.765 — it cannot pass the
  PF 1.2 gate even where it was found."* Not a bid-ask bounce (bid-only
  quote series; corr of the last two legs −0.07). On registering the mirror:
  legitimate *"only as a sign claim … it must be stated up front that at
  $0.28 the PF gate is unreachable even if the effect is real, so the
  outcome is 'direction exists / does not', never a paper run"*; the
  parameters frozen as they are; *"require ≥ 95th on both unseen windows
  independently (joint chance 0.25%) or ≥ 99th on one; a single 95th is the
  same coin"*; and *"the literature does not help it … the observed reversal
  looks like the 2011 blow-off regime, not a mechanism."*
- **data-integrity:** NO OBJECTION. 2,023 of 2,090 weekdays carry all four
  registered bars and 2,021 the 16:45 fill; the 67 missing are the US
  holiday calendar, five 2011–12 feed-dead mornings and two mid-day holes;
  six holds of ~2,022 exited late (one at 17:00, two through the break,
  three on a Sunday — Black Fridays with no 16:30 bar), gross −$3 an ounce
  of a −$628 net: immaterial. The window: mean +$0.156 (1.0 bp), median
  +$0.02, up 51.1%, mean absolute $1.27, 19% of moves inside ±$0.28; the
  predictor up 49.1%. *"With a $0.28 spread on a $1.27 mean absolute move
  and no information, expected PF ≈ 0.64 — the direction null's p50 (0.62)
  and the sized null's are exactly that."* Sign correlations first → last
  +0.011, day → last −0.031 (2011 −0.091, the rest |r| ≤ 0.073), mid → last
  −0.015. The 08:30 bar is 12:30Z in summer and 13:30Z in winter, nothing
  else. *"The SE of the mean move is $0.044, so a $0.28 edge would be 6 SE —
  the sample is ample, and the sign carries r ≈ 0.01."*
- **risk:** NO OBJECTION to closing with no promotion. Paper boundary
  intact (the two code commits add a strategy that returns intents and a
  null that draws holds; no network crate in either); the method is
  self-managed with a clock exit and *"can only outlive its day if the feed
  has no bar at all between `to` and the next day's `from`."* The null
  change touches no limit or guard. Enforced-vs-intended unchanged.

## Reading

The paper's effect is a few basis points in the last half hour of an
equity index, carried by late-informed traders and end-of-day hedging into
a cash close. Gold on a 24-hour CFD feed has no cash close at 17:00 New
York, its last hour is thin rather than crowded, and a 75-minute hold pays
0.6–2 basis points of spread. The receipts say the sign has no information
and the window has no room. What the window did show — the day's direction
reversing into its last hour — is the kind of thing this loop has learned
to write down and not to chase: it is one of six two-sided draws on the
window that produced it, and it is worth exactly one pre-registered look
on unseen bars, which the backlog carries.

## What would reopen this

Nothing for the claim as registered. The mirror — intraday *reversal*,
side = −sign of the day so far, parameters frozen — is in the backlog as a
sign claim only, on gold 2018-06 → 2026-05 and silver 2010–2026, both
unread by this mechanism, with the adversary's terms: ≥ 95th of the
direction null on both unseen windows or ≥ 99th on one, the PF gate stated
in advance as unreachable at $0.28 on a $1.27 window, and no paper run as
a possible outcome. It is a low-priority item: the effect where it was
seen is 64% one year and 0.765 in profit factor.

## What this does not say

- It does not say intraday momentum is absent in the markets the paper
  measured; it says the last hour of gold on this feed does not carry it.
- It does not say the reversal is real; it says it was observed once, on
  the window that produced it, at the 1st percentile of a test with six
  two-sided draws.
- It does not test silver or EURUSD for the momentum claim; the
  registration said it would not be re-run with another half hour or
  another asset without a new reason, and no new reason has appeared.
