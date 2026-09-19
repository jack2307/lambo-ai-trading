/**
 * What this desk has measured about every construct the chart can draw.
 *
 * THE VERDICT TRAVELS WITH THE NAME. Every registered attempt to trade an
 * indicator or a level as a rule has been killed out of sample — 25
 * registrations closed with zero survivors by 2026-09-14, and the count kept
 * rising — so an indicator offered on a chart with nothing beside it is an
 * implicit recommendation. This file is the badge text, and it is data rather
 * than prose so that `scripts/verdicts.check.mjs` can read every number in
 * every `line` back against the document the row cites. When a record is
 * amended and a badge is not, the check fails; that coupling is the point.
 *
 * Nothing here is recomputed or re-rounded. The long form, with the full
 * tables and the places the documents disagree with each other, is
 * `docs/research/VERDICTS.md`.
 */

export interface Verdict {
  id: string
  kind: 'indicator' | 'strategy'
  status: 'killed' | 'open' | 'parked' | 'untested'
  line: string
  detail: string
  registration: string | null
}

/**
 * The twelve indicator ids, from `crates/fd-indicators/src/lib.rs`.
 *
 * ALL TWELVE RESOLVE TO A VERDICT AND NONE IS DELIBERATELY NULL — see
 * `DELIBERATE_NULLS` below, and the `sma` row, which resolves with status
 * `untested` rather than returning null because "we never asked" is itself
 * something the chart must say. A blank badge reads as approval.
 */
