# The euro does fall through European hours — three pips a day, on two thousand holds, in eight of nine years — and the gate could not have passed at any spread

**Date:** 2026-09-14 (morning)
**Question:** Does EURUSD held short through European hours (03:00 → 11:00
New York) and long through American ones (11:00 → 17:00) beat sized random
holds of the same windows and its own flipped sides on Dukascopy 2010-06 →
2018-06, and again on 2018-06 → 2026-05 — Breedon and Ranaldo (2013): a
currency depreciates during its own local trading hours? With a 19:00 →
03:00 long as the contrast. Registered at `12481aa`, before any run, on
windows no run had read. `docs/hypotheses/2026-09-14-fx-local-hours.md`.
**Outcome:** **Closed on the gate, as registered; the confirmation was not
opened.** Every row has the sign the paper predicts, at the 96th–100th
percentile of both nulls on 1,669–2,087 holds: the euro fell 6,150 pips in
European hours over eight years in which it fell 707 pips in total. And
every row fails the profit-factor gate — 1.069, 0.983, 0.979 — because the
drift is three pips an eight-hour hold and the registration priced it at
1.4 pips of spread and a gate of five: at zero spread the short's profit
factor is 1.158, and the gate needs a spread of minus 0.7 pips. The
experiment was designed unable to confirm its own hypothesis, and the
registration said as much in advance. A clock-shaped sign is on record; a
trade is not. No paper run.

## What was measured

- Method: `session-hold` — entry signalled on the first bar of the window
  and filled at the next open, exit signalled on the first bar at or after
  `to` and filled at the next open; 1% of $10,000 per one mean New York-day
  range, a sizing unit only; `Exits::Strategy`; `weekdays`. Rows:
  `fx/eu-short` 03:00 → 11:00 short; `fx/us-long` 11:00 → 17:00 long;
  `fx/asia-long` 19:00 → 03:00 long (Monday–Thursday evenings in practice:
  Friday has no 18:45 bar).
- Data: `eurduka` 15m, 2010-06-01 → 2018-06-14; 2,092 / 2,087 / 1,673
  weekdays carry the 02:45 / 10:45 / 18:45 bars (counted before the run).
  Costs: Vantage EURUSD.sc, spread 0.00014 (p90 0.00020 inside the
  16:15–17:00 gate; not measured inside 02:45–03:00), swap-free by the
  account's stated terms, one euro a unit.
- Fixed replay (the test) against 300 random holds of the same window with
  a coin-flip side and the same sizing — the control corrected at
  `4eaef94` to fill its exit where the method's fills; the batch was first
  started before that fix, killed, its partial receipt deleted, and re-run
  (the adversary checked the sequence) — walk-forward as the check; 1,000
  side permutations. Receipts `docs/research/runs/2026-09-14-fx-local-hours/`;
  `diag-eu-short.txt`, `diag-us-long.txt` from `examples/diag_close.rs`,
  committed after the reviews asked (`1c94913`). Guards off, as in every
  receipt. The receipt header prints `spread 0.00` — a two-decimal format
  on a five-decimal number; the diagnostic's $417 a year of spread shows
  it was charged.

## Evidence

```
hypothesis      trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fx/eu-short       2087   1.069   0.014    0.922    1.027  100%   99th       fail: profit factor 1.069 < 1.2
fx/us-long        2082   0.983  -0.002    0.879    0.971   96%   97th       fail: profit factor 0.983 < 1.2
fx/asia-long      1669   0.979  -0.002    0.847    0.949   98%   98th       fail: profit factor 0.979 < 1.2
```

(`in-sample-fixed.txt`, `direction-fx-*.txt`.) Walk-forward
(`in-sample.txt`): 1.104 (100%) / 0.975 (93%) / 0.913 (87%). "Nothing
survived".

The windows on the bars themselves (data-integrity, weekdays, open to
open): 03:00 → 11:00 mean −2.94 pips a day, negative on 52.9% of days, sum
**−6,150 pips**; 11:00 → 17:00 +1.39, sum +2,909; 19:00 → 03:00 +1.65, sum
+2,753. EURUSD 1.2270 → 1.1563 over the primary: **−707 pips**. A pure
trend would put 8/24 of the fall in the European window, −383 pips; the
window's excess over its trend share is −5,785 pips, and it under-performs
its share in eight of nine years (2017, the euro's +14% year, is the one
that does not). Gross per hold −2.38 SE on 2,088 holds. Of the 6,150 pips,
5,776 sit in 03:00 → 08:30, before the US data prints — the paper's
mechanism, not the 08:30 release.

