import { useMemo, useState } from 'react'

import { Empty, Label, Panel, Pill } from '@/components/dashboard/primitives'
import { Skeleton } from '@/components/ui/skeleton'
import type { Proposal, Research as ResearchData } from '@/lib/api'
import { cn } from '@/lib/utils'

/**
 * The scouting board — ideas before they are hypotheses.
 *
 * Three roles write into `docs/research/scouting/`: a scout that proposes and
 * has no veto, a historian that answers whether it has been tried before, and
 * a feasibility gate that answers whether it can be tested with what is on
 * this disk. `docs/research/SCOUTING.md` is the design.
 *
 * **The rejected ones are shown, not hidden.** A board that displayed only
 * live ideas would be a board that quietly re-proposes the dead ones, which is
 * the exact failure the historian exists to prevent; the reason an idea was
 * killed is the half of this screen that stops the team going in circles. The
 * default filter is everything, and the graveyard is one click away rather
 * than one build away.
 *
 * Status is computed by the server from the verdicts and never written by an
 * agent, so nothing here can promote itself.
 */

const STATUS: Record<string, { tone: 'live' | 'neutral' | 'caution' | 'blocked'; label: string }> = {
  shortlisted: { tone: 'live', label: 'shortlisted' },
  registered: { tone: 'neutral', label: 'registered' },
  proposed: { tone: 'caution', label: 'awaiting the gates' },
  rejected: { tone: 'blocked', label: 'rejected' },
}

/** A verdict that killed the proposal, whoever said it. */
const FATAL = new Set(['CLOSED', 'BLOCKED', 'BROKEN'])

type Filter = 'all' | 'live' | 'rejected'

export function ScoutingBoard({
  research,
  updatedAgo,
}: {
  research: ResearchData | null
  updatedAgo: string
}) {
  const [filter, setFilter] = useState<Filter>('all')
  const [open, setOpen] = useState<string | null>(null)

  const all = research?.scouting ?? []
  const counts = useMemo(() => {
    const c: Record<string, number> = { shortlisted: 0, registered: 0, proposed: 0, rejected: 0 }
    for (const p of all) c[p.status] = (c[p.status] ?? 0) + 1
    return c
  }, [all])

  const shown = useMemo(() => {
    if (filter === 'live') return all.filter((p) => p.status !== 'rejected')
    if (filter === 'rejected') return all.filter((p) => p.status === 'rejected')
    return all
  }, [all, filter])

  if (!research) {
    return (
      <Panel className="space-y-3 p-4">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-20 w-full" />
      </Panel>
    )
  }

  if (all.length === 0) {
    return (
      <Panel className="p-4">
        <Empty
          title="The scouting team has proposed nothing yet."
          hint="Run it with /scout. Three roles write here: a scout that proposes, a historian that says whether it has been tried, and a feasibility gate that says whether it can be tested with what is on this disk."
        />
      </Panel>
    )
  }

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-4 py-2.5">
        <span className="text-[11px] font-medium">
          {all.length} idea{all.length === 1 ? '' : 's'}
        </span>
        <span className="text-muted-foreground num text-[11px]">
          {counts.shortlisted} shortlisted · {counts.proposed} awaiting · {counts.rejected} rejected
        </span>
        <div className="ml-auto flex gap-1">
          {(['all', 'live', 'rejected'] as Filter[]).map((f) => (
            <button
              key={f}
              type="button"
              onClick={() => setFilter(f)}
              className={cn(
                'rounded-full border px-2.5 py-0.5 text-[11px] transition-colors',
                filter === f
                  ? 'border-primary/40 bg-primary/10 text-foreground'
                  : 'border-border text-muted-foreground hover:text-foreground',
              )}
            >
              {f === 'all' ? 'All' : f === 'live' ? 'Still alive' : 'The graveyard'}
            </button>
          ))}
        </div>
        <span className="text-muted-foreground w-full text-[10px]">{updatedAgo}</span>
      </div>

      <ul className="divide-border divide-y">
        {shown.map((p) => (
          <ProposalRow key={p.id} proposal={p} open={open === p.id} onToggle={() => setOpen(open === p.id ? null : p.id)} />
        ))}
      </ul>
    </Panel>
  )
}

