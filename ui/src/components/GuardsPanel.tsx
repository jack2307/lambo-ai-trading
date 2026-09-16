import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'
import { api, type GuardEdit, type GuardValues, type GuardsView } from '@/lib/api'
import { cn } from '@/lib/utils'

/**
 * The guards, and what each one is for.
 *
 * The `why` lines are not decoration. Four of these came out of specific losses
 * in the research record, and one came out of finding that a book calling
 * itself "1% risk" was leveraged 1.3x with no stop. A panel that offered the
 * numbers without the reasons would invite someone to loosen a guard whose cost
 * has already been measured, on the grounds that it looked inconvenient.
 */
const FIELDS: {
  key: keyof GuardEdit
  label: string
  unit: string
  step: number
  what: string
  why?: string
}[] = [
  {
    key: 'max_trades_per_day',
    label: 'Trades a day',
    unit: '',
    step: 1,
    what: 'Entries are refused past this count, per book, per UTC day.',
  },
  {
    key: 'daily_loss_limit_usd',
    label: 'Daily loss limit',
    unit: 'USD',
    step: 10,
    what: 'Once a day is down this much, the book stops entering until tomorrow.',
  },
  {
    key: 'cooldown_min',
    label: 'Cooldown',
    unit: 'min',
    step: 5,
    what: 'How long a book waits after a trade closes before it may take another.',
  },
  {
    key: 'max_open_loss_r',
    label: 'Open-loss cap',
    unit: 'R',
    step: 0.5,
    what: 'An open position further under water than this is closed at market. Checked every bar, including the bar it reopened on.',
    why: 'From the silver TSMOM run: a self-managed exit can sit through a loss no rule ever sized for.',
  },
  {
    key: 'max_notional_pct_equity',
    label: 'Notional cap',
    unit: '% of equity',
    step: 25,
    what: 'Lots are cut until lots x contract x price fits under this. Below the minimum lot the entry is refused instead of shrunk.',
    why: 'From fx-local-hours: the row labelled "1% risk" was 1.3x leveraged with no stop. This is what cuts every trade on this desk from 0.10 to 0.06 lots.',
  },
  {
    key: 'flat_before_weekend_hhmm',
    label: 'Weekend flat',
    unit: 'HHMM New York, Friday',
    step: 5,
    what: 'From this time on Friday the book is closed out and takes nothing new. 0 turns it off.',
    why: 'From close-reopen-drift: a 50-hour weekend hold is not a trade this desk can price.',
  },
  {
    key: 'news_flat_before_min',
    label: 'Flat before news',
    unit: 'min',
    step: 15,
    what: 'Minutes ahead of a scheduled event that the book stays out.',
    why: 'From fx-local-hours: worst hold -3.12R, on the ECB of 2015-12-03.',
  },
  {
    key: 'news_flat_after_min',
    label: 'Flat after news',
    unit: 'min',
    step: 15,
    what: 'And minutes after it.',
  },
  {
    key: 'news_min_impact',
    label: 'News impact floor',
    unit: '0-3',
    step: 1,
    what: 'The lowest calendar impact the news guard reacts to. 3 is the high-impact releases only.',
  },
]

const valueOf = (v: GuardValues, key: keyof GuardEdit): number => v[key as keyof GuardValues] as number

