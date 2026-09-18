import { useRef } from 'react'

import { cn } from '@/lib/utils'
import { DESK_TIMEFRAMES, type Timeframe } from '@/lib/timeframes'

/**
 * Which timeframe the chart is drawn on.
 *
 * A radiogroup rather than five buttons, because that is what it is: one of a
 * set, exactly one chosen. The difference is not pedantry — a screen reader
 * announces "4 hours, radio button, 4 of 5, selected" instead of reading five
 * unrelated buttons whose relationship exists only in the layout.
 *
 * Roving tabindex: ONE stop for the whole group, arrows to move within it.
 * Five separate tab stops in a chart header is five presses to get past a
 * control most visits never touch, and a person tabbing to the fills table
 * should not have to walk the timeframes to reach it.
 *
 * THE RUN'S OWN TIMEFRAME IS MARKED. Every other choice is a view of the same
 * market; that one is the series the book actually decided on. Without the
 * mark a person can read a 4h chart, see no entry near an obvious level, and
 * conclude the bot missed it — when the bot was never looking at these candles.
 */
export function TimeframeBar({
  value,
  onChange,
  runTf,
  unavailable,
}: {
  value: Timeframe
  onChange: (tf: Timeframe) => void
  /** The timeframe the run trades, when it is one of the offered ones. */
  runTf?: string | null
  /** Timeframes the server has nothing for, by name. Shown, not hidden. */
  unavailable?: Set<string>
}) {
  const items = useRef<(HTMLButtonElement | null)[]>([])

  // Disabled choices are still walked by the arrow keys. A choice you cannot
  // reach is a choice you cannot find out is unavailable, and "why is 1d
  // missing" is a worse question than "why is 1d greyed out".
  const move = (from: number, step: number) => {
    const next = (from + step + DESK_TIMEFRAMES.length) % DESK_TIMEFRAMES.length
    items.current[next]?.focus()
  }

  return (
    <span
      role="radiogroup"
      aria-label="Chart timeframe"
      className="border-border ml-2 inline-flex overflow-hidden rounded-sm border align-middle"
    >
      {DESK_TIMEFRAMES.map((tf, index) => {
        const selected = tf === value
        const missing = unavailable?.has(tf) === true
        const traded = runTf === tf

        return (
          <button
            key={tf}
            ref={(el) => {
              items.current[index] = el
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            // The one tab stop is whichever is selected; the rest are reached
            // with the arrows once focus is inside.
            tabIndex={selected ? 0 : -1}
            disabled={missing && !selected}
            onClick={() => !missing && onChange(tf)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
                event.preventDefault()
                move(index, 1)
              } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
                event.preventDefault()
                move(index, -1)
              } else if (event.key === 'Home') {
                event.preventDefault()
                items.current[0]?.focus()
              } else if (event.key === 'End') {
                event.preventDefault()
                items.current[DESK_TIMEFRAMES.length - 1]?.focus()
              }
            }}
            title={
              missing
                ? `No stored ${tf} series for this market — the chart cannot draw it`
                : traded
                  ? `${tf} — the timeframe this book actually trades`
                  : `Draw the chart on ${tf}`
            }
            className={cn(
              'num focus-visible:ring-ring relative px-1.5 py-px fd-caption normal-case transition-colors duration-100 focus-visible:z-10 focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
              index > 0 && 'border-border border-l',
              missing && !selected
                ? 'text-muted-foreground/40 cursor-not-allowed line-through'
                : selected
                  ? 'bg-primary text-primary-foreground font-medium'
                  : 'text-muted-foreground hover:bg-accent hover:text-foreground',
            )}
          >
            {tf}
            {/* The traded timeframe, as a mark rather than a colour: on the
                selected segment the background is already the accent, so a
                tint would be invisible exactly when it is being looked at. */}
            {traded && !missing && (
              <span
                aria-hidden
                className={cn(
                  'absolute top-px right-px size-1 rounded-full',
                  selected ? 'bg-primary-foreground/70' : 'bg-primary',
                )}
              />
            )}
          </button>
        )
      })}
    </span>
  )
}
