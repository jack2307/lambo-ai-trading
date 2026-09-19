import { useCallback, useEffect, useMemo, useState } from 'react'

import { api, type IndicatorInfo, type IndicatorPoint, type MeasuredCell } from '@/lib/api'
import type { ActiveIndicator } from '@/components/PriceChart'
import { LevelSwitch } from '@/components/LevelToggles'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { LIBRARY_COPY, LIBRARY_GROUPS, METHOD_IDS } from '@/lib/library'
import { LEVEL_FAMILIES, type LevelFamily, type LevelMode } from '@/lib/levels'
import { cn } from '@/lib/utils'
import { verdictFor } from '@/lib/verdicts'

/**
 * The viewer's own indicators on the Desk chart.
 *
 * TWO SETS ON ONE CHART, AND THEY ARE NOT THE SAME KIND OF THING. The book's
 * indicators are the record: the lines the running strategy actually read
 * when it decided, delivered with the run and computed on the timeframe it
 * trades. These are a LOOK — chosen here, computed by the server for the
 * timeframe on screen, read by nothing. Mixing them silently would make the
 * chart unable to answer the only question that matters about a line on it:
 * did the bot see this?
 *
 * So every one of them is marked `source: 'viewer'`, drawn DASHED where the
 * book's are solid, named `yours · …` where the book's are `book · …`, and
 * listed under its own word in the caption. The distinction rides on the
 * drawing and on each entry in the list, never on a heading above them: a
 * peer session shipped a bug this week where cached and freshly-fetched
 * figures were identical on screen because the only thing separating them was
 * a caption in another corner.
 *
 * COLOUR IS DELIBERATELY NOT THE CHANNEL. There are five data hues in the
 * palette (`ui/DESIGN.md`) and both sets cycle the same five; a sixth hue
 * invented for this would be a colour nobody measured for contrast, and a
 * reader who learned "teal means viewer" would be wrong the moment the book's
 * fifth indicator came round to teal. Line style and the word carry it.
 */

/* ------------------------------------------------------------ verdicts */

/*
 * The real table landed with `ui/src/lib/verdicts.ts` (merge 159d109): 39
 * constructs read out of the closed registrations, checked by
 * `ui/scripts/verdicts.check.mjs`, which fails the build when a badge cites a
 * file that is not on disk or prints a number that file does not contain.
 * The stub that stood here through the parallel build is gone; its five
 * entries are among the thirty-nine, and the interface was agreed in advance
 * so the swap was a deletion.
 */

/* ------------------------------------------------------------ measured */

/*
 * THE SECOND KIND OF EVIDENCE, AND IT IS NOT THE VERDICT.
 *
 * A verdict says what happened when somebody registered a rule on an
 * indicator and ran it. A measurement says what the LINE does: how often it
 * changes its mind, how much of that it takes back within three bars, how
 * late it is to a turn, how many turns it sleeps through. Three definitions
 * carry one — `supertrend`, `zigzag` and `avwap` — because this desk spent
 * 2026-09-18 measuring seventeen of them over 25,708 H1 and 6,728 H4 bars,
 * and no chart anywhere shows a trader those numbers beside the line. This
 * one does, in the menu where the line is chosen rather than in a caption
 * under a chart that already has it drawn.
 *
 * Three rules hold the display honest:
 *
 *  - THE TIMEFRAME IS ALWAYS PRINTED. A number measured on H1 is not a
 *    number about H4, and the Desk can be on 5m. The label never claims the
 *    chart's timeframe; it names the one the study used.
 *  - THE PARAMETERS MUST MATCH. `avwap` on the day anchor is 19.7 flips per
 *    100 bars and on the week 9.2. A viewer who changed a period is told the
 *    numbers do not describe their line rather than shown numbers about a
 *    different one.
 *  - THE CAUTION IS NOT OPTIONAL. Where the study wrote one — it wrote one
 *    for the day-anchored VWAP, whose turns reverse 57% of the time within
 *    three bars — it is rendered in the caution colour, in full, before the
 *    line can be added.
 */

/** Does this measured cell describe the parameters actually in use? */
function cellMatches(cell: MeasuredCell, params: Record<string, number>): boolean {
  return Object.entries(cell.params).every(([name, value]) => {
    const chosen = params[name]
    // A parameter the viewer has not overridden is at the catalog default,
    // and the catalog default is the cell that was measured; `undefined`
    // therefore matches. An overridden one must match exactly.
    return chosen === undefined || chosen === value
  })
}

