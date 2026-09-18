import { useEffect, useState } from 'react'

import type { HtfD1, HtfH4, HtfResponse } from '@/lib/api'
import { clock, since } from '@/lib/format'
import {
  BIAS_RULE,
  biasGlyph,
  biasHue,
  biasTally,
  biasTint,
  biasWash,
  htfBias,
  structureGlyph,
  structureTint,
  type HtfBias,
} from '@/lib/htfBias'
import { cn } from '@/lib/utils'

/**
 * The higher timeframe, as CONTEXT and not as a signal.
 *
 * This card must not look like a buy/sell indicator. A large green UP would
 * be read as an instruction, and nothing here instructs: it is the state of
 * the hourly, four-hour and daily charts, which a person weighs against what
 * their book is doing. So the structure label is a small directional mark in
 * the text colour, tinted only on the glyph, and no panel here is filled with
 * a direction's colour.
 *
 * A LADDER, BECAUSE THE COMPARISON IS THE POINT. Three timeframes down the
 * page in the same columns, so "H1 down while H4 ranges" is read by scanning
 * rather than by parsing differently-shaped sentences. It was prose before —
 * free-text runs of `break · ATR · ADX · ER` per timeframe — and prose makes
 * the eye re-find each quantity on every row, which is the work a table
 * exists to remove.
 *
 * TWO CLOCKS, AND CONFUSING THEM IS WHAT MOST LIKELY GOES WRONG HERE.
 *
 *   - The FACTS are as of the last CLOSED bar of that timeframe. That is the
 *     data age: `computed_at_bar_ms`.
 *   - The structure LABEL is as of the bar that CONFIRMED the swing. A
 *     fractal(2) needs two bars after it, so on H4 the label can be eight
 *     hours older than the facts beside it: `confirmed_at_bar_ms`.
 *
 * The card printed both as bare ages in two places — "as of … 14 h 36 m ago"
 * at the top and "H4 bar closed 6 h 36 m ago" at the bottom — with nothing
 * saying they measured different things. Two numbers that disagree about one
 * timeframe and never explain themselves read as a bug. Now the confirming
 * stamp sits in the `as of` column beside the label it qualifies, the data
 * stamp is named once in the header, and every row's tooltip states both of
 * its own.
 */

/** How stale is too stale to show without saying so, in bars. */
const STALE_BARS = 2

export function HtfCard({
  market,
  data,
  error,
}: {
  market: string
  /** Fetched ONCE in the Desk and handed here, because the chart draws a line
   *  at the same break level this card names. Two fetches would be two
   *  answers about one price, side by side. */
  data: HtfResponse | null
  error: string | null
}) {
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  const h4 = data?.h4 ?? null
  const staleBy = h4 && h4.bar_ms > 0 ? (now - h4.computed_at_bar_ms) / h4.bar_ms : 0
  const stale = staleBy > STALE_BARS + 1

  // Computed here rather than inside the table so the card itself can be lit
  // by it. One call, handed down, so the wash and the word can never disagree.
  const bias = h4 ? htfBias(h4) : null

  return (
    <section className="relative overflow-hidden px-3 py-2">
      <BiasWash bias={bias} />
      <div className="relative mb-1.5 flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <h2 className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">
          Higher timeframe
        </h2>
        <span className="text-muted-foreground/60 num fd-caption">{market}</span>
        {/* THE DATA CLOCK, named once and named for the timeframe it belongs
            to. Each row's facts carry their own last-closed-bar stamp — the
            hourly is fresher than the daily by construction — so an
            unlabelled "data to" here would be silently wrong about two of the
            three rows. H4 is the timeframe the bias is computed from, so it
            is the one that earns the header; the other two are in their row
            tooltips. */}
        {h4 && (
          <span
            className={cn(
              'num ml-auto fd-caption tabular-nums',
              stale ? 'text-caution' : 'text-muted-foreground/60',
            )}
            title={
              stale
                ? `More than ${STALE_BARS} H4 bars have closed since these facts were computed, so they may not be current.`
                : 'The last CLOSED H4 bar these facts come from. NOT the age of the structure label, which is in the "as of" column.'
            }
          >
            H4 data to {clock(h4.computed_at_bar_ms)}Z · {since(h4.computed_at_bar_ms, now)} ago
          </span>
        )}
      </div>

      {error ? (
        <p className="text-muted-foreground fd-label">the higher-timeframe route did not answer — {error}</p>
      ) : !data ? (
        <p className="text-muted-foreground/60 fd-label">reading…</p>
      ) : !data.h4 ? (
        // `h4: null` means the stored bars are missing entirely. The route
        // writes a sentence for exactly this and it is shown verbatim.
        <p className="text-muted-foreground fd-label">
          {data.unavailable_by_tf?.['4h'] ?? data.unavailable ?? 'no H4 data for this market'}
        </p>
      ) : (
        <Ladder data={data} h4={data.h4} bias={bias} now={now} />
      )}
    </section>
  )
}

