import { useEffect, useMemo, useState } from 'react'

import { Skeleton } from '@/components/ui/skeleton'
import { api, type BacktestTrade, type PaperRun } from '@/lib/api'
import { cn } from '@/lib/utils'

/**
 * Analytics — what each strategy has actually done, across every paper book.
 *
 * The Desk answers "is this run alive and what did it just do". This answers
 * the other question: over everything that has closed, which strategy is
 * carrying the account and which is bleeding it. So it is terminal mode
 * (`ui/DESIGN.md`) — flat, dense, tables that fill the width — and not the
 * card grid an account-summary screen usually wears. A page whose job is to
 * let someone compare eleven numbers across ten strategies cannot spend its
 * width on padding.
 *
 * Every figure here is computed from closed trades in the browser. Nothing is
 * asked of the server that the Desk does not already ask: `/api/paper/status`
 * for the runs, then one `/api/paper/run/{id}` per run for its fills. That
 * costs ten small requests a minute and keeps the analytics honest — there is
 * no second code path that could disagree with the book.
 *
 * **It will look empty for a while, and that is the truth rather than a bug.**
 * The ten books started on 2026-09-14 and close a handful of trades a day. A
 * table of ten strategies over thirteen trades is what thirteen trades look
 * like, and every panel here says how thin its own sample is rather than
 * drawing a confident shape over nothing.
 */

/** The books change on a bar, not on a tick: a minute is often enough. */
const REFRESH_MS = 60_000

/** Below this, a panel says the number is too thin to read instead of drawing it. */
const THIN_SAMPLE = 20

const MINUS = '−'

const usd = (v: number): string => {
  const sign = v < 0 ? MINUS : v > 0 ? '+' : ''
  return `${sign}$${Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 })}`
}

const pct = (v: number): string => `${(v * 100).toFixed(1)}%`

const signedR = (v: number): string => `${v < 0 ? MINUS : v > 0 ? '+' : ''}${Math.abs(v).toFixed(2)}R`

const hold = (ms: number): string => {
  if (!Number.isFinite(ms) || ms <= 0) return '—'
  const m = Math.round(ms / 60_000)
  if (m < 60) return `${m}m`
  const h = m / 60
  return h < 48 ? `${h.toFixed(1)}h` : `${(h / 24).toFixed(1)}d`
}

/** Colour a number by its sign, using the tape's own up/down tokens. */
const bySign = (v: number): string => (v > 0 ? 'text-lc' : v < 0 ? 'text-lp' : 'text-muted-foreground')

/**
 * The hour of a stamp on a New York wall clock.
 *
 * Sessions in this repository are New York hours — `Workbench` spells them
 * that way and so do the research batches — so the bucket has to be read on
 * that clock and not on the browser's. `Intl` carries the daylight-saving
 * rule; a fixed offset would put a third of the year in the wrong session,
 * which is the fault the research loop logged twice (`2026-09-13-instrument-faults.md`).
 */
const NY_HOUR = new Intl.DateTimeFormat('en-US', {
  timeZone: 'America/New_York',
  hour: 'numeric',
  hour12: false,
})

const nyHour = (ms: number): number => {
  const parsed = Number(NY_HOUR.format(new Date(ms)))
  // `Intl` renders midnight as 24 in some engines.
  return parsed === 24 ? 0 : parsed
}

/**
 * The three sessions exactly as `Workbench` and the research batches define
 * them, in New York hours. **They overlap** — London and New York share
 * 08:00–11:00 — so a trade can belong to two, and the session table's rows
 * deliberately do not sum to the total. Inventing non-overlapping boundaries
 * here would make this page disagree with the filter the strategies are
 * actually run under, which is worse than a footnote.
 */
const SESSIONS: { label: string; from: number; to: number; title: string }[] = [
  { label: 'Asia', from: 18, to: 2, title: '18:00–02:00 New York' },
  { label: 'London', from: 3, to: 11, title: '03:00–11:00 New York' },
  { label: 'NY', from: 8, to: 16, title: '08:00–16:00 New York' },
]

const inSession = (h: number, from: number, to: number): boolean =>
  from < to ? h >= from && h < to : h >= from || h < to

/** A closed trade, tagged with the book it came from. */
interface Tagged extends BacktestTrade {
  runId: string
  strategy: string
  market: string
  tf: string
}

