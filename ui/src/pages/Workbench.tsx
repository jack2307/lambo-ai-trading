import { useCallback, useEffect, useMemo, useState } from 'react'

import { PriceChart, type ActiveIndicator } from '@/components/PriceChart'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  api,
  type BacktestResult,
  type BarsResponse,
  type Catalog,
  type IndicatorPoint,
  type LeaderboardRow,
  type OptionsFrame,
} from '@/lib/api'
import { day, money, num, pct, price, signClass } from '@/lib/format'
import { useLiveBar, type LiveStatus } from '@/lib/useLiveBar'
import { cn } from '@/lib/utils'

/**
 * Terminal mode (see ui/DESIGN.md). Three regions and no cards:
 *
 * ```
 * ┌ rail 272px ──────┬ toolbar ─────────────────────────────────┐
 * │ strategy          │ timeframe · indicators · overlays         │
 * │ parameters        ├ chart (fills whatever is left) ──────────┤
 * │ [run]             │                                           │
 * │ ───────           ├ status strip ────────────────────────────┤
 * │ result            ├ dock: leaderboard | trades | model ──────┤
 * └───────────────────┴───────────────────────────────────────────┘
 * ```
 *
 * The rail is what you *set*; the chart is what you *see*; the dock is what
 * you *read*. Tables in the dock run the full width so a leaderboard row can
 * carry seven figures with headers instead of four squeezed into a sidebar.
 */

/** One accent plus the data colours; indicator lines cycle through these. */
const LINE_COLORS = ['#7d94e8', '#d99446', '#3fbcc0', '#e05d6a', '#46c98a', '#b48ae0', '#c9d1de']

/** Height of the sticky app bar, which the page fills the rest of the viewport under. */
const APP_BAR = 45

interface Props {
  catalog: Catalog
  market: string
  onError: (message: string) => void
}