/**
 * The market's bias, as light on the card rather than as a fill.
 *
 * A TOP EDGE FADING DOWN, not a radial from the corner, and the reason is
 * measurability. A linear fade puts its maximum at a known band, so the text
 * that sits in that band can be measured once and asserted forever; a radial's
 * intensity at any given word depends on the card's size, which means a card
 * that grows quietly changes its own contrast and no check can pin it. The
 * card's content is also a table anchored top-left, so a top-left radial would
 * put peak tint under the header and the first column — the densest small
 * text on the card.
 *
 * NOTHING FOR A RANGE, and that is the signal. A neutral wash would be a
 * colour that reads as a verdict where there is none; unlit IS the range
 * state, and the hairline stays so the card still has an edge.
 *
 * NOT ANIMATED, deliberately. The bias can only change when an H4 bar closes,
 * so a crossfade here would be code that runs at most six times a day and is
 * seen by nobody — and the wash mounts and unmounts with the hue rather than
 * transitioning between two of them, so a transition would not fire on the
 * flip that matters anyway. Nothing moves, so `prefers-reduced-motion` has
 * nothing to respect.
 */
function BiasWash({ bias }: { bias: HtfBias | null }) {
  const hue = bias ? biasHue(bias.word) : null

  return (
    <>
      {/* The edge is always drawn: a card with no rule at all would read as
          unfinished rather than as neutral. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-px"
        style={{
          background: hue ? `color-mix(in oklab, ${hue} 60%, transparent)` : 'var(--border)',
        }}
      />
      {hue && bias && (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 top-0 h-11"
          style={{
            background: `linear-gradient(to bottom, color-mix(in oklab, ${hue} ${biasWash(bias)}, transparent), transparent)`,
          }}
        />
      )}
    </>
  )
}

function Ladder({
  data,
  h4,
  bias,
  now,
}: {
  data: HtfResponse
  h4: HtfH4
  bias: HtfBias | null
  now: number
}) {
  return (
    <div className="relative flex flex-col gap-1.5">
      {bias && (
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <span className={cn('num fd-display font-semibold', biasTint(bias.word))}>
            <span aria-hidden>{biasGlyph(bias.word)}</span> {bias.word}
          </span>
          <span className="text-muted-foreground fd-caption">{biasTally(bias)}</span>
          {/* THE LAMPS, as three dots rather than three phrases. WHICH of the
              three agreed is worth showing; spelling out "structure bull ·
              EMA stack no vote · momentum bear" put a sentence where a glance
              belongs. Each dot keeps its own tooltip, so the working is one
              hover away and nothing has been hidden. */}
          <span className="flex items-center gap-1" role="list" aria-label="the three votes">
            {bias.votes.map((v) => (
              <span
                key={v.name}
                role="listitem"
                title={`${v.name}: ${v.vote ?? 'no vote'} — ${v.why}`}
                className={cn(
                  'size-1.5 rounded-full',
                  v.vote === 'bull'
                    ? 'bg-lc'
                    : v.vote === 'bear'
                      ? 'bg-lp'
                      : 'bg-muted-foreground/40',
                )}
              />
            ))}
          </span>
          {/* The rule was five lines of paragraph in the middle of the card.
              It is the thing a reader needs once and then never again, which
              is what a tooltip is for. Focusable, so it is not mouse-only. */}
          <span
            tabIndex={0}
            role="note"
            aria-label="how the bias word is computed"
            title={`${BIAS_RULE} A summary, not a signal.`}
            className="text-muted-foreground/60 focus-visible:ring-ring inline-flex size-3.5 cursor-help items-center justify-center rounded-full border border-current fd-caption leading-none focus-visible:ring-2 focus-visible:outline-none"
          >
            i
          </span>
        </div>
      )}

      {/* One grid, the same columns on every row. `overflow-x-auto` because
          this card lives in a narrow pane and a table is the one thing
          allowed to scroll sideways rather than reflow — a break level that
          wrapped onto its own line would stop being comparable with the row
          above it, which is the entire reason for the table. */}
      <div className="overflow-x-auto">
        <div className="grid min-w-[21rem] grid-cols-[auto_minmax(4.5rem,1fr)_auto_auto_auto_auto] items-baseline gap-x-3 gap-y-1">
          <HeadCell />
          <HeadCell>structure</HeadCell>
          <HeadCell right>break</HeadCell>
          <HeadCell right>vs EMA21</HeadCell>
          <HeadCell right>ADX / ER</HeadCell>
          <HeadCell right>as of</HeadCell>

          <TfRow tf="H1" block={data.h1} sentence={data.unavailable_by_tf?.['1h'] ?? null} now={now} />
          <TfRow tf="H4" block={h4} sentence={null} now={now} />
        </div>
      </div>

      <D1Line d1={data.d1} h4={h4} sentence={data.unavailable_by_tf?.['1d'] ?? null} />

      <p className="text-muted-foreground/50 fd-caption leading-snug">
        H4 rule: structure + EMA stack + ADX/DI majority — a summary, not a signal; the books do not read
        it.
      </p>
    </div>
  )
}