interface Stats {
  trades: number
  wins: number
  winRate: number
  net: number
  grossWin: number
  grossLoss: number
  profitFactor: number | null
  meanR: number
  medianR: number
  bestR: number
  worstR: number
  maxDrawdown: number
  meanHoldMs: number
  meanMae: number
  meanMfe: number
  longestWin: number
  longestLoss: number
  /** Cumulative P&L after each trade, oldest first — the curve, from zero. */
  curve: { t: number; v: number }[]
}

const EMPTY: Stats = {
  trades: 0,
  wins: 0,
  winRate: 0,
  net: 0,
  grossWin: 0,
  grossLoss: 0,
  profitFactor: null,
  meanR: 0,
  medianR: 0,
  bestR: 0,
  worstR: 0,
  maxDrawdown: 0,
  meanHoldMs: 0,
  meanMae: 0,
  meanMfe: 0,
  longestWin: 0,
  longestLoss: 0,
  curve: [],
}

/**
 * Every statistic this page shows, from one list of closed trades.
 *
 * Trades are sorted by **exit** time, because that is when the money moved and
 * therefore the order the equity curve and the streaks happen in. A book that
 * holds two positions at once can exit them out of entry order, and a curve
 * drawn on entry time would show a drawdown the account never had.
 *
 * `maxDrawdown` is on that cumulative curve from its own running peak, in
 * dollars. It is a property of this group of trades alone, not of the run's
 * equity, so summing it across strategies means nothing and no row does.
 */
function summarise(trades: Tagged[]): Stats {
  if (trades.length === 0) return EMPTY
  const sorted = [...trades].sort((a, b) => a.exitTime - b.exitTime)

  let net = 0
  let grossWin = 0
  let grossLoss = 0
  let wins = 0
  let peak = 0
  let maxDrawdown = 0
  let run = 0
  let longestWin = 0
  let longestLoss = 0
  let holdSum = 0
  let maeSum = 0
  let mfeSum = 0
  const rs: number[] = []
  const curve: { t: number; v: number }[] = []

  for (const t of sorted) {
    net += t.pnlUsd
    if (t.pnlUsd > 0) {
      grossWin += t.pnlUsd
      wins += 1
      run = run > 0 ? run + 1 : 1
      longestWin = Math.max(longestWin, run)
    } else if (t.pnlUsd < 0) {
      grossLoss += -t.pnlUsd
      run = run < 0 ? run - 1 : -1
      longestLoss = Math.max(longestLoss, -run)
    }
    // A trade that closed at exactly zero breaks neither streak and starts
    // neither: it is not a win and not a loss, and pretending otherwise would
    // let a scratch inflate a run of seven into a run of fifteen.
    peak = Math.max(peak, net)
    maxDrawdown = Math.max(maxDrawdown, peak - net)
    holdSum += t.holdMs
    maeSum += t.mae
    mfeSum += t.mfe
    rs.push(t.r)
    curve.push({ t: t.exitTime, v: net })
  }

  const ordered = [...rs].sort((a, b) => a - b)
  const mid = ordered.length >> 1
  const medianR =
    ordered.length === 0
      ? 0
      : ordered.length % 2
        ? ordered[mid]
        : (ordered[mid - 1] + ordered[mid]) / 2

  return {
    trades: sorted.length,
    wins,
    winRate: wins / sorted.length,
    net,
    grossWin,
    grossLoss,
    // A book that has lost nothing has no profit factor: gross win over zero
    // is not "infinitely good", it is a sample too small to have a ratio.
    profitFactor: grossLoss > 0 ? grossWin / grossLoss : null,
    meanR: rs.reduce((a, b) => a + b, 0) / rs.length,
    medianR,
    bestR: Math.max(...rs),
    worstR: Math.min(...rs),
    maxDrawdown,
    meanHoldMs: holdSum / sorted.length,
    meanMae: maeSum / sorted.length,
    meanMfe: mfeSum / sorted.length,
    longestWin,
    longestLoss,
    curve,
  }
}

