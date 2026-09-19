import { useRef } from 'react'

import {
  LEVEL_FAMILIES,
  LEVEL_MODES,
  PROFILE_NOTE,
  TAGS_PER_PANE,
  type LevelFamily,
  type LevelMode,
} from '@/lib/levels'
import { cn } from '@/lib/utils'

/**
 * THE ONE LEVEL SWITCH, and the colour key the families kept.
 *
 * This file used to hold five independent toggles — profile, liquidity,
 * gaps, blocks, other — beside a master `levels on/off` and a `spent on/off`.
 * The owner's instruction on 2026-09-19 was "gộp hết vào làm 1 đi, khi bật là
 * sẽ hiển thị toàn bộ": one control, and on shows everything. The filename is
 * the old one on purpose, so `git log --follow` still reaches the toggles and
 * the argument that produced them.
 *
 * A RADIOGROUP AND NOT A TOOLBAR, which is the exact distinction the toggles'
 * own comment used to draw from the other side. Five toggles were five things
 * each separately on or off, announced as "profile, toggle button, pressed".
 * This is ONE CHOICE OUT OF THREE, announced as "levels, radio group, live,
 * 2 of 3" — and a reader who expects the others to switch off when they pick
 * one is now right, so the announcement has to say so.
 *
 * ROVING TABINDEX, like the timeframe bar and like the toggles before it. One
 * tab stop for the whole control and arrows inside it: the chart header's
 * keyboard cost stays at one press regardless of how many positions exist.
 * Arrows MOVE THE CHOICE as well as the focus here, which is the radiogroup
 * pattern and was not the toolbar's.
 */
export function LevelSwitch({
  mode,
  onChange,
  total,
  drawn,
}: {
  mode: LevelMode
  onChange: (next: LevelMode) => void
  /** Every level on the response, for the `everything` position's title —
   *  the reader deserves the size of what they are about to ask for BEFORE
   *  they press it, not after the chart fills up. */
  total?: number
  /** How many of them the current position draws — the same unit as `total`,
   *  levels off the route and not lines on the canvas. */
  drawn?: number
}) {
  const items = useRef<(HTMLButtonElement | null)[]>([])
  // The group's single tab stop. It follows the checked position, and falls
  // back to the first rather than to none: a stored value nobody recognises
  // would otherwise leave the whole control unreachable from the keyboard,
  // which is a worse failure than starting on `off`.
  const checked = LEVEL_MODES.findIndex((m) => m.key === mode)
  const stop = checked === -1 ? 0 : checked

  const pick = (next: LevelMode, index: number) => {
    onChange(next)
    items.current[index]?.focus()
  }

  const move = (from: number, step: number) => {
    const next = (from + step + LEVEL_MODES.length) % LEVEL_MODES.length
    pick(LEVEL_MODES[next].key, next)
  }

  const titleFor = (key: LevelMode): string => {
    if (key === 'off') return 'Draw no levels at all — neither the H4 structure nor anything the levels route sent.'
    if (key === 'live') {
      return `Draw the levels still in play and near the last close: every family, nothing already swept or broken, nothing further than the distance window. The activity profile is the one exemption — ${PROFILE_NOTE}. The count beside this switch says how many that leaves out, and "everything" is one press away.`
    }
    return `Draw EVERYTHING the response carries${
      total ? ` — all ${total} levels` : ''
    }: spent pools and broken blocks included, with no distance window. Deliberately crowded: past ${TAGS_PER_PANE} tags the pane can no longer place them at their own prices and spaces them evenly instead, so the lines stay true and the labels drift.`
  }

  return (
    <span
      role="radiogroup"
      aria-label="Chart levels"
      className="ml-1 inline-flex items-center gap-1 align-middle"
    >
      <span className="text-muted-foreground/50 fd-caption">levels</span>
      <span className="border-border inline-flex items-center overflow-hidden rounded-sm border">
        {LEVEL_MODES.map((position, index) => {
          const active = position.key === mode
          return (
            <button
              key={position.key}
              ref={(el) => {
                items.current[index] = el
              }}
              type="button"
              role="radio"
              aria-checked={active}
              tabIndex={index === stop ? 0 : -1}
              onClick={() => pick(position.key, index)}
              onKeyDown={(event) => {
                if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
                  event.preventDefault()
                  move(index, 1)
                } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
                  event.preventDefault()
                  move(index, -1)
                }
              }}
              title={titleFor(position.key)}
              className={cn(
                'focus-visible:ring-ring border-border px-1.5 py-px fd-caption normal-case transition-colors duration-100 focus-visible:ring-2 focus-visible:-outline-offset-2 focus-visible:outline-none not-first:border-l motion-reduce:transition-none',
                active
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground/60 hover:text-muted-foreground hover:bg-accent',
              )}
            >
              {position.label}
              {/* The size of the ask, on the position that asks for it. A
                  reader should be able to see that "everything" is 252 and
                  not 15 before pressing it. */}
              {position.key === 'everything' && total ? (
                <span className="num text-muted-foreground/50"> {total}</span>
              ) : position.key === 'live' && active && drawn != null ? (
                <span className="num text-muted-foreground/50"> {drawn}</span>
              ) : null}
            </button>
          )
        })}
      </span>
    </span>
  )
}

