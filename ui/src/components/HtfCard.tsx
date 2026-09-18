import { useEffect, useState } from 'react'
import { ArrowDown, ArrowUp, MoveHorizontal } from 'lucide-react'

import type { HtfH4, HtfResponse } from '@/lib/api'
import { liveAge } from '@/lib/format'
import { BIAS_RULE, biasGlyph, biasTally, biasTint, htfBias } from '@/lib/htfBias'
import { cn } from '@/lib/utils'

/**
 * The higher timeframe, as CONTEXT and not as a signal.
 *
 * This card must not look like a buy/sell indicator. A large green UP would
 * be read as an instruction, and nothing here instructs: it is the state of
 * the four-hour and daily charts, which a person weighs against what their
 * book is doing. So the structure label is a small directional mark in the
 * text colour, tinted only on the glyph, and no panel here is filled with a
 * direction's colour.
 *
 * TWO AGES, NOT ONE, and it is the thing most likely to be got wrong by
 * whoever edits this next. The facts are as of the closed H4 bar. The
 * structure LABEL is as of the bar that CONFIRMED the swing — a fractal(2)
 * needs two bars after it, so on H4 that is up to eight hours older. Showing
 * one stamp for both would make the label claim more than the rule supports.
 */

/** How stale is too stale to show without saying so, in bars. */
const STALE_BARS = 2