/**
 * The row to show: the study's numbers for these parameters, preferring the
 * timeframe on screen and otherwise saying plainly which one it used.
 *
 * Returns `null` when nothing was measured, and `'other-params'` when the
 * definition was measured but not at the parameters in the box — a state
 * that must be said out loud, because showing the measured row anyway is how
 * a number stops describing the line it sits next to.
 */
function measuredFor(
  info: IndicatorInfo | undefined,
  timeframe: string,
  params: Record<string, number>,
): MeasuredCell | 'other-params' | null {
  const rows = info?.measured
  if (!rows || rows.length === 0) return null
  const fitting = rows.filter((r) => cellMatches(r, params))
  if (fitting.length === 0) return 'other-params'
  return fitting.find((r) => r.timeframe === timeframe) ?? fitting[0]
}

/** `19.7 flips/100 · 57% undone ≤3 · lag 3 · 2% missed`, on one line. */
function measuredLine(cell: MeasuredCell): string {
  return (
    `${cell.timeframe}: ${cell.flipsPer100Bars} flips/100 · ` +
    `${cell.undoneWithin3Pct}% undone ≤3 · lag ${cell.medianLagBars} · ${cell.missedPct}% missed`
  )
}

/** The whole of it, for the hover, including where it came from. */
function measuredDetail(cell: MeasuredCell): string {
  const sample = cell.sampleBars.toLocaleString('en-US')
  const params = Object.entries(cell.params)
    .map(([k, v]) => `${k} ${v}`)
    .join(', ')
  return (
    `"${cell.definition}" (${params}) measured on ${sample} ${cell.timeframe} bars: ` +
    `${cell.flipsPer100Bars} label changes per 100 bars, ${cell.undoneWithin3Pct}% of them reversed ` +
    `within three bars, ${cell.medianLagBars} bars of median lag to a 4×ATR turn, ` +
    `${cell.missedPct}% of turns never agreed with.\n\n${cell.source}` +
    (cell.caution ? `\n\n${cell.caution}` : '')
  )
}

/**
 * The measurement under an indicator's name, wherever it is offered.
 *
 * Nothing is rendered for an unmeasured definition WHERE THE SLOT IS
 * INVISIBLE WHEN EMPTY. The temptation is a grey "no measurement", and in a
 * dense menu it is the same mistake as a blank verdict badge in the other
 * direction: a row in the slot where evidence goes is read as evidence, and
 * twelve of the fifteen definitions here have none.
 *
 * `absent="say"` is for the library, where the slot is NOT invisible: the
 * measurement has a column with a heading over it, and a blank cell under a
 * heading is itself an answer — the reader takes it for a pass. There the
 * absence is spelled out in words, which cannot be mistaken for evidence
 * because it carries no number at all.
 */
function Measured({
  cell,
  className,
  absent = 'silent',
}: {
  cell: MeasuredCell | 'other-params' | null
  className?: string
  absent?: 'silent' | 'say'
}) {
  if (cell === null) {
    if (absent === 'silent') return null
    return (
      <span
        className={cn('text-muted-foreground/70 fd-caption', className)}
        title="This desk measured seventeen definitions over 25,708 H1 and 6,728 H4 bars on 2026-09-18 and this was not one of them. There is no figure for how often this line changes its mind, how much of that it takes back, or how late it is to a turn — not a good one and not a bad one."
      >
        {LIBRARY_COPY.unmeasured}
      </span>
    )
  }
  if (cell === 'other-params') {
    return (
      <span className={cn('text-muted-foreground/70 fd-caption', className)} title="The study measured this definition at its default parameters. These are not those, so its latency and whipsaw numbers do not describe this line.">
        measured, but not at these parameters
      </span>
    )
  }
  return (
    <span className={cn('flex flex-col items-start', className)}>
      <span className="text-muted-foreground fd-caption num" title={measuredDetail(cell)}>
        {measuredLine(cell)}
      </span>
      {/* THE STUDY'S OWN WARNING, IN FULL AND IN THE CAUTION COLOUR. It is
          carried from the server as data (`caution`) rather than written
          here, so it cannot drift from the note that says it. */}
      {cell.caution && (
        <span className="text-caution fd-caption" title={cell.source}>
          ⚠ {cell.caution}
        </span>
      )}
    </span>
  )
}

/**
 * The same fact on a chip, where there is room for one number.
 *
 * The one kept is the COST — the share of this line's own turns it takes
 * back within three bars — because the two favourable numbers travel on
 * their own and this is the one a summary drops. The rest is on hover.
 */