function ProposalRow({
  proposal,
  open,
  onToggle,
}: {
  proposal: Proposal
  open: boolean
  onToggle: () => void
}) {
  const status = STATUS[proposal.status] ?? STATUS.proposed
  const killer = proposal.verdicts.find((v) => FATAL.has(v.verdict.toUpperCase()))

  return (
    <li>
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className="hover:bg-elevated/50 focus-visible:ring-ring w-full px-4 py-3 text-left transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none"
      >
        <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
          <span className={cn('text-[13px] font-medium', proposal.status === 'rejected' && 'text-muted-foreground')}>
            {proposal.title || proposal.id}
          </span>
          <Pill tone={status.tone}>{status.label}</Pill>
          {proposal.verdicts.map((v) => (
            <span
              key={v.role + v.at}
              className={cn(
                'num text-[10px]',
                FATAL.has(v.verdict.toUpperCase()) ? 'text-lp' : 'text-muted-foreground',
              )}
            >
              {v.role} {v.verdict}
            </span>
          ))}
        </div>
        {/* The one line that matters when it is closed: why it died, or what it
            claims. A row that only shows a title makes the reader open every
            one of them to find the interesting ones. */}
        <p className="text-muted-foreground mt-1 line-clamp-2 text-[12px] leading-snug">
          {killer ? killer.note : proposal.scout.mechanism}
        </p>
      </button>

      {open && (
        <div className="bg-elevated/30 space-y-3 px-4 pt-1 pb-4 text-[12px]">
          <Field label="Mechanism">{proposal.scout.mechanism}</Field>
          <Field label="Why it survives being known">{proposal.scout.why_unarbitraged}</Field>
          <Field label="What would kill it">{proposal.scout.falsifier_sketch}</Field>
          <Field label="Closest thing already closed">{proposal.scout.closest_known}</Field>
          {proposal.scout.data_needed.length > 0 && (
            <Field label="Data it needs">
              <span className="num">{proposal.scout.data_needed.join(' · ')}</span>
            </Field>
          )}

          <div className="space-y-2 pt-1">
            <Label>The gates</Label>
            {proposal.verdicts.length === 0 ? (
              <p className="text-caution text-[11px]">
                Nobody has looked at this yet. It cannot be shortlisted until the historian and feasibility have both
                spoken.
              </p>
            ) : (
              proposal.verdicts.map((v) => (
                <div key={v.role + v.at} className="border-border border-l-2 pl-3">
                  <p className="flex flex-wrap items-baseline gap-2">
                    <span className="num text-foreground text-[11px] font-medium">{v.role}</span>
                    <span
                      className={cn(
                        'num text-[11px]',
                        FATAL.has(v.verdict.toUpperCase()) ? 'text-lp' : 'text-lc',
                      )}
                    >
                      {v.verdict}
                    </span>
                  </p>
                  <p className="text-muted-foreground mt-0.5 leading-snug">{v.note}</p>
                  {v.instrument && (
                    <p className="text-muted-foreground/80 num mt-0.5 text-[11px]">instrument: {v.instrument}</p>
                  )}
                  {v.sample && <p className="text-muted-foreground/80 num text-[11px]">sample: {v.sample}</p>}
                  {v.evidence.length > 0 && (
                    <p className="text-muted-foreground/70 num mt-0.5 text-[10px]">{v.evidence.join(' · ')}</p>
                  )}
                </div>
              ))
            )}
          </div>

          <p className="text-muted-foreground/70 num text-[10px]">
            {proposal.file}
            {proposal.registered_as && ` → ${proposal.registered_as}`}
          </p>
        </div>
      )}
    </li>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <Label>{label}</Label>
      <p className="mt-0.5 leading-snug">{children || <span className="text-caution">not stated — the gates reject a proposal missing any of the five fields</span>}</p>
    </div>
  )
}
