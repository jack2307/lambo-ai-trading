import { useEffect, useRef, useState } from 'react'
import {
  Activity,
  BarChart3,
  ChevronDown,
  ChevronsLeft,
  FlaskConical,
  LayoutDashboard,
  type LucideIcon,
  PanelLeft,
  Settings as SettingsIcon,
  SlidersHorizontal,
  Users,
} from 'lucide-react'

import type { Book, View } from '@/App'
import type { BrokerAccount } from '@/lib/api'
import { cn } from '@/lib/utils'

/**
 * The desk's navigation, and the one decision that comes before navigating.
 *
 * WHY A SIDEBAR AND NOT THE ROW OF PILLS IT REPLACES. Seven views in a flat
 * row say nothing about what they are for, so a reader scans all seven every
 * time. Grouped by what a person is DOING — trading, or researching — the two
 * halves of this desk become visible in the chrome, and the group a page
 * belongs to is a fact you can read without opening it.
 *
 * WHY THE BOOK SWITCHER IS AT THE TOP AND NOT IN THE STRIP. It is the first
 * decision of every visit and it decides what every number on every page
 * means: paper, or an account with money in it. It is not a setting, it is the
 * subject. Putting it where the eye lands first is the point.
 *
 * Hand-written rather than shadcn's sidebar. That brings a provider, a Sheet,
 * a rail, its own skeleton and cookie persistence, plus a dependency this
 * project does not have — for seven links and a switcher.
 */

type Group = { label: string; items: { id: View; label: string; icon: LucideIcon }[] }

/**
 * Grouped by the job, not by the file.
 *
 * `Trade` is what is happening now and what it is happening to; `Research` is
 * what to do next. Settings sits alone at the bottom because it is neither,
 * and because a control that changes how the desk behaves should not be one
 * slip of the cursor from the page you watch money on.
 */
const GROUPS: Group[] = [
  {
    label: 'Trade',
    items: [
      { id: 'desk', label: 'Desk', icon: LayoutDashboard },
      { id: 'floor', label: 'Floor', icon: Users },
      { id: 'tape', label: 'Tape', icon: Activity },
    ],
  },
  {
    label: 'Research',
    items: [
      { id: 'workbench', label: 'Workbench', icon: SlidersHorizontal },
      { id: 'analytics', label: 'Analytics', icon: BarChart3 },
      { id: 'research', label: 'Research', icon: FlaskConical },
    ],
  },
]

const SETTINGS: { id: View; label: string; icon: LucideIcon } = {
  id: 'settings',
  label: 'Settings',
  icon: SettingsIcon,
}

/** Collapse state, remembered per viewer. Same shape as `fd.desk.book`. */
const RAIL_KEY = 'fd.desk.sidebar'

export const readRail = (): boolean => {
  try {
    return localStorage.getItem(RAIL_KEY) === '1'
  } catch {
    return false
  }
}

/* The WRITER lives in `App`, which owns the state. Two writers for one key is
 * how a remembered preference ends up depending on which component rendered
 * last. `RAIL_KEY` is exported in spirit by `readRail` alone. */

interface Props {
  view: View
  onViewChange: (view: View) => void
  book: Book
  onBookChange: (book: Book) => void
  accounts: BrokerAccount[]
  /** Collapsed to the icon rail. */
  railed: boolean
  onRailedChange: (railed: boolean) => void
  /** Open as an overlay on narrow screens. */
  drawerOpen: boolean
  onDrawerOpenChange: (open: boolean) => void
  isLive: (a: BrokerAccount) => boolean
}

