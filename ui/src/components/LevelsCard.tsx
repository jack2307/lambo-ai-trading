import { useEffect, useMemo, useState } from 'react'

import type { PriceLevelsResponse } from '@/lib/api'
import { clock, since } from '@/lib/format'
import {
  LEVEL_FAMILIES,
  LIVE_WINDOW_ATR,
  PROFILE_NOTE,
  censusOf,
  harvestLevels,
  type DeskLevel,
} from '@/lib/levels'
import { cn } from '@/lib/utils'

/**
 * The price levels, READ OUT.
 *
 * The chart says WHERE a level is; this says WHAT it is. A line on a canvas
 * cannot carry the half of a level that is not its price — how old it is,
 * what state it is in, whether the pool under it has already been swept and
 * when — and those are the facts that decide whether a reader cares about it
 * at all. So the two halves sit side by side and read one response.
 *
 * SAME SHAPE AS THE PROMPT BLOCK, DELIBERATELY. `py/live/smc_context.py`
 * renders these same levels for the model: nearest to the last close first,
 * above and below listed separately, at most six a side, a census of the
 * whole response in the header and a count of what the cap left out. This
 * panel does the same thing in the same order with the same words, because
 * the model and the screen reading one route differently is the fault this
 * desk keeps writing tests against — and when a book explains itself by
 * naming a level, the person checking it needs to find that level here, in
 * those words, without translating.
 *
 * IT CARRIES NO VERDICT, and that is a pre-commitment rather than a taste.
 * No score, no ranking, no confluence count, no zone labelled with a word:
 * `docs/hypotheses/2026-09-18-smc-context.md` was registered before the route
 * existed and says so in advance. The ordering here is DISTANCE from the last
 * close and nothing else — the one ordering that is arithmetic rather than an
 * opinion — and the state words are the route's own. Every mechanical use of
 * levels of this family this desk has tested has failed out of sample, so a
 * number between 0 and 100 beside one of these prices would be a refuted
 * claim wearing a new name.
 *
 * TWO CLOCKS AGAIN, as on the HTF card and for the same reason. The facts are
 * as of the last CLOSED bar (`computed_at_bar_ms`), and a level's own age is
 * counted in BARS from the bar it formed on. A pool's sweep is the one time
 * printed as a UTC stamp rather than a bar count: the route publishes the
 * sweep's timestamp and only the forming bar's age, and a bar count derived
 * here would be clock time wearing a bar's name — `smc_context` measured that
 * mistake printing "swept 675 bars ago" on a pool 594 bars old.
 */

/**
 * How many levels a side are listed.
 *
 * SIX, the same cap `smc_context.MAX_PER_SIDE` uses, and it is the same
 * number for a different reason: there it keeps the prompt shorter than the
 * forty bars in front of it, here it keeps the panel shorter than the card
 * above it in a 460px rail. The two agreeing matters more than either
 * number — a reader comparing the screen against what the model was shown
 * should be comparing the same twelve rows.
 */
const MAX_PER_SIDE = 6

/** How stale the response may be before the header says so, in bars. */
const STALE_BARS = 2

