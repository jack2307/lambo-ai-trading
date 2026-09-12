import { Card } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { num, pct, price, stamp } from '@/lib/format'
import { useLiveLevels, type LevelsSource } from '@/lib/useLiveLevels'
import { cn } from '@/lib/utils'

interface Props {
  market: string
  onError: (message: string) => void
}

export function Tape({ market }: Props) {
  const { frame, source, prints, lastPrintAt } = useLiveLevels(market)
  const loading = source === 'loading' && !frame

  if (loading) {
    return (
      <div className="space-y-3 p-3">
        <div className="flex gap-3">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-[168px] w-[230px]" />
          ))}
        </div>
        <Skeleton className="h-[240px] w-full" />
      </div>
    )
  }

  if (!frame) {
    return (
      <Card className="m-3 rounded-lg p-8 text-center">
        <p className="font-medium">No options tape for this market</p>
        <p className="text-muted-foreground mt-1 text-sm">
          Fetch one first: <span className="num text-primary">gof fetch --market={market || '<id>'}</span>
        </p>
      </Card>
    )
  }

  return (
    <div className="space-y-3 p-3">
      <Card className="flex flex-wrap items-center gap-6 rounded-lg px-4 py-3">
        <div className="flex items-baseline gap-3">
          <span className="text-muted-foreground text-[10px] tracking-[0.08em] uppercase">spot</span>
          <span className="num text-[19px] leading-none font-semibold">{price(frame.spot)}</span>
          <time className="text-muted-foreground num text-[11px]">{stamp(frame.t)}</time>
        </div>

        <FlowMeter label="bull ratio" value={frame.bullRatio} />
        <FlowMeter label="15m" value={frame.bullRatio15m} />

        <div className="text-muted-foreground num text-[11px]">
          flow velocity {num(frame.netFlowVelocityNorm)} · big-print imbalance {num(frame.bigTradeImbalance)}
        </div>

        <LevelsBadge source={source} prints={prints} lastPrintAt={lastPrintAt} />
      </Card>

      <section aria-label="Expirations" className="flex gap-3 overflow-x-auto pb-1">
        {frame.contexts.map((ctx) => (
          <Card key={ctx.symbol} className="min-w-[236px] shrink-0 rounded-lg p-3">
            <header className="mb-2 flex items-baseline justify-between border-b pb-2">
              <span className="num font-semibold">{ctx.symbol}</span>
              <span className="text-muted-foreground text-[10px] tracking-[0.06em] uppercase">
                {num(ctx.dte, 1)}d
              </span>
            </header>

            <dl className="grid grid-cols-[1fr_auto] gap-x-3 gap-y-px text-xs">
              <Row label="Max pain" value={price(ctx.maxPain)} />
              <Row label="POC" value={price(ctx.poc)} />
              <Row label="Call BE" value={price(ctx.callBE)} experimental />
              <Row label="Put BE" value={price(ctx.putBE)} experimental />
              <Row label="wSup / wRes" value={`${price(ctx.wSup)} / ${price(ctx.wRes)}`} experimental />
            </dl>

            <div className="mt-2 flex items-center gap-2 border-t pt-2">
              <span className="bg-bear/40 h-1 flex-1 overflow-hidden rounded-full">
                <span className="bg-bull block h-full" style={{ width: `${ctx.bullRatio * 100}%` }} />
              </span>
              <span className="text-muted-foreground num text-[11px]">{pct(ctx.bullRatio)} bull</span>
            </div>
          </Card>
        ))}
      </section>

      <Card className="rounded-lg p-0">
        <div className="flex items-center justify-between border-b px-4 py-2">
          <h2 className="text-muted-foreground text-xs font-semibold">Confluence clusters</h2>
          <span className="text-muted-foreground text-[11px]">
            score = level weight × type diversity × expiry diversity
          </span>
        </div>
        <ScrollArea className="max-h-[320px]">
          {frame.clusters.length === 0 ? (
            <p className="text-muted-foreground p-4 text-sm">
              No clusters yet — they need at least two derived levels near one price.
            </p>
          ) : (
            frame.clusters.map((cluster, index) => {
              const support = cluster.center < frame.spot
              return (
                <div
                  key={`${cluster.low}-${index}`}
                  className="grid grid-cols-[150px_110px_1fr] items-center gap-3 border-b px-4 py-1.5 text-xs last:border-0"
                >
                  <span className="num">
                    {price(cluster.low)} – {price(cluster.high)}
                  </span>
                  <span>
                    <b className={cn(support ? 'text-bull' : 'text-bear')}>
                      {support ? 'support' : 'resistance'}
                    </b>{' '}
                    <span className="num text-muted-foreground">{num(cluster.score, 1)}</span>
                  </span>
                  <span className="text-muted-foreground text-[11px]">
                    {cluster.types} level type{cluster.types === 1 ? '' : 's'} · {cluster.expirations} expir
                    {cluster.expirations === 1 ? 'y' : 'ies'}
                  </span>
                </div>
              )
            })
          )}
        </ScrollArea>
      </Card>

      <p className="text-muted-foreground max-w-[92ch] px-1 text-[11px] leading-relaxed">
        <strong className="text-foreground/80">Read these as structure, not prediction.</strong> Levels come from the
        option tape alone. Neither feed publishes open interest, so max pain here is a positioning proxy rather than OI
        max pain, and LC/LP/SC/SP describe which side aggressed a print — not whether anyone opened a position. Levels
        marked ⚠ use formulas that have never been reproduced.
      </p>
    </div>
  )
}