export function Sidebar({
  view,
  onViewChange,
  book,
  onBookChange,
  accounts,
  railed,
  onRailedChange,
  drawerOpen,
  onDrawerOpenChange,
  isLive,
}: Props) {
  // Ctrl+B, and not a bare `[`. This desk has text inputs — run ids, guard
  // values, the advisor key — where a bare letter either steals the keystroke
  // or needs a focus guard in every one of them. Ctrl+B has neither problem
  // and is what every editor already means by "toggle the sidebar".
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'b') return
      e.preventDefault()
      onRailedChange(!railed)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [railed, onRailedChange])

  // Escape closes the overlay. Only the overlay: collapsing the rail is not a
  // thing Escape should do, because nothing is covering anything.
  useEffect(() => {
    if (!drawerOpen) return
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onDrawerOpenChange(false)
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [drawerOpen, onDrawerOpenChange])

  const chosen = typeof book === 'number' ? (accounts.find((a) => a.login === book) ?? null) : null
  const go = (next: View) => {
    onViewChange(next)
    onDrawerOpenChange(false)
  }

  const item = (entry: { id: View; label: string; icon: LucideIcon }) => {
    const active = view === entry.id
    const Icon = entry.icon
    return (
      <button
        key={entry.id}
        type="button"
        onClick={() => go(entry.id)}
        aria-current={active ? 'page' : undefined}
        title={railed ? entry.label : undefined}
        className={cn(
          'group relative flex w-full items-center gap-2.5 rounded-sm py-1.5 text-[12px] transition-colors duration-100 motion-reduce:transition-none',
          railed ? 'justify-center px-0' : 'px-2.5',
          active
            ? 'bg-sidebar-accent text-sidebar-primary font-medium'
            : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/60 hover:text-sidebar-foreground',
        )}
      >
        {/* The active mark is a shape as well as a colour: a lime edge down the
            left of the item. Colour alone fails for a reader who cannot see
            it, and fails again on a dim panel at a glance. */}
        <span
          className={cn(
            'bg-sidebar-primary absolute inset-y-1 left-0 w-[2px] rounded-full transition-opacity duration-100 motion-reduce:transition-none',
            active ? 'opacity-100' : 'opacity-0',
          )}
          aria-hidden
        />
        <Icon className="size-3.5 shrink-0" aria-hidden />
        {!railed && <span className="truncate">{entry.label}</span>}
      </button>
    )
  }

  const body = (
    <div className="flex h-full min-h-0 flex-col gap-3 py-3">
      <div className={cn('flex items-center', railed ? 'justify-center px-0' : 'justify-between px-3')}>
        {!railed && (
          <span className="text-sidebar-foreground flex items-baseline gap-2 text-[13px] font-semibold tracking-tight">
            <span className="bg-sidebar-primary size-[7px] rounded-[2px]" aria-hidden />
            flowdesk
          </span>
        )}
        <button
          type="button"
          onClick={() => onRailedChange(!railed)}
          title={`${railed ? 'Expand' : 'Collapse'} sidebar · Ctrl+B`}
          aria-label={`${railed ? 'Expand' : 'Collapse'} sidebar`}
          className="text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-foreground hidden size-6 shrink-0 items-center justify-center rounded-sm transition-colors duration-100 motion-reduce:transition-none lg:flex"
        >
          {railed ? <PanelLeft className="size-3.5" /> : <ChevronsLeft className="size-3.5" />}
        </button>
      </div>

      <BookCard
        book={book}
        onBookChange={onBookChange}
        accounts={accounts}
        chosen={chosen}
        railed={railed}
        isLive={isLive}
      />

      <nav className={cn('flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto', railed ? 'px-2' : 'px-2')} aria-label="Views">
        {GROUPS.map((group, i) => (
          <div key={group.label} className="flex flex-col gap-0.5">
            {railed ? (
              // In the rail the label has nowhere to go, so the separation
              // becomes a line. The grouping survives the collapse; only its
              // name does not.
              i > 0 && <span className="bg-sidebar-border mx-auto my-1 h-px w-5" aria-hidden />
            ) : (
              <span className="text-sidebar-foreground/40 px-2.5 pb-0.5 text-[10px] font-medium tracking-wider uppercase">
                {group.label}
              </span>
            )}
            {group.items.map(item)}
          </div>
        ))}
      </nav>

      <div className={cn('flex flex-col gap-0.5', railed ? 'px-2' : 'px-2')}>
        <span className="bg-sidebar-border mx-2.5 mb-1 h-px" aria-hidden />
        {item(SETTINGS)}
      </div>
    </div>
  )

  return (
    <>
      {/* Wide: in the flow, so the page narrows with it and the chart is never
          covered. */}
      <aside
        className={cn(
          'bg-sidebar border-sidebar-border hidden shrink-0 border-r transition-[width] duration-150 motion-reduce:transition-none lg:block',
          railed ? 'w-[52px]' : 'w-[216px]',
        )}
      >
        {body}
      </aside>

      {/* Narrow: an overlay, because squeezing a 380px chart to make room for
          navigation is the wrong trade on the one screen this desk exists for. */}
      {drawerOpen && (
        <>
          <button
            type="button"
            aria-label="Close menu"
            onClick={() => onDrawerOpenChange(false)}
            className="fixed inset-0 z-30 bg-black/60 backdrop-blur-[2px] lg:hidden"
          />
          <aside className="bg-sidebar border-sidebar-border fixed inset-y-0 left-0 z-40 w-[216px] border-r lg:hidden">
            {body}
          </aside>
        </>
      )}
    </>
  )
}