export function GuardsPanel() {
  const [open, setOpen] = useState(false)
  const [view, setView] = useState<GuardsView | null>(null)
  const [draft, setDraft] = useState<Record<string, string>>({})
  const [saving, setSaving] = useState(false)
  const box = useRef<HTMLDivElement>(null)

  // Read when it opens, not on mount: this is a panel someone visits, and the
  // values only change when someone changes them here.
  useEffect(() => {
    if (!open) return
    api
      .guards()
      .then((v) => {
        setView(v)
        const next: Record<string, string> = {}
        for (const f of FIELDS) next[f.key] = String(valueOf(v.effective, f.key))
        setDraft(next)
      })
      .catch((err: Error) => toast.error('Could not read the guards', { description: err.message }))
  }, [open])

  // Click away and Escape both close it. A settings panel that can only be
  // dismissed by finding its own button again is a panel people leave open.
  useEffect(() => {
    if (!open) return
    const away = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) setOpen(false)
    }
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', away)
    document.addEventListener('keydown', key)
    return () => {
      document.removeEventListener('mousedown', away)
      document.removeEventListener('keydown', key)
    }
  }, [open])

  const edited = view ? Object.values(view.edited).some((v) => v != null) : false
  const dirty = view ? FIELDS.some((f) => Number(draft[f.key]) !== valueOf(view.effective, f.key)) : false

  const save = () => {
    if (!view) return
    const body: GuardEdit = {}
    for (const f of FIELDS) {
      const n = Number(draft[f.key])
      if (!Number.isFinite(n)) continue
      // Only what differs from the FILE is sent. A value put back to the
      // configured one stops being an override rather than being pinned to the
      // same number by a different authority.
      if (n !== valueOf(view.configured, f.key)) (body as Record<string, number>)[f.key] = n
    }
    setSaving(true)
    api
      .setGuards(body)
      .then((v) => {
        setView(v)
        toast.success('Guards changed', {
          description: `Written into the record of ${v.guarded_runs} book${v.guarded_runs === 1 ? '' : 's'}. Takes effect on their next bar.`,
        })
      })
      .catch((err: Error) => toast.error('Could not change the guards', { description: err.message }))
      .finally(() => setSaving(false))
  }

  return (
    <div className="relative" ref={box}>
      <Button
        variant="ghost"
        size="sm"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        title="The risk rules every guarded book runs under"
        className={cn(
          'h-6 rounded-full px-3 text-xs font-medium',
          edited ? 'text-caution' : 'text-muted-foreground hover:text-foreground',
        )}
      >
        guards{edited && ' *'}
      </Button>

      {open && (
        <div className="bg-card absolute top-8 right-0 z-50 flex max-h-[80vh] w-[380px] flex-col rounded-md border shadow-lg">
          <div className="border-b px-3 py-2">
            <h2 className="text-[13px] font-semibold">Guards</h2>
            <p className="text-muted-foreground mt-0.5 text-[11px] leading-snug">
              One set, shared by every book that asks for guards
              {view ? ` — ${view.guarded_runs} of them right now` : ''}. A change is written into each of their
              records, because their numbers only mean something under a stated set of rules.
            </p>
          </div>

          {!view ? (
            <p className="text-muted-foreground px-3 py-6 text-center text-[11px]">Reading…</p>
          ) : (
            <>
              <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3 py-3">
                <p className="text-muted-foreground border-border rounded-sm border px-2 py-1.5 text-[10px] leading-snug">
                  One position per book is not here. The reconciler, the mirror and the desk are all built on it; a
                  control for it would be a switch with nothing behind it.
                </p>

                {FIELDS.map((f) => {
                  const now = Number(draft[f.key])
                  const configured = valueOf(view.configured, f.key)
                  const off = Number.isFinite(now) && now !== configured
                  return (
                    <label key={f.key} className="flex flex-col gap-1">
                      <span className="flex items-baseline gap-2">
                        <span className="text-[12px] font-medium">{f.label}</span>
                        {off && <span className="text-caution num text-[10px]">file says {configured}</span>}
                      </span>
                      <span className="flex items-center gap-2">
                        <input
                          type="number"
                          step={f.step}
                          value={draft[f.key] ?? ''}
                          onChange={(e) => setDraft((d) => ({ ...d, [f.key]: e.target.value }))}
                          className="border-border bg-background num h-7 w-24 rounded-sm border px-2 text-[12px]"
                        />
                        <span className="text-muted-foreground text-[11px]">{f.unit}</span>
                      </span>
                      <span className="text-muted-foreground/80 text-[10px] leading-snug">{f.what}</span>
                      {f.why && <span className="text-muted-foreground/55 text-[10px] leading-snug">{f.why}</span>}
                    </label>
                  )
                })}
              </div>

              <div className="bg-card flex items-center gap-2 border-t px-3 py-2">
                <Button size="sm" disabled={!dirty || saving} onClick={save} className="h-7 text-xs">
                  {saving ? 'Saving…' : 'Apply'}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    const next: Record<string, string> = {}
                    for (const f of FIELDS) next[f.key] = String(valueOf(view.configured, f.key))
                    setDraft(next)
                  }}
                  className="h-7 text-xs"
                >
                  Back to the file
                </Button>
                <span className="text-muted-foreground ml-auto text-[10px]">next bar</span>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  )
}
