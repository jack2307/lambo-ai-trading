/**
 * Checks the level ladder's tag stacking and its kind→family mapping.
 *
 * THE STACKING IS THE PART THAT CAN SILENTLY LOSE A LEVEL. Two prices four
 * ticks apart are one pixel apart on screen, so their tags overlap and the
 * top one wins — a level the route published and the chart does not show,
 * with nothing saying so. That is the same failure as a marker dropped for
 * being off-grid, which this repo has now found three times, so the placement
 * is a pure function and this is its test.
 *
 * The property that matters is not "nothing overlaps". It is "nothing
 * overlaps AND nothing was reordered": a declutter that swapped two tags
 * would put a higher price below a lower one, which is worse than an overlap
 * because it is legible and wrong.
 *
 *     node --experimental-strip-types scripts/levels.check.mjs
 */

import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import {
  COLLAPSE_TOL_PRICE,
  KIND_TAGS,
  KIND_WORDS,
  LEVEL_FAMILIES,
  LIVE_WINDOW_ATR,
  STATE_WORDS,
  censusOf,
  familyOf,
  foldSamePrice,
  harvestLevels,
  hiddenSentence,
  selectLevels,
  stackTags,
} from '../src/lib/levels.ts'

let failures = 0
const check = (name, ok, detail) => {
  if (!ok) failures += 1
  console.log(`${ok ? 'ok  ' : 'FAIL'}  ${name}`)
  if (!ok && detail) console.log(`        ${detail}`)
}

const GAP = 14
const TOP = 0
const BOTTOM = 400

/** Every invariant the caller relies on, asserted together. */
function invariants(label, input, placed, { gap = GAP, top = TOP, bottom = BOTTOM } = {}) {
  const byPrice = [...input].sort((a, b) => a.y - b.y).map((t) => t.item)
  check(`${label}: nothing is lost`, placed.length === input.length, `${placed.length} of ${input.length}`)
  check(
    `${label}: order is preserved`,
    placed.map((p) => p.item).join(',') === byPrice.join(','),
    `got ${placed.map((p) => p.item).join(',')} want ${byPrice.join(',')}`,
  )
  const drawn = placed.map((p) => p.drawnY)
  const monotonic = drawn.every((y, i) => i === 0 || y >= drawn[i - 1])
  check(`${label}: drawn positions ascend`, monotonic, drawn.join(' '))
  const tooClose = drawn.find((y, i) => i > 0 && y - drawn[i - 1] < gap - 0.001)
  check(`${label}: every pair is at least ${gap}px apart`, tooClose === undefined, drawn.join(' '))
  check(`${label}: nothing is drawn above the plot`, drawn[0] >= top - 0.001, `${drawn[0]}`)
  check(
    `${label}: nothing is drawn below the plot`,
    drawn[drawn.length - 1] <= bottom + 0.001,
    `${drawn[drawn.length - 1]}`,
  )
}

console.log('-- stacking --')

// The ordinary case: well separated, nothing should move at all.
{
  const input = [
    { y: 40, item: 'VAH' },
    { y: 120, item: 'POC' },
    { y: 260, item: 'VAL' },
  ]
  const placed = stackTags(input, GAP, TOP, BOTTOM)
  invariants('spread out', input, placed)
  check('spread out: nothing is marked moved', placed.every((p) => !p.moved), JSON.stringify(placed.map((p) => p.moved)))
}

// The case that loses a level when drawn naively: four prices inside a pixel.
{
  const input = [
    { y: 200.0, item: 'a' },
    { y: 200.4, item: 'b' },
    { y: 200.8, item: 'c' },
    { y: 201.2, item: 'd' },
  ]
  const placed = stackTags(input, GAP, TOP, BOTTOM)
  invariants('four inside a pixel', input, placed)
  check('four inside a pixel: all are marked moved', placed.filter((p) => p.moved).length >= 3)
}

// Input arriving out of order must come back ordered by PRICE, not by arrival.
{
  const input = [
    { y: 300, item: 'low' },
    { y: 10, item: 'high' },
    { y: 150, item: 'mid' },
  ]
  const placed = stackTags(input, GAP, TOP, BOTTOM)
  invariants('unsorted input', input, placed)
}

// A crowd at the very bottom must push UP into the plot, not pile on the last
// pixel. This is the pass that exists because the first one alone does not.
{
  const input = Array.from({ length: 8 }, (_, i) => ({ y: 396 + i * 0.1, item: `t${i}` }))
  const placed = stackTags(input, GAP, TOP, BOTTOM)
  invariants('crowded at the bottom', input, placed)
  check(
    'crowded at the bottom: the stack was pushed up into the plot',
    placed[0].drawnY < 396,
    `first at ${placed[0].drawnY}`,
  )
}

