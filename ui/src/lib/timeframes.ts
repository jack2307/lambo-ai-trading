import type { Bar } from '@/lib/api'

/**
 * Which timeframes the desk chart offers, and how a time on one of them is
 * placed on another.
 *
 * THE ONE THING THIS FILE EXISTS TO PREVENT. A fill happens at an exact
 * millisecond — 13:17:04 — and the chart draws it on a candle. On the 15m
 * chart the fill's own bar is on the series and the marker lands. On the 4h
 * chart there is no candle at 13:17, and Lightweight Charts will drop or
 * misplace a marker whose time is not a time on the series. So every overlay
 * time must be moved to the START of the candle that CONTAINS it, at whatever
 * timeframe is on screen.
 *
 * THE OBVIOUS WAY TO DO THAT IS WRONG HERE. `floor(t / barMs) * barMs` anchors
 * the buckets to the Unix epoch — 00:00, 04:00, 08:00 UTC. This broker's H4
 * candles start at 21:00 UTC, because the server runs UTC+3 and its day starts
 * there, and the anchor moves to 22:00 outside US summer time (measured
 * 2026-09-18: hour 21Z holds exactly zero 15m bars against 332-348 in every
 * neighbouring hour — see the note on `HtfSourceDto` in `htf.rs`, where the
 * same fact is the reason that route refuses to resample). Arithmetic snapping
 * would put every marker three hours into the wrong candle, and be wrong by a
 * different amount in winter.
 *
 * Gaps make it worse in a way no anchor constant would fix. Gold does not
 * trade over the weekend. Arithmetic snapping happily computes a Saturday
 * bucket, and a marker at a time no candle occupies is a marker the chart
 * discards — an entry that silently vanishes from the record rather than an
 * error anybody sees.
 *
 * So the snap is a SEARCH over the bars the route actually returned: the last
 * bar whose time is at or before the marker's. That is correct for any anchor,
 * on both sides of a daylight shift, and across a weekend or a feed outage,
 * because it asks the series where its candles are instead of predicting them.
 */

export type Timeframe = '5m' | '15m' | '1h' | '4h' | '1d'

/** The ones the chart offers, in the order they are shown. */
export const DESK_TIMEFRAMES: Timeframe[] = ['5m', '15m', '1h', '4h', '1d']

/**
 * The timeframe the desk opens on.
 *
 * 15m is the timeframe the books actually trade, so it is the only one where
 * the chart, the tables and the run's own indicators are all reading the same
 * series. Every other choice is a view; this one is the record.
 */
export const DEFAULT_TIMEFRAME: Timeframe = '15m'

export function isTimeframe(value: unknown): value is Timeframe {
  return typeof value === 'string' && (DESK_TIMEFRAMES as string[]).includes(value)
}

/**
 * A timeframe's period in milliseconds.
 *
 * For LABELLING and for asking "is the newest bar old enough that it should
 * have closed by now" — NEVER for placing a time on a candle. Use `snapToBar`
 * for that, for every reason in the header. `1d` is 24h of wall clock, which
 * is what this broker's daily candles span; it is not a calendar day and the
 * two differ across a daylight shift.
 *
 * EVERY timeframe the API knows, not only the five the chart offers. `Desk.tsx`
 * carried its own copy that stopped at `1h`, and `staleAfter` reads it to
 * decide whether a book has gone quiet: a 4h book would have fallen back to a
 * flat 45-minute window and been called stale three and a half hours early,
 * every bar, forever. Nothing trades 4h today so nobody had seen it — which is
 * the same shape as every other bug this desk has found lately, a state that
 * exists and has never been entered. One map, so the next timeframe added is
 * added once.
 */
export const TF_MS: Record<string, number> = {
  '1m': 60_000,
  '5m': 5 * 60_000,
  '15m': 15 * 60_000,
  '30m': 30 * 60_000,
  '1h': 60 * 60_000,
  '4h': 4 * 60 * 60_000,
  '1d': 24 * 60 * 60_000,
}

/** `4h` as a person reads it out. Used in prose, not on the buttons. */
export const TF_WORD: Record<Timeframe, string> = {
  '5m': 'five-minute',
  '15m': 'fifteen-minute',
  '1h': 'hourly',
  '4h': 'four-hour',
  '1d': 'daily',
}

/**
 * The start of the candle containing `ms`, from the bars themselves.
 *
 * `bars` must be ascending by `time`, which is how every route on this API
 * returns them. Returns `null` when the time is before the first bar on the
 * chart — an older fill than the window shows — and the caller must then draw
 * nothing rather than clamp it onto the first candle, because a marker parked
 * on the left edge reads as "this happened here" and it did not.
 *
 * A time AFTER the last bar snaps to the last bar. That is the forming candle,
 * and a fill that just happened does belong on it.
 */
export function snapToBar(ms: number, bars: Bar[]): number | null {
  if (bars.length === 0 || !Number.isFinite(ms)) return null
  if (ms < bars[0].time) return null

  // Binary search for the last bar at or before `ms`. Linear would be fine at
  // a few hundred bars, but this runs once per marker per redraw and the
  // trades list is unbounded.
  let lo = 0
  let hi = bars.length - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (bars[mid].time <= ms) lo = mid
    else hi = mid - 1
  }
  return bars[lo].time
}

/**
 * Snap, and say whether the time fell off the front of the chart.
 *
 * The two callers want different things from a miss: markers drop silently
 * (there are many and the chart is a window), while the position overlay wants
 * to say "entered before this window" rather than draw nothing and look like
 * there is no position. Both need to be able to tell a miss from a hit, which
 * a bare `number | null` only just manages, so this spells it.
 */
export function placeOnChart(ms: number | null | undefined, bars: Bar[]): { time: number } | { before: true } | null {
  if (ms == null || !Number.isFinite(ms)) return null
  const time = snapToBar(ms, bars)
  if (time != null) return { time }
  return bars.length > 0 ? { before: true } : null
}

/**
 * Whether the newest bar in a series is one that has closed.
 *
 * Asked of the FILE-backed timeframes, where the bars are an export and the
 * newest one may be hours old. `at` is now. The answer is not "is the data
 * fresh" — an export from four hours ago is perfectly current on a 4h chart —
 * it is "has a candle finished that this series does not have", which is the
 * only staleness a person reading the chart is harmed by.
 */
export function barsBehindBy(bars: Bar[], tf: Timeframe, at: number): number {
  if (bars.length === 0) return 0
  const last = bars[bars.length - 1].time
  const period = TF_MS[tf]
  // The bar at `last` closes at `last + period`. Every whole period since then
  // is a candle that closed and is missing.
  const elapsed = at - (last + period)
  return elapsed <= 0 ? 0 : Math.floor(elapsed / period) + 1
}

const KEY = 'fd.desk.tf'

/**
 * The viewer's timeframe, remembered.
 *
 * Not per market and not per run. A person looks at the desk at the timeframe
 * they think in, and having the chart jump back to 15m because they clicked a
 * different book would be the desk forgetting something it was just told.
 */
export function readTimeframe(): Timeframe {
  try {
    const raw = localStorage.getItem(KEY)
    return isTimeframe(raw) ? raw : DEFAULT_TIMEFRAME
  } catch {
    return DEFAULT_TIMEFRAME
  }
}

export function writeTimeframe(tf: Timeframe) {
  try {
    localStorage.setItem(KEY, tf)
  } catch {
    /* private mode: the choice lasts the page */
  }
}
