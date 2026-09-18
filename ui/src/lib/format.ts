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