The short's P&L (`diag-eu-short.txt`; $10,000, 1% a hold): net $3,072 over
eight years, max drawdown $2,967 (20.9%), top five 44% of net, mean 11,469
euro-units ≈ $13,000 notional. Per year 2010 0.79, 2011 1.03, 2012 1.07,
2013 0.96, **2014 1.35, 2015 1.27, 2016 1.20**, 2017 0.72, 2018 1.39. The
adversary's split: in the four years the euro rose (916 holds) the row is
PF 0.910, −1.07 pips a hold after spread; in the four it fell (1,156) PF
1.226, +3.75; correlation of yearly PF with the year's euro change −0.90.
The relative effect — the window against its trend share — is there in
every year but one; the absolute row pays only when the dollar is
rallying. Spread sensitivity of the short: 0 → 1.158, 0.5 → 1.128, 1.0 →
1.099, 1.4 → 1.077, 3.0 → 0.991.

The long (`diag-us-long.txt`): net −$414, max drawdown 16.7%. Its 417
Friday holds exit not at 17:00 — Friday's last bar is 16:45 on this feed —
but at **Sunday 17:00**, a 54-hour weekend hold the registration did not
intend (the record's "the feed has no halt" was wrong for Fridays;
`weekdays:MoTuWeTh` existed and was not used); they sum −823 pips against
the Monday–Thursday holds' +3,732, so they dilute the American-hours sign
rather than make it. The Asian row is entered on a wide bid and exited on
a narrow one on a bid-only feed — a mechanical 0.2–0.4-pip tailwind
against a +1.63-pip gross drift; the adversary discounts it and so does
this record.

## What each role said

- **adversary:** SURVIVED, for "closed on the gate; the sign is real on
  this window" — *"I could not break the sign; I can break the P&L, and
  the gate was unreachable by construction."* Provenance clean, including
  the killed pre-fix run. On the gate: *"PF 1.2 needs a spread of −0.70
  pips … 0.05R on a 93.6-pip mean 20-day range is 4.68 pips net — more
  than the paper's whole effect."* On multiplicity: the three windows plus
  17:00–19:00 sum to the period change, so given the European window's
  fall *"us + asia > 0 is arithmetic, not evidence. Three rows at ≥ 96th
  is roughly one finding."* On the trend: *"it is 'short the euro during
  its own hours', not 'short the euro' … But the standalone P&L is
  trend-modulated: corr(year change, PF) = −0.90 … the row's $3,072 is
  entirely the dollar-rally years."* On tradability: *"no venue reaches
  PF 1.2 (needs a rebate), and size does not change a ratio. The one
  legitimate use is execution timing — a book that must sell euros anyway
  sells 03:00–11:00, buys 11:00–17:00, paying no incremental spread."* Its
  falsifiable objection — *"eu-short after 1.4 pips is a loss in years the
  euro rises: 916 holds, PF 0.910"* — stands unrefuted. Terms for a
  sign-only follow-up on the unread window: rows byte-identical, fixed
  replay, the gate declared unreachable with the number, no paper run
  under any outcome; and not "95th on all three rows" — *"the rows are one
  degree of freedom"* — but the short at ≥ 95th of the direction null
  **and** a pre-declared effect size, the window's drift minus its trend
  share at or below −1.5 pips a hold, half the primary's −2.77, with the
  Asian row excluded for the bid-feed artefact.
