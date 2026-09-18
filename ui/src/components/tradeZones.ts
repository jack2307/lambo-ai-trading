/**
 * Stop and target zones, drawn on the candles.
 *
 * A trade is three prices and a span of time, and reading those off a marker
 * label is work the eye should not have to do. Drawn as bands instead, the
 * question a chart is actually asked — *how much room did this trade have, and
 * which way did price go first* — is answered by looking at it.
 *
 * Lightweight Charts has no box: a primitive is the supported way to put custom
 * geometry on a pane, so this is a primitive rather than a stack of line series
 * pretending to be a shape.
 *
 * Two things it is careful about:
 *
 * * **Risk is drawn from the entry, not from zero.** The red band spans entry
 *   to stop and the green band entry to target, so their relative heights *are*
 *   the reward-to-risk ratio. A band drawn from the axis would make a distant
 *   target look generous on any trade with a wide stop.
 * * **A trade with no target is not a trade with a target at infinity.** Several
 *   strategies here manage their own exit and set `target: null`; those get the
 *   stop band and an open-ended marker rather than a fabricated upper bound.
 */

import type { CanvasRenderingTarget2D } from 'fancy-canvas'
import type {
  IChartApi,
  IPrimitivePaneRenderer,
  IPrimitivePaneView,
  ISeriesApi,
  ISeriesPrimitive,
  SeriesAttachedParameter,
  Time,
} from 'lightweight-charts'

export interface TradeZone {
  direction: 'LONG' | 'SHORT'
  /** Epoch seconds, matching the series' time scale. */
  from: number
  to: number
  entry: number
  stop: number
  target: number | null
  /** Shown against the stop band; kept short enough to sit inside it. */
  label?: string
  /**
   * The one trade the reader is asking about, picked in the fills table.
   *
   * When any zone carries this, every other zone is drawn faint and this one
   * keeps its full weight and gains a bracket. Dimming the rest rather than
   * hiding them is deliberate: a trade means little without the ones either
   * side of it, and a chart that empties when you click a row has answered a
   * different question from the one asked.
   */
  focused?: boolean
}

export interface ZoneColors {
  /** Reward side. */
  target: string
  /** Risk side. */
  stop: string
  entry: string
  text: string
  /** The chrome accent, for the bracket around the trade in focus. */
  focus: string
  /**
   * How opaque the band and its edge are, when the caller wants to differ.
   *
   * Light halves both. A 0.13 fill is a readable tint on a near-black ground
   * and a solid slab on a near-white one — the same number is not the same
   * weight against two different grounds, which is why this became a
   * parameter rather than staying a constant.
   */
  fill?: number
  edge?: number
}

/** How opaque the bands are. Low enough that candles stay readable through them. */
const FILL_ALPHA = 0.13
const EDGE_ALPHA = 0.55

/** What the unfocused zones fade to once one trade is picked. */
const DIMMED = 0.28

function withAlpha(color: string, alpha: number): string {
  const trimmed = color.trim()
  // The theme tokens are hex; anything else (a named colour, an existing rgba)
  // is passed through rather than mangled into something invalid.
  if (/^#([0-9a-f]{6})$/i.test(trimmed)) {
    const value = Number.parseInt(trimmed.slice(1), 16)
    const [r, g, b] = [(value >> 16) & 255, (value >> 8) & 255, value & 255]
    return `rgba(${r}, ${g}, ${b}, ${alpha})`
  }
  return trimmed
}

class ZonesRenderer implements IPrimitivePaneRenderer {
  private readonly zones: TradeZone[]
  private readonly chart: IChartApi
  private readonly series: ISeriesApi<'Candlestick'>
  private readonly colors: ZoneColors

  constructor(
    zones: TradeZone[],
    chart: IChartApi,
    series: ISeriesApi<'Candlestick'>,
    colors: ZoneColors,
  ) {
    this.zones = zones
    this.chart = chart
    this.series = series
    this.colors = colors
  }