function MeasuredChip({ cell }: { cell: MeasuredCell | 'other-params' | null }) {
  if (cell === null) return null
  if (cell === 'other-params') {
    return (
      <span
        className="text-muted-foreground/70"
        title="Measured at other parameters; those numbers do not describe this line."
      >
        unmeasured params
      </span>
    )
  }
  return (
    <span className={cn(cell.caution ? 'text-caution' : 'text-muted-foreground')} title={measuredDetail(cell)}>
      {cell.caution ? '⚠ ' : ''}
      {cell.undoneWithin3Pct}% undone ≤3 ({cell.timeframe})
    </span>
  )
}

/* ------------------------------------------------------------- storage */

/**
 * The viewer's choice, remembered per browser — the same idiom as
 * `fd.desk.book` in `App.tsx`, try/catch and all, because private mode
 * throws on the getter and a chart that fails to render is worse than a
 * chart that forgot a preference.
 */
const KEY = 'fd.desk.indicators'

/**
 * WHAT IS STORED IS THE CHOICE, NOT THE DRAWING.
 *
 * Only `{ id, params }` — the two things the viewer actually picked. The
 * outputs, the pane and the colour are rebuilt from the catalog on every
 * load, so a server that adds a `signal` output to MACD draws it for
 * everybody rather than only for people who cleared their storage. A stored
 * `outputs` array would have frozen this client's idea of the indicator on
 * the day it was clicked.
 */
export interface ViewerChoice {
  id: string
  params: Record<string, number>
}

function readChoices(): ViewerChoice[] {
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.flatMap((entry): ViewerChoice[] => {
      if (typeof entry !== 'object' || entry === null) return []
      const id = (entry as { id?: unknown }).id
      if (typeof id !== 'string' || id.length === 0) return []
      const stored = (entry as { params?: unknown }).params
      const params: Record<string, number> = {}
      if (typeof stored === 'object' && stored !== null) {
        for (const [k, v] of Object.entries(stored as Record<string, unknown>)) {
          // A hand-edited or half-written value is dropped, not coerced: NaN
          // reaches the server as `null` and comes back as a period of zero.
          if (typeof v === 'number' && Number.isFinite(v)) params[k] = v
        }
      }
      return [{ id, params }]
    })
  } catch {
    // Private mode, blocked storage, or nonsense in the slot. An empty chart
    // is the honest fallback here: these are lines the viewer asked for, and
    // guessing at them would put curves on screen nobody chose.
    return []
  }
}

function writeChoices(choices: ViewerChoice[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(choices))
  } catch {
    /* private mode: the choice lasts the page */
  }
}

/* -------------------------------------------------------------- colour */

/**
 * The same five data hues the book's lines cycle (`Desk.tsx`), read from the
 * stylesheet so neither set can drift from the theme. Lime is absent for the
 * same reason it is absent there: it is the one chrome accent, and an
 * indicator is data, not a control.
 */
const LINE_TOKENS = ['--chart-2', '--chart-3', '--chart-4', '--chart-5', '--caution']

function tokenValue(name: string): string {
  if (typeof window === 'undefined') return 'gray'
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || 'gray'
}

/* ---------------------------------------------------------------- hook */

/**
 * The prefix that keeps the two sets from colliding.
 *
 * `PriceChart` keys its series map and its live handles on
 * `indicator.key + '.' + output`. The book running `ema_21` and a viewer
 * adding `ema_21` would produce the same key twice: one handle overwrites the
 * other, the overwritten series is never removed, and the chart carries a
 * line nothing can clear. The prefix is plumbing only — `name` carries what
 * is shown.
 */
const VIEWER_PREFIX = 'you:'

export interface ViewerIndicators {
  /** What can be added. Empty until the catalog answers. */
  offered: IndicatorInfo[]
  choices: ViewerChoice[]
  /** Ready for the chart, already marked `source: 'viewer'`. */
  indicators: ActiveIndicator[]
  /** Keyed with the same prefix as `indicators`, to merge with the book's. */
  series: Record<string, IndicatorPoint[]>
  /** The timeframe the server says it computed these on. */
  computedFor: string | null
  /** A fetch that failed, verbatim, so the caption can say so. */
  error: string | null
  add: (id: string) => void
  remove: (key: string) => void
  setParam: (key: string, param: string, value: number) => void
}

/**
 * Chosen here, computed there.
 *
 * The series are never computed in this client. An indicator drawn with
 * different code from the one a strategy trades on is a lie on screen — the
 * Workbench has carried that comment since it was written, and it applies
 * twice over on a page that draws the two side by side.
 *
 * `paneBase` is how many panes the BOOK's indicators already occupy. Without
 * it the viewer's first pane would be the book's first pane, and an RSI the
 * viewer added would share a box with the MACD the bot trades — two different
 * scales stacked in one frame, which is the one way left to mislabel these
 * after everything else is marked.
 */
