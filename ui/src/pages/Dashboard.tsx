/**
 * Overview.
 *
 * The screen someone arriving cold should be able to read in ten seconds: what
 * is running, what cleared, and what is blocked. Overview mode — see
 * `DESIGN.md`; do not add a dense table here without reading that first.
 *
 * One editorial decision runs through the whole page: **a figure is shown with
 * whatever qualifies it, in the same row.** A profit factor of 2.9 over nine
 * trades in a 28-hour window has already been produced here and looked exactly
 * like a result. If this screen can show that number without showing the nine,
 * it is doing harm.
 */

import { Suspense, lazy, useEffect, useMemo, useState } from 'react'

import {
  Chips,
  Empty,
  Figure,
  Label,
  Monogram,
  Panel,
  Pill,
  RailRow,
  SectionHeader,
} from '@/components/dashboard/primitives'
import type { Department } from '@/components/dashboard/OfficeFloor'
import { ResearchPanel } from '@/components/dashboard/ResearchPanel'
import { Skeleton } from '@/components/ui/skeleton'
import { api, type BarsResponse, type Catalog, type LeaderboardRow, type OptionsFrame, type Research } from '@/lib/api'
import { cn } from '@/lib/utils'

interface Props {
  catalog: Catalog
  market: string
  onError: (message: string) => void
}

/**
 * The model the main session runs on — the arbiter.
 *
 * Not read live: a browser cannot ask Claude Code which model is driving the
 * terminal. Update this when you `/model`. Everything else on the board reads
 * its model from the agent definition in `.claude/agents/*.md`, where it is
 * pinned and therefore true.
 */
const SESSION_MODEL = { name: 'Fable 5.1', id: 'claude-fable-5-1' } as const

const MODELS = {
  fable: { name: 'Fable 5.1', id: 'claude-fable-5-1' },
  sonnet: { name: 'Sonnet 5', id: 'claude-sonnet-5' },
} as const

/**
 * The review team, as configured in `.claude/agents`.
 *
 * `model` mirrors the `model:` line of each definition. The veto roles are
 * pinned to the strongest model available because they can block alone; the
 * advisory roles mostly read files and cite them, which Sonnet does well at a
 * fraction of the cost of running all seven.
 */
const TEAM = [
  { id: 'data-integrity', title: 'Data Integrity', role: 'Veto', model: MODELS.fable, line: 'Can this data answer the question asked of it?' },
  { id: 'adversary', title: 'Adversary', role: 'Veto', model: MODELS.fable, line: 'Breaks a result that looks good. Owns the null distribution.' },
  { id: 'risk', title: 'Risk', role: 'Veto', model: MODELS.fable, line: 'Limits, tails, and whether a rule is enforced or merely intended.' },
  { id: 'researcher', title: 'Researcher', role: 'Advisory', model: MODELS.sonnet, line: 'Turns ideas into experiments that can fail.' },
  { id: 'execution-realist', title: 'Execution Realist', role: 'Advisory', model: MODELS.sonnet, line: 'What is left after the spread is paid.' },
  { id: 'portfolio', title: 'Portfolio', role: 'Advisory', model: MODELS.sonnet, line: 'How many independent bets are actually on the table.' },
  { id: 'historian', title: 'Historian', role: 'Advisory', model: MODELS.sonnet, line: 'Has this been tried, and what did it cost last time?' },
] as const

/**
 * The floor: open plan, one desk cluster per department on its own rug, and
 * a single glass office for the arbiter. North of the aisle: the office, the
 * advisory desks, the archive. South: the data racks, the strategy desk, the
 * engine bay and the three veto desks in a row. A file leaves the archive
 * and comes back to it, as a record or as closed.
 */
