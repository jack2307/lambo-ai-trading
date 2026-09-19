/**
 * The words the indicator library says, kept out of the component.
 *
 * WHY A DATA FILE AND NOT JSX. `levels.check.mjs` holds six words — score,
 * composite, confluence, bias, rank, strength — off every string the levels
 * screen can produce, by importing the strings and testing them. The library
 * is now where the level switch lives, so its copy is part of what that
 * screen says; a paragraph written inline in a `.tsx` would be a surface the
 * guard cannot see, and a popup introducing the families to somebody who has
 * never seen them drawn is the single most likely place for "strength" to
 * appear. So the prose lives here, the check imports it, and a new sentence
 * has to be added to this file to reach the screen.
 *
 * Nothing here is a recommendation and nothing orders one entry above
 * another. The library lists what exists.
 */

/**
 * The three method-level definitions, out of
 * `crates/fd-indicators/src/lib.rs` — the twelve above them in that table
 * answer "what is this number now", and these three answer "which way is
 * this market, and where does that answer change".
 *
 * KEPT AS A LIST RATHER THAN INFERRED. The obvious inference is "the ones
 * that carry a `measured` cell", and it is true today and is not the same
 * fact: a definition is method-level because it carries a direction, and it
 * carries a measurement because this desk got round to measuring it. Tying
 * the grouping to the measurement would silently regroup an entry the day a
 * fourth cell is published. The cost of the list is that a method-level
 * definition added to the crate lands under `readings` until this line is
 * updated, which is a wrong heading and not a wrong number.
 */
export const METHOD_IDS: readonly string[] = ['supertrend', 'zigzag', 'avwap']

export interface LibraryGroup {
  key: 'readings' | 'methods' | 'levels'
  heading: string
  /** What KIND of thing this group holds, so the three are not confused. */
  blurb: string
}

/**
 * Three kinds of thing, under three headings.
 *
 * The owner's complaint that produced this popup was that the chart drew a
 * mess he could not read, and the mess was partly that a moving average, a
 * direction definition and an order block all arrived through different
 * controls and landed on one canvas. They are still three different sorts of
 * object and the headings say which is which; what they share is that this
 * is now the one place any of them is switched on.
 */
export const LIBRARY_GROUPS: LibraryGroup[] = [
  {
    key: 'readings',
    heading: 'readings',
    blurb:
      'a number computed off the bars on screen, drawn dashed and named "yours" — the server computes it for the timeframe you are looking at, and nothing trades off it',
  },
  {
    key: 'methods',
    heading: 'direction definitions',
    blurb:
      'these carry a direction as well as a value, so they say which way the market is and where that answer changes; all three were measured on this desk before they were drawn',
  },
  {
    key: 'levels',
    heading: 'levels and market structure',
    blurb:
      'prices the levels route publishes, drawn as lines with a tag. One switch draws them and the families below are what it draws — they are a key to the hues, not six switches',
  },
]

/** Every other sentence the popup prints, in one place the check can read. */
export const LIBRARY_COPY = {
  /** The button in the chart header. */
  button: '+ indicator',
  title: 'Indicator library',
  description:
    'Everything this chart can draw, and what this desk has found out about each of it. Adding a line here draws it dashed on the candles on screen; nothing in this list is a suggestion.',
  onChart: 'on the chart now',
  /** When the viewer has added none and the levels are off. */
  onChartEmpty: 'nothing added — the candles, and whatever the run itself drew.',
  /** Above the book's own lines, which this popup cannot remove. */
  bookNote:
    'the run drew these itself, on the timeframe it trades. They are the record of what the bot read, so they are not yours to take off here.',
  filter: 'filter…',
  filterEmpty: 'nothing in the library matches that.',
  /**
   * THE ABSENT MEASUREMENT, SAID IN WORDS.
   *
   * The dropdown this popup replaced rendered nothing at all for the eleven
   * definitions nobody has measured, and its comment gave the reason: a row
   * in the slot where evidence goes is read as evidence. That reasoning is
   * about a dense menu where the slot is invisible when empty. Here the
   * measurement has a COLUMN with a heading over it, and a blank cell under
   * a heading is an answer — the reader takes it for "fine". So the absence
   * is spelled out, as a sentence rather than as a dash or a zero, and it
   * cannot be mistaken for a number because it contains none.
   */
  unmeasured: 'nobody has measured this line',
  /** `verdictFor` returning null, which must never render as silence. */
  noRecord: 'no record on this desk',
  /** Under the parameter list on a row that is not on the chart yet. */
  paramsNote: 'added at these defaults; the periods are editable on the chip once it is drawn',
  /** The levels section, above the one switch. */
  levelsNote:
    'one control for the whole ladder. "live" draws what is still in play near the last close, "everything" drops the distance window and the spent filter and draws the lot.',
  /** Under the family list inside the popup. */
  familiesNote:
    'the families cannot be drawn separately and none of them is preferred to another; each is a hue and the rule that produced it, so a line on the chart can be named.',
} as const

/** Every string above, flattened — what `levels.check.mjs` reads. */
export const LIBRARY_STRINGS: string[] = [
  ...LIBRARY_GROUPS.flatMap((g) => [g.heading, g.blurb]),
  ...Object.values(LIBRARY_COPY),
]
