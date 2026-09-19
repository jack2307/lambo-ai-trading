import { useEffect, useState } from 'react'

import { api, type PriceLevelsResponse } from '@/lib/api'

/**
 * The price-bar levels for one market, polled once for everyone who draws
 * them.
 *
 * Beside `useHtf` and shaped exactly like it, because the same rule applies:
 * the panel names a price and the chart draws a line at it, so two fetches
 * would be two answers about one price in the one place a reader compares
 * them. One poll in the Desk, handed to both.
 *
 * A MINUTE, the same cadence as `useHtf`. The route computes from CLOSED 15m
 * bars, so its answer can only change four times an hour and this already
 * asks fifteen times for each possible one — polling faster would be asking
 * for nothing. It matches the higher-timeframe poll rather than being tuned
 * on its own so the two context reads sitting side by side on the drill-down
 * can never be more than a minute apart in age, which is the kind of
 * disagreement a reader would take for a bug.
 *
 * The route answers 200 with an `unavailable` sentence when the market's bars
 * have not been exported, so THAT is not an error here: `error` means the
 * route did not answer at all, and the two render differently — the same
 * distinction `useHtf` keeps.
 */
const POLL_MS = 60_000

export function usePriceLevels(market: string): {
  levels: PriceLevelsResponse | null
  error: string | null
} {
  const [levels, setLevels] = useState<PriceLevelsResponse | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!market) return
    let alive = true
    // Cleared on a market change so one market's levels can never be drawn
    // over another's candles, even for a single frame.
    setLevels(null)
    setError(null)
    const tick = () => {
      api
        .priceLevels(market)
        .then((res) => {
          if (!alive) return
          setLevels(res)
          setError(null)
        })
        .catch((e: Error) => alive && setError(e.message))
    }
    tick()
    const timer = window.setInterval(tick, POLL_MS)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [market])

  return { levels, error }
}
