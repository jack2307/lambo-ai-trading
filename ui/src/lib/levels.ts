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

import type {
  PriceFairValueGap,
  PriceLevel,
  PriceLevelsResponse,
  PriceLiquidityPool,
  PriceOrderBlock,
} from '@/lib/api'

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
  /** What the legend says. */
  label: string
  /** The CSS variable holding its hue, per theme. */
  hue: string
}

/**
 * THE FAMILIES ARE NO LONGER SELECTABLE, AND THEY ARE STILL DIFFERENT KINDS.
 *
 * Until 2026-09-19 each of these carried an `onByDefault` and had a switch of
 * its own in the chart header; the one switch below replaced all five. What
 * survives the merge is the part that was never a control: a value area and
 * an order block come from different rules and are drawn in different hues,
 * and `LevelLegend` prints this table beside the switch so the hue still
 * names its kind. Merging the controls must not merge the meanings — a chart
 * where four kinds of line share one colour and one name is a chart that has
 * lost the distinction, not one that has simplified it.
 */
export const LEVEL_FAMILIES: FamilyStyle[] = [
  { key: 'profile', label: 'profile', hue: 'var(--level-profile)' },
  { key: 'liquidity', label: 'liquidity', hue: 'var(--level-liquidity)' },
  { key: 'gaps', label: 'gaps', hue: 'var(--level-gaps)' },
  { key: 'blocks', label: 'blocks', hue: 'var(--level-blocks)' },
  { key: 'other', label: 'other', hue: 'var(--muted-foreground)' },
]

/* ------------------------------------------------- the one level switch */

/**
 * The one control over the level ladder, in three positions.
 *
 * WHY ONE AND NOT FIVE. The header carried a master `levels on/off` plus a
 * switch per family plus a `spent on/off` — seven states to think about
 * before a single line is drawn, for a reader who wanted to see the levels.
 * The owner's instruction on 2026-09-19 was to merge them: "gộp hết vào làm
 * 1 đi, khi bật là sẽ hiển thị toàn bộ" — one control, and on shows
 * everything.
 *
 * WHY THREE POSITIONS AND NOT TWO, which is the part of the instruction that
 * needed a decision rather than an edit. "Everything" is literally 252
 * levels on the captured response of 2026-09-19, 197 of them spent, and
 * `stackTags` stops placing tags at their own prices past about 28 in a 400px
 * pane — so a plain on/off either hides most of the response (and is not what
 * was asked for) or degrades every tag on the chart (and is not a chart).
 * Three positions keep both true without a second control:
 *
 *   off         nothing is drawn.
 *   live        the levels still in play within `LIVE_WINDOW_ATR` of the
 *               last close, every family, plus the activity profile at any
 *               distance. 15 of 252 on the captured response.
 *   everything  no distance window and no spent filter: all 252, degraded
 *               tags and all, which is what "hiển thị toàn bộ" says.
 *
 * The count of what `live` is holding back is printed beside the switch and
 * `everything` is one press away, so the middle position is a WINDOW the
 * reader can see the edge of, not a filter that decided for them.
 */
export type LevelMode = 'off' | 'live' | 'everything'

export interface LevelModeStyle {
  key: LevelMode
  /** What the position says on the switch. */
  label: string
}

export const LEVEL_MODES: LevelModeStyle[] = [
  { key: 'off', label: 'off' },
  { key: 'live', label: 'live' },
  // Named for what it does and not for how much it draws, with the crowding
  // said out loud in the title and in `crowdedSentence` — a reader who picks
  // this and gets a wall of tags must recognise the wall as the thing they
  // asked for.
  { key: 'everything', label: 'everything' },
]

/** Absent means `live`: the view the header had by default before the merge
 *  — every family, live only, inside the distance window. */
export const DEFAULT_LEVEL_MODE: LevelMode = 'live'

const KEY = 'fd.desk.levels'
/** The master switch as it was before the merge, read once for the migration
 *  below and never written again. */
const LEGACY_MASTER_KEY = 'fd.desk.showHtf'
/** The spent switch as it was before the merge, same. */
const LEGACY_SPENT_KEY = 'fd.desk.levelsSpent'

function isMode(v: unknown): v is LevelMode {
  return LEVEL_MODES.some((m) => m.key === v)
}

