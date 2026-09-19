# VERDICTS — what this desk has measured about every construct it can draw

**Why this file exists.** The owner asked for indicators on the Desk chart and
for a way to see which of them this desk has researched. Every registered
attempt to trade an indicator or a level *as a rule* has been killed out of
sample — 25 registrations closed with zero survivors by 2026-09-14
(`docs/decisions/2026-09-14-night-synthesis.md`), and the count kept rising
after that. An indicator offered on a chart with no verdict beside it is an
implicit recommendation, so the verdict has to travel with the name.

**How to read it.** Every number below is quoted from the document named in
the row. Nothing here is recomputed, re-rounded or re-derived; where two
documents give different numbers for the same thing, both are printed and both
are named (see *Contradictions in the record*). Where a registration is still
open, the row says open — not "no result".

**The machine-readable copy is `ui/src/lib/verdicts.ts`**, and
`ui/scripts/verdicts.check.mjs` re-reads every number in every badge against
the file the badge cites, so a badge cannot drift away from the record when the
record is amended.

**Status words.**

| word | meaning |
|---|---|
| `killed` | registered, run, and closed against its own pre-declared falsifier |
| `open` | registered and running; no verdict yet, and none may be implied |
| `parked` | run, but the instrument could not answer; closed on that ground, not on the numbers |
| `untested` | the desk has no registration for it as a rule — the record says so in as many words |

The gate every row was judged against: **PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30
trades**, plus ≥ 95th percentile of a count- and hours-matched null, plus
≥ 95th of the direction null (the method's own trades with their sides
permuted). `pct` below is the matched-null percentile; `direction` is the
direction-null percentile where the document gives one.

---

## 1. The twelve indicator ids

These are the ids in `crates/fd-indicators/src/lib.rs`. Each one is the series
a chart would draw; the verdict is about the **rule** built on that series, not
about the arithmetic of the line.