export function LevelsCard({
  data,
  error,
}: {
  /** Polled ONCE in the Desk and handed here, because the chart draws lines
   *  at the prices this panel names. Two fetches would be two answers. */
  data: PriceLevelsResponse | null
  error: string | null
}) {
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  const levels = useMemo(() => harvestLevels(data), [data])
  const census = useMemo(() => censusOf(levels), [levels])

  /**
   * The two sides, nearest first.
   *
   * STRADDLING IS ITS OWN GROUP and not rounded into one of the others: a
   * band the last close is inside is neither above nor below it, and calling
   * it either would be the panel inventing a direction the route did not
   * report.
   *
   * Levels with no measurable distance — a response with no `last_close` —
   * are listed under `straddling` too rather than dropped, because a level
   * this panel cannot place is still a level the route sent.
   */
  const sides = useMemo(() => {
    const above = levels.filter((l) => l.dist != null && l.dist > 0).sort((a, b) => (a.dist ?? 0) - (b.dist ?? 0))
    const below = levels.filter((l) => l.dist != null && l.dist < 0).sort((a, b) => (b.dist ?? 0) - (a.dist ?? 0))
    const straddling = levels.filter((l) => l.dist == null || l.dist === 0)
    return { above, below, straddling }
  }, [levels])

  const barMs = data?.bar_ms ?? 0
  const staleBy =
    data?.computed_at_bar_ms != null && barMs > 0 ? (now - data.computed_at_bar_ms) / barMs : 0
  const stale = staleBy > STALE_BARS + 1

  return (
    <section className="border-border/60 border-t px-3 py-2">
      <div className="mb-1.5 flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <h2 className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">Price levels</h2>
        {data && (
          <span className="text-muted-foreground/60 num fd-caption">
            {data.market} {data.timeframe}
          </span>
        )}
        {data?.computed_at_bar_ms != null && (
          <span
            className={cn('num ml-auto fd-caption tabular-nums', stale ? 'text-caution' : 'text-muted-foreground/60')}
            title={
              stale
                ? `More than ${STALE_BARS} ${data.timeframe} bars have closed since these levels were computed, so they may not be current.`
                : `The last CLOSED ${data.timeframe} bar every level here was computed from. A level's own age is counted in bars from the bar it formed on, which is a different clock.`
            }
          >
            data to {clock(data.computed_at_bar_ms)}Z · {since(data.computed_at_bar_ms, now)} ago
          </span>
        )}
      </div>

      {error ? (
        // The route not answering and the route saying it has nothing are
        // different states, and this one is the first.
        <p className="text-muted-foreground fd-label">the levels route did not answer — {error}</p>
      ) : !data ? (
        <p className="text-muted-foreground/60 fd-label">reading…</p>
      ) : data.unavailable ? (
        // Written by the route to be displayed, and shown as written. (A
        // PROMPT must not print it verbatim — that is `9a5dbfb`'s lesson and
        // the route's own doc comment says so — but a screen is where the
        // sentence was meant to go.)
        <p className="text-muted-foreground fd-label">{data.unavailable}</p>
      ) : levels.length === 0 ? (
        // An empty response is a MEASUREMENT: the window held no levels of
        // any family. It renders differently from the sentence above, which
        // is the one distinction the route's module doc insists on.
        <p className="text-muted-foreground fd-label">
          the route answered and this window produced no levels at all
        </p>
      ) : (
        <div className="flex flex-col gap-1.5">
          <Census census={census} data={data} />
          <Thin data={data} />
          <Side title="above the last close" rows={sides.above} data={data} />
          <Side title="below the last close" rows={sides.below} data={data} />
          {sides.straddling.length > 0 && (
            <Side title="straddling the last close" rows={sides.straddling} data={data} />
          )}
          <p className="text-muted-foreground/50 fd-caption leading-snug">
            Ordered by distance from the last close and by nothing else — no score, no ranking, no
            confluence count. Age is in {data.timeframe} bars; a band's distance is to its nearer edge.
          </p>
        </div>
      )}
    </section>
  )
}

/**
 * THE TIMEFRAME THAT IS TOO COARSE FOR ITS OWN WINDOW, said out loud.
 *
 * The analysis window is ten TRADING DAYS measured in the stamps, the same
 * span on every timeframe, which is 879 bars of 15m, 230 of 1h — and ten bars
 * of 1d. Ten is fewer than the fourteen ATR(14) needs, so the route answers
 * with `atr14: null`, no activity profile, and only the levels that need
 * neither: five period pools and nothing else. That is honest on the wire and
 * it would read on screen as "the levels vanished", which is the one thing it
 * does not mean.
 *
 * NO OTHER TIMEFRAME'S ATR IS BORROWED TO FILL IT, and nothing here should
 * ever add that. A distance of "2.4 ATR" computed from 15m bars, printed on a
 * daily chart, is a number that reads right and describes nothing — the same
 * mislabelling the Desk refuses when it hides a run's indicators off the
 * traded timeframe, and for the same reason: it would be visibly wrong to
 * nobody, so it would be believed.
 */
function Thin({ data }: { data: PriceLevelsResponse }) {
  const noAtr = data.atr14 == null
  const noProfile = data.profile == null
  if (!noAtr && !noProfile) return null

  const bars = data.window?.bars ?? 0
  const missing = noAtr && noProfile ? 'no ATR(14) and no activity profile' : noAtr ? 'no ATR(14)' : 'no activity profile'
  const why =
    bars > 0 && bars < 14
      ? `this window is ${bars} ${data.timeframe} bars — ten trading days, the same ten days that are 879 bars of 15m — and ATR(14) needs 14`
      : `the route reported none for this window of ${bars} ${data.timeframe} bars`

  return (
    <p className="text-caution fd-caption leading-snug">
      {missing} on {data.timeframe}: {why}. The levels that need neither are below, and their
      distances are in price alone — no timeframe's ATR is borrowed to make an ATR figure this
      response does not have.
    </p>
  )
}

