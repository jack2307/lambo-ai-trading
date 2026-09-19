/**
 * The level ladder: which kinds exist, what colour they wear, and how their
 * tags are stopped from sitting on top of each other.
 *
 * NOTHING HERE SCORES ANYTHING. No percentage, no strength, no "buy zone".
 * A level is a kind and a price; the rule that produced it is a string the
 * route publishes and the tag shows on hover. The moment this file grows a
 * number between 0 and 100 beside a price it has started making a claim
 * nobody measured — which is the same rule the HTF bias word lives under.
 */

/**
 * The kinds the route publishes.
 *
 * Open on purpose: `string` and not a union. An unknown kind is drawn in the
 * neutral family and labelled with its raw string, because a level the server
 * believes in and the chart silently omits is the worst outcome available
 * here — worse than an ugly tag. `familyOf` is what closes the set for
 * colouring.
 */
export type LevelKind = string

/** The four colour families, plus the fallback for a kind we do not know. */
export type LevelFamily = 'profile' | 'gaps' | 'liquidity' | 'blocks' | 'other'

/**
 * Which family a kind belongs to.
 *
 * Grouped by WHAT PRODUCED the level rather than by what it might mean. A
 * value area and a point of control come from the same volume profile; a
 * fair-value gap and an imbalance are the same reading of displacement;
 * buy-side and sell-side liquidity are the same reading of stops. Grouping by
 * meaning instead — "support" and "resistance", say — would be the chart
 * taking a side, and a level above price is only resistance until it isn't.
 */
export function familyOf(kind: LevelKind): LevelFamily {
  const k = kind.toLowerCase()
  if (k === 'poc' || k === 'vah' || k === 'val' || k.startsWith('profile')) return 'profile'
  if (k === 'fvg' || k.includes('gap') || k.includes('imbalance')) return 'gaps'
  if (k === 'bsl' || k === 'ssl' || k.includes('liquidity') || k.includes('equal')) return 'liquidity'
  if (k === 'ob' || k.includes('order_block') || k.includes('order-block') || k.includes('block')) {
    return 'blocks'
  }
  // Session, day and week extremes are where stops rest, which is what the
  // liquidity family IS. They are not a fifth colour.
  if (k.includes('high') || k.includes('low') || k.includes('extreme')) return 'liquidity'
  // The prior week's MIDPOINT is produced by the same two prices as the
  // prior week's high and low — one range, one producer — so it wears their
  // colour. It matches nothing above because it is not itself an extreme,
  // and without this line `prior_week_mid` lands in `other`, which is off by
  // default: a level the route sends every poll, never drawn, with nothing
  // on screen to say it exists.
  if (k.includes('mid') || k.includes('equilibrium')) return 'liquidity'
  // A break level is not an independent price: by construction it IS the
  // swing high or low the structure label hangs on (see `htfLevels` in
  // Desk.tsx). Same price, same producer, same family — naming it after the
  // break must not move it into a colour that is switched off.
  if (k.includes('break')) return 'liquidity'
  return 'other'
}

export interface FamilyStyle {
  key: LevelFamily
  /** What the toggle says. */
  label: string
  /** The CSS variable holding its hue, per theme. */
  hue: string
  /** Shown on first load? Profile and liquidity only — everything at once is
   *  a wall, and a wall is what a reader turns off entirely. */
  onByDefault: boolean
}

export const LEVEL_FAMILIES: FamilyStyle[] = [
  { key: 'profile', label: 'profile', hue: 'var(--level-profile)', onByDefault: true },
  { key: 'liquidity', label: 'liquidity', hue: 'var(--level-liquidity)', onByDefault: true },
  { key: 'gaps', label: 'gaps', hue: 'var(--level-gaps)', onByDefault: false },
  { key: 'blocks', label: 'blocks', hue: 'var(--level-blocks)', onByDefault: false },
  { key: 'other', label: 'other', hue: 'var(--muted-foreground)', onByDefault: false },
]

const KEY = 'fd.desk.levels'

/**
 * Which families this viewer has switched on.
 *
 * Stored as the list that is ON rather than a map of every family, so a
 * family added later starts at its own default instead of inheriting a
 * `false` written before it existed. The failure that shape avoids is the one
 * where a new feature ships switched off for everybody who ever opened the
 * page, and nobody can tell because it looks like it is working.
 */
