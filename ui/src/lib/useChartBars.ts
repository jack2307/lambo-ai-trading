import { useEffect, useRef, useState } from 'react'

import { api, type BarsResponse } from '@/lib/api'
import type { Timeframe } from '@/lib/timeframes'

/**
 * Bars for a timeframe the run does not trade.
 *
 * The desk's chart has always drawn the RUN's own bars — the series the book
 * decided on, delivered with the run and already in step with its indicators
 * and its fills. That path is untouched and stays the default, because on the
 * traded timeframe the chart IS the record rather than a view of it.
 *
 * This is the other case. When somebody asks for 4h on a book that trades 15m,
 * there is no run-shaped answer and the bars come from the chart store
 * instead. Everything that makes the traded timeframe trustworthy is then
 * absent — the indicators were computed elsewhere, the export has its own age,
 * the forming candle has to be assembled server-side — so this hook returns
 * the WHOLE response rather than just the bars. Every one of those facts ends
 * up on screen, and a hook that handed back a bare array would have quietly
 * decided they do not matter.
 *
 * `null` while there is nothing yet, which the caller must render as "loading"
 * and not as "no bars": an empty chart that means "still asking" and an empty
 * chart that means "this market has no 1d series" look identical, and only one
 * of them is worth going to look at.
 */
export interface ChartBarsState {
  data: BarsResponse | null
  /** The message from a refused or failed request, verbatim. */
  error: string | null
  /** True until the first answer for the current market and timeframe. */
  loading: boolean
  /** When this client received `data`, epoch ms — for ageing the export. */
  at: number | null
}

const EMPTY: ChartBarsState = { data: null, error: null, loading: false, at: null }

/** How often the store is re-read while an off-timeframe chart is open. */
const POLL_MS = 20_000

/**
 * How many bars to ask for.
 *
 * Enough to fill the viewport at every offered timeframe without asking the
 * store for a year of 5m candles to draw a screen that holds two hundred.
 */
const WANT = 400

export function useChartBars(
  market: string | null,
  tf: Timeframe,
  enabled: boolean,
  /**
   * Bump to ask again for the SAME market and timeframe.
   *
   * Nothing reads the value; it is a dependency and only a dependency. Picking
   * a different timeframe re-runs this effect on its own, but picking the one
   * already selected does not change `tf`, so React bails and nothing is
   * re-requested — which would make "click to try again" on a failed
   * timeframe a promise the control does not keep. The failed timeframe is the
   * one a person is most likely looking at when they want to retry.
   */
  nonce = 0,
): ChartBarsState {
  const [state, setState] = useState<ChartBarsState>(EMPTY)

  // Which request the answer belongs to. A slow 1d fetch that resolves after
  // the viewer has already clicked 5m must not paint 1d bars onto a 5m chart —
  // the candles would be wrong and nothing on screen would say so.
  const want = useRef(0)

  useEffect(() => {
    if (!enabled || !market) {
      want.current += 1
      setState(EMPTY)
      return
    }

    const mine = ++want.current
    let live = true
    // Cleared rather than kept: showing the previous timeframe's candles under
    // the new timeframe's label is the one failure this whole selector exists
    // to avoid.
    setState({ data: null, error: null, loading: true, at: null })

    const read = async () => {
      try {
        const data = await api.bars(market, tf, WANT)
        if (!live || want.current !== mine) return
        setState({ data, error: null, loading: false, at: Date.now() })
      } catch (error) {
        if (!live || want.current !== mine) return
        setState({
          data: null,
          error: error instanceof Error ? error.message : String(error),
          loading: false,
          at: Date.now(),
        })
      }
    }

    void read()
    const timer = window.setInterval(read, POLL_MS)
    return () => {
      live = false
      window.clearInterval(timer)
    }
    // `nonce` is deliberately in here and deliberately unread above.
  }, [market, tf, enabled, nonce])

  return state
}