export function Analytics() {
  const [runs, setRuns] = useState<PaperRun[] | null>(null)
  const [trades, setTrades] = useState<Tagged[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let alive = true

    const read = async () => {
      try {
        const status = await api.paperStatus()
        if (!alive) return
        setRuns(status.runs)

        // One detail request per run, in parallel. A run whose detail fails is
        // reported and skipped rather than blanking the page: nine books'
        // statistics are worth more than a clean error over all ten.
        const settled = await Promise.allSettled(
          status.runs.map(async (run) => {
            const detail = await api.paperRun(run.id, 1)
            return detail.fills.map<Tagged>((fill) => ({
              ...fill,
              runId: run.id,
              strategy: run.strategy,
              market: run.market,
              tf: run.tf,
            }))
          }),
        )
        if (!alive) return
        const collected: Tagged[] = []
        const failed: string[] = []
        settled.forEach((result, i) => {
          if (result.status === 'fulfilled') collected.push(...result.value)
          else failed.push(status.runs[i].id)
        })
        setTrades(collected)
        setError(failed.length ? `no book for ${failed.join(', ')}` : null)
      } catch (e) {
        if (alive) setError((e as Error).message)
      } finally {
        if (alive) setLoading(false)
      }
    }

    void read()
    const timer = window.setInterval(() => void read(), REFRESH_MS)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  /** One row per strategy, with the books that ran it folded together. */
  const byStrategy = useMemo(() => {
    const groups = new Map<string, Tagged[]>()
    for (const t of trades) {
      const list = groups.get(t.strategy)
      if (list) list.push(t)
      else groups.set(t.strategy, [t])
    }
    // A run that has closed nothing still gets a row: "this strategy has
    // traded nothing" is a fact about the desk, and dropping it would make
    // the page quietly disagree with the Desk's ten.
    for (const run of runs ?? []) if (!groups.has(run.strategy)) groups.set(run.strategy, [])
    return [...groups.entries()]
      .map(([strategy, list]) => ({
        strategy,
        books: (runs ?? []).filter((r) => r.strategy === strategy),
        stats: summarise(list),
        trades: list,
      }))
      .sort((a, b) => b.stats.net - a.stats.net || a.strategy.localeCompare(b.strategy))
  }, [trades, runs])

  const overall = useMemo(() => summarise(trades), [trades])

  if (loading && !runs) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    )
  }

  if ((runs?.length ?? 0) === 0) {
    return (
      <div className="text-muted-foreground px-4 py-8 text-[13px]">
        <p className="text-foreground">No paper run is registered, so there is nothing to analyse.</p>
        <p className="mt-1">Start the books on the Desk first.</p>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-col">
      <TopStrip overall={overall} runs={runs ?? []} error={error} />
      <div className="min-h-0 flex-1 space-y-4 overflow-auto px-3 py-3">
        <StrategyTable rows={byStrategy} />
        <Curves rows={byStrategy} />
        <div className="grid gap-4 xl:grid-cols-2">
          <SessionTable trades={trades} />
          <SymbolTable trades={trades} runs={runs ?? []} />
        </div>
        <Distribution trades={trades} overall={overall} rows={byStrategy} />
      </div>
    </div>
  )
}

/* ----------------------------------------------------------- top strip */

function TopStrip({
  overall,
  runs,
  error,
}: {
  overall: Stats
  runs: PaperRun[]
  error: string | null
}) {
  // The desk's age is the oldest book's start, not the newest: "uptime" is how
  // long this account has been running, and a book added yesterday does not
  // reset it.
  const started = runs.length ? Math.min(...runs.map((r) => r.started_at)) : null
  const days = started ? (Date.now() - started) / 86_400_000 : 0

  // Balance is what the ten books hold now; capital is what they were opened
  // with, derived rather than assumed — each book's own equity less its own
  // net is its starting balance, so a book started at something other than the
  // configured default is still counted correctly.
  const balance = runs.reduce((a, r) => a + r.equity, 0)
  const capital = runs.reduce((a, r) => a + (r.equity - r.net_usd), 0)
  const ret = capital > 0 ? balance / capital - 1 : 0

  return (
    <div className="text-muted-foreground flex h-8 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b px-3 text-[11px]">
      <span className="num text-foreground">
        {runs.length} <span className="text-muted-foreground">books</span>
      </span>
      <span className="num">
        balance <span className="text-foreground">${Math.round(balance).toLocaleString('en-US')}</span>
        <span className="text-muted-foreground"> of ${Math.round(capital).toLocaleString('en-US')}</span>
      </span>
      <span className="num">
        net <span className={bySign(overall.net)}>{usd(overall.net)}</span>
        <span className={cn('ml-1', bySign(ret))}>({ret >= 0 ? '+' : MINUS}{pct(Math.abs(ret))})</span>
      </span>
      <span className="num">
        {overall.trades} <span className="text-muted-foreground">closed</span>
      </span>
      <span className="num">
        win <span className="text-foreground">{overall.trades ? pct(overall.winRate) : '—'}</span>
      </span>
      <span className="num">
        PF{' '}
        <span className="text-foreground">
          {overall.profitFactor === null ? '—' : overall.profitFactor.toFixed(2)}
        </span>
      </span>
      <span className="num">
        peak-to-trough <span className="text-lp">{overall.trades ? usd(-overall.maxDrawdown) : '—'}</span>
      </span>
      <span className="num">
        uptime <span className="text-foreground">{days < 1 ? `${(days * 24).toFixed(0)}h` : `${days.toFixed(1)}d`}</span>
      </span>
      {error && <span className="text-caution ml-auto truncate">{error}</span>}
    </div>
  )
}