// More tags than the plot can hold: still ordered, still inside, evenly
// spaced. Crowded is acceptable; wrong is not.
{
  const input = Array.from({ length: 40 }, (_, i) => ({ y: 200 + i * 0.05, item: `t${i}` }))
  const placed = stackTags(input, GAP, TOP, BOTTOM)
  const drawn = placed.map((p) => p.drawnY)
  check('overflowing: order preserved', placed.map((p) => p.item).join(',') === input.map((t) => t.item).join(','))
  check('overflowing: stays inside the plot', drawn[0] >= TOP - 0.001 && drawn[drawn.length - 1] <= BOTTOM + 0.001, `${drawn[0]}..${drawn[drawn.length - 1]}`)
  check('overflowing: still ascending', drawn.every((y, i) => i === 0 || y >= drawn[i - 1]))
}

check('an empty list places nothing', stackTags([], GAP, TOP, BOTTOM).length === 0)
check('one tag never moves', stackTags([{ y: 50, item: 'x' }], GAP, TOP, BOTTOM)[0].moved === false)

console.log('\n-- kind to family --')
const CASES = [
  ['poc', 'profile'],
  ['vah', 'profile'],
  ['val', 'profile'],
  ['fvg', 'gaps'],
  ['imbalance', 'gaps'],
  ['bsl', 'liquidity'],
  ['ssl', 'liquidity'],
  ['equal_highs', 'liquidity'],
  ['order_block', 'blocks'],
  ['ob', 'blocks'],
  // Session, day and week extremes are where stops rest, which is what the
  // liquidity family is. Not a fifth colour.
  ['day_high', 'liquidity'],
  ['week_low', 'liquidity'],
  ['session_extreme', 'liquidity'],
  // The five the route has served since the HTF card shipped and the chart
  // never plotted. The MIDPOINT is not an extreme and matches none of the
  // words above, so it earns its own case: it is produced by the same two
  // prices as the week's high and low, and in `other` it would be off by
  // default — served every poll, never drawn, with nothing saying so.
  ['prior_day_high', 'liquidity'],
  ['prior_day_low', 'liquidity'],
  ['prior_week_high', 'liquidity'],
  ['prior_week_low', 'liquidity'],
  ['prior_week_mid', 'liquidity'],
  // A break level IS the swing it hangs on, by construction. Naming it after
  // the break must not move it out of that swing's family and switch it off.
  ['h4_break', 'liquidity'],
  // An unfamiliar kind is drawn neutral and labelled with its raw string,
  // never dropped: a level the server believes in and the chart omits is the
  // worst outcome available here.
  ['something_new', 'other'],
]
for (const [kind, want] of CASES) {
  const got = familyOf(kind)
  check(`${kind} -> ${want}`, got === want, `got ${got}`)
}

console.log('\n-- defaults --')
const on = LEVEL_FAMILIES.filter((f) => f.onByDefault).map((f) => f.key)
check('profile and liquidity are on by default', on.join(',') === 'profile,liquidity', on.join(','))
check('every family has a hue', LEVEL_FAMILIES.every((f) => f.hue.startsWith('var(--')))

/* ------------------------------------------------------- the vocabulary */

/**
 * THE SCREEN AND THE PROMPT MUST NAME A LEVEL THE SAME WAY.
 *
 * `py/live/smc_context.py` renders these levels for the model and this file's
 * tables render them for the person checking the model. A desk where the
 * prompt says "high of the last complete trading day" and the panel says
 * "yesterday's high" has two vocabularies for one fact, and the reader
 * reconciling them is doing work no one measured. So the Python is PARSED
 * here rather than trusted: whichever side is edited, this fails.
 */
console.log('\n-- the words are the prompt block\'s --')

const py = readFileSync(fileURLToPath(new URL('../../py/live/smc_context.py', import.meta.url)), 'utf8')

/** One `NAME = { "k": "v", ... }` table out of the Python source. */
function pyTable(name) {
  const start = py.indexOf(`${name} = {`)
  if (start === -1) return null
  const end = py.indexOf('\n}', start)
  if (end === -1) return null
  const body = py.slice(start, end)
  const table = {}
  for (const [, key, value] of body.matchAll(/"([a-z_]+)":\s*"([^"]*)"/g)) table[key] = value
  return table
}

for (const [name, mine] of [
  ['KIND_WORDS', KIND_WORDS],
  ['STATE_WORDS', STATE_WORDS],
]) {
  const theirs = pyTable(name)
  check(`${name} was found in py/live/smc_context.py`, theirs != null && Object.keys(theirs).length > 0)
  if (!theirs) continue
  const missing = Object.keys(theirs).filter((k) => mine[k] == null)
  const extra = Object.keys(mine).filter((k) => theirs[k] == null)
  const different = Object.keys(theirs).filter((k) => mine[k] != null && mine[k] !== theirs[k])
  check(`${name}: the screen has every key the prompt has`, missing.length === 0, missing.join(','))
  check(`${name}: the screen has no key the prompt does not`, extra.length === 0, extra.join(','))
  check(
    `${name}: every word is the same word`,
    different.length === 0,
    different.map((k) => `${k}: "${mine[k]}" vs "${theirs[k]}"`).join(' | '),
  )
}