export function Workbench({ catalog, market, onError }: Props) {
  const [timeframe, setTimeframe] = useState(catalog.defaults.timeframe)
  const [strategyId, setStrategyId] = useState(catalog.strategies[0]?.id ?? '')
  const [params, setParams] = useState<Record<string, number>>({})
  const [bars, setBars] = useState<BarsResponse | null>(null)
  const [frame, setFrame] = useState<OptionsFrame | null>(null)
  const [leaderboard, setLeaderboard] = useState<LeaderboardRow[] | null>(null)
  const [result, setResult] = useState<BacktestResult | null>(null)
  const [indicators, setIndicators] = useState<ActiveIndicator[]>([])
  const [series, setSeries] = useState<Record<string, IndicatorPoint[]>>({})
  const [indicatorPick, setIndicatorPick] = useState(catalog.indicators[0]?.id ?? '')
  const [running, setRunning] = useState(false)
  const [showLevels, setShowLevels] = useState(true)
  const [showMarkers, setShowMarkers] = useState(true)
  const [showZones, setShowZones] = useState(true)
  const [dock, setDock] = useState<'leaderboard' | 'trades' | 'model'>('leaderboard')
  // Gates in the research loop's spelling, one per line or comma-separated.
  const [filtersText, setFiltersText] = useState('')
  // Sessions as the research batches spell them: New York wall-clock hours,
  // the same `hours:` token `Filter::parse` reads, so a Workbench run over a
  // session is the batch's computation over that session.
  const SESSIONS: [string, string, string][] = [
    ['All', '', 'no hours filter'],
    ['Asia', '1800-0200', '18:00-02:00 New York'],
    ['London', '0300-1100', '03:00-11:00 New York'],
    ['NY', '0800-1600', '08:00-16:00 New York'],
  ]
  const hoursToken = (text: string) => text.match(/hours:(\d{4}-\d{4})/)?.[1] ?? ''
  const session = (() => {
    const h = hoursToken(filtersText)
    if (!h) return 'All'
    return SESSIONS.find(([, token]) => token === h)?.[0] ?? 'Custom'
  })()
  const pickSession = (token: string) => {
    const rest = filtersText
      .split(/[,;\n]/)
      .map((f) => f.trim())
      .filter((f) => f && !f.startsWith('hours:'))
    setFiltersText([...rest, ...(token ? [`hours:${token}`] : [])].join(', '))
  }
  const [guards, setGuards] = useState(false)
  const [rangeFrom, setRangeFrom] = useState('')
  const [rangeTo, setRangeTo] = useState('')

  const strategy = useMemo(
    () => catalog.strategies.find((s) => s.id === strategyId),
    [catalog.strategies, strategyId],
  )

  // Only subscribe once the historical bars are in: the chart has to have a
  // series before a forming bar can be folded onto the end of it.
  const { bar: liveBar, status: liveStatus } = useLiveBar(market, timeframe, Boolean(bars?.live))

  useEffect(() => {
    setParams(strategy ? { ...strategy.params } : {})
  }, [strategy])

  useEffect(() => {
    if (!market) return
    setBars(null)
    api.bars(market, timeframe).then(setBars).catch((err: Error) => onError(err.message))
  }, [market, timeframe, onError])

  useEffect(() => {
    if (!market) return
    // A market with no options tape simply has no levels; that is not an error.
    api.levels(market).then((r) => setFrame(r.frame)).catch(() => setFrame(null))
  }, [market])

  useEffect(() => {
    if (!market) return
    setLeaderboard(null)
    api
      .leaderboard(market, timeframe)
      .then((r) => setLeaderboard(r.rows))
      .catch((err: Error) => onError(err.message))
  }, [market, timeframe, onError])

  // Indicator data is fetched from the server, never computed here: an
  // indicator drawn with different code from the one a strategy trades on is a
  // lie on screen.
  useEffect(() => {
    if (!market || indicators.length === 0) {
      setSeries({})
      return
    }
    const specs = indicators.map((entry) => ({ id: entry.id, params: entry.params }))
    api
      .indicators(market, timeframe, specs)
      .then((r) => setSeries(r.series))
      .catch((err: Error) => onError(err.message))
  }, [market, timeframe, indicators, onError])

  const addIndicator = useCallback(() => {
    const def = catalog.indicators.find((i) => i.id === indicatorPick)
    if (!def) return
    const numeric = Object.entries(def.params).filter(([, v]) => typeof v === 'number') as [string, number][]
    const key = numeric.length ? `${def.id}_${numeric.map(([, v]) => v).join('_')}` : def.id
    setIndicators((current) => {
      if (current.some((entry) => entry.key === key)) return current
      return [
        ...current,
        {
          key,
          id: def.id,
          params: Object.fromEntries(numeric),
          outputs: def.outputs,
          pane: def.pane === 'pane' ? current.filter((e) => e.pane > 0).length + 1 : 0,
          color: LINE_COLORS[current.length % LINE_COLORS.length],
        },
      ]
    })
  }, [catalog.indicators, indicatorPick])

  const runBacktest = useCallback(async () => {
    if (!market || !strategyId || running) return
    setRunning(true)
    try {
      const filters = filtersText
        .split(/[,;]/)
        .flatMap((chunk) => chunk.split(String.fromCharCode(10)))
        .map((f) => f.trim())
        .filter(Boolean)
      const outcome = await api.backtest(market, timeframe, strategyId, params, filters, guards, { from: rangeFrom, to: rangeTo })
      setResult(outcome)
      // The rail's result block and the dock switching to the fills are the
      // announcement; a toast on top of them covered the newest trades.
      setDock('trades')
    } catch (err) {
      onError((err as Error).message)
    } finally {
      setRunning(false)
    }
  }, [market, timeframe, strategyId, params, filtersText, guards, rangeFrom, rangeTo, running, onError])

  // ⌘/Ctrl+Enter runs from anywhere on the page — the rail's inputs included.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
        event.preventDefault()
        void runBacktest()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [runBacktest])

  return (
    <div
      className="grid grid-cols-[272px_minmax(0,1fr)] overflow-hidden"
      style={{ height: `calc(100dvh - ${APP_BAR}px)` }}
    >
      {/* ---------------------------------------------------------- rail */}
      <aside className="flex min-h-0 flex-col overflow-y-auto border-r">
        <section className="border-b px-3 pt-3 pb-3">
          <RailHeading>Strategy</RailHeading>
          <Select value={strategyId} onValueChange={setStrategyId}>
            <SelectTrigger size="sm" className="h-7 w-full text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {catalog.strategies.map((s) => (
                <SelectItem key={s.id} value={s.id}>
                  {s.name}
                  {s.needsOptions ? ' · needs tape' : ''}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {strategy && (
            <p className="text-muted-foreground mt-2 text-[11px] leading-snug">{strategy.description}</p>
          )}
          {strategy?.needsOptions && !frame && (
            <p className="text-caution mt-2 text-[11px] leading-snug">
              Reads the options frame, and this market has no tape loaded — it will take no trades.
            </p>
          )}
        </section>

        <section className="border-b px-3 pt-3 pb-3">
          <RailHeading>Parameters</RailHeading>
          {Object.keys(params).length === 0 ? (
            <p className="text-muted-foreground text-[11px]">This method has no parameters.</p>
          ) : (
            <div className="grid grid-cols-[1fr_84px] items-center gap-x-3 gap-y-1.5">
              {Object.entries(params).map(([key, value]) => {
                const grid = strategy?.grid?.[key]
                return (
                  <label key={key} className="contents">
                    <span
                      className="text-muted-foreground truncate text-[11px]"
                      title={grid ? `sweep grid: ${grid.join(', ')}` : undefined}
                    >
                      {key}
                      {grid && <span className="text-muted-foreground/60 ml-1">·{grid.length}</span>}
                    </span>
                    <input
                      type="number"
                      step="any"
                      value={value}
                      onChange={(e) => setParams((current) => ({ ...current, [key]: Number(e.target.value) }))}
                      className="border-input bg-background num focus-visible:ring-ring/50 h-7 w-full rounded-md border px-2 text-right text-xs focus-visible:ring-[3px] focus-visible:outline-none"
                    />
                  </label>
                )
              })}
            </div>
          )}
          <div className="mt-3">
            <span className="text-muted-foreground block text-[11px]">session (New York hours)</span>
            <div className="mt-1 flex flex-wrap gap-1" role="group" aria-label="session">
              {SESSIONS.map(([label, token, title]) => (
                <button
                  key={label}
                  type="button"
                  title={title}
                  aria-pressed={session === label}
                  onClick={() => pickSession(token)}
                  className={
                    'rounded border px-2 py-0.5 font-mono text-[10px] transition-colors ' +
                    (session === label
                      ? 'border-primary bg-primary/15 text-foreground'
                      : 'border-border bg-background hover:bg-accent/60')
                  }
                >
                  {label}
                </button>
              ))}
              {session === 'Custom' && (
                <span className="text-muted-foreground/70 self-center font-mono text-[10px]">custom hours:{hoursToken(filtersText)}</span>
              )}
            </div>
          </div>
          <label className="mt-2 block">
            <span className="text-muted-foreground block text-[11px]">
              filters <span className="text-muted-foreground/60">· weekdays, hours:0800-1200, flat:1630-1815, vol:14/100:1.2-99, volabs:14:0.075-9</span>
            </span>
            <textarea
              value={filtersText}
              onChange={(e) => setFiltersText(e.target.value)}
              rows={2}
              placeholder="weekdays, flat:1630-1815, hours:0920-1200"
              className="border-input bg-background num focus-visible:ring-ring/50 mt-1 w-full resize-none rounded-md border px-2 py-1 text-[11px] focus-visible:ring-[3px] focus-visible:outline-none"
            />
          </label>
          {/* Quick ranges: back from today, in UTC. */}
          <div className="mt-2 flex flex-wrap gap-1">
            {(
              [
                ['1W', 0, 7], ['2W', 0, 14], ['1M', 1, 0], ['2M', 2, 0], ['3M', 3, 0], ['6M', 6, 0],
                ['1Y', 12, 0], ['2Y', 24, 0], ['3Y', 36, 0], ['YTD', -1, 0], ['All', -2, 0],
              ] as [string, number, number][]
            ).map(([label, months, days]) => {
              const iso = (d: Date) => d.toISOString().slice(0, 10)
              const pick = () => {
                const now = new Date()
                if (months === -2) {
                  setRangeFrom('')
                  setRangeTo('')
                  return
                }
                const from = new Date(now)
                if (months === -1) from.setUTCMonth(0, 1)
                else if (days) from.setUTCDate(from.getUTCDate() - days)
                else from.setUTCMonth(from.getUTCMonth() - months)
                setRangeFrom(iso(from))
                setRangeTo(iso(now))
              }
              return (
                <button
                  key={label}
                  type="button"
                  onClick={pick}
                  className="border-border bg-background hover:bg-accent/60 rounded border px-1.5 py-0.5 font-mono text-[10px] transition-colors"
                >
                  {label}
                </button>
              )
            })}
          </div>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <label className="block">
              <span className="text-muted-foreground block text-[11px]">from (UTC)</span>
              <input
                type="date"
                value={rangeFrom}
                onChange={(e) => setRangeFrom(e.target.value)}
                className="border-input bg-background num focus-visible:ring-ring/50 mt-1 h-7 w-full rounded-md border px-2 text-[11px] focus-visible:ring-[3px] focus-visible:outline-none"
              />
            </label>
            <label className="block">
              <span className="text-muted-foreground block text-[11px]">to (UTC)</span>
              <input
                type="date"
                value={rangeTo}
                onChange={(e) => setRangeTo(e.target.value)}
                className="border-input bg-background num focus-visible:ring-ring/50 mt-1 h-7 w-full rounded-md border px-2 text-[11px] focus-visible:ring-[3px] focus-visible:outline-none"
              />
            </label>
          </div>
          <p className="text-muted-foreground/70 mt-1 text-[10px]">empty = the whole stored series for this market and timeframe</p>
          <label className="text-muted-foreground mt-2 flex items-center gap-2 text-[11px]">
            <input type="checkbox" checked={guards} onChange={(e) => setGuards(e.target.checked)} className="accent-[var(--primary)]" />
            enforce [trading.guards] (daily cap, loss limit, cooldown)
          </label>
          <Button size="sm" className="mt-3 h-8 w-full text-xs" disabled={running} onClick={runBacktest}>
            {running ? 'Running…' : 'Run backtest'}
            <kbd className="text-primary-foreground/60 ml-auto font-mono text-[10px]">⌘⏎</kbd>
          </Button>
          {strategy && (
            <button
              type="button"
              className="text-muted-foreground hover:text-foreground mt-1.5 w-full text-left text-[11px] underline-offset-2 hover:underline"
              onClick={() => setParams({ ...strategy.params })}
            >
              reset to defaults
            </button>
          )}
        </section>

        <ResultSection result={result} />
      </aside>

      {/* -------------------------------------------------------- center */}
      <div className="flex min-h-0 min-w-0 flex-col">
        <div className="flex h-9 shrink-0 items-center gap-3 border-b px-3">
          <Segmented value={timeframe} options={catalog.timeframes} onChange={setTimeframe} label="timeframe" />

          <span className="bg-border h-4 w-px" aria-hidden />

          <div className="flex min-w-0 flex-1 items-center gap-2 overflow-x-auto">
            <Select value={indicatorPick} onValueChange={setIndicatorPick}>
              <SelectTrigger size="sm" className="h-6 w-[150px] shrink-0 text-[11px]" aria-label="indicator">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {catalog.indicators.map((i) => (
                  <SelectItem key={i.id} value={i.id}>
                    {i.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button size="sm" variant="secondary" className="h-6 shrink-0 px-2 text-[11px]" onClick={addIndicator}>
              + add
            </Button>
            {indicators.map((entry) => (
              <span
                key={entry.key}
                className="num flex shrink-0 items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px]"
                style={{ borderColor: entry.color, color: entry.color }}
              >
                {entry.key}
                <button
                  type="button"
                  aria-label={`Remove ${entry.key}`}
                  className="hover:text-bear px-0.5"
                  onClick={() => setIndicators((c) => c.filter((i) => i.key !== entry.key))}
                >
                  ×
                </button>
              </span>
            ))}
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            <Toggle checked={showLevels} onChange={setShowLevels} label="levels" disabled={!frame} />
            <Toggle checked={showMarkers} onChange={setShowMarkers} label="markers" />
            <Toggle checked={showZones} onChange={setShowZones} label="stop / target" />
          </div>
        </div>

        <div className="relative min-h-0 flex-1">
          {bars ? (
            <PriceChart
              bars={bars.bars}
              indicators={indicators}
              series={series}
              frame={frame}
              showLevels={showLevels}
              trades={result?.trades ?? []}
              showMarkers={showMarkers}
              showZones={showZones}
              liveBar={liveBar}
            />
          ) : (
            <div className="grid h-full place-items-center">
              <Skeleton className="h-[85%] w-[95%]" />
            </div>
          )}
        </div>

        <div className="text-muted-foreground flex h-7 shrink-0 items-center gap-3 border-t px-3 text-[11px]">
          {bars ? (
            <>
              <LiveBadge status={liveStatus} supported={bars.live} price={liveBar?.close} />
              <span className="num">
                {bars.symbol} · {bars.bars.length.toLocaleString()} × {bars.timeframe} · {day(bars.stats.from)} →{' '}
                {day(bars.stats.to)}
                {bars.stats.gaps > 0 && ` · ${bars.stats.gaps} gaps`}
              </span>
              {bars.synthetic && (
                <span className="text-caution">synthetic — closes only; pick 5m or higher for real ranges</span>
              )}
              <span className="ml-auto hidden xl:inline">
                indicators are computed server-side by the functions the strategies trade on
              </span>
            </>
          ) : (
            <Skeleton className="h-3 w-64" />
          )}
        </div>

        <Tabs
          value={dock}
          onValueChange={(v) => setDock(v as typeof dock)}
          className="h-[248px] shrink-0 gap-0 border-t"
        >
          <TabsList variant="line" className="h-8 w-full shrink-0 justify-start gap-3 rounded-none border-b px-3">
            <TabsTrigger value="leaderboard" className="h-full flex-none px-0 text-xs">
              Leaderboard
            </TabsTrigger>
            <TabsTrigger value="trades" className="h-full flex-none px-0 text-xs">
              Trades
              {result && <span className="text-muted-foreground num ml-1">{result.trades.length}</span>}
            </TabsTrigger>
            <TabsTrigger value="model" className="h-full flex-none px-0 text-xs">
              Fill model
            </TabsTrigger>
            <span className="text-muted-foreground ml-auto truncate text-[11px]">
              {dock === 'leaderboard' &&
                'every method at its defaults on this window — in-sample, a ranking, not a result'}
              {dock === 'trades' && result && `${result.name} · ${result.timeframe} · newest first`}
            </span>
          </TabsList>
          <TabsContent value="leaderboard" className="min-h-0 flex-1 overflow-auto">
            <LeaderboardTable rows={leaderboard} selected={strategyId} onPick={setStrategyId} />
          </TabsContent>
          <TabsContent value="trades" className="min-h-0 flex-1 overflow-auto">
            <TradesTable result={result} />
          </TabsContent>
          <TabsContent value="model" className="min-h-0 flex-1 overflow-auto">
            <FillModel result={result} note={catalog.fillModel} />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  )
}

/* ------------------------------------------------------------------ rail */

function RailHeading({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="text-muted-foreground mb-2 text-[11px] font-semibold tracking-wide uppercase">{children}</h2>
  )
}

function ResultSection({ result }: { result: BacktestResult | null }) {
  if (!result) {
    return (
      <section className="px-3 pt-3 pb-3">
        <RailHeading>Result</RailHeading>
        <p className="text-muted-foreground text-[11px] leading-snug">
          No run yet. Set the parameters and press <span className="num text-primary">Run backtest</span>; the
          trades land on the chart and in the dock.
        </p>
      </section>
    )
  }

  const m = result.metrics
  const rows: [string, string, string][] = [
    ['expectancy', `${num(m.expectancy)} R`, signClass(m.expectancy)],
    ['avg R', num(m.avgR), signClass(m.avgR)],
    ['total R', num(m.totalR), signClass(m.totalR)],
    ['net P&L', money(m.netPnlUsd), signClass(m.netPnlUsd)],
    ['return', `${num(m.returnPct)}%`, signClass(m.returnPct)],
    ['max drawdown', money(m.maxDrawdownUsd), ''],
    ['sharpe', num(m.sharpe), ''],
    ['avg hold', `${num(m.avgHoldMin, 0)} min`, ''],
    ['avg MAE / MFE', `${num(m.avgMae)} / ${num(m.avgMfe)} R`, ''],
  ]
  const exits = Object.entries(m.exits ?? {})

  return (
    <section className="px-3 pt-3 pb-3">
      <div className="flex items-baseline justify-between">
        <RailHeading>Result</RailHeading>
        <span className="text-muted-foreground num mb-2 text-[11px]">{result.timeframe}</span>
      </div>

      <div className="grid grid-cols-3 gap-2">
        <Stat label="trades" value={String(m.trades)} caution={m.trades < 30 ? '< 30' : undefined} />
        <Stat label="win rate" value={pct(m.winRate)} />
        <Stat label="PF" value={num(m.profitFactor)} tone={signClass(m.profitFactor - 1)} />
      </div>

      <dl className="mt-3 grid grid-cols-[1fr_auto] gap-x-3 gap-y-[3px] text-xs">
        {rows.map(([label, value, tone]) => (
          <div key={label} className="contents">
            <dt className="text-muted-foreground">{label}</dt>
            <dd className={cn('num text-right', tone)}>{value}</dd>
          </div>
        ))}
      </dl>

      {exits.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1">
          {exits.map(([kind, count]) => (
            <span key={kind} className="num rounded-sm border px-1.5 py-0.5 text-[10px]">
              {kind.toLowerCase()} <span className="text-muted-foreground">{count}</span>
            </span>
          ))}
        </div>
      )}

      <p
        className={cn(
          'mt-3 rounded-md border px-2.5 py-2 text-[11px] leading-snug',
          result.verdict.promising ? 'border-bull/40 bg-bull/10' : 'border-caution/40 bg-caution/10',
        )}
      >
        {result.verdict.promising
          ? 'Promising in-sample. That is a ranking, not a result — confirm with a walk-forward and a null.'
          : result.verdict.reasons.join('; ')}
      </p>
    </section>
  )
}

function Stat({ label, value, tone, caution }: { label: string; value: string; tone?: string; caution?: string }) {
  return (
    <div className="min-w-0">
      <span className="text-muted-foreground block text-[10px] tracking-[0.07em] uppercase">{label}</span>
      <span className={cn('num block truncate text-[19px] leading-tight font-semibold', tone)}>{value}</span>
      {caution && <span className="text-caution num block text-[10px]">{caution}</span>}
    </div>
  )
}

/* --------------------------------------------------------------- toolbar */

function Segmented({
  value,
  options,
  onChange,
  label,
}: {
  value: string
  options: string[]
  onChange: (value: string) => void
  label: string
}) {
  return (
    <div role="radiogroup" aria-label={label} className="bg-background flex shrink-0 rounded-md border p-[2px]">
      {options.map((option) => (
        <button
          key={option}
          type="button"
          role="radio"
          aria-checked={option === value}
          onClick={() => onChange(option)}
          className={cn(
            'num focus-visible:ring-ring/50 h-5 rounded-[4px] px-2 text-[11px] transition-colors focus-visible:ring-[3px] focus-visible:outline-none',
            option === value ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground',
          )}
        >
          {option}
        </button>
      ))}
    </div>
  )
}

function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean
  onChange: (value: boolean) => void
  label: string
  disabled?: boolean
}) {
  return (
    <label
      className={cn(
        'flex cursor-pointer items-center gap-1.5 rounded-md border px-2 py-[3px] text-[11px] transition-colors',
        checked && !disabled ? 'border-primary/50 text-foreground bg-primary/10' : 'text-muted-foreground',
        disabled && 'cursor-not-allowed opacity-50',
      )}
      title={disabled ? 'No options tape for this market' : undefined}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="sr-only"
      />
      <span className={cn('size-2 rounded-[2px]', checked && !disabled ? 'bg-primary' : 'bg-muted-foreground/40')} />
      {label}
    </label>
  )
}