/* ------------------------------------------------------ strategy table */

/** Track widths; the wrapper's `min-w` is their sum plus the gaps. */
const STRAT_COLS =
  'grid-cols-[minmax(150px,1.4fr)_minmax(120px,1fr)_58px_86px_56px_58px_64px_64px_78px_66px_64px]'

function StrategyTable({
  rows,
}: {
  rows: { strategy: string; books: PaperRun[]; stats: Stats; trades: Tagged[] }[]
}) {
  return (
    <section>
      <Heading
        note={`${rows.length} strateg${rows.length === 1 ? 'y' : 'ies'} · every closed trade, folded by strategy`}
      >
        By strategy
      </Heading>
      <div className="overflow-x-auto">
        <div className="min-w-[1000px]">
          <div
            className={cn(
              'text-muted-foreground grid gap-2 border-b px-2 py-1 text-[10px] tracking-wide uppercase',
              STRAT_COLS,
            )}
          >
            <span>Strategy</span>
            <span>Books</span>
            <span className="text-right">Trades</span>
            <span className="text-right">Net</span>
            <span className="text-right">Win</span>
            <span className="text-right">PF</span>
            <span className="text-right" title="Mean profit and loss in units of the trade's own risk.">
              Mean R
            </span>
            <span className="text-right">Median R</span>
            <span className="text-right" title="Deepest fall from this strategy's own running peak, in dollars.">
              Drawdown
            </span>
            <span className="text-right">Hold</span>
            <span className="text-right" title="Longest run of consecutive winners / losers.">
              Streak
            </span>
          </div>
          {rows.map(({ strategy, books, stats }) => (
            <div
              key={strategy}
              className={cn(
                'hover:bg-elevated/60 grid items-baseline gap-2 border-b px-2 py-1 text-[12px] last:border-b-0',
                STRAT_COLS,
              )}
            >
              <span className="num text-foreground truncate" title={strategy}>
                {strategy}
              </span>
              <span className="text-muted-foreground num truncate text-[11px]" title={books.map((b) => b.id).join(', ')}>
                {books.map((b) => b.id).join(', ') || '—'}
              </span>
              <span className="num text-right">{stats.trades || '—'}</span>
              <span className={cn('num text-right', bySign(stats.net))}>
                {stats.trades ? usd(stats.net) : '—'}
              </span>
              <span className="num text-right">{stats.trades ? pct(stats.winRate) : '—'}</span>
              <span className="num text-right">
                {stats.profitFactor === null ? '—' : stats.profitFactor.toFixed(2)}
              </span>
              <span className={cn('num text-right', bySign(stats.meanR))}>
                {stats.trades ? signedR(stats.meanR) : '—'}
              </span>
              <span className={cn('num text-right', bySign(stats.medianR))}>
                {stats.trades ? signedR(stats.medianR) : '—'}
              </span>
              <span className="num text-lp text-right">{stats.trades ? usd(-stats.maxDrawdown) : '—'}</span>
              <span className="num text-muted-foreground text-right">
                {stats.trades ? hold(stats.meanHoldMs) : '—'}
              </span>
              <span className="num text-right text-[11px]">
                {stats.trades ? (
                  <>
                    <span className="text-lc">{stats.longestWin}</span>
                    <span className="text-muted-foreground">/</span>
                    <span className="text-lp">{stats.longestLoss}</span>
                  </>
                ) : (
                  '—'
                )}
              </span>
            </div>
          ))}
        </div>
      </div>
      <Footnote>
        One row per strategy, not per book — a strategy several books run is folded into one row and its books are
        named beside it. A strategy that has closed nothing is listed rather than hidden, because a book that has not
        traded is a fact about the desk. Drawdown is measured on each strategy&rsquo;s own cumulative profit and loss
        from zero, so the column does not add up and is not meant to.
      </Footnote>
    </section>
  )
}

