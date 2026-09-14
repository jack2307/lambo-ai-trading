import type { BacktestTrade, PaperRun } from '@/lib/api'

import { Figure, Label, Panel, Pill } from './primitives'

/**
 * The paper desk: what the paper loop holds and has done, read from
 * `/api/paper/status` every few seconds.
 *
 * This is the only thing in the project that "executes", and it executes
 * into a JSON file. The panel shows the same numbers the receipts show — net,
 * profit factor, the guard counters — because the fills are read by the same
 * reviewers, and it says plainly when no bar has arrived for a while.
 */

const fmtTime = (ms: number | null | undefined) => {
  if (!ms) return '—'
  const d = new Date(ms)
  return `${d.toISOString().slice(0, 10)} ${d.toISOString().slice(11, 16)}Z`
}
const usd = (v: number) => `${v < 0 ? '−' : '+'}$${Math.abs(v).toFixed(0)}`
const ago = (ms: number | null | undefined, now: number) => {
  if (!ms) return '—'
  const m = Math.max(0, Math.round((now - ms) / 60_000))
  return m < 60 ? `${m} min ago` : m < 48 * 60 ? `${(m / 60).toFixed(1)} h ago` : `${Math.round(m / 1440)} d ago`
}

function Counters({ label, map }: { label: string; map: Record<string, number> }) {
  const entries = Object.entries(map).filter(([, n]) => n > 0)
  if (entries.length === 0) return null
  return (
    <span className="text-muted-foreground text-[11px]">
      {label}{' '}
      {entries.map(([k, n]) => (
        <span key={k} className="num text-foreground/80 mr-2">
          {k.toLowerCase().replace(/_/g, ' ')} ×{n}
        </span>
      ))}
    </span>
  )
}

function Fill({ t }: { t: BacktestTrade }) {
  return (
    <li className="grid grid-cols-[92px_44px_1fr_auto] items-baseline gap-2 text-[11px]">
      <span className="num text-muted-foreground">{fmtTime(t.exitTime)}</span>
      <span className={t.direction === 'LONG' ? 'text-lc' : 'text-lp'}>{t.direction.toLowerCase()}</span>
      <span className="text-muted-foreground truncate">{t.exitReason.toLowerCase().replace(/_/g, ' ')}</span>
      <Figure value={usd(t.pnlUsd)} unit={`${t.r >= 0 ? '+' : ''}${t.r.toFixed(2)}R`} tone={t.pnlUsd >= 0 ? 'up' : 'down'} />
    </li>
  )
}

export function PaperPanel({ runs, now, error }: { runs: PaperRun[] | null; now: number; error?: string | null }) {
  if (error) {
    return (
      <Panel className="p-5">
        <Label>Paper desk</Label>
        <p className="text-muted-foreground mt-2 text-[13px]">Could not read the paper status: {error}</p>
      </Panel>
    )
  }
  if (!runs) {
    return (
      <Panel className="p-5">
        <Label>Paper desk</Label>
        <p className="text-muted-foreground mt-2 text-[13px]">Reading the paper book…</p>
      </Panel>
    )
  }
  if (runs.length === 0) {
    return (
      <Panel className="p-5">
        <Label>Paper desk</Label>
        <p className="text-muted-foreground mt-2 text-[13px]">
          No paper run. Start one with <span className="num text-primary">POST /api/paper/start</span> and feed it closed bars
          (<span className="num">py/live/mt5_bars.py</span> for the broker&apos;s gold; read-only towards the terminal).
        </p>
      </Panel>
    )
  }
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      {runs.map((run) => {
        const stale = run.last_bar_time ? now - run.last_bar_time > 45 * 60_000 : true
        const pf = run.profit_factor
        return (
          <Panel key={run.id} className="p-5">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <Label>Paper desk</Label>
                <Pill tone={stale ? 'caution' : 'live'}>{stale ? 'no bar for a while' : 'fed'}</Pill>
                {run.guards ? <Pill tone="neutral">guards on</Pill> : <Pill tone="blocked">guards off</Pill>}
              </div>
              <span className="text-muted-foreground num text-[11px]">
                {run.market}:{run.tf} · {run.strategy} · last bar {ago(run.last_bar_time, now)}
              </span>
            </div>

            <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-[12px] sm:grid-cols-4">
              <div>
                <Label>Equity</Label>
                <Figure value={`$${run.equity.toFixed(0)}`} className="text-[15px]" />
              </div>
              <div>
                <Label>Net</Label>
                <Figure value={usd(run.net_usd)} tone={run.net_usd > 0 ? 'up' : run.net_usd < 0 ? 'down' : 'neutral'} className="text-[15px]" />
              </div>
              <div>
                <Label>Trades</Label>
                <Figure value={String(run.trades)} unit={pf == null ? '' : `PF ${pf.toFixed(2)}`} className="text-[15px]" />
              </div>
              <div>
                <Label>Bars</Label>
                <Figure value={String(run.bars_seen)} unit={run.gaps ? `${run.gaps} gap${run.gaps > 1 ? 's' : ''}` : 'no gaps'} className="text-[15px]" />
              </div>
            </div>

            <div className="mt-3 text-[12px]">
              <Label>Open position</Label>
              {run.open ? (
                <p className="num mt-1">
                  <span className={run.open.side === 'LONG' ? 'text-lc' : 'text-lp'}>{run.open.side.toLowerCase()}</span>{' '}
                  {run.open.lots} @ {run.open.entry_price.toFixed(2)} since {fmtTime(run.open.entry_time)} · stop{' '}
                  {run.open.stop == null ? 'none (sizing unit only)' : run.open.stop.toFixed(2)} ·{' '}
                  <Figure value={usd(run.open.unrealised_usd_at_last_close)} tone={run.open.unrealised_usd_at_last_close >= 0 ? 'up' : 'down'} /> at the last close
                </p>
              ) : (
                <p className="text-muted-foreground mt-1">flat</p>
              )}
            </div>

            <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1">
              <Counters label="refused" map={run.skipped_by_guard} />
              <Counters label="closed by guard" map={run.closed_by_guard} />
              {run.sized_down > 0 && <span className="text-muted-foreground text-[11px]">sized down ×{run.sized_down}</span>}
              <span className="text-muted-foreground text-[11px]">
                news: {run.news.events_loaded} events
                {run.news.next_blackout && (
                  <>
                    {' '}· next blackout {fmtTime(run.news.next_blackout.time)} {run.news.next_blackout.currency}
                    {run.news.next_blackout.name ? ` ${run.news.next_blackout.name}` : ''}
                  </>
                )}
              </span>
            </div>

            <div className="mt-3">
              <Label>Last fills</Label>
              {run.last_fills.length === 0 ? (
                <p className="text-muted-foreground mt-1 text-[12px]">none yet — the first live bar decides, the warm-up bars do not</p>
              ) : (
                <ul className="mt-1 space-y-1">
                  {run.last_fills.slice().reverse().map((t) => (
                    <Fill key={`${t.entryTime}-${t.exitTime}`} t={t} />
                  ))}
                </ul>
              )}
            </div>
          </Panel>
        )
      })}
    </div>
  )
}
