# Volman's three boxes on 5-minute gold: the break of a tight box carries no direction, and the box geometry loses the spread — on eight years and on the last one

**Date:** 2026-09-14 (afternoon)
**Question:** The owner's request — Bob Volman's measured move, mechanised:
on gold 5-minute bars, a close beyond a tight box (the preceding 12–30
bars, no taller than three ATR) on the right side of the 18-period EMA,
entered at the next open with the stop at the box's far side and the
target one box (box 2) or two boxes (box 3) beyond the near edge — does it
beat random entries with the same exits and its own flipped sides on
Dukascopy 2010-06 → 2018-06, and again on 2018-06 → 2026-05? With the
owner's own criterion run alongside: the broker's 5-minute bars,
2025-04 → 2026-09. Registered at `34b1b71` after the method was implemented
and tested (`7464167`); amended once at `8397c62`, before any informative
run, because the first compression threshold fired once in eight years.
`docs/hypotheses/2026-09-14-volman-box.md`.
**Outcome:** **Closed on the primary; the confirmation was not opened.**
The box-2 row: 3,460 trades, PF 0.639, 8th percentile of random entries
with the same stop and target, 1st of its own sides permuted *in the
receipt's compounded dollars* — and, as the adversary showed by
re-implementing the rule, the **38th in R**: the receipt's profit factor
is weighted by an equity curve that fell from $10,000 to $84, so 47% of
it is the 2010 stub. Equal-weighted, the breakout side is a coin flip and
the faded side loses too (PF 0.73 in R). The box-3 row: 3,309 trades, PF
0.706, 43rd and 12th. Before the spread the entry is flat (−$79 an ounce
over eight years); after it, at a spread that is 13% of the median risk,
it loses a seventh of its risk a trade. On the broker's last seventeen
months, the owner's criterion: PF 0.92 and 0.97, 47th–78th, the same
picture at a spread that is 3% of the risk. A tight box's break on
5-minute gold carries no direction, and the box geometry is a cost
structure, not an edge — the closed breakout family's finding, restated
with the stop and target in box heights. No paper run.

## What was measured

- Method: `volman-box`, new (`crates/fd-strategy/src/volman_box.rs`, seven
  tests including no-lookahead). Box = the `boxBars` bars before the
  signal bar, height ≤ `maxBoxAtr` × ATR(14) of the bar before; Long when
  the close exceeds the box high, is above EMA(18) and at most one ATR
  past the edge; Short mirrored. Stop at the box's far side, target
  `boxes` box-heights beyond the near edge; the engine fills at the next
  open and enforces both (`Exits::Engine`). `boxBars` selected by the
  walk-forward from {12, 20, 30}; everything else pinned (`maxBoxAtr = 3`,
  `maxBreakAtr = 1`, EMA 18, ATR 14); the fixed replay at `boxBars = 20`.
  Not implemented, by intent: the discretionary early exit on a reversal
  bar with volume (no volume on the feed) and the trendline
  re-measurement (no trendline in the method).
- Primary: `xauduka` 5m, 2010-06-01 → 2018-06-14. Context: `xauusd` 5m
  (Vantage), 2025-04-11 → 2026-09-12. Costs: Vantage's, $0.28 spread,
  swap-free, one ounce a unit.
- Walk-forward (the test for a gridded method) against 200 count-matched
  random entries with the same exit geometry; the fixed replay alongside;
  1,000 side permutations with stops and targets mirrored. Receipts
  `docs/research/runs/2026-09-14-volman-box/` (the one-trade run under
  `one-trade/`) and `…-volman-box-vantage/`. Guards off, as in every
  receipt.

## Evidence

Primary, fixed replay (`in-sample-fixed.txt`) with the direction
percentile (`direction-box-*.txt`):

```
hypothesis   trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
box/b2         3460   0.639  -0.137    0.715    0.806    8%    1st       fail: profit factor 0.639 < 1.2
box/b3         3309   0.706  -0.139    0.716    0.805   43%   12th       fail: profit factor 0.706 < 1.2
```

