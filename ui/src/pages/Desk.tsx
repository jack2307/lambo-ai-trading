/**
 * The Desk — every paper run at once.
 *
 * Terminal mode (see `ui/DESIGN.md`): dense, flat, small radius, no cards, no
 * hero. The table fills the viewport and a drill-down pane sits beside it from
 * 1280px up, below it under that. Its job is the opposite of the Research
 * screen's: someone already working reads ten rows of numbers without
 * scrolling, and reaches one run's fills in a keystroke.
 *
 * ```
 * ┌ summary strip ───────────────────────────────────────────────┐
 * ├ runs table (fills the width) ──────────┬ drill-down ─────────┤
 * │ one row per run, sorted by id           │ equity curve        │
 * │                                         │ fills · events      │
 * └─────────────────────────────────────────┴─────────────────────┘
 * ```
 *
 * Every figure carries its unit, because `1.03` is not a profit factor and
 * `−277` is not a loss until it says dollars.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { Skeleton } from '@/components/ui/skeleton'
import { api, type BacktestTrade, type PaperEvent, type PaperRun, type PaperRunDetail } from '@/lib/api'
import { clock, num } from '@/lib/format'
import { cn } from '@/lib/utils'

/** Height of the sticky app bar, which this page fills the rest of the viewport under. */
const APP_BAR = 45

/**
 * A run whose last closed bar is older than this is stale: the poller has
 * stopped, or the market is shut. Forty-five minutes is three 15m bars, so a
 * single late bar does not raise the flag.
 */
const STALE_MS = 45 * 60_000

/** The selected run, remembered per browser. A reload should land where you were. */
const SELECTED_KEY = 'fd.desk.selected'

/** `/status` sends the last ten fills per run; a count from them can only be a floor. */
const LAST_FILLS_CAP = 10

const readSelected = (): string | null => {
  try {
    return localStorage.getItem(SELECTED_KEY)
  } catch {
    return null
  }
}
const writeSelected = (id: string) => {
  try {
    localStorage.setItem(SELECTED_KEY, id)
  } catch {
    /* private mode: the choice lasts the page */
  }
}

/* ------------------------------------------------------------- formatting */

const MINUS = '−'

/** A signed dollar figure. The sign is the point, so it is never dropped. */
const signedUsd = (v: number | null | undefined): string => {
  if (v == null || !Number.isFinite(v)) return '—'
  const rounded = Math.abs(v) >= 1000 ? Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: 0 }) : Math.abs(v).toFixed(0)
  return `${v < 0 ? MINUS : '+'}$${rounded}`
}

const signedR = (v: number | null | undefined): string =>
  v == null || !Number.isFinite(v) ? '—' : `${v < 0 ? MINUS : '+'}${Math.abs(v).toFixed(2)}R`

const ago = (ms: number | null | undefined, now: number): string => {
  if (!ms) return '—'
  const minutes = Math.max(0, Math.round((now - ms) / 60_000))
  if (minutes < 60) return `${minutes} min`
  if (minutes < 48 * 60) return `${(minutes / 60).toFixed(1)} h`
  return `${Math.round(minutes / 1440)} d`
}

/** `HH:MMZ` — the wall clock the whole product reads in, UTC. */
const zulu = (ms: number | null | undefined): string =>
  ms == null || !Number.isFinite(ms) ? '—' : `${clock(ms)}Z`

/**
 * A price at the precision its own market quotes in. `format.price` rounds for
 * a gold chart; a euro printed to one decimal is 1.2, which is not a price.
 */
const quote = (v: number | null | undefined): string => {
  if (v == null || !Number.isFinite(v)) return '—'
  const magnitude = Math.abs(v)
  if (magnitude >= 1000) return v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  if (magnitude >= 10) return v.toFixed(2)
  return v.toFixed(5)
}

