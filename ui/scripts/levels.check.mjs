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
  DEFAULT_LEVEL_MODE,
  KIND_TAGS,
  KIND_WORDS,
  LEVEL_FAMILIES,
  LEVEL_MODES,
  LIVE_WINDOW_ATR,
  PROFILE_NOTE,
  STATE_WORDS,
  TAGS_PER_PANE,
  censusOf,
  crowdedSentence,
  familyOf,
  foldSamePrice,
  harvestLevels,
  hiddenSentence,
  readLevelMode,
  selectLevels,
  stackTags,
  writeLevelMode,
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

console.log('\n-- the one switch, its positions and its defaults --')

/**
 * THE FIVE FAMILY SWITCHES ARE GONE and this is where that is asserted
 * rather than assumed. Until 2026-09-19 each family carried an `onByDefault`
 * and `LEVEL_FAMILIES.filter(f => f.onByDefault)` was the chart's opening
 * view — profile and liquidity on, gaps and blocks off. One switch cannot
 * express a per-family default, so the flag was removed rather than left
 * lying around for a reader to trust; the default is now the switch's own
 * position and it is checked below. What the families KEPT is their hues,
 * because they are still different kinds of level.
 */
check(
  'no family carries a default of its own any more',
  LEVEL_FAMILIES.every((f) => !('onByDefault' in f)),
  LEVEL_FAMILIES.filter((f) => 'onByDefault' in f).map((f) => f.key).join(','),
)
check('every family still has a hue of its own', LEVEL_FAMILIES.every((f) => f.hue.startsWith('var(--')))
check(
  'no two families share a hue — a merged switch must not merge the meanings',
  new Set(LEVEL_FAMILIES.map((f) => f.hue)).size === LEVEL_FAMILIES.length,
  LEVEL_FAMILIES.map((f) => `${f.key}:${f.hue}`).join(' '),
)
check(
  'the switch has exactly three positions, in the order off, live, everything',
  LEVEL_MODES.map((m) => m.key).join(',') === 'off,live,everything',
  LEVEL_MODES.map((m) => m.key).join(','),
)
check('a fresh viewer lands on live', DEFAULT_LEVEL_MODE === 'live', DEFAULT_LEVEL_MODE)

/* ------------------------------------------------------ the migration */

/**
 * NOBODY'S SAVED STATE THROWS, AND NOBODY'S COMES BACK HALF-ON.
 *
 * `fd.desk.levels` held a JSON list of the families that were on;
 * `fd.desk.showHtf` was the master and `fd.desk.levelsSpent` the spent
 * switch. None of those three states is expressible in three positions, so
 * every one of them has to land somewhere, and the direction they round in
 * is the thing this block pins: NEVER towards drawing less than the viewer
 * had switched on. Somebody who had the spent levels on and comes back to a
 * chart with 197 of them silently gone would have no way of telling that
 * from the tape having changed.
 *
 * A stub rather than a real `localStorage`, because `readLevelMode` reads it
 * at call time and node has none. The throwing case is the one that matters
 * most — private mode, blocked storage — and it is here too.
 */
console.log('\n-- migrating what viewers already had --')

const storage = (entries) => ({
  getItem: (k) => (k in entries ? entries[k] : null),
  setItem: (k, v) => {
    entries[k] = v
  },
})
const modeFrom = (entries) => {
  globalThis.localStorage = storage(entries)
  return readLevelMode()
}

