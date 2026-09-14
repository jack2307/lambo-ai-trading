# The euro's European-hours drift stopped: three pips a day on 2010–2018, a tenth of a pip on 2018–2026

**Date:** 2026-09-14 (late morning)
**Question:** On the eight years no run had read — `eurduka` 2018-06-16 →
2026-05-31 — does EURUSD short from 03:00 to 11:00 New York still beat its
own flipped sides (≥ 95th) and still drift at least 1.5 pips a hold below
its share of the day's move, as it did on 2010–2018 (99th; −2.84 pips,
t = −3.18)? A sign claim only, on the adversary's terms, with the gate
declared unreachable and no paper run named as an outcome. Registered at
`2fcb9bd` with the effect-size script, before any run.
`docs/hypotheses/2026-09-14-fx-local-hours-sign.md`.
**Outcome:** **Closed; the sign did not persist.** 2,057 holds, PF 0.926,
67th percentile of the row's own sides permuted (p = 0.33); detrended
excess −0.10 pips a hold (t = −0.16); five of nine years negative, four
positive, 2025 the most positive of all sixteen. Breedon and Ranaldo's
clock, which this feed showed running through 2016, is not running on
2018–2026 at any cost. With `2026-09-14-fx-local-hours` this closes the
local-hours family on the one pair on disk.

## What was measured

- One row, byte-identical to the closed registration's `fx/eu-short`:
  `session-hold`, short, signal on the 02:45 bar and fill at the 03:00 open,
  exit signal on the 10:45 bar and fill at 11:00; 1% of $10,000 per one
  mean New York-day range, sizing only; `Exits::Strategy`; `weekdays`.
- Data: `eurduka` 15m, 2018-06-16 → 2026-05-31 (exclusive); 2,062 of 2,071 weekdays
  carry the 02:45 bar (counted for the earlier registration). Costs as
  before (spread 0.00014, swap zero stated, one euro a unit).
- Fixed replay against 300 corrected random holds (reported, not
  deciding); 1,000 side permutations (deciding); and
  `scripts/fx_window_drift.py eurduka 0300 1100 2018-06-16 2026-05-31`
  (deciding): the window's open-to-open drift per weekday hold minus 8/24
  of the same New York day's change, in pips. Receipts
  `docs/research/runs/2026-09-14-fx-local-hours-sign/`. Guards off, as in
  every receipt.

## Evidence

```
hypothesis      trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fx/eu-short       2057   0.926  -0.015    0.895    0.984   72%   67th       both conditions fail
```

(`in-sample-fixed.txt`; `direction-fx-eu-short.txt`: null p05 0.812, p50
0.902, p95 1.003, actual 0.926 → 67th, p = 0.330.) Walk-forward
(`in-sample.txt`): 1,649 holds PF 0.906 (51%).

The effect size, both windows, from the same script (`fx-window-drift.txt`
and the registration's primary figure):

```
window              holds   drift/hold   trend share   EXCESS/hold    t       years negative
2010-06 → 2018-06   2088    -2.95 pips   -0.11         -2.84         -3.18   8 of 9
2018-06 → 2026-05   2056    -0.36 pips   -0.27         -0.10         -0.16   5 of 9
```

By year on the confirmation: 2018 +1.27, 2019 −1.56, 2020 −0.80, 2021
−0.90, 2022 +1.07, 2023 −2.50, 2024 −0.99, 2025 +3.38, 2026 +2.05 (pips a
hold, excess). The primary's worst year was +0.72 (2017); the
confirmation has three years above that.

## What each role said

- **adversary:** SURVIVED — *"I could not make the negative an artefact."*
  Provenance clean: the script and the batch file are byte-identical
  between the registration commit and the receipts; the threshold was set
  before registration. The script *"is the measure I asked for"*; replayed
  read-only it reproduces both windows (−2.84 / t −3.18; −0.09 / t −0.15).
  Not a noisier clock but a stopped one: *"per-hold SD fell from 40.8 to
  27.5 pips, so the confirmation was the less noisy sample: a true −1.5
  would have printed t ≈ −2.5 … Removing 2025–26 entirely gives −0.76 on
  1,691 holds — still above −1.5."* It withdrew its own earlier reading of
  the primary: *"the share measure removed only −0.11/hold from the primary
  … the decisive test is 2022: the largest intra-year dollar rally of all
  sixteen years, and the window's excess is +1.07. So the primary's effect
  was not 'the dollar rally leaking into European hours'"* — it was
  concentrated in 2011–2016, *"the euro-crisis and ECB-easing regime, and
  either that regime's euro-specific news landing inside the window (ECB
  07:45/08:30 NY, eurozone data 04:00–05:00 NY) or a 3-SE fluctuation on a
  first pre-registered row."* Agrees the other rows must not be run. Its
  falsifiable objection is about the mechanism, not the closure: *"on
  2010–2018, holds on ECB-decision Thursdays plus the 04:00–05:00 NY hour
  carry more than half of it"* — untested, and not this registration's
  question.