/**
 * Says which of three states the chart is actually in. A market with no
 * upstream websocket reads "snapshot" rather than wearing a live badge that can
 * never light up — the previous dashboard polled a static file every thirty
 * seconds and looked alive while nothing moved.
 */
function LiveBadge({ status, supported, price: lastPrice }: { status: LiveStatus; supported: boolean; price?: number }) {
  if (!supported) {
    return <span className="text-muted-foreground rounded-full border px-2 py-px text-[10px]">snapshot · no stream</span>
  }

  const tone =
    status === 'live'
      ? 'border-bull/50 text-bull bg-bull/10'
      : status === 'connecting'
        ? 'border-caution/50 text-caution bg-caution/10'
        : 'border-bear/50 text-bear bg-bear/10'
  const label = status === 'live' ? 'live' : status === 'connecting' ? 'connecting' : 'reconnecting'

  return (
    <span className={cn('flex items-center gap-1.5 rounded-full border px-2 py-px text-[10px]', tone)}>
      <span className={cn('size-1.5 rounded-full bg-current', status === 'live' && 'animate-pulse')} aria-hidden />
      {label}
      {status === 'live' && lastPrice != null && <span className="num">{price(lastPrice)}</span>}
    </span>
  )
}

/* ------------------------------------------------------------------ dock */