/* ------------------------------------------------------- equity curves */

function Curves({ rows }: { rows: { strategy: string; stats: Stats }[] }) {
  const withTrades = rows.filter((r) => r.stats.trades > 0)
  return (
    <section>
      <Heading note="cumulative profit and loss from zero, by exit time">Equity</Heading>
      {withTrades.length === 0 ? (
        <Empty>No book has closed a trade yet, so there is no curve to draw.</Empty>
      ) : (
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-5">
          {withTrades.map(({ strategy, stats }) => (
            <figure key={strategy} className="border-border bg-card rounded-sm border px-2 py-1.5">
              <figcaption className="flex items-baseline justify-between gap-2 text-[11px]">
                <span className="num text-foreground truncate" title={strategy}>
                  {strategy}
                </span>
                <span className={cn('num shrink-0', bySign(stats.net))}>{usd(stats.net)}</span>
              </figcaption>
              <Spark points={stats.curve} />
              <p className="text-muted-foreground num mt-0.5 text-[10px]">
                {stats.trades} trade{stats.trades === 1 ? '' : 's'}
              </p>
            </figure>
          ))}
        </div>
      )}
    </section>
  )
}

/**
 * A step line, because money moves when a trade closes and not between.
 *
 * Deliberately axis-free: at this size a tick label is unreadable and the
 * number that matters is printed beside the caption. The zero line is drawn
 * because the sign of the curve is the whole question.
 */
function Spark({ points }: { points: { t: number; v: number }[] }) {
  const W = 220
  const H = 46
  if (points.length === 0) return <div style={{ height: H }} />

  const xs = points.map((p) => p.t)
  const vs = [0, ...points.map((p) => p.v)]
  const tMin = Math.min(...xs)
  const tMax = Math.max(...xs)
  let vMin = Math.min(...vs)
  let vMax = Math.max(...vs)
  if (!(vMax - vMin > 1e-9)) {
    vMin -= 1
    vMax += 1
  }
  // Breathing room, or the extreme of every curve is drawn along the border of
  // its own card and reads as clipped rather than as a high.
  const pad = (vMax - vMin) * 0.12
  vMin -= pad
  vMax += pad
  const x = (t: number) => (tMax === tMin ? W / 2 : ((t - tMin) / (tMax - tMin)) * W)
  const y = (v: number) => H - ((v - vMin) / (vMax - vMin)) * H

  let d = `M 0 ${y(0).toFixed(1)}`
  let prev = 0
  for (const p of points) {
    const px = x(p.t).toFixed(1)
    d += ` L ${px} ${y(prev).toFixed(1)} L ${px} ${y(p.v).toFixed(1)}`
    prev = p.v
  }
  const last = points[points.length - 1].v
  const zeroY = y(0)
  const inside = zeroY >= 0 && zeroY <= H

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      // Stretch rather than letterbox: the default `xMidYMid meet` centres a
      // 220-wide drawing inside whatever the card is and leaves the curve
      // floating in the middle of empty space.
      preserveAspectRatio="none"
      className="mt-1 block w-full"
      style={{ height: H }}
      role="img"
      aria-hidden
    >
      {inside && (
        <line x1={0} y1={zeroY} x2={W} y2={zeroY} className="stroke-border" strokeWidth={1} strokeDasharray="2 2" />
      )}
      {/* `vectorEffect` keeps the stroke 1.5px after the non-uniform stretch;
          without it the vertical segments would be drawn much thinner than the
          horizontal ones. */}
      <path
        d={d}
        fill="none"
        strokeWidth={1.5}
        vectorEffect="non-scaling-stroke"
        className={last >= 0 ? 'stroke-lc' : 'stroke-lp'}
      />
    </svg>
  )
}

/* ------------------------------------------------- session and symbol */