/**
 * Where the numbers on this screen came from, and whether the tape is moving.
 *
 * An options tape is not a price tape: a BTC board can go a minute without a
 * single print, and gold's feed has no stream at all. A level that has not
 * changed usually means nothing traded — the print counter is what separates
 * that from a broken feed, so it is shown rather than assumed.
 */
function LevelsBadge({
  source,
  prints,
  lastPrintAt,
}: {
  source: LevelsSource
  prints: number
  lastPrintAt: number | null
}) {
  const tone =
    source === 'live'
      ? 'border-bull/50 text-bull bg-bull/10'
      : source === 'offline'
        ? 'border-bear/50 text-bear bg-bear/10'
        : 'text-muted-foreground'

  const label =
    source === 'live' ? 'live' : source === 'snapshot' ? 'snapshot' : source === 'offline' ? 'offline' : 'loading'

  const quiet = source === 'live' && lastPrintAt != null && Date.now() - lastPrintAt > 60_000

  return (
    <span className={cn('ml-auto flex items-center gap-2 rounded-full border px-2 py-0.5 text-[10px]', tone)}>
      <span className={cn('size-1.5 rounded-full bg-current', source === 'live' && !quiet && 'animate-pulse')} aria-hidden />
      {label}
      {source === 'live' && (
        <span className="num text-muted-foreground">
          {prints} print{prints === 1 ? '' : 's'}
          {quiet ? ' · tape quiet' : ''}
        </span>
      )}
      {source === 'snapshot' && <span className="text-muted-foreground">this feed has no options stream</span>}
    </span>
  )
}

function Row({ label, value, experimental }: { label: string; value: string; experimental?: boolean }) {
  return (
    <div className="contents">
      <dt className={cn('text-muted-foreground', experimental && 'text-caution')}>
        {experimental ? '⚠ ' : ''}
        {label}
      </dt>
      <dd className="num text-right">{value}</dd>
    </div>
  )
}

function FlowMeter({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex min-w-[180px] items-center gap-2">
      <span className="text-muted-foreground text-[10px] tracking-[0.08em] uppercase">{label}</span>
      <span className="bg-bear/40 h-1.5 flex-1 overflow-hidden rounded-full">
        <span className="bg-bull block h-full transition-all" style={{ width: `${value * 100}%` }} />
      </span>
      <span className="num text-muted-foreground text-[11px]">{pct(value)}</span>
    </div>
  )
}