export function useViewerIndicators(
  market: string | null,
  timeframe: string,
  theme: 'light' | 'dark',
  paneBase: number,
): ViewerIndicators {
  const [offered, setOffered] = useState<IndicatorInfo[]>([])
  const [choices, setChoices] = useState<ViewerChoice[]>(readChoices)
  const [series, setSeries] = useState<Record<string, IndicatorPoint[]>>({})
  const [computedFor, setComputedFor] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // The Desk reads no catalog (see `App.tsx`), so this fetches its own and
  // the rest of the page never waits on it: until it answers there is simply
  // nothing to add, which is what an empty `offered` renders as.
  useEffect(() => {
    let live = true
    api
      .catalog()
      .then((c) => {
        if (live) setOffered(c.indicators)
      })
      .catch((err: Error) => {
        if (live) setError(err.message)
      })
    return () => {
      live = false
    }
  }, [])

  const known = useMemo(() => new Map(offered.map((i) => [i.id, i])), [offered])

  /**
   * The chosen set, resolved against the catalog.
   *
   * A stored id the server no longer offers is dropped rather than drawn from
   * memory: the client cannot compute it, so the chip would sit there
   * labelled and empty forever.
   */
  const resolved = useMemo<{ at: number; indicator: ActiveIndicator }[]>(() => {
    void theme // colours are read from the stylesheet, so a palette change re-reads them
    const out: { at: number; indicator: ActiveIndicator }[] = []
    choices.forEach((choice, at) => {
      const def = known.get(choice.id)
      if (!def) return
      const params = { ...numericParams(def), ...choice.params }
      const name = instanceKey(def, params)
      out.push({
        // WHERE IN `choices` THIS CAME FROM, carried rather than recomputed.
        // A stored id the catalog no longer offers is skipped above, so the
        // two lists are not the same length — and editing a period by
        // position in the drawn list would then rewrite the wrong row.
        at,
        indicator: {
          key: `${VIEWER_PREFIX}${name}`,
          name,
          source: 'viewer',
          id: def.id,
          params,
          outputs: def.outputs,
          pane:
            def.pane === 'pane'
              ? paneBase + out.filter((e) => e.indicator.pane > 0).length + 1
              : 0,
          color: tokenValue(LINE_TOKENS[out.length % LINE_TOKENS.length]),
        },
      })
    })
    return out
  }, [choices, known, paneBase, theme])

  const indicators = useMemo(() => resolved.map((r) => r.indicator), [resolved])

  /**
   * What is asked for, as a string.
   *
   * Serialised deliberately. The effect below must run when the SET changes
   * and not when a repaint mints a fresh array: `indicators` is rebuilt with
   * newly-read colours on every palette change, and an array dependency
   * would re-post every series to the server for a theme switch.
   */
  const request = useMemo(
    () => JSON.stringify(indicators.map((entry) => ({ id: entry.id, params: entry.params }))),
    [indicators],
  )

  useEffect(() => {
    const specs = JSON.parse(request) as { id: string; params: Record<string, number> }[]
    if (!market || specs.length === 0) {
      setSeries({})
      setComputedFor(null)
      return
    }
    let live = true
    api
      .indicators(market, timeframe, specs)
      .then((reply) => {
        if (!live) return
        setError(null)
        setComputedFor(reply.timeframe)
        // Prefixed on arrival, so the merged record the chart receives cannot
        // have one set's key silently standing for the other's series.
        const prefixed: Record<string, IndicatorPoint[]> = {}
        for (const [key, points] of Object.entries(reply.series)) {
          prefixed[`${VIEWER_PREFIX}${key}`] = points
        }
        setSeries(prefixed)
      })
      .catch((err: Error) => {
        if (!live) return
        // The lines are dropped as well as the message shown. A curve left on
        // screen from the previous timeframe, under a caption naming this
        // one, is the exact mislabelling this whole feature is arranged to
        // prevent.
        setSeries({})
        setComputedFor(null)
        setError(err.message)
      })
    return () => {
      live = false
    }
  }, [market, timeframe, request])

  const commit = useCallback((next: ViewerChoice[]) => {
    setChoices(next)
    writeChoices(next)
  }, [])

  const add = useCallback(
    (id: string) => {
      const def = known.get(id)
      if (!def) return
      const params = numericParams(def)
      const key = `${VIEWER_PREFIX}${instanceKey(def, params)}`
      // The same indicator at the same settings twice is one line drawn
      // twice; the same indicator at a different period is a different line
      // and is allowed — so the guard is on the instance key, not on the id.
      if (resolved.some((r) => r.indicator.key === key)) return
      commit([...choices, { id, params }])
    },
    [choices, commit, known, resolved],
  )

  const remove = useCallback(
    (key: string) => {
      const at = resolved.find((r) => r.indicator.key === key)?.at
      if (at == null) return
      commit(choices.filter((_, i) => i !== at))
    },
    [choices, commit, resolved],
  )

  const setParam = useCallback(
    (key: string, param: string, value: number) => {
      const at = resolved.find((r) => r.indicator.key === key)?.at
      if (at == null) return
      commit(
        choices.map((choice, i) =>
          i === at ? { ...choice, params: { ...choice.params, [param]: value } } : choice,
        ),
      )
    },
    [choices, commit, resolved],
  )

  return { offered, choices, indicators, series, computedFor, error, add, remove, setParam }
}