const seat = (id: (typeof TEAM)[number]['id']) => {
  const agent = TEAM.find((a) => a.id === id)!
  return { id: agent.id, title: agent.title, model: agent.model.name }
}
const DEPARTMENTS: Department[] = [
  // The west end: reception, open to the aisle, the show car on its plinth.
  { id: 'reception', title: 'Reception', line: 'Public lounge, with the show car.', tone: 'public', x: -10.4, z: 0.2, w: 3.6, d: 8.6, occupants: [], furniture: 'lounge' },
  // The east end: the meeting room, glass like the office.
  { id: 'meeting', title: 'Meeting room', line: 'Where a decision is argued before it is written.', tone: 'public', x: 10.4, z: -3.0, w: 3.4, d: 3.4, enclosed: true, occupants: [], furniture: 'meeting' },
  // North of the aisle: the glass office, the advisory desks, the archive.
  { id: 'arbiter', title: "Arbiter's office", line: 'Reads the receipts and the vetoes; decides last.', tone: 'arbiter', x: -6.6, z: -3.0, w: 3.4, d: 3.2, enclosed: true, occupants: [{ id: 'arbiter', title: 'Arbiter', model: SESSION_MODEL.name }] },
  { id: 'advisory', title: 'Advisory', line: 'Notes to the arbiter; none of them can stop anything alone.', tone: 'advisory', x: -0.6, z: -3.0, w: 5.2, d: 3.6, arrange: 'grid', occupants: [seat('researcher'), seat('execution-realist'), seat('portfolio'), seat('historian')] },
  { id: 'archive', title: 'Archive', line: 'Hypotheses, run receipts, decisions, backlog.', tone: 'ops', x: 5.6, z: -3.0, w: 4.4, d: 3.4, occupants: [], furniture: 'shelves' },
  // South of the aisle: data, the lab, the engine bay, then the veto desks in a row.
  { id: 'data', title: 'Data room', line: 'Broker bars, two free feeds, the tape collectors.', tone: 'ops', x: -6.8, z: 3.0, w: 3.4, d: 3.0, occupants: [], furniture: 'racks', racks: ['MT5 · Vantage', 'Dukascopy', 'Binance', 'OTL tape'] },
  { id: 'lab', title: 'Strategy lab', line: 'Pre-registers, implements, tests for look-ahead.', tone: 'ops', x: -3.4, z: 3.0, w: 2.6, d: 3.0, occupants: [{ id: 'strategy-implementer', title: 'Implementer', model: MODELS.sonnet.name }] },
  { id: 'engine', title: 'Engine room', line: 'Backtest, walk-forward, nulls; the search binary.', tone: 'ops', x: -0.2, z: 3.0, w: 3.0, d: 3.0, occupants: [], furniture: 'engine' },
  { id: 'data-integrity', title: 'Data Integrity', line: 'Veto', tone: 'veto', x: 3.0, z: 3.0, w: 1.6, d: 3.0, occupants: [seat('data-integrity')] },
  { id: 'adversary', title: 'Adversary', line: 'Veto', tone: 'veto', x: 5.0, z: 3.0, w: 1.6, d: 3.0, occupants: [seat('adversary')] },
  { id: 'risk', title: 'Risk', line: 'Veto', tone: 'veto', x: 7.0, z: 3.0, w: 1.6, d: 3.0, occupants: [seat('risk')] },
]

/**
 * Loaded on demand. three.js is half a megabyte, and the tape reader and the
 * workbench never draw a triangle — they should not pay for one.
 */
const OfficeFloor = lazy(() => import('@/components/dashboard/OfficeFloor').then((m) => ({ default: m.OfficeFloor })))

type Filter = 'all' | 'promising' | 'thin'

/**
 * Motion on the floor: follows the system's reduced-motion setting unless
 * the reader has said otherwise here. Windows with "animation effects" off
 * reports reduced motion, and the floor then stands still — which reads as
 * broken to someone who came to see it run. The choice is kept per browser.
 */
type Motion = 'auto' | 'on' | 'off'
const MOTION_KEY = 'fd.floor.motion'
const readMotion = (): Motion => {
  try {
    const v = localStorage.getItem(MOTION_KEY)
    return v === 'on' || v === 'off' ? v : 'auto'
  } catch {
    return 'auto'
  }
}
const systemReducedMotion = () =>
  typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

/** Below this a metric is a sample, not a measurement. Mirrors the gate. */
const THIN_TRADES = 30

