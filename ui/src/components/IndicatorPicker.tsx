import { useCallback, useEffect, useMemo, useState } from 'react'

import { api, type IndicatorInfo, type IndicatorPoint } from '@/lib/api'
import type { ActiveIndicator } from '@/components/PriceChart'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { cn } from '@/lib/utils'

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

/**
 * STUB. `ui/src/lib/verdicts.ts` is being written by another session this
 * round and lands first; when it does, this whole block is deleted and
 * replaced by one line:
 *
 *     import { verdictFor, type Verdict } from '@/lib/verdicts'
 *
 * The interface below is the agreed shape, copied exactly, so the swap is a
 * deletion and not a rewrite. The five entries are the registrations this
 * session could read receipts for in `docs/decisions/`; the real table has
 * about forty.
 */
export interface Verdict {
  id: string
  kind: 'indicator' | 'strategy'
  status: 'killed' | 'open' | 'parked' | 'untested'
  line: string
  detail: string
  registration: string | null
}

/**
 * What was measured when somebody tried to TRADE these lines as a rule.
 *
 * Every figure here is copied from a closed registration in `docs/decisions/`,
 * with the file named in `registration` so it can be checked. Nothing is
 * inferred and nothing is rounded into a kinder shape: `keltner` reads 1.052
 * because that is the number in the table, and it sits at the 78th percentile
 * of its own null, which is the reason it failed rather than a detail about
 * how it failed.
 */
const STUB_VERDICTS: Verdict[] = [
  {
    id: 'ema',
    kind: 'indicator',
    status: 'killed',
    line: 'killed out of sample · PF 0.379 over 5,766 trades, 0th percentile',
    detail:
      'Trend pullback — the first close back through EMA20 in the direction of a rising EMA200 — replayed at profit factor 0.379 over 5,766 gold trades, the 0th percentile of its count-matched null, losing 0.39R a time. BTC 0.511 on 2,931 trades, also 0th. Closed on both primaries 2026-09-13.',
    registration: 'docs/decisions/2026-09-13-trend-pullback.md',
  },
  {
    id: 'keltner',
    kind: 'indicator',
    status: 'killed',
    line: 'killed out of sample · PF 1.052 over 533 trades, 78th percentile',
    detail:
      'Keltner break, all day, Vantage xauusd 15m over the last year: 533 trades, walk-forward PF 1.052, expectancy 0.028R — against a null whose own p50 is 0.965 and p95 1.210. The 78th percentile is inside the noise, which is the whole finding: the profit factor above 1 is what luck looks like here.',
    registration: 'docs/decisions/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'macd',
    kind: 'indicator',
    status: 'killed',
    line: 'killed out of sample · PF 1.010 over 923 trades, 66th percentile',
    detail:
      'MACD cross, all day, Vantage xauusd 15m over the last year: 923 trades, walk-forward PF 1.010, 66th percentile of its null (p50 0.965, p95 1.210). The Asian-session row reached the 98th on 300 trades but its direction null reads only the 93rd, and the screen expected eight or nine passes by luck out of about 170 rows.',
    registration: 'docs/decisions/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'rsi',
    kind: 'indicator',
    status: 'killed',
    line: 'killed out of sample · PF 1.110 over 198 trades, 74th percentile',
    detail:
      'RSI(2) pullback, New York hours, Vantage xauusd 15m over the last year: 198 trades, walk-forward PF 1.110, 74th percentile of its null. One of about 170 rows in a screen where a single row passed and eight or nine passes were expected from luck alone.',
    registration: 'docs/decisions/2026-09-13-recent-year-screen.md',
  },
  {
    id: 'donchian',
    kind: 'indicator',
    status: 'killed',
    line: 'killed out of sample · PF 0.979 over 519 trades, 58th percentile',
    detail:
      'Donchian breakout, all day, Vantage xauusd 15m over the last year: 519 trades, walk-forward PF 0.979 — it loses money before the null is even consulted, and the null puts it at the 58th. Breakouts were closed as a family on this desk before the screen ran.',
    registration: 'docs/decisions/2026-09-13-recent-year-screen.md',
  },
]

/**
 * The registration against an indicator id, or `null`.
 *
 * `null` MEANS NO REGISTRATION, and renders as nothing at all. It must never
 * become a badge saying "untested" in a reassuring grey, because an absence
 * of evidence drawn in the same slot as evidence reads as evidence.
 */
export function verdictFor(id: string): Verdict | null {
  return STUB_VERDICTS.find((v) => v.id === id) ?? null
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
  const values = Object.keys(numericParams(def)).map((k) => params[k])
  return values.length ? `${def.id}_${values.join('_')}` : def.id
}

/* ---------------------------------------------------------------- view */