/** The numeric half of a definition's defaults. A `source: "close"` is not a
 *  thing this picker can offer a spinner for, and is left to the server. */
function numericParams(def: IndicatorInfo): Record<string, number> {
  return Object.fromEntries(
    Object.entries(def.params).filter(([, v]) => typeof v === 'number'),
  ) as Record<string, number>
}

/**
 * The instance name, built exactly as the Workbench builds it — `ema_21`,
 * `keltner_20_10_1.5` — because the server answers on these keys and the two
 * screens must ask the same question the same way.
 */
function instanceKey(def: IndicatorInfo, params: Record<string, number>): string {
  // `def.paramOrder`, NOT `Object.keys(def.params)`: the server serves the
  // defaults in a sorted map and the key is built in the order the
  // definition declares them. For macd those differ - fast, signal, slow
  // against fast, slow, signal - so a key from the map asks for
  // `macd_12_9_26` while the server answers on `macd_12_26_9`, and the line
  // never arrives. The chip, the pane and the legend all render; only the
  // data is missing, which is why it read as "nothing on the chart".
  const numeric = numericParams(def)
  const order = def.paramOrder.filter((name) => name in numeric)
  const values = order.map((k) => params[k])
  return values.length ? `${def.id}_${values.join('_')}` : def.id
}

/* ---------------------------------------------------------------- view */

/**
 * ONE BUTTON, ONE POPUP, AND EVERYTHING ON THE CHART IS CHOSEN IN IT.
 *
 * The owner's instruction on 2026-09-19, after saying the chart was drawing a
 * mess he could not read: "Giờ tất cả đều qua luồng như này — CÓ nút thêm chỉ
 * báo: chọn các chỉ báo có sẵn trong thư viện (dạng popup)". One flow. A
 * button that adds an indicator, and behind it the library of what there is.
 *
 * WHAT WENT AWAY. A 130px select in the chart header that could show one
 * entry at a time, beside a `+ add` that did nothing until something was
 * picked, beside a three-position `levels` switch that belonged to a
 * different control entirely. Three ways to put something on one canvas, two
 * of them invisible until opened. The select is gone and the level switch has
 * moved INSIDE the popup, which is the part of the instruction that needed
 * doing rather than agreeing with: a library that lists the indicators and
 * leaves the levels on a switch outside it is not one flow, it is two.
 *
 * WHAT DID NOT GO AWAY, and the reasons are at the top of this file and in
 * `LevelToggles.tsx`. The viewer's lines are still `source: 'viewer'`, still
 * dashed, still named `yours · …`; colour is still not the channel. And the
 * `LevelLegend` stays OUTSIDE, on the chart, because merging the choice must
 * not merge the meanings — a hue is unreadable in a popup that is shut.
 *
 * THE POPUP IS NOT A RECOMMENDATION. The entries are in the order the server
 * serves them, which is the order the crate declares them; nothing is sorted
 * by anything, nothing is preferred, and the only thing beside a name is what
 * this desk actually found out — the verdict, where a registration exists,
 * and what the LINE does, where somebody measured it. Thirty-one of the
 * thirty-nine constructs in `verdicts.ts` are killed, and the moment of
 * choosing is the last moment that fact can still change a mind.
 */

/** A number the catalog serves as a parameter default, printed as given. */
function defaultsLine(def: IndicatorInfo): string {
  const names = def.paramOrder.filter((n) => n in def.params)
  return names.map((n) => `${n} ${def.params[n]}`).join(' · ')
}

/**
 * The verdict, in the library's column.
 *
 * `null` is rendered as "no record on this desk" and never as a gap, which
 * is the rule `verdictFor`'s own doc comment states: every one of the fifteen
 * ids resolves today, so this branch is for the day the crate ships a
 * sixteenth before this desk has written its row.
 */