/**
 * Which position this viewer left the switch in.
 *
 * THE KEY IS THE OLD ONE AND THE VALUE IS NOT. `fd.desk.levels` used to hold
 * the JSON list of families that were ON — a list rather than a map of every
 * family, so a family added later started at its own default instead of
 * inheriting a `false` written before it existed. There are no families to
 * switch any more, so the key now holds one of the three mode strings, and
 * everything saved under the old shape has to land somewhere. Renaming the
 * key instead would have been the same bug from the other side: a fresh
 * default for everybody who had ever touched the control, silently.
 *
 * THE MIGRATION NEVER COMES BACK SHOWING LESS THAN THE VIEWER HAD ON. The new
 * control cannot express "blocks only" or "spent, but still windowed", so
 * every legacy state is mapped to the nearest position that hides nothing the
 * viewer had asked to see:
 *
 *   a mode string            →  itself       already migrated, and then the
 *                                            legacy keys are dead to it.
 *   showHtf === '0'          →  off          they had the whole overlay off.
 *   levelsSpent === '1'      →  everything   they had asked for the spent
 *                                            ones; `live` would hide all 197
 *                                            of them again, which is the
 *                                            half-on failure this ordering
 *                                            exists to avoid.
 *   a non-empty family list  →  live         one family or four, they get all
 *                                            of them: the control cannot say
 *                                            "blocks only" any more, and the
 *                                            honest direction to round in is
 *                                            towards more drawn, not less.
 *   an empty family list     →  off          nothing was drawn and nothing is
 *                                            drawn. That state the new
 *                                            control CAN express exactly.
 *   anything else            →  live         a hand-edited value is a working
 *                                            chart, never a blank one.
 *
 * Nothing is deleted from storage. The two legacy keys are left where they
 * are, so a viewer who rolls back to the build before this one finds their
 * old choices intact rather than reset.
 */
export function readLevelMode(): LevelMode {
  try {
    const raw = localStorage.getItem(KEY)
    // ALREADY MIGRATED, AND THEN THE LEGACY KEYS ARE NEVER CONSULTED AGAIN.
    // This line has to come first: read `fd.desk.showHtf` ahead of it and a
    // viewer who had the master off could never leave `off` — they would
    // press `live`, it would be written, and the next reload would read that
    // stale key and overrule them, with the control apparently ignoring them.
    if (isMode(raw)) return raw
    // The master beat the family list before the merge, so it beats it here.
    if (localStorage.getItem(LEGACY_MASTER_KEY) === '0') return 'off'
    if (localStorage.getItem(LEGACY_SPENT_KEY) === '1') return 'everything'
    if (raw == null) return DEFAULT_LEVEL_MODE
    const parsed: unknown = JSON.parse(raw)
    if (isMode(parsed)) return parsed
    if (Array.isArray(parsed)) {
      const known = new Set(LEVEL_FAMILIES.map((f) => f.key))
      const had = parsed.filter((k) => typeof k === 'string' && known.has(k as LevelFamily))
      return had.length > 0 ? 'live' : 'off'
    }
    return DEFAULT_LEVEL_MODE
  } catch {
    // Private mode, blocked storage, or a value someone hand-edited into
    // nonsense. The default is a working chart, so fall back to it rather
    // than to nothing drawn.
    return DEFAULT_LEVEL_MODE
  }
}

