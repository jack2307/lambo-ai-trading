/**
 * Checks `src/lib/timeframes.ts` against the series this broker actually
 * publishes.
 *
 * WHY THIS FILE EXISTS. Placing a fill on a candle looks like arithmetic:
 * `floor(t / barMs) * barMs`. That is wrong here in three separate ways, and
 * every one of them fails SILENTLY — a marker at a time the series does not
 * hold is one the chart discards, so the symptom is a fill missing from a
 * chart, not an error anybody sees. The three cases are pinned below.
 *
 * It imports the REAL module rather than a copy of it. A check that restates
 * the implementation passes whatever the implementation does, which is the one
 * thing a check must not do. Node runs the TypeScript directly.
 *
 *     node --experimental-strip-types scripts/timeframes.check.mjs
 */

import { TF_MS, barsBehindBy, snapToBar } from '../src/lib/timeframes.ts'

const H = 3_600_000
const arithmetic = (ms, barMs) => Math.floor(ms / barMs) * barMs
const iso = (ms) => (ms == null ? 'null' : new Date(ms).toISOString().replace('.000Z', 'Z'))

let failures = 0
const check = (name, got, want) => {
  const ok = got === want
  if (!ok) failures += 1
  console.log(`${ok ? 'ok  ' : 'FAIL'}  ${name}`)
  if (!ok) console.log(`        got ${iso(got)}  want ${iso(want)}`)
}

/*
 * A real H4 series for this broker: candles open at 21/01/05/09/13/17 UTC,
 * because the server runs UTC+3 and its trading day starts at 21:00Z. Then the
 * weekend, when gold does not trade at all, then Sunday's 21:00Z reopen.
 */
const bars = []
let t = Date.UTC(2026, 8, 17, 21, 0, 0)
for (let i = 0; i < 6; i += 1) {
  bars.push({ time: t, open: 0, high: 0, low: 0, close: 0 })
  t += 4 * H
}
bars.push({ time: Date.UTC(2026, 8, 20, 21, 0, 0), open: 0, high: 0, low: 0, close: 0 })
bars.push({ time: Date.UTC(2026, 8, 21, 1, 0, 0), open: 0, high: 0, low: 0, close: 0 })

console.log('-- the three cases arithmetic gets wrong --')

// 1. The ordinary case. Arithmetic lands on an epoch boundary — 12:00Z — which
// is not a candle in this series at all.
const fill = Date.UTC(2026, 8, 18, 13, 17, 4)
check('a 13:17 fill sits on the 13:00Z candle', snapToBar(fill, bars), Date.UTC(2026, 8, 18, 13, 0, 0))
console.log(`        arithmetic: ${iso(arithmetic(fill, 4 * H))} — not a candle on this chart`)

// 2. Three hours wrong. The broker's candle opened at 21:00Z; the epoch's
// opened at 20:00Z. In winter the anchor moves to 22:00Z and a hard-coded
// offset would be wrong again, in the other direction.
const late = Date.UTC(2026, 8, 17, 23, 30, 0)
check('a 23:30 fill sits on the 21:00Z candle', snapToBar(late, bars), Date.UTC(2026, 8, 17, 21, 0, 0))
console.log(`        arithmetic: ${iso(arithmetic(late, 4 * H))} — 3h off`)

// 3. A hole in the week. Arithmetic computes a Saturday bucket for a market
// that is shut, and the marker is dropped.
const weekend = Date.UTC(2026, 8, 19, 12, 0, 0)
check('a Saturday time falls back to Friday 17:00Z', snapToBar(weekend, bars), Date.UTC(2026, 8, 18, 17, 0, 0))
console.log(`        arithmetic: ${iso(arithmetic(weekend, 4 * H))} — no such candle`)

console.log('\n-- the edges --')

// Older than the window is null and NOT the first bar: a marker parked on the
// left edge reads as "this happened here", and it did not.
check('older than the chart is null', snapToBar(Date.UTC(2026, 8, 1), bars), null)
check('exactly on an open stays put', snapToBar(Date.UTC(2026, 8, 18, 5, 0, 0), bars), Date.UTC(2026, 8, 18, 5, 0, 0))
// After the last bar is the forming candle, and a fill that just happened does
// belong on it.
check('a just-now fill lands on the forming candle', snapToBar(Date.UTC(2026, 8, 21, 3, 0, 0), bars), Date.UTC(2026, 8, 21, 1, 0, 0))
check('an empty series places nothing', snapToBar(fill, []), null)

// Same code, no constant to change, on the other side of the daylight shift.
const winter = [
  { time: Date.UTC(2026, 11, 10, 22, 0, 0), open: 0, high: 0, low: 0, close: 0 },
  { time: Date.UTC(2026, 11, 11, 2, 0, 0), open: 0, high: 0, low: 0, close: 0 },
]
check('the winter 22:00Z anchor needs no change', snapToBar(Date.UTC(2026, 11, 11, 1, 59, 0), winter), Date.UTC(2026, 11, 10, 22, 0, 0))

console.log('\n-- how far behind an export is --')

const noon = Date.UTC(2026, 8, 21, 12, 0, 0) // 11h after the last bar's open
check('a current export is behind by nothing', barsBehindBy(bars, '4h', Date.UTC(2026, 8, 21, 2, 0, 0)), 0)
check('one whole candle late counts as one', barsBehindBy(bars, '4h', Date.UTC(2026, 8, 21, 6, 0, 0)), 1)
check('eleven hours late is two closed candles', barsBehindBy(bars, '4h', noon), 2)
check('an empty series is behind by nothing', barsBehindBy([], '4h', noon), 0)

console.log('\n-- every timeframe the API knows has a period --')

// `Desk.tsx` reads this to decide whether a book has gone quiet. Its own copy
// stopped at 1h, so a 4h book would have been called stale three and a half
// hours early, every bar. Nothing trades 4h today, which is exactly why nobody
// had seen it.
for (const tf of ['1m', '5m', '15m', '30m', '1h', '4h', '1d']) {
  check(`${tf} has a period`, Number.isFinite(TF_MS[tf]) && TF_MS[tf] > 0, true)
}

console.log(failures === 0 ? '\nall checks pass' : `\n${failures} FAILED`)
process.exit(failures === 0 ? 0 : 1)
