# Task D — `spread / stop` across every instrument on the feed

**Registration:** `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`, task D.
**Store:** `E:/rust/flowdesk/data-sealed`, clamped at 2025-09-23 00:00:00Z exclusive.
The clamp dropped **zero** bars from every file it read, which is what the sealed root
is supposed to guarantee and is printed in both receipts. The withheld year was not
read; a cost structure is not a year's result.
**Tool:** `fd-backtest --bin cost_table` (new), with `crates/fd-backtest/src/cost_table.rs`.
**Receipts:** `cost-table-all-instruments.txt`, `cost-table-long-history-only.txt`.

## The claim, answered in one sentence

**Refuted.** On a stop rule held identical across instruments, **XAUUSD is the cheapest
instrument on this feed that the desk can actually trade**, at every one of three
horizons; the only row that beats it is BTCUSD.sc by 13–28% on a spread the config
itself says has "never been checked over time at all", and the one figure cheaper than
that belongs to a Binance spot market whose spread is explicitly **assumed** and which
is not an account this desk trades. Every other instrument measured is **1.5× to 3.5×
more expensive** than gold. The cost problem is not an instrument-selection problem and
no further instrument screening is warranted.

## The stop rules, declared

Three, because the two gold figures already in the record (0.67% of R at a 41.57-point
multi-day stop, 14.0% at a 2.00-point intraday one — the same spread, a 21× difference)
show one rule cannot answer the question.

| rule | stop | ATR series |
|---|---|---|
| **I** intraday | `1.5 × ATR14` | 15-minute bars |
| **H** the cap's horizon | `1.5 × ATR14` | 4-hour bars — four hours is `[trading] max_hold_ms`, which `trading_rules_for` applies to every market, so this is the horizon every `Exits::Engine` method in this record was force-closed at |
| **M** multi-day | `1.5 × ATR20` | session-daily bars, aggregated from the 15m series with the day boundary on the 17:00 New York metals/FX break (21:00Z = midnight on the broker's UTC+3 clock) |

1.5 is not chosen here: it is the multiple the registry's nulls run at and the one both
funded books sit at. **The multiple cannot reorder instruments** — the ratio is
`spread / (k × ATR)`, so `k` is a pure rescale and the ordering is identical at 0.8, 1.5
or 3.0. What reorders instruments is the ATR's own timeframe, which is why three are
reported.

Rule M's aggregation **groups by timestamp, it does not count bars**, which is what makes
it safe across the 2013 Dukascopy bar-rule change (longest consecutive 15m run 476 before,
exactly 92 after). A day that lost bars is a day with a smaller range, not a day that
vanishes; two tests pin this.

## Units, and the cancellation shown

```
cost = spread [price/unit] × lots × contract_size [units/lot]
R    = stop   [price/unit] × lots × contract_size [units/lot]
cost/R = spread / stop     — dimensionless
```

`lots`, `contract_size` and the price unit all cancel. That is the only reason a figure
for gold (dollars per ounce, contract 1), silver (dollars per ounce, contract 50), EURUSD
(dollars per euro, contract 1000) and BTC (dollars per bitcoin, contract 0.01) can share a
column. A position pays the spread **once** for the round turn. `cost_fraction_of_r`
performs the division with both products written out rather than asserting the
cancellation, and the receipt reproduces the record's own two gold figures from it.

## The table — median `spread / stop` (p10, p90), configured spreads

Pooled over each instrument's whole usable history, which differs enormously:

| market | symbol | spread | history | **rule I** | **rule H** | **rule M** |
|---|---|---:|---|---:|---:|---:|
| btc | BTCUSD (Binance spot) | 5.000 ⚠ assumed | 2024-09→2025-09 | **1.322%** (0.697 / 2.513) | **0.285%** | **0.110%** |
| btcusd | BTCUSD.sc | 17.05 ⚠ one read | 2023-10→2025-09 | **5.131%** (2.638 / 11.538) | **1.139%** | **0.410%** |
| **xauusd** | **XAUUSD.sc** | **0.28** | **2022-06→2025-09** | **7.502%** (3.844 / 13.234) | **1.813%** | **0.691%** |
| xauduka | XAUUSD.sc on Dukascopy bars | 0.28 | 2010-06→2025-09 | 10.892% (5.170 / 20.145) | 2.547% | 0.962% |
| eurduka | EURUSD.sc | 0.00014 | 2010-06→2025-09 | 13.015% (6.675 / 24.445) | 3.042% | 1.154% |
| eurusd | EURUSD.sc | 0.00014 | 2024-03→2025-09 | 15.413% (8.625 / 28.652) | 3.454% | 1.331% |
| xagduka | XAGUSD.sc | 0.021 | 2010-06→2025-09 | 27.872% (11.777 / 61.497) | 6.784% | 2.646% |

`n` per cell is in the receipt: 36k–381k bars at rule I, 2.2k–25k at H, 358–4,243 at M.

Two figures in the record are reproduced independently by this table: gold's multi-day
share, **0.691%** here against the record's **0.67%** (median stop 40.54 points against
its 41.57), and silver's, **2.646%** here against the record's **2.70%** — silver at
**3.8× gold's share on a spread a thirteenth the size**, because its stops are
proportionally tighter. The cheap-looking spread really is the trap.