export function HtfCard({ market, data, error }: {
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

  return (
    <section className="px-3 py-2">
      <h2 className="text-muted-foreground mb-1 fd-caption font-medium tracking-wide uppercase">
        Higher timeframe{' '}
        <span className="text-muted-foreground/60 num normal-case">{market}</span>
      </h2>

      {error ? (
        <p className="text-muted-foreground fd-label">
          the higher-timeframe route did not answer — {error}
        </p>
      ) : !data ? (
        <p className="text-muted-foreground/60 fd-label">reading…</p>
      ) : !data.h4 ? (
        // `h4: null` means the stored bars are missing entirely. The route
        // writes a sentence for exactly this and it is shown verbatim.
        <p className="text-muted-foreground fd-label">{data.unavailable ?? 'no H4 data for this market'}</p>
      ) : (
        <H4Body h4={data.h4} d1={data.d1} now={now} />
      )}
    </section>
  )
}

function H4Body({ h4, d1, now }: { h4: HtfH4; d1: HtfResponse['d1']; now: number }) {
  const s = h4.structure
  // "Not enough yet" is a different state from "no data", and the route
  // reports them differently on purpose. A populated object whose facts are
  // all null is a measurement in progress and will fix itself.
  const thin = h4.ema21 == null && h4.adx14 == null && h4.atr14 == null
  const staleBy = h4.bar_ms > 0 ? (now - h4.computed_at_bar_ms) / h4.bar_ms : 0
  const stale = staleBy > STALE_BARS + 1

  const Mark = s.label === 'UP' ? ArrowUp : s.label === 'DOWN' ? ArrowDown : MoveHorizontal
  const markTint =
    s.label === 'UP' ? 'text-lc' : s.label === 'DOWN' ? 'text-lp' : 'text-muted-foreground'

  const bias = htfBias(h4)

  return (
    <div className="flex flex-col gap-1.5">
      {/* THE READ, and the rule that produced it, never one without the other.
          It is a summary of the facts below and the caption says so, because
          the route publishes no verdict and the books do not read this. */}
      {bias && (
        <div className="border-border/60 flex flex-col gap-1 border-b pb-1.5">
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
            <span
              className={cn('num fd-display font-semibold', biasTint(bias.word))}
              title={BIAS_RULE}
            >
              <span aria-hidden>{biasGlyph(bias.word)}</span> {bias.word}
            </span>
            <span className="text-muted-foreground fd-caption">{biasTally(bias)}</span>
            <span className="text-muted-foreground/70 num fd-caption tabular-nums">
              {s.confirmed_at_bar_ms != null
                ? `as of ${liveAge(s.confirmed_at_bar_ms, now)}`
                : 'not yet confirmed'}
            </span>
          </div>
          {/* The lamps: WHICH agreed, not just how many. A word with no
              working is the thing a reader cannot argue with. */}
          <div className="flex flex-wrap gap-x-3 gap-y-0.5 fd-caption">
            {bias.votes.map((v) => (
              <span key={v.name} className="flex items-center gap-1" title={v.why}>
                <span
                  className={cn(
                    'size-1.5 rounded-full',
                    v.vote === 'bull' ? 'bg-lc' : v.vote === 'bear' ? 'bg-lp' : 'bg-muted-foreground/40',
                  )}
                  aria-hidden
                />
                <span className="text-muted-foreground/70">
                  {v.name} {v.vote === 'bull' ? 'bull' : v.vote === 'bear' ? 'bear' : '—'}
                </span>
              </span>
            ))}
          </div>
          <p className="text-muted-foreground/50 fd-caption leading-snug">
            A summary of the three facts below, not a signal — the books do not read it.{' '}
            {BIAS_RULE}
          </p>
        </div>
      )}

      {/* The facts he would use to argue with the word. */}
      {d1 && (
        <p className="text-muted-foreground num fd-caption tabular-nums">
          {[
            d1.prior_day_high != null && d1.prior_day_low != null && h4.last_close != null
              ? `${h4.last_close > (d1.prior_day_high + d1.prior_day_low) / 2 ? 'above' : 'below'} prior-day mid`
              : null,
            d1.close_pct_of_prior_week_range != null
              ? `${d1.close_pct_of_prior_week_range.toFixed(0)}% of the week’s range`
              : null,
            h4.dist_ema21_atr != null
              ? `${h4.dist_ema21_atr >= 0 ? 'above' : 'below'} EMA21 by ${Math.abs(h4.dist_ema21_atr).toFixed(2)} ATR`
              : null,
          ]
            .filter(Boolean)
            .join(' · ')}
        </p>
      )}

      <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
        {/* Shape AND colour. The glyph carries the direction on its own, so a
            reader who cannot separate the two colours loses nothing. */}
        <span className="flex items-center gap-1 fd-body">
          <Mark className={cn('size-3.5 shrink-0', markTint)} aria-hidden />
          <span className="font-medium">{s.label.toLowerCase()}</span>
        </span>
        <span className="text-muted-foreground/60 fd-caption">{s.rule}</span>
        {/* The LABEL's own age, which is not the facts' age. */}
        <span className="text-muted-foreground num fd-caption tabular-nums">
          {s.confirmed_at_bar_ms != null
            ? `confirmed ${liveAge(s.confirmed_at_bar_ms, now)}`
            : 'not yet confirmed'}
        </span>
      </div>

      {/* THE LABEL'S OWN WORKING, which is where the priors belong.
          They are not levels: they have already been exceeded, and that is
          what makes the label what it is. A line on a chart claims price may
          react there; these claim the opposite. So they are shown as the
          COMPARISON that produced the label, where a reader can check it -
          on d1's real RANGE, "highs over, lows under" is the whole reason the
          label is not UP. */}
      {(s.last_high && s.prior_high) || (s.last_low && s.prior_low) ? (
        <div className="text-muted-foreground/70 num flex flex-wrap gap-x-3 fd-caption tabular-nums">
          {s.last_high && s.prior_high && (
            <span>
              highs {quote(s.last_high.price)}{' '}
              <span className="text-muted-foreground/50">
                {s.last_high.price > s.prior_high.price ? 'over' : 'under'}
              </span>{' '}
              {quote(s.prior_high.price)}
            </span>
          )}
          {s.last_low && s.prior_low && (
            <span>
              lows {quote(s.last_low.price)}{' '}
              <span className="text-muted-foreground/50">
                {s.last_low.price > s.prior_low.price ? 'over' : 'under'}
              </span>{' '}
              {quote(s.prior_low.price)}
            </span>
          )}
        </div>
      ) : null}

      {thin ? (
        <p className="text-muted-foreground fd-label">
          not enough H4 bars yet — the bars are there, the indicators need more of them
        </p>
      ) : (
        <>
          {/* The two numbers a person reads first. */}
          <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
            <Fact
              label={s.break_level != null ? `breaks ${(s.break_side ?? '').toLowerCase()}` : 'break level'}
              value={s.break_level != null ? quote(s.break_level) : '—'}
              hint={
                s.break_level == null
                  ? 'a range has no single level whose break changes the label'
                  : 'the price that would change the structure'
              }
            />
            <Fact
              label="from EMA21"
              value={h4.dist_ema21_atr != null ? `${signed(h4.dist_ema21_atr)} ATR` : '—'}
              hint={
                h4.atr14 != null
                  ? `ATR14 is ${quote(h4.atr14)} in quote units — the denominator of the figure beside it`
                  : undefined
              }
            />
          </div>

          <div className="text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 fd-caption">
            <Small label="ADX14" value={h4.adx14 != null ? h4.adx14.toFixed(1) : '—'} />
            <Small
              label="ER20"
              value={h4.efficiency_20 != null ? h4.efficiency_20.toFixed(2) : '—'}
              hint="Kaufman efficiency ratio, 0 to 1"
            />
            <Small
              label="EMA21"
              value={h4.ema21 != null ? quote(h4.ema21) : '—'}
              suffix={slope(h4.ema21_slope_sign)}
            />
            <Small
              label="EMA55"
              value={h4.ema55 != null ? quote(h4.ema55) : '—'}
              suffix={slope(h4.ema55_slope_sign)}
            />
            {/* 0 is a measurement: THIS bar made the new extreme. Null is not,
                and is an em dash. */}
            <Small
              label="since high"
              value={bars(h4.donchian20.bars_since_new_high)}
              hint={h4.donchian20.bars_since_new_high === 0 ? 'this bar made a new high' : undefined}
            />
            <Small
              label="since low"
              value={bars(h4.donchian20.bars_since_new_low)}
              hint={h4.donchian20.bars_since_new_low === 0 ? 'this bar made a new low' : undefined}
            />
          </div>

          {d1 && (
            <div className="text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 fd-caption">
              <Small label="prior day" value={range(d1.prior_day_low, d1.prior_day_high)} />
              <Small label="prior week" value={range(d1.prior_week_low, d1.prior_week_high)} />
              {/* NOT clamped and NOT a bar that stops at the ends: above 100
                  is price out of the prior week's range upward and below 0
                  downward, and those are the most informative states it has.
                  A capped bar would draw a breakout as a ceiling. */}
              <Small
                label="in week range"
                value={
                  d1.close_pct_of_prior_week_range != null
                    ? `${d1.close_pct_of_prior_week_range.toFixed(0)}%`
                    : '—'
                }
                tint={
                  d1.close_pct_of_prior_week_range == null
                    ? undefined
                    : d1.close_pct_of_prior_week_range > 100
                      ? 'text-lc'
                      : d1.close_pct_of_prior_week_range < 0
                        ? 'text-lp'
                        : undefined
                }
                hint={
                  d1.close_pct_of_prior_week_range == null
                    ? undefined
                    : d1.close_pct_of_prior_week_range > 100
                      ? 'above the prior week’s high'
                      : d1.close_pct_of_prior_week_range < 0
                        ? 'below the prior week’s low'
                        : undefined
                }
              />
            </div>
          )}
        </>
      )}

      {/* The FACTS' age, and it says so rather than showing a number that has
          stopped moving. */}
      <div className={cn('num fd-caption tabular-nums', stale ? 'text-caution' : 'text-muted-foreground/60')}>
        {stale
          ? `the last H4 bar closed ${liveAge(h4.computed_at_bar_ms, now)} — more than ${STALE_BARS} bars ago, so these may not be current`
          : `H4 bar closed ${liveAge(h4.computed_at_bar_ms, now)}`}
      </div>
    </div>
  )
}

function Fact({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <span className="flex flex-col" title={hint}>
      <span className="text-muted-foreground fd-caption">{label}</span>
      <span className="num fd-body tabular-nums">{value}</span>
    </span>
  )
}

function Small({
  label,
  value,
  suffix,
  hint,
  tint,
}: {
  label: string
  value: string
  suffix?: string
  hint?: string
  tint?: string
}) {
  return (
    <span className="num tabular-nums" title={hint}>
      <span className="text-muted-foreground/60">{label} </span>
      <span className={cn('text-foreground/80', tint)}>
        {value}
        {suffix}
      </span>
    </span>
  )
}

/** A slope sign. 0 is a genuine flat and is drawn as one, not as absence. */
function slope(sign: number | null): string {
  if (sign == null) return ''
  return sign > 0 ? ' ↗' : sign < 0 ? ' ↘' : ' →'
}

function bars(n: number | null): string {
  return n == null ? '—' : String(n)
}

function range(lo: number | null, hi: number | null): string {
  return lo == null || hi == null ? '—' : `${quote(lo)} – ${quote(hi)}`
}

function signed(v: number): string {
  return `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(2)}`
}

function quote(v: number): string {
  return Math.abs(v) >= 1000
    ? v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
    : v.toFixed(Math.abs(v) >= 10 ? 2 : 4)
}