/** `MM-DD HH:MMZ`, short enough for an axis label and a fills column. */
const shortStamp = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms)) return '—'
  const iso = new Date(ms).toISOString()
  return `${iso.slice(5, 10)} ${iso.slice(11, 16)}Z`
}

/** Midnight UTC today, the boundary "closed today" is measured from. */
const startOfDayUtc = (now: number): number => {
  const d = new Date(now)
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
}

/**
 * Guard labels, short. The wire spells them `WEEKEND_FLAT`; a chip has room
 * for `weekend` and the column header already says these are guards.
 */
const GUARD_LABEL: Record<string, string> = {
  WEEKEND_FLAT: 'weekend',
  NEWS_FLAT: 'news',
  OPEN_LOSS_CAP: 'open loss',
  DAILY_TRADE_CAP: 'daily cap',
  DAILY_LOSS_LIMIT: 'loss limit',
  MAX_CONCURRENT: 'concurrent',
  NOTIONAL_CAP: 'notional',
  COOLDOWN: 'cooldown',
  NO_RISK_UNIT: 'no risk unit',
}
const guardLabel = (key: string) => GUARD_LABEL[key] ?? key.toLowerCase().replace(/_/g, ' ')

const isStale = (run: PaperRun, now: number) => (run.last_bar_time ? now - run.last_bar_time > STALE_MS : true)

/* ------------------------------------------------------------------ page */