## Per era, because coverage differs and the ratio moves with the price level

Median, rule M (the full grid for all three rules is in the receipt):

| market | 2010-12 | 2013-15 | 2016-18 | 2019-21 | 2022-25 | pooled |
|---|---:|---:|---:|---:|---:|---:|
| xauduka | 0.937% | 1.149% | 1.481% | 0.823% | **0.706%** | 0.962% |
| eurduka | 0.747% | 1.019% | 1.162% | 1.539% | 1.275% | 1.154% |
| xagduka | 1.646% | 3.311% | 4.950% | 2.693% | 2.338% | 2.646% |

Gold's own share **halved** between 2016–2018 and 2022–2025 with nothing but the price
level moving, and EURUSD's roughly **doubled** over the same span — so a pooled column
that mixes an instrument covering 2010–2025 with one covering 2024–2025 is partly an
artefact of the calendar. **EURUSD overtook gold somewhere in the 2010s**: it was cheaper
than gold in 2010–2012 (0.747% against 0.937%) and is 1.8× more expensive now.

Fixed windows both vendors and both gold feeds cover, **2022-06-16 → 2025-09-22**
(second receipt):

| market | rule I | rule H | rule M |
|---|---:|---:|---:|
| xauusd (Vantage bars) | 7.502% | 1.813% | 0.691% |
| xauduka (Dukascopy bars) | 7.408% | 1.803% | 0.704% |
| eurduka | 14.695% | 3.400% | 1.287% |
| xagduka | 21.791% | 5.473% | 2.320% |

Two independent vendors on the same instrument agree to within **1.3%** at all three
horizons, and EURUSD's two feeds agree to within **0.8%** on the shorter window they
share. The measurement is not a vendor artefact.

On the auto common era every surviving instrument covers, **2024-09-12 → 2025-09-22**
(the one BTC forces), rule I: btc 1.322%, btcusd 4.390%, **xauusd 5.067%**, xauduka
4.971%, eurusd 13.356%, eurduka 13.466%, xagduka 17.976%.

## The horizon is worth 10×; the instrument is worth at most 1.15×

Gold from rule I to rule M is **7.502% → 0.691%, a factor of 10.9** (21× on the record's
own tighter intraday stop). The best instrument switch available among tradeable rows, at
a fixed horizon on the common era, is **5.067% → 4.390%, a factor of 1.15**. Picking the
wrong instrument costs **3.5×**. That asymmetry is the finding: holding longer is worth an
order of magnitude and switching instruments is worth rounding error, in the direction it
helps and a large multiple in the direction it hurts.

## What would overturn the BTCUSD.sc row, arithmetically

Its median rule-I stop on the common era is `17.05 / 0.04390 = 388.4` BTC points. For it
to reach gold's 5.067% its spread would have to be `0.05067 × 388.4 = 19.68` — the single
read would need to be **15% too low**. Against gold's own *measured* 0.220 (below) gold
sits at 3.981% and BTCUSD.sc would need **15.46**, i.e. the read **9% too high**. The
whole BTC advantage lives inside the error bar of a number measured once, on an instrument
whose spread the config says has never been checked over time. That is not a material
difference and it is not a basis for moving instruments.

## Spread provenance, which is comment-only — and the sensitivity

| market | charged | measured | provenance, verbatim from `config/default.toml` |
|---|---:|---:|---|
| xauusd / xauduka | 0.28 | **0.220** | "sampled XAUUSD.sc 2,315 times over 12.8 hours … p10 0.210, p50 0.220, p90 0.220, max 0.260 — the configured 0.28 is above every spread ever observed, and charges roughly 27% too much" |
| xagduka | 0.021 | 0.021 | "measured 2026-09-13 from 205k ticks over three days … p50/p90 $0.021 an ounce" |
| eurduka / eurusd | 0.00014 | 0.00014 | "measured 2026-09-13 from 119k ticks over three days … p50/p90 0.00014" |
| btcusd | 17.05 | **null** | "a SINGLE read of the spread"; "whose 17.05 has never been checked over time at all" |
| btc | 5.00 | **null** | "3.4x the 5.00 **assumed** for the Binance market" |

At gold's measured 0.220 the pooled figures fall to **I 5.894% / H 1.425% / M 0.543%**,
and on the common era gold's rule I becomes **3.981%** — below BTCUSD.sc's 4.390%, which
would make gold outright cheapest at rule I as well. **Nothing is changed in the config
to say so**: the amendment the config demands is still owed two things it does not have
(the 21:00 UTC rollover hour and a weekend), and repricing globally would make every
receipt in `docs/decisions/` unreproducible. The figure is reported beside the charged one
instead, which is what pre-commitment 1 asks for.

Note what `null` means in that table and what it does not: btc and btcusd have a
*configured* spread, so they have a ratio and they appear in the main table. What they do
not have is a *measured* spread, so their rescaled column is null — not the charged number
wearing a different label.