export function Dashboard({ catalog, market, onError }: Props) {
  const [rows, setRows] = useState<LeaderboardRow[] | null>(null)
  const [bars, setBars] = useState<BarsResponse | null>(null)
  const [levels, setLevels] = useState<{ frame: OptionsFrame | null; frames?: number } | null>(null)
  const [filter, setFilter] = useState<Filter>('all')
  const [motion, setMotion] = useState<Motion>(readMotion)
  const reduced = systemReducedMotion()
  const animate = motion === 'on' || (motion === 'auto' && !reduced)
  const cycleMotion = () => {
    const next: Motion = animate ? 'off' : 'on'
    setMotion(next)
    try {
      localStorage.setItem(MOTION_KEY, next)
    } catch {
      /* private mode: the choice lasts the page */
    }
  }
  const [research, setResearch] = useState<Research | null>(null)
  const [now, setNow] = useState(() => Date.now())

  // The research files change when the loop writes them; re-read every
  // fifteen seconds and say how old the reading is. Not a stream — nothing
  // pushes — and the label says so.
  useEffect(() => {
    let cancelled = false
    const load = () =>
      api
        .research()
        .then((r) => {
          if (!cancelled) {
            setResearch(r)
            setNow(Date.now())
          }
        })
        .catch(() => {})
    load()
    const timer = window.setInterval(load, 15_000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [])

  const timeframe = catalog.defaults.timeframe

  useEffect(() => {
    if (!market) return
    let cancelled = false

    api
      .leaderboard(market, timeframe)
      .then((response) => !cancelled && setRows(response.rows))
      .catch((error: Error) => onError(error.message))
    api
      .bars(market, timeframe)
      .then((response) => !cancelled && setBars(response))
      .catch((error: Error) => onError(error.message))
    api
      .levels(market)
      .then((response) => !cancelled && setLevels(response))
      .catch(() => {
        // A market with no tape has no levels. That is a state, not a failure,
        // and the coverage card says so in words.
        if (!cancelled) setLevels({ frame: null, frames: 0 })
      })

    return () => {
      cancelled = true
    }
  }, [market, timeframe, onError])

  const scored = useMemo(
    () =>
      (rows ?? [])
        .filter((row) => row.metrics)
        .sort((a, b) => (b.metrics?.profitFactor ?? 0) - (a.metrics?.profitFactor ?? 0)),
    [rows],
  )

  const visible = useMemo(() => {
    if (filter === 'promising') return scored.filter((row) => (row.metrics?.trades ?? 0) >= THIN_TRADES)
    if (filter === 'thin') return scored.filter((row) => (row.metrics?.trades ?? 0) < THIN_TRADES)
    return scored
  }, [scored, filter])

  // Options coverage: the fraction of bars the tape actually reaches. This is
  // the number that decides whether half the strategies mean anything, and it
  // has been 0.16% for the life of the project so far.
  const coverage = useMemo(() => {
    if (!bars?.stats.bars || !levels?.frames) return null
    const frames = levels.frames
    const stepMinutes = timeframe.endsWith('m') ? Number.parseInt(timeframe, 10) : 60
    // Frames are five minutes apart; express them in bars of the chart's own
    // timeframe so the comparison is apples to apples.
    const covered = Math.min(Math.round((frames * 5) / stepMinutes), bars.stats.bars)
    return { covered, total: bars.stats.bars, pct: (covered / bars.stats.bars) * 100 }
  }, [bars, levels, timeframe])

  return (
    <main className="mx-auto w-full max-w-[1440px] space-y-6 p-4 lg:p-6">
      {/* ---- row one: two rails around a hero ---- */}
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.6fr)_minmax(0,1fr)]">
        <Panel className="p-4">
          <div className="mb-3 flex items-center justify-between">
            <Label>Ranked by profit factor</Label>
            <Pill>{timeframe}</Pill>
          </div>
          {!rows ? (
            <RailSkeleton />
          ) : scored.length === 0 ? (
            <Empty title="No strategy has produced a trade here." hint="search --market=btc --mode=compare" />
          ) : (
            <ol className="space-y-0.5">
              {scored.slice(0, 6).map((row, index) => (
                <RailRow
                  key={row.id}
                  rank={index + 1}
                  name={row.id}
                  meta={`${row.metrics?.trades ?? 0} trades`}
                  value={(row.metrics?.profitFactor ?? 0).toFixed(3)}
                  delta={{
                    text: `${(row.metrics?.expectancy ?? 0) >= 0 ? '+' : ''}${(row.metrics?.expectancy ?? 0).toFixed(3)}R`,
                    tone: (row.metrics?.expectancy ?? 0) >= 0 ? 'up' : 'down',
                  }}
                />
              ))}
            </ol>
          )}
        </Panel>

        <HeroCard market={market} bars={bars} coverage={coverage} />

        <Panel className="p-4">
          <div className="mb-3 flex items-center justify-between">
            <Label>Review team</Label>
            <Pill>{TEAM.length} roles</Pill>
          </div>
          <ul className="space-y-0.5">
            {TEAM.slice(0, 6).map((agent) => (
              <li
                key={agent.id}
                className="hover:bg-accent/40 -mx-2 flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors"
              >
                <Monogram seed={agent.id} label={agent.title} size={26} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline gap-2">
                    <span className="truncate text-[13px] font-medium">{agent.title}</span>
                    <span className="text-muted-foreground shrink-0 text-[10px]">{agent.model.name}</span>
                  </span>
                  <span className="text-muted-foreground block truncate text-[11px]">{agent.line}</span>
                </span>
                <span
                  className={cn(
                    'shrink-0 text-[11px]',
                    agent.role === 'Veto' ? 'text-caution' : 'text-muted-foreground',
                  )}
                >
                  {agent.role}
                </span>
              </li>
            ))}
          </ul>
        </Panel>
      </div>

      {/* ---- research: what the loop is doing ---- */}
      <section>
        <SectionHeader
          title="Research"
          subtitle="One hypothesis at a time through the six steps. The tables are the run receipts, quoted; the loop never widens a grid to pass a gate."
        />
        <ResearchPanel
          research={research}
          updatedAgo={research ? `${Math.max(0, Math.round((now - research.updatedAt) / 60_000))} min since the files changed` : '…'}
        />
      </section>

      {/* ---- row two: the floor ---- */}
      <section>
        <SectionHeader
          title="The floor"
          subtitle="The company as an open-plan floor; only the arbiter sits behind glass. A file leaves the archive, is written up in the lab, run in the engine bay, and passes three desks that can each send it back before it reaches the office."
        />
        <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
          <Panel className="relative min-h-[460px] overflow-hidden">
            <Suspense fallback={<Skeleton className="absolute inset-0 rounded-[inherit]" />}>
              <OfficeFloor departments={DEPARTMENTS} animate={animate} className="absolute inset-0" />
            </Suspense>
            {/* Which model each wing runs on. The arbiter's is the session's
                and is set by hand; the rest are read from the agent
                definitions. */}
            <div className="pointer-events-none absolute top-3 left-4 flex flex-wrap items-center gap-2 text-[11px]">
              <span className="bg-background/70 border-border rounded-full border px-2 py-0.5 backdrop-blur">
                <span className="text-primary font-medium">Arbiter</span>
                <span className="text-muted-foreground"> · {SESSION_MODEL.name}</span>
              </span>
              <span className="bg-background/70 border-border rounded-full border px-2 py-0.5 backdrop-blur">
                <span className="text-caution font-medium">Veto wing ×3</span>
                <span className="text-muted-foreground"> · {MODELS.fable.name}</span>
              </span>
              <span className="bg-background/70 border-border rounded-full border px-2 py-0.5 backdrop-blur">
                <span className="font-medium">Advisory ×4</span>
                <span className="text-muted-foreground"> · {MODELS.sonnet.name}</span>
              </span>
              <span className="bg-background/70 border-border rounded-full border px-2 py-0.5 backdrop-blur">
                <span className="text-brand-mint font-medium">Operations</span>
                <span className="text-muted-foreground"> · data, lab, engine, archive</span>
              </span>
              <span className="bg-background/70 border-border rounded-full border px-2 py-0.5 backdrop-blur">
                <span className="font-medium">Public</span>
                <span className="text-muted-foreground"> · reception, meeting room</span>
              </span>
            </div>
            {/* The things the scene cannot say on its own. */}
            <div className="pointer-events-none absolute bottom-3 left-4 flex flex-wrap items-center gap-3 text-[11px]">
              <span className="flex items-center gap-1.5">
                <span className="bg-brand-mint size-2 rounded-full" aria-hidden /> the file, in review
              </span>
              <span className="flex items-center gap-1.5">
                <span className="bg-caution size-2 rounded-full" aria-hidden /> stamped: sent back, closed
              </span>
              <span className="flex items-center gap-1.5">
                <span className="bg-primary size-2 rounded-full" aria-hidden /> on the arbiter's desk
              </span>
              <span className="text-muted-foreground">drag to orbit</span>
            </div>
            {/* Motion: the one control on the scene. Reads the system setting,
                says so when that is what stopped it, and lets the reader
                override it. */}
            <button
              type="button"
              onClick={cycleMotion}
              className="bg-background/70 border-border hover:bg-accent/60 absolute right-4 bottom-3 flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] backdrop-blur transition-colors"
              aria-pressed={animate}
              title={reduced && motion === 'auto' ? 'Your system asks for reduced motion; the floor is standing still. Click to run it anyway.' : 'Toggle motion on the floor'}
            >
              <span className={cn('size-2 rounded-full', animate ? 'bg-brand-mint' : 'bg-muted-foreground')} aria-hidden />
              {animate ? 'running' : reduced && motion === 'auto' ? 'paused by system · run' : 'paused · run'}
            </button>
          </Panel>

          <Panel className="p-4">
            <Label className="mb-3">Departments</Label>
            <ul className="space-y-2.5">
              {DEPARTMENTS.map((dept) => (
                <li key={dept.id} className="flex items-start gap-2.5">
                  <Monogram seed={dept.id} label={dept.title} size={28} />
                  <span className="min-w-0 flex-1">
                    <span className="flex items-baseline gap-2">
                      <span className="truncate text-[13px] font-medium">{dept.title}</span>
                      {dept.occupants[0] && (
                        <span className="text-muted-foreground shrink-0 font-mono text-[10px]">{dept.occupants[0].model}</span>
                      )}
                    </span>
                    <span className="text-muted-foreground mt-0.5 block text-[11px] leading-snug">
                      {dept.occupants.length > 1 ? dept.occupants.map((o) => o.title).join(' · ') : dept.line}
                    </span>
                  </span>
                  <Pill tone={dept.tone === 'veto' ? 'caution' : 'neutral'}>
                    {dept.tone === 'veto'
                      ? 'Veto'
                      : dept.tone === 'arbiter'
                        ? 'Decides'
                        : dept.tone === 'advisory'
                          ? 'Advisory'
                          : dept.tone === 'public'
                            ? 'Public'
                            : 'Ops'}
                  </Pill>
                </li>
              ))}
            </ul>
            <p className="text-muted-foreground border-border mt-4 border-t pt-3 text-[11px] leading-relaxed">
              No walls but one: the veto desks sit in a row between the engine and the glass office, and a file
              passes all three or goes back to the archive. The gates are not up for a vote at any desk.
            </p>
          </Panel>
        </div>
      </section>

      {/* ---- row three: the table ---- */}
      <section>
        <SectionHeader
          title="Strategies"
          subtitle="Default parameters, in sample. A leaderboard ranks methods; it does not measure any of them."
          action={
            <Chips
              value={filter}
              onChange={setFilter}
              options={[
                { id: 'all', label: 'All' },
                { id: 'promising', label: `${THIN_TRADES}+ trades` },
                { id: 'thin', label: 'Too thin to read' },
              ]}
            />
          }
        />
        <Panel className="overflow-hidden">
          {!rows ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 5 }, (_, i) => (
                <Skeleton key={i} className="h-8 w-full" />
              ))}
            </div>
          ) : visible.length === 0 ? (
            <Empty title="Nothing in this filter." />
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full min-w-[720px] text-[13px]">
                <thead>
                  <tr className="border-border text-muted-foreground border-b text-[11px] tracking-wide uppercase">
                    <th className="px-4 py-2.5 text-left font-medium">Strategy</th>
                    <th className="px-4 py-2.5 text-right font-medium">Trades</th>
                    <th className="px-4 py-2.5 text-right font-medium">Win</th>
                    <th className="px-4 py-2.5 text-right font-medium">Profit factor</th>
                    <th className="px-4 py-2.5 text-right font-medium">Expectancy</th>
                    <th className="px-4 py-2.5 text-right font-medium">Max DD</th>
                    <th className="px-4 py-2.5 text-left font-medium">Reading</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((row) => {
                    const m = row.metrics!
                    const thin = m.trades < THIN_TRADES
                    return (
                      <tr key={row.id} className="border-border/60 hover:bg-accent/30 border-b transition-colors last:border-0">
                        <td className="px-4 py-2.5">
                          <span className="flex items-center gap-2.5">
                            <Monogram seed={row.id} label={row.id} size={24} />
                            <span className="font-medium">{row.id}</span>
                          </span>
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          <Figure value={String(m.trades)} />
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          <Figure value={Number.isFinite(m.winRate) ? `${(m.winRate * 100).toFixed(1)}%` : '—'} />
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          <Figure value={Number.isFinite(m.profitFactor) ? m.profitFactor.toFixed(3) : '—'} />
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          {/* A metric the server could not compute (NaN) arrives as null:
                              a method with one trade has no expectancy to show. */}
                          <Figure
                            value={Number.isFinite(m.expectancy) ? `${m.expectancy >= 0 ? '+' : ''}${m.expectancy.toFixed(3)}` : '—'}
                            unit="R"
                            tone={!Number.isFinite(m.expectancy) ? 'neutral' : m.expectancy >= 0 ? 'up' : 'down'}
                          />
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          <Figure value={`$${Math.round(m.maxDrawdownUsd).toLocaleString()}`} />
                        </td>
                        <td className="px-4 py-2.5">
                          {/* The qualifier travels with the number, in the same
                              row. A 2.9 profit factor over nine trades is not a
                              better version of one over nine hundred. */}
                          {thin ? (
                            <Pill tone="caution">{m.trades} trades — too few to read</Pill>
                          ) : (
                            <Pill>in sample</Pill>
                          )}
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </Panel>
      </section>
    </main>
  )
}

function RailSkeleton() {
  return (
    <div className="space-y-2">
      {Array.from({ length: 5 }, (_, i) => (
        <Skeleton key={i} className="h-9 w-full" />
      ))}
    </div>
  )
}

/**
 * The hero.
 *
 * The reference this was styled from puts a progress path here. Ours carries
 * the one fact that decides whether half the product means anything: how much
 * of the price history the options tape actually reaches.
 */
function HeroCard({
  market,
  bars,
  coverage,
}: {
  market: string
  bars: BarsResponse | null
  coverage: { covered: number; total: number; pct: number } | null
}) {
  const thin = coverage !== null && coverage.pct < 5
  return (
    <Panel className="hero-gradient relative overflow-hidden p-5">
      <div className="flex items-start justify-between gap-3">
        <div>
          <Label className="text-foreground/60">Options tape coverage</Label>
          <p className="mt-1 text-[13px] font-medium">{market || '—'}</p>
        </div>
        {bars?.live ? <Pill tone="live">live feed</Pill> : <Pill>snapshot</Pill>}
      </div>

      {coverage === null ? (
        <div className="mt-6 space-y-2">
          <Skeleton className="h-10 w-40" />
          <Skeleton className="h-3 w-full" />
        </div>
      ) : (
        <>
          <div className="mt-5 flex items-baseline gap-2">
            <Figure value={coverage.pct.toFixed(2)} className="text-[40px] leading-none font-semibold" />
            <span className="text-foreground/60 text-[15px]">%</span>
          </div>
          <p className="text-foreground/70 mt-1 text-[13px]">
            <Figure value={coverage.covered.toLocaleString()} /> of{' '}
            <Figure value={coverage.total.toLocaleString()} /> bars carry an options frame
          </p>

          {/* A bar that is almost entirely empty, drawn honestly. Rounding it up
              to something visible would be a lie about the only number here. */}
          <div
            className="bg-background/40 mt-4 h-1.5 overflow-hidden rounded-full"
            role="progressbar"
            aria-valuenow={Math.round(coverage.pct)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="options tape coverage"
          >
            <div
              className="bg-primary h-full rounded-full transition-[width] duration-500"
              style={{ width: `${Math.max(coverage.pct, 0.4)}%` }}
            />
          </div>

          {thin && (
            <p className="text-foreground/70 mt-4 text-[12px] leading-relaxed">
              Every options-derived strategy is bounded by this. Until it grows, their results describe
              a day — not a method.
              <span className="mt-1 block font-mono text-[11px] opacity-80">
                fd-ingest --bin collect --market={market || 'btc'}
              </span>
            </p>
          )}
        </>
      )}
    </Panel>
  )
}
