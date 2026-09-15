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
 * ├ chart: the run's own candles,           │ fills · events      │
 * │ indicators, fills, stop / target bands  │                     │
 * ├ fills — pick one and it is read out ────┤                     │
 * └─────────────────────────────────────────┴─────────────────────┘
 * ```
 *
 * Picking a fill opens a plain-language account of it under the table: what the
 * strategy saw, where the entry, stop and target sat, and what actually closed
 * it. No fill and no run total is evidence a strategy works, and the account
 * says so under any run with fewer than thirty of them.
 *
 * Every figure carries its unit, because `1.03` is not a profit factor and
 * `−277` is not a loss until it says dollars.
 *
 * One number on this screen is not a decision: the live price. The bot steps
 * on closed bars, so between two of them every figure here is up to fifteen
 * minutes old — which reads as a dead feed. `run.live` is the bar still
 * forming, posted straight to the server by the same poller and kept out of
 * every book; it is drawn so the screen moves, labelled so nobody mistakes it
 * for something the bot acted on, and dropped by the server after ninety
 * seconds so a stopped poller shows as no price rather than a frozen one.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { PriceChart, type ActiveIndicator } from '@/components/PriceChart'
import { Skeleton } from '@/components/ui/skeleton'
import { api, type Bar, type BacktestTrade, type LiveBar, type PaperEvent, type PaperRun, type PaperRunDetail } from '@/lib/api'
import { clock, num } from '@/lib/format'
import { cn } from '@/lib/utils'

/** Height of the sticky app bar, which this page fills the rest of the viewport under. */
const APP_BAR = 45

/**
 * Bars the drill-down asks for.
 *
 * `PriceChart` opens on its last 140 candles, so anything under that leaves the
 * chart with nothing to scroll back into. 240 is a little under two days of 15m
 * bars — enough context around the newest fills without making the poll heavy.
 */
const DETAIL_BARS = 240

/** The chart's pane, in px. Fixed, because a chart that changes height on data reflows the column. */
const CHART_H = 380

/**
 * How many of a run's OWN bars may pass before it is called stale.
 *
 * Bars rather than minutes, and that is the whole point. A flat 45-minute
 * threshold is three missed bars on a 15-minute book and **nine** on a
 * five-minute one, so the two five-minute books could lose most of an hour of
 * feed and still show a reassuring pulsing green dot. The reader wants the
 * same question answered for every row — "is this one still being fed" — and
 * the only unit in which that question means the same thing on both books is
 * the book's own bar.
 */
const STALE_BARS = 3

/** The timeframes a run can carry, in milliseconds. */
const TF_MS: Record<string, number> = { '1m': 60_000, '5m': 300_000, '15m': 900_000, '1h': 3_600_000 }

/** Fallback for a timeframe this table does not know: the old flat window. */
const STALE_MS_FALLBACK = 45 * 60_000

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

/**
 * How old the live price is, in seconds.
 *
 * Seconds, not the minutes `ago` deals in: the server drops a tick older than
 * ninety seconds, so anything shown here is inside a minute and a half, and
 * "0 min" would say nothing about whether the feed is moving.
 */
const liveAge = (at: number, now: number): string => `${Math.max(0, Math.round((now - at) / 1000))} s ago`

/**
 * `ask − bid`, at the precision the price itself is quoted in.
 *
 * A gold spread printed by `quote` would read `0.25000`, because `quote`
 * chooses its decimals from the magnitude and a spread is a small number in a
 * large market. It takes its decimals from the close instead.
 */
const spreadOf = (live: LiveBar): string | null => {
  if (live.bid == null || live.ask == null || !Number.isFinite(live.bid) || !Number.isFinite(live.ask)) return null
  const decimals = (quote(live.close).split('.')[1] ?? '').length
  return (live.ask - live.bid).toFixed(decimals)
}

/**
 * Which way the forming bar has gone since the bot last decided anything:
 * `1`, `-1` or `0` against the previous closed bar's close.
 */
const liveDirection = (live: LiveBar, lastClose: number | null): number => {
  if (lastClose == null || !Number.isFinite(lastClose)) return 0
  return Math.sign(live.close - lastClose)
}

/** Up green, down red — the data colours, never the accent, which means selection here. */
const DIRECTION_CLASS = ['text-lp', 'text-foreground', 'text-lc']
/** The arrow carries the direction too, so colour is not the only channel. */
const DIRECTION_MARK = ['▾', '·', '▴']

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

const staleAfter = (tf: string): number => (TF_MS[tf] ?? 0) * STALE_BARS || STALE_MS_FALLBACK

const isStale = (run: PaperRun, now: number) =>
  run.last_bar_time ? now - run.last_bar_time > staleAfter(run.tf) : true

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
  // Which fill the explanation below the table is describing. Held here rather
  // than in the table because the table is rendered twice — once in the wide
  // layout's left column, once in the rail under it — and a reader who picks a
  // fill, then resizes, should still be reading the same fill. Tagged with the
  // run it belongs to, so a key from the run you just left is simply not the
  // run you are looking at rather than something an effect has to clear.
  const [fillPick, setFillPick] = useState<{ id: string; key: string } | null>(null)

  // The book changes when a bar arrives — every five or fifteen minutes — but
  // the live price rides on this same small JSON, so it is read every 10 s:
  // half the old interval, and well inside the ninety seconds after which the
  // server stops reporting a tick at all.
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
    const timer = window.setInterval(read, 10_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  // The price arrives every 10 s; its *age* is a second hand, and an age that
  // sat still for ten seconds at a time would itself look like a stuck feed.
  // Only `now` changes here — no request, and nothing the chart depends on.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1_000)
    return () => window.clearInterval(timer)
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
        .paperRun(activeId, DETAIL_BARS)
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

  const openFill = useCallback(
    (key: string | null) => setFillPick(key && activeId ? { id: activeId, key } : null),
    [activeId],
  )

  // The detail only ever belongs to one run; while a newly picked run is still
  // loading, the panes below show their skeletons rather than the old run's
  // chart under the new run's name.
  const live = detail && detail.run.id === activeId ? detail : null
  const pickedFill = fillPick && fillPick.id === activeId ? fillPick.key : null
  // The same field from whichever poll read it last: `/status` every 10 s,
  // `/run/{id}` every 60. Taking it from the detail alone would leave the
  // chart's forming candle a minute behind the price in the table above it.
  const activeRun = sorted.find((r) => r.id === activeId) ?? null
  const livePrice = activeRun?.live ?? live?.live ?? null
  // The fill the chart is asked to show. Derived from the same key the table
  // marks open, so the row and the bands can never point at different trades.
  const focusFill = useMemo(
    () => (pickedFill && live ? (live.fills.find((f, i) => fillId(f, i) === pickedFill) ?? null) : null),
    [pickedFill, live],
  )

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
        /* The chart owns the screen and the list of books moved to the rail.
           A ten-row table across the top pushed the candles into a 380px
           letterbox for the sake of columns a reader consults once a session;
           the chart is the thing being read continuously, so it takes the
           height and the run list becomes a picker beside it. */
        <div className="flex min-h-0 flex-1 flex-col overflow-auto xl:flex-row xl:overflow-hidden">
          <div className="flex min-w-0 flex-col xl:h-full xl:min-w-0 xl:flex-1">
            {/* What the bot traded on: its own indicators over the candles it
                stepped, with every fill's entry, exit and stop / target band.
                Fills the column's height above xl and keeps a floor below it,
                where the page scrolls instead. */}
            <div className="border-border min-h-[340px] shrink-0 border-b xl:min-h-0 xl:flex-1 xl:shrink">
              <RunChart detail={live} live={livePrice} focus={focusFill} />
            </div>
            {/* The fills have seven columns and the rail has 460px, so they
                stay here where the width is. Capped at two fifths of the
                column: a long book must not push the chart off the screen the
                move above was made to give it. Below xl they are in the rail. */}
            <div className="hidden xl:block xl:max-h-[40%] xl:shrink-0 xl:overflow-y-auto">
              <FillsSection detail={live} openFill={pickedFill} onOpenFill={openFill} />
            </div>
          </div>
          <div className="flex min-w-0 flex-col border-t xl:h-full xl:w-[460px] xl:shrink-0 xl:border-t-0 xl:border-l">
            {/* The books, compact. The picker the wide table used to be, in the
                width a rail has: everything the wide table's eleven columns
                carried that is not per-run configuration, and the rest moved
                into the drill-down under it where it belongs to one run. */}
            <div className="border-border shrink-0 border-b xl:max-h-[46%] xl:overflow-y-auto">
              <RunsList runs={sorted} now={now} selected={activeId} onPick={pick} />
            </div>
            <div className="min-h-0 xl:flex-1 xl:overflow-y-auto">
              <Drilldown
                detail={live}
                summary={activeRun}
                error={detailError && detailError.id === activeId ? detailError.message : null}
                now={now}
                openFill={pickedFill}
                onOpenFill={openFill}
              />
            </div>
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
    // The shortest calendar on the desk, because the guard fails per run and a
    // desk is only as covered as its least covered book.
    let horizon: { days: number; name: string } | null = null
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
      const days = run.news.horizon_days
      if (typeof days === 'number' && (horizon === null || days < horizon.days)) {
        horizon = { days, name: run.news.horizon_name ?? 'the calendar' }
      }
    }
    return { stale, net, today, capped, blackout, horizon }
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
      {/* A guard that stops guarding on a date nobody is watching is the
          failure this desk keeps logging. Shown from ninety days out, in the
          strip rather than in a file, and it goes red inside a month. */}
      {stats.horizon !== null && stats.horizon.days < 90 && (
        <span
          className={cn('num', stats.horizon.days < 30 ? 'text-destructive' : 'text-caution')}
          title="The first release series to run out. After its last entry the news blackout stops for that release, silently."
        >
          {stats.horizon.name} ends in {stats.horizon.days}d
        </span>
      )}
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

/**
 * The books, as a picker in the rail.
 *
 * This replaced an eleven-column table that ran the full width of the screen.
 * The columns were not wasted — they were just in the wrong place: a reader
 * watches the candles continuously and consults a run's guards once a session,
 * so the chart now has the width and the list has what a picker needs. The
 * two columns that belonged to one run rather than to the comparison, the open
 * position and the guards that fired, moved into the drill-down below.
 *
 * Two lines a row: what it is and what it has made, then what it is trading
 * and what that is worth right now. Ten of those fit a rail without scrolling.
 */
function RunsList({
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

  // The same walk the fills table has: ↑/↓ move the cursor, Enter opens.
  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, runs.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  return (
    <div role="group" aria-label="Paper runs">
      <div className="text-muted-foreground bg-background sticky top-0 z-10 flex items-baseline gap-2 border-b px-3 py-1 text-[10px] tracking-wide uppercase">
        <span>books</span>
        <span className="num text-muted-foreground/70 normal-case">{runs.length}</span>
        <span className="text-muted-foreground/60 ml-auto normal-case">click to load the chart</span>
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
              'hover:bg-accent/60 focus-visible:ring-ring w-full border-b px-3 py-[6px] text-left transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none',
              isOn && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
            )}
          >
            <span className="flex items-center gap-2 text-xs">
              <StatusPill stale={stale} />
              <span className="num min-w-0 flex-1 truncate">{run.id}</span>
              <span
                className={cn(
                  'num shrink-0',
                  run.net_usd > 0 && 'text-lc',
                  run.net_usd < 0 && 'text-lp',
                  run.net_usd === 0 && 'text-muted-foreground',
                )}
              >
                {signedUsd(run.net_usd)}
              </span>
            </span>
            <span className="mt-0.5 flex items-end gap-2 text-[10px] leading-tight">
              <span className="text-muted-foreground min-w-0 flex-1 truncate">
                <span className="num">{run.strategy}</span>
                <span className="text-muted-foreground/60"> · {run.market}:{run.tf}</span>
                <span className="text-muted-foreground/60">
                  {' '}
                  · {run.trades} fill{run.trades === 1 ? '' : 's'}
                  {run.profit_factor != null && ` · PF ${num(run.profit_factor)}`}
                  {run.open && <span className={run.open.side === 'LONG' ? 'text-lc' : 'text-lp'}> · {run.open.side.toLowerCase()}</span>}
                </span>
              </span>
              <span className="shrink-0">
                <LiveCell run={run} now={now} />
              </span>
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

/**
 * The price right now, and how old it is.
 *
 * The forming bar's close against the last closed bar's close: up green, down
 * red, with an arrow so the direction survives without colour. Underneath, the
 * age in seconds — because a price with no age is the thing that made this
 * screen look broken in the first place.
 *
 * With no live bar — no poller, or a market that is shut — the column is what
 * it was before: how long ago the last bar closed.
 */
function LiveCell({ run, now }: { run: PaperRun; now: number }) {
  const live = run.live
  if (!live) {
    return (
      <span className="num text-muted-foreground text-right">
        {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'no bar yet'}
      </span>
    )
  }
  const direction = liveDirection(live, run.last_bar_close)
  const spread = spreadOf(live)
  return (
    <span className="flex flex-col items-end leading-tight">
      <span
        className={cn(
          'num whitespace-nowrap transition-colors duration-300 motion-reduce:transition-none',
          DIRECTION_CLASS[direction + 1],
        )}
      >
        <span aria-hidden>{DIRECTION_MARK[direction + 1]}</span> {quote(live.close)}
      </span>
      <span className="num text-muted-foreground/80 text-[10px] whitespace-nowrap" title={spread ? `spread ${spread}` : undefined}>
        {liveAge(live.at, now)}
      </span>
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
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail | null
  summary: PaperRun | null
  error: string | null
  now: number
  openFill: string | null
  onOpenFill: (key: string | null) => void
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
        <LivePrice live={summary?.live ?? detail.live} lastClose={(summary ?? run).last_bar_close} now={now} />
        {run.label && <p className="text-muted-foreground mt-1 text-[11px] leading-snug">{run.label}</p>}
        {/* The two columns that left the runs table when it became a rail
            picker: both describe this one run rather than compare it to the
            others, so this is where they belonged all along. */}
        <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px]">
          <span className="text-muted-foreground text-[10px] tracking-wide uppercase">position</span>
          <OpenCell run={run} />
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px]">
          <span className="text-muted-foreground text-[10px] tracking-wide uppercase">guards</span>
          <GuardChips run={run} />
        </div>
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
        <FillsTable detail={detail} openFill={openFill} onOpenFill={onOpenFill} />
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
/**
 * The live price, the size the drill-down has room for.
 *
 * The same number as the row above, read larger, with the quote it came from
 * and its age. The sentence under it is the whole point: this is the only
 * figure on the screen the bot has not acted on.
 */
function LivePrice({ live, lastClose, now }: { live: LiveBar | null; lastClose: number | null; now: number }) {
  if (!live) {
    return (
      <p className="text-muted-foreground mt-2 text-[11px]">
        No live price — nothing has posted a forming bar in the last 90 seconds.
      </p>
    )
  }
  const direction = liveDirection(live, lastClose)
  const spread = spreadOf(live)
  return (
    <div className="mt-2 flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <span
        className={cn(
          'num text-[22px] leading-none transition-colors duration-300 motion-reduce:transition-none',
          DIRECTION_CLASS[direction + 1],
        )}
      >
        <span aria-hidden>{DIRECTION_MARK[direction + 1]}</span> {quote(live.close)}
      </span>
      <span className="text-muted-foreground num text-[10px]">
        {live.bid != null && live.ask != null
          ? `bid ${quote(live.bid)} / ask ${quote(live.ask)}${spread ? ` · spread ${spread}` : ''}`
          : 'no quote'}
        {' · '}
        {liveAge(live.at, now)}
      </span>
      <span className="text-muted-foreground/70 text-[10px]">forming bar — not traded on</span>
    </div>
  )
}

function FillsSection({
  detail,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail | null
  openFill: string | null
  onOpenFill: (key: string | null) => void
}) {
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
      <FillsTable detail={detail} openFill={openFill} onOpenFill={onOpenFill} />
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

/* ------------------------------------------------------------------ chart */

/**
 * Indicator line colours, named rather than written out.
 *
 * Lightweight Charts paints on a canvas, where `var(--sp)` is not a colour, so
 * a token has to be resolved to whatever it currently computes to — but the
 * component still never carries a colour of its own. Lime is deliberately
 * absent: it is the one chrome accent (ui/DESIGN.md) and an indicator line is
 * data, not a control.
 */
const LINE_TOKENS = ['--chart-2', '--chart-3', '--chart-4', '--chart-5', '--caution']

/** What a palette token computes to right now. `gray` is a last resort no theme reaches. */
function tokenValue(name: string): string {
  if (typeof window === 'undefined') return 'gray'
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || 'gray'
}

/**
 * The candles the run stepped, its own indicators, and every fill on top.
 *
 * Nothing here is chosen for the run: `detail.indicators` is what the
 * strategy's definition declares it reads, and `overlay` on each one says
 * whether its series belongs over the candles or wants a pane of its own.
 */
function RunChart({
  detail,
  live,
  focus,
}: {
  detail: PaperRunDetail | null
  live: LiveBar | null
  /** The fill picked in the table below, or null. */
  focus: BacktestTrade | null
}) {
  // `bars` arrives as `[ms, o, h, l, c]` and `PriceChart` takes milliseconds
  // and divides, so the tuple goes straight across. (`series` times are
  // already seconds, which is what the chart wants there — the two halves of
  // the payload are in different units and neither is converted here.)
  const bars = useMemo<Bar[]>(
    () => (detail?.bars ?? []).map(([time, open, high, low, close]) => ({ time, open, high, low, close })),
    [detail],
  )

  // The forming candle, in the shape the chart takes. `PriceChart` appends it
  // past the last closed bar and ignores a frame older than one, so a stream
  // that has fallen behind draws nothing rather than a candle in the past.
  const forming = useMemo<Bar | null>(
    () =>
      live
        ? { time: live.time, open: live.open, high: live.high, low: live.low, close: live.close }
        : null,
    [live],
  )

  const indicators = useMemo<ActiveIndicator[]>(() => {
    const active: ActiveIndicator[] = []
    for (const entry of detail?.indicators ?? []) {
      // The wire sends fully-qualified outputs (`ema_21.ema`) and `PriceChart`
      // glues `key + '.' + output` back together, so the prefix comes off
      // here. Sliced by length rather than split on '.', because a float
      // parameter puts a dot inside the key itself: `keltner_20_10_1.5`.
      const prefix = `${entry.key}.`
      active.push({
        key: entry.key,
        id: entry.id,
        params: entry.params,
        outputs: entry.outputs.map((o) => (o.startsWith(prefix) ? o.slice(prefix.length) : o)),
        // Pane 0 is the candles. Every indicator whose own definition says it
        // does not belong over them gets a pane of its own, numbered from 1.
        pane: entry.overlay ? 0 : active.filter((e) => e.pane > 0).length + 1,
        color: tokenValue(LINE_TOKENS[active.length % LINE_TOKENS.length]),
      })
    }
    return active
  }, [detail])

  if (!detail) {
    return (
      <div className="space-y-2 px-3 py-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="w-full" style={{ height: CHART_H - 26 }} />
      </div>
    )
  }

  if (bars.length === 0) {
    return (
      <section className="px-3 py-2">
        <Heading>Chart</Heading>
        <p className="text-muted-foreground py-8 text-center text-[11px]">
          No bars to draw — the poller has fed this run nothing yet.
        </p>
      </section>
    )
  }

  return (
    <section className="flex h-full min-h-0 flex-col px-3 py-2">
      <Heading>
        Chart <span className="text-muted-foreground/70 num">{detail.run.market}:{detail.run.tf}</span>{' '}
        <span className="text-muted-foreground/70 num">{bars.length} bars</span>
        {focus && (
          <span className="text-primary num ml-2 normal-case">
            showing the {focus.direction.toLowerCase()} closed {shortStamp(focus.exitTime)} — click the row again to
            release
          </span>
        )}
      </Heading>
      <div className="border-border min-h-0 w-full flex-1 overflow-hidden rounded-sm border" style={{ minHeight: CHART_H - 60 }}>
        {/* Keyed by run: a new run brings a different set of panes, and
            remounting is cheaper to reason about than reconciling them. */}
        <PriceChart
          key={detail.run.id}
          bars={bars}
          indicators={indicators}
          series={detail.series ?? {}}
          frame={null}
          showLevels={false}
          trades={detail.fills}
          showMarkers
          showZones
          focus={focus}
          liveBar={forming}
        />
      </div>
      <p className="text-muted-foreground mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px]">
        {indicators.length === 0 ? (
          <span>This strategy reads no indicator — it trades the clock or the bar itself.</span>
        ) : (
          indicators.map((entry) => (
            <span key={entry.key} className="num inline-flex items-center gap-1">
              <span
                className="inline-block h-px w-3 align-middle"
                style={{ backgroundColor: entry.color }}
                aria-hidden
              />
              {entry.key}
              {entry.pane > 0 && <span className="text-muted-foreground/60">pane {entry.pane}</span>}
            </span>
          ))
        )}
        <span className="text-muted-foreground/70">
          entries and exits are marked; the bands behind them are each fill&rsquo;s stop and target.
        </span>
        <span className="text-muted-foreground/70">
          The bot decides on closed bars only; the last candle is still forming and is never traded on.
        </span>
      </p>
    </section>
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

/** Identifies a fill across polls: a run cannot open two trades on the same bar. */
const fillId = (fill: BacktestTrade, index: number) => `${fill.entryTime}-${fill.exitTime}-${index}`

/**
 * The run's closed trades, one per row, newest first.
 *
 * A row is a button: clicking it (or pressing Enter on it) opens the account of
 * that fill beneath the table, and clicking it again closes it. ↑/↓ walk the
 * rows the way they do in the runs table above.
 */
function FillsTable({
  detail,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail
  openFill: string | null
  onOpenFill: (key: string | null) => void
}) {
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([])
  // Newest first, but keyed by the position the fill holds in the book, so the
  // key survives the reversal and the next poll.
  const rows = useMemo(
    () => detail.fills.map((fill, index) => ({ fill, key: fillId(fill, index) })).reverse(),
    [detail.fills],
  )
  const open = rows.find((row) => row.key === openFill) ?? null

  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, rows.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  if (rows.length === 0) {
    return (
      <p className="text-muted-foreground py-2 text-[11px]">
        None yet — the first live bar decides; the warm-up bars do not.
      </p>
    )
  }

  return (
    <>
      <div className="overflow-x-auto">
        <div className="min-w-[600px]" role="group" aria-label="Closed fills">
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
          {rows.map(({ fill, key }, index) => {
            const isOpen = key === openFill
            return (
              <button
                key={key}
                type="button"
                ref={(el) => {
                  rowRefs.current[index] = el
                }}
                onClick={() => onOpenFill(isOpen ? null : key)}
                onKeyDown={(e) => onKeyDown(e, index)}
                aria-expanded={isOpen}
                aria-controls={`fill-account-${detail.run.id}`}
                className={cn(
                  'hover:bg-accent/60 focus-visible:ring-ring grid w-full items-center gap-2 border-b py-[3px] text-left text-[11px] transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none motion-reduce:transition-none',
                  FILL_COLS,
                  isOpen && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
                )}
              >
                <span className="num text-muted-foreground">{shortStamp(fill.exitTime)}</span>
                <span className={cn('num', fill.direction === 'LONG' ? 'text-lc' : 'text-lp')}>
                  {fill.direction.toLowerCase()}
                </span>
                <span className="num text-right">{num(fill.lots, fill.lots >= 100 ? 0 : 2)}</span>
                <span className="num text-right">
                  {quote(fill.entryPrice)} <span className="text-muted-foreground">→</span> {quote(fill.exitPrice)}
                </span>
                <span className="text-muted-foreground truncate">
                  {fill.exitReason.toLowerCase().replace(/_/g, ' ')}
                </span>
                <span className={cn('num text-right', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>
                  {signedUsd(fill.pnlUsd)}
                </span>
                <span className={cn('num text-right', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>
              </button>
            )
          })}
        </div>
      </div>
      <div id={`fill-account-${detail.run.id}`}>
        {open ? (
          <FillAccount fill={open.fill} run={detail.run} />
        ) : (
          <p className="text-muted-foreground/70 mt-1.5 text-[10px]">
            Pick a fill to read what the strategy saw, where its stop and target sat, and how it ended.
          </p>
        )}
      </div>
    </>
  )
}

/* ------------------------------------------------------- one fill, in words */

/** `2 h 15 min` — the scale a person holding a trade thinks in. */
const heldFor = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms) || ms <= 0) return 'less than a minute'
  const minutes = Math.round(ms / 60_000)
  if (minutes < 1) return 'less than a minute'
  if (minutes < 60) return `${minutes} min`
  const hours = Math.floor(minutes / 60)
  const restMin = minutes % 60
  if (hours < 24) return restMin > 0 ? `${hours} h ${restMin} min` : `${hours} h`
  const days = Math.floor(hours / 24)
  const restH = hours % 24
  return restH > 0 ? `${days} d ${restH} h` : `${days} d`
}

/**
 * What ended the trade, in words.
 *
 * The guards spell themselves in SCREAMING_CASE and the engine's own exits are
 * `STOP` and `TARGET`. Anything else is the strategy describing its own rule —
 * `window closed`, say — and is quoted rather than paraphrased, because a
 * paraphrase of a sentence we did not write is a sentence we invented.
 */
const exitSentence = (reason: string): string => {
  switch (reason.trim().toUpperCase()) {
    case 'STOP':
      return 'Price reached the stop'
    case 'TARGET':
      return 'Price reached the target'
    case 'WEEKEND_FLAT':
      return 'The weekend guard closed it before Friday’s close'
    case 'NEWS_FLAT':
      return 'The news guard closed it before a release'
    case 'OPEN_LOSS_CAP':
      return 'The open-loss cap closed it at 2R'
    case 'DAILY_LOSS_LIMIT':
      return 'The daily loss limit closed the book, and this trade with it'
    default:
      return `The strategy’s own rule closed it — “${reason}”`
  }
}

/**
 * One fill, read out.
 *
 * Everything here is on the fill itself; nothing is modelled. `mae` and `mfe`
 * arrive already divided by the position's risk (see `BacktestTrade`), so they
 * are R and need no conversion — which is why the stop distance below is only
 * ever used for the *target's* R, never for the excursion's.
 */
function FillAccount({ fill, run }: { fill: BacktestTrade; run: PaperRun }) {
  const long = fill.direction === 'LONG'
  const side = long ? 'long' : 'short'
  const lots = num(fill.lots, fill.lots >= 100 ? 0 : 2)

  const stopAway = Math.abs(fill.entryPrice - fill.stop)
  // Prices come off the wire rounded to two decimals, so on a five-decimal
  // market a stop a few pips away can arrive equal to the entry. "0.00 away"
  // would be a lie; saying nothing about the distance in price is not.
  const stopKnown = Number.isFinite(stopAway) && stopAway > 0
  const targetAway = fill.target == null ? null : Math.abs(fill.target - fill.entryPrice)
  const targetR = targetAway != null && stopKnown ? targetAway / stopAway : null

  return (
    <div className="border-border bg-card/50 mt-2 space-y-1.5 rounded-sm border px-3 py-2 text-[12px] leading-relaxed">
      <p className="text-muted-foreground">
        {fill.reason ? (
          <>
            <Label>What it saw</Label> the strategy wrote “
            <span className="text-foreground">{fill.reason}</span>” as its reason for taking this one.
          </>
        ) : (
          <>
            <Label>What it saw</Label> the run recorded no entry sentence for this fill.
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>Entry</Label> it went{' '}
        <span className={cn('num', long ? 'text-lc' : 'text-lp')}>{side}</span>{' '}
        <span className="num text-foreground">{lots}</span> lots of{' '}
        <span className="num">{run.market}</span> at{' '}
        <span className="num text-foreground">{quote(fill.entryPrice)}</span>, on{' '}
        <span className="num">{shortStamp(fill.entryTime)}</span>.
      </p>

      <p className="text-muted-foreground">
        <Label>SL</Label> the stop sat at <span className="num text-foreground">{quote(fill.stop)}</span>
        {stopKnown ? (
          <>
            , <span className="num">{quote(stopAway)}</span> in price {long ? 'below' : 'above'} the entry
          </>
        ) : (
          <> (closer to the entry than the two decimals this feed reports prices in)</>
        )}
        . That distance <em className="not-italic">is</em> one R by construction — every R figure on this fill is a
        multiple of it.
        {fill.target == null && (
          <>
            {' '}
            Here it is only the sizing unit: this strategy exits on its own rule, so the stop says how big the trade
            was, not how it was meant to end.
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>TP</Label>{' '}
        {fill.target == null ? (
          <>no target — the strategy&rsquo;s own rule closes it, and nothing was set to take profit at.</>
        ) : (
          <>
            the target sat at <span className="num text-foreground">{quote(fill.target)}</span>
            {targetAway != null && (
              <>
                , <span className="num">{quote(targetAway)}</span> in price {long ? 'above' : 'below'} the entry
              </>
            )}
            {targetR != null && (
              <>
                {' '}
                — <span className="num text-foreground">{targetR.toFixed(2)}R</span> of reward against the 1R it was
                risking
              </>
            )}
            .
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>What happened</Label> {exitSentence(fill.exitReason)}. It closed at{' '}
        <span className="num text-foreground">{quote(fill.exitPrice)}</span> on{' '}
        <span className="num">{shortStamp(fill.exitTime)}</span>, held{' '}
        <span className="num">{heldFor(fill.holdMs)}</span>, for{' '}
        <span className={cn('num', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>{signedUsd(fill.pnlUsd)}</span> —{' '}
        <span className={cn('num', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>.
      </p>

      <p className="text-muted-foreground">
        <Label>On the way</Label> it went as far as{' '}
        <span className="num text-lc">{signedR(fill.mfe)}</span> in favour and{' '}
        <span className="num text-lp">{signedR(fill.mae)}</span> against before it closed.
      </p>

      {run.trades < 30 && (
        <p className="text-caution border-border border-t pt-1.5 text-[11px]">
          This is one trade out of {run.trades} this run has closed. Fewer than 30 fills cannot be read as a result —
          neither this fill nor the run&rsquo;s total says whether the strategy works.
        </p>
      )}
    </div>
  )
}

/** The word that opens each sentence. Weight, not colour — lime is for controls. */
function Label({ children }: { children: React.ReactNode }) {
  return <span className="text-foreground font-medium">{children} — </span>
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
