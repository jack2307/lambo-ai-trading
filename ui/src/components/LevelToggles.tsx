import { useRef } from 'react'

import { LEVEL_FAMILIES, type LevelFamily } from '@/lib/levels'
import { cn } from '@/lib/utils'

/**
 * Which families of level the chart draws.
 *
 * A toolbar of independent switches, not a radiogroup: these are not one
 * choice out of a set, they are four things each separately on or off. The
 * distinction is announced — "profile, toggle button, pressed" — and it is
 * the difference between a reader expecting the others to switch off when
 * they pick one and expecting them not to.
 *
 * ROVING TABINDEX, like the timeframe bar. Five separate tab stops in a chart
 * header is five presses to get past a control most visits never touch; one
 * stop and arrow keys inside it is the toolbar pattern, and it keeps the
 * header's keyboard cost at one regardless of how many families exist.
 *
 * ONLY THE FAMILIES PRESENT ARE SHOWN. A switch for a kind this market has
 * none of is a control that does nothing, and a control that does nothing
 * teaches a reader that the controls do nothing. `other` in particular should
 * appear only when the route has sent a kind this client did not recognise —
 * which is worth seeing, because it means the route grew something the chart
 * is drawing in grey.
 */
export function LevelToggles({
  on,
  present,
  onChange,
  counts,
}: {
  on: Set<LevelFamily>
  /** Families the current response actually contains. */
  present: Set<LevelFamily>
  onChange: (next: Set<LevelFamily>) => void
  /** How many levels of each family, for the title. */
  counts?: Map<LevelFamily, number>
}) {
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const shown = LEVEL_FAMILIES.filter((f) => present.has(f.key))

  if (shown.length === 0) return null

  const toggle = (key: LevelFamily) => {
    const next = new Set(on)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    onChange(next)
  }

  const move = (from: number, step: number) => {
    const next = (from + step + shown.length) % shown.length
    items.current[next]?.focus()
  }

  return (
    <span role="toolbar" aria-label="Chart levels" className="ml-2 inline-flex items-center gap-1 align-middle">
      <span className="text-muted-foreground/50 fd-caption">levels</span>
      {shown.map((family, index) => {
        const active = on.has(family.key)
        const n = counts?.get(family.key)
        return (
          <button
            key={family.key}
            ref={(el) => {
              items.current[index] = el
            }}
            type="button"
            aria-pressed={active}
            tabIndex={index === 0 ? 0 : -1}
            onClick={() => toggle(family.key)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
                event.preventDefault()
                move(index, 1)
              } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
                event.preventDefault()
                move(index, -1)
              }
            }}
            title={
              active
                ? `Hide ${family.label} levels${n ? ` (${n} drawn)` : ''}`
                : `Draw ${family.label} levels${n ? ` (${n} available)` : ''}`
            }
            className={cn(
              'focus-visible:ring-ring inline-flex items-center gap-1 rounded-sm border px-1.5 py-px fd-caption transition-colors duration-100 focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
              active ? 'text-foreground' : 'text-muted-foreground/60 hover:text-muted-foreground',
            )}
            style={{
              // The family's own hue carries the state: a filled dot and a
              // tinted edge when on, a hollow dot and the plain border when
              // off. Colour alone would not say which — hence the fill.
              borderColor: active ? `color-mix(in oklab, ${family.hue} 50%, transparent)` : 'var(--border)',
              backgroundColor: active ? `color-mix(in oklab, ${family.hue} 8%, transparent)` : undefined,
            }}
          >
            <span
              aria-hidden
              className="size-1.5 rounded-full border"
              style={{
                borderColor: family.hue,
                backgroundColor: active ? family.hue : 'transparent',
              }}
            />
            {family.label}
          </button>
        )
      })}
    </span>
  )
}