const MIGRATIONS = [
  ['nothing saved at all', {}, 'live'],
  ['a viewer already on the new control', { 'fd.desk.levels': 'everything' }, 'everything'],
  ['off stays off', { 'fd.desk.levels': 'off' }, 'off'],
  // The instruction's own example: the control cannot say "blocks only", so
  // it says everything that is live rather than picking one of the four.
  ['only blocks were on', { 'fd.desk.levels': '["blocks"]' }, 'live'],
  ['the old default, profile and liquidity', { 'fd.desk.levels': '["profile","liquidity"]' }, 'live'],
  ['all four families were on', { 'fd.desk.levels': '["profile","liquidity","gaps","blocks"]' }, 'live'],
  // An empty list drew nothing, and `off` draws nothing. This one the new
  // control CAN express exactly, so it is not rounded up.
  ['every family had been switched off', { 'fd.desk.levels': '[]' }, 'off'],
  ['a list of families this client no longer knows', { 'fd.desk.levels': '["unicorns"]' }, 'off'],
  // The master beat everything else before the merge and it still does.
  ['the master was off', { 'fd.desk.showHtf': '0', 'fd.desk.levels': '["profile"]' }, 'off'],
  // THE LEGACY KEYS LOSE TO THE NEW ONE, and these two are the reason the
  // order in `readLevelMode` is what it is. Read the old master first and a
  // viewer who had it off could press `live`, have it saved, and find the
  // control back at `off` on the next load with nothing explaining why.
  ['the master was off and they have since chosen live', { 'fd.desk.showHtf': '0', 'fd.desk.levels': 'live' }, 'live'],
  ['spent was on and they have since chosen live', { 'fd.desk.levelsSpent': '1', 'fd.desk.levels': 'live' }, 'live'],
  ['the master was off and spent was on', { 'fd.desk.showHtf': '0', 'fd.desk.levelsSpent': '1' }, 'off'],
  // Spent on is the case that must NOT land on `live`: that would hide the
  // 197 levels the viewer had explicitly asked to see.
  ['spent was on', { 'fd.desk.levelsSpent': '1', 'fd.desk.levels': '["profile"]' }, 'everything'],
  ['spent was on and the families were never touched', { 'fd.desk.levelsSpent': '1' }, 'everything'],
  ['spent was explicitly off', { 'fd.desk.levelsSpent': '0', 'fd.desk.levels': '["gaps"]' }, 'live'],
  // Hand-edited nonsense is a working chart, not an empty one.
  ['a value somebody hand-edited', { 'fd.desk.levels': 'not json at all' }, 'live'],
  ['an object where a list was', { 'fd.desk.levels': '{"profile":true}' }, 'live'],
]
for (const [what, entries, want] of MIGRATIONS) {
  const got = modeFrom(entries)
  check(`${what} -> ${want}`, got === want, `got ${got}`)
}

check(
  'storage that throws is a working chart, not a blank one',
  (() => {
    globalThis.localStorage = {
      getItem() {
        throw new Error('blocked')
      },
      setItem() {
        throw new Error('blocked')
      },
    }
    return readLevelMode() === DEFAULT_LEVEL_MODE
  })(),
)
check(
  'and writing into storage that throws does not take the page down',
  (() => {
    try {
      writeLevelMode('everything')
      return true
    } catch {
      return false
    }
  })(),
)

// THE KEY IS THE OLD ONE AND THE LEGACY KEYS ARE LEFT ALONE. Renaming
// `fd.desk.levels` would have reset every viewer who ever touched the
// control; deleting the other two would strand anyone who rolls back.
{
  const entries = { 'fd.desk.levels': '["blocks"]', 'fd.desk.showHtf': '0', 'fd.desk.levelsSpent': '1' }
  globalThis.localStorage = storage(entries)
  writeLevelMode('live')
  check('the position is written back under the old key', entries['fd.desk.levels'] === 'live', entries['fd.desk.levels'])
  check(
    'and the two switches it replaced are left where they are',
    entries['fd.desk.showHtf'] === '0' && entries['fd.desk.levelsSpent'] === '1',
    JSON.stringify(entries),
  )
  check('what was written back reads back the same', readLevelMode() === 'live')
}
delete globalThis.localStorage

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

console.log('\n-- what each of the three positions draws --')

