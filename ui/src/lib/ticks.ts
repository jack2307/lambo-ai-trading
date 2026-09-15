import { useEffect, useState } from 'react'

import type { LiveBar } from '@/lib/api'

/**
 * The forming candle, pushed rather than polled.
 *
 * Three hops used to each add their own delay and the chart was last to hear:
 * the terminal was read every ten seconds, posted on the same ten seconds, and
 * the screen asked for the result on its own ten-second poll. Worst case the
 * candle someone was watching was twenty seconds old, which is exactly what a
 * dead feed looks like.
 *
 * `GET /api/paper/stream` is a server-sent event per tick. One connection
 * carries every market and the caller filters, because a socket per book would
 * be ten connections showing the same one chart.
 */

export interface TickEvent {
  market: string
  tf: string
  /** `market:tf` — the same key the status route uses. */
  stream: string
  live: LiveBar
}

/**
 * Subscribe to the live stream and keep the newest bar per stream.
 *
 * Returns a map keyed by `market:tf`, plus whether the connection is up. The
 * flag is not decoration: a chart that silently stops updating is
 * indistinguishable from a quiet market, and this is what lets the screen say
 * which one it is.
 *
 * `EventSource` reconnects on its own after a drop, with its own backoff, so
 * there is no retry logic here — adding one would fight the browser's.
 */
export function useTicks(): { ticks: Record<string, LiveBar>; connected: boolean } {
  const [ticks, setTicks] = useState<Record<string, LiveBar>>({})
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    const source = new EventSource('/api/paper/stream')

    source.onopen = () => setConnected(true)
    source.onerror = () => {
      // Fired on a drop AND while the browser is retrying. Not closed here:
      // closing it would turn a reconnect into a permanent outage.
      setConnected(false)
    }
    source.addEventListener('tick', (event) => {
      try {
        const tick = JSON.parse((event as MessageEvent).data) as TickEvent
        // Keyed by stream and replaced wholesale: there is one current bar per
        // market, never a history, so nothing here can grow.
        setTicks((previous) => ({ ...previous, [tick.stream]: tick.live }))
        setConnected(true)
      } catch {
        // A malformed frame is one bad tick, not a broken stream. The next one
        // is a second away.
      }
    })

    return () => source.close()
  }, [])

  return { ticks, connected }
}
