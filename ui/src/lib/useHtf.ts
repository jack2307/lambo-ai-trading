import { useEffect, useState } from 'react'

import { api, type HtfResponse } from '@/lib/api'

/**
 * Higher-timeframe facts for one market, polled once for everyone who needs
 * them.
 *
 * Called in the Desk and handed to both the card and the chart, because the
 * card names a break level and the chart draws a line at it: two fetches would
 * be two answers about one price, and the desk would be arguing with itself in
 * the one place a reader compares them. Same rule as the tick stream and the
 * accounts poll.
 *
 * A minute, because H4 bars close every four hours. Polling faster would ask
 * the same question sixty times for one possible answer.
 */
const POLL_MS = 60_000

export function useHtf(market: string): { htf: HtfResponse | null; error: string | null } {
  const [htf, setHtf] = useState<HtfResponse | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!market) return
    let alive = true
    // Cleared on a market change so the previous market's levels can never be
    // drawn on this one's chart, even for one frame.
    setHtf(null)
    setError(null)
    const tick = () => {
      api
        .htf(market)
        .then((res) => {
          if (!alive) return
          setHtf(res)
          setError(null)
        })
        // The route being unreachable is a different state from the route
        // saying it has no data for this market, and the card says which.
        .catch((e: Error) => alive && setError(e.message))
    }
    tick()
    const timer = window.setInterval(tick, POLL_MS)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [market])

  return { htf, error }
}
