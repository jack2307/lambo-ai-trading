import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'

import { PriceChart, type ActiveIndicator } from '@/components/PriceChart'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
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

/** One accent plus the data colours; indicator lines cycle through these. */
const LINE_COLORS = ['#7d94e8', '#d99446', '#3fbcc0', '#e05d6a', '#46c98a', '#b48ae0', '#c9d1de']

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
    if (!market || !strategyId) return
    setRunning(true)
    try {
      const outcome = await api.backtest(market, timeframe, strategyId, params)
      setResult(outcome)
      toast(`${outcome.name}: ${outcome.metrics.trades} trades`, {
        description: outcome.verdict.promising
          ? 'Promising in-sample — confirm with a walk-forward.'
          : outcome.verdict.reasons[0],
      })
    } catch (err) {
      onError((err as Error).message)
    } finally {
      setRunning(false)
    }
  }, [market, timeframe, strategyId, params, onError])

  return (
    <div className="space-y-3 p-3">
      <Card className="flex flex-wrap items-center gap-3 rounded-lg px-3 py-2">
        <Field label="timeframe">
          <Select value={timeframe} onValueChange={setTimeframe}>
            <SelectTrigger size="sm" className="h-7 w-[86px] text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {catalog.timeframes.map((tf) => (
                <SelectItem key={tf} value={tf}>
                  {tf}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <Field label="strategy">
          <Select value={strategyId} onValueChange={setStrategyId}>
            <SelectTrigger size="sm" className="h-7 w-[210px] text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {catalog.strategies.map((s) => (
                <SelectItem key={s.id} value={s.id}>
                  {s.name}
                  {s.needsOptions ? ' ·⚙' : ''}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <div className="flex flex-wrap items-center gap-2">
          {Object.entries(params).map(([key, value]) => (
            <label key={key} className="text-muted-foreground flex items-center gap-1 text-[11px]">
              {key}
              <input
                type="number"
                step="any"
                value={value}
                onChange={(e) =>
                  setParams((current) => ({ ...current, [key]: Number(e.target.value) }))
                }
                className="border-input bg-background num h-7 w-[68px] rounded-md border px-2 text-xs"
              />
            </label>
          ))}
        </div>

        <Button size="sm" className="h-7 text-xs" disabled={running} onClick={runBacktest}>
          {running ? 'Running…' : 'Run backtest'}
        </Button>

        <div className="ml-auto flex items-center gap-3">
          <Toggle checked={showLevels} onChange={setShowLevels} label="option levels" />
          <Toggle checked={showMarkers} onChange={setShowMarkers} label="trade markers" />
          <Toggle checked={showZones} onChange={setShowZones} label="stop / target" />
        </div>
      </Card>

      <Card className="flex flex-wrap items-center gap-3 rounded-lg px-3 py-2">
        <Field label="indicator">
          <Select value={indicatorPick} onValueChange={setIndicatorPick}>
            <SelectTrigger size="sm" className="h-7 w-[180px] text-xs">
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
        </Field>
        <Button size="sm" variant="secondary" className="h-7 text-xs" onClick={addIndicator}>
          Add
        </Button>

        <div className="flex flex-wrap gap-2">
          {indicators.map((entry) => (
            <Badge
              key={entry.key}
              variant="outline"
              className="num gap-1 text-[11px]"
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
            </Badge>
          ))}
        </div>

        <p className="text-muted-foreground ml-auto text-[11px]">
          {bars?.synthetic
            ? 'This timeframe is synthetic — the feed gives closes only. Pick 5m or higher.'
            : 'Indicators are computed server-side by the same functions the strategies trade on.'}
        </p>
      </Card>

      <div className="grid gap-3 xl:grid-cols-[minmax(520px,1fr)_360px]">
        <Card className="relative h-[calc(100vh-260px)] min-h-[440px] overflow-hidden rounded-lg p-0">
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
          {bars && (
            <div className="pointer-events-none absolute right-3 bottom-2 flex items-center gap-3">
              <LiveBadge status={liveStatus} supported={bars.live} price={liveBar?.close} />
              <p className="text-muted-foreground num text-[11px]">
                {bars.market} · {bars.symbol} · {bars.bars.length} × {bars.timeframe} ·{' '}
                {day(bars.stats.from)} → {day(bars.stats.to)}
              </p>
            </div>
          )}
        </Card>

        <div className="space-y-3">
          <ResultCard result={result} />
          <LeaderboardCard
            rows={leaderboard}
            selected={result?.strategy}
            onPick={(id) => setStrategyId(id)}
          />
          <TradesCard result={result} />
        </div>
      </div>

      <p className="text-muted-foreground max-w-[92ch] px-1 text-[11px] leading-relaxed">
        <strong className="text-foreground/80">How fills are modelled.</strong> A signal on one bar fills at the{' '}
        <em>next</em> bar's open, never the close that produced it. Half the spread is charged each side. When a single
        bar covers both stop and target the stop is taken, and a gap through the stop fills at the open — the
        pessimistic reading, because OHLC cannot say what happened first inside a bar.
      </p>
    </div>
  )
}