Walk-forward (`in-sample.txt`): b2 6,070 trades PF 0.700 (4%); b3 255
trades PF 0.859 (100th of a null whose median is 0.751 — the exit
geometry loses a quarter of its risk by construction on random entries;
the adversary's check: a 255-trade PF drawn from a population at 0.76
reaches 0.859 about a fifth of the time, so the 100th is a variance
mismatch between a thin row and thick null runs, not a finding).

The adversary's independent replay of the rule and the fill model on the
same bars (`scratchpad/volman_oracle2.py`, not a receipt): b2 3,317 trades
PF 0.637 / −0.141R against the engine's 3,460 / 0.639 / −0.137R; b3
2,973 / 0.730 / −0.124R against 3,309 / 0.706 / −0.139R; every grid cell
negative in R (boxBars 12/20/30: b2 0.69/0.70/0.70, b3 0.80/0.81/0.86).
The realised geometry is not one-to-one: median box $1.80 (2.6 ATR),
entry excess $0.21, half-spread each side → risk $2.19 against reward
$1.40 on b2, a reward-to-risk of 0.66, break-even hit rate 68.8% after
spread against 57.3% realised; b3 break-even 45.5% against 38.8%. Before
the spread, b2's eight years net −$79 an ounce on 3,317 trades: the entry
is flat. Signed drift after entry: −0.17 ATR at one bar (the half-spread),
−0.36 ATR at 24 bars — at most a tenth of a spread of reversion.

The owner's context (`…-vantage/in-sample-fixed.txt`, `in-sample.txt`,
`direction-box-*.txt`):

```
hypothesis   trades  OOS PF  expect  null p50 null p95   pct  direction     walk-forward
box/b2          697   0.920  -0.030    0.925    1.050   47%   68th          441 trades 0.926 (42%)
box/b3          672   0.966  -0.013    0.928    1.051   73%   78th          181 trades 1.027 (88%)
```

## What each role said