const INDICATORS: Verdict[] = [
  {
    id: 'sma',
    kind: 'indicator',
    status: 'untested',
    line: 'untested as a rule; only the SMA200 gate in rsi2-pullback',
    detail:
      'No registration in docs/hypotheses/ tests a simple moving average as a signal. SMA200 appears once, as the trend gate inside rsi2-pullback, whose own rows on Vantage gold 15m over 2025-09-13 to 2026-09-12 are 542 trades at PF 0.881 all day and 198 at PF 1.110 in New York hours — both inside their nulls. Receipt: docs/research/runs/2026-09-13-recent-year-screen/in-sample.txt.',
    registration: null,
  },
  {
    id: 'ema',
    kind: 'indicator',
    status: 'killed',
    line: 'as ema-cross: 787 trades, PF 1.033, 85th pct - closed',
    detail:
      'Binance BTCUSDT 15m, 70,080 bars over 2024-09 to 2026-09, four-fold walk-forward: 787 trades at PF 1.033, the 85th percentile of 300 random-entry runs through the identical pipeline — "inside the noise" (docs/decisions/2026-09-12-technical-baselines.md). On Vantage gold 15m over 2022-06 to 2026-09 the same method is 1026 trades at PF 0.768 and the 1st percentile (docs/decisions/2026-09-12-gold-intraday-batch-1.md). The one Asian-session row that passed a gate on the last year had a direction null of 49th.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'rsi',
    kind: 'indicator',
    status: 'killed',
    line: 'as rsi-reversion: 1247 trades, PF 1.011, 78th - closed',
    detail:
      'Two rules were built on it and both closed. rsi-reversion on Binance BTCUSDT 15m over 2024-09 to 2026-09: 1247 trades, PF 1.011, 78th percentile of the null (docs/decisions/2026-09-12-technical-baselines.md); on Vantage gold 15m, 1269 trades at PF 0.890 and the 38th (docs/decisions/2026-09-12-gold-intraday-batch-1.md). Connors’ rsi2-pullback on the last twelve months at Vantage reached 198 trades at PF 1.110, expectancy 0.039R, 74th.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'macd',
    kind: 'indicator',
    status: 'killed',
    line: 'as macd-cross: 923 trades, PF 1.010, 66th pct - closed',
    detail:
      'Vantage gold 15m, 2025-09-13 to 2026-09-12, four folds: 923 trades at PF 1.010 and expectancy 0.011R, the 66th percentile of its hours-matched null. The Asian-session row looked like the screen’s best find at 300 trades and PF 1.556 (98th), and its direction null on the defaults is 93rd, below the declared 95th; on the 2022-25 context window the same direction null is 92nd at PF 0.908. Record: docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'bbands',
    kind: 'indicator',
    status: 'killed',
    line: 'as bb-fade: 2463 trades, PF 0.952, 44th pct - closed',
    detail:
      'Binance BTCUSDT 15m over two years: 2463 trades, PF 0.952, 44th percentile — the worst of the four baselines (docs/decisions/2026-09-12-technical-baselines.md). Vantage gold 15m: 3352 trades at PF 0.889, 36th (docs/decisions/2026-09-12-gold-intraday-batch-1.md). The squeeze variant on the last year is 160 trades at PF 0.849 and the 14th.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'atr',
    kind: 'indicator',
    status: 'killed',
    line: 'as a regime gate: PF 0.950-1.004, 70th-84th pct - closed',
    detail:
      'ATR is never a signal on this desk; it is the risk unit, which is not a claim. It was registered once as a condition — that a range break pays only where ATR14/close on 5m bars is at or above 0.075% — on Dukascopy gold over 2022-06-16 to 2025-04-10. The high-volatility rows came in at PF 0.950 and 1.004, the 70th to 84th percentile of nulls gated to the hours and the volatility condition, and the confirmation window was never opened. Record: docs/decisions/2026-09-13-volcond-breakout.md.',
    registration: 'docs/hypotheses/2026-09-13-volcond-breakout.md',
  },
  {
    id: 'vwap',
    kind: 'indicator',
    status: 'killed',
    line: 'as vwap-fade: 5828 trades, PF 0.951, 44th - closed',
    detail:
      'Binance BTC 5m, 2024-08 to 2026-09, where the volume defining the benchmark is real: 5828 trades at PF 0.951, the 44th percentile of random entries in the same hours; the US-hours row 1392 trades at PF 0.917 and the 24th. The direction null puts the fade at the 13th to 15th percentile, so if anything a stretch continues. The Vantage confirmation was never opened. Record: docs/decisions/2026-09-13-vwap-fade.md.',
    registration: 'docs/hypotheses/2026-09-13-vwap-fade.md',
  },
  {
    id: 'stoch',
    kind: 'indicator',
    status: 'killed',
    line: 'as stoch-reversal: 1297 trades, PF 0.912, 34th - closed',
    detail:
      'One of five mechanisms added for the recent-year screen and run on Vantage gold 15m over 2025-09-13 to 2026-09-12: 1297 trades all day at PF 0.912 and expectancy -0.044R, the 34th percentile; 507 trades in New York hours at PF 0.923, the 40th. Receipt: docs/research/runs/2026-09-13-recent-year-screen/in-sample.txt.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'adx',
    kind: 'indicator',
    status: 'killed',
    line: 'only as a bias label: best of 34 tests is noise-sized',
    detail:
      'No entry rule was ever built on ADX. It was measured as a market-bias label (adx20 +DI, adx25 +DI) among seventeen definitions on 25,708 hours of Vantage XAUUSD H1 and 6,728 of H4, against a circular-shift null on non-overlapping samples. Seventeen definitions by two horizons is 34 tests; the best cell in the whole grid reaches z = +1.98 against an expected largest of 34 standard normals of +1.94. Study: docs/decisions/2026-09-18-market-bias-definitions.md.',
    registration: null,
  },
  {
    id: 'donchian',
    kind: 'indicator',
    status: 'killed',
    line: 'as donchian-breakout: 2088 trades, PF 0.972, 54th - closed',
    detail:
      'Binance BTCUSDT 15m over two years: 2088 trades, PF 0.972, 54th percentile of the null (docs/decisions/2026-09-12-technical-baselines.md). Vantage gold 15m: 3493 trades at PF 0.902 and the 42nd (docs/decisions/2026-09-12-gold-intraday-batch-1.md). As a bias label its channel is the most anchor-stable definition measured, at 1.2% of H4 labels differing, and its WITH-minus-AGAINST statistic collapsed to +0.003R.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'keltner',
    kind: 'indicator',
    status: 'killed',
    line: 'as keltner-break: 533 trades, PF 1.052, 78th pct - closed',
    detail:
      'Vantage gold 15m, 2025-09-13 to 2026-09-12, four folds, the best mechanism row of about 170 looks at that year: 533 trades at PF 1.052 and expectancy 0.028R, the 78th percentile of its hours-matched null and short of both gate legs. The Asian row reached PF 1.315 on 262 trades (90th) with a direction null of 93rd, and 68th on the 2022-25 context window. Record: docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'swing',
    kind: 'indicator',
    status: 'open',
    line: 'structure as a rule is live and unjudged: htf-filter book',
    detail:
      'Confirmed fractal swings are the fractal(k) family of the bias study, which measured them as labels and licensed no rule: fractal(2) on H4 is the best of its 34 tests at z = +1.98 against an expected largest-of-34 of +1.94, and it labels 18.5% of H4 bars differently between anchors (docs/decisions/2026-09-18-market-bias-definitions.md). As a rule it is registered and RUNNING: the ai-xau-ds-ctx-htf-filter book began at 2026-09-18T04:07:34Z. No verdict may be implied for it, in either direction, until that book is decided.',
    registration: 'docs/hypotheses/2026-09-18-htf-context.md',
  },
]