/**
 * THE THREE POSITIONS, COUNTED ON THE REAL RESPONSE. This is the block the
 * merge is worth anything by: one control, and a reader has to be able to
 * say what each press does. Every number here was measured on
 * `docs/api-samples/paper-levels.json` and every one of them is quoted in a
 * comment in `lib/levels.ts`.
 */
const off = selectLevels(levels, { mode: 'off' })
const live = selectLevels(levels, { mode: 'live' })
const all = selectLevels(levels, { mode: 'everything' })

check('off draws nothing at all', off.drawn.length === 0, `${off.drawn.length}`)
check('and says so as 252 held back by the switch', off.hiddenOff === 252, `${off.hiddenOff}`)
check(
  'off holds nothing back for distance or state — the switch is the only reason',
  off.hidden === 0 && off.hiddenSpent === 0 && off.hiddenFar === 0,
)
check('and the window is not in force when nothing is drawn', off.windowed === false)

check('the window is three ATR', LIVE_WINDOW_ATR === 3, `${LIVE_WINDOW_ATR}`)
// The measurement the constant's comment is written on. Widening it is
// allowed; letting the comment claim a number the response does not produce
// is not.
// 15 is the 13 live levels inside the window plus the profile's three marks,
// one of which (VAH at 1.77 ATR) was inside it anyway.
check('live draws 15 of the 252', live.drawn.length === 15, `${live.drawn.length}`)
check('197 are held back for being spent', live.hiddenSpent === 197, `${live.hiddenSpent}`)
check('40 more are held back for being further away', live.hiddenFar === 40, `${live.hiddenFar}`)
check(
  'nothing is lost between drawn, hidden and held back by the switch',
  live.drawn.length + live.hidden + live.hiddenOff === levels.length,
  `${live.drawn.length} + ${live.hidden} + ${live.hiddenOff} vs ${levels.length}`,
)
check('the window was applied', live.windowed === true)

// THE POSITION THE OWNER ASKED FOR IN SO MANY WORDS: "khi bật là sẽ hiển thị
// toàn bộ". No distance window, no spent filter, nothing held back for any
// reason — and 252 is nine times what the pane can label, which is why it is
// the third position and not the second.
check('everything draws all 252', all.drawn.length === 252, `${all.drawn.length}`)
check('everything holds nothing back, for any reason', all.hidden === 0 && all.hiddenOff === 0)
check(
  'everything draws every spent level the response carries',
  all.drawn.filter((l) => l.spent).length === 197,
  `${all.drawn.filter((l) => l.spent).length}`,
)
check(
  'and every level live draws, everything draws too — the positions nest',
  live.drawn.every((l) => all.drawn.includes(l)),
)
check(
  'the window is not in force at everything, and it is dropped rather than widened',
  all.windowed === false,
)
check(
  'so the distance window is nowhere in the everything set',
  all.drawn.some((l) => l.distAtr != null && Math.abs(l.distAtr) > LIVE_WINDOW_ATR),
)

// Six ATR was the alternative to the third position — a wider window instead
// of no window — and this is why it is not one: a 400px pane holds 28 tags
// at a 14px gap, and `stackTags` stops placing them at their own prices past
// that, so six ATR is already degraded and still is not "everything".
const wider = selectLevels(levels, { mode: 'live', windowAtr: 6 })
check('six ATR would draw 36, which is past what the pane holds', wider.drawn.length === 36, `${wider.drawn.length}`)
check('the pane holds 28 tags', TAGS_PER_PANE === 28, `${TAGS_PER_PANE}`)
check(
  'and the window option cannot reach everything, because spent is not a distance',
  selectLevels(levels, { mode: 'live', windowAtr: Infinity }).drawn.length === 55,
  `${selectLevels(levels, { mode: 'live', windowAtr: Infinity }).drawn.length}`,
)

/* ------------------------------------------- the one exemption, and why */