function SessionTable({ trades }: { trades: Tagged[] }) {
  const rows = useMemo(
    () =>
      SESSIONS.map((s) => {
        const inside = trades.filter((t) => inSession(nyHour(t.entryTime), s.from, s.to))
        return { ...s, stats: summarise(inside) }
      }),
    [trades],
  )
  const outside = useMemo(
    () =>
      summarise(
        trades.filter((t) => {
          const h = nyHour(t.entryTime)
          return !SESSIONS.some((s) => inSession(h, s.from, s.to))
        }),
      ),
    [trades],
  )

  return (
    <section>
      <Heading note="by the hour the trade was entered, on a New York clock">By session</Heading>
      {trades.length === 0 ? (
        <Empty>No closed trade to place in a session yet.</Empty>
      ) : (
        <Grid head={['Session', 'Trades', 'Net', 'Win', 'Mean R']}>
          {[...rows, { label: 'Outside', title: 'No named session covers this hour', stats: outside }].map((r) => (
            <div key={r.label} className="contents">
              <span className="num text-foreground" title={r.title}>
                {r.label}
              </span>
              <span className="num text-right">{r.stats.trades || '—'}</span>
              <span className={cn('num text-right', bySign(r.stats.net))}>
                {r.stats.trades ? usd(r.stats.net) : '—'}
              </span>
              <span className="num text-right">{r.stats.trades ? pct(r.stats.winRate) : '—'}</span>
              <span className={cn('num text-right', bySign(r.stats.meanR))}>
                {r.stats.trades ? signedR(r.stats.meanR) : '—'}
              </span>
            </div>
          ))}
        </Grid>
      )}
      <Footnote>
        These are the same windows the Workbench offers and the research batches use, and London and New York
        <strong className="text-foreground/80"> overlap between 08:00 and 11:00</strong> — a trade entered then is
        counted in both rows, so the rows do not sum to the total. Inventing tidier boundaries here would make this page
        disagree with the filter the strategies actually run under.
      </Footnote>
    </section>
  )
}

function SymbolTable({ trades, runs }: { trades: Tagged[]; runs: PaperRun[] }) {
  const rows = useMemo(() => {
    const groups = new Map<string, Tagged[]>()
    for (const t of trades) {
      const key = `${t.market}:${t.tf}`
      const list = groups.get(key)
      if (list) list.push(t)
      else groups.set(key, [t])
    }
    for (const r of runs) {
      const key = `${r.market}:${r.tf}`
      if (!groups.has(key)) groups.set(key, [])
    }
    return [...groups.entries()]
      .map(([key, list]) => ({ key, stats: summarise(list) }))
      .sort((a, b) => b.stats.trades - a.stats.trades || a.key.localeCompare(b.key))
  }, [trades, runs])

  return (
    <section>
      <Heading note="by the instrument and timeframe the book steps">By instrument</Heading>
      <Grid head={['Instrument', 'Trades', 'Net', 'Win', 'Mean R']}>
        {rows.map((r) => (
          <div key={r.key} className="contents">
            <span className="num text-foreground">{r.key}</span>
            <span className="num text-right">{r.stats.trades || '—'}</span>
            <span className={cn('num text-right', bySign(r.stats.net))}>
              {r.stats.trades ? usd(r.stats.net) : '—'}
            </span>
            <span className="num text-right">{r.stats.trades ? pct(r.stats.winRate) : '—'}</span>
            <span className={cn('num text-right', bySign(r.stats.meanR))}>
              {r.stats.trades ? signedR(r.stats.meanR) : '—'}
            </span>
          </div>
        ))}
      </Grid>
    </section>
  )
}

/* ------------------------------------------- streaks and distribution */

/** R buckets, chosen so a full stop loss and a clean win each land in one. */
const R_BUCKETS: { label: string; lo: number; hi: number }[] = [
  { label: '≤ −2R', lo: -Infinity, hi: -2 },
  { label: '−2 … −1R', lo: -2, hi: -1 },
  { label: '−1 … 0R', lo: -1, hi: 0 },
  { label: '0 … +1R', lo: 0, hi: 1 },
  { label: '+1 … +2R', lo: 1, hi: 2 },
  { label: '≥ +2R', lo: 2, hi: Infinity },
]