/**
 * Says which of three states the chart is actually in. A market with no
 * upstream websocket reads "snapshot" rather than wearing a live badge that can
 * never light up — the previous dashboard polled a static file every thirty
 * seconds and looked alive while nothing moved.
 */
function LiveBadge({
  status,
  supported,
  price: lastPrice,
}: {
  status: LiveStatus
  supported: boolean
  price?: number
}) {
  if (!supported) {
    return (
      <span className="text-muted-foreground rounded-full border px-2 py-0.5 text-[10px]">
        snapshot · this feed has no stream
      </span>
    )
  }

  const tone =
    status === 'live'
      ? 'border-bull/50 text-bull bg-bull/10'
      : status === 'connecting'
        ? 'border-caution/50 text-caution bg-caution/10'
        : 'border-bear/50 text-bear bg-bear/10'

  const label = status === 'live' ? 'live' : status === 'connecting' ? 'connecting' : 'reconnecting'

  return (
    <span className={cn('flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10px]', tone)}>
      <span
        className={cn('size-1.5 rounded-full bg-current', status === 'live' && 'animate-pulse')}
        aria-hidden
      />
      {label}
      {status === 'live' && lastPrice != null && <span className="num">{price(lastPrice)}</span>}
    </span>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="text-muted-foreground flex items-center gap-2 text-xs">
      {label}
      {children}
    </label>
  )
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (value: boolean) => void
  label: string
}) {
  return (
    <label
      className={cn(
        'flex cursor-pointer items-center gap-2 rounded-md border px-2 py-1 text-[11px] transition-colors',
        checked ? 'border-primary/50 text-foreground bg-primary/10' : 'text-muted-foreground',
      )}
    >
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="sr-only" />
      <span className={cn('size-2 rounded-[2px]', checked ? 'bg-primary' : 'bg-muted-foreground/40')} />
      {label}
    </label>
  )
}

function ResultCard({ result }: { result: BacktestResult | null }) {
  if (!result) {
    return (
      <Card className="rounded-lg p-4">
        <h2 className="text-muted-foreground mb-2 text-xs font-semibold">Result</h2>
        <p className="text-muted-foreground text-sm">
          No run yet. Pick a strategy and press <span className="num text-primary">Run backtest</span>.
        </p>
      </Card>
    )
  }

  const m = result.metrics
  const rows: [string, string, string][] = [
    ['avg R', num(m.avgR), signClass(m.avgR)],
    ['total R', num(m.totalR), signClass(m.totalR)],
    ['return', `${num(m.returnPct)}%`, signClass(m.returnPct)],
    ['max drawdown', money(m.maxDrawdownUsd), ''],
    ['sharpe', num(m.sharpe), ''],
    ['avg hold', `${num(m.avgHoldMin)} min`, ''],
    ['exits', Object.entries(m.exits ?? {}).map(([k, v]) => `${k.toLowerCase()} ${v}`).join(', ') || '—', ''],
  ]

  return (
    <Card className="rounded-lg p-0">
      <div className="flex items-center justify-between border-b px-4 py-2">
        <h2 className="text-muted-foreground text-xs font-semibold">Result</h2>
        <span className="text-muted-foreground num text-[11px]">
          {result.strategy} · {result.timeframe}
        </span>
      </div>

      <div className="grid grid-cols-3 gap-3 px-4 py-3">
        <Stat label="trades" value={String(m.trades)} />
        <Stat label="win rate" value={pct(m.winRate)} />
        <Stat label="profit factor" value={num(m.profitFactor)} tone={signClass(m.profitFactor - 1)} />
      </div>

      <dl className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5 px-4 pb-3 text-sm">
        {rows.map(([label, value, tone]) => (
          <div key={label} className="contents">
            <dt className="text-muted-foreground">{label}</dt>
            <dd className={cn('num text-right', tone)}>{value}</dd>
          </div>
        ))}
      </dl>

      <p
        className={cn(
          'mx-4 mb-4 rounded-md border px-3 py-2 text-[11px] leading-relaxed',
          result.verdict.promising ? 'border-bull/40 bg-bull/10' : 'border-caution/40 bg-caution/10',
        )}
      >
        {result.verdict.promising
          ? '✔ Promising in-sample. Confirm with a walk-forward before believing it.'
          : `✖ ${result.verdict.reasons.join('; ')}`}
      </p>
    </Card>
  )
}