/**
 * How many levels there are, and how many are already spent.
 *
 * THIS IS THE HEADER'S MOST USEFUL SENTENCE and it is here for the reason
 * `smc_context._census_lines` gives: on the response of 2026-09-19 the route
 * served 252 levels of which 135 pools were already swept and 62 blocks
 * already broken, and a reader shown the nearest twelve with no idea they
 * were twelve of 252 would read a tidy tape. It is a count, not a ranking,
 * and it must not become one.
 *
 * Counted by the family that COLOURS the level on the chart, so the numbers
 * add up against the switches in the chart header rather than against the
 * prompt's five families — which put the session, day and week extremes in a
 * group of their own where the chart puts them with the pools whose stops
 * they are.
 */
function Census({ census, data }: { census: ReturnType<typeof censusOf>; data: PriceLevelsResponse }) {
  const word = (key: string) => LEVEL_FAMILIES.find((f) => f.key === key)?.label ?? key
  return (
    <p className="text-muted-foreground/70 fd-caption leading-snug">
      <span className="num">{census.total}</span> levels in this window, none of them ranked:{' '}
      {census.families.map((row, i) => (
        <span key={row.family}>
          {i > 0 ? ' · ' : ''}
          <span className="num">{row.n}</span> {word(row.family)}
          {row.spent > 0 && <span className="text-muted-foreground/50"> ({row.spent} spent)</span>}
        </span>
      ))}
      .{' '}
      <span
        tabIndex={0}
        role="note"
        className="focus-visible:ring-ring cursor-help underline decoration-dotted underline-offset-2 focus-visible:ring-2 focus-visible:outline-none"
        title={`Spent means a pool already swept or a block already broken — ${census.spent} of the ${census.total}. The chart draws the live ones within ${LIVE_WINDOW_ATR} ATR of the last close, plus the activity profile at any distance (${PROFILE_NOTE}); this list is every level, nearest first, spent ones included, so the two together account for all of them. Window: ${data.window?.bars ?? '—'} bars of ${data.timeframe} over ${data.window?.days ?? '—'} trading days, from ${data.source?.file ?? 'an unnamed file'}.`}
      >
        spent
      </span>{' '}
      means swept or broken.
    </p>
  )
}

function Side({ title, rows, data }: { title: string; rows: DeskLevel[]; data: PriceLevelsResponse }) {
  const shown = rows.slice(0, MAX_PER_SIDE)
  const hidden = rows.length - shown.length
  return (
    <div>
      <p className="text-muted-foreground/50 fd-caption">
        {title}
        {data.last_close != null && <span className="num"> {quote(data.last_close)}</span>}, nearest first
      </p>
      {shown.length === 0 ? (
        <p className="text-muted-foreground/40 fd-caption">none.</p>
      ) : (
        <div className="overflow-x-auto">
          <div className="grid min-w-[20rem] grid-cols-[minmax(6rem,1fr)_auto_auto_auto] items-baseline gap-x-2 gap-y-0.5">
            {/* Keyed with the row's position as well as the level, because
                two equal-highs pools can share a kind AND a price to the
                cent and differ only in the bars they formed on — the route
                serves 78 of them on one response, and a key that collided
                would drop the second silently. */}
            {shown.map((level, i) => (
              <Row key={`${i}:${level.kind}@${level.anchor}`} level={level} data={data} />
            ))}
          </div>
        </div>
      )}
      {hidden > 0 && (
        // COUNTED, NEVER SILENT — the same debt the prompt block pays with
        // the same sentence. A list that looks complete claims the tape is
        // tidier than it is.
        <p className="text-muted-foreground/40 fd-caption">
          ({hidden} further level{hidden === 1 ? '' : 's'} on this side not listed, being further away)
        </p>
      )}
    </div>
  )
}

/**
 * One level: what it is, where it is, how far, how old.
 *
 * THE STATE IS NOT DECORATION. An UNTESTED order block four bars old and an
 * UNTESTED order block two hundred bars old are different facts, and a row
 * printing only the price would say they are the same one. Both are on every
 * row, in the route's own vocabulary.
 *
 * The working that does not earn a column — the rule that produced the level,
 * the sweep's stamp, how full a gap is, how many swings a pool is made of and
 * how far apart they sit — is in the row's tooltip, which is where this desk
 * puts a fact a reader asks for one row at a time.
 */
