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
import { useCallback, useEffect, useRef, useState } from 'react'

import type { Bar, BacktestTrade, IndicatorPoint, OptionsFrame } from '@/lib/api'
import { LEVEL_FAMILIES, familyOf, stackTags, type LevelFamily, type LevelKind } from '@/lib/levels'
import { LevelBands, SPENT_WEIGHT, withAlpha, type LevelBand } from './levelBands'
import { TradeZones, type TradeZone } from './tradeZones'

/**
 * A level the chart draws behind the candles: a price, a name, and the kind
 * that produced it.
 *
 * `kind` is the route's own word — `prior_week_mid`, `h4_high` — and
 * `familyOf` turns it into a colour and a switch. It is optional because a
 * caller with nothing better to say should still get its level drawn, in the
 * neutral family: a level the server believes in and the chart silently omits
 * is the worst outcome available here (see `lib/levels.ts`).
 */
export interface ChartLevel {
  label: string
  /**
   * Where the line and the tag go. For a level that IS a band, this is the
   * band edge nearer the last close — a price the route published — and
   * never the midpoint, which is a number it never sent and price never has
   * to reach.
   */
  price: number
  kind?: LevelKind
  /**
   * The two prices of a level that is a RANGE: a fair value gap, an order
   * block, the spread of an equal-highs pool. Both or neither, the way the
   * route sends them; one alone is not a band and is not guessed at from the
   * other. Drawn by `levelBands`, behind the candles.
   */
  bandLow?: number | null
  bandHigh?: number | null
  /**
   * A pool already swept or a block already broken. Drawn dashed and faded
   * when the viewer has asked to see the spent ones, because a level price
   * has already been through reads differently from one it has not — and
   * showing them at full weight is what makes a chart of 251 levels
   * unreadable. It says what HAPPENED to the level; it does not rank it.
   */
  spent?: boolean
}

/**
 * The family's colour, resolved.
 *
 * The hues are declared once, in `LEVEL_FAMILIES`, as CSS references. This
 * unwraps `var(--level-liquidity)` to ask the stylesheet rather than keeping
 * a second copy of the mapping here — a copy is a thing that drifts, and the
 * toggle's dot and the line it switches must be the same colour or the
 * control does not obviously belong to the level.
 */
function familyColor(family: LevelFamily): string {
  const hue = LEVEL_FAMILIES.find((f) => f.key === family)?.hue ?? 'var(--muted-foreground)'
  return token(hue.slice(4, -1), '#9aa39a')
}

/** The least space two level tags may sit apart, and so their height. */
const TAG_GAP = 14

/** A level tag, placed: where its price is, and where the label is drawn. */
interface PlacedLevelTag {
  label: string
  /** The family's CSS reference, used as written so the theme can move it. */
  hue: string
  y: number
  drawnY: number
  moved: boolean
  /** Swept or broken: faded, like its line and its band. */
  spent: boolean
}

interface TagLayer {
  /** The price axis's width, so the tags can sit just clear of it. */
  axis: number
  tags: PlacedLevelTag[]
}

const EMPTY_TAGS: TagLayer = { axis: 0, tags: [] }

/**
 * Has anything moved enough to be worth a re-render?
 *
 * Half a pixel, the same threshold `stackTags` uses to decide a tag has
 * moved at all. Below that the layer is identical on screen and the only
 * thing a new object buys is a render per scroll frame.
 */
function sameLayer(a: TagLayer, b: TagLayer): boolean {
  if (a.tags.length !== b.tags.length || Math.abs(a.axis - b.axis) > 0.5) return false
  return a.tags.every((tag, i) => {
    const other = b.tags[i]
    return (
      tag.label === other.label &&
      tag.hue === other.hue &&
      tag.moved === other.moved &&
      tag.spent === other.spent &&
      Math.abs(tag.y - other.y) <= 0.5 &&
      Math.abs(tag.drawnY - other.drawnY) <= 0.5
    )
  })
}

