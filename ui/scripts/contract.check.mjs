/**
 * Does the server still send what this client reads?
 *
 * WHY THIS EXISTS. Three times in one day a client and a route agreed on a
 * list of field NAMES and disagreed about the shape underneath them. The
 * worst was `exported_at_ms`: the contract put it beside `bars`, the route put
 * it inside `source`, the client read `undefined` forever, and the chart's
 * export age simply never rendered. No error, no wrong number — a fact
 * quietly missing from a caption that looked finished.
 *
 * The fix is mechanical, not more care. The side that writes the route commits
 * a real served response; this side declares the paths it dereferences; a
 * script diffs them. Neither party has to be alert.
 *
 * TWO RULES LEARNED THE HARD WAY, both encoded below:
 *
 *   - Compare PATHS AND TYPES, NEVER VALUES. A check that reddens because
 *     gold moved is one people switch off, and a switched-off check is worse
 *     than no check because its presence is taken as evidence.
 *   - An empty container is PRESENT. d1's first key-path dump skipped empty
 *     objects, so `unavailable_by_tf: {}` and a missing `unavailable_by_tf`
 *     printed identically — the checker was wrong before the code was. A
 *     checker that goes quiet on an empty container cannot tell it from an
 *     absent one, and that distinction is the whole point here.
 *
 *     node scripts/contract.check.mjs
 */

import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../docs/api-samples/', import.meta.url))

/**
 * `null` is a value, not an absence. `?` marks a path this client tolerates
 * being missing entirely (an older server); everything else must be present,
 * even if its value is null.
 */