/**
 * A display name, from the registry ID rather than its label.
 *
 * `label` is a description meant for a settings page — "Vantage cent - REAL
 * MONEY", "Vantage demo - cent mirror" — and in a 216px rail it truncated to
 * "Vantage cent - REAL MON…", which is a name cut off mid-warning. The ID is
 * already the short, stable handle: `vantage-cent`.
 *
 * Whether the account is real money is a TAG beside the name, not words inside
 * it. A fact that can be rendered as a mark should not be spelled into a string
 * that has to fit.
 */
function displayName(a: BrokerAccount): string {
  const words = a.id.replace(/[-_]+/g, ' ').trim()
  return words.charAt(0).toUpperCase() + words.slice(1)
}

/**
 * Which book, and what it is worth — the first thing on the panel.
 *
 * The card shows the CURRENT book only and opens the list on click. It used to
 * render the choices permanently underneath, so the selected book appeared
 * twice, a disabled account took a row it could not be picked from, and the
 * whole thing read as a form control somebody left open.
 *
 * The lime mark appears only on REAL MONEY. It is the one place the chrome
 * accent is spent on state rather than on navigation, and it earns that:
 * "am I on the funded account" is the question the owner opens this desk
 * asking, and a demo and a funded account otherwise look identical.
 */
function BookCard({
  book,
  onBookChange,
  accounts,
  chosen,
  railed,
  isLive,
}: {
  book: Book
  onBookChange: (book: Book) => void
  accounts: BrokerAccount[]
  chosen: BrokerAccount | null
  railed: boolean
  isLive: (a: BrokerAccount) => boolean
}) {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])

  const real = chosen?.real_money === true
  const live = chosen ? isLive(chosen) : false
  const name = chosen ? displayName(chosen) : 'Paper'

  // Books the registry says this account should mirror, against the ones
  // actually reporting. This count answers a question the position cards raise
  // one at a time: is that red edge one book, or the whole desk?
  const want = chosen?.runs?.length ?? 0
  const have = chosen?.mirroring?.length ?? 0
  const short = chosen != null && want > 0 && have < want

  const equity =
    chosen?.equity != null
      ? `${Math.round(chosen.equity).toLocaleString('en-US')} ${chosen.currency ?? ''}`.trim()
      : null

  // Enabled first, disabled last and greyed. Hiding a disabled account would
  // make one somebody switched off look like one nobody ever configured; the
  // two are different and only one of them is a thing to go and fix.
  const enabled = accounts.filter((a) => a.enabled)
  const disabled = accounts.filter((a) => !a.enabled)

  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      setOpen(false)
      trigger.current?.focus()
    }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  }, [open])

  if (railed) {
    const initial = name.trim().charAt(0).toUpperCase() || 'P'
    return (
      <div className="px-2">
        <button
          type="button"
          title={`${name}${equity ? ` · ${equity}` : ''}${real ? ' · real money' : ''}`}
          onClick={() => onBookChange(book === 'paper' && enabled[0] ? enabled[0].login : 'paper')}
          className={cn(
            'relative flex size-7 items-center justify-center rounded-sm border text-[11px] font-semibold transition-colors duration-100 motion-reduce:transition-none',
            real
              ? 'border-sidebar-primary/60 text-sidebar-primary bg-sidebar-accent'
              : 'border-sidebar-border text-sidebar-foreground/70 hover:bg-sidebar-accent',
          )}
        >
          {initial}
          {live && <span className="bg-lc absolute -top-px -right-px size-1.5 rounded-full" aria-hidden />}
        </button>
      </div>
    )
  }

  /** One row of the menu. Arrow keys walk them; Enter and Space are the
   *  button's own. */
  const choice = (
    key: string,
    label: string,
    selected: boolean,
    opts: { realMoney?: boolean; disabled?: boolean; note?: string; onPick: () => void },
    index: number,
  ) => (
    <button
      key={key}
      ref={(el) => {
        items.current[index] = el
      }}
      type="button"
      role="option"
      aria-selected={selected}
      disabled={opts.disabled}
      onClick={() => {
        opts.onPick()
        setOpen(false)
        trigger.current?.focus()
      }}
      onKeyDown={(e) => {
        if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
        e.preventDefault()
        const next = index + (e.key === 'ArrowDown' ? 1 : -1)
        items.current[(next + items.current.length) % items.current.length]?.focus()
      }}
      className={cn(
        'flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[12px] transition-colors duration-100 motion-reduce:transition-none',
        selected ? 'text-sidebar-primary font-medium' : 'text-sidebar-foreground/80',
        opts.disabled
          ? 'cursor-not-allowed opacity-45'
          : 'hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:outline-none',
      )}
    >
      <span
        className={cn('w-[2px] self-stretch rounded-full', selected ? 'bg-sidebar-primary' : 'bg-transparent')}
        aria-hidden
      />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {opts.realMoney && <RealTag />}
      {opts.note && <span className="text-sidebar-foreground/45 text-[10px]">{opts.note}</span>}
    </button>
  )

  let i = -1
  return (
    <div className="relative px-3" ref={box}>
      <button
        ref={trigger}
        type="button"
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (e.key !== 'ArrowDown') return
          e.preventDefault()
          setOpen(true)
          window.setTimeout(() => items.current[0]?.focus(), 0)
        }}
        aria-haspopup="listbox"
        aria-expanded={open}
        className={cn(
          'relative w-full overflow-hidden rounded-sm border px-2.5 py-2 text-left transition-colors duration-100 motion-reduce:transition-none',
          'bg-sidebar-accent/50 hover:bg-sidebar-accent',
          real ? 'border-sidebar-primary/35' : 'border-sidebar-border',
        )}
      >
        {real && <span className="bg-sidebar-primary absolute inset-y-0 left-0 w-[2px]" aria-hidden />}
        <div className="flex items-center gap-1.5">
          <span className="text-sidebar-foreground min-w-0 flex-1 truncate text-[12px] font-medium">{name}</span>
          {real && <RealTag />}
          {chosen && (
            <span
              className={cn('size-1.5 shrink-0 rounded-full', live ? 'bg-lc' : 'bg-muted-foreground/50')}
              title={live ? 'reporting' : 'not reporting'}
              aria-hidden
            />
          )}
          <ChevronDown
            className={cn(
              'text-sidebar-foreground/40 size-3 shrink-0 transition-transform duration-100 motion-reduce:transition-none',
              open && 'rotate-180',
            )}
            aria-hidden
          />
        </div>

        {chosen ? (
          <div className="num text-sidebar-foreground/80 mt-0.5 text-[13px] tabular-nums">{equity ?? '—'}</div>
        ) : (
          <div className="text-sidebar-foreground/50 mt-0.5 text-[11px]">the decisions, not the money</div>
        )}

        {short && (
          <div className="text-lp mt-1 text-[10px]">
            {have} of {want} mirrored
          </div>
        )}
      </button>

      {open && (
        <div
          role="listbox"
          className="bg-sidebar border-sidebar-border absolute inset-x-3 top-full z-50 mt-1 overflow-hidden rounded-sm border py-1 shadow-lg shadow-black/40"
        >
          {choice('paper', 'Paper', book === 'paper', { onPick: () => onBookChange('paper') }, ++i)}
          {enabled.map((a) =>
            choice(
              String(a.login),
              displayName(a),
              book === a.login,
              {
                realMoney: a.real_money,
                disabled: !isLive(a),
                note: isLive(a) ? undefined : 'not running',
                onPick: () => onBookChange(a.login),
              },
              ++i,
            ),
          )}
          {disabled.map((a) =>
            choice(
              String(a.login),
              displayName(a),
              false,
              { realMoney: a.real_money, disabled: true, note: 'disabled', onPick: () => {} },
              ++i,
            ),
          )}
        </div>
      )}
    </div>
  )
}

/** Real money, as a mark rather than as words inside a name that has to fit. */
function RealTag() {
  return (
    <span
      className="border-sidebar-primary/50 text-sidebar-primary shrink-0 rounded-[3px] border px-1 text-[9px] leading-[1.4] font-semibold tracking-wide"
      title="real money"
    >
      REAL
    </span>
  )
}