/**
 * A trade as the CHART needs it, which is not quite a `BacktestTrade`.
 *
 * `BacktestTrade.pnlUsd` means US dollars, and for the paper book it is. The
 * ACCOUNT's trades are in the account's own currency - USC on the funded cent
 * account - and until 2026-09-17 they were assigned straight into `pnlUsd` and
 * carried across this boundary under a name that says dollars. Nothing broke,
 * because the only consumer here reads the SIGN and a sign is unit-invariant;
 * but `Analytics.tsx` already folds `BacktestTrade[]` with `net += t.pnlUsd`
 * and prints the total with a dollar mark, so the first feature to route the
 * account's trades through it would have stated a real-money P&L a hundred
 * times too large.
 *
 * So account money travels in a field that says what it is, and `pnlUsd` is
 * left NaN for those rows - the same idiom `accountTrades` already uses for
 * the R and the stop a broker's fill cannot know, where a zero would read as
 * a measurement. See docs/decisions/2026-09-17-unit-carrying.md.
 */
export type ChartTrade = BacktestTrade & {
  /** P&L in the ACCOUNT's currency. Set only for the account's own trades. */
  pnlAccount?: number
}

/**
 * Did this trade make money? Read from whichever field actually carries the
 * result, so the answer does not depend on which book it came from. The sign
 * is all this is for, and the sign is the one thing both units agree on.
 */
function won(trade: ChartTrade): boolean {
  const pnl = trade.pnlAccount ?? trade.pnlUsd
  return Number.isFinite(pnl) && pnl > 0
}

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
  /**
   * WHO ASKED FOR THIS LINE: the running book's strategy, or the viewer.
   *
   * The Desk draws both at once and they are not the same kind of fact. The
   * book's indicators are what the bot actually read when it decided; the
   * viewer's are a look, computed for whatever timeframe is on screen. So the
   * two are separated ON THE DRAWING — solid against dashed, `book · …`
   * against `yours · …` — and not only in the caption underneath, because a
   * caption in another corner is how a peer session shipped cached and fresh
   * figures that were identical on screen.
   *
   * Optional and defaulting to `book`, so the Workbench — one set, no
   * ambiguity — draws exactly as it did before.
   */
  source?: 'book' | 'viewer'
  /**
   * What to call it on screen. Defaults to `key`.
   *
   * The Desk namespaces the viewer's keys (`you:ema_21`) so two sets cannot
   * collide in one `series` record or in the handle map below — one handle
   * would overwrite the other and the overwritten series would never be
   * removed. That prefix is plumbing; this is the name.
   */
  name?: string
}

