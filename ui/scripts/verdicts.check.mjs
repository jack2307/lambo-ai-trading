/**
 * Checks that every badge in `src/lib/verdicts.ts` still matches the record.
 *
 * THE LAST CHECK IS THE WHOLE POINT. A badge is a number lifted out of a
 * research document, and research documents on this desk get amended — the
 * doji's matched null was recomputed and moved a row from the 100th
 * percentile to the 90th, the ORB's from the 100th to the 92nd, Volman's box
 * from the 1st percentile to the 38th once it was equal-weighted in R. Each
 * of those corrections would have left a badge quietly asserting the old
 * number. So every numeric token in a `line` is searched for, literally, in
 * the file that `line` cites; if the record is amended and the badge is not,
 * this fails.
 *
 * The other three failures are cheaper but not softer:
 *   - a cited registration or decision that is not on disk (a renamed or
 *     deleted record leaves a badge citing nothing),
 *   - a `line` over the badge budget (it would be clipped, and a clipped
 *     verdict is a different verdict),
 *   - an id that is neither a known indicator nor a known strategy (a badge
 *     nothing on the chart can ever show, or worse, a typo that means the
 *     real id resolves to null and renders as a reassuring blank).
 *
 * The id lists are parsed out of the Rust sources rather than copied, because
 * a copy is a third place to forget.
 *
 *     node --experimental-strip-types scripts/verdicts.check.mjs
 */

