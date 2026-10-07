# 2026-10-07 — the gate prints a risk number: `max_drawdown_usd` / `max_drawdown_pct` reach the receipt, and are checked before they are believed

**Registered:** committed ALONE, before the first line of code on this branch.
**Branch:** `agent/drawdown`, worktree `E:/rust/fd-drawdown`, cut from
`agent/instr-repair`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + `AGENT-BRIEF-ADDENDUM-5.md`
section G.
**Status:** registered.

## 0. What this axis is, and what it is not

Two parts, and they spend differently.

**Parts (1) and (2) are INSTRUMENT. They have NO GATE and spend 0 cells.**
No method is run to see whether it clears PF >= 1.200 / expectancy >= +0.050R /
>= 40 trades. Nothing here is a hypothesis about a market, and no percentile is
read. The deliverable is a column that exists and an arithmetic that has been
checked. Following the precedent of `2026-10-07-instrument-repair.md`: **added
beside, never in place of**, and proved bit-identical on everything already
published.

**Part (3) is a MEASUREMENT and it spends cells. Declared: 20.** They are the
rows the record itself already printed a positive verdict for at >= 40 trades,
named in §4 below before anything is run. The 20 are not chosen by me from a
sweep — they are chosen by the record — and the full batch each one sits in is
re-run because that is the only way to reproduce the row, while only the 20
declared rows are read.

## 1. Hypothesis, one sentence

Every profit figure in this record was printed without a risk figure beside it,
because `max_drawdown_usd` / `max_drawdown_pct` exist in
`engine.rs::Metrics` and `--mode=hypotheses` never printed them; printing them
will show that at least one row the record calls a survivor carries a drawdown
a real account could not hold.

## 2. Falsifier, specific and firable

**F1 (on the instrument, part 2).** `max_drawdown_*` has never been printed by
the mode that produced this record's ~9,858 rows, so it has never been read, so
it has never been checked. If a hand-built trade sequence with a drawdown known
by arithmetic disagrees with what `metrics_of` reports, **the quantity is wrong
and the finding of this job is that defect** — and the number is NOT then
printed across the record as if it were a measurement. Three sequences, written
before the code: a known drawdown, an all-winning sequence (drawdown must be
**0.00 USD**, and `pct` must be **0**, not `NaN`, because a curve that never
fell did measure a fall of zero), and a single trade.

**F2 (on the measurement, part 3).** If every one of the 20 declared rows comes
back with a drawdown a $10,000 account holds without flinching, then the claim
in §1 is false: the record's profit numbers were missing a risk number but no
decision would have changed, and that is the result to report.

**F3 (anti-smuggling).** If any figure already published moves — trades,
profit factor, expectancy, `expectancy_net`, null percentile, or any golden
parity field — the patch is not additive and is wrong regardless of how good
the new column looks. Checked by `to_bits()` equality, not by eye, exactly as
`instr-repair` checked `r_net == r` on a costless row.

## 3. What the quantity MEANS, to be stated in the receipt

`metrics_of` walks the trades in order, `equity += trade.pnl_usd`, and tracks
`peak - equity`. So it is a drawdown on the **CLOSED-TRADE equity curve**: the
worst peak-to-trough fall measured only at trade closes. It is **not** the
intrabar excursion — a position that went 3R against the book and came back to
close green contributes **nothing** to it, and `avg_mae` is the only field that
sees that. A receipt that prints the number must say which of the two it is, or
the reader will assume the worse one. The denominator of `pct` is to be stated
too, and checked against the oracle's golden files, which carry
`maxDrawdownPct` that no test has ever compared.

## 4. The 20 declared cells, named before the run

| # | row | source receipt | trades on record |
|---|---|---|---|
| 1 | `close/fri-1630-1815` | `2026-09-13-close-reopen-drift/in-sample-fixed.txt` | 336 |
| 2 | `ict-B-balanced` | `2026-09-13-ict-sweep-mss-fvg/in-sample.txt` | 62 |
| 3 | `ict-B-allday` | same | 157 |
| 4 | `london/nyam` | `2026-09-13-london-range/in-sample.txt` | 257 |
| 5 | `london/early` | same | 203 |
| 6 | `orb/60m` | `2026-09-13-orb-ny/in-sample.txt` | 84 |
| 7 | `orb/60m-expansion` | same | 48 |
| 8 | `hold/04-06-short` | `2026-09-13-recent-year-hours/in-sample.txt` | 205 |
| 9 | `hold/16-18-long` | same | 699 |
| 10 | `hold/18-20-long` | same | 164 |
| 11 | `hold/20-22-long` | same | 463 |
| 12 | `macd-cross/asia` | `2026-09-13-recent-year-sessions/in-sample.txt` | 300 |
| 13 | `keltner-break/asia` | same | 262 |
| 14 | `ict-sweep-mss-fvg/asia` | same | 81 |
| 15 | `tsmom2/60d` | `2026-09-13-tsmom-2/in-sample-fixed.txt` | 75 |
| 16 | `tsmom2/20d` | same | 149 |
| 17 | `tsmom/20d` | `2026-09-13-tsmom/in-sample.txt` | 189 |
| 18 | `tsmom/60d` | same | 108 |
| 19 | `struct-80` | `2026-09-23-designed-1-cost-term/xauusd-guarded.txt` | 175 |
| 20 | `struct-80-f14` | same | 152 |

`ema-cross/asia` (PF 1.632, 99th percentile) is **excluded on purpose**: 37
trades is below the desk's 40 and the brief's ~40 floor, and the tool's own
`need 30` printed it as a pass. It will be shown in the table marked
"under the floor, not a candidate".

## 5. How to read the result

- A drawdown next to a profit factor, nothing more. **There is no drawdown
  gate** on this branch and none is proposed: the desk has not set one, and
  inventing a threshold here would be a new gate smuggled in under an
  instrument repair.
- `pct` is of the account the config sizes against (`starting_equity_usd`), so
  it answers "what fraction of the book" and not "what fraction of the risk
  taken".
- Addendum-5 §D still binds: any long-horizon row that only exists in the
  no-guards arm is not a tradeable candidate, and its drawdown is a reading of
  an arm the owner has forbidden. The table says so per row.
- The walk-forward rows concatenate out-of-sample folds, and **each fold's
  engine run restarts at `starting_equity_usd`** while `metrics_of` then walks
  the concatenation as one curve. The USD drawdown is therefore the drawdown of
  a book that was re-sized at each fold boundary, which is the only curve this
  record has. Stated, not hidden.

## 6. Note added 2026-10-07, after the run

See the RESULT section appended at the bottom of this file. Nothing above is
rewritten.