/**
 * THE PROFILE IS DRAWN WHATEVER ITS DISTANCE, and this is the check that says
 * so. It is a statement about a CATEGORY — the activity profile is one object
 * describing the whole window, so its distance from price is not a fact about
 * its relevance — and not a ranking within one, which is what the rest of
 * this file exists to refuse. Turning the profile's own switch on and being
 * shown one mark of three was the window contradicting the switch. That
 * switch is gone — there is one now — and the exemption is untouched by
 * that: it was never about which switch was on.
 */
const profileDrawn = live.drawn.filter((l) => l.family === 'profile')
check(
  'all three profile marks are drawn at live, at 1.8, 5.9 and 7.5 ATR away',
  profileDrawn.length === 3,
  `${profileDrawn.length} drawn`,
)
check(
  'and two of the three are far outside the window that drew them anyway',
  profileDrawn.filter((l) => Math.abs(l.distAtr) > LIVE_WINDOW_ATR).length === 2,
  profileDrawn.map((l) => `${l.kind} ${l.distAtr.toFixed(2)}`).join(' '),
)
check(
  'nothing else is exempt: every one of the 40 held back for distance is from another family',
  live.hiddenFar === 40 &&
    levels.filter(
      (l) => l.family !== 'profile' && !l.spent && l.distAtr != null && Math.abs(l.distAtr) > LIVE_WINDOW_ATR,
    ).length === 40,
)
check(
  'the note that explains the exemption says what the profile IS',
  PROFILE_NOTE.includes("window's own statistic") && PROFILE_NOTE.includes('not a price the market turned at'),
  PROFILE_NOTE,
)

/* ------------------------------------ the timeframe with no ATR at all */

/**
 * TEN TRADING DAYS OF 1d BARS IS TEN BARS, four short of what ATR(14) needs,
 * so the live route answers `?tf=1d` with `atr14: null`, no profile, and five
 * period pools. Every distance is then unmeasurable in ATR and the window
 * cannot be applied at all — which must draw everything live and SAY so,
 * never look like a market that went quiet.
 *
 * Built by blanking the two fields on the captured response rather than by
 * hand, so the shape under test is a real one.
 */
console.log('\n-- a response with no ATR --')

const thin = harvestLevels({ ...sample, atr14: null, profile: null })
const thinPick = selectLevels(thin, { mode: 'live' })
check('the profile is gone with it', thin.every((l) => l.family !== 'profile'))
check('no level can be placed in ATR', thin.every((l) => l.distAtr === null))
check('so the window says it was not applied', thinPick.windowed === false)
check('nothing is held back for distance', thinPick.hiddenFar === 0, `${thinPick.hiddenFar}`)
check(
  'and every live level is drawn',
  thinPick.drawn.length === thin.filter((l) => !l.spent).length,
  `${thinPick.drawn.length} of ${thin.filter((l) => !l.spent).length}`,
)
check(
  'the count still accounts for the spent ones',
  hiddenSentence(thinPick) === '197 further levels, already spent, not drawn',
  hiddenSentence(thinPick),
)
check(
  'distance in price survives without an ATR to scale it',
  thin.every((l) => typeof l.dist === 'number' && Number.isFinite(l.dist)),
)

/* ------------------------------ the same rules on a coarser timeframe */

/**
 * The 1h response is served by the route from the test fixture's seeded walk,
 * so its PRICES mean nothing — what it pins is that the same code reads a
 * non-default timeframe without a single number being rescaled by hand:
 * `age_bars` counts 1h bars, `atr14` is the 1h ATR, and the window is that
 * ATR times three. The counts are asserted so the comment in `levels.ts`
 * quoting them cannot drift away from the file.
 */
console.log('\n-- the same window on 1h --')

