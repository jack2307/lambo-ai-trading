/**
 * Level bands: the levels that are a RANGE of price rather than one price.
 *
 * A fair value gap and an order block are two prices with room between them,
 * and that room is the level — "price came back into the gap" is a statement
 * about the whole band. Drawn as a line at one edge they become a claim the
 * route never made, and drawn at their midpoint they become a price the route
 * never sent. So they are drawn as what they are.
 *
 * Lightweight Charts has no box, and a primitive is the supported way to put
 * custom geometry on a pane, so this is a primitive — the same shape as
 * `tradeZones.ts` and deliberately a SEPARATE one:
 *
 *  * a trade zone spans the bars the trade was open for; a level band spans
 *    the whole pane, because a level does not stop applying when you scroll,
 *  * a trade zone's near edge is the entry and only the far edge matters; a
 *    level band's two edges are both prices the route published, so both are
 *    stroked,
 *  * and folding the two would put the book's own stop and target under the
 *    same code path as context from a slower read, which is the distinction
 *    the whole overlay is built on.
 *
 * BOTH EDGES, AND NOTHING WRITTEN INSIDE. The labels go through `stackTags`
 * in `PriceChart` with every other level's, because a caption drawn inside
 * the band here would be the one label on the chart that is not decluttered —
 * and two gaps eleven cents apart would print their names on top of each
 * other, which is the failure `stackTags` exists to prevent.
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

export interface LevelBand {
  low: number
  high: number
  /** The family's colour, already resolved to a hex the canvas understands. */
  color: string
  /**
   * A block already broken or a pool already swept, shown only when the
   * viewer asks for the spent ones. Drawn fainter and dashed: the state is a
   * fact the route published and it is the difference between a level price
   * may still react at and one it has already been through. It is NOT a
   * ranking — nothing here says the live ones are stronger, only that the
   * spent ones already happened.
   */
  spent?: boolean
}

export interface BandColors {
  /** How opaque a band's fill is. See the caller for why it is not the trade
   *  zones' number. */
  fill: number
  edge: number
}

/**
 * A hex theme token at an alpha the canvas understands.
 *
 * Exported so `PriceChart` fades a spent level's LINE with the same function
 * that fades a spent band's fill. Two copies of this drifted once already
 * between here and `tradeZones.ts` — that copy stays where it is because the
 * trade zones are not this overlay, but the two halves of ONE overlay must
 * fade together or a spent gap's band and its edge line say different things
 * about the same level.
 */
export function withAlpha(color: string, alpha: number): string {
  const trimmed = color.trim()
  // The theme tokens are hex; anything else (a named colour, an existing
  // rgba) is passed through rather than mangled into something invalid.
  if (/^#([0-9a-f]{6})$/i.test(trimmed)) {
    const value = Number.parseInt(trimmed.slice(1), 16)
    const [r, g, b] = [(value >> 16) & 255, (value >> 8) & 255, value & 255]
    return `rgba(${r}, ${g}, ${b}, ${alpha})`
  }
  return trimmed
}

/** What a spent level fades to, as a multiplier on its alphas. Exported so
 *  the price lines fade by the same amount as the bands. */
export const SPENT_WEIGHT = 0.45

class BandsRenderer implements IPrimitivePaneRenderer {
  private readonly bands: LevelBand[]
  private readonly series: ISeriesApi<'Candlestick'>
  private readonly colors: BandColors

  constructor(bands: LevelBand[], series: ISeriesApi<'Candlestick'>, colors: BandColors) {
    this.bands = bands
    this.series = series
    this.colors = colors
  }

  draw(target: CanvasRenderingTarget2D): void {
    target.useBitmapCoordinateSpace((scope) => {
      const ctx = scope.context
      const hr = scope.horizontalPixelRatio
      const vr = scope.verticalPixelRatio
      const width = scope.bitmapSize.width

      for (const band of this.bands) {
        const a = this.series.priceToCoordinate(band.low)
        const b = this.series.priceToCoordinate(band.high)
        if (a === null || b === null) continue
        const top = Math.min(a, b) * vr
        const bottom = Math.max(a, b) * vr
        // Scrolled or scaled out of the pane. Cheap to check, and the window
        // can hand this a dozen bands on every frame of a scroll.
        if (bottom < 0 || top > scope.bitmapSize.height) continue
        const weight = band.spent ? SPENT_WEIGHT : 1

        ctx.save()
        ctx.fillStyle = withAlpha(band.color, this.colors.fill * weight)
        // A band thinner than a pixel still has to be visible: a fair value
        // gap of four cents on gold is a real gap and would otherwise be a
        // fill of zero height with two edges drawn over each other.
        ctx.fillRect(0, top, width, Math.max(bottom - top, hr))
        ctx.strokeStyle = withAlpha(band.color, this.colors.edge * weight)
        ctx.lineWidth = Math.max(1, hr)
        // BOTH edges: they are two prices the route published, and which of
        // them price reaches first is the thing a reader is looking at.
        ctx.setLineDash(band.spent ? [2 * hr, 3 * hr] : [])
        for (const y of [top, bottom]) {
          ctx.beginPath()
          ctx.moveTo(0, y)
          ctx.lineTo(width, y)
          ctx.stroke()
        }
        ctx.restore()
      }
    })
  }
}

class BandsPaneView implements IPrimitivePaneView {
  private readonly source: LevelBands

  constructor(source: LevelBands) {
    this.source = source
  }

  /** Behind the series: the candles are the data, the levels are context. */
  zOrder(): 'bottom' {
    return 'bottom'
  }

  renderer(): IPrimitivePaneRenderer | null {
    const attached = this.source.attachment
    if (!attached || this.source.bands.length === 0) return null
    return new BandsRenderer(this.source.bands, attached.series, this.source.colors)
  }
}

export class LevelBands implements ISeriesPrimitive<Time> {
  bands: LevelBand[] = []
  attachment: { chart: IChartApi; series: ISeriesApi<'Candlestick'> } | null = null
  colors: BandColors

  private readonly view = new BandsPaneView(this)
  private requestUpdate: (() => void) | null = null

  constructor(colors: BandColors) {
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

  setBands(bands: LevelBand[]): void {
    this.bands = bands
    this.requestUpdate?.()
  }
}