const LEADERBOARD_COLS = 'grid-cols-[minmax(160px,1.2fr)_64px_64px_72px_80px_96px_96px_minmax(120px,1fr)]'

function LeaderboardTable({
  rows,
  selected,
  onPick,
}: {
  rows: LeaderboardRow[] | null
  selected?: string
  onPick: (id: string) => void
}) {
  return (
    <div className="min-w-[760px]">
      <div
        className={cn(
          'text-muted-foreground bg-background sticky top-0 z-10 grid items-center gap-3 border-b px-3 py-1 text-[10px] tracking-wide uppercase',
          LEADERBOARD_COLS,
        )}
      >
        <span>strategy</span>
        <Th>trades</Th>
        <Th>win</Th>
        <Th>PF</Th>
        <Th>expect</Th>
        <Th>net P&L</Th>
        <Th>max DD</Th>
        <span>note</span>
      </div>
      {!rows ? (
        <div className="space-y-2 px-3 py-3">
          {[0, 1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-3 w-full" />
          ))}
        </div>
      ) : (
        rows.map((row) => {
          const m = row.metrics
          const thin = (m?.trades ?? 0) > 0 && (m?.trades ?? 0) < 30
          return (
            <button
              key={row.id}
              type="button"
              disabled={Boolean(row.skipped)}
              onClick={() => onPick(row.id)}
              aria-selected={row.id === selected}
              className={cn(
                'hover:bg-accent/60 grid w-full items-center gap-3 border-b px-3 py-[5px] text-left text-xs transition-colors last:border-0',
                LEADERBOARD_COLS,
                row.id === selected && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
                row.skipped && 'cursor-default opacity-60',
              )}
            >
              <span className="truncate">
                {row.name}
                <span className="text-muted-foreground num ml-2 text-[10px]">{row.id}</span>
              </span>
              {row.skipped || !m ? (
                <span className="text-muted-foreground col-span-7 truncate text-[11px]">
                  {row.skipped ?? 'no result'}
                </span>
              ) : (
                <>
                  <Td className={cn(thin && 'text-caution')}>{m.trades}</Td>
                  <Td>{pct(m.winRate)}</Td>
                  <Td className={signClass(m.profitFactor - 1)}>{num(m.profitFactor)}</Td>
                  <Td className={signClass(m.expectancy)}>{num(m.expectancy)} R</Td>
                  <Td className={signClass(m.netPnlUsd)}>{money(m.netPnlUsd)}</Td>
                  <Td>{money(m.maxDrawdownUsd)}</Td>
                  <span className="text-muted-foreground truncate text-[11px]">
                    {thin ? 'too few trades to read' : m.profitFactor >= 1.2 ? 'clears the gate in-sample only' : ''}
                  </span>
                </>
              )}
            </button>
          )
        })
      )}
    </div>
  )
}