// The short form on the chart is a CHOSEN abbreviation, not a truncation, and
// it must cover exactly the same set: a kind with a word and no tag would be
// drawn under its raw wire token, which is the one thing neither surface does.
check(
  'every kind with a word has a tag',
  Object.keys(KIND_WORDS).every((k) => typeof KIND_TAGS[k] === 'string'),
  Object.keys(KIND_WORDS).filter((k) => KIND_TAGS[k] == null).join(','),
)
check(
  'every tag names a kind that has a word',
  Object.keys(KIND_TAGS).every((k) => typeof KIND_WORDS[k] === 'string'),
  Object.keys(KIND_TAGS).filter((k) => KIND_WORDS[k] == null).join(','),
)

/* ------------------------------------------- against a real response */

/**
 * THE SAMPLE IS A REAL 120 KB RESPONSE, not a fixture written to pass.
 * Every number asserted below was measured from it on 2026-09-19 and every
 * one of them is quoted in a comment somewhere in `lib/levels.ts`; if the
 * route's shape moves, the comments and the code fail together rather than
 * the comments quietly becoming folklore.
 */
console.log('\n-- harvest, census and the window --')

const sample = JSON.parse(
  readFileSync(fileURLToPath(new URL('../../docs/api-samples/paper-levels.json', import.meta.url)), 'utf8'),
)
const levels = harvestLevels(sample)

check('every level on the response is harvested', levels.length === 252, `${levels.length}`)
check(
  'every level has a finite anchor to draw at',
  levels.every((l) => Number.isFinite(l.anchor)),
)
check(
  'every kind the route serves has a word of our own',
  levels.every((l) => Object.values(KIND_WORDS).includes(l.word)),
  levels
    .filter((l) => !Object.values(KIND_WORDS).includes(l.word))
    .map((l) => l.kind)
    .join(','),
)
check(
  'every state the route serves has a word of our own',
  levels.every((l) => Object.values(STATE_WORDS).includes(l.stateWord)),
  [...new Set(levels.filter((l) => !Object.values(STATE_WORDS).includes(l.stateWord)).map((l) => l.state))].join(','),
)

const c = censusOf(levels)
check('the census counts them all', c.total === 252, `${c.total}`)
// 135 swept pools and 62 broken blocks, which is the single most useful fact
// about how messy this tape is — and the reason the window exists at all.
check('197 of the 252 are spent', c.spent === 197, `${c.spent}`)
check(
  'the census is largest family first',
  c.families.every((row, i) => i === 0 || row.n <= c.families[i - 1].n),
  c.families.map((r) => `${r.family}:${r.n}`).join(' '),
)
check(
  'every level is counted under exactly one family',
  c.families.reduce((sum, row) => sum + row.n, 0) === levels.length,
)

// The distance rules, on levels picked out of the real response.
{
  const close = sample.last_close
  const poc = levels.find((l) => l.kind === 'poc')
  check(
    'the POC is measured from its PRICE and not from its bucket',
    Math.abs(poc.anchor - sample.profile.poc.price) < 1e-9,
    `${poc.anchor}`,
  )
  const bands = levels.filter((l) => l.price == null && l.bandLow != null)
  check(
    'a band is measured to its nearer edge, and to zero from inside it',
    bands.every((l) =>
      close < l.bandLow
        ? Math.abs(l.dist - (l.bandLow - close)) < 1e-9
        : close > l.bandHigh
          ? Math.abs(l.dist - (l.bandHigh - close)) < 1e-9
          : l.dist === 0,
    ),
  )
  check(
    'the ATR distance is the price distance over the published atr14',
    levels.every((l) => l.distAtr == null || Math.abs(l.distAtr - l.dist / sample.atr14) < 1e-9),
  )
  check(
    'a swept pool carries the bar it was swept on',
    levels
      .filter((l) => l.swept === true)
      .every((l) => Number.isFinite(l.sweptAtBarMs)),
  )
}

console.log('\n-- what is drawn and what is counted --')

const EVERY_FAMILY = new Set(LEVEL_FAMILIES.map((f) => f.key))
const live = selectLevels(levels, { families: EVERY_FAMILY, showSpent: false })