export function readLevelFamilies(): Set<LevelFamily> {
  const fallback = () => new Set(LEVEL_FAMILIES.filter((f) => f.onByDefault).map((f) => f.key))
  try {
    const raw = localStorage.getItem(KEY)
    if (raw == null) return fallback()
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return fallback()
    const known = new Set(LEVEL_FAMILIES.map((f) => f.key))
    return new Set(parsed.filter((k): k is LevelFamily => typeof k === 'string' && known.has(k as LevelFamily)))
  } catch {
    // Private mode, blocked storage, or a value someone hand-edited into
    // nonsense. The defaults are a working chart, so fall back to them
    // rather than to nothing drawn.
    return fallback()
  }
}

export function writeLevelFamilies(on: Set<LevelFamily>) {
  try {
    localStorage.setItem(KEY, JSON.stringify([...on]))
  } catch {
    /* private mode: the choice lasts the page */
  }
}

export interface Tag<T> {
  /** Where the price actually is, in pixels from the top of the plot. */
  y: number
  item: T
}

export interface PlacedTag<T> extends Tag<T> {
  /** Where the tag is DRAWN, after being pushed clear of its neighbours. */
  drawnY: number
  /** True when the tag had to move — the caller draws a leader line to `y`. */
  moved: boolean
}

/**
 * Stack tags so none overlaps, without ever reordering them.
 *
 * Two levels four ticks apart are two different prices and one pixel apart on
 * screen. Drawn naively their tags overlap and the top one wins, so a level
 * silently disappears — the same failure as a marker dropped off a chart,
 * which is the thing this codebase keeps finding.
 *
 * ORDER IS PRESERVED ABSOLUTELY. A tag is moved, never swapped: if VAH sits
 * above POC in price it sits above it on screen, even when both have been
 * pushed. A declutter that reorders would put a higher price below a lower
 * one, which is worse than an overlap because it is legible and wrong.
 *
 * Two passes. Down first, pushing each tag below the one before it; then up
 * from the bottom if the stack has run past the plot, which redistributes the
 * overflow instead of piling every excess tag on the last pixel. When there
 * is genuinely not enough room for all of them the result is evenly spaced
 * and still ordered — crowded, but never a lie about which is higher.
 *
 * `moved` is returned rather than inferred so the caller can draw a leader
 * line back to the true price: a tag that has been pushed is no longer AT its
 * price, and a label a few pixels off its line is a small untruth that the
 * leader line repairs.
 */
export function stackTags<T>(tags: Tag<T>[], minGap: number, top: number, bottom: number): PlacedTag<T>[] {
  if (tags.length === 0) return []

  const sorted = [...tags].sort((a, b) => a.y - b.y)
  const placed: PlacedTag<T>[] = sorted.map((t) => ({ ...t, drawnY: t.y, moved: false }))

  // MORE TAGS THAN THE PLOT CAN HOLD, handled first because the two passes
  // below cannot. They assume the stack fits: push-down then push-up, and if
  // the run is longer than the plot the push-up walks the first tag off the
  // top — measured at −146px for forty tags in a 400px plot, drawn outside
  // the chart where nobody would ever see it was wrong.
  //
  // When the room genuinely is not there, spread evenly across the whole plot
  // instead. That is crowded and honest: still ordered, still inside, still
  // every level present. Dropping the ones that do not fit would hide levels
  // the route published, and that is the failure this function exists to
  // prevent — so it degrades by getting tighter, never by getting shorter.
  const span = bottom - top
  if (placed.length > 1 && (placed.length - 1) * minGap > span) {
    const step = span / (placed.length - 1)
    placed.forEach((tag, i) => {
      tag.drawnY = top + i * step
      tag.moved = Math.abs(tag.drawnY - tag.y) > 0.5
    })
    return placed
  }

  // Down: nothing may sit within `minGap` of the tag above it.
  let cursor = top
  for (const tag of placed) {
    const y = Math.max(tag.drawnY, cursor)
    tag.drawnY = y
    cursor = y + minGap
  }

  // Up: if the stack overran the plot, push back from the bottom. Without
  // this every overflowing tag lands on the last pixel in a heap.
  cursor = bottom
  for (let i = placed.length - 1; i >= 0; i -= 1) {
    const y = Math.min(placed[i].drawnY, cursor)
    placed[i].drawnY = y
    cursor = y - minGap
  }

  for (const tag of placed) {
    // Sub-pixel drift is not a move; a leader line for half a pixel is noise.
    tag.moved = Math.abs(tag.drawnY - tag.y) > 0.5
  }
  return placed
}
