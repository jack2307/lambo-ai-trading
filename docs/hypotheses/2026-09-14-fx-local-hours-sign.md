# 2026-09-14-fx-local-hours-sign: the euro's European-hours drift persists on 2018–2026 — a sign claim, not a trade

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** run — both conditions fail on 2018–2026; the sign did not persist; closed, under review
**Batch file:** `docs/hypotheses/2026-09-14-fx-local-hours-sign.toml`

## Where this comes from

`2026-09-14-fx-local-hours` closed on its gate: the short 03:00 → 11:00 New
York row was at the 100th percentile of random holds and the 99th of its
own permuted sides on 2,087 holds over 2010–2018, with a profit factor of
1.069 at 1.4 pips of spread and 1.158 at zero — the gate of 1.2 needed a
spread of −0.70 pips. The adversary's terms for one more look: *rows
byte-identical, fixed replay, the gate declared unreachable with the
number, no paper run under any outcome; the short at ≥ 95th of the
direction null and a pre-declared effect size, the window's drift minus
its trend share at or below −1.5 pips a hold, half the primary's; the
Asian row excluded for the bid-feed artefact.* This is that registration,
and nothing else: a question about whether Breedon and Ranaldo's clock is
still running, asked of eight years no run has read.

## Claim

On weekdays from 2018-06-16 to 2026-05-31, EURUSD held short from the
03:00 New York open to the 11:00 open — European trading hours — earns
more than the same holds with the side flipped, and the window's drift,
net of 8/24 of the same days' full-day change, is at or below −1.5 pips a
hold. The mechanism is the paper's: net local demand for the foreign leg
during local business hours. The primary's figures, for the record before
the confirmation is read: direction 99th (p = 0.007); excess drift −2.84
pips a hold (t = −3.18), negative in eight of nine years
(`scripts/fx_window_drift.py eurduka 0300 1100 2010-06-01 2018-06-15`).

## Falsifier

Both, on the single row, fixed replay, parameters byte-identical to the
closed registration's `fx/eu-short`:

1. ≥ 95th percentile of 1,000 permutations of the row's own sides
   (`direction-fx-eu-short.txt`).
2. `python scripts/fx_window_drift.py eurduka 0300 1100 2018-06-16 2026-05-31`
   reports EXCESS ≤ −1.5 pips per hold.

Either failing closes the sign; the gate (PF ≥ 1.2, 0.05R) is declared
unreachable at this cost — at zero spread the primary's short is 1.158 —
and is reported, not judged. **No outcome of this registration is a paper
run.** The sized random-hold null is reported as before and decides
nothing here: for a fixed window it is the direction null in another
spelling. The `us-long` and `asia-long` rows are not run: the American row
held Fridays through the weekend on this feed and the Asian row rides a
bid-only feed's spread cycle; both are one degree of freedom with the
European row, as the adversary said.

## Base method

`session-hold`, `from = 245`, `to = 1045`, `side = -1`, `riskDailyRanges = 1`,
`rangeDays = 20`, filters `weekdays`, `hours:0245-0250`. Unchanged.

## Data

- The window: `eurduka:15m` bounded `2018-06-16 → 2026-05-31`. Never read
  by this mechanism (it was read once, by `2026-09-14-tsmom-eurusd`'s
  registration, which named it as a confirmation it did not open; and its
  coverage was counted for `2026-09-14-fx-local-hours`: 2,062 of 2,071
  weekdays carry the 02:45 bar).
- There is no further window on this feed; the registration names none.
  If the sign holds, the record says it held on sixteen years; if it does
  not, it says the clock stopped, and when.
- Costs: as the closed registration; the `search` header now prints the
  spread in full (`49b5139`).

## Sample needed

About 2,060 holds; the primary's t-statistics were −2.38 on the raw drift
and −3.18 on the excess at 2,088.

## What each outcome means

- Both conditions hold → the effect is on record for 2010–2026 on one feed
  at one cost, as a fact about the euro's day; the decision record says
  what it would take to trade it (a cost at or below 0.5 pips, a book that
  must sell euros anyway) and proposes nothing.
- Either fails → the sign was the 2014–2016 dollar rally, or the effect
  has decayed since the paper; closed, with the year it stopped.
- Under no outcome is a paper run proposed; the risk role has said that
  promise is prose, so the record repeats it and names no strategy.

## The window (2018-06-16 → 2026-05-31, `eurduka` 15m)

Fixed replay (`in-sample-fixed.txt`) and the direction null
(`direction-fx-eu-short.txt`):

```
hypothesis      trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fx/eu-short       2057   0.926  -0.015    0.895    0.984   72%   67th       both conditions fail
```

`scripts/fx_window_drift.py eurduka 0300 1100 2018-06-16 2026-05-31`
(`fx-window-drift.txt`): drift −0.36 pips a hold (t = −0.43), trend share
−0.27, **excess −0.10 pips a hold (t = −0.16)**; by year 2018 +1.27, 2019
−1.56, 2020 −0.80, 2021 −0.90, 2022 +1.07, 2023 −2.50, 2024 −0.99, 2025
+3.38, 2026 +2.05. Against the primary's −2.84 (t = −3.18), eight of nine
years negative.

The clock stopped. On the eight years after the paper's sample the euro's
European-hours drift is a tenth of a pip a hold net of trend, and the
row's own sides permuted put it at the 67th percentile. Closed.
