/**
 * Number formatting.
 *
 * One place, because a dashboard that renders 4,356.3 in one panel and 4356.30
 * in the next makes a reader check whether they are the same number.
 */

const EM_DASH = '—'
/** A real minus sign, not a hyphen: it aligns with digits in tabular figures. */
const MINUS = '−'

export function price(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return EM_DASH
  return Math.abs(value) >= 1000
    ? value.toLocaleString('en-US', { maximumFractionDigits: 0 })
    : value.toFixed(1)
}

export function usd(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return EM_DASH
  const magnitude = Math.abs(value)
  const sign = value < 0 ? MINUS : ''
  if (magnitude >= 1e9) return `${sign}$${(magnitude / 1e9).toFixed(2)}B`
  if (magnitude >= 1e6) return `${sign}$${(magnitude / 1e6).toFixed(2)}M`
  if (magnitude >= 1e3) return `${sign}$${(magnitude / 1e3).toFixed(0)}K`
  return `${sign}$${magnitude.toFixed(0)}`
}

export function money(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return EM_DASH
  const sign = value < 0 ? MINUS : ''
  return `${sign}$${Math.abs(value).toLocaleString('en-US', { maximumFractionDigits: 0 })}`
}

export function num(value: number | null | undefined, digits = 2): string {
  if (value == null || !Number.isFinite(value)) return EM_DASH
  return Math.abs(value) >= 1000 ? value.toFixed(0) : value.toFixed(digits)
}

export function pct(value: number | null | undefined, digits = 0): string {
  if (value == null || !Number.isFinite(value)) return EM_DASH
  return `${(value * 100).toFixed(digits)}%`
}

export function clock(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return EM_DASH
  return new Date(ms).toISOString().slice(11, 16)
}

export function stamp(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return EM_DASH
  return `${new Date(ms).toISOString().replace('T', ' ').slice(0, 19)}Z`
}

export function day(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return EM_DASH
  return new Date(ms).toISOString().slice(0, 10)
}

/** Tailwind class for a value whose sign carries meaning. */
export function signClass(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value === 0) return ''
  return value > 0 ? 'text-bull' : 'text-bear'
}

export const FLOW_CLASS_COLOR: Record<string, string> = {
  LC: 'text-lc',
  SP: 'text-sp',
  LP: 'text-lp',
  SC: 'text-sc',
  UNKNOWN: 'text-muted-foreground',
}

/** What each flow class actually means, for tooltips and the legend. */
export const FLOW_CLASS_MEANING: Record<string, string> = {
  LC: 'call, buyer-aggressed',
  SP: 'put, seller-aggressed',
  LP: 'put, buyer-aggressed',
  SC: 'call, seller-aggressed',
}

/**
 * How long ago a live reading arrived, in seconds.
 *
 * Seconds and not minutes: this measures a feed that should move several times
 * a second, and "0 min" would say nothing about whether it is moving. Lives
 * here rather than in a page because the Desk and the status strip both show
 * it and they must not phrase it differently — the same instant reading two
 * ways on one screen is the defect this repo spent the week removing.
 */
export const liveAge = (at: number, now: number): string =>
  `${Math.max(0, Math.round((now - at) / 1000))} s ago`

/**
 * How long ago something happened, for things measured in BARS rather than in
 * ticks.
 *
 * `liveAge` above is seconds and must stay seconds: it measures a feed that
 * should move several times a second, where "0 m" would say nothing about
 * whether it is moving. That is exactly why it was the wrong function for a
 * four-hour bar — the higher-timeframe card was printing `as of 51404 s ago`,
 * which is a true number that no person reads as fourteen hours. One
 * formatter was doing two jobs and only one of them was its own.
 *
 * Returns a bare duration — `14 h 17 m` — and never the word "ago", so the
 * caller decides whether it is "closed 14 h ago", "as of 14 h ago" or
 * "confirmed 14 h ago". The one place that appended its own "ago" to
 * `liveAge` was printing "ago ago", which is what happens when a formatter's
 * output carries a word the call site cannot see.
 *
 * Two units at most, largest first, and the smaller one dropped when it is
 * zero: `2 h` rather than `2 h 0 m`. Precision below the second unit is noise
 * on something that only changes when a bar closes.
 */
export function since(at: number | null | undefined, now: number): string {
  if (at == null || !Number.isFinite(at)) return EM_DASH
  const seconds = Math.max(0, Math.round((now - at) / 1000))
  if (seconds < 60) return `${seconds} s`

  // ROUNDED to the smallest unit shown, not floored, because `liveAge` above
  // rounds and two ages on one screen that disagree about the same instant is
  // the defect this file exists to prevent. 51,404 s is 14 h 16 m 44 s and
  // reads as 14 h 17 m.
  //
  // Rounding a remainder can carry — 3,599 s rounds to 60 minutes — so each
  // branch normalises upward rather than printing "60 m" or "24 h", which
  // would be the arithmetic showing through the words.
  if (seconds < 3600) {
    const minutes = Math.round(seconds / 60)
    return minutes >= 60 ? '1 h' : `${minutes} m`
  }
  if (seconds < 86_400) {
    let hours = Math.floor(seconds / 3600)
    let minutes = Math.round((seconds % 3600) / 60)
    if (minutes === 60) {
      hours += 1
      minutes = 0
    }
    if (hours >= 24) return '1 d'
    return minutes ? `${hours} h ${minutes} m` : `${hours} h`
  }
  let days = Math.floor(seconds / 86_400)
  let hours = Math.round((seconds % 86_400) / 3600)
  if (hours === 24) {
    days += 1
    hours = 0
  }
  return hours ? `${days} d ${hours} h` : `${days} d`
}
