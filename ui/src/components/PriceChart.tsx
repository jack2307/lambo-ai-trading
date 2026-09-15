import {
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  createChart,
  createSeriesMarkers,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  type ISeriesMarkersPluginApi,
  type SeriesMarker,
  type Time,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import type { Bar, BacktestTrade, IndicatorPoint, OptionsFrame } from '@/lib/api'
import { TradeZones, type TradeZone } from './tradeZones'

export interface ActiveIndicator {
  /**
   * Instance key, e.g. `ema_21` or `macd_12_26_9`.
   *
   * Note that a float parameter puts a dot inside the key —
   * `keltner_20_10_1.5` — so keys are built and compared whole and never split
   * on `.` to recover an output name.
   */
  key: string
  id: string
  /** Numeric overrides, sent back to the server when the series is refreshed. */
  params: Record<string, number>
  outputs: string[]
  /** 0 draws over the candles; anything higher gets its own pane. */
  pane: number
  color: string
}

interface Props {
  bars: Bar[]
  indicators: ActiveIndicator[]
  series: Record<string, IndicatorPoint[]>
  frame: OptionsFrame | null
  showLevels: boolean
  trades: BacktestTrade[]
  showMarkers: boolean
  /** Stop and target bands behind the candles. */
  showZones: boolean
  /** The bucket currently being built from the live feed, if any. */
  liveBar?: Bar | null
  /**
   * An entry decided at the last close, waiting for the next open.
   *
   * Drawn dashed, and with no entry line: the entry price does not exist yet
   * and inventing one — the last close, say — would put a level on the chart
   * that nothing will ever fill at. The stop and the target ARE decided, so
   * those are the two that get drawn.
   */
  pending?: { side: string; stop: number | null; target: number | null } | null
  /**
   * The one trade the reader picked in the fills table, if any.
   *
   * Everything else stays drawn and fades; this one keeps its bands, its
   * captions and its markers, gains a bracket at its two edges, and the chart
   * scrolls to it. Matched on the pair of stamps rather than on an index,
   * because the fills list is reversed for display and re-fetched every minute.
   */
  focus?: BacktestTrade | null
}

/** Reads a CSS custom property so the chart can never drift from the theme. */
function token(name: string, fallback: string): string {
  if (typeof window === 'undefined') return fallback
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback
}

const seconds = (ms: number) => Math.floor(ms / 1000) as unknown as Time

/** How many bars the chart opens on. The rest stays one scroll away. */
const VISIBLE_BARS = 140

export function PriceChart({
  bars,
  indicators,
  series,
  frame,
  showLevels,
  trades,
  showMarkers,
  showZones,
  liveBar,
  pending,
  focus = null,
}: Props) {
  const container = useRef<HTMLDivElement>(null)
  const chart = useRef<IChartApi | null>(null)
  const candles = useRef<ISeriesApi<'Candlestick'> | null>(null)
  const overlays = useRef<Map<string, ISeriesApi<'Line' | 'Histogram'>>>(new Map())
  const priceLines = useRef<IPriceLine[]>([])
  const pendingLines = useRef<IPriceLine[]>([])
  const markers = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const zones = useRef<TradeZones | null>(null)

  // Create the chart once; data arrives through the effects below.
  useEffect(() => {
    if (!container.current) return

    const theme = {
      card: token('--card', '#131413'),
      border: token('--border', '#242724'),
      muted: token('--muted-foreground', '#9aa39a'),
      bull: token('--lc', '#46c98a'),
      bear: token('--lp', '#e05d6a'),
    }

    const instance = createChart(container.current, {
      layout: {
        background: { color: theme.card },
        textColor: theme.muted,
        fontFamily: token('--font-mono', 'monospace'),
        fontSize: 11,
        panes: { separatorColor: theme.border, separatorHoverColor: theme.muted },
      },
      grid: {
        vertLines: { color: theme.border, style: 1 },
        horzLines: { color: theme.border, style: 1 },
      },
      rightPriceScale: { borderColor: theme.border, scaleMargins: { top: 0.08, bottom: 0.08 } },
      timeScale: { borderColor: theme.border, timeVisible: true, secondsVisible: false },
      crosshair: { mode: 0 },
      autoSize: true,
    })

    candles.current = instance.addSeries(CandlestickSeries, {
      upColor: theme.bull,
      downColor: theme.bear,
      borderUpColor: theme.bull,
      borderDownColor: theme.bear,
      wickUpColor: theme.bull,
      wickDownColor: theme.bear,
      // The last-price line and its axis label are the only thing that moves
      // when the market is quiet. BTC can spend a whole minute inside a one-cent
      // range, which no zoom level can render as candle movement — so the
      // numeric readout carries the liveness instead.
      lastValueVisible: true,
      priceLineVisible: true,
      priceLineWidth: 1,
      priceLineStyle: 2,
      priceLineColor: token('--primary', '#8dff08'),
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })
    chart.current = instance

    // Bands go on before the markers so the arrows sit on top of them.
    zones.current = new TradeZones({
      target: theme.bull,
      stop: theme.bear,
      entry: token('--primary', '#8dff08'),
      text: theme.muted,
      focus: token('--primary', '#8dff08'),
    })
    candles.current.attachPrimitive(zones.current)

    return () => {
      instance.remove()
      chart.current = null
      candles.current = null
      overlays.current.clear()
      priceLines.current = []
      markers.current = null
      zones.current = null
    }
  }, [])

  useEffect(() => {
    if (!candles.current || !chart.current) return
    candles.current.setData(
      bars.map((bar) => ({
        time: seconds(bar.time),
        open: bar.open,
        high: bar.high,
        low: bar.low,
        close: bar.close,
      })),
    )
    // Open on the recent window rather than fitting the whole history.
    //
    // Fitting seven days of BTC puts a ~2,700 point range on screen, where a
    // live tick of a few dollars is smaller than one pixel — the chart looks
    // frozen while it is in fact updating. A trading chart should open where
    // the trading is.
    const visible = Math.min(VISIBLE_BARS, bars.length)
    if (visible > 1) {
      chart.current.timeScale().setVisibleLogicalRange({
        from: bars.length - visible,
        to: bars.length + 4,
      })
    } else {
      chart.current.timeScale().fitContent()
    }
  }, [bars])

  /**
   * Bring the picked trade into view.
   *
   * Logical indices rather than timestamps: `setVisibleRange` takes times that
   * must exist on the series, and a trade that opened on a bar the window no
   * longer carries would silently do nothing. Finding the two bars by index
   * also gives the honest answer when the trade is off the loaded window —
   * `entry < 0` means the chart is showing 240 bars and this fill is older,
   * which is a thing to leave alone rather than to fake a scroll for.
   *
   * Only runs when the picked trade changes. A reader who then scrolls away
   * is not yanked back on the next sixty-second poll.
   */
  useEffect(() => {
    if (!focus || !chart.current || bars.length === 0) return
    const entry = bars.findIndex((b) => b.time >= focus.entryTime)
    if (entry < 0) return
    let exit = bars.findIndex((b) => b.time >= focus.exitTime)
    if (exit < 0) exit = bars.length - 1
    // Half the trade's own length either side, and never so tight that a
    // trade closed on its entry bar fills the screen with two candles.
    const pad = Math.max(8, Math.round((exit - entry) * 0.6))
    chart.current.timeScale().setVisibleLogicalRange({
      from: Math.max(0, entry - pad),
      to: Math.min(bars.length + 4, exit + pad),
    })
    // `bars` is deliberately absent: a poll that appends one bar must not
    // re-centre a chart the reader has since scrolled.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focus])

  // The forming bucket from the live feed.
  //
  // `update` replaces the last bar when the timestamp matches and appends when
  // it is newer, which is exactly the behaviour a forming bar needs. It is also
  // why the historical `setData` above must run first: updating a series that
  // has no data yet would put a lone candle on an empty chart.
  useEffect(() => {
    if (!liveBar || !candles.current || bars.length === 0) return
    if (liveBar.time < bars[bars.length - 1].time) return // a late frame from a previous bucket
    candles.current.update({
      time: seconds(liveBar.time),
      open: liveBar.open,
      high: liveBar.high,
      low: liveBar.low,
      close: liveBar.close,
    })
  }, [liveBar, bars])

  // Indicator series: rebuilt whenever the active set or its data changes.
  useEffect(() => {
    const instance = chart.current
    if (!instance) return

    for (const series of overlays.current.values()) instance.removeSeries(series)
    overlays.current.clear()

    for (const indicator of indicators) {
      for (const output of indicator.outputs) {
        const key = `${indicator.key}.${output}`
        const points = series[key]
        if (!points?.length) continue

        const isHistogram = output === 'histogram'
        const created = instance.addSeries(
          isHistogram ? HistogramSeries : LineSeries,
          isHistogram
            ? { color: indicator.color, priceLineVisible: false, lastValueVisible: false }
            : {
                color: indicator.color,
                lineWidth: 1,
                priceLineVisible: false,
                lastValueVisible: false,
                title: key,
              },
          indicator.pane,
        )
        created.setData(points.map((point) => ({ time: point.time as unknown as Time, value: point.value })))
        overlays.current.set(key, created)
      }
    }
  }, [indicators, series])

  // The waiting trade. Its own effect and its own line handles, so it can
  // appear and clear on every bar without touching the options levels, which
  // change on a completely different clock.
  useEffect(() => {
    const series = candles.current
    if (!series) return
    for (const line of pendingLines.current) series.removePriceLine(line)
    pendingLines.current = []
    if (!pending) return

    const long = pending.side === 'LONG'
    const rows: [string, number | null, string][] = [
      [`pending ${pending.side.toLowerCase()} · stop`, pending.stop, token('--lp', '#e05d6a')],
      [`pending ${pending.side.toLowerCase()} · target`, pending.target, token('--lc', '#46c98a')],
    ]
    for (const [title, price, color] of rows) {
      if (price == null || !Number.isFinite(price)) continue
      pendingLines.current.push(
        series.createPriceLine({
          price,
          color,
          lineWidth: 1,
          // Dotted, not dashed: the fill bands of a trade that HAPPENED are
          // dashed already, and a level that may still never exist must not
          // look like one that did.
          lineStyle: 1,
          axisLabelVisible: true,
          title: `${long ? '\u25b2' : '\u25bc'} ${title}`,
        }),
      )
    }
  }, [pending])

  // Options-derived levels, drawn as price lines on the candles.
  useEffect(() => {
    const series = candles.current
    if (!series) return

    for (const line of priceLines.current) series.removePriceLine(line)
    priceLines.current = []
    if (!showLevels) return

    const front = frame?.contexts?.[0]
    if (!front) return

    const entries: [string, number | null, string, boolean][] = [
      ['max pain', front.maxPain, token('--primary', '#8dff08'), false],
      ['POC', front.poc, token('--primary', '#8dff08'), false],
      ['wSup', front.wSup, token('--lc', '#46c98a'), true],
      ['wRes', front.wRes, token('--lp', '#e05d6a'), true],
      ['call BE', front.callBE, token('--sc', '#d99446'), true],
      ['put BE', front.putBE, token('--sc', '#d99446'), true],
    ]

    for (const [title, price, color, experimental] of entries) {
      if (price == null || !Number.isFinite(price)) continue
      priceLines.current.push(
        series.createPriceLine({
          price,
          color,
          lineWidth: 1,
          lineStyle: 2,
          axisLabelVisible: true,
          // The warning triangle follows the level all the way to the screen:
          // an unverified formula must not look like a settled one.
          title: `${experimental ? '⚠ ' : ''}${title} ${front.symbol}`,
        }),
      )
    }
  }, [frame, showLevels])

  // Entry and exit markers from the most recent backtest.
  useEffect(() => {
    const series = candles.current
    if (!series) return
    if (!markers.current) markers.current = createSeriesMarkers(series, [])

    if (!showMarkers) {
      markers.current.setMarkers([])
      return
    }

    // Markers have no opacity of their own, so a picked trade is separated by
    // losing the others' labels rather than by fading them: ten arrows each
    // carrying a price is unreadable at the best of times, and the one row the
    // reader clicked is the only one whose numbers they asked for.
    const picked = focus
      ? trades.find((t) => t.entryTime === focus.entryTime && t.exitTime === focus.exitTime) ?? null
      : null

    const points: SeriesMarker<Time>[] = []
    for (const trade of trades) {
      const long = trade.direction === 'LONG'
      const lit = !picked || trade === picked
      points.push({
        time: seconds(trade.entryTime),
        position: long ? 'belowBar' : 'aboveBar',
        color: lit
          ? long
            ? token('--lc', '#46c98a')
            : token('--lp', '#e05d6a')
          : token('--muted-foreground', '#9aa39a'),
        shape: long ? 'arrowUp' : 'arrowDown',
        text: lit ? `${trade.direction} ${trade.entryPrice}` : '',
      })
      points.push({
        time: seconds(trade.exitTime),
        position: long ? 'aboveBar' : 'belowBar',
        color: lit
          ? trade.pnlUsd > 0
            ? token('--sp', '#3fbcc0')
            : token('--sc', '#d99446')
          : token('--muted-foreground', '#9aa39a'),
        shape: 'circle',
        text: lit ? `${trade.exitReason} ${trade.r}R` : '',
      })
    }
    points.sort((a, b) => (a.time as number) - (b.time as number))
    markers.current.setMarkers(points)
  }, [trades, showMarkers, focus])

  // Stop and target bands, one per trade.
  useEffect(() => {
    const primitive = zones.current
    if (!primitive) return
    if (!showZones) {
      primitive.setZones([])
      return
    }
    primitive.setZones(
      trades.map<TradeZone>((trade) => ({
        direction: trade.direction,
        from: Math.floor(trade.entryTime / 1000),
        to: Math.floor(trade.exitTime / 1000),
        focused: focus != null && trade.entryTime === focus.entryTime && trade.exitTime === focus.exitTime,
        entry: trade.entryPrice,
        stop: trade.stop,
        // A strategy that manages its own exit reports no target; drawing one
        // would invent a level the trade never had.
        target: trade.target,
      })),
    )
  }, [trades, showZones, focus])

  return <div ref={container} className="h-full w-full" />
}