const hourly = JSON.parse(
  readFileSync(fileURLToPath(new URL('../../docs/api-samples/paper-levels-1h.json', import.meta.url)), 'utf8'),
)
const hourlyLevels = harvestLevels(hourly)
const hourlyPick = selectLevels(hourlyLevels, { mode: 'live' })
check('the 1h response carries 66 levels', hourlyLevels.length === 66, `${hourlyLevels.length}`)
check('45 of them are spent', censusOf(hourlyLevels).spent === 45, `${censusOf(hourlyLevels).spent}`)
check('6 are drawn: three live and the profile', hourlyPick.drawn.length === 6, `${hourlyPick.drawn.length}`)
check(
  'the ATR the window uses is the 1h one the response published',
  hourlyLevels.every((l) => l.distAtr == null || Math.abs(l.distAtr - l.dist / hourly.atr14) < 1e-9),
)

// THE SENTENCE ITSELF. It is the audit of the window and it is asserted
// verbatim, because a count that quietly stops being printed is exactly the
// silence this whole file is against.
check(
  'the hidden count reads like the prompt block',
  hiddenSentence(live) === '237 further levels, spent or further away, not drawn',
  hiddenSentence(live),
)
// The branch that names distance alone. Before the merge it was reached by
// the `spent on` switch with the window still in force — a view that drew
// 101 of the 252 and that the one control deliberately cannot express any
// more, because `everything` means everything. The branch itself is still
// live code and still reachable, on any response with nothing spent on it,
// so it keeps its verbatim test built that way.
const noneSpent = selectLevels(
  levels.filter((l) => !l.spent),
  { mode: 'live' },
)
check(
  'on a response with nothing spent, the count names distance alone',
  hiddenSentence(noneSpent) === '40 further levels, further than 3 ATR away, not drawn',
  hiddenSentence(noneSpent),
)
check(
  'a single hidden level is not pluralised',
  hiddenSentence({ hidden: 1, hiddenSpent: 1, hiddenFar: 0 }) === '1 further level, already spent, not drawn',
  hiddenSentence({ hidden: 1, hiddenSpent: 1, hiddenFar: 0 }),
)
check('nothing hidden says nothing', hiddenSentence({ hidden: 0, hiddenSpent: 0, hiddenFar: 0 }) === null)

// THE OTHER HALF OF THE DEBT, which `everything` owes. `hiddenSentence` says
// nothing there because nothing is hidden, and a position that draws 252
// tags into a pane that can place 28 must not be silent about it — a reader
// who chose the wall has to be able to read it as the wall they asked for.
check(
  'everything says what it is doing to the tags',
  crowdedSentence(all.drawn.length) ===
    'all 252 drawn, spent and distant included — past 28 tags the pane spaces them evenly, so a tag is no longer at its own price',
  crowdedSentence(all.drawn.length),
)
check(
  'and says nothing while the pane can still place them',
  crowdedSentence(TAGS_PER_PANE) === null && crowdedSentence(live.drawn.length) === null,
  `${crowdedSentence(live.drawn.length)}`,
)
check('one more than the pane holds is already worth saying', crowdedSentence(TAGS_PER_PANE + 1) !== null)

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
  check('15 levels are drawn as 14 lines', folded.length === 14, `${folded.length}`)
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

// AND AT `everything`, WHICH IS THE NUMBER OF LINES THE OWNER WILL ACTUALLY
// SEE. 252 levels fold onto 245 distinct prices — seven duplicate prices in
// the whole response, so the fold is not what makes this position legible
// and nothing should be tempted to lean on it for that. 245 lines against
// the 28 tags a pane can place is the crowd `crowdedSentence` names.
{
  const folded = foldSamePrice(
    all.drawn.map((l) => ({
      label: l.label,
      price: l.anchor,
      bandLow: l.bandLow,
      bandHigh: l.bandHigh,
      spent: l.spent,
    })),
  )
  check('all 252 are drawn as 245 lines', folded.length === 245, `${folded.length}`)
  check(
    'and the fold still loses no level: every line names at least one',
    folded.length <= all.drawn.length && folded.every((f) => f.label.length > 0),
  )
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