- **data-integrity:** NO OBJECTION. The 2018–2026 slice is *"as complete
  as the 2010–2018 slice it is being compared with"*: 2,056 holds on 2,075
  calendar weekdays (99.1%) against 2,088 on 2,098 (99.5%); six non-holiday
  weekday outages (0.3%), one of them a four-day feed gap in December 2024;
  DST-misaligned days 7.1% against 6.4%; the script's output identical to
  the receipt and its primary figure reproduced; the receipt's command
  line matches the batch file and the binary post-dates both fixes. The
  raw window by year, both windows side by side, is in its report and the
  reading it supports is: *"2019/2021/2023/2024 still drift −2 pips a hold;
  2025 alone (+1,134 pips) erases them."* Two caveats taken: the earlier
  record's "trend share −383 pips" was the adversary's first method, not
  the script's (−232), and the registration's −2.84 is the figure that
  reproduces; and the year where the sign flipped, if one is to be named,
  is 2025, not 2018.
- **risk:** NO OBJECTION to closing with no promotion. Paper boundary
  intact (a print format, a diagnostic argument, a read-only script);
  *"'No outcome of this registration is a paper run' … in the
  pre-registration commit, before the run."* Enforced-vs-intended unchanged.

## Reading

The paper's sample ended in 2010 and this feed showed the effect for six
more years, concentrated in 2011–2016 — the euro-crisis and ECB-easing
regime — and not, as the earlier record read it, in the dollar's rally
(2022, the largest dollar year of the sixteen, has the window's excess
positive). After 2018 it is a third of its size in the years it keeps its
sign and reversed in the others, and 2025 alone undoes them. Whether it was
arbitraged, or was that regime's euro news landing inside European hours,
or was a three-standard-error fluctuation on a first registered row, the
receipts cannot say, and the registration did not ask them to: it asked
whether the clock was still running, and the answer is no. The registration
promised "the year it stopped"; the honest answer is that no year is
individually significant, and the year the sign flipped is 2025. This is the cleanest closure of the night — one row, two
conditions named in advance with numbers, a script committed before the
run, an unread window, and a miss on both by a wide margin.

## What would reopen this

Nothing on this pair and this feed. The paper's claim on another pair
(sterling in London hours, the yen in Tokyo hours) would be its own
registration with its own data and the same terms, and the same sentence
about cost: the effect where it exists is the size of the retail spread.

## What this does not say

- It does not say the 2010–2018 finding was noise; it was at the 99th
  percentile on 2,087 holds and its excess was 3 standard errors from
  zero. It says it did not continue.
- It does not say when the clock stopped; the yearly excess is negative in
  2019–2021 and 2023–2024 at a third of the primary's size and positive in
  2018, 2022 and 2025–2026, and no year is individually significant.
- It does not test the American or Asian rows, by registration: they are
  one degree of freedom with this one, and one of them was a weekend hold
  on this feed.
