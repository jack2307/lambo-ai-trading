# Ten candidates for ten accounts

**Date:** 2026-09-14. The owner will run up to ten independent accounts,
each with one strategy. The research loop has closed 26 registrations
without a survivor, so "the best strategy" cannot mean the best backtest:
every good backtest here was one window that failed the next. The
criteria below are the ones the receipts can actually rank on.

## Criteria

1. **Expectancy near zero after the spread, not deeply negative.** Rows
   whose direction null sits mid-distribution and whose profit factor is
   near one on the long window. Breakouts at PF 0.6 are out.
2. **Risk that the guards can define.** A stop or a clock exit, no
   weekend hold, no hold through a release: the four guards apply to
   every run and the strategy must still make sense with them on.
3. **One to three trades a day.** Enough to be a bot, not enough to be a
   spread machine.
4. **Different mechanisms and hours across the ten**, so the ten books
   do not lose on the same day.
5. **The demo track record decides the order**, not this list: all ten
   run on paper first; the three steadiest go to demo terminals; then
   the rest.

Every number is from the record named; every candidate is a closed
row, and the expectation for each is approximately zero minus cost.

## The list

| # | run id | market:tf | strategy · filters | why it is here | what the receipts say |
|---|---|---|---|---|---|
| 1 | `xau-ema` | xauusd:15m | `ema-cross` · weekdays, news:60-30 | the baseline; trend, all day; running since 14/09 | 2022–26 inside the noise (`2026-09-12-vantage-bars-baselines.md`) |
| 2 | `xau-macd-asia` | xauusd:15m | `macd-cross` · weekdays, hours:1800-0200, news:60-30 | the last year's best gridded row: PF 1.56 on 300 trades, 98th of random, 93rd direction | 3 years PF 0.91 (`2026-09-13-recent-year-screen.md`); risk allowed paper at 0.5% |
| 3 | `xau-keltner-asia` | xauusd:15m | `keltner-break` · weekdays, hours:1800-0200, news:60-30 | Asian breakout, 90th/93rd on the year | 3 years 0.88; a closed family, run for the contrast with #2 |
| 4 | `xau-close` | xauusd:15m | `session-hold` from=1615 to=1815 side=1 riskDailyRanges=1 · weekdays:MoTuWeTh | the only sign at the 96th–100th on both windows; worth the spread | net $132/yr at 1% on 2018–25 (`2026-09-13-close-reopen-drift.md`); the news flat cannot touch it, the weekend flat is irrelevant |
| 5 | `xau-evening` | xauusd:15m | `session-hold` from=1800 to=2000 side=1 riskDailyRanges=1 · weekdays:MoTuWeTh | the one row of ~170 that passed the year screen (95th/95th) | 3 years 1.14, 100th; noise by the screen's own count |
| 6 | `xau-rsi2-ny` | xauusd:15m | `rsi2-pullback` · weekdays, hours:0800-1600, news:60-30 | mean reversion in New York hours; PF 1.11 on 198 trades, 74th | the contrast to the trend rows; 3 years inside the noise |
| 7 | `xau-stoch` | xauusd:15m | `stoch-reversal` · weekdays, news:60-30 | the Workbench leaderboard's top on the last week (PF 1.48, 35 trades — in-sample only) | nothing out of sample; here to show what an in-sample leader does live |
| 8 | `xau-ict-5m` | xauusd:5m | `ict-sweep-mss-fvg` · weekdays, hours:0800-1600, news:60-30 | the long window's best 5m row: PF 1.14 on 288, 100th of random, 95th direction | closed family (`2026-09-13-ict-sweep-mss-fvg.md`); needs the 5m poller |
| 9 | `xau-box-5m` | xauusd:5m | `volman-box` boxes=2 maxBoxAtr=3 · weekdays, news:60-30 | the owner's method; 0.97 on the last Vantage year, no direction | `2026-09-14-volman-box.md`: a cost structure, not an edge — run so its live fills say the same |
| 10 | `eur-hours` | eurusd:15m | `session-hold` from=245 to=1045 side=-1 riskDailyRanges=1 · weekdays, news:480-30 | the euro's European hours; sign at the 99th on 2010–18, gone on 2018–26 | `2026-09-14-fx-local-hours-sign.md`; `news:480-30` skips a hold that spans a release; needs the EURUSD.sc poller |

Not on the list: `tsmom` (a multi-day hold that the weekend flat would
cut every Friday — the guard changes the method), every 2010–18-only
pass, and anything at PF < 0.9 on its long window.

## How they run

Each row is one `POST /api/paper/start` with `guards: true`; the bar
pollers feed every run on the same `market:tf` (`py/live/mt5_bars.py`
for `XAUUSD.sc` M15 and M5, and `EURUSD.sc` M15). The books live in
`data/paper/<run id>/`; the Overview shows all of them. After two weeks
the risk role and the news-desk read the fills, and the three steadiest
books go to demo terminals through `py/live/mt5_executor.py` with the
demo-only lock.