function HeadCell({ children, right }: { children?: React.ReactNode; right?: boolean }) {
  return <span className={cn('text-muted-foreground/50 fd-caption', right && 'text-right')}>{children}</span>
}

/**
 * One timeframe's row. The same six cells on every row.
 *
 * THE BREAK LEVEL IS THE ONE NUMBER IN FOREGROUND WEIGHT. It is the only
 * figure here that anybody acts on — the price whose break changes the label
 * — and everything beside it is the working behind it. Giving them all equal
 * weight is what made this card a wall.
 *
 * ABSENCE IS THREE DIFFERENT THINGS. `undefined` is an API older than this
 * desk: a deploy-order artefact, nothing to investigate. `null` is that
 * timeframe's bars missing, which is. A populated block whose facts are all
 * null is a measurement in progress that will fix itself. One dash for all
 * three would send somebody looking for a problem that is either not theirs
 * or not there.
 */
function TfRow({
  tf,
  block,
  sentence,
  now,
}: {
  tf: 'H1' | 'H4'
  block: HtfH4 | null | undefined
  sentence: string | null
  now: number
}) {
  if (block === undefined) {
    return (
      <>
        <RowLabel tf={tf} />
        <span className="text-muted-foreground/50 col-span-5 fd-caption">
          this desk shows the hourly read; the API it is talking to does not send it yet
        </span>
      </>
    )
  }

  if (!block) {
    return (
      <>
        <RowLabel tf={tf} />
        <span className="text-muted-foreground col-span-5 fd-caption">
          {sentence ?? `no ${tf} bars stored for this market`}
        </span>
      </>
    )
  }

  const s = block.structure
  const thin = block.ema21 == null && block.adx14 == null && block.atr14 == null

  // BOTH of this row's clocks, spelled out, so the pair is never two numbers
  // a reader has to reconcile alone.
  const clocks =
    `Facts from the ${tf} bar that closed ${clock(block.computed_at_bar_ms)}Z, ` +
    `${since(block.computed_at_bar_ms, now)} ago. ` +
    (s.confirmed_at_bar_ms != null
      ? `The ${s.label} label is as of the bar that confirmed the swing, ${clock(s.confirmed_at_bar_ms)}Z, ` +
        `${since(s.confirmed_at_bar_ms, now)} ago. A swing can only be confirmed after the fact, so the ` +
        `label is MEANT to lag the facts beside it rather than being stale.`
      : 'The swing has not been confirmed yet.')

  // The working that does not earn a column: it is checkable, and a reader
  // asks for it about one row at a time.
  const extras = [
    block.ema21 != null ? `EMA21 ${quote(block.ema21)}${slope(block.ema21_slope_sign)}` : null,
    block.ema55 != null ? `EMA55 ${quote(block.ema55)}${slope(block.ema55_slope_sign)}` : null,
    block.atr14 != null ? `ATR14 ${quote(block.atr14)}, the denominator of the ATR column` : null,
    // 0 is a measurement — THIS bar made the new extreme — and null is not.
    block.donchian20.bars_since_new_high != null
      ? `${block.donchian20.bars_since_new_high} bars since a new high`
      : null,
    block.donchian20.bars_since_new_low != null
      ? `${block.donchian20.bars_since_new_low} bars since a new low`
      : null,
  ].filter(Boolean)

  return (
    <>
      <RowLabel tf={tf} label={s.label} />

      {/* THE LABEL'S OWN WORKING rides with the label, in its tooltip.
          The priors are not levels and must never be drawn as any: they have
          already been exceeded, and that is what MAKES the label. A line on a
          chart claims price may react there; these claim the opposite. Stated
          as the comparison that produced the word — on a real RANGE, "highs
          over, lows under" is the whole reason the label is not UP — so the
          reader can check the rule rather than take it. */}
      <span
        className={cn('fd-label', structureTint(s.label))}
        title={[`${s.label} by ${s.rule}.`, swings(s), clocks].filter(Boolean).join(' ')}
      >
        <span aria-hidden>{structureGlyph(s.label)}</span> {s.label.toLowerCase()}
      </span>

      {/* The one number in foreground weight. A dash on a RANGE is not a
          missing measurement: a range has no single price whose break changes
          the label, and the tooltip says so rather than leaving the dash to
          be read as a gap. */}
      <span
        className="num text-right fd-label tabular-nums"
        title={
          s.break_level != null
            ? `${quote(s.break_level)} — a close ${(s.break_side ?? '').toLowerCase()} this would change the ${tf} structure`
            : 'a range has no single level whose break changes the label'
        }
      >
        {s.break_level != null ? quote(s.break_level) : <span className="text-muted-foreground/40">—</span>}
      </span>

      <span
        className="text-muted-foreground num text-right fd-caption tabular-nums"
        title={extras.length ? extras.join(' · ') : 'not enough bars yet for the indicators'}
      >
        {thin || block.dist_ema21_atr == null ? '—' : `${signed(block.dist_ema21_atr)} ATR`}
      </span>

      <span
        className="text-muted-foreground num text-right fd-caption tabular-nums"
        title="ADX14, and the Kaufman efficiency ratio over 20 bars (0 to 1)"
      >
        {block.adx14 != null ? block.adx14.toFixed(0) : '—'}
        <span className="text-muted-foreground/40"> / </span>
        {block.efficiency_20 != null ? block.efficiency_20.toFixed(2) : '—'}
      </span>

      {/* THE CONFIRMING BAR, not the data bar — and its clock time rather
          than its age, because an age alone reads as staleness where the
          bar's own time says which candle the word describes. */}
      <span className="text-muted-foreground/70 num text-right fd-caption tabular-nums" title={clocks}>
        {s.confirmed_at_bar_ms != null ? `${clock(s.confirmed_at_bar_ms)}Z` : '—'}
      </span>
    </>
  )
}