export function Desk() {
  const [runs, setRuns] = useState<PaperRun[] | null>(null)
  const [statusError, setStatusError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(readSelected)
  const [detail, setDetail] = useState<PaperRunDetail | null>(null)
  // Tagged with the run it belongs to, so an error from the row you just left
  // does not sit over the row you just opened.
  const [detailError, setDetailError] = useState<{ id: string; message: string } | null>(null)
  const [now, setNow] = useState(() => Date.now())

  // The book changes when a bar arrives — every five or fifteen minutes — so a
  // poll every 20 s is generous and costs one small JSON.
  useEffect(() => {
    let alive = true
    const read = () =>
      api
        .paperStatus()
        .then((r) => {
          if (!alive) return
          setRuns(r.runs)
          setStatusError(null)
          setNow(Date.now())
        })
        .catch((e: Error) => {
          if (alive) setStatusError(e.message)
        })
    read()
    const timer = window.setInterval(read, 20_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  const sorted = useMemo(() => [...(runs ?? [])].sort((a, b) => a.id.localeCompare(b.id)), [runs])

  // The row the drill-down reads, derived rather than stored: a remembered id
  // that no longer names a run (renamed, never started) falls to the first row
  // without spending a render correcting itself.
  const activeId = selected && sorted.some((r) => r.id === selected) ? selected : (sorted[0]?.id ?? null)

  // The selected run in full: at once on selection, then every 60 s. The detail
  // is heavier than the status and changes no faster. Nothing is cleared here —
  // the pane shows its skeleton while `detail` still belongs to the previous run.
  useEffect(() => {
    if (!activeId) return
    let alive = true
    const read = () =>
      api
        .paperRun(activeId, 120)
        .then((d) => {
          if (!alive) return
          setDetail(d)
          setDetailError(null)
        })
        .catch((e: Error) => {
          if (alive) setDetailError({ id: activeId, message: e.message })
        })
    read()
    const timer = window.setInterval(read, 60_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [activeId])

  const pick = useCallback((id: string) => {
    setSelected(id)
    writeSelected(id)
  }, [])

  return (
    <div
      className="flex min-h-0 flex-col overflow-hidden"
      style={{ height: `calc(100dvh - ${APP_BAR}px)` }}
    >
      <SummaryStrip runs={sorted} now={now} loading={runs === null} error={statusError} />

      {statusError && runs === null ? (
        <p className="text-destructive px-3 py-4 text-[13px]">
          Could not read the paper status: {statusError}
        </p>
      ) : runs === null ? (
        <div className="space-y-2 p-3">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <Skeleton key={i} className="h-6 w-full" />
          ))}
        </div>
      ) : sorted.length === 0 ? (
        <NoRuns />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col overflow-auto xl:flex-row xl:overflow-hidden">
          <div className="min-w-0 shrink-0 overflow-x-auto xl:h-full xl:flex-1 xl:shrink xl:overflow-auto">
            <RunsTable runs={sorted} now={now} selected={activeId} onPick={pick} />
            {/* The fills have seven columns and the rail has 460px; below the
                runs table they get the width, and the space a ten-row table
                leaves empty. Narrower than xl they stay in the rail. */}
            <div className="border-border hidden border-t xl:block">
              <FillsSection detail={detail && detail.run.id === activeId ? detail : null} />
            </div>
          </div>
          <div className="min-w-0 border-t xl:h-full xl:w-[460px] xl:shrink-0 xl:overflow-y-auto xl:border-t-0 xl:border-l">
            <Drilldown
              detail={detail && detail.run.id === activeId ? detail : null}
              summary={sorted.find((r) => r.id === activeId) ?? null}
              error={detailError && detailError.id === activeId ? detailError.message : null}
              now={now}
            />
          </div>
        </div>
      )}
    </div>
  )
}

/* --------------------------------------------------------- summary strip */

function SummaryStrip({
  runs,
  now,
  loading,
  error,
}: {
  runs: PaperRun[]
  now: number
  loading: boolean
  error: string | null
}) {
  const stats = useMemo(() => {
    const midnight = startOfDayUtc(now)
    let stale = 0
    let net = 0
    let today = 0
    let capped = false
    let blackout: { time: number; name: string } | null = null
    for (const run of runs) {
      if (isStale(run, now)) stale += 1
      net += run.net_usd
      const fills = run.last_fills.filter((t) => t.exitTime >= midnight)
      today += fills.length
      // The status route sends ten fills per run; when all ten closed today the
      // count is a floor, not a total, and the strip says so with a `≥`.
      if (run.last_fills.length >= LAST_FILLS_CAP && fills.length === run.last_fills.length) capped = true
      const next = run.news.next_blackout
      if (next && (!blackout || next.time < blackout.time)) {
        blackout = { time: next.time, name: next.name ?? next.currency }
      }
    }
    return { stale, net, today, capped, blackout }
  }, [runs, now])

  if (loading) {
    return (
      <div className="flex h-8 shrink-0 items-center gap-4 border-b px-3">
        <Skeleton className="h-3 w-80" />
      </div>
    )
  }

  return (
    <div className="text-muted-foreground flex h-8 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b px-3 text-[11px]">
      <span className="num text-foreground">
        {runs.length} <span className="text-muted-foreground">run{runs.length === 1 ? '' : 's'}</span>
      </span>
      <span className="num">
        <span className="text-lc">{runs.length - stats.stale} fed</span>
        {' · '}
        <span className={stats.stale > 0 ? 'text-caution' : undefined}>{stats.stale} stale</span>
      </span>
      <span className="num">
        net{' '}
        <span className={stats.net > 0 ? 'text-lc' : stats.net < 0 ? 'text-lp' : 'text-foreground'}>
          {signedUsd(stats.net)}
        </span>
      </span>
      <span className="num" title="Closed since 00:00 UTC, counted from the last ten fills each run reports.">
        {stats.capped ? '≥' : ''}
        {stats.today} <span className="text-muted-foreground">closed today</span>
      </span>
      <span className="num">
        next blackout{' '}
        {stats.blackout ? (
          <span className="text-caution">
            {stats.blackout.name} {zulu(stats.blackout.time)}
          </span>
        ) : (
          <span>none scheduled</span>
        )}
      </span>
      {error && <span className="text-destructive ml-auto truncate">status: {error}</span>}
    </div>
  )
}

function NoRuns() {
  return (
    <div className="text-muted-foreground min-h-0 flex-1 px-4 py-8 text-[13px]">
      <p className="text-foreground">No paper run is registered.</p>
      <p className="mt-1">Start the ten registered runs, then start the pollers that feed them closed bars:</p>
      <pre className="border-border bg-card text-foreground mt-3 w-fit rounded-md border px-3 py-2 font-mono text-[11px] leading-relaxed">
        <code>{'python py/live/start_runs.py\npowershell -File py\\live\\start_pollers.ps1'}</code>
      </pre>
      <p className="mt-2 text-[11px]">
        Nothing here reaches a broker: a run steps on closed bars and writes a book to disk.
      </p>
    </div>
  )
}

/* ------------------------------------------------------------ runs table */

const RUN_COLS =
  'grid-cols-[minmax(150px,1.4fr)_80px_104px_66px_74px_68px_80px_72px_minmax(140px,1fr)_minmax(130px,1fr)_86px]'

function RunsTable({
  runs,
  now,
  selected,
  onPick,
}: {
  runs: PaperRun[]
  now: number
  selected: string | null
  onPick: (id: string) => void
}) {
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([])

  // ↑/↓ walk the rows, Enter opens the one under the cursor — a button already
  // fires its click on Enter, so the only thing missing is the walk.
  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, runs.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  return (
    <div className="min-w-[1130px]" role="group" aria-label="Paper runs">
      <div
        className={cn(
          'text-muted-foreground bg-background sticky top-0 z-10 grid items-center gap-2 border-b px-3 py-1 text-[10px] tracking-wide uppercase',
          RUN_COLS,
        )}
      >
        <span>run</span>
        <span>market:tf</span>
        <span>strategy</span>
        <span>status</span>
        <span className="text-right">bars</span>
        <span className="text-right">trades</span>
        <span className="text-right">net</span>
        <span className="text-right">PF</span>
        <span>open position</span>
        <span>guards</span>
        <span className="text-right">last bar</span>
      </div>

      {runs.map((run, index) => {
        const stale = isStale(run, now)
        const isOn = run.id === selected
        return (
          <button
            key={run.id}
            type="button"
            ref={(el) => {
              rowRefs.current[index] = el
            }}
            onClick={() => onPick(run.id)}
            onKeyDown={(e) => onKeyDown(e, index)}
            aria-pressed={isOn}
            className={cn(
              'hover:bg-accent/60 focus-visible:ring-ring grid w-full items-center gap-2 border-b px-3 py-[5px] text-left text-xs transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none',
              RUN_COLS,
              isOn && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
            )}
          >
            <span className="min-w-0 truncate">
              <span className="num">{run.id}</span>
              {run.label && <span className="text-muted-foreground ml-2 text-[10px]">{run.label}</span>}
            </span>
            <span className="num text-muted-foreground truncate">
              {run.market}:{run.tf}
            </span>
            <span className="text-muted-foreground truncate">{run.strategy}</span>
            <StatusPill stale={stale} />
            <span className="num text-right">
              {run.bars_seen}
              <span className="text-muted-foreground"> bars</span>
            </span>
            <span className="num text-right">
              {run.trades}
              <span className="text-muted-foreground"> fills</span>
            </span>
            <span
              className={cn(
                'num text-right',
                run.net_usd > 0 && 'text-lc',
                run.net_usd < 0 && 'text-lp',
                run.net_usd === 0 && 'text-muted-foreground',
              )}
            >
              {signedUsd(run.net_usd)}
            </span>
            <span className="num text-muted-foreground text-right">
              {run.profit_factor == null ? '—' : `PF ${num(run.profit_factor)}`}
            </span>
            <OpenCell run={run} />
            <GuardChips run={run} />
            <span className="num text-muted-foreground text-right">
              {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'no bar yet'}
            </span>
          </button>
        )
      })}
    </div>
  )
}

/**
 * Fed or stale. The dot pulses so a glance across ten rows finds the live ones,
 * and it stops pulsing when the reader's system asks for less motion.
 */
function StatusPill({ stale }: { stale: boolean }) {
  return (
    <span
      className={cn(
        'inline-flex w-fit items-center gap-1 rounded-sm border px-1.5 py-px text-[10px]',
        stale ? 'border-caution/30 bg-caution/10 text-caution' : 'border-lc/30 bg-lc/10 text-lc',
      )}
    >
      <span
        className={cn(
          'size-1.5 rounded-full',
          stale ? 'bg-caution' : 'bg-lc animate-pulse motion-reduce:animate-none',
        )}
        aria-hidden
      />
      {stale ? 'stale' : 'fed'}
    </span>
  )
}

function OpenCell({ run }: { run: PaperRun }) {
  if (!run.open) return <span className="text-muted-foreground truncate">flat</span>
  const open = run.open
  return (
    <span className="num truncate">
      <span className={open.side === 'LONG' ? 'text-lc' : 'text-lp'}>{open.side.toLowerCase()}</span>
      <span className="text-muted-foreground"> · </span>
      {num(open.lots, open.lots >= 100 ? 0 : 2)}
      <span className="text-muted-foreground"> lots · </span>
      <span className={open.unrealised_usd_at_last_close >= 0 ? 'text-lc' : 'text-lp'}>
        {signedUsd(open.unrealised_usd_at_last_close)}
      </span>
    </span>
  )
}

/** Only the guards that actually fired, so an empty cell means a clean run. */
function GuardChips({ run }: { run: PaperRun }) {
  const closed = Object.entries(run.closed_by_guard).filter(([, n]) => n > 0)
  const refused = Object.values(run.skipped_by_guard).reduce((sum, n) => sum + n, 0)
  const chips: string[] = closed.map(([key, n]) => `${guardLabel(key)} ×${n}`)
  if (refused > 0) chips.push(`refused ×${refused}`)
  if (run.sized_down > 0) chips.push(`sized down ×${run.sized_down}`)
  if (chips.length === 0) {
    return <span className="text-muted-foreground/60 truncate text-[10px]">{run.guards ? 'none fired' : 'guards off'}</span>
  }
  return (
    <span className="flex min-w-0 flex-wrap gap-1">
      {chips.map((chip) => (
        <span key={chip} className="border-border text-muted-foreground num rounded-sm border px-1 py-px text-[10px]">
          {chip}
        </span>
      ))}
    </span>
  )
}

/* ------------------------------------------------------------- drilldown */

function Drilldown({
  detail,
  summary,
  error,
  now,
}: {
  detail: PaperRunDetail | null
  summary: PaperRun | null
  error: string | null
  now: number
}) {
  if (error) {
    return (
      <div className="p-3">
        <Heading>Drill-down</Heading>
        <p className="text-destructive mt-1 text-[12px]">{error}</p>
      </div>
    )
  }
  if (!detail) {
    return (
      <div className="space-y-3 p-3">
        <Skeleton className="h-3 w-40" />
        <Skeleton className="h-[150px] w-full" />
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-5/6" />
      </div>
    )
  }

  const run = detail.run
  const stale = summary ? isStale(summary, now) : isStale(run, now)

  return (
    <div className="divide-border divide-y">
      <section className="px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="num text-[13px] font-medium">{run.id}</span>
          <StatusPill stale={stale} />
          <span className="text-muted-foreground num ml-auto text-[10px]">
            {run.market}:{run.tf} · last bar {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'none'}
          </span>
        </div>
        {run.label && <p className="text-muted-foreground mt-1 text-[11px] leading-snug">{run.label}</p>}
        <ConfigLine run={run} />
      </section>

      <section className="px-3 py-2">
        <Heading>Equity</Heading>
        <EquityCurve points={detail.equity_curve} />
        <p className="text-muted-foreground num mt-1 text-[10px]">
          {run.trades} fill{run.trades === 1 ? '' : 's'} · net {signedUsd(run.net_usd)} ·{' '}
          {run.profit_factor == null ? 'no PF yet' : `PF ${num(run.profit_factor)}`} ·{' '}
          {run.bars_seen} bars seen{run.gaps > 0 ? ` · ${run.gaps} gap${run.gaps === 1 ? '' : 's'}` : ''}
          {run.trades > 0 && run.trades < 30 && (
            <span className="text-caution"> · {run.trades} fills is too few to read as a result</span>
          )}
        </p>
      </section>

      <section className="px-3 py-2 xl:hidden">
        <Heading>
          Fills <span className="text-muted-foreground/70 num">{detail.fills.length}</span>
        </Heading>
        <FillsTable fills={detail.fills} />
      </section>

      <section className="px-3 py-2">
        <Heading>
          Events <span className="text-muted-foreground/70 num">{detail.events.length}</span>
        </Heading>
        <EventsList events={detail.events} />
      </section>
    </div>
  )
}

/**
 * The fills of the selected run, for the wide layout's left column.
 *
 * The same table the rail shows on a narrow screen; here it has the room
 * its seven columns want, under a runs table that rarely fills the height.
 */
function FillsSection({ detail }: { detail: PaperRunDetail | null }) {
  if (!detail) {
    return (
      <div className="space-y-2 px-3 py-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="h-3 w-full" />
      </div>
    )
  }
  return (
    <section className="px-3 py-2">
      <Heading>
        Fills <span className="text-muted-foreground/70 num">{detail.run.id}</span>{' '}
        <span className="text-muted-foreground/70 num">{detail.fills.length}</span>
      </Heading>
      <FillsTable fills={detail.fills} />
    </section>
  )
}

function Heading({ children }: { children: React.ReactNode }) {
  return <h2 className="text-muted-foreground mb-1 text-[10px] font-semibold tracking-wide uppercase">{children}</h2>
}

/** Strategy, parameters, filters and whether the guards are enforced — one line. */
function ConfigLine({ run }: { run: PaperRun }) {
  const params = Object.entries(run.params)
    .map(([k, v]) => `${k}=${Number.isInteger(v) ? v : num(v, 2)}`)
    .join(' ')
  return (
    <p className="text-muted-foreground num mt-1.5 text-[10px] leading-relaxed break-words">
      <span className="text-foreground/80">{run.strategy}</span>
      {params && <> · {params}</>}
      {run.filters.length > 0 && <> · {run.filters.join(', ')}</>}
      {' · '}
      <span className={run.guards ? 'text-lc' : 'text-caution'}>{run.guards ? 'guards on' : 'guards off'}</span>
      {' · '}
      {run.news.events_loaded} news events
      {run.news.next_blackout && (
        <>
          {' · next blackout '}
          <span className="text-caution">
            {run.news.next_blackout.name ?? run.news.next_blackout.currency} {zulu(run.news.next_blackout.time)}
          </span>
        </>
      )}
    </p>
  )
}

/* ---------------------------------------------------------- equity curve */

/**
 * The plot's own coordinate space. The margins are part of the `viewBox` so
 * the outermost label — a five-figure dollar amount on the left, a stamp at
 * each end — has room inside the box and nothing clips when the SVG is scaled
 * to the pane's width.
 */
const PLOT = { w: 700, h: 168, l: 62, r: 16, t: 12, b: 26 }

/**
 * A stepped equity curve, drawn by hand.
 *
 * Equity changes at a fill and holds between them, so the line steps rather
 * than slopes: a diagonal between two fills would draw money the book never
 * had. The curve is not sorted on the wire — warm-up trades close before the
 * run's own start stamp — so it is sorted here before anything is measured.
 */
function EquityCurve({ points }: { points: [number, number][] }) {
  if (!points || points.length === 0) {
    return (
      <p className="text-muted-foreground px-1 py-6 text-center text-[11px]">
        No curve yet — the run has closed no trade.
      </p>
    )
  }

  const sorted = [...points].sort((a, b) => a[0] - b[0])
  const first = sorted[0]
  const last = sorted[sorted.length - 1]
  const start = first[1]
  const times = sorted.map((p) => p[0])
  const values = sorted.map((p) => p[1])
  const tMin = Math.min(...times)
  const tMax = Math.max(...times)
  let vMin = Math.min(...values)
  let vMax = Math.max(...values)
  // One point, or a book that has not moved: a zero range would divide by zero
  // and a flat line pinned to an edge reads as a bug. Give it a dollar either
  // way, and say nothing about a min and a max the book never had.
  const flat = !(vMax - vMin > 1e-9)
  if (flat) {
    vMin -= 1
    vMax += 1
  }

  const { w, h, l, r, t, b } = PLOT
  const boxW = l + w + r
  const boxH = t + h + b
  const bottom = t + h
  const x = (ms: number) => (tMax === tMin ? l + w / 2 : l + ((ms - tMin) / (tMax - tMin)) * w)
  const y = (v: number) => t + ((vMax - v) / (vMax - vMin)) * h

  let line = `M ${x(first[0]).toFixed(1)} ${y(first[1]).toFixed(1)}`
  for (let i = 1; i < sorted.length; i += 1) {
    const px = x(sorted[i][0]).toFixed(1)
    line += ` L ${px} ${y(sorted[i - 1][1]).toFixed(1)} L ${px} ${y(sorted[i][1]).toFixed(1)}`
  }
  const area = `${line} L ${x(last[0]).toFixed(1)} ${bottom} L ${x(first[0]).toFixed(1)} ${bottom} Z`

  const up = last[1] >= start
  const dollars = (v: number) => `$${Math.round(v).toLocaleString('en-US')}`

  return (
    <svg
      viewBox={`0 0 ${boxW} ${boxH}`}
      className={cn('w-full', up ? 'text-lc' : 'text-lp')}
      role="img"
      aria-label={`Equity from ${dollars(start)} to ${dollars(last[1])} over ${sorted.length} point${sorted.length === 1 ? '' : 's'}`}
    >
      {/* The plot's floor and left edge: a frame, not a grid. */}
      <path
        d={`M ${l} ${t} L ${l} ${bottom} L ${l + w} ${bottom}`}
        className="stroke-border"
        strokeWidth={1}
        fill="none"
      />
      {/* Where the run started. Everything above this line is profit. */}
      <line
        x1={l}
        x2={l + w}
        y1={y(start)}
        y2={y(start)}
        className="stroke-muted-foreground"
        strokeWidth={1}
        strokeDasharray="4 4"
        opacity={0.6}
      />
      <path d={area} fill="currentColor" opacity={0.12} stroke="none" />
      <path d={line} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round" />
      <circle cx={x(last[0])} cy={y(last[1])} r={3} fill="currentColor" />

      {/* y: the floor, the start, the ceiling — the two ends only when the
          book actually reached them. */}
      {!flat && (
        <text x={l - 6} y={t + 4} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
          {dollars(vMax)}
        </text>
      )}
      <text x={l - 6} y={y(start) + 3.5} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
        {dollars(start)}
      </text>
      {!flat && (
        <text x={l - 6} y={bottom} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
          {dollars(vMin)}
        </text>
      )}

      {/* x: first and last stamp, no ticks between — the steps carry the rest. */}
      <text x={l} y={boxH - 8} textAnchor="start" className="fill-muted-foreground" fontSize={10}>
        {shortStamp(tMin)}
      </text>
      <text x={l + w} y={boxH - 8} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
        {shortStamp(tMax)}
      </text>
    </svg>
  )
}

/* ------------------------------------------------------------------ fills */

const FILL_COLS = 'grid-cols-[96px_44px_56px_minmax(120px,1fr)_96px_72px_64px]'

function FillsTable({ fills }: { fills: BacktestTrade[] }) {
  if (fills.length === 0) {
    return (
      <p className="text-muted-foreground py-2 text-[11px]">
        None yet — the first live bar decides; the warm-up bars do not.
      </p>
    )
  }
  return (
    <div className="overflow-x-auto">
      <div className="min-w-[600px]">
        <div
          className={cn(
            'text-muted-foreground grid items-center gap-2 border-b pb-1 text-[10px] tracking-wide uppercase',
            FILL_COLS,
          )}
        >
          <span>exit time</span>
          <span>side</span>
          <span className="text-right">lots</span>
          <span className="text-right">entry → exit</span>
          <span>exit reason</span>
          <span className="text-right">P&amp;L</span>
          <span className="text-right">R</span>
        </div>
        {[...fills].reverse().map((fill, index) => (
          <div
            key={`${fill.entryTime}-${fill.exitTime}-${index}`}
            className={cn('grid items-center gap-2 border-b py-[3px] text-[11px] last:border-0', FILL_COLS)}
          >
            <span className="num text-muted-foreground">{shortStamp(fill.exitTime)}</span>
            <span className={cn('num', fill.direction === 'LONG' ? 'text-lc' : 'text-lp')}>
              {fill.direction.toLowerCase()}
            </span>
            <span className="num text-right">{num(fill.lots, fill.lots >= 100 ? 0 : 2)}</span>
            <span className="num text-right">
              {quote(fill.entryPrice)} <span className="text-muted-foreground">→</span> {quote(fill.exitPrice)}
            </span>
            <span className="text-muted-foreground truncate">{fill.exitReason.toLowerCase().replace(/_/g, ' ')}</span>
            <span className={cn('num text-right', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>
              {signedUsd(fill.pnlUsd)}
            </span>
            <span className={cn('num text-right', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

/* ----------------------------------------------------------------- events */

/** What each event line says, in the reader's words rather than the wire's. */
function eventLine(event: PaperEvent): string {
  switch (event.kind) {
    case 'started':
      return event.guards ? `guards: ${event.guards}` : 'guards off'
    case 'gap':
      return `${event.missing_bars ?? 0} bar${event.missing_bars === 1 ? '' : 's'} missing before this one`
    case 'refused':
      return `entry refused — ${guardLabel(event.reason ?? '')}`
    case 'guard_close':
      return `position closed — ${guardLabel(event.reason ?? '')}`
    case 'stopped':
      return `book closed at ${event.equity == null ? '—' : `$${Math.round(event.equity).toLocaleString('en-US')}`} over ${event.trades ?? 0} fills`
    default:
      return event.reason ?? ''
  }
}

const EVENT_TONE: Record<string, string> = {
  started: 'text-muted-foreground',
  gap: 'text-caution',
  refused: 'text-caution',
  guard_close: 'text-lp',
  stopped: 'text-lp',
}

function EventsList({ events }: { events: PaperEvent[] }) {
  if (events.length === 0) {
    return <p className="text-muted-foreground py-2 text-[11px]">Nothing but fills — no gap, no refusal.</p>
  }
  return (
    <ul className="space-y-0.5">
      {[...events].reverse().map((event, index) => (
        <li key={`${event.kind}-${event.time}-${index}`} className="grid grid-cols-[96px_78px_1fr] items-start gap-2 text-[11px]">
          <span className="num text-muted-foreground">{shortStamp(event.time)}</span>
          <span className={cn('num truncate', EVENT_TONE[event.kind] ?? 'text-muted-foreground')}>{event.kind}</span>
          <span className="text-muted-foreground leading-snug break-words">{eventLine(event)}</span>
        </li>
      ))}
    </ul>
  )
}
