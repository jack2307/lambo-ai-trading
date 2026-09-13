# 2026-09-13-recent-year-screen: every mechanism in the registry on the last twelve months at Vantage

**Registered:** (commit time is authoritative) — before any run
**Status:** decided → docs/decisions/2026-09-13-recent-year-screen.md (one row of ~170 passes, noise by the registration's own count; the long-window context shows an evening drift, registered next as 2026-09-13-close-reopen-drift)
**Batch files:** `docs/hypotheses/2026-09-13-recent-year-screen.toml` (15m), `2026-09-13-recent-year-screen-5m.toml` (5m), `2026-09-13-recent-year-hours.toml` (hour-of-day holds), `2026-09-13-recent-year-sessions.toml` / `-5m` (each mechanism per session: Asia 18:00–02:00, London 03:00–11:00 New York; added at the owner's request after the first receipts), `2026-09-13-recent-year-gap.toml` (the gap rows, amended)

## The criterion changed, and by whom

The owner's instruction (2026-09-13, 16:xx): *"Bắt đầu back test thêm nhiều
phương pháp, độ hiệu quả 1 năm gần nhất ổn là được"* — test many more
methods; good on the most recent year is enough. That is a different
criterion from the loop's standing rule (the long window is primary, the
2025–26 Vantage year is a confirmation and a known high-volatility regime),
and this file records the change so no later reader mistakes a pass here
for a pass under the earlier rule.

What this program keeps: the gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30
trades), the count-matched null gated to the row's hours, the direction
null, pre-registration, receipts. What it changes: the **primary window is
`xauusd` 2025-09-13 → 2026-09-12** (the broker's own bars, the last twelve
months), and the long window (`xauduka` 2022-06-16 → 2025-04-10) is run as
**context**, reported beside every survivor, not as a gate.

## Claim

Among the registry's mechanisms — the four indicator baselines, the ORB,
previous-day levels, VWAP fade, doji, trend pullback, gap fade, volume
thrust, the ICT chain, and five added for this screen (Keltner break, MACD
cross, RSI(2) pullback, squeeze break, stochastic reversal) — some pass the
gate and both nulls on the last twelve months of Vantage gold, on 15-minute
and on 5-minute bars, all day or in New York hours. Separately, some
hour-of-day hold (two-hour blocks, long or short) beats random holds of the
same length on the same year.

## Falsifier

Per row: gate and ≥ 95th percentile of the matched null (walk-forward, 4
folds) and direction null ≥ 95th on the primary year. A row that passes is
a **candidate**; the record lists it with its long-window number beside it.
A row that fails is closed for this program.

## Multiplicity

This is a screen: about 36 rows on 15m, 36 on 5m, 24 hour-holds, and 32
session rows per timeframe — some 170 looks at one year. At the 95th percentile, eight or nine would
pass by luck. The record will say how many passed and how many were expected by
chance, and a candidate list shorter than that expectation is noise.

## Data

- Primary: `xauusd:15m` and `xauusd:5m`, `2025-09-13 → 2026-09-12`.
- Context: `xauduka:15m` / `:5m`, `2022-06-16 → 2025-04-10`, the same
  batch, run after the primary receipts exist.
- Weekdays, flat over the daily break; a New York-hours variant of each row.

## What each outcome means

- Candidates exist and their long-window numbers are also above 1 → the
  best case; a paper run on the top two or three, sized as the risk role
  says.
- Candidates exist and the long-window numbers are below 1 → they are the
  year, not the method; the owner decides whether to paper-trade a regime
  bet knowing that.
- No candidates → the last year passes fewer things than the earlier
  records suggested, and the program is closed.