export function writeLevelMode(mode: LevelMode) {
  try {
    localStorage.setItem(KEY, mode)
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

/* ------------------------------------------- the levels the route serves */

/**
 * One level of `GET /api/paper/levels`, flattened into the shape the chart
 * and the panel both read.
 *
 * ONE HARVEST FOR BOTH SURFACES, for the reason `useHtf` is polled once for
 * the whole page: the panel names a price and the chart draws a line at it,
 * and two walks of the same response would eventually disagree in the one
 * place a reader compares them.
 *
 * The WORDS are `py/live/smc_context.py`'s, copied deliberately rather than
 * invented here. The model and the screen read the same route, and a desk
 * where the prompt says "high of the last complete trading day" and the panel
 * says "yesterday's high" has two vocabularies for one fact — which is the
 * fault this desk keeps writing tests against. `levels.check.mjs` pins the
 * table against that file.
 *
 * Nothing on it is a score. `dist` and `distAtr` are arithmetic on the
 * response's own `last_close` and `atr14` — the same two numbers the prompt
 * block computes — and `spent` is the route's own state read in one place
 * instead of in four.
 */
export interface DeskLevel {
  /** The route's own token, lowercased: `equal_highs`, `order_block`. */
  kind: string
  family: LevelFamily
  /** For a chart tag, where there is room for two or three words. */
  label: string
  /** For the panel, in the prompt block's wording. */
  word: string
  price: number | null
  bandLow: number | null
  bandHigh: number | null
  /**
   * The price a line is drawn at and distance is measured from: the level's
   * own price where it has one, otherwise the band edge NEARER the last
   * close. Never a band's midpoint — that is a number the route never said
   * and the price never has to reach (the same rule `smc_context._level`
   * states for the prompt).
   */
  anchor: number
  /** Signed, in quote units: positive above the close. `null` when the
   *  response carries no close, which is not a distance of zero. */
  dist: number | null
  /** The same distance in ATR(14), whose denominator is the response's own
   *  `atr14`. `null` when the response has no ATR this bar. */
  distAtr: number | null
  /** Bars of the response's timeframe. `0` is a measurement. */
  ageBars: number | null
  /** The wire token, lowercased. */
  state: string
  stateWord: string
  sideWord: string | null
  directionWord: string | null
  /**
   * A pool already swept or a block already broken — the census's own
   * definition in `smc_context.census`, so the two agree on the word.
   * A partly filled gap is NOT spent: the route drops a gap once it fills,
   * so every gap it serves still has room in it.
   */
  spent: boolean
  swept: boolean | null
  sweptAtBarMs: number | null
  filledFraction: number | null
  spreadAtr: number | null
  /** How many swings a pool is made of; `null` for anything that is not a
   *  pool. The ids themselves are another route's strings and are not read. */
  swings: number | null
  displacementBodyAtr: number | null
  formedAtBarMs: number | null
  /** The rule, in the route's words. Shown on hover: a level without its
   *  rule is a stronger claim than the rule supports. */
  rule: string
}

/** The panel's word for a kind, keyed on the route's token lowercased. The
 *  same table as `smc_context.KIND_WORDS`, including the two definitions a
 *  reader would otherwise assume wrongly: "session" is the trading day IN
 *  PROGRESS and "day" the last COMPLETE one. */
export const KIND_WORDS: Record<string, string> = {
  poc: 'point of control',
  vah: 'value area high',
  val: 'value area low',
  fair_value_gap: 'fair value gap',
  order_block: 'order block',
  equal_highs: 'equal highs',
  equal_lows: 'equal lows',
  prior_day_high: 'prior day high',
  prior_day_low: 'prior day low',
  prior_week_high: 'prior week high',
  prior_week_low: 'prior week low',
  session_high: 'high of the trading day in progress',
  session_low: 'low of the trading day in progress',
  day_high: 'high of the last complete trading day',
  day_low: 'low of the last complete trading day',
  week_high: 'high of the last complete week',
  week_low: 'low of the last complete week',
}

/**
 * The same kinds again, short enough for a tag beside the price axis.
 *
 * A SECOND TABLE RATHER THAN A TRUNCATION. "high of the trading day in
 * progress" cut to a tag's width is "high of the trading d…", which reads as
 * a different level from the one the panel names; a chosen short form does
 * not. Every key here exists in `KIND_WORDS` and `levels.check.mjs` checks
 * that it does, so the two can never name different sets.
 */
export const KIND_TAGS: Record<string, string> = {
  poc: 'POC',
  vah: 'VAH',
  val: 'VAL',
  fair_value_gap: 'gap',
  order_block: 'block',
  equal_highs: 'equal highs',
  equal_lows: 'equal lows',
  prior_day_high: 'prior day high',
  prior_day_low: 'prior day low',
  prior_week_high: 'prior week high',
  prior_week_low: 'prior week low',
  session_high: 'session high',
  session_low: 'session low',
  day_high: 'day high',
  day_low: 'day low',
  week_high: 'week high',
  week_low: 'week low',
}

/** The route's closed set of states, in the prompt block's words. */
export const STATE_WORDS: Record<string, string> = {
  current: 'CURRENT',
  forming: 'STILL FORMING',
  complete: 'COMPLETE',
  unfilled: 'UNFILLED',
  partially_filled: 'PARTLY FILLED',
  untested: 'UNTESTED',
  tested: 'TESTED',
  broken: 'BROKEN',
  resting: 'RESTING',
  swept: 'SWEPT',
}

/** A state this client has never heard of. Rendered in our words, never as
 *  the wire's token — the same rule `smc_context` keeps for the prompt. */
export const UNKNOWN_STATE = "state not in this screen's vocabulary"

const SIDE_WORDS: Record<string, string> = {
  buy_side: 'buy-side liquidity',
  sell_side: 'sell-side liquidity',
}

/** Which way the move that left the level went. NOT "bullish" and NOT
 *  "bearish": the engine's own comment says the name says which side of price
 *  the imbalance is on and not what price will do next, and printing the word
 *  would invite exactly that reading. */
const DIRECTION_WORDS: Record<string, string> = {
  bullish: 'left by an up move',
  bearish: 'left by a down move',
}
const OB_DIRECTION_WORDS: Record<string, string> = {
  bullish: 'before an up move',
  bearish: 'before a down move',
}

/** A wire token reduced to the form the tables are keyed on. */
function normalise(token: unknown): string {
  return String(token ?? '')
    .trim()
    .toLowerCase()
    .replace(/[- ]/g, '_')
}

function finite(v: unknown): number | null {
  return typeof v === 'number' && Number.isFinite(v) ? v : null
}

/**
 * One wire level, measured from the close.
 *
 * `null` for anything carrying neither a price nor a two-price band: a level
 * with no location cannot be drawn and cannot be ordered by distance. The
 * route sends both band edges or neither, and one alone is never guessed at
 * from the other.
 */
function toDeskLevel(
  raw: PriceLevel | null | undefined,
  close: number | null,
  atr: number | null,
): DeskLevel | null {
  if (!raw || typeof raw !== 'object') return null
  // The per-family extras, read off ONE optional shape rather than through
  // four branches. Every field below is `| undefined` here on purpose: a gap
  // has no `side` and a pool no `direction`, and the code that reads them
  // must treat an absent field and a null one the same way.
  const extra = raw as Partial<PriceFairValueGap & PriceOrderBlock & PriceLiquidityPool>
  const price = finite(raw.price)
  let lo = finite(raw.band_low)
  let hi = finite(raw.band_high)
  if (lo != null && hi != null) {
    const [a, b] = [Math.min(lo, hi), Math.max(lo, hi)]
    lo = a
    hi = b
  } else {
    lo = null
    hi = null
  }
  if (price == null && lo == null) return null

  // The POC is the one level carrying BOTH a price and a band — the bucket it
  // sits in — and the price is the one to measure from, because the bucket is
  // an artefact of the histogram's resolution and the price is the level.
  let anchor: number
  let dist: number | null = null
  if (price != null) {
    anchor = price
    dist = close == null ? null : price - close
  } else {
    const low = lo as number
    const high = hi as number
    if (close == null) {
      anchor = low
    } else if (close < low) {
      anchor = low
      dist = low - close
    } else if (close > high) {
      anchor = high
      dist = high - close
    } else {
      // Inside the band. The distance is zero and the line is drawn at
      // whichever edge is nearer, which is still a price the route sent.
      anchor = close - low <= high - close ? low : high
      dist = 0
    }
  }

  const kind = normalise(raw.kind)
  const family = familyOf(kind)
  const state = normalise(raw.state)
  const stateWord = raw.state == null ? '' : (STATE_WORDS[state] ?? UNKNOWN_STATE)
  const side = extra.side == null ? null : (SIDE_WORDS[normalise(extra.side)] ?? null)
  const directions = family === 'blocks' ? OB_DIRECTION_WORDS : DIRECTION_WORDS
  const direction = extra.direction == null ? null : (directions[normalise(extra.direction)] ?? null)
  const swings = Array.isArray(extra.swing_ids) ? extra.swing_ids.length : null

  return {
    kind,
    family,
    label: KIND_TAGS[kind] ?? kind.replace(/_/g, ' '),
    word: KIND_WORDS[kind] ?? kind.replace(/_/g, ' '),
    price,
    bandLow: lo,
    bandHigh: hi,
    anchor,
    dist,
    distAtr: dist == null || atr == null || atr <= 0 ? null : dist / atr,
    ageBars: finite(raw.age_bars),
    state,
    stateWord,
    sideWord: side,
    directionWord: direction,
    spent: state === 'swept' || state === 'broken',
    swept: typeof extra.swept === 'boolean' ? extra.swept : null,
    sweptAtBarMs: finite(extra.swept_at_bar_ms),
    filledFraction: finite(extra.filled_fraction),
    spreadAtr: finite(extra.spread_atr),
    swings,
    displacementBodyAtr: finite(extra.displacement_body_atr),
    // `formed_at_bar_ms` and never `bar_ms`: on this route `bar_ms` is the
    // TIMEFRAME'S LENGTH and lives at the top level, so reading it as a
    // timestamp would date every level to 1970 with nothing complaining.
    formedAtBarMs: finite(raw.formed_at_bar_ms),
    rule: typeof raw.rule === 'string' ? raw.rule : '',
  }
}

/**
 * Every level on one response, in this file's words.
 *
 * The route groups by family in five differently shaped fields — `profile` is
 * an object of three optional levels, `extremes` an object of three periods
 * each with a high and a low, the other three are plain lists — so each is
 * unpacked where it is rather than through one generic walk that would have
 * to guess. Same structure as `smc_context._harvest`, and for the same
 * reason: a walk that guessed would silently stop finding a family the day
 * the route added a key.
 *
 * `null` lists are skipped and empty lists contribute nothing, which is not
 * the same thing and is why the CALLER, not this function, decides what to
 * say about an empty screen.
 */
export function harvestLevels(res: PriceLevelsResponse | null): DeskLevel[] {
  if (!res) return []
  const close = finite(res.last_close)
  const atr = finite(res.atr14)
  const out: DeskLevel[] = []
  const take = (raw: PriceLevel | null | undefined) => {
    const level = toDeskLevel(raw, close, atr)
    if (level) out.push(level)
  }
  if (res.profile) {
    take(res.profile.poc)
    take(res.profile.vah)
    take(res.profile.val)
  }
  for (const gap of res.fair_value_gaps ?? []) take(gap)
  for (const block of res.order_blocks ?? []) take(block)
  for (const pool of res.liquidity ?? []) take(pool)
  if (res.extremes) {
    for (const period of [res.extremes.session, res.extremes.day, res.extremes.week]) {
      take(period?.high)
      take(period?.low)
    }
  }
  return out
}

/**
 * How many levels of each family there are, and how many of them are spent.
 *
 * The screen's copy of `smc_context.census`, and it is here for the reason
 * that file gives: on the captured response of 2026-09-19 the route served
 * 252 levels of which 135 pools were already swept and 62 blocks already
 * broken, so a reader shown the nearest dozen with no idea they were a dozen
 * of 252 would read a tidy tape. The count is not a ranking and must not
 * become one; what it says is how big the thing the window sampled from was.
 */
export interface LevelCensus {
  total: number
  spent: number
  families: { family: LevelFamily; n: number; spent: number }[]
}

export function censusOf(levels: DeskLevel[]): LevelCensus {
  const counts = new Map<LevelFamily, { n: number; spent: number }>()
  for (const level of levels) {
    const row = counts.get(level.family) ?? { n: 0, spent: 0 }
    row.n += 1
    if (level.spent) row.spent += 1
    counts.set(level.family, row)
  }
  return {
    total: levels.length,
    spent: levels.filter((l) => l.spent).length,
    // Largest first, which is the order `_census_lines` prints them in. It is
    // an ordering of COUNTS, not of levels: nothing here says a family with
    // more members matters more.
    families: [...counts.entries()]
      .map(([family, row]) => ({ family, ...row }))
      .sort((a, b) => b.n - a.n),
  }
}

/**
 * How far from the last close a level is still drawn, in ATR(14).
 *
 * THREE ATR, and the number is measured rather than picked. On the captured
 * response of 2026-09-19 (`docs/api-samples/paper-levels.json`) ATR(14) on
 * 15m gold is 12.46 USD/oz, so this window is ±37.4 — and the last COMPLETE
 * trading day on that same response ran 4235.18 to 4367.48, a range of
 * 132.30, which is 10.6 ATR. So ±3 ATR is a bit over a third of a day's range
 * either way: the distance price actually covers inside a session, rather
 * than a number of dollars that would mean something different on gold and on
 * the euro.
 *
 * WHAT IT COSTS AND WHAT IT BUYS, both counted on that response. Of its 252
 * levels, 197 are spent and 55 are live; inside ±3 ATR there are 13 live
 * levels at 12 distinct prices, and the profile's three marks are drawn
 * beside them at whatever distance they sit (see `exemptFromWindow`), so the
 * chart carries 15 levels on 14 lines. Fifteen tags is a chart. The alternatives
 * were measured on the same file rather than imagined: ±6 ATR leaves 35 live
 * levels, 36 drawn once the profile's marks are counted, and
 * a 400px pane holds 28 tags at `TAG_GAP`, so six ATR is already past the
 * point where `stackTags` stops placing tags and starts evenly spreading them
 * — legible, ordered, and no longer at their own prices. Drawing all 252 puts
 * 9 tags in the space of one.
 *
 * ONE NUMBER FOR EVERY TIMEFRAME, AND IT NEEDS NO TUNING PER TIMEFRAME. The
 * denominator is the SERIES' OWN ATR(14), which the route publishes as
 * `atr14`, so the window widens with the chart by construction. Probed
 * against the machine that trades on 2026-09-19, where 1h, 4h and 1d are
 * exported beside the fine three:
 *
 *     tf     atr14    ±3 ATR is     levels on the response
 *     15m     6.91      ±20.7       242 (11 gaps, 80 blocks, 151 pools)
 *     1h     17.04      ±51.1        75 (5, 20, 50)
 *     4h     39.15      ±117.5       26 (0, 8, 18)
 *     1d      null      no window     5 (0, 0, 5 period pools)
 *
 * So the same rule that keeps 13 of 252 on a 15m chart has barely anything to
 * hold back on 4h — which is the point: 26 levels is a different screen from
 * 251, and a window tuned per timeframe would be tuning away the thing the
 * reader came to see. THE "15 OF 252" ABOVE IS THE 15m CLAIM and no other:
 * on the 1h response in `docs/api-samples/paper-levels-1h.json` the same
 * window draws 6 of 66 (45 of them spent) — three live levels and the
 * profile's three marks — though that file is served from the test fixture's
 * seeded walk, so its counts pin the CODE and not the tape.
 *
 * In ATR and not in points because the same rule has to read the same way in
 * a quiet week and a violent one, and on gold and on the euro; the route's
 * own thresholds are in ATR for that reason and this is the viewer's end of
 * the same discipline.
 */
export const LIVE_WINDOW_ATR = 3

/**
 * The one exemption from the distance window, and why it is not the ranking
 * the rest of this file refuses.
 *
 * A RANKING IS A CLAIM ABOUT ONE LEVEL AGAINST ANOTHER INSIDE A CATEGORY:
 * this pool matters more than that pool, this block is "stronger". Nothing
 * here does that and nothing here may. This is a statement about a CATEGORY
 * instead, and the statement is about what kind of object the thing is.
 *
 * The activity profile is not a family of competing levels. It is ONE object
 * with three marks — the point of control and the two edges of the value
 * area — describing the WHOLE window: where the last five trading days spent
 * their time. Its distance from the last close is not a fact about how
 * relevant it is, the way a distant order block's distance is a fact about an
 * event that happened far from here; it is a fact about how far price has
 * travelled from its own recent centre, which is the thing the profile is
 * for. Hiding VAL for being 7.5 ATR away therefore answers a question nobody
 * asked, and it made the profile's own switch lie: turning it on and being
 * shown one mark of three was the window contradicting the control. The
 * switch is gone — one control now — and the argument is untouched by that:
 * it was never about which switch was on, it was about what kind of object
 * the profile is.
 *
 * So the profile's three are drawn whenever anything is drawn, however far
 * away they are, and `PROFILE_NOTE` says on hover what they are. NOTHING ELSE
 * IS EXEMPT — a second family wanting this needs its own argument, made in
 * these terms: what kind of object is it, and is its distance a statement
 * about it or about the market. Decision by the coordinator, 2026-09-19, over
 * my own call the other way.
 */
function exemptFromWindow(level: DeskLevel): boolean {
  return level.family === 'profile'
}

/** What the profile's marks are, wherever one of them is shown. The same
 *  sentence on the chart tag, in the panel row and on the level switch's
 *  title, so the exemption is explained wherever a reader meets it. */
export const PROFILE_NOTE =
  "the activity profile is the window's own statistic — where these bars spent their time — and not a price the market turned at, so all three marks are drawn however far away they are"

/**
 * Which levels to draw, in the position the one switch is in.
 *
 * THIS IS A VIEW, NOT A VERDICT, and the distinction is the one thing not to
 * blur here. Filtering by DISTANCE and by STATE is the viewer choosing a
 * window onto the response — one press moves to `everything` and the window
 * and the spent filter both come off, and the counts below say exactly how
 * much is outside it, so the choice is auditable and reversible. RANKING the levels against each other,
 * scoring them, colouring one "stronger", or drawing a "confluence zone"
 * would be this screen deciding, and it is forbidden:
 * `docs/hypotheses/2026-09-18-smc-context.md` pre-commits against it, the
 * route refuses to do it, and `py/live/smc_context.py` carries the same
 * discipline for the prompt. The prior behind that pre-commitment is not
 * neutral — every mechanical use of these levels this desk has tested has
 * failed out of sample — so a score added here would smuggle a refuted claim
 * back in wearing a new name. If you are about to sort by anything other than
 * distance from the close, or to give a level a number between 0 and 100,
 * that is the thing this comment exists to stop.
 *
 * `hiddenFar` and `hiddenSpent` overlap by construction — a swept pool forty
 * ATR away is both — so they are counted into one `hidden` total and the
 * caller's sentence names the reasons rather than adding the two numbers.
 */
export interface LevelSelection {
  drawn: DeskLevel[]
  /** Outside the window or already spent — what `live` is holding back. */
  hidden: number
  hiddenSpent: number
  hiddenFar: number
  /** Held back because the one switch is at `off`. Counted separately, and
   *  never named in the sentence, because the switch beside it already says
   *  `off` — a count repeating a control is noise. It exists so that drawn +
   *  hidden + hiddenOff still accounts for every level on the response, which
   *  is the invariant the counts are worth anything for. */
  hiddenOff: number
  /**
   * Whether the distance window was actually in force on this pass.
   *
   * False in two different ways, and the caller says something different
   * about each. At `everything` the window was DROPPED ON PURPOSE, which the
   * switch itself announces. Otherwise it is false because no level on the
   * response could be placed in the window at all, which on this route means
   * `atr14` was null: every level is then drawn and the caller has to say why
   * the window is not in force.
   *
   * It is a measured state and not an error. Ten trading days of 1d bars is
   * ten bars, fewer than the fourteen ATR(14) needs, so the route answers
   * with `atr14: null`, no profile and no ATR-derived list — five period
   * pools and nothing else. The screen must say that rather than look empty,
   * and it must NOT borrow another timeframe's ATR to fill the gap: a
   * distance in "ATR" that is 15m ATR on a daily chart is a number that reads
   * right and means nothing, which is exactly what the Desk refuses for
   * indicators off the traded timeframe.
   */
  windowed: boolean
}

export function selectLevels(
  levels: DeskLevel[],
  options: { mode: LevelMode; windowAtr?: number },
): LevelSelection {
  const windowAtr = options.windowAtr ?? LIVE_WINDOW_ATR
  const drawn: DeskLevel[] = []
  let hiddenSpent = 0
  let hiddenFar = 0
  let hiddenOff = 0
  let windowed = false
  // `everything` is the instruction taken literally: no window and no spent
  // filter. It is not a wider window — a wider window would still be this
  // screen choosing a number, and the whole point of the third position is
  // that it chooses nothing.
  const showSpent = options.mode === 'everything'
  const windowInForce = options.mode === 'live'
  for (const level of levels) {
    if (options.mode === 'off') {
      hiddenOff += 1
      continue
    }
    const spentOut = level.spent && !showSpent
    // A level whose distance cannot be measured — no close, or no ATR on the
    // response, which is the ordinary 1d case — is DRAWN, not dropped. The
    // window is a convenience and the level is a fact; hiding a fact because
    // the convenience is unavailable is the failure this file's first
    // paragraph is about. `windowed` below is how the caller knows to say so.
    const farOut =
      windowInForce && level.distAtr != null && Math.abs(level.distAtr) > windowAtr && !exemptFromWindow(level)
    if (windowInForce && level.distAtr != null) windowed = true
    if (spentOut) hiddenSpent += 1
    else if (farOut) hiddenFar += 1
    if (spentOut || farOut) continue
    drawn.push(level)
  }
  return { drawn, hidden: hiddenSpent + hiddenFar, hiddenSpent, hiddenFar, hiddenOff, windowed }
}

/**
 * What the chart says about the levels it is not drawing.
 *
 * COUNTED, NEVER SILENT. The same debt the prompt block pays with "(88
 * further levels on this side are not shown, being further away)": a list
 * that looks complete tells a reader the tape is tidier than it is, and how
 * much tape there is is itself the most useful fact about it. The reasons are
 * named rather than summed, because one level can be hidden for both.
 */
export function hiddenSentence(selection: LevelSelection, windowAtr = LIVE_WINDOW_ATR): string | null {
  if (selection.hidden === 0) return null
  const n = selection.hidden
  const plural = n === 1 ? '' : 's'
  const why =
    selection.hiddenSpent > 0 && selection.hiddenFar > 0
      ? 'spent or further away'
      : selection.hiddenSpent > 0
        ? 'already spent'
        : `further than ${windowAtr} ATR away`
  return `${n} further level${plural}, ${why}, not drawn`
}

/**
 * How many tags a chart pane can place AT THEIR OWN PRICES.
 *
 * Measured, not chosen: `stackTags` is given a 400px pane and a 14px minimum
 * gap by `PriceChart`, and 400/14 is 28. Past that the function takes its
 * overflow branch and spreads every tag evenly across the pane — still
 * ordered, still all present, no longer at the prices they label. That is the
 * number the `everything` position crosses, so it is the number the sentence
 * below quotes.
 */
export const TAGS_PER_PANE = 28

/**
 * What `everything` says about itself.
 *
 * THE COUNT IS THE POINT IN BOTH DIRECTIONS. At `live` the reader is owed the
 * number of levels the window is holding back (`hiddenSentence`); at
 * `everything` nothing is held back, so the same debt is paid the other way
 * round — how many are on the chart, and that past `TAGS_PER_PANE` the tags
 * are no longer at their own prices. A reader who chose this position and
 * sees a wall must be able to read the wall as the thing they asked for
 * rather than as a chart that broke. Below the cap it says nothing, because
 * then nothing has degraded.
 */
export function crowdedSentence(drawn: number, tagsPerPane = TAGS_PER_PANE): string | null {
  if (drawn <= tagsPerPane) return null
  return `all ${drawn} drawn, spent and distant included — past ${tagsPerPane} tags the pane spaces them evenly, so a tag is no longer at its own price`
}

/* --------------------------------------- one price is one line, again */

/** The number of decimals a price is drawn with, and so the tolerance below. */
export const PRICE_DECIMALS = 2

/**
 * Two levels are at the same place when this screen WOULD HAVE DRAWN THE SAME
 * PRICE TWICE — half of the last decimal it prints, not a fraction of ATR.
 *
 * The fraction of ATR was tried on the prompt side and measured wrong: a
 * hundredth of an ATR is 0.125 USD/oz on this response and folded an
 * equal-highs pool at 4367.60 into the prior day's high at 4367.48, which is
 * not "stop repeating a price", it is inventing one. See `COLLAPSE_TOL_PRICE`
 * in `py/live/smc_context.py` for the full receipt; the two tolerances are
 * deliberately the same number.
 */
export const COLLAPSE_TOL_PRICE = 0.5 * 10 ** -PRICE_DECIMALS

export interface FoldableLevel {
  label: string
  price: number
  bandLow?: number | null
  bandHigh?: number | null
  spent?: boolean
}

/**
 * Fold levels the chart would draw at one price into one line naming all of
 * them.
 *
 * WHY, MEASURED: `/api/paper/htf` and `/api/paper/levels` both report the
 * prior day's high, and the levels route reports it a second time as an
 * equal-highs pool and a third as the last complete day's high — three
 * objects, one price, and before this the chart drew three identical dashed
 * lines with three tags stacked 14px apart pretending to be three levels.
 *
 * IT IS THE OPPOSITE OF A RANKING. A ranking would choose one and drop the
 * others; this keeps every name, in the order they arrived, and stops
 * repeating a price. Nothing is dropped, so the count of what is drawn is
 * still the count of what the route sent.
 *
 * WHAT COUNTS AS THE SAME PLACE HERE IS WHAT THE CHART DRAWS, which is not
 * quite the prompt block's rule, and the difference is deliberate rather than
 * drift. `smc_context._same_place` folds two levels when it would PRINT the
 * same price or the same range, so a band and a price are never the same
 * thing there. This folds two levels when it would DRAW the same line: the
 * anchors must agree to the cent, and where both carry a band those bands
 * must agree too, so a gap and a block that happen to share an edge stay two
 * lines with two bands instead of losing one of them. An equal-highs pool
 * and the session high at 4381.20 do fold — one line, both names — because
 * one line is what the chart has to draw for them.
 *
 * A fold is `spent` only when every member is: one live level at that price
 * means the price is still live. It keeps the first band any member carried,
 * so folding a banded level into a bare one never loses the band.
 */
export function foldSamePrice<T extends FoldableLevel>(levels: T[]): T[] {
  const out: T[] = []
  const members: T[][] = []
  const near = (a: number, b: number) =>
    Math.abs(a - b) <= Math.max(COLLAPSE_TOL_PRICE, 1e-6 * Math.max(Math.abs(a), Math.abs(b)))
  const samePlace = (a: T, b: T): boolean => {
    if (!near(a.price, b.price)) return false
    const aBand = a.bandLow != null && a.bandHigh != null
    const bBand = b.bandLow != null && b.bandHigh != null
    if (!aBand || !bBand) return true
    return near(a.bandLow as number, b.bandLow as number) && near(a.bandHigh as number, b.bandHigh as number)
  }
  for (const level of levels) {
    // Compared against the group's FIRST member rather than its last, so a
    // chain of levels each within the tolerance of the one before cannot
    // drift a group wider than the tolerance.
    const found = out.findIndex((head) => samePlace(head, level))
    if (found === -1) {
      out.push(level)
      members.push([level])
    } else {
      members[found].push(level)
    }
  }
  return out.map((head, i) => {
    const group = members[i]
    if (group.length === 1) return head
    const banded = group.find((m) => m.bandLow != null && m.bandHigh != null)
    return {
      ...head,
      label: group.map((m) => m.label).join(' · '),
      bandLow: banded?.bandLow ?? head.bandLow,
      bandHigh: banded?.bandHigh ?? head.bandHigh,
      spent: group.every((m) => m.spent === true),
    }
  })
}