const TRADE_COLS = 'grid-cols-[110px_56px_96px_96px_64px_80px_120px_minmax(160px,1fr)]'

function TradesTable({ result }: { result: BacktestResult | null }) {
  const trades = result?.trades ?? []
  if (trades.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-4 text-sm">
        {result
          ? 'The strategy never triggered on this window.'
          : 'Run a backtest to see its fills here and on the chart.'}
      </p>
    )
  }
  return (
    <div className="min-w-[820px]">
      <div
        className={cn(
          'text-muted-foreground bg-background sticky top-0 z-10 grid items-center gap-3 border-b px-3 py-1 text-[10px] tracking-wide uppercase',
          TRADE_COLS,
        )}
      >
        <span>entry</span>
        <span>side</span>
        <Th>entry px</Th>
        <Th>exit px</Th>
        <Th>hold</Th>
        <Th>R</Th>
        <Th>P&L</Th>
        <span>exit · reason</span>
      </div>
      {[...trades].reverse().map((trade, index) => (
        <div
          key={`${trade.entryTime}-${index}`}
          className={cn('grid items-center gap-3 border-b px-3 py-[4px] text-xs last:border-0', TRADE_COLS)}
        >
          <span className="num text-muted-foreground">{stampShort(trade.entryTime)}</span>
          <span className={cn('num', trade.direction === 'LONG' ? 'text-bull' : 'text-bear')}>{trade.direction}</span>
          <Td>{price(trade.entryPrice)}</Td>
          <Td>{price(trade.exitPrice)}</Td>
          <Td className="text-muted-foreground">{holdLabel(trade.holdMs)}</Td>
          <Td className={signClass(trade.pnlUsd)}>{num(trade.r)}</Td>
          <Td className={signClass(trade.pnlUsd)}>{money(trade.pnlUsd)}</Td>
          <span className="text-muted-foreground truncate text-[11px]" title={trade.reason}>
            <span className="text-foreground/80">{trade.exitReason.toLowerCase()}</span> · {trade.reason}
          </span>
        </div>
      ))}
    </div>
  )
}