/**
 * Which hue is which kind of level.
 *
 * A LEGEND, NOT A CONTROL, and the difference is the whole reason it exists.
 * Merging five switches into one merged the CHOICE; it must not merge the
 * MEANINGS. A value area, a swept pool, a fair-value gap and an order block
 * come from four different rules and keep four different hues on the chart
 * (`familyOf` in lib/levels.ts decides, `PriceChart` paints), and until now
 * the only thing on screen mapping hue to kind was the row of switches. Take
 * the switches away without this and the chart grows four colours nobody can
 * name — which is exactly the "merged switch merges the meanings" failure.
 *
 * It is not focusable and not pressable: nothing here does anything, and a
 * dead control that looks live is worse than a caption. The tag on each line
 * still names the kind in words as well, so the colour is a shortcut and
 * never the only carrier.
 *
 * ONLY THE FAMILIES PRESENT ARE LISTED. A hue for a kind this market has none
 * of is a key to a colour that is not on the chart. `other` in particular
 * appears only when the route has sent a kind this client did not recognise —
 * which is worth seeing, because it means the route grew something the chart
 * is drawing in grey.
 */
export function LevelLegend({
  present,
  counts,
  drawn,
}: {
  /** Families the current response actually contains. */
  present: Set<LevelFamily>
  /** How many levels of each family the response carries. */
  counts?: Map<LevelFamily, number>
  /**
   * How many of them are actually on the chart, which at `live` is usually
   * fewer: the distance window and the spent filter both sit between the two
   * numbers. Kept apart from `counts` because "80" over a family the window
   * is showing twelve of is the misreading this legend would otherwise
   * invite — and that difference is what the hidden-count sentence beside it
   * accounts for.
   */
  drawn?: Map<LevelFamily, number>
}) {
  const shown = LEVEL_FAMILIES.filter((f) => present.has(f.key))
  if (shown.length === 0) return null

  // `role="note"` so the label is actually announced: an `aria-label` on a
  // bare span is ignored by most screen readers, and a colour key nobody can
  // hear is the same hole this legend exists to close.
  return (
    <span role="note" aria-label="Level colours" className="ml-2 inline-flex items-center gap-1.5 align-middle">
      {shown.map((family) => {
        const n = counts?.get(family.key)
        const shownNow = drawn?.get(family.key) ?? 0
        return (
          <span
            key={family.key}
            className="text-muted-foreground/70 inline-flex items-center gap-1 fd-caption"
            title={
              family.key === 'profile'
                ? `The activity profile${n ? ` (${n} marks)` : ''} — ${PROFILE_NOTE}`
                : !n
                  ? `Levels in the ${family.label} family`
                  : // The reason the two numbers differ is only true while
                    // something IS being held back. At `everything` they are
                    // equal and the sentence must not claim a window that is
                    // not in force.
                    shownNow === n
                    ? `All ${n} in the ${family.label} family are on the chart`
                    : `${shownNow} of ${n} in the ${family.label} family are on the chart; the rest are spent or outside the distance window`
            }
          >
            <span
              aria-hidden
              className="size-1.5 rounded-full"
              style={{ backgroundColor: family.hue }}
            />
            {family.label}
            {n ? <span className="num text-muted-foreground/40">{shownNow === n ? n : `${shownNow}/${n}`}</span> : null}
          </span>
        )
      })}
    </span>
  )
}