/**
 * The strategy ids, from `crates/fd-strategy/src/builtin.rs` and `screen.rs`.
 *
 * Six of the twenty-seven have never been registered as a claim, and they say
 * so rather than staying silent — four of them because the options tape is
 * still too short to ask, two because they are machinery rather than claims.
 */
const STRATEGIES: Verdict[] = [
  {
    id: 'ema-cross',
    kind: 'strategy',
    status: 'killed',
    line: 'BTC 787 trades PF 1.033 (85th); gold 1026, 0.768 (1st)',
    detail:
      'Binance BTCUSDT 15m, 70,080 bars, four-fold walk-forward against 300 random-entry runs through the identical pipeline: PF 1.033 at the 85th percentile, and at zero cost the best of the four baselines only reaches 1.051, so there is no gross edge to rescue. On Vantage gold 15m with swap corrected to zero the same method is 1026 trades at PF 0.768, expectancy -0.132R, the 1st percentile. Records: docs/decisions/2026-09-12-technical-baselines.md, docs/decisions/2026-09-12-gold-intraday-batch-1.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'rsi-reversion',
    kind: 'strategy',
    status: 'killed',
    line: 'BTC 1247 trades PF 1.011 (78th); gold 1269, 0.890 (38th)',
    detail:
      'Fade an oversold or overbought RSI and exit at the midline. Binance BTCUSDT 15m over 2024-09 to 2026-09: 1247 trades, PF 1.011, 78th percentile. Vantage gold 15m over 2022-06 to 2026-09 with swap at zero: 1269 trades, PF 0.890, 38th; gating it to Asia or to a compression regime moved nothing. Records: docs/decisions/2026-09-12-technical-baselines.md, docs/decisions/2026-09-12-gold-intraday-batch-1.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'donchian-breakout',
    kind: 'strategy',
    status: 'killed',
    line: 'BTC 2088 trades PF 0.972 (54th); gold 3493, 0.902 (42nd)',
    detail:
      'A close outside the N-bar channel. Binance BTCUSDT 15m over two years: 2088 trades, PF 0.972, 54th percentile. Vantage gold 15m with swap at zero: 3493 trades, PF 0.902, 42nd. On the last twelve months at Vantage the all-day row is 519 trades at PF 0.979 and the 58th. Records: docs/decisions/2026-09-12-technical-baselines.md, docs/decisions/2026-09-12-gold-intraday-batch-1.md, docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'bb-fade',
    kind: 'strategy',
    status: 'killed',
    line: 'BTC 2463 trades PF 0.952 (44th); gold 3352, 0.889 (36th)',
    detail:
      'Fade a close outside the bands back to the middle band. Binance BTCUSDT 15m over two years: 2463 trades, PF 0.952, 44th percentile, the worst of the four baselines. Vantage gold 15m with swap at zero: 3352 trades, PF 0.889, 36th. On the 2022-25 context window the Asian row is 100th on its direction null while still losing, which is the closed level-fade shape: the side is right and the exits lose it. Records: docs/decisions/2026-09-12-technical-baselines.md, docs/decisions/2026-09-12-gold-intraday-batch-1.md, docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'keltner-break',
    kind: 'strategy',
    status: 'killed',
    line: 'tested as a rule: 533 trades, PF 1.052, 78th pct - closed',
    detail:
      'Vantage gold 15m, 2025-09-13 to 2026-09-12, four folds, nulls gated to the row’s hours: 533 trades at PF 1.052 and expectancy 0.028R, the 78th percentile. It was the best mechanism row of about 170 looks at that year and it still fails both gate legs. Asia reached PF 1.315 on 262 trades at the 90th with a direction null of 93rd. Record: docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'macd-cross',
    kind: 'strategy',
    status: 'killed',
    line: '923 trades, PF 1.010, 66th; asia 1.556 but direction 93rd',
    detail:
      'Vantage gold 15m over the last twelve months: 923 trades at PF 1.010, expectancy 0.011R, the 66th percentile. The Asian row passed the gate and the matched null at PF 1.556 on 300 trades (98th) and then failed the direction null at the 93rd, p = 0.074; on the 2022-25 context window that direction null is 92nd at PF 0.908. Risk did not veto it at 0.5% with guards on, and it fails the gate on its default cell. Record: docs/decisions/2026-09-13-recent-year-screen.md.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'rsi2-pullback',
    kind: 'strategy',
    status: 'killed',
    line: 'best row 198 trades, PF 1.110, 74th pct - closed',
    detail:
      'Connors’ rule: in a trend measured by close against SMA200, an extreme two-bar RSI is bought or sold back toward the trend. Vantage gold 15m, 2025-09-13 to 2026-09-12: 198 trades in New York hours at PF 1.110 and expectancy 0.039R, the 74th percentile; 542 trades all day at PF 0.881, the 25th. Receipt: docs/research/runs/2026-09-13-recent-year-screen/in-sample.txt.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'squeeze-break',
    kind: 'strategy',
    status: 'killed',
    line: '160 trades, PF 0.849, 14th pct of its null - closed',
    detail:
      'Bollinger width at a lookback low, then a close outside the bands. Vantage gold 15m over the last twelve months: 160 trades all day at PF 0.849 and expectancy -0.088R, the 14th percentile; the New York row could not reach the 30-trade floor at 27. On the 2022-25 context window the London row is 161 trades at PF 0.961, the 76th. Receipt: docs/research/runs/2026-09-13-recent-year-screen/in-sample.txt.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'stoch-reversal',
    kind: 'strategy',
    status: 'killed',
    line: '1297 trades, PF 0.912, 34th pct of its null - closed',
    detail:
      'Stochastic %K crossing %D from an extreme. Vantage gold 15m, 2025-09-13 to 2026-09-12, four folds: 1297 trades all day at PF 0.912 and expectancy -0.044R, the 34th percentile; 507 trades in New York hours at PF 0.923, the 40th. Receipt: docs/research/runs/2026-09-13-recent-year-screen/in-sample.txt.',
    registration: 'docs/hypotheses/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'orb',
    kind: 'strategy',
    status: 'killed',
    line: 'out of sample PF 0.95-1.12, direction 34th and 54th',
    detail:
      'The opening-range break carried five registrations — New York, London, BTC at the equity open, the volatility-conditional variant, and the 15-minute check — and the shape repeated on every one. New York: two of three variants passed in sample on Vantage 5m, then PF 0.95 to 1.12 on four years of Dukascopy 5m, with the direction null at the 34th in sample and the 54th out. London: the cleanest in-sample pass the project has produced, 257 trades at PF 1.317 and the 98th with direction 96th, then 525 trades at PF 0.953 and the 77th with direction 46th. Records: docs/decisions/2026-09-13-orb-ny.md, docs/decisions/2026-09-13-london-range.md, docs/decisions/2026-09-13-m15-check.md.',
    registration: 'docs/hypotheses/2026-09-13-orb-ny.md',
  },
  {
    id: 'pdhl',
    kind: 'strategy',
    status: 'killed',
    line: '147-555 trades, PF 0.645-0.758, 1st-6th pct - closed',
    detail:
      'Yesterday’s high and low, faded in the thin Asian hours and broken in the New York morning. Dukascopy gold 5m, 2022-06-16 to 2025-04-10, nulls gated to each row’s session: fade-asia 147 trades at PF 0.645 (1%), fade-allday 555 at PF 0.739 (5%), break-nyam 172 at PF 0.758 (6%). Direction nulls 61st, 47th and 82nd. Closed without opening the confirmation window. Record: docs/decisions/2026-09-13-pdhl.md.',
    registration: 'docs/hypotheses/2026-09-13-pdhl.md',
  },
  {
    id: 'ict-sweep-mss-fvg',
    kind: 'strategy',
    status: 'killed',
    line: 'out of sample PF 0.75-0.77 on 613-1428 trades - closed',
    detail:
      'The expert’s A+ chain: higher-timeframe fair-value gap, liquidity sweep, market-structure shift with displacement, retrace entry. It passed the gate and beat its matched null on the only three months of broker M1 available, then lost on four years of Dukascopy minutes at PF 0.75 to 0.77 and expectancy -0.17 to -0.19R over 613 to 1428 trades. The in-sample direction null had already put the side at the 79th percentile. Record: docs/decisions/2026-09-13-ict-sweep-mss-fvg.md.',
    registration: 'docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md',
  },
  {
    id: 'vwap-fade',
    kind: 'strategy',
    status: 'killed',
    line: '5828 trades, PF 0.951, 44th; direction 13th - closed',
    detail:
      'Binance BTC 5m, 2024-08 to 2026-09: 5828 trades at PF 0.951 and the 44th percentile all day, 1392 at PF 0.917 and the 24th in US hours. The direction null sits at the 13th to 15th percentile, so the flipped side would have been at the 85th to 87th and still inside — a stretch continues more often than it reverts, but not reliably enough to trade either way. Confirmation not opened. Record: docs/decisions/2026-09-13-vwap-fade.md.',
    registration: 'docs/hypotheses/2026-09-13-vwap-fade.md',
  },
  {
    id: 'doji-reversal',
    kind: 'strategy',
    status: 'killed',
    line: 'strict row 61 trades PF 1.203 -> 90th once matched',
    detail:
      'Three registrations, four windows. On Dukascopy gold 15m over 2022-06 to 2025-04 the strict row passed at 61 trades and PF 1.203 with a direction null of 99th — against a null matched to the hours but not to the trade count; matched to the count the same row is at the 90th and inside. The registered parameters then replay at PF 0.782 on 25 trades in 2025-26 and PF 0.618 on 121 in the never-seen 2018-22 window, and BTC fails outright at PF 0.79 and 0.81. Record: docs/decisions/2026-09-13-doji.md.',
    registration: 'docs/hypotheses/2026-09-13-doji.md',
  },
  {
    id: 'gap-fade',
    kind: 'strategy',
    status: 'killed',
    line: '207 gaps, PF 0.423, 1st pct of its null - closed',
    detail:
      'Fade the Sunday-reopen gap back toward the Friday close within the engine’s four-hour maximum hold. Dukascopy gold 5m bounded 2018-06-16 to 2025-04-10, 481,044 bars, entries and null both gated to 17:00-19:00 New York: the registered preset replays at PF 0.423 on 207 gaps, the 1st percentile; the large-gap row 0.768 on 109, the 72nd. Direction nulls 33rd and 66th. Record: docs/decisions/2026-09-13-gap-fade.md.',
    registration: 'docs/hypotheses/2026-09-13-gap-fade.md',
  },
  {
    id: 'trend-pullback',
    kind: 'strategy',
    status: 'killed',
    line: 'gold 5766 trades PF 0.379, 0th pct; BTC 2931, 0.511',
    detail:
      'Direction from a slow EMA, timing from the first close back through a fast one. Dukascopy gold 15m over 2018-06 to 2025-04: 5766 trades at PF 0.379, the 0th percentile, direction 58th. Binance BTC 15m over 2024-09 to 2026-09: 2931 trades at PF 0.511, also the 0th; its direction null reads 97th to 99th, which says the chosen side beats its mirror while still losing 0.38R a time either way. Record: docs/decisions/2026-09-13-trend-pullback.md.',
    registration: 'docs/hypotheses/2026-09-13-trend-pullback.md',
  },
  {
    id: 'volume-thrust',
    kind: 'strategy',
    status: 'killed',
    line: 'BTC 840 thrusts PF 0.960 (50th); gold 297, 0.931 (78th)',
    detail:
      'The registry’s only rule that uses volume as the signal rather than as a filter. Binance BTC 15m with real volume over 2024-09-12 to 2025-12-31: 840 thrusts at PF 0.960, the 50th percentile, direction 30th. Vantage gold 15m tick volume over 2022-06 to 2025-04: 297 thrusts at PF 0.931, the 78th, direction 78th. Neither confirmation was opened. Record: docs/decisions/2026-09-13-volume-thrust.md.',
    registration: 'docs/hypotheses/2026-09-13-volume-thrust.md',
  },
  {
    id: 'volman-box',
    kind: 'strategy',
    status: 'killed',
    line: '3460 trades, PF 0.639; in R the side is 38th - closed',
    detail:
      'Volman’s measured move on Dukascopy gold 5m over 2010-06 to 2018-06: the box-2 row is 3460 trades at PF 0.639, the 8th percentile of random entries with the same exits. Its receipt read 1st of its own permuted sides in compounded dollars, and re-implemented equal-weighted in R it is the 38th — 47% of that profit factor was the 2010 stub of an equity curve that fell from $10,000 to $84. The box-3 row is 3309 trades at PF 0.706, the 43rd and 12th. Record: docs/decisions/2026-09-14-volman-box.md.',
    registration: 'docs/hypotheses/2026-09-14-volman-box.md',
  },
  {
    id: 'tsmom',
    kind: 'strategy',
    status: 'killed',
    line: 'gold 60d PF 2.031 on 75, permuted sides 86th - closed',
    detail:
      'Three assets, one shape. Gold on Dukascopy 15m over 2018-06 to 2025-04: the 60-day row replays at PF 2.031 on 75 trades and the 98th percentile of the sized random-hold null, then 86th of its own permuted sides, with three of those 75 trades carrying all the profit. Silver’s 20-day row passes 2010-18 at 100th/98th and is a coin flip at 63rd/62nd on 2018-26; EURUSD’s 60-day row is 85th on direction with one 356-day trade at 71% of net. The first registration, docs/hypotheses/2026-09-13-tsmom.md, was parked rather than closed because the engine sized a multi-week hold on a five-minute ATR. Records: docs/decisions/2026-09-13-tsmom-2.md, docs/decisions/2026-09-13-tsmom-silver.md, docs/decisions/2026-09-14-tsmom-eurusd.md.',
    registration: 'docs/hypotheses/2026-09-13-tsmom-2.md',
  },
  {
    id: 'session-hold',
    kind: 'strategy',
    status: 'killed',
    line: 'weekday drift 96th-100th, under the 0.05R gate - closed',
    detail:
      'Five registrations of a clock. Gold long across the New York close on Dukascopy 2018-06 to 2025-04: 1355 holds at PF 1.254 and the 100th percentile of both nulls, with expectancy 0.006R against a gate of 0.05R — the drift is real and worth about the spread. The weekend leg passed seven years at 336 holds and PF 2.064, then failed the eight years before anyone had looked: 400 holds, PF 0.983, net -$73, 91st and 92nd, negative in every year from 2014 to 2018. Records: docs/decisions/2026-09-13-close-reopen-drift.md, docs/decisions/2026-09-13-friday-weekend-hold.md.',
    registration: 'docs/hypotheses/2026-09-13-close-reopen-drift.md',
  },
  {
    id: 'intraday-momentum',
    kind: 'strategy',
    status: 'killed',
    line: '2022 sessions, PF 0.606, 35th; own sides 40th - closed',
    detail:
      'Gao, Han, Li and Zhou’s market intraday momentum, asked of gold on Dukascopy over 2010-06 to 2018-06: the first-half-hour row is PF 0.606 on 2022 sessions, the 35th percentile of the sized random-hold null and the 40th of its own sides permuted, and the target window itself loses the spread. The day-so-far spelling sits at the 1st percentile of its own sides. The confirmation window was not opened. Record: docs/decisions/2026-09-14-intraday-momentum.md.',
    registration: 'docs/hypotheses/2026-09-14-intraday-momentum.md',
  },
  {
    id: 'level-reversion',
    kind: 'strategy',
    status: 'untested',
    line: 'never run: the options tape covered 114 of 70080 bars',
    detail:
      'One of four options-flow methods that have never been tested. At the time of the baselines the tape covered 114 of 70,080 bars, 0.16%, and all four returned zero out-of-sample trades: "Not rejected — unasked" (docs/decisions/2026-09-12-technical-baselines.md). Note the record disagrees with itself here — docs/hypotheses/2026-09-17-otl-context.md asserts that twenty-five registrations tested levels of exactly this kind as mechanical rules; see docs/research/VERDICTS.md section 4.',
    registration: null,
  },
  {
    id: 'maxpain-magnet',
    kind: 'strategy',
    status: 'untested',
    line: 'never run: the options tape covered 114 of 70080 bars',
    detail:
      'One of four options-flow methods that have never been tested. The tape covered 114 of 70,080 bars, 0.16%, and all four returned zero out-of-sample trades: "Not rejected — unasked" (docs/decisions/2026-09-12-technical-baselines.md). The same contradiction applies as for level-reversion; see docs/research/VERDICTS.md section 4.',
    registration: null,
  },
  {
    id: 'flow-momentum',
    kind: 'strategy',
    status: 'untested',
    line: 'never run: the options tape covered 114 of 70080 bars',
    detail:
      'One of four options-flow methods that have never been tested. The tape covered 114 of 70,080 bars, 0.16%, and all four returned zero out-of-sample trades: "Not rejected — unasked" (docs/decisions/2026-09-12-technical-baselines.md). The same contradiction applies as for level-reversion; see docs/research/VERDICTS.md section 4.',
    registration: null,
  },
  {
    id: 'flow-at-level',
    kind: 'strategy',
    status: 'untested',
    line: 'never run: the options tape covered 114 of 70080 bars',
    detail:
      'The founding thesis of this project and the one thing it has never been able to ask. The tape covered 114 of 70,080 bars, 0.16%, and all four options-flow methods returned zero out-of-sample trades (docs/decisions/2026-09-12-technical-baselines.md); a night of the loop later it was still "waiting on a tape long enough to test" (docs/decisions/2026-09-14-night-synthesis.md). Untested is not a recommendation.',
    registration: null,
  },
  {
    id: 'buy-and-hold',
    kind: 'strategy',
    status: 'untested',
    line: 'a control, not a claim - never registered as a method',
    detail:
      'The engine carries it so that other methods have something to be measured against; it is the only strategy allowed a warmup of zero. No file in docs/hypotheses/ states it as a claim, and the two places it is named are warnings that a long lookback makes a trend rule nearly indistinguishable from it in a rising sample.',
    registration: null,
  },
  {
    id: 'external',
    kind: 'strategy',
    status: 'untested',
    line: 'a harness for a foreign signal, not a claim of this desk',
    detail:
      'A shell that replays signals produced outside the engine. Whatever it is fed is the claim, and it carries whatever verdict that source earned; the id itself has never been registered and can never be killed or survive on its own.',
    registration: null,
  },
]

/** Every construct this desk has a record for, indicators first. */
export const VERDICTS: Verdict[] = [...INDICATORS, ...STRATEGIES]

/**
 * The one indicator id that deliberately resolves to `null`, and why.
 *
 * NONE. All twelve ids in `crates/fd-indicators/src/lib.rs` resolve to a
 * Verdict: `sma` and the four options-flow strategies carry status `untested`
 * rather than a null, because "never asked" is a fact the chart owes the
 * reader and a blank badge reads as approval. The null return is reserved for
 * an id this file has never heard of — a new indicator shipped without a row
 * here — and that case must render as "no record", never as silence. The
 * check script fails the build if any of the twelve stops resolving.
 */
export const DELIBERATE_NULLS: readonly string[] = []

const BY_ID = new Map(VERDICTS.map((v) => [v.id, v]))

/**
 * The verdict for an indicator or strategy id, or `null` if this file has no
 * row for it.
 *
 * A caller must render the null as an explicit "no record on this desk" and
 * never as an empty space: an indicator drawn with nothing beside it is the
 * implicit recommendation this whole file exists to prevent.
 */
export function verdictFor(id: string): Verdict | null {
  return BY_ID.get(id) ?? null
}
