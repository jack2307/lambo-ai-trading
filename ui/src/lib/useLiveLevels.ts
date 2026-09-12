import { useEffect, useRef, useState } from 'react'

import { api, type OptionsFrame } from '@/lib/api'

export type LevelsSource = 'loading' | 'snapshot' | 'live' | 'offline'

interface LevelsEvent {
  type: 'levels' | 'prints' | 'options-status'
  market: string
  frame?: OptionsFrame
  count?: number
}

/**
 * The options picture for a market: fetched once, then kept current from the
 * live stream when the market has one.
 *
 * The print counter is surfaced deliberately. An options tape is not a price
 * tape — BTC expiries can go a minute without a single trade — so a level that
 * has not moved usually means nothing traded, not that the feed is broken. The
 * counter is what tells those two apart.
 */
export function useLiveLevels(market: string) {
  const [frame, setFrame] = useState<OptionsFrame | null>(null)
  const [source, setSource] = useState<LevelsSource>('loading')
  const [prints, setPrints] = useState(0)
  const [lastPrintAt, setLastPrintAt] = useState<number | null>(null)
  const streaming = useRef(false)

  useEffect(() => {
    if (!market) return
    let cancelled = false
    streaming.current = false
    setSource('loading')
    setFrame(null)
    setPrints(0)
    setLastPrintAt(null)

    api
      .levels(market)
      .then((response) => {
        if (cancelled) return
        setFrame(response.frame)
        // A live frame may already have arrived over the socket; do not demote.
        if (!streaming.current) setSource(response.live ? 'live' : 'snapshot')
      })
      .catch(() => {
        if (!cancelled) setSource('offline')
      })

    const events = new EventSource(`/api/live?market=${encodeURIComponent(market)}`)

    events.onmessage = (event) => {
      let payload: LevelsEvent
      try {
        payload = JSON.parse(event.data)
      } catch {
        return
      }
      if (payload.type === 'levels' && payload.frame) {
        streaming.current = true
        setFrame(payload.frame)
        setSource('live')
      } else if (payload.type === 'prints' && payload.count) {
        setPrints((current) => current + payload.count!)
        setLastPrintAt(Date.now())
      }
    }

    // A market without an options stream answers 404 and EventSource reports an
    // error. That is the normal case for gold, not a failure worth shouting
    // about — the REST snapshot above still stands.
    events.onerror = () => {
      if (!streaming.current) setSource((current) => (current === 'live' ? 'offline' : current))
    }

    return () => {
      cancelled = true
      events.close()
    }
  }, [market])

  return { frame, source, prints, lastPrintAt }
}