function Row({ level, data }: { level: DeskLevel; data: PriceLevelsResponse }) {
  const band = level.bandLow != null && level.bandHigh != null
  // The profile's three marks are never off the chart however far away they
  // are — they are the window's own statistic, not a level price came from,
  // and `exemptFromWindow` in lib/levels.ts carries the argument.
  const profile = level.family === 'profile'
  const offChart =
    !profile && (level.spent || (level.distAtr != null && Math.abs(level.distAtr) > LIVE_WINDOW_ATR))

  const extras = [
    level.sideWord,
    level.directionWord,
    level.filledFraction != null ? `${(100 * level.filledFraction).toFixed(0)}% filled` : null,
    // A pool's sweep, as a UTC stamp: the route publishes the sweep's
    // timestamp and not its age in bars, and deriving one here would be
    // clock time wearing a bar's name.
    level.swept === true
      ? level.sweptAtBarMs != null
        ? `swept ${clock(level.sweptAtBarMs)}Z on ${new Date(level.sweptAtBarMs).toISOString().slice(0, 10)}`
        : 'swept'
      : level.swept === false
        ? 'not swept'
        : null,
    level.swings != null ? (level.swings === 1 ? '1 swing' : `${level.swings} swings`) : null,
    level.spreadAtr != null ? `spread ${level.spreadAtr.toFixed(2)} ATR` : null,
    level.displacementBodyAtr != null ? `made by a ${level.displacementBodyAtr.toFixed(2)} ATR body` : null,
    level.formedAtBarMs != null ? `formed ${clock(level.formedAtBarMs)}Z` : null,
    level.rule ? `rule: ${level.rule}` : null,
    profile ? PROFILE_NOTE : null,
    offChart
      ? level.spent
        ? 'spent, so the chart does not draw it unless the spent switch is on'
        : `more than ${LIVE_WINDOW_ATR} ATR from the last close, so it is outside the chart's window`
      : null,
    data.atr14 != null ? `ATR(14) is ${quote(data.atr14)}, the denominator of the ATR figure here` : null,
  ].filter(Boolean)

  const title = extras.join(' · ')

  return (
    <>
      <span className="fd-caption leading-snug" title={title}>
        <span className={cn(offChart && 'text-muted-foreground/60')}>{level.word}</span>{' '}
        <span className="text-muted-foreground/50">{level.stateWord}</span>
      </span>
      <span className="num text-right fd-caption tabular-nums" title={title}>
        {band ? `${quote(level.bandLow)}–${quote(level.bandHigh)}` : quote(level.price ?? level.anchor)}
      </span>
      {/* BOTH UNITS, ALWAYS. A distance in dollars does not say whether it is
          far, and a distance in ATR cannot be checked without its
          denominator; the ATR itself is in the tooltip beside it. */}
      <span className="num text-muted-foreground text-right fd-caption tabular-nums" title={title}>
        {level.dist == null
          ? '—'
          : level.dist === 0
            ? 'on it'
            : `${signed(level.dist)}${level.distAtr != null ? ` · ${Math.abs(level.distAtr).toFixed(2)} ATR` : ''}`}
      </span>
      <span className="num text-muted-foreground/60 text-right fd-caption tabular-nums" title={title}>
        {/* 0 is a measurement — it formed on the newest closed bar — and is
            printed as one rather than as an absence. */}
        {level.ageBars == null ? '—' : `${level.ageBars}b`}
      </span>
    </>
  )
}

/** A real minus sign, not a hyphen: it aligns with digits in tabular figures. */
function signed(v: number): string {
  return `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(2)}`
}

/**
 * A price, at the two decimals this desk prints prices at.
 *
 * TWO, and not `format.price`'s rounding: that one drops to whole units above
 * 1,000, which on gold turns 4367.48 and 4367.60 — two levels the route calls
 * separate — into one number printed twice. The fold that stops the chart
 * repeating a price is defined on exactly this many decimals
 * (`COLLAPSE_TOL_PRICE`), so the panel has to print them.
 */
function quote(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '—'
  return Math.abs(v) >= 1000
    ? v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
    : v.toFixed(Math.abs(v) >= 10 ? 2 : 4)
}