function VerdictCell({ id }: { id: string }) {
  const v = verdictFor(id)
  if (!v) {
    return (
      <span
        className="text-caution fd-caption"
        title="This chart has no measured record for this construct at all. That is not a pass: it means nobody here has registered a rule on it or closed one."
      >
        {LIBRARY_COPY.noRecord}
      </span>
    )
  }
  return (
    <span
      className={cn('fd-caption', v.status === 'killed' ? 'text-caution' : 'text-muted-foreground')}
      title={v.registration ? `${v.detail}\n\n${v.registration}` : v.detail}
    >
      {v.status} — {v.line}
    </span>
  )
}

/** One definition in the library: what it is, what it costs, and an add. */
function LibraryRow({
  def,
  timeframe,
  on,
  onAdd,
}: {
  def: IndicatorInfo
  timeframe: string
  /** True when an instance of this definition is already drawn. */
  on: boolean
  onAdd: (id: string) => void
}) {
  const params = defaultsLine(def)
  return (
    <div className="border-border/60 flex items-start gap-2 border-b py-1.5 last:border-b-0">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-baseline gap-x-2">
          <span className="fd-body">{def.name}</span>
          <span className="text-muted-foreground/70 num fd-caption">{def.id}</span>
          <span
            className="text-muted-foreground/70 num fd-caption"
            title={
              params
                ? `${LIBRARY_COPY.paramsNote}. The instance key is built in this order — ${def.paramOrder.join(', ')} — because that is the order the server answers on.`
                : 'This definition takes no numeric parameter.'
            }
          >
            {params}
          </span>
          <span className="text-muted-foreground/50 fd-caption">
            {def.pane === 'pane' ? 'own pane' : 'over the candles'}
          </span>
        </div>
        <div className="mt-px flex flex-col items-start gap-px">
          <VerdictCell id={def.id} />
          <Measured cell={measuredFor(def, timeframe, {})} absent="say" />
        </div>
      </div>
      <button
        type="button"
        disabled={on}
        onClick={() => onAdd(def.id)}
        className="hover:bg-accent focus-visible:ring-ring border-border text-muted-foreground mt-px shrink-0 rounded-sm border px-1.5 py-px fd-caption transition-colors focus-visible:ring-2 focus-visible:outline-none disabled:opacity-40 motion-reduce:transition-none"
        title={
          on
            ? `${def.name} is already on the chart at these settings. Change a period on its chip above and it becomes a different line, which can be added again.`
            : `Draw ${def.name} on the ${timeframe} candles on screen, computed by the server for ${timeframe}, dashed and named "yours"`
        }
      >
        {on ? 'on chart' : 'add'}
      </button>
    </div>
  )
}

/** A heading with the kind of thing under it said once, not per row. */
function GroupHead({ heading, blurb }: { heading: string; blurb: string }) {
  return (
    <div className="mb-1">
      <h3 className="fd-label text-foreground">{heading}</h3>
      <p className="text-muted-foreground/70 fd-caption">{blurb}</p>
    </div>
  )
}

export interface LibraryLevels {
  mode: LevelMode
  onMode: (next: LevelMode) => void
  /** Every level on the response, for the switch's `everything` position. */
  total?: number
  /** How many of them the current position draws. */
  drawn?: number
  /** How many of each family the response carries, where one has arrived. */
  counts?: Map<LevelFamily, number>
}

/**
 * The chart's library, and the one button that opens it.
 *
 * `offered` and `chosen` are the viewer's own set, exactly as before.
 * `book` is the RUN's set, listed read-only: a popup headed "on the chart
 * now" that showed half of what is on the chart would be the same untruth
 * the dashed/solid split exists to prevent, and these cannot be removed here
 * because they are the record of what the bot read.
 */