check('the window is three ATR', LIVE_WINDOW_ATR === 3, `${LIVE_WINDOW_ATR}`)
// The measurement the constant's comment is written on. Widening it is
// allowed; letting the comment claim a number the response does not produce
// is not.
check('13 live levels are inside it on the captured response', live.drawn.length === 13, `${live.drawn.length}`)
check('197 are held back for being spent', live.hiddenSpent === 197, `${live.hiddenSpent}`)
check('42 more are held back for being further away', live.hiddenFar === 42, `${live.hiddenFar}`)
check(
  'nothing is lost between drawn, hidden and switched off',
  live.drawn.length + live.hidden + live.hiddenFamily === levels.length,
  `${live.drawn.length} + ${live.hidden} + ${live.hiddenFamily} vs ${levels.length}`,
)
check('the window was applied', live.windowed === true)

// Six ATR was the alternative, and this is why it is not the default: a
// 400px pane holds 28 tags at a 14px gap, and `stackTags` stops placing them
// at their own prices past that.
const wider = selectLevels(levels, { families: EVERY_FAMILY, showSpent: false, windowAtr: 6 })
check('six ATR would draw 35, which is past what the pane holds', wider.drawn.length === 35, `${wider.drawn.length}`)

const withSpent = selectLevels(levels, { families: EVERY_FAMILY, showSpent: true })
check('the spent switch reveals them', withSpent.drawn.length === 99, `${withSpent.drawn.length}`)
check('and then nothing is hidden for being spent', withSpent.hiddenSpent === 0)

const oneFamily = selectLevels(levels, { families: new Set(['profile']), showSpent: false })
check(
  'a family switched off is counted apart from the window',
  oneFamily.hiddenFamily === 249 && oneFamily.hiddenSpent === 0,
  `${oneFamily.hiddenFamily} / ${oneFamily.hiddenSpent}`,
)

// THE SENTENCE ITSELF. It is the audit of the window and it is asserted
// verbatim, because a count that quietly stops being printed is exactly the
// silence this whole file is against.
check(
  'the hidden count reads like the prompt block',
  hiddenSentence(live) === '239 further levels, spent or further away, not drawn',
  hiddenSentence(live),
)
check(
  'with the spent ones on, it names distance alone',
  hiddenSentence(withSpent) === '153 further levels, further than 3 ATR away, not drawn',
  hiddenSentence(withSpent),
)
check(
  'a single hidden level is not pluralised',
  hiddenSentence({ hidden: 1, hiddenSpent: 1, hiddenFar: 0 }) === '1 further level, already spent, not drawn',
  hiddenSentence({ hidden: 1, hiddenSpent: 1, hiddenFar: 0 }),
)
check('nothing hidden says nothing', hiddenSentence({ hidden: 0, hiddenSpent: 0, hiddenFar: 0 }) === null)

console.log('\n-- one price is one line --')

// The live case, from the response: an equal-highs pool and the session high
// are both at 4381.20, and before the fold the chart drew two identical lines
// with two tags stacked pretending to be two levels.
{
  const drawn = live.drawn.map((l) => ({
    label: l.label,
    price: l.anchor,
    bandLow: l.bandLow,
    bandHigh: l.bandHigh,
    spent: l.spent,
  }))
  const folded = foldSamePrice(drawn)
  check('13 levels are drawn as 12 lines', folded.length === 12, `${folded.length}`)
  check(
    'the fold names every level at the price',
    folded.some((f) => f.label === 'equal highs · session high'),
    folded.map((f) => f.label).join(' | '),
  )
  check(
    'and keeps the band one of them carried',
    folded.find((f) => f.label === 'equal highs · session high')?.bandLow != null,
  )
  check('nothing is invented', folded.every((f) => drawn.some((d) => d.price === f.price)))
}

// The tolerance that was tried and measured wrong, kept as a case: a
// hundredth of an ATR is 0.125 on this response and would have folded these
// two into one price the route never reported.
check(
  'twelve cents apart is two levels, not one',
  foldSamePrice([
    { label: 'equal highs', price: 4367.6 },
    { label: 'prior day high', price: 4367.48 },
  ]).length === 2,
)
check(
  'the same price to the cent is one line',
  foldSamePrice([
    { label: 'a', price: 4381.2 },
    { label: 'b', price: 4381.204 },
  ]).length === 1,
)
check('the tolerance is half of the last decimal drawn', COLLAPSE_TOL_PRICE === 0.005, `${COLLAPSE_TOL_PRICE}`)
check(
  'two different bands on one edge stay two bands',
  foldSamePrice([
    { label: 'gap', price: 4353.33, bandLow: 4340, bandHigh: 4353.33 },
    { label: 'block', price: 4353.33, bandLow: 4351, bandHigh: 4353.33 },
  ]).length === 2,
)
check(
  'a fold is spent only when every level in it is',
  foldSamePrice([
    { label: 'swept pool', price: 4381.2, spent: true },
    { label: 'session high', price: 4381.2, spent: false },
  ])[0].spent === false,
)

console.log(failures === 0 ? '\nall level checks pass' : `\n${failures} FAILED`)
process.exit(failures === 0 ? 0 : 1)