function Stat({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div>
      <span className="text-muted-foreground block text-[10px] tracking-[0.07em] uppercase">{label}</span>
      <span className={cn('num block text-[19px] leading-tight font-semibold', tone)}>{value}</span>
    </div>
  )
}

function LeaderboardCard({
  rows,
  selected,
  onPick,
}: {
  rows: LeaderboardRow[] | null
  selected?: string
  onPick: (id: string) => void
}) {
  return (
    <Card className="rounded-lg p-0">
      <div className="border-b px-4 py-2">
        <h2 className="text-muted-foreground text-xs font-semibold">Leaderboard</h2>
      </div>
      <ScrollArea className="max-h-[240px]">
        {!rows ? (
          <div className="space-y-2 p-4">
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-2/3" />
          </div>
        ) : (
          <div>
            {rows.map((row) => (
              <button
                key={row.id}
                type="button"
                disabled={Boolean(row.skipped)}
                onClick={() => onPick(row.id)}
                aria-selected={row.id === selected}
                className={cn(
                  'hover:bg-accent grid w-full grid-cols-[1fr_34px_52px_64px] items-center gap-2 border-b px-4 py-1.5 text-left text-xs transition-colors last:border-0',
                  row.id === selected && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
                  row.skipped && 'cursor-default opacity-60',
                )}
              >
                <span className="truncate">{row.id}</span>
                {row.skipped ? (
                  <span className="text-muted-foreground col-span-3 truncate text-[11px]">{row.skipped}</span>
                ) : (
                  <>
                    <span className="num text-muted-foreground text-right">{row.metrics?.trades}</span>
                    <span className={cn('num text-right', signClass((row.metrics?.profitFactor ?? 0) - 1))}>
                      {num(row.metrics?.profitFactor)}
                    </span>
                    <span className={cn('num text-right', signClass(row.metrics?.netPnlUsd))}>
                      {money(row.metrics?.netPnlUsd)}
                    </span>
                  </>
                )}
              </button>
            ))}
          </div>
        )}
      </ScrollArea>
    </Card>
  )
}

function TradesCard({ result }: { result: BacktestResult | null }) {
  const trades = result?.trades ?? []
  return (
    <Card className="rounded-lg p-0">
      <div className="flex items-center justify-between border-b px-4 py-2">
        <h2 className="text-muted-foreground text-xs font-semibold">Trades</h2>
        {trades.length > 0 && <span className="text-muted-foreground text-[11px]">{trades.length} fills</span>}
      </div>
      <ScrollArea className="max-h-[240px]">
        {trades.length === 0 ? (
          <p className="text-muted-foreground p-4 text-sm">
            {result ? 'The strategy never triggered on this window.' : 'Run a backtest to see fills.'}
          </p>
        ) : (
          <div>
            {[...trades].reverse().map((trade, index) => (
              <div
                key={`${trade.entryTime}-${index}`}
                title={trade.reason}
                className="grid grid-cols-[1fr_48px_64px] items-center gap-2 border-b px-4 py-1.5 text-xs last:border-0"
              >
                <span className="text-muted-foreground truncate text-[11px]">
                  {new Date(trade.entryTime).toISOString().slice(5, 16).replace('T', ' ')} · {trade.direction} ·{' '}
                  {trade.exitReason.toLowerCase()}
                </span>
                <span className={cn('num text-right', signClass(trade.pnlUsd))}>{trade.r}R</span>
                <span className={cn('num text-right', signClass(trade.pnlUsd))}>{money(trade.pnlUsd)}</span>
              </div>
            ))}
          </div>
        )}
      </ScrollArea>
    </Card>
  )
}

