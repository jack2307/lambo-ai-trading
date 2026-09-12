import type { Research, ResearchHypothesis, ResearchRun } from '@/lib/api'
import { num } from '@/lib/format'
import { cn } from '@/lib/utils'

import { Label, Panel, Pill } from './primitives'

/**
 * What the research loop is doing, read from the files it writes.
 *
 * Overview mode: one hypothesis at a time, its place in the six steps, and
 * the tables that decided it — quoted from the run receipts, not retyped.
 * The backlog beside it says what comes next. Nothing here is live in the
 * websocket sense; the page re-reads `docs/` every few seconds and says when
 * it last did.
 */

const STEPS = ['Registered', 'Implemented', 'In-sample', 'Out-of-sample', 'Decided'] as const

/** A percentile the server could not compute arrives as null (NaN has no JSON). */
const pct = (v: number | null | undefined) => (v == null || Number.isNaN(v) ? '—' : `${v.toFixed(0)}`)

function stepIndex(status: string): number {
  const s = status.toLowerCase()
  if (s.startsWith('decided')) return 4
  if (s.includes('out-of-sample')) return 3
  if (s.includes('in-sample')) return 2
  if (s.includes('implemented')) return 1
  return 0
}

export function ResearchPanel({ research, updatedAgo }: { research: Research | null; updatedAgo: string }) {
  const current = research?.hypotheses[0] ?? null
  const inFlight = current && !current.status.toLowerCase().startsWith('decided')
  const next = research?.backlog.open[0]

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
      <Panel className="p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <Label>{inFlight ? 'In flight' : 'Last pass'}</Label>
            {current && (
              <Pill tone={inFlight ? 'live' : 'neutral'}>{inFlight ? 'researching' : 'closed'}</Pill>
            )}
          </div>
          <span className="text-muted-foreground text-[11px]">read from docs/ · {updatedAgo}</span>
        </div>

        {!research ? (
          <p className="text-muted-foreground text-[13px]">Reading the research files…</p>
        ) : !current ? (
          <p className="text-muted-foreground text-[13px]">
            Nothing in flight.{' '}
            {next ? (
              <>
                <span className="num text-primary">/research next</span> picks up <b className="text-foreground">{next.title}</b>.
              </>
            ) : (
              'The backlog is empty; add an idea with a reason.'
            )}
          </p>
        ) : (
          <HypothesisView hypothesis={current} />
        )}
      </Panel>

      <div className="space-y-4">
        <Panel className="p-4">
          <div className="mb-3 flex items-baseline justify-between">
            <Label>Backlog</Label>
            <span className="text-muted-foreground num text-[11px]">
              {research?.backlog.open.length ?? '–'} open · {research?.backlog.closed.length ?? '–'} closed
            </span>
          </div>
          {research && research.backlog.open.length === 0 ? (
            <p className="text-muted-foreground text-[12px]">Empty. An idea needs a reason to be listed.</p>
          ) : (
            <ol className="space-y-2">
              {research?.backlog.open.slice(0, 6).map((item, i) => (
                <li key={item.title} className="flex gap-2.5 text-[12px] leading-snug">
                  <span className="text-muted-foreground num w-4 shrink-0 text-right">{i + 1}</span>
                  <span className="min-w-0">
                    <span className="font-medium">{item.title}</span>
                    {item.note && <span className="text-muted-foreground block truncate">{item.note}</span>}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </Panel>

        <Panel className="p-4">
          <div className="mb-3 flex items-baseline justify-between">
            <Label>Decisions</Label>
            <span className="text-muted-foreground num text-[11px]">{research?.decisions.length ?? '–'} on record</span>
          </div>
          <ul className="space-y-1.5">
            {research?.decisions.slice(0, 5).map((d) => (
              <li key={d.file} className="flex gap-2.5 text-[12px] leading-snug">
                <span className="text-muted-foreground num shrink-0">{d.date.slice(5)}</span>
                <span className="min-w-0 truncate" title={d.title}>
                  {d.title}
                </span>
              </li>
            ))}
          </ul>
        </Panel>
      </div>
    </div>
  )
}

function HypothesisView({ hypothesis: h }: { hypothesis: ResearchHypothesis }) {
  const step = stepIndex(h.status)
  return (
    <div className="space-y-4">
      <div>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <span className="num text-[13px] font-semibold">{h.id}</span>
          <span className="text-muted-foreground text-[11px]">registered {h.registered || '—'}</span>
        </div>
        <p className="mt-1 max-w-[70ch] text-[13px] leading-snug">{h.claim}</p>
      </div>

      {/* The six steps as a line: done, current, ahead. */}
      <ol className="flex items-center gap-0" aria-label="Pipeline">
        {STEPS.map((name, i) => {
          const state = i < step ? 'done' : i === step ? 'current' : 'ahead'
          return (
            <li key={name} className="flex flex-1 items-center last:flex-none">
              <span className="flex items-center gap-1.5">
                <span
                  className={cn(
                    'size-2.5 rounded-full border',
                    state === 'done' && 'bg-foreground/70 border-foreground/70',
                    state === 'current' && 'bg-primary border-primary shadow-[0_0_0_4px_rgb(141_255_8/0.18)]',
                    state === 'ahead' && 'border-border',
                  )}
                  aria-hidden
                />
                <span className={cn('text-[11px]', state === 'ahead' ? 'text-muted-foreground' : 'text-foreground', state === 'current' && 'font-medium')}>
                  {name}
                </span>
              </span>
              {i < STEPS.length - 1 && <span className={cn('mx-2 h-px flex-1', i < step ? 'bg-foreground/40' : 'bg-border')} aria-hidden />}
            </li>
          )
        })}
      </ol>

      <div className="text-muted-foreground flex flex-wrap gap-x-4 gap-y-1 text-[11px]">
        <span>
          in-sample <span className="num text-foreground">{h.inSample ?? '—'}</span>
        </span>
        <span>
          out-of-sample <span className="num text-foreground">{h.outOfSample ?? '—'}</span>
        </span>
        <span>
          batch <span className="num text-foreground">{h.batch.length}</span> hypothes{h.batch.length === 1 ? 'is' : 'es'}
        </span>
        {h.runs.direction.map((d) => (
          <span key={d.base}>
            direction null{' '}
            <span className={cn('num', d.outside ? 'text-lc' : 'text-foreground')}>
              {pct(d.percentile)}th pct
            </span>{' '}
            · {d.outside ? 'outside' : 'inside'}
          </span>
        ))}
      </div>

      {h.runs.inSample && <RunTable title="In-sample" market={h.inSample} run={h.runs.inSample} />}
      {h.runs.outOfSample && <RunTable title="Out-of-sample" market={h.outOfSample} run={h.runs.outOfSample} />}
      {!h.runs.inSample && (
        <p className="text-muted-foreground text-[12px]">No run receipts yet — the in-sample stage has not written a table.</p>
      )}
    </div>
  )
}

function RunTable({ title, market, run }: { title: string; market: string | null; run: ResearchRun }) {
  return (
    <div>
      <div className="mb-1.5 flex items-baseline justify-between">
        <span className="text-[11px] font-medium tracking-wide uppercase">
          {title} <span className="text-muted-foreground num normal-case">{market}</span>
        </span>
        <span className="text-[11px]">
          {!run.concluded ? (
            <Pill tone="live">running</Pill>
          ) : run.survivors.length ? (
            <Pill tone="caution">{run.survivors.length} survivor{run.survivors.length === 1 ? '' : 's'}</Pill>
          ) : (
            <Pill>nothing survived</Pill>
          )}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-[12px]">
          <thead className="text-muted-foreground text-[10px] tracking-wide uppercase">
            <tr className="border-border border-b">
              <th className="py-1 pr-2 text-left font-normal">hypothesis</th>
              <th className="px-2 py-1 text-right font-normal">trades</th>
              <th className="px-2 py-1 text-right font-normal">PF</th>
              <th className="px-2 py-1 text-right font-normal">expect</th>
              <th className="px-2 py-1 text-right font-normal">null p95</th>
              <th className="px-2 py-1 text-right font-normal">pct</th>
              <th className="py-1 pl-2 text-left font-normal">verdict</th>
            </tr>
          </thead>
          <tbody>
            {run.rows.map((r) => {
              const survives = r.verdict.startsWith('SURVIVES')
              return (
                <tr key={r.label} className="border-border/60 border-b last:border-0">
                  <td className="py-1 pr-2">
                    <span className="font-medium">{r.label}</span>
                    <span className="text-muted-foreground num ml-1.5 text-[10px]">{r.base}</span>
                  </td>
                  <td className={cn('num px-2 py-1 text-right', r.trades < 30 && 'text-caution')}>{r.trades}</td>
                  <td className={cn('num px-2 py-1 text-right', r.profitFactor >= 1 ? 'text-lc' : 'text-lp')}>{num(r.profitFactor, 3)}</td>
                  <td className="num px-2 py-1 text-right">{num(r.expectancy, 3)} R</td>
                  <td className="num text-muted-foreground px-2 py-1 text-right">{num(r.nullP95, 3)}</td>
                  <td className={cn('num px-2 py-1 text-right', (r.percentile ?? 0) >= 95 && 'text-lc')}>{pct(r.percentile)}%</td>
                  <td className="py-1 pl-2">
                    {survives ? (
                      <Pill tone="caution">survives</Pill>
                    ) : (
                      <span className="text-muted-foreground block max-w-[36ch] truncate" title={r.verdict}>
                        {r.verdict.replace(/^fail: /, '')}
                      </span>
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
