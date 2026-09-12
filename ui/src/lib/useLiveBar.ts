import { useEffect, useRef, useState } from 'react'

import type { Bar } from '@/lib/api'

export type LiveStatus = 'off' | 'connecting' | 'live' | 'offline'

export const TIMEFRAME_MS: Record<string, number> = {
  '1m': 60_000,
  '5m': 300_000,
  '15m': 900_000,
  '30m': 1_800_000,
  '1h': 3_600_000,
  '4h': 14_400_000,
  '1d': 86_400_000,
}

interface LiveBarEvent {
  type: 'bar'
  market: string
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
  closed: boolean
}

/**
 * Subscribes to the server's live stream and folds one-minute bars into the
 * timeframe on screen.
 *
 * The subtlety is that a forming minute bar is re-sent several times a second
 * with a *growing* high, low and volume. Accumulating those increments would
 * inflate the bucket, so the minute bars are kept individually and the bucket is
 * recomputed from them — the last value for each minute always wins.
 */
export function useLiveBar(market: string, timeframe: string, enabled: boolean) {
  const [bar, setBar] = useState<Bar | null>(null)
  const [status, setStatus] = useState<LiveStatus>('off')
  /** Minute bars belonging to the bucket currently being built. */
  const minutes = useRef<Map<number, Bar>>(new Map())
  const bucketStart = useRef<number | null>(null)

  useEffect(() => {
    minutes.current.clear()
    bucketStart.current = null
    setBar(null)

    if (!enabled || !market || !timeframe) {
      setStatus('off')
      return
    }

    const bucketMs = TIMEFRAME_MS[timeframe] ?? 900_000
    setStatus('connecting')
    const source = new EventSource(`/api/live?market=${encodeURIComponent(market)}`)

    source.onopen = () => setStatus('live')
    source.onerror = () => {
      // EventSource retries on its own; report the gap rather than hiding it.
      setStatus('offline')
    }

    source.onmessage = (event) => {
      let payload: LiveBarEvent
      try {
        payload = JSON.parse(event.data)
      } catch {
        return
      }
      if (payload.type !== 'bar') return
      setStatus('live')

      const start = Math.floor(payload.time / bucketMs) * bucketMs
      if (bucketStart.current !== start) {
        bucketStart.current = start
        minutes.current.clear()
      }
      minutes.current.set(payload.time, {
        time: payload.time,
        open: payload.open,
        high: payload.high,
        low: payload.low,
        close: payload.close,
        volume: payload.volume,
      })

      const ordered = [...minutes.current.values()].sort((a, b) => a.time - b.time)
      if (ordered.length === 0) return
      setBar({
        time: start,
        open: ordered[0].open,
        high: Math.max(...ordered.map((m) => m.high)),
        low: Math.min(...ordered.map((m) => m.low)),
        close: ordered[ordered.length - 1].close,
        volume: ordered.reduce((sum, m) => sum + (m.volume ?? 0), 0),
      })
    }

    return () => {
      source.close()
      setStatus('off')
    }
  }, [market, timeframe, enabled])

  return { bar, status }
}