## Excluded, and why

- **`gold` (COMEX GC, spread 0.30).** Its only bar file in the sealed store is
  `GC-1m.parquet` with **0 rows**, and there is no `GC-15m.parquet` at all. In the full
  store that file holds 11,813 minutes — about eight days — so the series is a rolling
  window whose entire content is after the cutoff and the seal correctly emptied it. An
  empty bar file is empty, **not an instrument with no volatility**, so `gold` has no row
  rather than a row of zeros. Independently of the seal it could not be measured this way:
  `fd-store/src/resample.rs` documents that the gold feed carries no highs or lows, so
  every bucket built from it has `open == high == low == close` and its true range is flat
  by construction, not by a quiet market.
- Nothing else. Every other market in `config/default.toml` has a configured spread —
  the struct makes that mandatory — and non-empty 15m bars.
- The same seal emptied `XAUUSD-1m`, `BTCUSD-1m`, `BTCUSDT-1m` and `EURUSD-5m` for the
  same reason. They are not used here; the 15m series is the one every receipt in the
  record is quoted on.

## Defects found

1. **`TradingRules::default()` hands out COMEX gold's numbers — `spread: 0.3`,
   `contract_size: 100.0` (`crates/fd-backtest/src/engine.rs:78`).** This is the exact
   shape of the silver trap: any `TradingRules { .., ..TradingRules::default() }` that
   omits the spread silently charges gold's to whatever instrument it is measuring. All 15
   current sites that omit it are inside `#[cfg(test)]`, so **the defect is latent, not
   live**, and a test that means gold's numbers is entitled to them. It is still a default
   that reads as neutral and is not. A `Default` that produced a spread of NaN — so an
   unset spread yields no ratio rather than gold's — would have made yesterday's voided
   16 cells fail loudly instead of quietly. Not changed here: it would rewrite the
   expectations of a dozen tests and this task is a measurement.
2. **A spread's provenance exists only in comments.** Nothing in the codebase can
   distinguish a spread that was measured from 205,000 ticks from one that was assumed:
   `MarketTradingOverride` carries the number and the TOML comment carries whether to
   believe it. The cheapest row in this entire table rests on an assumption, and only a
   human reading prose can tell. A `spread_provenance` key, or a measured/assumed pair,
   would put it where a program can print it — as this binary now does, by
   **hand-transcription**, which is the wrong mechanism and is labelled as such in the
   source.
3. **Not a defect, and worth recording because it was the first thing checked:**
   `MarketTradingOverride::spread` has no `#[serde(default)]`, so a market block that
   omits a spread **fails to parse** rather than inheriting the top-level
   `[trading] spread = 0.3`. Yesterday's silver error therefore did not come from the
   config plumbing — it came from a literal in an agent's own code. Both facts are pinned
   by tests so neither can drift.

## What went wrong, and judgement calls another agent might have made differently

- **My rule I is looser than the intraday stop the record quotes.** The record's gold
  intraday figure is 14.0% at a **2.00-point** stop; `1.5 × ATR14` on the same
  instrument's 15m bars has a median of **3.73 points**, so my rule I reports 7.50% where
  the record reports 14.0%. Both are right about their own stop. Because `k` only
  rescales, every instrument in my rule I column would double together at a 2.00-point-
  equivalent stop and **no comparison changes** — but a reader comparing my gold number
  to the record's must know the stops differ. The record does not say what produced its
  2.00, and I did not go looking; a different agent might have reverse-engineered it and
  matched it exactly.
- **Rule M's session boundary is a choice.** UTC+3 puts the daily bucket on the metals/FX
  break; a plain UTC day would glue two half-sessions together. It is meaningless for BTC,
  which has no break, and I applied it anyway rather than branching per instrument —
  identical treatment mattered more than per-instrument realism. The two are within a few
  percent for the 24/5 instruments; I did not measure the difference for BTC.
- **I did not weight by tradeability.** `btc` is Binance spot with an assumed spread and
  no account behind it, and it sits at the top of the table. I left it in with its
  provenance stated rather than dropping it, because deleting the cheapest row is a
  judgement someone should be able to check.
- **Commission and swap are excluded.** Every market on this account configures both at
  zero (measured swap-free from 340 closed positions), so `spread / stop` is the whole
  round-trip cost *for this account* and would not be for another. An instrument whose
  swap is real would be understated here.
- **p10/p90 are of the ratio, not confidence intervals.** They describe how much the cost
  share varies across the tape — silver's p90 of 61% at rule I means the quiet decile of
  silver's tape is close to untradeable at an intraday stop — and nothing about sampling
  error.
- **I ran nothing.** No strategy, no trade, no backtest, no percentile, and nothing was
  ranked by profitability. One thing noticed and deliberately not pursued: silver's cost
  share swings more than 3× between eras (1.65% → 4.95% → 2.34% at rule M) while gold's
  moves by well under 2×, which is a statement about silver's volatility regimes and
  might interest someone measuring a regime; **I did not look into it.**