- **data-integrity:** NO OBJECTION, with three corrections carried above:
  the Friday `us-long` exits fill on Sunday (416 of 417), the Asian row does
  not cross the 17:00 rollover (0 of 1,669 nominal holds; it is the
  American row's Friday holds that do, unmeasured), and the yearly
  European-hours change correlates 0.92 with the yearly total — *"the
  sign is the market's, but a large part of it is the 2014–16 decline's,
  not a clock's; 2011–13 (+2,243 short pips, flat years) is the residual
  that looks like the paper's effect."* Its replay of the rule reproduces
  the receipts' per-year counts exactly; fill bars present on 416–419 of
  ~420 weekdays; 135 days a year-set (6.4%) have London opening at 07:00
  New York rather than 08:00 and contribute 206 of the 6,150 pips —
  immaterial. Provenance: the corrected binary was built at 08:51 and the
  receipts stamped 10:23–10:35.
- **risk:** NO OBJECTION to closing with no promotion. Paper boundary
  intact. Tail at 1% risk: *"maxDD $2,967 (20.9%); worst hold −3.12R /
  −$410 (2015-12-03, ECB); 2017 −$1,964, PF 0.72 after three winning
  years; mean 11,469 units ≈ $13k notional on $10k, i.e. the '1% risk' row
  is 1.3× leveraged with no stop."* *"To earn 10%/yr it must be run at
  ~2.6× — a ~55% drawdown, and 2017 alone would take ~$5,100 of $10k. The
  edge is 1.4% of a daily range per hold; the drawdown is not."* On the
  follow-up's "never a paper run": *"not enforceable by the record alone —
  the record is prose … Enforcement is: a registration that names no
  hypothesis with a paper-run outcome, and this role re-reviewing before
  any decision record proposes one."*

## Reading

This is the third time this loop has found a real clock in a price series
— gold across the New York close, gold's weekend, now the euro's working
day — and the third time the clock has been worth about what it costs to
touch. The effect here is the most robust of the three in sign (eight of
nine years relative to trend, 2,087 holds, the paper's mechanism visible
in the hours before the US data) and the least tradable: three pips a day
against 1.4 pips of spread, an unstopped 8-hour hold whose P&L is the
dollar's year, and a gate that arithmetic put out of reach before the run.
The registration knew the arithmetic and ran anyway, which was the right
thing to do for the observation and the wrong thing to call a test. What
is left is worth one more pre-registered look, on the adversary's terms,
as a question about the world and not about a bot.

## What would reopen this

- For the *sign*: a separate registration on `eurduka` 2018-06-16 →
  2026-05-31, never read by this mechanism — the `eu-short` row
  byte-identical, fixed replay, at or above the 95th of the direction null
  **and** a detrended effect size at or below −1.5 pips a hold (the
  window's drift minus 8/24 of the same days' change, computed by a script
  committed before the run), with the profit-factor gate declared
  unreachable (−0.70 pips) in the registration text and no paper run
  named as a possible outcome. It becomes a re-tune if any window edge,
  side, sizing, percentile or spread is touched after the run, if the
  walk-forward replaces the fixed replay, or if a row is added or dropped
  because of what the window shows.
- For the *trade*: nothing on this account. A measured all-in cost at or
  below 0.5 pips and a relative excess of 2.5 pips a hold in both euro-up
  and euro-down years on the confirmation would give a profit factor near
  1.13 — the adversary's number — which is still not a trade.

## What this does not say

- It does not say Breedon and Ranaldo are wrong; the sign they predicted
  is at the 99th percentile on this feed. It says the effect is the size
  they reported, and that size is the retail spread.
- It does not say the drift is a clock and not the dollar. The relative
  measure says clock; the absolute P&L says dollar; the confirmation
  window, if registered, is the place to ask which.
- It does not say the American-hours row is a five-day row. On this feed
  its Friday hold runs to Sunday, and the registration should have said
  `MoTuWeTh`.
- It does not price the 03:00 spread; the measurement was inside the
  16:15–17:00 gate. London's open is a narrow hour, and the adversary's
  sensitivity table covers 0 to 3 pips.

## Amendment (2026-09-14, later): the trend share, and the dollar reading

The "8/24 of the fall, −383 pips" above was the adversary's first
arithmetic (the period change apportioned by hours); the script the
sign-only registration committed (`scripts/fx_window_drift.py`) detrends
each hold by 8/24 of its own day's change and gives a share of −232 pips
and an excess of −2.84 pips a hold (t = −3.18), which is the figure that
reproduces. And the reading "the row's $3,072 is entirely the dollar-rally
years" is withdrawn by the adversary on the sign record
(`2026-09-14-fx-local-hours-sign.md`): the excess was concentrated in
2011–2016 and is positive in 2022, the largest dollar year on the feed.
Nothing in the verdict moves.

## Addendum (2026-09-14, afternoon): the news-desk's first check, and the row under a news blackout

The owner asked for a role that owns the scheduled-news calendar and, when
a paper bot runs, switches it off around releases. `news-desk`
(`.claude/agents/news-desk.md`) was given this row as its first
assignment, with the calendar `data/news/events.parquet` (747 releases
2010–2027: FOMC, US CPI, US NFP, ECB; `docs/news/README.md`) and the trade
list `diag-eu-short-trades.txt`. Its verdict: **CONTAMINATED for the P&L;
the sign survives, weakened.**

- Events inside the 03:00–11:00 window on 2010–2018: 83 ECB decisions
  (07:45 New York; two at 08:45 on DST-mismatch weeks), 97 CPI and 96 NFP
  (08:30); the 65 FOMC statements (14:00) fall outside every hold.
- 275 of 2,087 holds (13.2%) contain a release. They carry **$1,900 of the
  $3,072 net (61.8%)**, 32.0% of the gross pips (1,894 of 5,914 in the trade
  file) and 50.4% of the net pips; 6.9 gross pips a hold against 2.2 on the
  other 1,812. ECB Thursdays alone: 83 holds, 14.0 pips a hold, 43.9% of the
  net dollars, 19.7% of the gross pips; three of the top five holds and the
  worst one (2015-12-03) are ECB days. Win rate is the same on both kinds of
  day — the release days move more, they do not move more often.
- The adversary's claim — *"ECB Thursdays plus the 04:00–05:00 hour carry
  more than half"* — is **false for the sign** (68% of the gross pips sit on
  holds with no release, positive in seven of nine years, t = 1.78) and
  **true for the after-spread dollars** (13% of holds, 62% of net); the
  04:00–05:00 half is untestable from a trade list and a calendar without
  eurozone data releases.