- **adversary:** BLOCK on the closure *as first worded* ("the box breakout
  reverts; faded it would be at the 99th"), SURVIVED for "closed on the
  primary; the entry carries no direction; the geometry loses to the
  spread" — and the record is worded that way. The negative is real and
  independently reproduced (*"oracle b2: 3,317 trades PF 0.637 vs engine
  3,460 / 0.639"*). The 1st percentile is an instrument artefact: *"Engine
  PF is gross win/loss on `pnl_usd` with lots = 1% of compounding equity.
  At −0.14R/trade equity runs $10,000 → $84, so the 3,460-trade PF is
  weighted 46.7% on Jun–Dec 2010 … Equal-weighted in R, b2 PF 0.695 sits at
  the 38th percentile of a coin-flip null … The full faded replay: PF(R)
  0.731, −0.122R — the fade loses too."* *"A 'box fade' is the sign-flipped
  coin of the closed range-break family — same family, not a
  registration."* The mechanisation is fair: the stop and target are
  Volman's drawing; the amended 3.0 ATR admits the tightest 8% of 20-bar
  windows (*"a genuine compression filter"*); the amendment was a units
  correction made after seeing one trade and a negative walk-forward,
  *"clean, with that caveat stated."* On the owner's batch: *"no direction
  anywhere; the PF level is the spread-to-box ratio (12.8% in 2010–18 vs
  2.6% now)."* Its falsifiable claim — an engine direction null on fixed
  lots or on R would put b2 between the 25th and 50th, not the 1st — is
  not yet runnable and is the instrument item below. *"Log the compounding
  weight as an instrument fault: it touches every thousands-of-trades
  negative in this loop."*
- **data-integrity:** NO OBJECTION. The 5m file's 2010–2018 part:
  574,640 bars in the window (the receipt's count), 2.996× the 15m,
  monotone and unique, zero OHLC violations, zero flat bars, the old part
  byte-identical to its backup, and a full recompute from the 1m file
  matches every bar. Coverage 92–98% of weekdays by year, the missing
  7% from 2013 the daily halt; 87 holes over 90 minutes in eight years,
  all holidays. The rule on this data: 7.9% of bars sit in a box under
  3 ATR (2011 5.0% … 2017 8.2%), 4,734 EMA-filtered breakouts before
  one-at-a-time, signals spread over every hour and year. EMA and ATR are
  single-pass causal recursions (`fd-indicators/src/lib.rs`), keys
  `ema_18`/`atr_14` verified, the no-future test present. Caveat: the
  2010 stub has a 20.7% tight-box rate against 5–8% elsewhere (a bid-only
  quoting property), 16% of signals; dropping it leaves ~2,900 trades at
  the same verdict — and it is the stub the compounded PF over-weights.
  The Vantage 5m file: 100,749 bars, volume present, tight-box rate 6.6%.
  Provenance in order; the binary in the receipts post-dates the
  implementation by mtime.
- **risk:** NO OBJECTION to closing with no promotion. Paper boundary
  intact. This is the night's first `Exits::Engine` method, and the stop,
  target and four-hour timeout *are* enforced (`engine.rs:552–574`), a
  gap fills at the bar's open (losses over 1R are modelled), stop before
  target within a bar. Tail numbers are not in the receipt table (no
  equity, drawdown or worst trade printed — say so). Spread arithmetic:
  *"33 oz/trade × $0.28 = $9.3 ≈ 0.09R per trade; ×430/yr ≈ 40R ≈ 40% of
  equity a year on spread alone. That is the PF 0.64–0.71."* Every
  receipt unguarded, as before.

## Reading

The owner asked for a specific method and the loop did what it does with
one: wrote the rule down before running, ran it on eight years the method
had never seen and on the broker's last year, and read it against random
entries with the same exits and against its own sides flipped. The
breakout side of a tight box on 5-minute gold is a coin flip — flat
before costs on 3,317 trades, the 38th percentile of its own sides in R —
which is the finding the loop has now made on opening ranges, London
ranges, ICT sweeps, volatility-conditional breaks, Keltner and squeeze
breaks, and Volman's boxes. The measured-move exit changes how much each
break costs, not whether it carries information: with the stop a box away
and the target a box away, the spread is an eighth of the risk and the
break-even hit rate is 69%, and a coin flip does not get there. The mirror
loses too. What the review found on the way is the more useful thing: the
receipts' profit factor is a compounded-dollar number, and on a long
losing sequence it is a number about the first year; the loop's negatives
on thousands of trades need reading in R, and the instrument will be
changed to print it.

## What would reopen this

Nothing on gold. The method as Volman trades it — 70-tick or one-minute
charts, boxes drawn by eye, a discretionary exit on the first reversal
bar into box 3 — is not a mechanical claim, and the mechanical parts of
it have now been tested. A different asset would be a different
registration with a reason; on this feed the reason is absent.

## What this does not say

- It does not say Volman's method loses for a trader who draws the boxes
  by hand and exits on a reversal bar. It says the mechanical core — a
  tight-box break on the right side of the EMA, stop and target in box
  heights — loses on gold at $0.28 on 3,460 trades over eight years and
  ~700 over the last one.
- It does not say the box fade is a trade. In R it loses (0.73) like the
  break does; the "99th" in compounded dollars is the artefact above.
- It does not report a drawdown or a worst trade; the receipt table does
  not carry them and no diagnostic was run on an `Exits::Engine` method
  tonight. At −0.14R a trade over 3,460 trades the equity curve is the
  drawdown.
- It does not test M1 (the Dukascopy minutes are on disk); a five-minute
  box of 12–30 bars is the same structure at 60–150 minutes, and nothing
  in the result suggests the minute would differ in sign.