interface Props {
  bars: Bar[]
  indicators: ActiveIndicator[]
  series: Record<string, IndicatorPoint[]>
  frame: OptionsFrame | null
  showLevels: boolean
  trades: ChartTrade[]
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
   * The price the pending entry will fill at, when that is already knowable —
   * the forming bar's open. Null before that bar starts, and then nothing is
   * drawn, because a level nothing can fill at is worse than no level.
   */
  pendingFill?: number | null
  /**
   * Higher-timeframe levels, already reduced to name/price pairs by the
   * caller. Drawn thin and DASHED, so they read as context behind the book's
   * own solid levels rather than as anything this run decided.
   */
  htfLevels?: ChartLevel[]
  /**
   * The position the book is holding right now.
   *
   * Drawn SOLID, where a pending entry is dotted: one is money already at
   * risk and the other is a plan. The chart drew closed fills and pending
   * entries and not this — the live trade, the only line on the page that can
   * still cost anything, was the one thing it did not show.
   */
  open?: { side: string; entry_price: number; stop: number | null; target: number | null } | null
  /**
   * The live result of that open position, ALREADY FORMATTED WITH ITS UNIT.
   *
   * A string and not a number, deliberately. The paper book marks in USD and
   * the account marks in its own currency, and this chart draws both - so a
   * bare number arriving here would be one the component could not label
   * without knowing which book sent it, which is exactly the mistake
   * `ChartTrade` above exists to stop. The caller owns the currency and does
   * the formatting; this draws the text it is given.
   */
  openPnl?: { label: string; positive: boolean } | null
  /**
   * Draw the open position at all. Off hides the entry, stop, target and the
   * result - the chart then shows only what has already happened.
   */
  showOpen?: boolean
  /**
   * The one trade the reader picked in the fills table, if any.
   *
   * Everything else stays drawn and fades; this one keeps its bands, its
   * captions and its markers, gains a bracket at its two edges, and the chart
   * scrolls to it. Matched on the pair of stamps rather than on an index,
   * because the fills list is reversed for display and re-fetched every minute.
   */
  focus?: ChartTrade | null
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
  pendingFill,
  htfLevels,
  open,
  openPnl = null,
  showOpen = true,
  focus = null,
}: Props) {
  const container = useRef<HTMLDivElement>(null)
  const chart = useRef<IChartApi | null>(null)
  const candles = useRef<ISeriesApi<'Candlestick'> | null>(null)
  const overlays = useRef<Map<string, ISeriesApi<'Line' | 'Histogram'>>>(new Map())
  const priceLines = useRef<IPriceLine[]>([])
  const pendingLines = useRef<IPriceLine[]>([])
  const htfLines = useRef<IPriceLine[]>([])
  const markers = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const zones = useRef<TradeZones | null>(null)
  const bands = useRef<LevelBands | null>(null)

  // Create the chart once; data arrives through the effects below.
  useEffect(() => {
    if (!container.current) return

    const theme = {
      card: token('--card', '#131413'),
      border: token('--border', '#242724'),
      muted: token('--muted-foreground', '#9aa39a'),
      // Candles read their OWN tokens, not the text colours. `--lc` and
      // `--lp` are tuned to be legible as small text; as fills across half a
      // light panel they are far too loud. The fallbacks are dark's values,
      // so a stylesheet that predates these tokens draws exactly as before.
      bull: token('--candle-up', '#46c98a'),
      bear: token('--candle-down', '#e05d6a'),
      // Under the data rather than around the panel: the border holds the
      // edge, the grid must not compete with the candles.
      grid: token('--chart-grid', '#242724'),
    }

    // Band opacity comes from the stylesheet so light can halve it. Parsed
    // rather than assumed: a missing or malformed token falls back to the
    // value tradeZones.ts has always used instead of drawing an invisible
    // band or a solid one.
    const alpha = (name: string, fallback: number): number => {
      const raw = Number.parseFloat(token(name, ''))
      return Number.isFinite(raw) && raw > 0 && raw <= 1 ? raw : fallback
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
        vertLines: { color: theme.grid, style: 1 },
        horzLines: { color: theme.grid, style: 1 },
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
      // The BANDS keep the data colours rather than the candle colours: a
      // stop and a target are levels the book chose, and they are read as
      // "which side" at a glance. The candles around them are the ones that
      // had to quieten down.
      target: token('--lc', '#46c98a'),
      stop: token('--lp', '#e05d6a'),
      entry: token('--primary', '#8dff08'),
      text: theme.muted,
      focus: token('--primary', '#8dff08'),
      fill: alpha('--band-fill', 0.13),
      edge: alpha('--band-edge', 0.55),
    })
    candles.current.attachPrimitive(zones.current)

    // The level bands, attached after the trade zones so a stop band sits on
    // top of the context it was chosen against rather than under it.
    //
    // HALF THE TRADE BANDS' FILL, and the number is not a guess. A trade has
    // one stop band and at most one target band on screen; the level window
    // can put a dozen gaps and blocks up at once, and at 0.13 apiece two
    // overlapping bands are darker than an unlit candle — at which point the
    // context is louder than the data it is context for.
    bands.current = new LevelBands({
      fill: alpha('--band-fill', 0.13) * 0.5,
      edge: alpha('--band-edge', 0.55) * 0.7,
    })
    candles.current.attachPrimitive(bands.current)

    return () => {
      instance.remove()
      chart.current = null
      candles.current = null
      overlays.current.clear()
      priceLines.current = []
      markers.current = null
      zones.current = null
      bands.current = null
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

    indicators.forEach((indicator, index) => {
      for (const output of indicator.outputs) {
        const key = `${indicator.key}.${output}`
        const points = series[key]
        if (!points?.length) continue

        const isHistogram = output === 'histogram'
        // The series' own name, with its owner in front of it. The library
        // prints this beside the last-value label when one is shown, and it
        // is the name anything else that reads the series gets — so the two
        // sets cannot be confused by a caller that only has the handle. What
        // separates them ON THE CANVAS is the line style below; this is the
        // name, not the marking.
        const shown = `${indicator.source === 'viewer' ? 'yours' : 'book'} · ${indicator.name ?? indicator.key}.${output}`
        const created = instance.addSeries(
          isHistogram ? HistogramSeries : LineSeries,
          isHistogram
            ? { color: indicator.color, priceLineVisible: false, lastValueVisible: false, title: shown }
            : {
                color: indicator.color,
                lineWidth: 1,
                // DASHED IS THE VIEWER'S. Two sets on one chart in the same
                // five hues are two sets nobody can tell apart at a glance,
                // and colour cannot carry it: the palette has five data hues
                // (ui/DESIGN.md) and both sets cycle them, so the book's
                // fifth line and the viewer's first are the same colour by
                // construction. Style is a channel neither set shares.
                lineStyle: indicator.source === 'viewer' ? 2 : 0,
                priceLineVisible: false,
                lastValueVisible: false,
                title: shown,
              },
          indicator.pane,
        )
        created.setData(points.map((point) => ({ time: point.time as unknown as Time, value: point.value })))
        // THE HANDLE IS KEYED ON THE POSITION, not on the series key. Two
        // entries can legitimately resolve to the same key — a viewer
        // editing one line's period onto another's, and, before the `you:`
        // prefix existed, any indicator the book and the viewer shared. Two
        // `set`s under one key leave the first series on the chart with
        // nothing holding it: a line that can never be removed, which is the
        // sort of thing that is only noticed weeks later as a mystery curve.
        overlays.current.set(`${index}:${key}`, created)
      }
    })
  }, [indicators, series])

  // The waiting trade. Its own effect and its own line handles, so it can
  // appear and clear on every bar without touching the options levels, which
  // change on a completely different clock.
  useEffect(() => {
    const series = candles.current
    if (!series) return
    for (const line of pendingLines.current) series.removePriceLine(line)
    pendingLines.current = []

    // An open position and a pending entry are mutually exclusive — the desk
    // allows one position at a time — so they share these handles. Solid for
    // the one that is real, dotted for the one that is still a plan.
    // Off means off: the handles are cleared above, so flipping the toggle
    // removes the lines rather than leaving them behind the new state.
    if (!showOpen) return
    const live = open ?? pending
    if (!live) return
    const held = open != null
    const side = live.side
    const long = side === 'LONG'
    const mark = long ? '\u25b2' : '\u25bc'
    const rows: [string, number | null, string][] = held
      ? [
          [
            // The result rides on the ENTRY line because that is the level it
            // is measured from; putting it on its own line would need a price
            // to sit at, and there isn't one.
            openPnl ? `${side.toLowerCase()} from · ${openPnl.label}` : `${side.toLowerCase()} from`,
            open!.entry_price,
            openPnl ? token(openPnl.positive ? '--lc' : '--lp', openPnl.positive ? '#46c98a' : '#e05d6a') : token('--primary', '#8dff08'),
          ],
          ['stop', open!.stop, token('--lp', '#e05d6a')],
          ['target', open!.target, token('--lc', '#46c98a')],
        ]
      : [
          [`pending ${side.toLowerCase()} · stop`, pending!.stop, token('--lp', '#e05d6a')],
          [`pending ${side.toLowerCase()} · target`, pending!.target, token('--lc', '#46c98a')],
          // Drawn only once the forming bar exists, when its open IS the fill.
          ['fills here', pendingFill ?? null, token('--primary', '#8dff08')],
        ]
    for (const [title, price, color] of rows) {
      if (price == null || !Number.isFinite(price)) continue
      pendingLines.current.push(
        series.createPriceLine({
          price,
          color,
          lineWidth: held ? 2 : 1,
          // Solid for a position that exists; dotted for one that may never.
          // The fill bands of a trade that HAPPENED are dashed already, so a
          // level that is only a plan must not borrow that look.
          lineStyle: held ? 0 : 1,
          axisLabelVisible: true,
          title: `${mark} ${title}`,
        }),
      )
    }
  }, [pending, pendingFill, open, openPnl, showOpen])

  // The higher timeframe's levels, on their own handles so the toggle can
  // clear them without touching the book's.
  useEffect(() => {
    const series = candles.current
    if (!series) return
    for (const line of htfLines.current) series.removePriceLine(line)
    htfLines.current = []
    const drawnBands: LevelBand[] = []
    for (const level of htfLevels ?? []) {
      if (!Number.isFinite(level.price)) continue
      const color = familyColor(familyOf(level.kind ?? ''))
      // A LEVEL IS A PRICE OR A BAND, and the route says which by sending
      // both edges or neither. The band goes behind the candles as a band and
      // still gets its edge line and its tag, so a gap is not a different
      // kind of object on screen from the price levels around it — it is the
      // same object with room in it.
      if (level.bandLow != null && level.bandHigh != null) {
        drawnBands.push({ low: level.bandLow, high: level.bandHigh, color, spent: level.spent })
      }
      htfLines.current.push(
        series.createPriceLine({
          price: level.price,
          // The family's own hue, which is also the colour of the switch
          // that turns it off. Grey for everything was fine while these were
          // three H4 swings; with the prior day's and prior week's extremes
          // beside them, eight identical dashes say nothing about which is
          // which.
          //
          // A SPENT LEVEL IS FADED BY THE SAME FUNCTION AND THE SAME WEIGHT
          // ITS BAND IS, so a broken block's line and its band cannot end up
          // saying different things about one level.
          color: level.spent ? withAlpha(color, SPENT_WEIGHT) : color,
          lineWidth: 1,
          // Dashed and one pixel: this is context from a slower chart, and it
          // must not compete with the book's own stop and target, which are
          // the levels that decide this trade. A spent one is dotted, which
          // is the same distinction the bands draw.
          lineStyle: level.spent ? 1 : 2,
          axisLabelVisible: false,
          // NO TITLE HERE ON PURPOSE. The library draws its titles where the
          // price falls and lets two of them sit on top of each other, and
          // the top one wins — a level the route published, drawn, and
          // effectively unlabelled. The tags below are placed by `stackTags`
          // instead, which exists for exactly this and is tested for it.
          title: '',
        }),
      )
    }
    // Set in one call whether there are bands or not: a response that loses
    // its last gap must clear the primitive, and a `setBands` skipped on the
    // empty case would leave the previous market's bands behind the candles.
    bands.current?.setBands(drawnBands)
  }, [htfLevels])

  /**
   * The level tags, placed so none covers another.
   *
   * Kept in React state and drawn as HTML over the canvas, because the
   * placement is a pure function of the pixel positions and those are only
   * knowable after the chart has scaled. `prior_day_high` and `prior_week_mid`
   * can be thirteen dollars apart, which on a 380px pane showing a $200 range
   * is twenty-five pixels — and on a quiet day they are four ticks and one
   * pixel apart, which is the case that loses one of them.
   *
   * A tag that had to move gets a leader line back to its true price:
   * a label a few pixels off its own line is a small untruth, and the line
   * repairs it.
   */
  const [layer, setLayer] = useState<TagLayer>(EMPTY_TAGS)

  const placeTags = useCallback(() => {
    const series = candles.current
    const instance = chart.current
    const levels = htfLevels ?? []
    if (!series || !instance || levels.length === 0) {
      setLayer((current) => (current.tags.length === 0 ? current : EMPTY_TAGS))
      return
    }
    // Pane 0 only: `priceToCoordinate` answers in the candles' own pane, and
    // an indicator pane below it is somebody else's space.
    const height = instance.paneSize(0).height
    if (height <= TAG_GAP * 2) {
      setLayer((current) => (current.tags.length === 0 ? current : EMPTY_TAGS))
      return
    }
    const input = levels.flatMap((level) => {
      if (!Number.isFinite(level.price)) return []
      const y = series.priceToCoordinate(level.price)
      // Off the top or bottom of the pane. The LINE is off-screen too, so
      // there is nothing to label; clamping the tag to an edge would park it
      // beside a price that is not there.
      if (y == null || y < 0 || y > height) return []
      return [{ y: y as number, item: level }]
    })
    const placed = stackTags(input, TAG_GAP, TAG_GAP / 2, height - TAG_GAP / 2)
    const next: TagLayer = {
      // The tags hang off the RIGHT of the plot, clear of the price axis.
      // The left is taken: the library anchors a price line's own title
      // there, and the open position's entry, stop and target all carry one
      // — three labels this would have been drawn straight through.
      axis: instance.priceScale('right').width(),
      tags: placed.map((p) => ({
        label: p.item.label,
        hue: LEVEL_FAMILIES.find((f) => f.key === familyOf(p.item.kind ?? ''))?.hue ?? 'var(--muted-foreground)',
        y: p.y,
        drawnY: p.drawnY,
        moved: p.moved,
        spent: p.item.spent === true,
      })),
    }
    // Only when something actually moved. This runs on every frame of a
    // scroll, and re-rendering a dozen spans per frame for positions that
    // are the same to within half a pixel is jank bought for nothing.
    setLayer((current) => (sameLayer(current, next) ? current : next))
  }, [htfLevels])

  /**
   * When to place them again.
   *
   * There is no "the price scale moved" event, so this covers the three
   * things that can move it: new data, a scroll or zoom, and a resize. An
   * autoscale only ever happens because of one of those, so a tag cannot sit
   * at a stale price without one of these firing.
   */
  useEffect(() => {
    placeTags()
    const instance = chart.current
    if (!instance || !container.current) return
    const onRange = () => placeTags()
    instance.timeScale().subscribeVisibleLogicalRangeChange(onRange)
    const observer = new ResizeObserver(() => placeTags())
    observer.observe(container.current)
    return () => {
      instance.timeScale().unsubscribeVisibleLogicalRangeChange(onRange)
      observer.disconnect()
    }
  }, [placeTags, bars, liveBar, indicators, series])

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
          ? won(trade)
            ? token('--sp', '#3fbcc0')
            : token('--sc', '#d99446')
          : token('--muted-foreground', '#9aa39a'),
        shape: 'circle',
        // A broker's fill has no R - it knows what it filled, not what the
        // rule risked - and it arrives here with r NaN rather than 0 so that
        // an absence never renders as a measurement.
        text: lit ? `${trade.exitReason}${Number.isFinite(trade.r) ? ` ${trade.r}R` : ''}` : '',
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

  return (
    <div className="relative h-full w-full">
      <div ref={container} className="h-full w-full" />
      {/* The tag layer. `pointer-events-none` throughout: the chart owns the
          crosshair, and a label that swallowed a drag would break the scroll
          the reader was in the middle of. */}
      <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden>
        {layer.tags.map((tag) => (
          // `inset-0` rather than a bare `right-0`: an inline box with only
          // absolutely-positioned children is zero-high and sits on the line
          // box's baseline, which would offset every tag by a constant
          // nobody would be able to see was there.
          //
          // KEYED ON THE PRICE AS WELL AS THE LABEL. While the levels were
          // five H4 and daily prices every label on the chart was unique; the
          // levels route serves 78 equal-lows pools on one response, and a
          // key of `equal lows` alone makes React treat six tags at six
          // different prices as one — five of them simply never rendered,
          // which is the silent-level failure this whole layer exists to
          // prevent.
          <span key={`${tag.label}@${tag.y.toFixed(1)}`} className="absolute inset-0">
            {tag.moved && (
              // Back to the price it actually sits at. A tag that had to be
              // pushed clear of its neighbour is no longer ON its own line,
              // and the leader says which line it belongs to.
              <span
                className="absolute w-px"
                style={{
                  right: layer.axis + 2,
                  top: Math.min(tag.y, tag.drawnY),
                  height: Math.abs(tag.drawnY - tag.y),
                  backgroundColor: tag.hue,
                  opacity: 0.6,
                }}
              />
            )}
            <span
              className="bg-card/85 absolute rounded-xs border-r-2 px-1 py-px fd-caption whitespace-nowrap"
              style={{
                right: layer.axis + 4,
                top: tag.drawnY - TAG_GAP / 2,
                borderColor: tag.hue,
                color: tag.hue,
                // A spent level's tag fades with its line and its band. The
                // three parts of one level must never disagree about what
                // state it is in.
                opacity: tag.spent ? SPENT_WEIGHT : 1,
              }}
            >
              {tag.label}
            </span>
          </span>
        ))}
      </div>
    </div>
  )
}