import { readFileSync, existsSync, readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

import { VERDICTS, DELIBERATE_NULLS, verdictFor } from '../src/lib/verdicts.ts'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPO = path.resolve(HERE, '../..')

/**
 * The badge budget, in characters.
 *
 * The brief asks for about sixty; sixty-four is the point at which the badge
 * still fits beside an indicator name in the chart's legend at phone width.
 * It is a hard stop rather than an ellipsis because half a verdict reads as a
 * whole one.
 */
const LINE_BUDGET = 64

let failures = 0
const check = (name, ok, detail) => {
  if (!ok) failures += 1
  console.log(`${ok ? 'ok  ' : 'FAIL'}  ${name}`)
  if (!ok && detail) console.log(`        ${detail}`)
}

const read = (rel) => readFileSync(path.join(REPO, rel), 'utf8')

/* ---------------- the id lists, parsed from the code they describe ---------------- */

/** Every id in `INDICATORS`, in declaration order. */
function indicatorIds() {
  const src = read('crates/fd-indicators/src/lib.rs')
  const start = src.indexOf('pub static INDICATORS')
  if (start < 0) throw new Error('INDICATORS table not found in fd-indicators/src/lib.rs')
  const end = src.indexOf('\n];', start)
  const block = src.slice(start, end)
  return [...block.matchAll(/id:\s*"([a-z0-9_-]+)"/g)].map((m) => m[1])
}

/**
 * Every id a shipped `Strategy` impl returns.
 *
 * Each file is cut at its `#[cfg(test)]` first. `filter.rs` defines two test
 * doubles, `always` and `exit-now`, and a badge for either of those would be
 * a verdict on something no chart can ever show.
 */
function strategyIds() {
  const dir = path.join(REPO, 'crates/fd-strategy/src')
  const ids = new Set()
  for (const file of readdirSync(dir)) {
    if (!file.endsWith('.rs')) continue
    const whole = readFileSync(path.join(dir, file), 'utf8')
    const cut = whole.indexOf('#[cfg(test)]')
    const src = cut < 0 ? whole : whole.slice(0, cut)
    for (const m of src.matchAll(/fn id\(&self\)\s*->\s*&'static str\s*\{\s*"([a-z0-9-]+)"/g)) {
      ids.add(m[1])
    }
  }
  return [...ids]
}

const INDICATOR_IDS = indicatorIds()
const STRATEGY_IDS = strategyIds()
const KNOWN = new Map([
  ...INDICATOR_IDS.map((id) => [id, 'indicator']),
  ...STRATEGY_IDS.map((id) => [id, 'strategy']),
])

console.log(`-- ${INDICATOR_IDS.length} indicator ids, ${STRATEGY_IDS.length} strategy ids, ${VERDICTS.length} verdicts --\n`)

/* ---------------- 1. every id is one the chart can actually show ---------------- */

console.log('-- ids --')
for (const v of VERDICTS) {
  const kind = KNOWN.get(v.id)
  check(
    `${v.id} is a known ${v.kind} id`,
    kind === v.kind,
    kind === undefined
      ? `${v.id} is neither an indicator id nor a strategy id`
      : `declared ${v.kind}, the code says ${kind}`,
  )
}

const seen = new Set()
for (const v of VERDICTS) {
  check(`${v.id} appears once`, !seen.has(v.id), 'duplicate row')
  seen.add(v.id)
}

// An indicator the chart can draw with no row here would render blank, which
// is the implicit recommendation this whole file exists to prevent. A null is
// allowed only where `DELIBERATE_NULLS` says so out loud.
console.log('\n-- every indicator resolves --')
for (const id of INDICATOR_IDS) {
  const v = verdictFor(id)
  const deliberate = DELIBERATE_NULLS.includes(id)
  check(
    `${id} ${deliberate ? 'is a declared null' : 'resolves to a verdict'}`,
    deliberate ? v === null : v !== null,
    deliberate ? 'declared null but a verdict exists' : 'no verdict and not in DELIBERATE_NULLS',
  )
}
check('an unknown id returns null', verdictFor('not-an-indicator') === null)

/* ---------------- 2. the badge fits ---------------- */

console.log('\n-- badge budget --')
for (const v of VERDICTS) {
  check(`${v.id}: line is ${v.line.length} <= ${LINE_BUDGET}`, v.line.length <= LINE_BUDGET, v.line)
}

/* ---------------- 3. everything cited is on disk ---------------- */

/** Every repository path a row points at: its registration, plus any document named in the detail. */
function citations(v) {
  const found = new Set()
  if (v.registration) found.add(v.registration)
  for (const m of v.detail.matchAll(/(?:docs|crates|py|scripts|config)\/[\w./-]+\.(?:md|txt|rs|py)/g)) {
    found.add(m[0])
  }
  return [...found]
}

console.log('\n-- cited files exist --')
for (const v of VERDICTS) {
  for (const rel of citations(v)) {
    check(`${v.id} -> ${rel}`, existsSync(path.join(REPO, rel)), 'not on disk')
  }
}

/* ---------------- 4. every number in a badge is in the record ---------------- */

/**
 * Numeric tokens as a reader would say them: `1.052`, `533`, `96` out of
 * `96th`, `0.05` out of `0.05R`. Years inside an id or a filename never reach
 * here because only `line` is scanned, and a `line` has no filenames in it.
 */
const numbersIn = (line) => [...line.matchAll(/\d+(?:\.\d+)?/g)].map((m) => m[0])

/**
 * The record's own text, with thousands separators removed.
 *
 * The documents write `5,766 trades` and a badge writes `5766`; they are the
 * same number and the check must not turn that into a failure. Nothing else
 * is normalised — a badge that rounds 1.052 to 1.05 is a badge that stopped
 * quoting the record, and it should fail.
 */
const corpusCache = new Map()
function corpus(rel) {
  if (!corpusCache.has(rel)) {
    const text = existsSync(path.join(REPO, rel)) ? read(rel) : ''
    corpusCache.set(rel, text.replace(/(\d),(?=\d{3}\b)/g, '$1'))
  }
  return corpusCache.get(rel)
}

/**
 * Is this exact number in the record?
 *
 * A PLAIN SUBSTRING SEARCH IS NOT ENOUGH, and the negative test proves it:
 * `1.05` is a substring of `1.052`, so a badge that quietly rounded the
 * Keltner row would have sailed through. The number must therefore not be
 * preceded by a digit or a decimal point, and must not be the head of a
 * longer number — `78` may match `78th` and `78%`, never `780` or `0.78`,
 * and `1` may match `1st` but not `1.052`.
 */
function inRecord(haystack, token) {
  const escaped = token.replace('.', '\\.')
  return new RegExp(`(?<![\\d.])${escaped}(?!\\d|\\.\\d)`).test(haystack)
}

console.log('\n-- every number in a line is in a file it cites --')
for (const v of VERDICTS) {
  const wanted = numbersIn(v.line)
  if (wanted.length === 0) {
    check(`${v.id}: no numbers to verify`, true)
    continue
  }
  const cited = citations(v)
  if (cited.length === 0) {
    check(`${v.id}: cites a document for its numbers`, false, `line has ${wanted.join(', ')} and cites nothing`)
    continue
  }
  const haystack = cited.map(corpus).join('\n')
  const missing = wanted.filter((n) => !inRecord(haystack, n))
  check(
    `${v.id}: ${wanted.length} number(s) found in ${cited.length} cited file(s)`,
    missing.length === 0,
    missing.length ? `not in the record: ${missing.join(', ')} — cited ${cited.join(', ')}` : undefined,
  )
}

/* ---------------- 5. the statuses say what they mean ---------------- */

console.log('\n-- statuses --')
const STATUSES = ['killed', 'open', 'parked', 'untested']
for (const v of VERDICTS) {
  check(`${v.id}: status is one of ${STATUSES.join('/')}`, STATUSES.includes(v.status), v.status)
  // An open registration must name the registration that is open, or the
  // badge is claiming a running experiment it cannot point at.
  if (v.status === 'open') {
    check(`${v.id}: an open row names its registration`, v.registration !== null)
  }
  // A killed row must cite something; "closed" with no record behind it is
  // the same unsupported claim as a blank badge, pointed the other way.
  if (v.status === 'killed') {
    check(`${v.id}: a killed row cites a record`, citations(v).length > 0)
  }
}

console.log(failures === 0 ? '\nall verdict checks pass' : `\n${failures} FAILED`)
process.exit(failures === 0 ? 0 : 1)