/**
 * The row's own timeframe, and its direction as a shape.
 *
 * A two-pixel edge in the row's STRUCTURE colour — not the bias colour. The
 * card is washed by the H4 bias; these are the individual readings, and a row
 * whose edge disagreed with the card's wash is the ladder doing its job. A
 * range gets a neutral edge for the same reason the card gets no wash.
 */
function RowLabel({ tf, label }: { tf: string; label?: 'UP' | 'DOWN' | 'RANGE' }) {
  return (
    <span
      className={cn(
        'text-muted-foreground/60 num border-l-2 pl-1.5 fd-caption',
        label === 'UP' ? 'border-lc' : label === 'DOWN' ? 'border-lp' : 'border-border',
      )}
    >
      {tf}
    </span>
  )
}

/**
 * The daily row, which is a sentence rather than the same six columns.
 *
 * D1 publishes different facts — prior-day and prior-week ranges, and where
 * the close sits within the week — and forcing them into `structure / break /
 * vs EMA21` would mean five empty cells and one crowded one. A row that
 * borrows the table's shape without sharing its meaning is worse than a row
 * that plainly does something else.
 */
function D1Line({ d1, h4, sentence }: { d1: HtfD1 | null; h4: HtfH4; sentence: string | null }) {
  if (!d1) {
    return (
      <p className="text-muted-foreground/60 fd-caption">
        <span className="num">D1</span> {sentence ?? 'no daily bars stored for this market'}
      </p>
    )
  }

  const mid =
    d1.prior_day_high != null && d1.prior_day_low != null && h4.last_close != null
      ? `${h4.last_close > (d1.prior_day_high + d1.prior_day_low) / 2 ? 'above' : 'below'} prior-day mid`
      : null

  // NOT clamped and NOT drawn as a bar: above 100 is price out of the prior
  // week's range upward and below 0 downward, and those are the most
  // informative states it has. A capped bar would draw a breakout as a
  // ceiling.
  const pct = d1.close_pct_of_prior_week_range
  const pctTint = pct == null ? undefined : pct > 100 ? 'text-lc' : pct < 0 ? 'text-lp' : undefined

  return (
    <p
      className="text-muted-foreground num flex flex-wrap items-baseline gap-x-2 fd-caption tabular-nums"
      title={
        d1.prior_week_mid != null
          ? `The prior week's mid is ${quote(d1.prior_week_mid)}.`
          : 'No prior-week mid: the daily series has no completed week behind it.'
      }
    >
      <span className="text-muted-foreground/60">D1</span>
      {mid && <span>{mid}</span>}
      {pct != null && (
        <span className={pctTint}>
          {pct.toFixed(0)}% of week {range(d1.prior_week_low, d1.prior_week_high)}
        </span>
      )}
      <span className="text-muted-foreground/70">prior day {range(d1.prior_day_low, d1.prior_day_high)}</span>
    </p>
  )
}