| id | claim tested | registration | decision | numbers, as the document states them | verdict |
|---|---|---|---|---|---|
| `sma` | none of its own. SMA200 appears only as the trend gate inside `rsi2-pullback` ("RSI(2) extremes with the SMA200 trend") | — | `2026-09-13-recent-year-screen.md` | the containing rule: 542 trades PF 0.881 all day (25%), 198 trades PF 1.110 New York (74%) | **untested** as a rule of its own |
| `ema` | `ema-cross`: a fast EMA crossing a slow one, ATR stop | `2026-09-13-recent-year-screen.md` (as a screen row); the BTC and gold baselines predate the hypothesis discipline | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md`, `2026-09-13-recent-year-screen.md` | BTC 15m: 787 trades, PF 1.033, 85th percentile — "inside the noise". Gold 15m intraday (corrected run): 1026 trades, PF 0.768, expectancy −0.132, 1%. Asian session on the last year: 37 trades, PF 1.632, 99% — **direction null 49th**, and 30th on the long window | **killed** |
| `rsi` | `rsi-reversion` (fade an extreme, exit the midline) and `rsi2-pullback` (Connors' two-bar RSI in the SMA200 trend) | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md`, `2026-09-13-recent-year-screen.md` | BTC 15m: 1247 trades, PF 1.011, 78th percentile. Gold 15m intraday (corrected): 1269 trades, PF 0.890, 38%. `rsi2-pullback/ny` on the last year: 198 trades, PF 1.110, expectancy 0.039R, 74% | **killed** |
| `macd` | `macd-cross`: the MACD line crossing its signal, histogram agreeing | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` | all day: 923 trades, PF 1.010, expectancy 0.011R, 66%. Asia: 300 trades, PF 1.556, 98% — but its direction null on the defaults is **93rd (p = 0.074)**, and 92nd (PF 0.908) on the 2022–25 context window | **killed** |
| `bbands` | `bb-fade` (fade a close outside the bands to the middle) and `squeeze-break` (band width at a lookback low, then a close outside) | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md`, `2026-09-13-recent-year-screen.md` | BTC 15m: 2463 trades, PF 0.952, 44th percentile. Gold 15m intraday (corrected): 3352 trades, PF 0.889, 36%. `squeeze-break/all` on the last year: 160 trades, PF 0.849, 14%. On 2022–25, `bb-fade/asia` runs 703 trades at PF 0.906 (68%) and is 100th on **direction** — "the side is right and the exits lose it" | **killed** |
| `atr` | not a signal anywhere. Tested as a **regime condition**: a break pays only when ATR14/close ≥ 0.075% | `2026-09-13-volcond-breakout.md` | `2026-09-13-volcond-breakout.md` | the high-volatility rows: PF 0.950 and 1.004, 70th–84th percentile of nulls gated to the hours *and* the volatility condition; the direction nulls are 43rd–54th on every long-window row | **killed** as a condition; it remains the desk's risk unit, which is not a claim |
| `vwap` | `vwap-fade`: a close ≥ 1.5 ATR from the session VWAP is pulled back to it | `2026-09-13-vwap-fade.md` | `2026-09-13-vwap-fade.md` | BTC 5m 2024-08 → 2026-09: 5828 trades, PF 0.951, 44%; US hours 1392 trades, PF 0.917, 24%. **Direction null 13th–15th** — if anything a stretch continues. Confirmation not opened | **killed** |
| `stoch` | `stoch-reversal`: %K crossing %D from an extreme | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` (receipt: `runs/2026-09-13-recent-year-screen/in-sample.txt`) | all day: 1297 trades, PF 0.912, expectancy −0.044R, 34%. New York: 507 trades, PF 0.923, 40% | **killed** |
| `adx` | no entry rule was ever built on it. Measured only as a **bias label** (`adx20 +DI`, `adx25 +DI`) among seventeen definitions | — (a read-only study, not a registration) | `2026-09-18-market-bias-definitions.md` | `adx20 +DI`: 4.3 flips per 100 bars, 26% undone within 3 bars, 10 bars of median lag, 19% of turns missed. Information: 17 definitions × 2 horizons = **34 tests**; the best cell in the whole grid is z = +1.98 against an **expected largest of 34 standard normals of +1.94** | **killed** as a directional label; never tested as an entry |
| `donchian` | `donchian-breakout`: a close outside the N-bar channel | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md`, `2026-09-13-recent-year-screen.md` | BTC 15m: 2088 trades, PF 0.972, 54th percentile. Gold 15m intraday (corrected): 3493 trades, PF 0.902, 42%. Last year all day: 519 trades, PF 0.979, 58%. As a bias label it is the most *label-stable* definition across anchors (1.2% on H4) and its WITH-minus-AGAINST statistic collapsed to +0.003R | **killed** |
| `keltner` | `keltner-break`: the first close outside the channel goes with it, stop at the midline | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` | all day on the last year: 533 trades, PF 1.052, expectancy 0.028R, 78%. Asia: 262 trades, PF 1.315, 90% — direction null **93rd**, and 68th (PF 0.883) on the 2022–25 context | **killed** |
| `swing` | confirmed fractal swings, i.e. the `fractal(k)` family. Measured as a bias label; registered **as a rule** in the `htf-filter` book, which is running | `2026-09-18-htf-context.md` | `2026-09-18-market-bias-definitions.md` (labels only — explicitly licenses no rule) | `fractal(2)` on H4 is the best of the 34 tests at z = **+1.98**, against an expected largest-of-34 of **+1.94**; on H1 it lags 12 bars and misses 23% of turns where `zigzag 3×ATR` misses 1%; it labels 18.5% of H4 bars differently between anchors. The bias note states outright: "No definition tested carries usable directional information" | **open** — `ai-xau-ds-ctx-htf-filter` began at 2026-09-18T04:07:34Z and has no verdict |

---

## 2. The strategy ids

These are the ids registered in `crates/fd-strategy/src/builtin.rs`. Nine of
them come from `builtin.rs` itself, five from `screen.rs`, the rest from their
own modules.

### Closed on a registered falsifier

| id | claim | registration | decision | numbers, as the document states them |
|---|---|---|---|---|
| `ema-cross` | a fast EMA crossing a slow one pays | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md` | BTC 787 trades PF 1.033 (85th); gold 1026 trades PF 0.768, expectancy −0.132 (1%) |
| `rsi-reversion` | fade an oversold/overbought RSI back to the midline | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md` | BTC 1247 trades PF 1.011 (78th); gold 1269 trades PF 0.890 (38%) |
| `donchian-breakout` | a close outside the channel continues | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md` | BTC 2088 trades PF 0.972 (54th); gold 3493 trades PF 0.902 (42%) |
| `bb-fade` | a close outside the bands returns to the middle | `2026-09-13-recent-year-screen.md` | `2026-09-12-technical-baselines.md`, `2026-09-12-gold-intraday-batch-1.md` | BTC 2463 trades PF 0.952 (44th); gold 3352 trades PF 0.889 (36%) |
| `keltner-break` | a close outside the Keltner channel continues | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` | 533 trades, PF 1.052, expectancy 0.028R, 78% of its matched null |
| `macd-cross` | the MACD line crossing its signal carries direction | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` | 923 trades PF 1.010 (66%); Asia 300 trades PF 1.556 (98%) with direction 93rd |
| `rsi2-pullback` | Connors' two-bar RSI extreme inside the SMA200 trend | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` | best row: 198 trades, PF 1.110, expectancy 0.039R, 74% |
| `squeeze-break` | Bollinger width at a lookback low, then a break | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` (receipt `runs/.../in-sample.txt`) | 160 trades, PF 0.849, expectancy −0.088R, 14% |
| `stoch-reversal` | %K crossing %D from an extreme reverses | `2026-09-13-recent-year-screen.md` | `2026-09-13-recent-year-screen.md` (receipt `runs/.../in-sample.txt`) | 1297 trades, PF 0.912, expectancy −0.044R, 34% |
| `orb` | a close outside the opening range continues | `2026-09-13-orb-ny.md`, also `2026-09-13-london-range.md`, `2026-09-13-btc-us-open.md`, `2026-09-13-volcond-breakout.md`, `2026-09-13-gold-m15-check.md` | `2026-09-13-orb-ny.md`, `2026-09-13-london-range.md`, `2026-09-13-m15-check.md` | ORB-NY: in sample two of three pass, then **PF 0.95–1.12** out of sample on four years; direction null 34th in, 54th out — "a coin flip on both samples". London range: in sample 257 trades PF 1.317 at the 98th with direction 96th — the cleanest in-sample pass the project has produced — then 525 trades PF 0.953 (77%) with direction 46th out of sample |
| `pdhl` | yesterday's high/low is absorbed (Asia) or broken (NY morning) | `2026-09-13-pdhl.md` | `2026-09-13-pdhl.md` | fade-asia 147 trades PF 0.645 (1%); fade-allday 555 trades PF 0.739 (5%); break-nyam 172 trades PF 0.758 (6%). Direction nulls 61st / 47th / 82nd. Confirmation never opened |
| `ict-sweep-mss-fvg` | sweep → market-structure shift → fair-value-gap retrace | `2026-09-13-ict-sweep-mss-fvg.md` | `2026-09-13-ict-sweep-mss-fvg.md` | survives three months of broker M1, then **PF 0.75–0.77** on 613–1428 trades over four years, expectancy −0.17 to −0.19R; the in-sample direction null was already 79th |
| `vwap-fade` | price stretched from session VWAP is pulled back | `2026-09-13-vwap-fade.md` | `2026-09-13-vwap-fade.md` | 5828 trades, PF 0.951, 44%; direction null 13th |
| `doji-reversal` | a doji ending a 1.5-ATR move marks exhaustion | `2026-09-13-doji.md`, `2026-09-13-doji-btc.md`, `2026-09-13-doji-2018.md` | `2026-09-13-doji.md` | the strict row passed the primary (61 trades, PF 1.203, direction 99th) against a null that was **the wrong one**; matched to its trade count the same row is **90th — inside**. Confirmation PF 0.782 on 25; never-seen third window PF 0.618 on 121, direction 34th; BTC PF 0.79/0.81, direction 23rd/16th |
| `gap-fade` | the weekend gap fills within four hours | `2026-09-13-gap-fade.md` | `2026-09-13-gap-fade.md` | 207 gaps, PF 0.423, **1st percentile**; large-gap row 0.768 on 109 (72nd); direction 33rd and 66th |
| `trend-pullback` | the first pullback against a slow trend continues it | `2026-09-13-trend-pullback.md` | `2026-09-13-trend-pullback.md` | gold 5,766 trades PF 0.379, **0th percentile**, direction 58th; BTC 2,931 trades PF 0.511, 0th, direction 97th–99th — "the chosen side beats its mirror, not that the trade makes money: it loses 0.38R a time either way" |
| `volume-thrust` | a high-volume bar closing at its extreme continues | `2026-09-13-volume-thrust.md` | `2026-09-13-volume-thrust.md` | BTC 840 thrusts PF 0.960 (50th), direction 30th; gold 297 PF 0.931 (78th), direction 78th |
| `volman-box` | a break of a tight box travels two more boxes | `2026-09-14-volman-box.md` | `2026-09-14-volman-box.md` | box-2 row: 3,460 trades, PF 0.639, 8th percentile of random entries, 1st of its own permuted sides **in compounded dollars** — and **38th in R** once equal-weighted. Box-3: 3,309 trades, PF 0.706, 43rd and 12th |
| `tsmom` | the sign of the trailing 20/60/120-day return persists | `2026-09-13-tsmom-2.md`, `2026-09-13-tsmom-silver.md`, `2026-09-14-tsmom-eurusd.md` (predecessor `2026-09-13-tsmom.md` was parked) | `2026-09-13-tsmom-2.md`, `2026-09-13-tsmom-silver.md`, `2026-09-14-tsmom-eurusd.md` | gold 60-day: PF 2.031 on 75 trades, 98th of the sized random-hold null, **86th of its own permuted sides** (p = 0.14); three of those 75 trades are all of the profit. Silver 20-day: 189 trades PF 1.341, 100th/98th on 2010–18, then PF 1.026 at 63rd/62nd on 2018–26. EURUSD 60-day: 108 trades PF 1.81, 100th of the sized null, **85th on direction**, one 356-day trade = 71% of net |
| `session-hold` | a clock hold earns more than random holds of the same length | `2026-09-13-close-reopen-drift.md`, `2026-09-13-friday-weekend-hold.md`, `2026-09-14-fx-local-hours.md`, `2026-09-14-fx-local-hours-sign.md`, `2026-09-13-btc-us-hours.md` | the same five | gold across the NY close: 1355 holds PF 1.254, **100th of both nulls**, expectancy 0.006R against a 0.05R gate; the weekday rows are 96th–100th on both windows and never pass the gate. Weekend leg: 336 holds PF 2.064, 100th on 2018–25 — then **400 holds, PF 0.983, net −$73, 91st / 92nd** on 2010–18, negative 2014 through 2018. Euro European hours: 2,087 holds, 100th/99th, PF 1.069 at 1.4 pips and 1.158 at zero — "the gate of 1.2 needed a spread of −0.70 pips" |
| `intraday-momentum` | the first half hour of New York signs the last hour | `2026-09-14-intraday-momentum.md` | `2026-09-14-intraday-momentum.md` | 2,022 sessions, PF 0.606, 35th of the sized random-hold null, 40th of its own permuted sides; the day-so-far spelling sits at the **1st** percentile of its own sides |

### Never registered as a claim

| id | why there is no verdict | source |
|---|---|---|
| `level-reversion` | never run. "The tape covers 114 of 70,080 bars (0.16%), and all four returned zero out-of-sample trades. **Not rejected — unasked**" | `2026-09-12-technical-baselines.md` |
| `maxpain-magnet` | the same sentence, the same four strategies | `2026-09-12-technical-baselines.md` |
| `flow-momentum` | the same | `2026-09-12-technical-baselines.md` |
| `flow-at-level` | the same. It is the founding thesis of the project and is still "waiting on a tape long enough to test" | `2026-09-12-technical-baselines.md`, `2026-09-14-night-synthesis.md` |
| `buy-and-hold` | a control the engine carries so other methods have something to be measured against; no registration in `docs/hypotheses/` states it as a claim | `crates/fd-strategy/src/builtin.rs` |
| `external` | a harness for a signal supplied from outside the engine; the signal is the claim, not this id | `crates/fd-strategy/src/external.rs` |

---

## 3. Registrations that are not a chart construct

Catalogued here so the ledger is complete, but they map to no indicator or
strategy id and therefore carry no badge: `2026-09-14-pre-nfp-drift`
(the loop's first surviving registration — 3.2nd percentile, −1.536 $/oz,
35.6% up, n=90, worth about $46 a year on $10,000, and the repository's own
news guard forbids trading the hour), `2026-09-14-nfp-vs-first-friday`,
`2026-09-15-nfp-cross-asset`, `2026-09-15-pair-residual`,
`2026-09-15-quote-asymmetry`, `2026-09-15-venue-residual`,
`2026-09-15-monthend-fix-slope` (**registered, not run** — the instrument does
not exist yet), `2026-09-16-dealer-hedge-demand` (**registered and locked**
until the tape holds 500 non-overlapping qualifying bursts, about December
2026), `2026-09-16-trailing-stop`, `2026-09-17-otl-context`,
`2026-09-17-prompt-coin-penalty`, `2026-09-18-plan-entry`,
`2026-09-18-plan-trigger`, `2026-09-18-smc-context`.

Three of those are worth knowing about when reading a chart badge:

- **`2026-09-16-trailing-stop`** — the script printed SURVIVES and the record
  refuses it: "19 of the 20 cells beat `off`" on the held-out window and the
  selection test came back at the **53rd** percentile, "a coin flip".
- **`2026-09-18-plan-entry`** and **`2026-09-18-plan-trigger`** — registered,
  **not started**; the books do not exist.
- **`2026-09-18-smc-context`** — registered, **not built and not started**; it
  is the levels-as-context claim, and it is explicitly *not* the levels-as-rule
  claim that died.

---

## 4. Contradictions in the record

A contradiction found is a result. None of these is smoothed over below; both
sides are named so a later reader can decide which to believe.

1. **Have the options-flow levels been tested, or not?**
   `2026-09-12-technical-baselines.md` says of `level-reversion`,
   `maxpain-magnet`, `flow-momentum` and `flow-at-level`: "Those have never
   been tested … **Not rejected — unasked**", and
   `2026-09-14-night-synthesis.md` agrees — "Options flow at levels, the reason
   the project exists, still waiting on a tape long enough to test."
   `2026-09-17-otl-context.md` says the opposite: "**Twenty-five registrations**
   in `docs/decisions/` tested levels of exactly this kind — POC, value area,
   gamma walls, whale support and resistance, max pain — as **mechanical
   rules**, and not one survived out of sample." Both cannot be true of the
   same four strategy ids. The reading that fits the receipts is that the 25
   registrations tested *price* levels (previous-day high/low, opening ranges,
   session ranges, ICT sweeps), not options-flow levels, and that
   `2026-09-17-otl-context.md` overstates what was in them — but the record
   does not say so anywhere, and the badge for these four ids follows the two
   documents that are specific about the tape.

2. **How many registrations have closed?** `2026-09-14-night-synthesis.md`:
   "the running total is **25**". `2026-09-15-pair-residual.md`, one day later:
   "when **twenty-nine** registrations have died".
   `2026-09-15-intraday-frontier.md`, written *the same day* and after two more
   closed: "**Thirty-two** registrations have closed with no survivor." The two
   2026-09-15 documents disagree with each other, not merely with the earlier
   one.

3. **Zero survivors, or one?** `2026-09-14-night-synthesis.md` is titled "seven
   registrations, zero survivors" and `2026-09-13-night-synthesis.md` "five
   hypotheses, zero survivors". Later the same night,
   `2026-09-14-pre-nfp-drift.md` records "**the first registration in this loop
   to** [survive its falsifier]". The syntheses cover windows that close before
   the NFP result; nothing in either has been amended to point at it.

4. **How many Fridays could the weekend hold not be confirmed on?**
   `2026-09-13-close-reopen-drift.md` says **68** Fridays, twice (title and
   body), and its registration agrees. `2026-09-13-friday-weekend-hold.md` says
   "the **67** Vantage Fridays since 2025-04", and `docs/research/BACKLOG.md`
   says 67.

5. **The doji's surviving percentile.** `2026-09-13-doji.md`'s own in-sample
   table still prints `doji/strict … 100% SURVIVES` with direction 99th. The
   decision of the same name says that null "turned out to be the wrong one"
   and that matched to its trade count the same row is at the **90th
   percentile — inside**. The registration was never amended; the addendum to
   `2026-09-13-night-synthesis.md` says every matched-null percentile written
   before the fix "should be read as **upper bounds**".

6. **The ORB's in-sample percentile.** `2026-09-13-orb-ny.md`'s table prints
   `orb/60m … 100% SURVIVES`; `2026-09-13-night-synthesis.md` records "100th →
   **92nd** once the null was window-gated"; the decision gives 92nd/86th in
   sample and 72nd/96th out of sample. Three numbers for one row, all in the
   record.

7. **Volman's box.** The registration's own in-sample note says "1st
   percentile / faded at the 99th"; its status line already flags this as "the
   compounded-dollar artefact the record explains", and the decision gives
   **38th in R**.

8. **"Structure was the only anchor-robust definition."**
   `2026-09-18-market-bias-definitions.md` corrects that sentence about itself:
   it was true of the *statistic* (+0.552R → +0.597R) and false of the
   *labels* — `fractal(2)` labels **18.5%** of H4 bars differently between
   anchors, while Donchian, whose statistic collapsed to **+0.003R**, is the
   most label-stable at **1.2%**.

9. **The NFP cell table.** `2026-09-14-nfp-vs-first-friday.md` says of itself:
   "**the claim is not supported and the cell table below is wrong** — it sums
   to 868 against 835 Fridays in the span … Neither `D = 643` nor
   `C = 33 releases` may be quoted." The wrong table is still in the file.

10. **`2026-09-15-pair-residual.md`** likewise contradicts its own prose: "the
    sentence below claiming this cell 'is not the maximum of any column' is
    **wrong** on the column that matters … the registered cell's 57.59% ranks
    1 of 27."

---

## 5. What this file must never be read as saying

- It does not say these indicators are useless to look at. It says every one of
  them, turned into a rule and tested against a matched null and a direction
  null, has been closed — and that a chart that draws them without saying so is
  making a recommendation nobody measured.
- It does not say the bias row predicts anything.
  `2026-09-18-market-bias-definitions.md`: "A bias row is a description of the
  chart. It is not a signal, and the card should say so in as many words."
- It does not close anything that is still open. `swing` carries **open**
  because `ai-xau-ds-ctx-htf-filter` is running, and a running book with a
  guessed verdict is worse than no badge at all.