/**
 * Pick an indicator, set its periods, take it off again.
 *
 * The mechanics are the Workbench's — a select, an add, a chip per instance
 * with a remove — because that picker works and a second idiom for the same
 * job is a second thing to learn. The layout is the Desk's: one dense row in
 * the chart header, no cards.
 *
 * WHAT IS DIFFERENT IS THE VERDICT. Every registered attempt to trade these
 * lines as a rule has been killed out of sample, and an indicator offered on
 * a chart with nothing beside it is an implicit recommendation. So the
 * measured outcome sits in the menu where the choice is made, with the whole
 * paragraph on hover — not in a footnote under the chart, where it would be
 * read after the line was already drawn and believed.
 */
export function IndicatorPicker({
  offered,
  chosen,
  computedFor,
  timeframe,
  error,
  onAdd,
  onRemove,
  onParam,
}: {
  offered: IndicatorInfo[]
  chosen: ActiveIndicator[]
  computedFor: string | null
  timeframe: string
  error: string | null
  onAdd: (id: string) => void
  onRemove: (key: string) => void
  onParam: (key: string, param: string, value: number) => void
}) {
  const [pick, setPick] = useState('')

  if (offered.length === 0 && chosen.length === 0) {
    // Nothing to offer yet, or a server that has no catalog route. Either way
    // an empty select is a control that does nothing, so there is none.
    return error ? <span className="text-caution ml-2 fd-caption normal-case">indicators: {error}</span> : null
  }

  const verdict = pick ? verdictFor(pick) : null

  return (
    <span className="ml-2 inline-flex flex-wrap items-center gap-1 align-middle normal-case">
      <span className="text-muted-foreground/50 fd-caption">yours</span>
      <Select value={pick} onValueChange={setPick}>
        <SelectTrigger size="sm" className="h-5 w-[130px] fd-caption" aria-label="indicator to add">
          <SelectValue placeholder="add a line…" />
        </SelectTrigger>
        <SelectContent className="max-w-[380px]">
          {offered.map((info) => {
            const v = verdictFor(info.id)
            return (
              <SelectItem key={info.id} value={info.id} title={v?.detail}>
                <span className="flex flex-col items-start">
                  <span>{info.name}</span>
                  {/* Only when there IS a registration. No badge, no dash and
                      no grey "untested" for the rest: a blank in the slot
                      where evidence goes is read as evidence. */}
                  {v && (
                    <span className={cn('fd-caption', v.status === 'killed' ? 'text-caution' : 'text-muted-foreground')}>
                      {v.line}
                    </span>
                  )}
                </span>
              </SelectItem>
            )
          })}
        </SelectContent>
      </Select>
      <button
        type="button"
        disabled={!pick}
        onClick={() => {
          if (pick) onAdd(pick)
        }}
        className="hover:bg-accent focus-visible:ring-ring border-border text-muted-foreground rounded-sm border px-1.5 py-px fd-caption transition-colors focus-visible:ring-2 focus-visible:outline-none disabled:opacity-40 motion-reduce:transition-none"
        title={
          pick
            ? `Draw ${pick} on the ${timeframe} candles on screen, computed for ${timeframe}`
            : 'Pick an indicator first'
        }
      >
        + add
      </button>
      {/* The verdict for the highlighted choice, in the header, before it is
          added. `line` here, `detail` on hover — the agreed split. */}
      {verdict && (
        <span
          className={cn('fd-caption', verdict.status === 'killed' ? 'text-caution' : 'text-muted-foreground')}
          title={verdict.registration ? `${verdict.detail}\n\n${verdict.registration}` : verdict.detail}
        >
          {pick}: {verdict.line}
        </span>
      )}
      {chosen.map((entry, index) => {
        const v = verdictFor(entry.id)
        return (
          <span
            // KEYED ON THE SLOT, NOT ON THE INSTANCE KEY. The key contains
            // the periods, so typing `21` into a period box would change it
            // twice, remount the chip twice, and take the caret out of the
            // box after each digit.
            key={`slot-${index}`}
            className="num inline-flex items-center gap-1 rounded-sm border px-1 py-px fd-caption"
            style={{ borderColor: entry.color, color: entry.color }}
          >
            {/* The word rides on the chip itself, not on a heading over the
                row: this is the set that is drawn dashed and named `yours`
                on the chart, and the reader must be able to tell from THIS
                chip which lines it owns. */}
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
                  // An empty box is not zero. Sending 0 would ask the server
                  // for a period of nothing and draw whatever it returns.
                  if (e.target.value !== '' && Number.isFinite(next)) onParam(entry.key, param, next)
                }}
                className="border-border bg-background/40 w-11 rounded-sm border px-0.5 text-right"
              />
            ))}
            {v && (
              <span
                className={v.status === 'killed' ? 'text-caution' : 'text-muted-foreground'}
                title={v.registration ? `${v.detail}\n\n${v.registration}` : v.detail}
              >
                {v.status}
              </span>
            )}
            <button
              type="button"
              aria-label={`Remove ${entry.name ?? entry.key}`}
              className="hover:text-lp px-0.5"
              onClick={() => onRemove(entry.key)}
            >
              ×
            </button>
          </span>
        )
      })}
      {error && <span className="text-caution fd-caption">{error}</span>}
    </span>
  )
}