export function IndicatorLibrary({
  offered,
  chosen,
  book,
  computedFor,
  timeframe,
  error,
  levels,
  onAdd,
  onRemove,
  onParam,
}: {
  offered: IndicatorInfo[]
  chosen: ActiveIndicator[]
  book: ActiveIndicator[]
  computedFor: string | null
  timeframe: string
  error: string | null
  levels: LibraryLevels
  onAdd: (id: string) => void
  onRemove: (key: string) => void
  onParam: (key: string, param: string, value: number) => void
}) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')

  /**
   * A FILTER, AND IT WAS A CLOSE CALL. Fifteen definitions plus six families
   * is twenty-one entries, which a list does not need a search box for — but
   * each entry here is four lines tall, because a name without its verdict
   * and its measurement is the implicit recommendation this file exists to
   * refuse. Twenty-one four-line entries is three screens, and three screens
   * is where a reader who came to add the MACD starts scrolling. It filters
   * on the words already on screen — name, id, family label — and never on a
   * hidden keyword list, so a reader can always see why something matched.
   */
  const q = query.trim().toLowerCase()
  const matches = (...fields: string[]) =>
    q === '' || fields.some((f) => f.toLowerCase().includes(q))

  const groups = useMemo(() => {
    const method = new Set(METHOD_IDS)
    return {
      readings: offered.filter((d) => !method.has(d.id)),
      methods: offered.filter((d) => method.has(d.id)),
    }
  }, [offered])

  const shown = {
    readings: groups.readings.filter((d) => matches(d.name, d.id)),
    methods: groups.methods.filter((d) => matches(d.name, d.id)),
    families: LEVEL_FAMILIES.filter((f) => matches(f.label, f.note, 'levels', 'structure')),
  }
  const nothing =
    shown.readings.length === 0 && shown.methods.length === 0 && shown.families.length === 0

  /** Which definitions already have an instance drawn, for the row's state. */
  const onChart = new Set(chosen.map((c) => c.id))

  // How much is on, for the button — a reader should not have to open the
  // popup to find out whether anything is. Levels count as one thing here
  // because one switch draws them; the number beside `everything` inside is
  // where the 252 lives, and putting it on the button would be two units in
  // one count.
  const onCount = chosen.length + (levels.mode === 'off' ? 0 : 1)

  return (
    <span className="ml-2 inline-flex flex-wrap items-center gap-1 align-middle normal-case">
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger
          className={cn(
            'hover:bg-accent focus-visible:ring-ring rounded-sm border px-1.5 py-px fd-caption transition-colors focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
            onCount > 0 ? 'border-primary/40 text-primary' : 'border-border text-muted-foreground',
          )}
          title="Open the library: every indicator this server can compute, the level ladder, and what this desk has measured about each of them"
        >
          {LIBRARY_COPY.button}
          {onCount > 0 && <span className="num text-muted-foreground/70"> {onCount}</span>}
        </DialogTrigger>
        <DialogContent aria-describedby="fd-library-what">
          <DialogHeader>
            <DialogTitle>{LIBRARY_COPY.title}</DialogTitle>
            <DialogDescription id="fd-library-what">{LIBRARY_COPY.description}</DialogDescription>
          </DialogHeader>

          <div className="border-border flex items-center gap-2 border-b px-3 py-1.5">
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={LIBRARY_COPY.filter}
              aria-label="Filter the library"
              className="border-border bg-background/40 focus-visible:ring-ring w-40 rounded-sm border px-1.5 py-px fd-caption focus-visible:ring-2 focus-visible:outline-none"
            />
            {error && <span className="text-caution fd-caption">{error}</span>}
            {computedFor && computedFor !== timeframe && (
              <span className="text-caution fd-caption">
                the server computed these for {computedFor}, not {timeframe}
              </span>
            )}
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto px-3 py-2">
            {/* WHAT IS ON, AT THE TOP, AND TAKEN OFF HERE TOO. A library
                that can only add is a library a reader has to leave to undo
                anything, which is the second flow the popup exists to
                remove. */}
            <section className="mb-3">
              <h3 className="fd-label text-foreground mb-1">{LIBRARY_COPY.onChart}</h3>
              <div className="flex flex-wrap items-center gap-1">
                {levels.mode !== 'off' && (
                  <span className="border-border num inline-flex items-center gap-1 rounded-sm border px-1 py-px fd-caption">
                    <span className="text-muted-foreground/70">levels</span>
                    {levels.mode}
                    {levels.drawn != null && (
                      <span className="text-muted-foreground/50">{levels.drawn}</span>
                    )}
                    <button
                      type="button"
                      aria-label="Draw no levels"
                      className="hover:text-lp px-0.5"
                      onClick={() => levels.onMode('off')}
                    >
                      ×
                    </button>
                  </span>
                )}
                {chosen.map((entry, index) => (
                  <span
                    // KEYED ON THE SLOT, NOT ON THE INSTANCE KEY. The key
                    // contains the periods, so typing `21` into a period box
                    // would change it twice, remount the chip twice, and take
                    // the caret out of the box after each digit.
                    key={`slot-${index}`}
                    className="num inline-flex items-center gap-1 rounded-sm border px-1 py-px fd-caption"
                    style={{ borderColor: entry.color, color: entry.color }}
                  >
                    {/* The word rides on the chip itself, not on a heading
                        over the row: this is the set drawn dashed and named
                        `yours` on the chart, and the reader must be able to
                        tell from THIS chip which lines it owns. */}
                    <span className="text-muted-foreground/70">yours</span>
                    {entry.name ?? entry.key}
                    {Object.entries(entry.params).map(([param, value]) => (
                      <input
                        key={param}
                        type="number"
                        value={value}
                        step="any"
                        aria-label={`${entry.name ?? entry.key} ${param}`}
                        title={`${param} — the period this line is drawn with, on the ${computedFor ?? timeframe} candles`}
                        onChange={(e) => {
                          const next = Number(e.target.value)
                          // An empty box is not zero. Sending 0 would ask the
                          // server for a period of nothing and draw whatever
                          // it returned.
                          if (e.target.value !== '' && Number.isFinite(next)) {
                            onParam(entry.key, param, next)
                          }
                        }}
                        className="border-border bg-background/40 w-11 rounded-sm border px-0.5 text-right"
                      />
                    ))}
                    <MeasuredChip
                      cell={measuredFor(
                        offered.find((i) => i.id === entry.id),
                        timeframe,
                        entry.params,
                      )}
                    />
                    <button
                      type="button"
                      aria-label={`Remove ${entry.name ?? entry.key}`}
                      className="hover:text-lp px-0.5"
                      onClick={() => onRemove(entry.key)}
                    >
                      ×
                    </button>
                  </span>
                ))}
                {chosen.length === 0 && levels.mode === 'off' && (
                  <span className="text-muted-foreground/70 fd-caption">
                    {LIBRARY_COPY.onChartEmpty}
                  </span>
                )}
              </div>
              {book.length > 0 && (
                <div className="mt-1 flex flex-wrap items-center gap-1">
                  {book.map((entry) => (
                    <span
                      key={entry.key}
                      className="num inline-flex items-center gap-1 rounded-sm border px-1 py-px fd-caption"
                      style={{ borderColor: entry.color, color: entry.color }}
                      title={LIBRARY_COPY.bookNote}
                    >
                      <span className="text-muted-foreground/70">book</span>
                      {entry.name ?? entry.key}
                    </span>
                  ))}
                  <span className="text-muted-foreground/70 fd-caption">
                    {LIBRARY_COPY.bookNote}
                  </span>
                </div>
              )}
            </section>

            {nothing && <p className="text-muted-foreground fd-caption">{LIBRARY_COPY.filterEmpty}</p>}

            {LIBRARY_GROUPS.map((group) => {
              if (group.key === 'levels') {
                if (shown.families.length === 0) return null
                return (
                  <section key={group.key} className="mb-3">
                    <GroupHead heading={group.heading} blurb={group.blurb} />
                    <p className="text-muted-foreground/70 mb-1 fd-caption">
                      {LIBRARY_COPY.levelsNote}
                    </p>
                    {/* THE SAME SWITCH, MOVED. Not a copy of it: the
                        radiogroup, its roving tabindex and its titles are
                        `LevelToggles.tsx`'s, so the control a reader learns
                        here is the control that was in the header. */}
                    <LevelSwitch
                      mode={levels.mode}
                      onChange={levels.onMode}
                      total={levels.total}
                      drawn={levels.drawn}
                    />
                    <ul className="mt-1.5">
                      {shown.families.map((family) => {
                        const n = levels.counts?.get(family.key)
                        return (
                          <li
                            key={family.key}
                            className="border-border/60 flex items-start gap-2 border-b py-1 last:border-b-0"
                          >
                            <span
                              aria-hidden
                              className="mt-1 size-1.5 shrink-0 rounded-full"
                              style={{ backgroundColor: family.hue }}
                            />
                            <span className="min-w-0 flex-1">
                              <span className="fd-body">{family.label}</span>
                              {n != null && (
                                <span className="text-muted-foreground/70 num fd-caption">
                                  {' '}
                                  {n} on this response
                                </span>
                              )}
                              <span className="text-muted-foreground/70 block fd-caption">
                                {family.note}
                              </span>
                            </span>
                          </li>
                        )
                      })}
                    </ul>
                    <p className="text-muted-foreground/70 mt-1 fd-caption">
                      {LIBRARY_COPY.familiesNote}
                    </p>
                  </section>
                )
              }
              const defs = group.key === 'readings' ? shown.readings : shown.methods
              if (defs.length === 0) return null
              return (
                <section key={group.key} className="mb-3">
                  <GroupHead heading={group.heading} blurb={group.blurb} />
                  {defs.map((def) => (
                    <LibraryRow
                      key={def.id}
                      def={def}
                      timeframe={timeframe}
                      on={onChart.has(def.id)}
                      onAdd={onAdd}
                    />
                  ))}
                </section>
              )
            })}
          </div>
        </DialogContent>
      </Dialog>
      {/* THE FAILURE IS SAID IN THE HEADER TOO, not only behind a button.
          A catalog that never answered leaves a library with nothing in it,
          and a reader who has not opened it would otherwise see a working
          control over an empty chart. */}
      {error && <span className="text-caution fd-caption">indicators: {error}</span>}
    </span>
  )
}
