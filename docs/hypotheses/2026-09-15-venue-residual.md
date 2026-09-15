# 2026-09-15-venue-residual: the dealer's quote leaves the consensus before the price moves, and the leaving is readable

**Registered:** 2026-09-15 — written before any skew was computed; the only
numbers read beforehand were the row counts and time ranges of the bar files.
**Status:** decided (→ `docs/decisions/2026-09-15-venue-residual.md`). The declared
direction failed at the 1.90th / 12.75th / 0.95th percentile. Its mirror is
admissible under the two-sided wording above, survives the staleness and momentum
controls, and still reaches no cell's gate on the traded leg once trades may not
span a weekend (best 93.48, and it is one half-year). Three roles blocked; the
deciding one is that the residual is a sub-second quantity and both files are
stamped to the minute. The out-of-sample BTC venue pair was **not** opened.
**Script:** `scripts/venue_residual.py` (measurement; no strategy is implemented
unless the in-sample survives, and a survivor earns a paper proposal, nothing more)
**Batch file:** none — this is a two-feed measurement, not a `search` batch.

## Why this one, and why it is not on the internet

Every mechanism this loop has closed asks one venue's bars about themselves: a
level, a range, a session, a trend, a release. All of them are published, and
twenty-nine registrations later none survived out of sample. The owner's
instruction on 2026-09-15 was to stop testing what is published and to work the
data this repository owns and nobody else does.

Two feeds of the same metal, recorded simultaneously, is such a holding. It
cannot be reproduced from a paper because it requires an account at a dealer
and a feed from an ECN at the same time.

## Claim

Vantage is a dealer running its own book. Dukascopy is an ECN aggregating LP
quotes. They do not print the same price at the same instant, and the
difference is not only a constant markup.

A dealer moves its quote away from the consensus for one reason: **inventory**.
It has taken the other side of its clients' net position, and it prices to
discourage more of the same and to attract the offsetting flow. If that skew
carries information about where the price goes next — because the dealer sees
its own order flow and the consensus does not — then the skew must predict the
move on the **ECN leg**, not merely its own convergence back to consensus.

**Declared direction (primary):** a positive skew — Vantage mid above the ECN
mid after the markup is removed — precedes a **fall** in the common price. The
dealer marks up to unload a long it does not want; equivalently the clients are
net long and the clients are wrong.

The mirror (positive skew precedes a rise) is the other half of a two-sided
question and it is a second look at the same cell. Therefore **the falsifier is
the two-sided tail: the 97.5th percentile, not the 95th.** Stated here so that
a result in the unexpected direction cannot later be called a discovery at 95.

## Falsifier

All four, on the primary window, or the claim is closed:

1. **Alignment gate (first, and independent of any return).** See below. If the
   two feeds' clocks cannot be proven aligned to the minute, the study stops and
   the result is an instrument fault, not a measurement.
2. Mean forward ECN return, signed by the declared direction, over
   **non-overlapping** trades, at or beyond the **97.5th percentile** of the
   volatility-bucket-matched sign permutation at **100,000 draws**
   (SE 0.07 pp; declared per the backlog item "every percentile gate states the
   precision it requires").
3. At least 200 non-overlapping trades. The window holds about 70,000 bars; a
   cell that cannot reach 200 is a threshold chosen to find a handful of days.
4. Net of the round trip on the venue that would actually be traded (Vantage):
   positive at the **proportional** spread reading and reported at the
   dollar-constant one, both printed for every cell as `pair_residual` does.

A pass on (2) with a failure on (4) is written down as "direction exists, not a
trade" — the wording `2026-09-14-pre-nfp-drift` earned — and it is not narrowed
into a strategy.

## The faults this design has to clear, named before the run

- **Clock alignment is the whole study.** Both files are stamped UTC, but the
  Vantage bars come off the broker's clock (UTC+3 in New York summer, UTC+2 in
  winter) and were converted by our own exporter. This repository has been bitten
  by a timezone twice (OTL at UTC+7; MT5's broker clock). A misalignment of one
  bar manufactures a skew exactly the size of one bar's return — which is the
  size of the effect being looked for. **Proof, and it must not use returns:**
  gold stops for one hour a day (17:00 New York). Compute, per calendar week and
  per feed, the UTC minute-of-day at which the daily gap begins. The two feeds
  must agree to the minute, and both must shift by one hour on the New York DST
  dates. A week that disagrees stops the study.
- **A bar close on two venues is two different last ticks.** The skew therefore
  grows with volatility for a reason that has nothing to do with inventory.
  Control: the null permutes the skew's **sign within volatility deciles** of
  the bar's own range, so a skew that is only volatility cannot clear it.
- **The markup is a level and the signal is not.** The constant dealer markup is
  removed causally — a trailing median over the previous N bars, shifted by one,
  never the window's own mean.
- **Gaps.** Inner join on the stamp; a bar is dropped if either feed's previous
  bar is more than two bar-intervals behind it. Weekend reopens are not skew.
- **Overlap.** A signal fires on a large fraction of bars; only non-overlapping
  trades are counted, as `2026-09-15-pair-residual` established.

## Data

- **Primary in-sample** (the owner's recent-Vantage-year criterion, 2026-09-13):
  `XAUUSD-5m` x `XAUDUKA-5m`, **2025-06-01 to 2026-05-31** (12 months; inside both
  feeds, which overlap 2025-04-11 to 2026-05-31).
- **Context, not a second chance:** `XAUUSD-15m` x `XAUDUKA-15m`,
  **2022-06-16 to 2025-04-10** (the repository's long window). It is a different
  timeframe as well as a different era, so a disagreement between the two does
  not by itself say which of the two things caused it. Recorded, never used to
  rescue a failed primary.
- **Out-of-sample, named now, opened only after the in-sample result is written
  down:** `BTCUSD-15m` (Vantage CFD) x `BTCUSDT-15m` (Binance spot),
  **2024-09-12 to 2026-09-12**. A different asset and a different global venue,
  same mechanism. **Declared confound:** BTCUSDT is USDT-quoted spot against a
  USD CFD, so it carries a basis and funding term the metal pair does not; the
  residual is built from returns, never levels, which is exactly why.
- **Costs:** the Vantage round trip only (the ECN leg is the outcome variable,
  not a traded leg). The measured 2026-09-13 quote, $0.28 on gold, read both
  proportionally and dollar-constant over each window's own mean level.
- 1-minute bars cannot be used: `XAUUSD-1m` begins 2026-06-02 and `XAUDUKA-1m`
  ends 2026-05-31. **The two feeds do not overlap by a single minute at 1m.**

## Sample needed

At least 200 non-overlapping trades per cell. The cells are (markup window) x
(threshold) x (horizon); the count is printed with the results and the best of
them is a search, not evidence — the multiplicity is stated in the output.

## What each outcome means

- Survives the alignment gate, the permutation, the count and the cost, then the
  BTC venue pair → decision record, then a **paper** proposal. Nothing places an
  order, ever.
- Survives in-sample only → recorded as such; closed, not narrowed.
- Fails the alignment gate → an instrument fault that also puts a question to
  every past record that compared the two feeds (`2026-09-14-pre-nfp-drift`
  quoted a second venue's agreement), and it is written up as one.
- Fails the permutation → the dealer's skew is inventory noise or a
  last-tick artefact, and the venue family is closed on the metal.
