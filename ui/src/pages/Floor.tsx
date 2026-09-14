/**
 * The floor.
 *
 * Overview mode, and the one 3D scene the product is allowed (see `DESIGN.md`).
 * Moved here verbatim from the old Dashboard so the scene has the screen to
 * itself: it was competing with three tables for the same viewport.
 *
 * three.js is still lazy-loaded, `?floor=lite` still drops shadows and the
 * photoreal set, and the motion toggle still reads the system's
 * `prefers-reduced-motion` and lets the reader overrule it.
 */

import { Suspense, lazy, useState } from 'react'

import { Label, Monogram, Panel, Pill, SectionHeader } from '@/components/dashboard/primitives'
import type { Department } from '@/components/dashboard/OfficeFloor'
import { Skeleton } from '@/components/ui/skeleton'
import { MODELS, SESSION_MODEL, seat } from '@/lib/org'
import { cn } from '@/lib/utils'

/**
 * The floor: open plan on a timber field with two flat walls, north and
 * west. Along the long north wall, the reception and the show car. In front
 * of it the north wing — the glass office, the advisory desks, the archive,
 * the meeting room — then the aisle, then the south wing: the data racks,
 * the strategy desk, the engine bay and the three veto desks in a row. A
 * file leaves the archive and comes back to it, as a record or as closed.
 */
const DEPARTMENTS: Department[] = [
  // The north-west corner: the archive, whose shelves are tall, against both walls.
  { id: 'archive', title: 'Archive', line: 'Hypotheses, run receipts, decisions, backlog.', tone: 'ops', x: -10.4, z: -3.55, w: 3.0, d: 5.5, occupants: [], furniture: 'shelves' },
  // The rest of the long north wall: reception and the show car, one public space.
  { id: 'reception', title: 'Reception & showcase', line: 'The front desk, the show car, sofas, coffee, the tape on a screen.', tone: 'public', x: 1.6, z: -3.55, w: 20.4, d: 5.5, occupants: [], furniture: 'showcase' },
  // North wing, in front of it: the glass office, the advisory desks, the engine bay, the meeting room.
  { id: 'arbiter', title: "Arbiter's office", line: 'Reads the receipts and the vetoes; decides last.', tone: 'arbiter', x: -9.6, z: 2.3, w: 3.4, d: 2.8, enclosed: true, occupants: [{ id: 'arbiter', title: 'Arbiter', model: SESSION_MODEL.name }] },
  { id: 'advisory', title: 'Advisory', line: 'Notes to the arbiter; none of them can stop anything alone.', tone: 'advisory', x: -4.2, z: 2.3, w: 5.8, d: 2.8, arrange: 'grid', occupants: [seat('researcher'), seat('execution-realist'), seat('portfolio'), seat('historian')] },
  { id: 'engine', title: 'Engine room', line: 'Backtest, walk-forward, nulls; the search binary.', tone: 'ops', x: 1.4, z: 2.3, w: 3.2, d: 2.8, occupants: [], furniture: 'engine' },
  { id: 'meeting', title: 'Meeting room', line: 'Where a decision is argued before it is written.', tone: 'public', x: 8.6, z: 2.3, w: 3.4, d: 2.8, enclosed: true, occupants: [], furniture: 'meeting' },
  // South wing, along the aisle: data, the lab, then the veto desks in a row.
  { id: 'data', title: 'Data room', line: 'Broker bars, two free feeds, the tape collectors.', tone: 'ops', x: -9.0, z: 6.5, w: 3.4, d: 3.0, occupants: [], furniture: 'racks', racks: ['MT5 · Vantage', 'Dukascopy', 'Binance', 'OTL tape'] },
  { id: 'lab', title: 'Strategy lab', line: 'Pre-registers, implements, tests for look-ahead.', tone: 'ops', x: -5.4, z: 6.5, w: 2.8, d: 3.0, occupants: [{ id: 'strategy-implementer', title: 'Implementer', model: MODELS.sonnet.name }] },
  { id: 'data-integrity', title: 'Data Integrity', line: 'Veto', tone: 'veto', x: -1.6, z: 6.5, w: 1.7, d: 3.0, occupants: [seat('data-integrity')] },
  { id: 'adversary', title: 'Adversary', line: 'Veto', tone: 'veto', x: 0.7, z: 6.5, w: 1.7, d: 3.0, occupants: [seat('adversary')] },
  { id: 'risk', title: 'Risk', line: 'Veto', tone: 'veto', x: 3.0, z: 6.5, w: 1.7, d: 3.0, occupants: [seat('risk')] },
]

/**
 * Loaded on demand. three.js is half a megabyte, and the tape reader and the
 * workbench never draw a triangle — they should not pay for one.
 */
const OfficeFloor = lazy(() => import('@/components/dashboard/OfficeFloor').then((m) => ({ default: m.OfficeFloor })))

/**
 * The show car in the lobby. A Sketchfab model, CC BY-NC 4.0: the author is
 * credited under the scene, and it may not ship in anything sold — this
 * dashboard is the owner's own tool. Swap the file and the credit together.
 */
const CAR_MODEL = {
  path: '/models/f1-75.glb',
  title: 'Ferrari F1-75',
  url: 'https://sketchfab.com/3d-models/ferrari-f1-75-06454e0f23a44fcdabcc7808aee6caf9',
  author: 'Sketcher',
  authorUrl: 'https://sketchfab.com/sketcher987654321',
  license: 'CC BY-NC 4.0',
  licenseUrl: 'http://creativecommons.org/licenses/by-nc/4.0/',
} as const

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

export function Floor() {
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

  return (
    <main className="mx-auto w-full max-w-[1440px] space-y-6 p-4 lg:p-6">
      <section>
        <SectionHeader
          title="The floor"
          subtitle="An open-plan floor with the reception and the show car along its long wall. A file leaves the archive, is written up in the lab, run in the engine bay, and passes three desks that can each send it back before it reaches the glass office."
        />
        <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
          <Panel className="relative min-h-[560px] overflow-hidden">
            <Suspense fallback={<Skeleton className="absolute inset-0 rounded-[inherit]" />}>
              <OfficeFloor departments={DEPARTMENTS} animate={animate} carModel={CAR_MODEL.path} className="absolute inset-0" />
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
                <span className="text-muted-foreground"> · reception & showcase, meeting room</span>
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
              <span className="text-muted-foreground">drag to orbit · click a department to zoom in · double-click to come back</span>
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
            <p className="text-muted-foreground mt-2 text-[10px] leading-relaxed">
              Show car:{' '}
              <a href={CAR_MODEL.url} className="hover:text-foreground underline underline-offset-2" target="_blank" rel="noreferrer">
                {CAR_MODEL.title}
              </a>{' '}
              by{' '}
              <a href={CAR_MODEL.authorUrl} className="hover:text-foreground underline underline-offset-2" target="_blank" rel="noreferrer">
                {CAR_MODEL.author}
              </a>
              ,{' '}
              <a href={CAR_MODEL.licenseUrl} className="hover:text-foreground underline underline-offset-2" target="_blank" rel="noreferrer">
                {CAR_MODEL.license}
              </a>
              . Furniture:{' '}
              <a href="https://polyhaven.com/models" className="hover:text-foreground underline underline-offset-2" target="_blank" rel="noreferrer">
                Poly Haven
              </a>{' '}
              and{' '}
              <a href="https://kenney.nl" className="hover:text-foreground underline underline-offset-2" target="_blank" rel="noreferrer">
                Kenney
              </a>
              ; people: Kenney (all CC0).
            </p>
          </Panel>
        </div>
      </section>
    </main>
  )
}