/**
 * The swing comparison that produced the label, as a sentence.
 *
 * Two swings can share a bar — one outside bar can be both a fractal high and
 * a fractal low — so nothing here assumes the pairs are distinct or that a
 * full set exists. Empty string when neither pair is available, which the
 * caller filters out rather than printing a dangling clause.
 */
function swings(s: HtfH4['structure']): string {
  const parts: string[] = []
  if (s.last_high && s.prior_high) {
    parts.push(
      `Highs ${quote(s.last_high.price)} ${s.last_high.price > s.prior_high.price ? 'over' : 'under'} ${quote(s.prior_high.price)}`,
    )
  }
  if (s.last_low && s.prior_low) {
    parts.push(
      `lows ${quote(s.last_low.price)} ${s.last_low.price > s.prior_low.price ? 'over' : 'under'} ${quote(s.prior_low.price)}`,
    )
  }
  return parts.length ? `${parts.join(', ')}.` : ''
}

/** A slope sign. 0 is a genuine flat and is drawn as one, not as absence. */
function slope(sign: number | null): string {
  if (sign == null) return ''
  return sign > 0 ? ' ↗' : sign < 0 ? ' ↘' : ' →'
}

function range(lo: number | null, hi: number | null): string {
  return lo == null || hi == null ? '—' : `${quote(lo)}–${quote(hi)}`
}

function signed(v: number): string {
  return `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(2)}`
}

function quote(v: number): string {
  return Math.abs(v) >= 1000
    ? v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
    : v.toFixed(Math.abs(v) >= 10 ? 2 : 4)
}