  draw(target: CanvasRenderingTarget2D): void {
    target.useBitmapCoordinateSpace((scope) => {
      const ctx = scope.context
      const timeScale = this.chart.timeScale()
      const hr = scope.horizontalPixelRatio
      const vr = scope.verticalPixelRatio

      // One pass to learn whether anything is picked, so a chart with no
      // selection draws exactly as it did before this existed.
      const picked = this.zones.some((z) => z.focused)

      for (const zone of this.zones) {
        const weight = !picked ? 1 : zone.focused ? 1 : DIMMED
        const x1 = timeScale.timeToCoordinate(zone.from as unknown as Time)
        const x2 = timeScale.timeToCoordinate(zone.to as unknown as Time)
        const entryY = this.series.priceToCoordinate(zone.entry)
        if (x1 === null || x2 === null || entryY === null) continue

        // A zero-width band is invisible and a backwards one draws nothing, so
        // a trade that opened and closed on the same bar still gets a sliver.
        const left = Math.min(x1, x2) * hr
        const right = Math.max(x1, x2) * hr
        // Scrolled out of view. Cheap to check and worth checking: a long
        // backtest hands this thousands of trades and every one of them would
        // otherwise be drawn on every frame.
        if (right < 0 || left > scope.bitmapSize.width) continue
        const width = Math.max(right - left, hr)
        const entry = entryY * vr

        const stopY = this.series.priceToCoordinate(zone.stop)
        if (stopY !== null) {
          this.band(ctx, left, width, entry, stopY * vr, this.colors.stop, Math.max(1, hr), weight)
          // A faded band's caption is unreadable and still collides with its
          // neighbours, so the labels belong to whatever is in focus.
          if (weight === 1) {
            this.labelled(ctx, left, width, stopY * vr, zone.label ?? 'stoploss', this.colors.stop, vr)
          }
        }

        if (zone.target !== null && Number.isFinite(zone.target)) {
          const targetY = this.series.priceToCoordinate(zone.target)
          if (targetY !== null) {
            this.band(ctx, left, width, entry, targetY * vr, this.colors.target, Math.max(1, hr), weight)
            if (weight === 1) {
              this.labelled(ctx, left, width, targetY * vr, 'target', this.colors.target, vr)
            }
          }
        }

        // The entry itself, so the two bands have a visible hinge.
        ctx.save()
        ctx.strokeStyle = withAlpha(this.colors.entry, 0.9 * weight)
        ctx.lineWidth = Math.max(1, hr)
        ctx.setLineDash([4 * hr, 3 * hr])
        ctx.beginPath()
        ctx.moveTo(left, entry)
        ctx.lineTo(left + width, entry)
        ctx.stroke()
        ctx.restore()

        // The bracket. Two full-height verticals at the trade's own edges, in
        // the chrome accent, so the eye lands on WHEN as well as on what — a
        // dimmed neighbour can still be darker than an unlit candle, and the
        // span is the thing the reader clicked a row to find.
        if (picked && zone.focused) {
          ctx.save()
          ctx.strokeStyle = withAlpha(this.colors.focus, 0.85)
          ctx.lineWidth = Math.max(1, hr)
          ctx.setLineDash([])
          for (const x of [left, left + width]) {
            ctx.beginPath()
            ctx.moveTo(x, 0)
            ctx.lineTo(x, scope.bitmapSize.height)
            ctx.stroke()
          }
          ctx.restore()
        }
      }
    })
  }

  /** One filled band between two prices, with its far edge drawn solid. */
  private band(
    ctx: CanvasRenderingContext2D,
    left: number,
    width: number,
    from: number,
    to: number,
    color: string,
    lineWidth: number,
    weight = 1,
  ): void {
    const top = Math.min(from, to)
    const height = Math.abs(to - from)
    ctx.save()
    ctx.fillStyle = withAlpha(color, (this.colors.fill ?? FILL_ALPHA) * weight)
    ctx.fillRect(left, top, width, height)
    ctx.strokeStyle = withAlpha(color, (this.colors.edge ?? EDGE_ALPHA) * weight)
    ctx.lineWidth = lineWidth
    ctx.beginPath()
    // Only the boundary that matters: the price the trade was waiting for.
    const edge = to > from ? top + height : top
    ctx.moveTo(left, edge)
    ctx.lineTo(left + width, edge)
    ctx.stroke()
    ctx.restore()
  }

  /** A short caption on a band's edge, drawn only when the band is wide enough. */
  private labelled(
    ctx: CanvasRenderingContext2D,
    left: number,
    width: number,
    y: number,
    text: string,
    color: string,
    vr: number,
  ): void {
    const size = 10 * vr
    ctx.save()
    ctx.font = `${size}px ui-monospace, monospace`
    // Measured, not estimated: a caption wider than the band it describes runs
    // over the neighbouring trade, which is worse than no caption at all.
    if (ctx.measureText(text).width + 8 * vr > width) {
      ctx.restore()
      return
    }
    ctx.fillStyle = withAlpha(color, 0.95)
    ctx.textBaseline = 'bottom'
    ctx.fillText(text, left + 4 * vr, y - 2 * vr)
    ctx.restore()
  }
}

class ZonesPaneView implements IPrimitivePaneView {
  private readonly source: TradeZones

  constructor(source: TradeZones) {
    this.source = source
  }

  /** Behind the series: the candles are the data, the bands are the context. */
  zOrder(): 'bottom' {
    return 'bottom'
  }

  renderer(): IPrimitivePaneRenderer | null {
    const attached = this.source.attachment
    if (!attached || this.source.zones.length === 0) return null
    return new ZonesRenderer(this.source.zones, attached.chart, attached.series, this.source.colors)
  }
}

export class TradeZones implements ISeriesPrimitive<Time> {
  zones: TradeZone[] = []
  attachment: { chart: IChartApi; series: ISeriesApi<'Candlestick'> } | null = null
  colors: ZoneColors

  private readonly view = new ZonesPaneView(this)
  private requestUpdate: (() => void) | null = null

  constructor(colors: ZoneColors) {
    this.colors = colors
  }

  attached(param: SeriesAttachedParameter<Time>): void {
    this.attachment = {
      chart: param.chart as IChartApi,
      series: param.series as ISeriesApi<'Candlestick'>,
    }
    this.requestUpdate = param.requestUpdate
  }

  detached(): void {
    this.attachment = null
    this.requestUpdate = null
  }

  paneViews(): IPrimitivePaneView[] {
    return [this.view]
  }

  setZones(zones: TradeZone[]): void {
    this.zones = zones
    this.requestUpdate?.()
  }
}