function Distribution({
  trades,
  overall,
  rows,
}: {
  trades: Tagged[]
  overall: Stats
  rows: { strategy: string; stats: Stats }[]
}) {
  const buckets = useMemo(() => {
    const counts = R_BUCKETS.map(() => 0)
    for (const t of trades) {
      const i = R_BUCKETS.findIndex((b) => t.r >= b.lo && t.r < b.hi)
      if (i >= 0) counts[i] += 1
    }
    return counts
  }, [trades])
  const most = Math.max(1, ...buckets)

  const worstStreak = rows.reduce(
    (acc, r) => (r.stats.longestLoss > acc.n ? { n: r.stats.longestLoss, strategy: r.strategy } : acc),
    { n: 0, strategy: '' },
  )

  return (
    <section>
      <Heading note="what a single trade has looked like, across every book">Distribution and streaks</Heading>
      {trades.length === 0 ? (
        <Empty>Nothing has closed, so there is no distribution.</Empty>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <div>
            <div className="space-y-1">
              {R_BUCKETS.map((b, i) => (
                <div key={b.label} className="grid grid-cols-[86px_1fr_36px] items-center gap-2 text-[11px]">
                  <span className="num text-muted-foreground">{b.label}</span>
                  <span className="bg-elevated h-3 overflow-hidden rounded-[2px]">
                    <span
                      className={cn('block h-full', b.hi <= 0 ? 'bg-lp/70' : 'bg-lc/70')}
                      style={{ width: `${(buckets[i] / most) * 100}%` }}
                    />
                  </span>
                  <span className="num text-right">{buckets[i] || ''}</span>
                </div>
              ))}
            </div>
            <Footnote>
              R is the trade&rsquo;s profit in units of its own risk, as the engine sized it. A book whose stop is a
              sizing unit rather than an order can exceed −1R, which is why the first bucket exists.
            </Footnote>
          </div>

          <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 self-start text-[12px]">
            <Stat label="Longest winning run" value={`${overall.longestWin}`} />
            <Stat label="Longest losing run" value={`${overall.longestLoss}`} />
            <Stat
              label="Worst run on one strategy"
              value={worstStreak.n ? `${worstStreak.n}` : '—'}
              note={worstStreak.strategy}
            />
            <Stat label="Mean hold" value={hold(overall.meanHoldMs)} />
            <Stat label="Best trade" value={signedR(overall.bestR)} tone={bySign(overall.bestR)} />
            <Stat label="Worst trade" value={signedR(overall.worstR)} tone={bySign(overall.worstR)} />
            <Stat
              label="Mean excursion against"
              value={signedR(overall.meanMae)}
              tone="text-lp"
              note="how far a trade went the wrong way before it closed"
            />
            <Stat
              label="Mean excursion for"
              value={signedR(overall.meanMfe)}
              tone="text-lc"
              note="how far it went the right way before it closed"
            />
          </dl>
        </div>
      )}
      {trades.length > 0 && trades.length < THIN_SAMPLE && (
        <p className="text-caution mt-2 text-[11px]">
          {trades.length} closed trade{trades.length === 1 ? '' : 's'} across every book. Nothing on this page is a
          measurement yet — a win rate over {trades.length} trades has a margin of roughly ±
          {Math.round((100 / Math.sqrt(trades.length)) * 0.5)} points, which is wider than any difference it could show.
        </p>
      )}
    </section>
  )
}

/* -------------------------------------------------------------- bits */

function Stat({
  label,
  value,
  tone,
  note,
}: {
  label: string
  value: string
  tone?: string
  note?: string
}) {
  return (
    <div className="border-border border-b pb-1">
      <dt className="text-muted-foreground text-[10px] tracking-wide uppercase">{label}</dt>
      <dd className={cn('num text-[13px]', tone ?? 'text-foreground')}>
        {value}
        {note && <span className="text-muted-foreground ml-1.5 text-[10px] normal-case">{note}</span>}
      </dd>
    </div>
  )
}

function Grid({ head, children }: { head: string[]; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(90px,1.4fr)_58px_86px_56px_64px] gap-2 text-[12px]">
      {head.map((h, i) => (
        <span
          key={h}
          className={cn('text-muted-foreground border-b pb-1 text-[10px] tracking-wide uppercase', i > 0 && 'text-right')}
        >
          {h}
        </span>
      ))}
      {children}
    </div>
  )
}

function Heading({ children, note }: { children: React.ReactNode; note?: string }) {
  return (
    <h2 className="mb-1.5 flex flex-wrap items-baseline gap-x-2 text-[11px] tracking-wide uppercase">
      <span className="text-foreground">{children}</span>
      {note && <span className="text-muted-foreground text-[10px] normal-case">{note}</span>}
    </h2>
  )
}

function Footnote({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-foreground mt-1.5 max-w-[80ch] text-[10px] leading-relaxed">{children}</p>
}

function Empty({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-foreground border-border rounded-sm border border-dashed px-3 py-6 text-[11px]">{children}</p>
}