function FillModel({ result, note }: { result: BacktestResult | null; note?: string }) {
  return (
    <div className="grid gap-6 px-3 py-3 text-[11px] leading-relaxed md:grid-cols-2">
      <div className="max-w-[64ch]">
        <p>
          <strong className="text-foreground/80">How fills are modelled.</strong>{' '}
          {note ??
            "A signal on one bar fills at the next bar's open, never the close that produced it. Half the spread is charged each side. When a single bar covers both stop and target the stop is taken, and a gap through the stop fills at the open — the pessimistic reading, because OHLC cannot say what happened first inside a bar."}
        </p>
        <p className="text-muted-foreground mt-2">
          Every method runs through this one path, so the leaderboard compares methods rather than accidental
          differences in fill assumptions. An in-sample number is a ranking; only a walk-forward read against a
          null is a result.
        </p>
      </div>
      {result && (
        <dl className="num grid h-fit grid-cols-[auto_1fr] gap-x-4 gap-y-[3px]">
          <dt className="text-muted-foreground">run</dt>
          <dd>
            {result.name} · {result.market} · {result.timeframe}
          </dd>
          {Object.entries(result.params).map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-muted-foreground">{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
          {result.indicatorSpecs.length > 0 && (
            <>
              <dt className="text-muted-foreground">indicators</dt>
              <dd className="text-muted-foreground">
                {result.indicatorSpecs
                  .map((s) => `${s.id}${s.params ? `(${Object.values(s.params).join(', ')})` : ''}`)
                  .join(' · ')}
              </dd>
            </>
          )}
        </dl>
      )}
    </div>
  )
}

function Th({ children }: { children: React.ReactNode }) {
  return <span className="text-right">{children}</span>
}

function Td({ children, className }: { children: React.ReactNode; className?: string }) {
  return <span className={cn('num text-right', className)}>{children}</span>
}

function stampShort(ms: number): string {
  return new Date(ms).toISOString().slice(5, 16).replace('T', ' ')
}

function holdLabel(ms: number): string {
  const minutes = Math.round(ms / 60_000)
  if (minutes < 90) return `${minutes}m`
  const hours = minutes / 60
  return hours < 48 ? `${num(hours, 1)}h` : `${num(hours / 24, 1)}d`
}