const CONTRACTS = [
  {
    sample: 'htf-three-timeframes.json',
    what: 'GET /api/paper/htf — all three timeframes present',
    paths: {
      market: 'string',
      h1: 'object|null',
      h4: 'object|null',
      d1: 'object|null',
      h1_source: 'object|null',
      h4_source: 'object|null',
      'h4_source.file': 'string',
      'h4_source.timeframe': 'string',
      // The one that must never be `null`: the route sends `{}` when nothing
      // is missing, so the client can index it without a guard. `?` because
      // an API older than this client omits it altogether.
      'unavailable_by_tf?': 'object',
      unavailable: 'string|null',
      // Everything the card and the strip actually dereference on a block.
      'h1.timeframe': 'string',
      'h1.bar_ms': 'number',
      'h1.computed_at_bar_ms': 'number',
      'h1.structure.label': 'string',
      'h1.structure.rule': 'string',
      'h1.structure.confirmed_at_bar_ms': 'number|null',
      'h1.structure.break_level': 'number|null',
      'h1.structure.break_side': 'string|null',
      'h1.ema21': 'number|null',
      'h1.atr14': 'number|null',
      'h1.dist_ema21_atr': 'number|null',
      'h1.adx14': 'number|null',
      'h1.efficiency_20': 'number|null',
      'h4.structure.label': 'string',
      'h4.structure.last_high': 'object|null',
      'h4.structure.prior_high': 'object|null',
      'h4.structure.last_low': 'object|null',
      'h4.structure.prior_low': 'object|null',
      'h4.ema21': 'number|null',
      'h4.ema55': 'number|null',
      'h4.ema21_slope_sign': 'number|null',
      'h4.ema55_slope_sign': 'number|null',
      'h4.plus_di14': 'number|null',
      'h4.minus_di14': 'number|null',
      'h4.donchian20.bars_since_new_high': 'number|null',
      'h4.donchian20.bars_since_new_low': 'number|null',
      'h4.last_close': 'number|null',
      'd1.prior_day_high': 'number|null',
      'd1.prior_day_low': 'number|null',
      // The chart plots these three as price lines from 2026-09-19. They
      // were served and read by nothing before that, so nothing would have
      // noticed the route renaming them; a level that stops arriving must
      // fail here rather than quietly stop being drawn.
      'd1.prior_week_high': 'number|null',
      'd1.prior_week_low': 'number|null',
      'd1.prior_week_mid': 'number|null',
      'd1.close_pct_of_prior_week_range': 'number|null',
    },
    /** Asserted about the SHAPE of a container, never about its contents. */
    also: (doc, fail) => {
      if (doc.unavailable_by_tf !== undefined && doc.unavailable_by_tf === null) {
        fail('unavailable_by_tf is null; the route sends {} when nothing is missing')
      }
    },
  },
  {
    sample: 'htf-h1-missing.json',
    what: 'GET /api/paper/htf — the mixed state, which is the launch state',
    paths: {
      h1: 'object|null',
      h1_source: 'object|null',
      'unavailable_by_tf?': 'object',
      unavailable: 'string|null',
      'h4.structure.label': 'string',
    },
    also: (doc, fail) => {
      // The card renders an H1 row from `h1` and its reason from the keyed
      // sentence. If a block is null the matching key must be there, or the
      // row says "—" with no reason and the reader is sent nowhere.
      if (doc.h1 === null && !(doc.unavailable_by_tf && '1h' in doc.unavailable_by_tf)) {
        fail('h1 is null but unavailable_by_tf has no "1h" key — the row would have no reason')
      }
    },
  },
  {
    sample: 'paper-levels.json',
    what: 'GET /api/paper/levels — the price-bar levels, every family present',
    paths: {
      market: 'string',
      timeframe: 'string',
      bar_ms: 'number',
      computed_at_bar_ms: 'number|null',
      last_close: 'number|null',
      // The denominator of every `*_atr` below. A ratio whose denominator is
      // missing from the wire cannot be checked by anybody.
      atr14: 'number|null',
      unavailable: 'string|null',
      'source.file': 'string',
      'source.bars': 'number',
      'source.timeframe': 'string',
      'window.bars': 'number',
      'window.days': 'number',
      'window.profile_days': 'number',
      'profile.measure': 'string',
      'profile.bucket_size_price': 'number',
      'profile.buckets_per_atr': 'number',
      'profile.value_area_pct': 'number',
      'profile.activity_total_bar_buckets': 'number',
      'profile.activity_in_value_area_bar_buckets': 'number',
      'profile.poc.kind': 'string',
      'profile.poc.price': 'number|null',
      'profile.poc.band_low': 'number|null',
      'profile.poc.formed_at_bar_ms': 'number',
      'profile.poc.age_bars': 'number',
      'profile.poc.state': 'string',
      'profile.poc.rule': 'string',
      // THE ONE LEVEL THAT CARRIES BOTH A PRICE AND A BAND. The chart draws
      // the POC's price and `levelBands` draws the bucket it sits in, so the
      // far edge is dereferenced too — nothing read the band at all before
      // the levels went on the chart on 2026-09-19.
      'profile.poc.band_high': 'number|null',
      'profile.vah.price': 'number|null',
      'profile.vah.kind': 'string',
      'profile.vah.state': 'string',
      'profile.vah.age_bars': 'number',
      'profile.vah.formed_at_bar_ms': 'number',
      'profile.vah.rule': 'string',
      'profile.val.price': 'number|null',
      'profile.val.kind': 'string',
      'profile.val.state': 'string',
      'profile.val.age_bars': 'number',
      // THE NESTING THAT MATTERS. Each family is FLATTENED onto its level:
      // `kind` and the rest sit at the item's own top level, not under a
      // `level` key. Written as paths because nesting is the thing prose got
      // wrong three times in one day.
      'fair_value_gaps[0].kind': 'string',
      'fair_value_gaps[0].price': 'null',
      'fair_value_gaps[0].band_low': 'number',
      'fair_value_gaps[0].band_high': 'number',
      'fair_value_gaps[0].age_bars': 'number',
      'fair_value_gaps[0].state': 'string',
      'fair_value_gaps[0].rule': 'string',
      'fair_value_gaps[0].direction': 'string',
      'fair_value_gaps[0].filled_fraction': 'number',
      'fair_value_gaps[0].confirmed_at_bar_ms': 'number',
      // The bar the gap formed on, which the panel prints and the age is
      // counted from. `age_bars` alone cannot date it on screen.
      'fair_value_gaps[0].formed_at_bar_ms': 'number',
      'order_blocks[0].kind': 'string',
      'order_blocks[0].band_low': 'number',
      // A BAND IS TWO PRICES. The chart fills between them, so a route that
      // stopped sending the far edge would draw a block as a line with
      // nothing failing — which is what this file exists to prevent.
      'order_blocks[0].band_high': 'number',
      'order_blocks[0].price': 'null',
      'order_blocks[0].age_bars': 'number',
      'order_blocks[0].formed_at_bar_ms': 'number',
      'order_blocks[0].rule': 'string',
      'order_blocks[0].state': 'string',
      'order_blocks[0].direction': 'string',
      'order_blocks[0].displacement_at_bar_ms': 'number',
      'order_blocks[0].displacement_body_atr': 'number',
      'order_blocks[0].tested_at_bar_ms': 'number|null',
      'order_blocks[0].broken_at_bar_ms': 'number|null',
      // THE FOURTH STATE'S OWN STAMP, dereferenced since the chart started
      // drawing a breaker with a dash of its own on 2026-09-19. A route that
      // dropped it would leave a block labelled BROKEN, BACK INSIDE with no
      // bar anywhere saying when it came back inside.
      'order_blocks[0].breaker_retested_at_bar_ms': 'number|null',
      // THE SPINE. Nothing dereferenced these until the chart drew them.
      // Written out as paths for the reason the families above are: nesting
      // is the thing prose got wrong three times in one day, and `measured`
      // is a nested object whose numbers are printed one at a time.
      'market_structure.label': 'string',
      'market_structure.label_since_bar_ms': 'number|null',
      'market_structure.rule': 'string',
      'market_structure.events': 'array',
      'market_structure.unclassified_breaks': 'number',
      'market_structure.events[0].kind': 'string',
      'market_structure.events[0].direction': 'string',
      'market_structure.events[0].broke_price': 'number',
      'market_structure.events[0].broke_swing_id': 'string',
      'market_structure.events[0].broke_swing_bar_ms': 'number',
      'market_structure.events[0].closed_at_bar_ms': 'number',
      'market_structure.events[0].close': 'number',
      'market_structure.events[0].age_bars': 'number',
      // The flag the chart subordinates 79 of these 80 marks by. A route
      // that stopped sending it would draw every event at full weight with
      // nothing failing — the unreadable chart the flag exists to prevent.
      'market_structure.events[0].superseded': 'boolean',
      'market_structure.events[0].structure_after': 'string',
      'market_structure.events[0].rule': 'string',
      // THE CAVEAT'S OWN NUMBERS, which the panel prints as visible text
      // beside the marks. The client says nothing rather than inventing them
      // when they are missing, so this is what "nothing" would cost.
      'market_structure.measured.source': 'string',
      'market_structure.measured.choch_median_lag_bars': 'number',
      'market_structure.measured.choch_p90_lag_bars': 'number',
      'market_structure.measured.fractal_label_median_lag_bars': 'number',
      'market_structure.measured.zigzag_p90_lag_bars': 'number',
      'market_structure.measured.choch_absent_at_turns_pct': 'number',
      'market_structure.measured.zigzag_missed_turns_pct': 'number',
      'market_structure.measured.broken_back_within_10_bars_pct': 'number',
      'market_structure.measured.note': 'string',
      'dealing_range.high': 'number',
      'dealing_range.low': 'number',
      'dealing_range.equilibrium': 'number',
      'dealing_range.high_swing_id': 'string',
      'dealing_range.low_swing_id': 'string',
      'dealing_range.high_bar_ms': 'number',
      'dealing_range.low_bar_ms': 'number',
      // UNCLAMPED, and the `also` below asserts the route has not quietly
      // started clamping it: on the 1h response it is 5.15, and a client
      // shown 1.0 would read a market pinned to the top of a range it left.
      'dealing_range.close_fraction_of_range': 'number',
      'dealing_range.close_zone': 'string',
      'dealing_range.rule': 'string',
      'liquidity[0].kind': 'string',
      'liquidity[0].side': 'string',
      'liquidity[0].swept': 'boolean',
      'liquidity[0].swept_at_bar_ms': 'number|null',
      'liquidity[0].swing_ids': 'array',
      'liquidity[0].spread_atr': 'number|null',
      'liquidity[0].state': 'string',
      'liquidity[0].rule': 'string',
      // A POOL IS A PRICE AND A SPREAD. The line goes at the price and the
      // band shows how far the swings in it are apart; the prior-day and
      // prior-week pools are the ones with no band, which is why both edges
      // are `number|null` here and neither is assumed from the other.
      'liquidity[0].price': 'number|null',
      'liquidity[0].band_low': 'number|null',
      'liquidity[0].band_high': 'number|null',
      'liquidity[0].age_bars': 'number',
      'liquidity[0].formed_at_bar_ms': 'number',
      'extremes.session.high.kind': 'string',
      'extremes.session.high.state': 'string',
      'extremes.session.bars': 'number',
      // The six period extremes are drawn and read out like every other
      // level from 2026-09-19, so the fields a row needs are declared for
      // one high and one low rather than for the price alone.
      'extremes.session.high.price': 'number|null',
      'extremes.session.high.age_bars': 'number',
      'extremes.session.high.formed_at_bar_ms': 'number',
      'extremes.session.high.rule': 'string',
      'extremes.session.low.price': 'number|null',
      'extremes.session.low.state': 'string',
      'extremes.session.low.age_bars': 'number',
      'extremes.day.high.price': 'number|null',
      'extremes.day.high.kind': 'string',
      'extremes.day.high.state': 'string',
      'extremes.day.high.age_bars': 'number',
      'extremes.day.low.price': 'number|null',
      'extremes.week.high.price': 'number|null',
      'extremes.week.high.state': 'string',
      'extremes.week.low.price': 'number|null',
      'extremes.week.start_bar_ms': 'number|null',
    },
    also: (doc, fail) => {
      // `swept` and `swept_at_bar_ms` are ONE fact. A renderer will test
      // whichever is handier, and the two disagreeing shows up as a level
      // drawn struck-through with no bar beside it.
      for (const p of doc.liquidity ?? []) {
        if (p.swept !== (p.swept_at_bar_ms !== null)) {
          fail(`a liquidity pool has swept=${p.swept} and swept_at_bar_ms=${p.swept_at_bar_ms}`)
        }
      }
      // A filled gap is not a gap. The route drops them, so a 1.0 here means
      // the rule moved and every caption reading "unfilled" became wrong.
      for (const g of doc.fair_value_gaps ?? []) {
        if (!(g.filled_fraction >= 0 && g.filled_fraction < 1)) {
          fail(`a fair value gap reports filled_fraction ${g.filled_fraction}; only unfilled gaps are served`)
        }
      }
      // AT MOST ONE EVENT IS NOT SUPERSEDED, AND IT IS THE LAST ONE. The
      // route's definition makes it so — superseded means a later event
      // exists — and `liveStructureEvent` in lib/levels.ts returns a single
      // event rather than a list because of it. If the route ever served two
      // live events the chart would draw two full-weight marks and call both
      // of them the structure as it stands, so the invariant is asserted
      // here rather than trusted.
      {
        const events = doc.market_structure?.events ?? []
        const live = events.filter((e) => e.superseded === false)
        if (live.length > 1) fail(`${live.length} events are not superseded; at most one can be`)
        if (live.length === 1 && events[events.length - 1] !== live[0]) {
          fail('the event that is not superseded is not the last one in the list')
        }
      }
      // THE RANGE FRACTION IS NOT CLAMPED, and a route that started clamping
      // it would turn a breakout into a ceiling with nothing failing — the
      // field's own comment says so. Only the ORDERING of the two prices is
      // asserted, because that is what makes the fraction meaningful at all:
      // a high at or below its low is a divide by zero wearing a range's name.
      if (doc.dealing_range) {
        const { high, low, equilibrium, close_fraction_of_range: f, close_zone: zone } = doc.dealing_range
        if (!(high > low)) fail(`the dealing range has high ${high} and low ${low}`)
        if (Math.abs(equilibrium - (high + low) / 2) > 1e-6) {
          fail(`the equilibrium ${equilibrium} is not the midpoint of ${low}..${high}`)
        }
        if (doc.last_close != null && Math.abs(f - (doc.last_close - low) / (high - low)) > 1e-9) {
          fail(`close_fraction_of_range ${f} is not (close - low) / (high - low) — has it been clamped?`)
        }
        if (!['PREMIUM', 'DISCOUNT', 'EQUILIBRIUM'].includes(zone)) fail(`close_zone is ${zone}`)
      }
      // A BREAKER IS A BROKEN BLOCK THAT CAME BACK, so it carries both
      // stamps; nothing else carries the second one. The chart draws the two
      // at one weight and two dashes, and that only says something true
      // while the states mean what they say.
      for (const b of doc.order_blocks ?? []) {
        if (b.state === 'BREAKER' && (b.broken_at_bar_ms == null || b.breaker_retested_at_bar_ms == null)) {
          fail(`a BREAKER block has broken_at ${b.broken_at_bar_ms} and retested_at ${b.breaker_retested_at_bar_ms}`)
        }
        if (b.state !== 'BREAKER' && b.breaker_retested_at_bar_ms != null) {
          fail(`a ${b.state} block carries a breaker retest stamp`)
        }
      }
      // NO VERDICT. `docs/hypotheses/2026-09-18-smc-context.md` pre-commits
      // the route to carrying none, and the book that reads it must not read
      // one. Cheaper to fail here than to notice a "score" inside a prompt.
      const text = JSON.stringify(doc)
      for (const banned of ['"score"', '"composite"', '"confluence"', '"rank"', '"strength"']) {
        if (text.includes(banned)) fail(`the levels route grew ${banned}; the registration forbids a verdict`)
      }
    },
  },
  {
    sample: 'paper-levels-1h.json',
    what: 'GET /api/paper/levels?tf=1h — the client now asks for the timeframe on screen',
    paths: {
      // THE THREE THAT MUST TRACK THE REQUEST. The chart follows its own
      // timeframe from 2026-09-19, and every `age_bars` and `*_atr` on the
      // response is in units of the series the route read — so a response
      // that answered 1h with 15m's `bar_ms` would caption the panel in one
      // unit and draw the chart in another, with nothing failing.
      timeframe: 'string',
      bar_ms: 'number',
      atr14: 'number|null',
      last_close: 'number|null',
      'window.bars': 'number',
      'window.days': 'number',
      'source.timeframe': 'string',
      'profile.poc.age_bars': 'number',
      'liquidity[0].age_bars': 'number',
      'liquidity[0].swept': 'boolean',
      'fair_value_gaps[0].band_low': 'number',
      'extremes.session.high.price': 'number|null',
    },
    also: (doc, fail) => {
      if (doc.timeframe !== '1h') fail(`asked for 1h and the response says ${doc.timeframe}`)
      if (doc.bar_ms !== 3_600_000) fail(`1h bar_ms is ${doc.bar_ms}, not 3600000`)
      if (doc.source && doc.source.timeframe !== doc.timeframe) {
        fail(`the file read is ${doc.source.timeframe} under a response saying ${doc.timeframe}`)
      }
      // Ten TRADING DAYS on every timeframe, which is a different number of
      // bars on each: 879 on 15m, 230 here, 10 on 1d. A client reading only
      // `bars` cannot tell "ten days of 1h" from "ten bars of 1h", and the
      // panel prints both for that reason.
      if (doc.window && doc.window.days !== 10) {
        fail(`the analysis window is ${doc.window.days} trading days, not the documented 10`)
      }
    },
  },
  {
    sample: 'paper-levels-unavailable.json',
    what: 'GET /api/paper/levels — no exported bars, which is NOT an empty list',
    paths: {
      unavailable: 'string',
      profile: 'null',
      fair_value_gaps: 'null',
      order_blocks: 'null',
      liquidity: 'null',
      extremes: 'null',
      source: 'null',
      window: 'null',
      // Known without a bar, so still answered: a card can label itself.
      market: 'string',
      timeframe: 'string',
      bar_ms: 'number',
    },
    also: (doc, fail) => {
      // The distinction the whole branch exists for. `[]` says "this window
      // has no unfilled gaps", which is a measurement; `null` says nothing
      // was measured. A client that cannot tell them apart renders "no gaps"
      // over a market it has never looked at.
      for (const key of ['fair_value_gaps', 'order_blocks', 'liquidity']) {
        if (Array.isArray(doc[key])) fail(`${key} is [] with no bars; it must be null`)
      }
    },
  },
  {
    sample: 'chart-bars-4h-forming.json',
    what: 'GET /api/chart/bars — file-backed 4h with a partial bar',
    paths: {
      'bars[0].time': 'number',
      'bars[0].open': 'number',
      bar_ms: 'number',
      last_closed_bar_ms: 'number|null',
      // THE ONE THAT STARTED ALL THIS. The agreed contract put it beside
      // `bars`; the route puts it inside `source`. The client read the wrong
      // level and the chart's export age never rendered — silently, because
      // the caption clause was conditional.
      'source.file': 'string',
      'source.timeframe': 'string',
      'source.resampled': 'boolean',
      'source.exported_at_ms': 'number|null',
      'forming.time': 'number',
      'forming.open': 'number',
      'forming.high': 'number',
      'forming.low': 'number',
      'forming.close': 'number',
      'forming.from_timeframe': 'string',
      // How far into the partial bar the extremes are actually KNOWN. The
      // caption states it; without it a fifteen-minute-old wick would read as
      // the current high.
      'forming.complete_to_ms': 'number',
    },
    also: (doc, fail) => {
      // Not a value assertion — a SHAPE one. The forming bar must begin where
      // the closed series ends, or the client is drawing a candle that
      // belongs to a different grid than the bars behind it.
      if (doc.forming && doc.last_closed_bar_ms != null && doc.bar_ms) {
        if (doc.forming.time !== doc.last_closed_bar_ms + doc.bar_ms) {
          fail('forming.time is not last_closed_bar_ms + bar_ms — the partial bar is off-grid')
        }
      }
    },
  },
  {
    sample: 'chart-bars-1d-refused.json',
    what: 'GET /api/chart/bars — the refusal, which the client renders as words',
    paths: { error: 'string' },
    also: (doc, fail) => {
      // The client throws on any body carrying `error`, so a refusal that
      // ALSO carried bars would be discarded rather than drawn. Worth
      // knowing if that ever changes.
      if ('bars' in doc) fail('a refusal carries `bars`; the client throws on `error` and would drop them')
    },
  },
]

