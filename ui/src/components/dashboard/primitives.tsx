/**
 * Overview-mode building blocks.
 *
 * Everything here is presentational and takes plain props — nothing in this
 * file fetches, and nothing decides what a number means. See `DESIGN.md`; the
 * rules these encode are the ones most easily lost when a screen is added in a
 * hurry.
 */

import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

/** A raised card. `interactive` adds the lift and the focus ring. */
export function Panel({
  className,
  interactive,
  children,
  ...rest
}: React.ComponentProps<'div'> & { interactive?: boolean }) {
  return (
    <div
      className={cn(
        'panel',
        interactive &&
          'panel-interactive focus-visible:ring-ring cursor-pointer focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--background)] focus-visible:outline-none',
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  )
}

/** Section heading with an optional subtitle and a right-hand action slot. */
export function SectionHeader({
  title,
  subtitle,
  action,
}: {
  title: string
  subtitle?: string
  action?: ReactNode
}) {
  return (
    <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
      <div>
        <h2 className="text-[15px] font-semibold tracking-tight">{title}</h2>
        {subtitle && <p className="text-muted-foreground mt-0.5 text-[13px]">{subtitle}</p>}
      </div>
      {action}
    </div>
  )
}

/**
 * A figure with its unit.
 *
 * The unit is not decoration. `1.033` is not a result and `PF 1.033` is, and
 * every number on this screen has been mistaken for the other kind at least
 * once.
 */
export function Figure({
  value,
  unit,
  tone = 'neutral',
  className,
}: {
  value: string
  unit?: string
  tone?: 'neutral' | 'up' | 'down'
  className?: string
}) {
  return (
    <span
      className={cn(
        'tnum',
        // Up/down are DATA colours. No control anywhere may use them — the
        // moment a green button exists, green stops meaning LC on the chart.
        tone === 'up' && 'text-lc',
        tone === 'down' && 'text-lp',
        className,
      )}
    >
      {value}
      {unit && <span className="text-muted-foreground ml-1 text-[11px]">{unit}</span>}
    </span>
  )
}

/** Uppercase micro-label. */
export function Label({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn('label-micro', className)}>{children}</div>
}

/**
 * A small status pill.
 *
 * `caution` exists for the case this product cares about most: a number that is
 * real arithmetic over data that cannot support it. It is amber rather than red
 * because the number is not wrong — it is unsupported, and those need different
 * words as well as different colours.
 */
export function Pill({
  tone = 'neutral',
  children,
}: {
  tone?: 'neutral' | 'live' | 'caution' | 'blocked'
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[11px] font-medium',
        tone === 'neutral' && 'border-border text-muted-foreground',
        tone === 'live' && 'border-lc/30 text-lc bg-lc/10',
        tone === 'caution' && 'border-caution/30 text-caution bg-caution/10',
        tone === 'blocked' && 'border-lp/30 text-lp bg-lp/10',
      )}
    >
      {tone === 'live' && <span className="bg-lc size-1.5 animate-pulse rounded-full" aria-hidden />}
      {children}
    </span>
  )
}

/** Monogram avatar. A square with a generous radius rather than a circle. */
export function Monogram({
  seed,
  label,
  size = 32,
}: {
  seed: string
  label: string
  size?: number
}) {
  // Deterministic hue from the name, so an agent keeps its colour between
  // renders and between sessions without anyone maintaining a mapping.
  let hash = 0
  for (const character of seed) hash = (hash * 31 + character.charCodeAt(0)) >>> 0
  const hue = hash % 360

  return (
    <span
      aria-hidden
      style={{
        width: size,
        height: size,
        borderRadius: size * 0.32,
        background: `linear-gradient(145deg, oklch(0.55 0.13 ${hue}), oklch(0.4 0.1 ${(hue + 40) % 360}))`,
        fontSize: size * 0.4,
      }}
      className="inline-flex shrink-0 items-center justify-center font-semibold text-white/90"
      title={label}
    >
      {label.slice(0, 1).toUpperCase()}
    </span>
  )
}

/** A ranked rail row: rank, identity, figure. */
export function RailRow({
  rank,
  name,
  meta,
  value,
  delta,
}: {
  rank: number
  name: string
  meta?: string
  value: string
  delta?: { text: string; tone: 'up' | 'down' }
}) {
  return (
    <li className="hover:bg-accent/40 -mx-2 flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors">
      <span className="text-muted-foreground tnum w-4 shrink-0 text-[11px]">{rank}</span>
      <Monogram seed={name} label={name} size={26} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-medium">{name}</span>
        {meta && <span className="text-muted-foreground block truncate text-[11px]">{meta}</span>}
      </span>
      <span className="shrink-0 text-right">
        <Figure value={value} className="block text-[13px]" />
        {delta && <Figure value={delta.text} tone={delta.tone} className="block text-[11px]" />}
      </span>
    </li>
  )
}

/** Filter chips. Controlled; the parent owns the selection. */
export function Chips<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { id: T; label: string }[]
  value: T
  onChange: (next: T) => void
}) {
  return (
    <div className="flex flex-wrap gap-1.5" role="tablist">
      {options.map((option) => (
        <button
          key={option.id}
          type="button"
          role="tab"
          aria-selected={value === option.id}
          onClick={() => onChange(option.id)}
          className={cn(
            'focus-visible:ring-ring rounded-full border px-3 py-1 text-[12px] font-medium transition-colors focus-visible:ring-2 focus-visible:outline-none',
            value === option.id
              ? 'border-primary/40 bg-primary/15 text-foreground'
              : 'border-border text-muted-foreground hover:text-foreground hover:border-foreground/25',
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}

/**
 * Empty state.
 *
 * Says what would fill it and how. "No data" tells a reader nothing they can
 * act on, and this product's empty states are almost always "run the collector
 * for longer", which is actionable.
 */
export function Empty({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="text-muted-foreground px-3 py-8 text-center">
      <p className="text-[13px]">{title}</p>
      {hint && <p className="mt-1 font-mono text-[11px] opacity-80">{hint}</p>}
    </div>
  )
}
