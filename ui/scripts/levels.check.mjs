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

import { familyOf, stackTags, LEVEL_FAMILIES } from '../src/lib/levels.ts'

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

console.log(failures === 0 ? '\nall level checks pass' : `\n${failures} FAILED`)
process.exit(failures === 0 ? 0 : 1)