const typeOf = (v) => {
  if (v === null) return 'null'
  if (Array.isArray(v)) return 'array'
  return typeof v
}

/**
 * Walks a dotted path, with `name[0]` for "the first element".
 *
 * Distinguishes "absent" from "present and null", which is the entire reason
 * this is hand-written rather than an optional chain: `a?.b` returns undefined
 * for both, and those two are exactly what a contract has to tell apart.
 */
function resolve(doc, path) {
  let node = doc
  for (const part of path.split('.')) {
    const match = /^([^[]+)\[(\d+)\]$/.exec(part)
    const key = match ? match[1] : part
    if (node === null || node === undefined) return { absent: true }
    if (!Object.prototype.hasOwnProperty.call(node, key)) return { absent: true }
    node = node[key]
    if (match) {
      const index = Number(match[2])
      if (!Array.isArray(node)) return { absent: true, note: `${key} is not an array` }
      // An empty array is PRESENT and cannot satisfy an indexed path — said
      // rather than silently passing, for the same reason an empty object is
      // not an absent one.
      if (node.length <= index) return { absent: true, note: `${key} has no [${index}]` }
      node = node[index]
    }
  }
  return { absent: false, value: node }
}

let failures = 0
let checked = 0

for (const contract of CONTRACTS) {
  const file = root + contract.sample
  console.log(`\n-- ${contract.what}`)

  if (!existsSync(file)) {
    // LOUD, not quiet. A missing sample must never be mistaken for a passing
    // one — that is the same mistake as a walker skipping empty containers.
    console.log(`SKIP  ${contract.sample} is not in docs/api-samples/ — nothing was checked here.`)
    continue
  }

  const doc = JSON.parse(readFileSync(file, 'utf8'))
  const fail = (message) => {
    failures += 1
    console.log(`FAIL  ${message}`)
  }

  for (const [spec, expected] of Object.entries(contract.paths)) {
    const optional = spec.endsWith('?')
    const path = optional ? spec.slice(0, -1) : spec
    const found = resolve(doc, path)
    checked += 1

    if (found.absent) {
      if (optional) console.log(`ok    ${path} absent (tolerated: older server)`)
      else fail(`${path} is ABSENT from ${contract.sample}; this client dereferences it`)
      continue
    }

    const actual = typeOf(found.value)
    const allowed = expected.split('|')
    if (!allowed.includes(actual)) {
      fail(`${path} is ${actual}, expected ${expected}`)
    } else {
      const note = actual === 'object' && Object.keys(found.value).length === 0 ? ' (empty, but PRESENT)' : ''
      console.log(`ok    ${path}: ${actual}${note}`)
    }
  }

  contract.also?.(doc, fail)
}

console.log(
  failures === 0
    ? `\nall ${checked} contract paths hold`
    : `\n${failures} FAILED of ${checked} paths`,
)
process.exit(failures === 0 ? 0 : 1)