- The bot's view: a `news:60-30` entry blackout blocks **none** of the 2,087
  entries — no release falls within 60 minutes of the 02:45 signal; the
  releases sit inside the hold. A hold that spans a release is avoided by
  setting the blackout's `before` to the hold's length (`news:600-600` here),
  which drops the 275 event holds and leaves 1,812 at net $1,173, PF 1.03,
  0.82 net pips a hold. A paper bot honouring a release blackout would have
  kept about a third of this row's P&L.
- Odd stamps: five holds are not 03:00 → 11:00 at all — Christmas-week feed
  gaps made `session-hold` fire at the 17:00 reopen and exit fifteen minutes
  later, one of them on a Sunday (2015-12-27) under a `weekdays` filter; net
  −$26, immaterial to the row, logged as an instrument note. The trade file's
  gross (5,914 pips, from `pnl / units`) and the record's bar replay (6,150)
  differ by 3.8%, partly those holds.

The same row re-run with the blackout in the engine, as context
(`docs/hypotheses/2026-09-14-fx-local-hours-news.toml`,
`runs/2026-09-14-fx-local-hours-news/`; the receipt header prints the
events loaded):

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   note
fx/eu-short             2087   1.069   0.014    0.922    1.027  100%   99th       the closed row, unchanged
fx/eu-short-news        2087   1.069   0.014    0.922    1.027  100%   99th       news:60-30 blocks no entry: the releases sit inside the hold
fx/eu-short-newsday     1812   1.034   0.007    0.918    1.025   96%   93rd       news:600-600 drops the 275 release holds
```

(`in-sample-fixed.txt`, `direction-fx-*.txt`; walk-forward 1.104 / 1.104 /
1.061 at 100% / 100% / 98%.) Without the release days the drift keeps its
sign at the 96th of random holds and the 93rd of its own sides — a
between-releases drift of two pips a hold, real but a third weaker than
the row that carried the releases.

What this changes in the record: nothing in the verdict — the row was
closed on its gate — and one sentence in the reading: the European-hours
drift is a between-releases drift of about two pips a hold plus a release-day
drift of seven; the paper's mechanism is the first, and the dollars were
mostly the second.
