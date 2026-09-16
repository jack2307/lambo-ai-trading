/**
 * The Desk — every paper run at once.
 *
 * Terminal mode (see `ui/DESIGN.md`): dense, flat, small radius, no cards, no
 * hero. The table fills the viewport and a drill-down pane sits beside it from
 * 1280px up, below it under that. Its job is the opposite of the Research
 * screen's: someone already working reads ten rows of numbers without
 * scrolling, and reaches one run's fills in a keystroke.
 *
 * ```
 * ┌ summary strip ───────────────────────────────────────────────┐
 * ├ runs table (fills the width) ──────────┬ drill-down ─────────┤
 * │ one row per run, sorted by id           │ equity curve        │
 * ├ chart: the run's own candles,           │ fills · events      │
 * │ indicators, fills, stop / target bands  │                     │
 * ├ fills — pick one and it is read out ────┤                     │
 * └─────────────────────────────────────────┴─────────────────────┘
 * ```
 *
 * Picking a fill opens a plain-language account of it under the table: what the
 * strategy saw, where the entry, stop and target sat, and what actually closed
 * it. No fill and no run total is evidence a strategy works, and the account
 * says so under any run with fewer than thirty of them.
 *
 * Every figure carries its unit, because `1.03` is not a profit factor and
 * `−277` is not a loss until it says dollars.
 *
 * One number on this screen is not a decision: the live price. The bot steps
 * on closed bars, so between two of them every figure here is up to fifteen
 * minutes old — which reads as a dead feed. `run.live` is the bar still
 * forming, posted straight to the server by the same poller and kept out of
 * every book; it is drawn so the screen moves, labelled so nobody mistakes it
 * for something the bot acted on, and dropped by the server after ninety
 * seconds so a stopped poller shows as no price rather than a frozen one.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { PriceChart, type ActiveIndicator } from '@/components/PriceChart'
import { Skeleton } from '@/components/ui/skeleton'
import type { Book } from '@/App'
import { api, type BacktestTrade, type Bar, type LiveBar, type PaperBroker, type PaperEvent, type PaperRun, type PaperRunDetail } from '@/lib/api'
import { clock, num } from '@/lib/format'
import { useTicks } from '@/lib/ticks'
import { ClaudeMark, DeepSeekMark, OpenAIMark } from '@/components/BrandMarks'
import type { Consultation, Decision, Reasoning } from '@/lib/api'
import { cn } from '@/lib/utils'

/** Height of the sticky app bar, which this page fills the rest of the viewport under. */
const APP_BAR = 45

/**
 * Bars the drill-down asks for.
 *
 * `PriceChart` opens on its last 140 candles, so anything under that leaves the
 * chart with nothing to scroll back into. 240 is a little under two days of 15m
 * bars — enough context around the newest fills without making the poll heavy.
 */
const DETAIL_BARS = 240

/** The chart's pane, in px. Fixed, because a chart that changes height on data reflows the column. */
const CHART_H = 380

/**
 * How many of a run's OWN bars may pass before it is called stale.
 *
 * Bars rather than minutes, and that is the whole point. A flat 45-minute
 * threshold is three missed bars on a 15-minute book and **nine** on a
 * five-minute one, so the two five-minute books could lose most of an hour of
 * feed and still show a reassuring pulsing green dot. The reader wants the
 * same question answered for every row — "is this one still being fed" — and
 * the only unit in which that question means the same thing on both books is
 * the book's own bar.
 */
const STALE_BARS = 3

/** The timeframes a run can carry, in milliseconds. */
const TF_MS: Record<string, number> = { '1m': 60_000, '5m': 300_000, '15m': 900_000, '1h': 3_600_000 }

/** Fallback for a timeframe this table does not know: the old flat window. */
const STALE_MS_FALLBACK = 45 * 60_000

/** The selected run, remembered per browser. A reload should land where you were. */
const SELECTED_KEY = 'fd.desk.selected'

/** `/status` sends the last ten fills per run; a count from them can only be a floor. */
const LAST_FILLS_CAP = 10

const readSelected = (): string | null => {
  try {
    return localStorage.getItem(SELECTED_KEY)
  } catch {
    return null
  }
}
const writeSelected = (id: string) => {
  try {
    localStorage.setItem(SELECTED_KEY, id)
  } catch {
    /* private mode: the choice lasts the page */
  }
}

/* ------------------------------------------------------------- formatting */

const MINUS = '−'

/**
 * What a position of this size ties up, and how far the account is from a
 * margin call.
 *
 * Margin is `lots x contract_size x price / leverage`, which is what the
 * terminal returns for `order_calc_margin` — checked against it on 2026-09-16:
 * 0.10 lots of XAUUSD.sc came back 21.63 USC and this gives 21.63.
 *
 * The desk never SIZES on margin, it sizes on risk. This is here because "we
 * are nowhere near a margin call" is itself a number worth being able to see,
 * and because at 1:2000 it is very easy to assume otherwise.
 */
function marginOf(lots: number, price: number, run: PaperRun): { used: number; pct: number; level: number } | null {
  if (!run.leverage || !run.contract_size || !Number.isFinite(price) || price <= 0) return null
  const used = (lots * run.contract_size * price) / run.leverage
  return {
    used,
    pct: run.equity > 0 ? (100 * used) / run.equity : 0,
    level: used > 0 ? (100 * run.equity) / used : Infinity,
  }
}

/**
 * A money figure in the ACCOUNT's own unit.
 *
 * The wire is USD everywhere, because `lots x contract_size x price` is USD
 * and nothing else. The live Vantage books are a cent account: the same money,
 * counted in hundredths. Converting here and nowhere else is deliberate — a
 * factor of a hundred loose in the arithmetic would multiply through every
 * cost, every guard and every receipt.
 */
function accountMoney(usd: number | null | undefined, run: { account_currency?: string; units_per_usd?: number } | null | undefined, signed = true): string {
  if (usd == null || !Number.isFinite(usd)) return '—'
  const per = run?.units_per_usd && run.units_per_usd > 0 ? run.units_per_usd : 1
  const cur = run?.account_currency || 'USD'
  const v = usd * per
  const digits = Math.abs(v) >= 1000 || per > 1 ? 0 : 0
  const body = Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: digits })
  const sign = signed ? (v < 0 ? MINUS : '+') : v < 0 ? MINUS : ''
  return cur === 'USD' ? `${sign}$${body}` : `${sign}${body} ${cur}`
}

/** A signed dollar figure. The sign is the point, so it is never dropped. */
const signedUsd = (v: number | null | undefined): string => {
  if (v == null || !Number.isFinite(v)) return '—'
  const rounded = Math.abs(v) >= 1000 ? Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: 0 }) : Math.abs(v).toFixed(0)
  return `${v < 0 ? MINUS : '+'}$${rounded}`
}

const signedR = (v: number | null | undefined): string =>
  v == null || !Number.isFinite(v) ? '—' : `${v < 0 ? MINUS : '+'}${Math.abs(v).toFixed(2)}R`

const ago = (ms: number | null | undefined, now: number): string => {
  if (!ms) return '—'
  const minutes = Math.max(0, Math.round((now - ms) / 60_000))
  if (minutes < 60) return `${minutes} min`
  if (minutes < 48 * 60) return `${(minutes / 60).toFixed(1)} h`
  return `${Math.round(minutes / 1440)} d`
}

/**
 * How old the live price is, in seconds.
 *
 * Seconds, not the minutes `ago` deals in: the server drops a tick older than
 * ninety seconds, so anything shown here is inside a minute and a half, and
 * "0 min" would say nothing about whether the feed is moving.
 */
const liveAge = (at: number, now: number): string => `${Math.max(0, Math.round((now - at) / 1000))} s ago`

/**
 * `ask − bid`, at the precision the price itself is quoted in.
 *
 * A gold spread printed by `quote` would read `0.25000`, because `quote`
 * chooses its decimals from the magnitude and a spread is a small number in a
 * large market. It takes its decimals from the close instead.
 */
const spreadOf = (live: LiveBar): string | null => {
  if (live.bid == null || live.ask == null || !Number.isFinite(live.bid) || !Number.isFinite(live.ask)) return null
  const decimals = (quote(live.close).split('.')[1] ?? '').length
  return (live.ask - live.bid).toFixed(decimals)
}

/**
 * Which way the forming bar has gone since the bot last decided anything:
 * `1`, `-1` or `0` against the previous closed bar's close.
 */
const liveDirection = (live: LiveBar, lastClose: number | null): number => {
  if (lastClose == null || !Number.isFinite(lastClose)) return 0
  return Math.sign(live.close - lastClose)
}

/** Up green, down red — the data colours, never the accent, which means selection here. */
const DIRECTION_CLASS = ['text-lp', 'text-foreground', 'text-lc']
/** The arrow carries the direction too, so colour is not the only channel. */
const DIRECTION_MARK = ['▾', '·', '▴']

/** `HH:MMZ` — the wall clock the whole product reads in, UTC. */
const zulu = (ms: number | null | undefined): string =>
  ms == null || !Number.isFinite(ms) ? '—' : `${clock(ms)}Z`

/**
 * A price at the precision its own market quotes in. `format.price` rounds for
 * a gold chart; a euro printed to one decimal is 1.2, which is not a price.
 */
const quote = (v: number | null | undefined): string => {
  if (v == null || !Number.isFinite(v)) return '—'
  const magnitude = Math.abs(v)
  if (magnitude >= 1000) return v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  if (magnitude >= 10) return v.toFixed(2)
  return v.toFixed(5)
}

/** Vietnam is UTC+7 all year — no daylight saving, so one constant is correct. */
const VN_OFFSET_MS = 7 * 60 * 60 * 1000

/**
 * `MM-DD HH:MM:SS` in Vietnam time, for a moment on a wall clock.
 *
 * Used for when something HAPPENED — a model answered, a trade closed. Bar
 * stamps keep their UTC `Z` deliberately: the prompt shows the model UTC bars
 * and its own sentences quote them back ("the 02:30 breakout"), so relabelling
 * a bar would put the desk and the model's own words in different hours.
 */
const vnStamp = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms)) return '—'
  const iso = new Date(ms + VN_OFFSET_MS).toISOString()
  return `${iso.slice(5, 10)} ${iso.slice(11, 19)}`
}

/** The same moment in UTC, for a tooltip beside the local one. */
const utcStamp = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms)) return '—'
  return `${new Date(ms).toISOString().slice(5, 19).replace('T', ' ')}Z`
}

/** `MM-DD HH:MMZ`, short enough for an axis label and a fills column. */
const shortStamp = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms)) return '—'
  const iso = new Date(ms).toISOString()
  return `${iso.slice(5, 10)} ${iso.slice(11, 16)}Z`
}

/** Midnight UTC today, the boundary "closed today" is measured from. */
const startOfDayUtc = (now: number): number => {
  const d = new Date(now)
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate())
}

/**
 * Guard labels, short. The wire spells them `WEEKEND_FLAT`; a chip has room
 * for `weekend` and the column header already says these are guards.
 */
const GUARD_LABEL: Record<string, string> = {
  WEEKEND_FLAT: 'weekend',
  NEWS_FLAT: 'news',
  OPEN_LOSS_CAP: 'open loss',
  DAILY_TRADE_CAP: 'daily cap',
  DAILY_LOSS_LIMIT: 'loss limit',
  MAX_CONCURRENT: 'concurrent',
  NOTIONAL_CAP: 'notional',
  COOLDOWN: 'cooldown',
  NO_RISK_UNIT: 'no risk unit',
}
const guardLabel = (key: string) => GUARD_LABEL[key] ?? key.toLowerCase().replace(/_/g, ' ')

const staleAfter = (tf: string): number => (TF_MS[tf] ?? 0) * STALE_BARS || STALE_MS_FALLBACK

const isStale = (run: PaperRun, now: number) =>
  run.last_bar_time ? now - run.last_bar_time > staleAfter(run.tf) : true

/* ------------------------------------------------------------------ page */

/**
 * How long an executor's snapshot stays believable. The executor polls every
 * 15 s; three missed looks is a stopped process, not a slow one. The snapshot
 * is a file and a file outlives the program that wrote it, so without this a
 * dead mirror would keep showing its last account as though it were current.
 */
const BROKER_STALE_MS = 45_000

/**
 * This run's mirror into the SELECTED account, if it is still reporting.
 *
 * Picked by login out of however many accounts carry this book. A book can run
 * on several at once, and each keeps its own record - the whole point of
 * showing one account at a time is that their fills differ.
 */
/**
 * How many closed bars a model may miss before its book reads as stopped.
 *
 * Lower than the Telegram watch's three, on purpose. The two have different
 * costs of being wrong: an alert that fires early trains someone to ignore the
 * channel, while a row that says "stopped" one bar early costs nothing and is
 * corrected the moment the model answers again. A display can afford to be
 * quicker than an alarm.
 *
 * A model that DECLINES still posts, so its badge moves on every bar it sees.
 * Missing two closes is not a slow answer.
 */
const DECIDER_STALE_BARS = 1.5

/**
 * How many of a driver's own polls it may miss before its book reads as
 * stopped.
 *
 * Three, against a cadence the driver reports itself — about ninety seconds on
 * the default thirty-second poll. Enough to ride out one slow write or a
 * process briefly descheduled, short enough that "is anything driving this?"
 * stops being a half-hour question.
 */
const DRIVER_MISSED_POLLS = 3

/**
 * The books whose decider has gone quiet.
 *
 * The status pill says `fed`, and `fed` is about the FEED - bars are arriving.
 * A book can be fed perfectly while the model driving it is dead, and until
 * this existed the two looked identical on the desk: the AI books sat there
 * wearing their model badges for an hour after the processes were stopped.
 *
 * A coin book is judged by its model rather than by itself. Its own badge only
 * moves when it trades, so its silence means nothing on its own - but it is
 * driven by the same process as the model it controls, so when that model
 * stops, so has the coin. The `<model>-coin` naming is this repo's own
 * convention and the `--control` flag that creates the pair uses it; a coin
 * whose sibling is not found is left unjudged rather than guessed at.
 */
function silentBooks(runs: PaperRun[], now: number): Set<string> {
  const out = new Set<string>()
  const mark = (id: string) => {
    out.add(id)
    const coin = `${id}-coin`
    if (runs.some((x) => x.id === coin)) out.add(coin)
  }

  for (const r of runs) {
    // First choice: the driver's own heartbeat. It is written every poll,
    // independent of the market, so a killed process shows within a couple of
    // polls instead of within a couple of bar closes — and it still answers at
    // three in the morning with the market shut, when no bar will close for
    // days. Judged against the driver's OWN cadence, not a number baked in
    // here, so a slow-polling driver is not called dead for being slow.
    if (r.driver?.at) {
      const poll = (r.driver.poll_s ?? 30) * 1000
      if (now - r.driver.at > poll * DRIVER_MISSED_POLLS) mark(r.id)
      // A heartbeat that is present and fresh settles the question: a book with
      // a live driver is not stopped, whatever its bars say.
      continue
    }

    // Fallback, for a book whose driver predates the heartbeat or is a rule.
    // Measured against the book's own last CLOSED BAR rather than the wall
    // clock. A model answers at every close, so the gap between the last close
    // and its last word is the number of closes it missed — and over a weekend
    // or a dead feed no bar closes, so nothing is called stopped merely because
    // time passed.
    const d = r.decider
    if (!d || d.last === 'coin' || !d.last_at || !r.last_bar_time) continue
    const step = TF_MS[r.tf] ?? 900_000
    if (r.last_bar_time - d.last_at <= step * DECIDER_STALE_BARS) continue
    mark(r.id)
  }
  return out
}

const brokerLive = (run: PaperRun, now: number, login: number | null): PaperBroker | null => {
  if (login == null) return null
  const b = run.brokers?.find((x) => x.login === login)
  return b && now - b.at < BROKER_STALE_MS ? b : null
}

/**
 * The account's completed trades in the shape the chart draws.
 *
 * `r`, `stop` and `target` come back as NaN and null because the broker has
 * none of them: it knows what it filled, not what the rule intended. The chart
 * omits an R it cannot read and draws no stop band for these, which is the
 * honest rendering - a zero in those fields would look like a measurement.
 *
 * Drawn at the BROKER's prices on purpose. Against the same bars the book
 * decided on, the distance between the two entry markers is the slippage.
 */
function accountTrades(broker: PaperBroker | null): BacktestTrade[] {
  if (!broker) return []
  return (broker.fills ?? [])
    .filter((f) => f.entryTime != null && f.exitTime != null && f.entryPrice != null && f.exitPrice != null)
    .map((f) => ({
      direction: (f.direction ?? 'LONG') as 'LONG' | 'SHORT',
      entryTime: f.entryTime as number,
      entryPrice: f.entryPrice as number,
      exitTime: f.exitTime as number,
      exitPrice: f.exitPrice as number,
      exitReason: f.exitReason || 'closed',
      stop: Number.NaN,
      target: null,
      lots: f.lots ?? 0,
      pnlUsd: f.pnl ?? 0,
      r: Number.NaN,
      mae: Number.NaN,
      mfe: Number.NaN,
      holdMs: (f.exitTime as number) - (f.entryTime as number),
      reason: '',
    }))
}

/** Money in the BROKER's currency, which is not the paper book's. */
function brokerMoney(v: number | null | undefined, currency: string | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '--'
  const sign = v > 0 ? '+' : ''
  return `${sign}${v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${currency ?? ''}`.trim()
}

export function Desk({ book }: { book: Book }) {
  // Everything below asks one question of the mode - which account, or none -
  // so it is asked once here rather than re-derived at each use.
  const account = typeof book === 'number' ? book : null
  const [runs, setRuns] = useState<PaperRun[] | null>(null)
  const [statusError, setStatusError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(readSelected)
  const [detail, setDetail] = useState<PaperRunDetail | null>(null)
  // Tagged with the run it belongs to, so an error from the row you just left
  // does not sit over the row you just opened.
  const [detailError, setDetailError] = useState<{ id: string; message: string } | null>(null)
  const [now, setNow] = useState(() => Date.now())
  // Which of the two bottom panels is showing. The fills are the default
  // because they are what the book DID; the log is what it was thinking.
  const [bottomTab, setBottomTab] = useState<'fills' | 'log'>('fills')
  // The forming candle, pushed. The ten-second status poll still carries one,
  // and is still what keeps the table honest when the stream is down — this
  // only ever overrides it with something NEWER, never with something older.
  const { ticks, connected: streaming } = useTicks()
  // Which fill the explanation below the table is describing. Held here rather
  // than in the table because the table is rendered twice — once in the wide
  // layout's left column, once in the rail under it — and a reader who picks a
  // fill, then resizes, should still be reading the same fill. Tagged with the
  // run it belongs to, so a key from the run you just left is simply not the
  // run you are looking at rather than something an effect has to clear.
  const [fillPick, setFillPick] = useState<{ id: string; key: string } | null>(null)

  // The book changes when a bar arrives — every five or fifteen minutes — but
  // the live price rides on this same small JSON, so it is read every 10 s:
  // half the old interval, and well inside the ninety seconds after which the
  // server stops reporting a tick at all.
  useEffect(() => {
    let alive = true
    const read = () =>
      api
        .paperStatus()
        .then((r) => {
          if (!alive) return
          setRuns(r.runs)
          setStatusError(null)
          setNow(Date.now())
        })
        .catch((e: Error) => {
          if (alive) setStatusError(e.message)
        })
    read()
    const timer = window.setInterval(read, 10_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  // The price arrives every 10 s; its *age* is a second hand, and an age that
  // sat still for ten seconds at a time would itself look like a stuck feed.
  // Only `now` changes here — no request, and nothing the chart depends on.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1_000)
    return () => window.clearInterval(timer)
  }, [])

  // Alphabetical, except that the account view puts the mirrored books first.
  //
  // Every book is listed in both modes on purpose - a book that is NOT on the
  // account is a real and useful thing to see there, and hiding it would make
  // the account view quietly disagree with the desk about how many books exist.
  // But it must not bury the two or three that are, which is what plain
  // alphabetical order did.
  const sorted = useMemo(() => {
    const byId = [...(runs ?? [])].sort((a, b) => a.id.localeCompare(b.id))
    if (account == null) return byId
    const on = (r: PaperRun) => (brokerLive(r, now, account) ? 0 : 1)
    return byId.sort((a, b) => on(a) - on(b))
    // `now` deliberately absent: it ticks every second and would reorder the
    // list under the reader's cursor. Freshness only ever moves a book from
    // mirrored to not, and the next status poll reorders it then.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runs, account])

  // The row the drill-down reads, derived rather than stored: a remembered id
  // that no longer names a run (renamed, never started) falls to the first row
  // without spending a render correcting itself.
  const activeId = selected && sorted.some((r) => r.id === selected) ? selected : (sorted[0]?.id ?? null)

  // The selected account's record of the run being drilled into, or null while
  // the desk is on the paper book. Everything under the chart reads this and
  // not `detail`, which is always the paper book: the panel used to show
  // "3 closed, net +64 USC" under a heading that said "books on the account".
  //
  // Deliberately not recomputed against `now`. Freshness is already enforced
  // where the account is CHOSEN - the app bar drops back to paper when the
  // selected account stops reporting, which makes `account` null and this null
  // with it - and depending on the clock here would rebuild the chart's trade
  // array once a second for no reason.
  const activeBroker = useMemo(() => {
    if (account == null) return null
    const run = sorted.find((r) => r.id === activeId)
    return run?.brokers?.find((b) => b.login === account) ?? null
  }, [sorted, activeId, account])

  // The selected run in full: at once on selection, then every 60 s. The detail
  // is heavier than the status and changes no faster. Nothing is cleared here —
  // the pane shows its skeleton while `detail` still belongs to the previous run.
  useEffect(() => {
    if (!activeId) return
    let alive = true
    const read = () =>
      api
        .paperRun(activeId, DETAIL_BARS)
        .then((d) => {
          if (!alive) return
          setDetail(d)
          setDetailError(null)
        })
        .catch((e: Error) => {
          if (alive) setDetailError({ id: activeId, message: e.message })
        })
    read()
    const timer = window.setInterval(read, 60_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [activeId])

  const pick = useCallback((id: string) => {
    setSelected(id)
    writeSelected(id)
  }, [])

  const openFill = useCallback(
    (key: string | null) => setFillPick(key && activeId ? { id: activeId, key } : null),
    [activeId],
  )

  // The detail only ever belongs to one run; while a newly picked run is still
  // loading, the panes below show their skeletons rather than the old run's
  // chart under the new run's name.
  const live = detail && detail.run.id === activeId ? detail : null
  const pickedFill = fillPick && fillPick.id === activeId ? fillPick.key : null
  // The same field from whichever poll read it last: `/status` every 10 s,
  // `/run/{id}` every 60. Taking it from the detail alone would leave the
  // chart's forming candle a minute behind the price in the table above it.
  const activeRun = sorted.find((r) => r.id === activeId) ?? null
  // Whichever of the three sources read the market last. The stream is
  // normally newest by nine seconds; comparing `at` rather than preferring the
  // stream outright means a stalled socket cannot pin the chart to an old
  // candle while the polls are still bringing fresh ones.
  const livePrice = useMemo(() => {
    const streamed = activeRun ? (ticks[`${activeRun.market}:${activeRun.tf}`] ?? null) : null
    const polled = activeRun?.live ?? live?.live ?? null
    if (!streamed) return polled
    if (!polled) return streamed
    return streamed.at >= polled.at ? streamed : polled
  }, [activeRun, ticks, live])
  // The fill the chart is asked to show. Derived from the same key the table
  // marks open, so the row and the bands can never point at different trades.
  const focusFill = useMemo(
    () => (pickedFill && live ? (live.fills.find((f, i) => fillId(f, i) === pickedFill) ?? null) : null),
    [pickedFill, live],
  )

  return (
    <div
      className="flex min-h-0 flex-col overflow-hidden"
      style={{ height: `calc(100dvh - ${APP_BAR}px)` }}
    >
      <SummaryStrip runs={sorted} now={now} loading={runs === null} error={statusError} streaming={streaming} account={account} />

      {statusError && runs === null ? (
        <p className="text-destructive px-3 py-4 text-[13px]">
          Could not read the paper status: {statusError}
        </p>
      ) : runs === null ? (
        <div className="space-y-2 p-3">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <Skeleton key={i} className="h-6 w-full" />
          ))}
        </div>
      ) : sorted.length === 0 ? (
        <NoRuns />
      ) : (
        /* The chart owns the screen and the list of books moved to the rail.
           A ten-row table across the top pushed the candles into a 380px
           letterbox for the sake of columns a reader consults once a session;
           the chart is the thing being read continuously, so it takes the
           height and the run list becomes a picker beside it. */
        <div className="flex min-h-0 flex-1 flex-col overflow-auto xl:flex-row xl:overflow-hidden">
          <div className="flex min-w-0 flex-col xl:h-full xl:min-w-0 xl:flex-1">
            {/* What the bot traded on: its own indicators over the candles it
                stepped, with every fill's entry, exit and stop / target band.
                Fills the column's height above xl and keeps a floor below it,
                where the page scrolls instead. */}
            <div className="border-border min-h-[340px] shrink-0 border-b xl:min-h-0 xl:flex-1 xl:shrink">
              <RunChart detail={live} live={livePrice} focus={focusFill} broker={activeBroker} />
            </div>
            {/* The fills have seven columns and the rail has 460px, so they
                stay here where the width is. Capped at two fifths of the
                column: a long book must not push the chart off the screen the
                move above was made to give it. Below xl they are in the rail. */}
            <div className="hidden xl:flex xl:max-h-[40%] xl:shrink-0 xl:flex-col xl:overflow-hidden">
              <BottomTabs tab={bottomTab} onTab={setBottomTab} />
              <div className="min-h-0 flex-1 overflow-y-auto">
                {bottomTab === 'fills' ? (
                  <FillsSection detail={live} openFill={pickedFill} onOpenFill={openFill} broker={activeBroker} />
                ) : (
                  <ReasoningSection runId={activeId} detail={live} />
                )}
              </div>
            </div>
          </div>
          <div className="flex min-w-0 flex-col border-t xl:h-full xl:w-[460px] xl:shrink-0 xl:border-t-0 xl:border-l">
            {/* The books, compact. The picker the wide table used to be, in the
                width a rail has: everything the wide table's eleven columns
                carried that is not per-run configuration, and the rest moved
                into the drill-down under it where it belongs to one run. */}
            <div className="border-border shrink-0 border-b xl:max-h-[46%] xl:overflow-y-auto">
              <RunsList runs={sorted} now={now} selected={activeId} onPick={pick} ticks={ticks} account={account} />
            </div>
            <div className="min-h-0 xl:flex-1 xl:overflow-y-auto">
              <Drilldown
                detail={live}
                summary={activeRun}
                broker={activeBroker}
                error={detailError && detailError.id === activeId ? detailError.message : null}
                now={now}
                openFill={pickedFill}
                onOpenFill={openFill}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

/* --------------------------------------------------------- summary strip */

function SummaryStrip({
  runs,
  now,
  loading,
  error,
  streaming,
  account,
}: {
  runs: PaperRun[]
  now: number
  loading: boolean
  error: string | null
  /** The account the totals are of, or null for the paper book. The two nets
   *  are different numbers and the strip must never show one under the other's
   *  name. */
  account: number | null
  /** Whether the push stream is connected. A chart that has quietly stopped
   *  updating looks exactly like a quiet market; this is how a reader tells. */
  streaming: boolean
}) {
  const stats = useMemo(() => {
    const midnight = startOfDayUtc(now)
    let stale = 0
    let net = 0
    let today = 0
    let capped = false
    let blackout: { time: number; name: string } | null = null
    // The shortest calendar on the desk, because the guard fails per run and a
    // desk is only as covered as its least covered book.
    let horizon: { days: number; name: string } | null = null
    for (const run of runs) {
      if (isStale(run, now)) stale += 1
      net += run.net_usd
      const fills = run.last_fills.filter((t) => t.exitTime >= midnight)
      today += fills.length
      // The status route sends ten fills per run; when all ten closed today the
      // count is a floor, not a total, and the strip says so with a `≥`.
      if (run.last_fills.length >= LAST_FILLS_CAP && fills.length === run.last_fills.length) capped = true
      const next = run.news.next_blackout
      if (next && (!blackout || next.time < blackout.time)) {
        blackout = { time: next.time, name: next.name ?? next.currency }
      }
      const days = run.news.horizon_days
      if (typeof days === 'number' && (horizon === null || days < horizon.days)) {
        horizon = { days, name: run.news.horizon_name ?? 'the calendar' }
      }
    }
    return { stale, net, today, capped, blackout, horizon }
  }, [runs, now])

  if (loading) {
    return (
      <div className="flex h-8 shrink-0 items-center gap-4 border-b px-3">
        <Skeleton className="h-3 w-80" />
      </div>
    )
  }

  return (
    <div className="text-muted-foreground flex h-8 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b px-3 text-[11px]">
      <span className="num text-foreground">
        {runs.length} <span className="text-muted-foreground">run{runs.length === 1 ? '' : 's'}</span>
      </span>
      <span className="num">
        <span className="text-lc">{runs.length - stats.stale} fed</span>
        {' · '}
        <span className={stats.stale > 0 ? 'text-caution' : undefined}>{stats.stale} stale</span>
      </span>
      {account != null ? (
        <AccountTotals runs={runs} now={now} account={account} />
      ) : (
        <>
          <span className="num">
            net{' '}
            <span className={stats.net > 0 ? 'text-lc' : stats.net < 0 ? 'text-lp' : 'text-foreground'}>
              {signedUsd(stats.net)}
            </span>
          </span>
          <span className="num" title="Closed since 00:00 UTC, counted from the last ten fills each run reports.">
            {stats.capped ? '≥' : ''}
            {stats.today} <span className="text-muted-foreground">closed today</span>
          </span>
        </>
      )}
      <span className="num">
        next blackout{' '}
        {stats.blackout ? (
          <span className="text-caution">
            {stats.blackout.name} {zulu(stats.blackout.time)}
          </span>
        ) : (
          <span>none scheduled</span>
        )}
      </span>
      {/* A guard that stops guarding on a date nobody is watching is the
          failure this desk keeps logging. Shown from ninety days out, in the
          strip rather than in a file, and it goes red inside a month. */}
      <span className="num" title="The forming candle is pushed over /api/paper/stream. Polling still runs underneath it.">
        {streaming ? (
          <span className="text-lc">streaming</span>
        ) : (
          <span className="text-caution">polling only</span>
        )}
      </span>
      {stats.horizon !== null && stats.horizon.days < 90 && (
        <span
          className={cn('num', stats.horizon.days < 30 ? 'text-destructive' : 'text-caution')}
          title="The first release series to run out. After its last entry the news blackout stops for that release, silently."
        >
          {stats.horizon.name} ends in {stats.horizon.days}d
        </span>
      )}
      {error && <span className="text-destructive ml-auto truncate">status: {error}</span>}
    </div>
  )
}

/**
 * The account's side of the strip: banked, open and how many books are on it.
 *
 * Summed only over books whose mirror is still reporting. A stopped executor's
 * last file would otherwise be added into a total presented as current, which
 * is the one thing a total must never do.
 *
 * Currencies are not mixed. Everything here is one broker account and one
 * currency; if two accounts in different currencies ever appear, this shows
 * the count and not a sum, because adding them would be arithmetic on two
 * different units.
 */
function AccountTotals({ runs, now, account }: { runs: PaperRun[]; now: number; account: number }) {
  const live = runs
    .map((r) => brokerLive(r, now, account))
    .filter((b): b is NonNullable<typeof b> => b != null)
  if (live.length === 0) return <span className="num text-muted-foreground">no mirror reporting</span>
  const currencies = new Set(live.map((b) => b.currency ?? ''))
  const banked = live.reduce((sum, b) => sum + (b.realised ?? 0), 0)
  const open = live.reduce((sum, b) => sum + (b.position?.profit ?? 0), 0)
  const closed = live.reduce((sum, b) => sum + (b.closed ?? 0), 0)
  const holding = live.filter((b) => b.position).length
  if (currencies.size > 1) {
    return (
      <span className="num text-caution" title="These books are on accounts in different currencies; a total would be nonsense.">
        {live.length} mirrored &middot; mixed currencies
      </span>
    )
  }
  const ccy = live[0].currency
  return (
    <>
      <span className="num">
        banked{' '}
        <span className={banked > 0 ? 'text-lc' : banked < 0 ? 'text-lp' : 'text-foreground'}>
          {brokerMoney(banked, ccy)}
        </span>
      </span>
      <span className="num" title="Floating P&L on the positions the account holds right now.">
        open{' '}
        <span className={open > 0 ? 'text-lc' : open < 0 ? 'text-lp' : 'text-foreground'}>
          {brokerMoney(open, ccy)}
        </span>
        <span className="text-muted-foreground"> on {holding}</span>
      </span>
      <span className="num">
        {closed} <span className="text-muted-foreground">closed on the account</span>
      </span>
    </>
  )
}

function NoRuns() {
  return (
    <div className="text-muted-foreground min-h-0 flex-1 px-4 py-8 text-[13px]">
      <p className="text-foreground">No paper run is registered.</p>
      <p className="mt-1">Start the ten registered runs, then start the pollers that feed them closed bars:</p>
      <pre className="border-border bg-card text-foreground mt-3 w-fit rounded-md border px-3 py-2 font-mono text-[11px] leading-relaxed">
        <code>{'python py/live/start_runs.py\npowershell -File py\\live\\start_pollers.ps1'}</code>
      </pre>
      <p className="mt-2 text-[11px]">
        Nothing here reaches a broker: a run steps on closed bars and writes a book to disk.
      </p>
    </div>
  )
}

/* ------------------------------------------------------------ runs table */

/**
 * The books, as a picker in the rail.
 *
 * This replaced an eleven-column table that ran the full width of the screen.
 * The columns were not wasted — they were just in the wrong place: a reader
 * watches the candles continuously and consults a run's guards once a session,
 * so the chart now has the width and the list has what a picker needs. The
 * two columns that belonged to one run rather than to the comparison, the open
 * position and the guards that fired, moved into the drill-down below.
 *
 * Two lines a row: what it is and what it has made, then what it is trading
 * and what that is worth right now. Ten of those fit a rail without scrolling.
 */
function RunsList({
  runs,
  now,
  selected,
  onPick,
  ticks,
  account,
}: {
  runs: PaperRun[]
  now: number
  selected: string | null
  onPick: (id: string) => void
  /** Streamed forming bars by `market:tf`; newer than the row's own. */
  ticks: Record<string, LiveBar>
  /** The account the rows describe, or null for the paper book. Never merged
   *  into one row - the gap between what the rule decided and what an account
   *  did with it is the measurement, and a merged row hides exactly that. */
  account: number | null
}) {
  /** The row with its live price replaced, when the stream has a fresher one. */
  const streamed = (run: PaperRun): PaperRun | null => {
    const tick = ticks[`${run.market}:${run.tf}`]
    if (!tick || (run.live && run.live.at >= tick.at)) return null
    return { ...run, live: tick }
  }

  const silent = useMemo(() => silentBooks(runs, now), [runs, now])

  const rowRefs = useRef<(HTMLButtonElement | null)[]>([])

  // The same walk the fills table has: ↑/↓ move the cursor, Enter opens.
  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, runs.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  return (
    <div role="group" aria-label="Paper runs">
      <div className="text-muted-foreground bg-background sticky top-0 z-10 flex items-baseline gap-2 border-b px-3 py-1 text-[10px] tracking-wide uppercase">
        <span>{account != null ? 'books on the account' : 'books'}</span>
        <span className="num text-muted-foreground/70 normal-case">
          {account != null ? runs.filter((r) => brokerLive(r, now, account)).length : runs.length}
        </span>
        <span className="text-muted-foreground/60 ml-auto normal-case">click to load the chart</span>
      </div>

      {runs.map((run, index) => {
        const stale = isStale(run, now)
        const isOn = run.id === selected
        const broker = brokerLive(run, now, account)
        // A book nobody mirrors is not an empty account - it is a book that is
        // not on the account at all, and saying "0.00" for it would be a
        // measurement nobody made.
        const unmirrored = account != null && !broker
        return (
          <button
            key={run.id}
            type="button"
            ref={(el) => {
              rowRefs.current[index] = el
            }}
            onClick={() => onPick(run.id)}
            onKeyDown={(e) => onKeyDown(e, index)}
            aria-pressed={isOn}
            className={cn(
              'hover:bg-accent/60 focus-visible:ring-ring w-full border-b px-3 py-[6px] text-left transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none',
              isOn && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
              unmirrored && 'opacity-45',
            )}
          >
            <span className="flex items-center gap-2 text-xs">
              <StatusPill stale={stale} />
              <span className="num min-w-0 flex-1 truncate">{run.id}</span>
              {account != null ? (
                <span
                  className={cn(
                    'num shrink-0',
                    (broker?.realised ?? 0) > 0 && 'text-lc',
                    (broker?.realised ?? 0) < 0 && 'text-lp',
                    (!broker || broker.realised === 0) && 'text-muted-foreground',
                  )}
                >
                  {broker ? brokerMoney(broker.realised, broker.currency) : 'not mirrored'}
                </span>
              ) : (
                <span
                  className={cn(
                    'num shrink-0',
                    run.net_usd > 0 && 'text-lc',
                    run.net_usd < 0 && 'text-lp',
                    run.net_usd === 0 && 'text-muted-foreground',
                  )}
                >
                  {accountMoney(run.net_usd, run)}
                </span>
              )}
            </span>
            <span className="mt-0.5 flex items-end gap-2 text-[10px] leading-tight">
              <span className="text-muted-foreground min-w-0 flex-1 truncate">
                <DeciderTag run={run} silent={silent.has(run.id)} />
                <span className="num">{run.strategy}</span>
                <span className="text-muted-foreground/60"> · {run.market}:{run.tf}</span>
                <span className="text-muted-foreground/60">
                  {' '}
                  {/* "closed", not "fills". A book holding an open position read
                      "0 fills" beside a badge saying it was long two USC down —
                      both true, and together they say nothing happened. */}
                  · {account != null ? (broker?.closed ?? 0) : run.trades} closed
                  {account == null && run.profit_factor != null && ` · PF ${num(run.profit_factor)}`}
                  {account != null && broker?.symbol && ` · ${broker.symbol}`}
                </span>
                {account != null ? (
                  <BrokerBadge broker={broker} />
                ) : (
                  <>
                    {run.open && <OpenBadge run={run} tick={ticks[`${run.market}:${run.tf}`]} />}
                    {!run.open && run.pending && <PendingBadge pending={run.pending} />}
                  </>
                )}
                <span className="text-muted-foreground/60">
                </span>
              </span>
              <span className="shrink-0">
                <LiveCell run={streamed(run) ?? run} now={now} />
              </span>
            </span>
          </button>
        )
      })}
    </div>
  )
}

/**
 * Who is driving this book, when it is not a rule.
 *
 * Shown from `run.decider`, which the API writes only when an intent is
 * ACCEPTED — so this names a decision the book really took, never one that
 * was merely offered or a model id someone typed into a config. A run whose
 * strategy is `external` but which nobody has posted to reads "idle", which
 * is the truth about it.
 *
 * The coin is deliberately styled apart from the model. It is the control
 * book, and the whole campaign is the difference between the two; a badge
 * that made them look alike would hide the one comparison that matters.
 */
/**
 * Which house the decider belongs to, from the name it posted under.
 *
 * Read from the name rather than configured, for the same reason the badge
 * itself is: the row must describe what actually drove this book. `codex/` is
 * this repo's own marker for "the ChatGPT plan", so it is checked first — a
 * name can carry the route and the model at once.
 */
function houseOf(name: string | undefined): 'openai' | 'claude' | 'deepseek' | null {
  if (!name) return null
  const n = name.toLowerCase()
  if (n.startsWith('deepseek')) return 'deepseek'
  if (n.startsWith('codex/') || n.startsWith('gpt') || /^o[134]/.test(n)) return 'openai'
  if (n.startsWith('claude') || n.startsWith('opus') || n.startsWith('sonnet') || n.startsWith('haiku')) return 'claude'
  return null
}

/**
 * Who is driving this book, when it is not a rule.
 *
 * Shown from `run.decider`, which the API writes only when a decision is
 * ACCEPTED — so this names something the book really did, never a model id
 * someone typed into a config. A run whose strategy is `external` but which
 * nobody has posted to reads "idle", which is the truth about it.
 *
 * The colour is not decoration. Each house wears its own palette, taken from
 * the brand rather than invented: Anthropic's accent is #CC785C, and OpenAI's
 * brand is monochrome, so theirs is a metal gradient rather than a colour
 * borrowed from a product page. That also keeps the two readable apart at a
 * glance on a list of fourteen rows, which is the actual job.
 *
 * The coin is deliberately styled apart from both and carries NO mark: it is
 * the control, the campaign is the difference between it and the model, and a
 * badge that made them look alike would hide the one comparison that matters.
 */
function DeciderTag({ run, silent = false }: { run: PaperRun; silent?: boolean }) {
  if (run.strategy !== 'external' && !run.decider) return null

  const names = Object.keys(run.decider?.decisions ?? {})
  const mixed = names.length > 1
  const last = run.decider?.last
  const coin = last === 'coin'
  const total = Object.values(run.decider?.decisions ?? {}).reduce((a, b) => a + b, 0)
  const house = coin || mixed ? null : houseOf(last)

  const text = last ? (coin ? 'coin' : last) : 'idle'
  const aside = run.decider?.stood_aside ?? 0
  const spoke = last ? `${total} trade${total === 1 ? '' : 's'}, ${aside} stood aside` : ''
  const quiet = run.decider?.last_at ? Math.round((Date.now() - run.decider.last_at) / 60000) : null
  const title = silent
    ? `${last} has not answered for ${quiet} min. The feed is fine and the book is still being fed bars — what has stopped is the thing that decides. Nothing will be traded on this book until it comes back.`
    : mixed
      ? `driven by ${names.length} deciders (${names.map((n) => `${n} ${run.decider?.decisions[n]}`).join(', ')}) — this book's net is not any one of their records`
      : last
        ? `${last}: ${spoke}. Standing aside is a real answer; it keeps the badge alive without a trade.`
        : 'externally driven; nothing has posted to it yet'

  // Gradients are inline rather than Tailwind classes because the stops are
  // brand values, not theme tokens — putting #CC785C in the token set would
  // imply the desk owns that colour, and it does not.
  // Each house in the colour its own mark is published in, lifted until 9px
  // text holds on the dark ground. Inline rather than Tailwind classes because
  // these are brand values, not theme tokens — putting #D97757 in the token set
  // would imply the desk owns that colour.
  const SKINS = {
    claude: { rgb: '217,119,87', text: '#F2C3AC' },   // #D97757
    deepseek: { rgb: '77,107,254', text: '#B9C6FF' }, // #4D6BFE
    // OpenAI's brand is monochrome, so theirs is a metal sheen rather than a
    // borrowed hue — which also keeps it distinct from the flat grey the coin
    // wears.
    openai: { rgb: '255,255,255', text: '#F3F5F7' },
  } as const
  // A stopped model loses its house colour entirely. Dimming the brand tint
  // would still read as "this is the DeepSeek book, slightly faded"; dropping
  // it reads as "this book is not being driven", which is the true statement.
  const tone = silent ? null : house ? SKINS[house] : null
  const skin = tone
    ? {
        backgroundImage: `linear-gradient(100deg, rgba(${tone.rgb},0.32), rgba(${tone.rgb},0.06))`,
        borderColor: `rgba(${tone.rgb},0.5)`,
        color: tone.text,
      }
    : undefined

  return (
    <span
      title={title}
      style={skin}
      className={cn(
        'mr-1 inline-flex items-center gap-1 rounded-sm border px-1 py-px align-middle text-[9px] tracking-wide uppercase',
        silent && 'border-caution/40 bg-caution/10 text-caution',
        !silent &&
          !skin &&
          (mixed
            ? 'border-caution/40 bg-caution/10 text-caution'
            : coin
              ? 'border-muted-foreground/30 bg-muted-foreground/10 text-muted-foreground'
              : last
                ? 'border-primary/40 bg-primary/10 text-primary'
                : 'border-muted-foreground/25 bg-muted-foreground/5 text-muted-foreground/70'),
      )}
    >
      {/* The house mark goes with the house colour when the model has
          stopped: a book nothing is driving should not still be wearing a
          vendor's logo. */}
      {!silent && house === 'openai' && <OpenAIMark className="size-[10px] shrink-0" />}
      {!silent && house === 'claude' && <ClaudeMark className="size-[10px] shrink-0" />}
      {!silent && house === 'deepseek' && <DeepSeekMark className="size-[10px] shrink-0" />}
      {silent && <span aria-hidden>{'\u23f8'}</span>}
      {!silent && !house && !coin && <span aria-hidden>{mixed ? '\u26a0' : '\u25c6'}</span>}
      <span className="num normal-case">{text}</span>
      {/* Named, not implied. "stopped" beside the model is the one word
          that stops a reader taking the row's numbers as something still
          being added to. */}
      {silent && <span className="normal-case">stopped</span>}
    </span>
  )
}

/** The two bottom panels: what the book did, and what it was thinking. */
function BottomTabs({ tab, onTab }: { tab: 'fills' | 'log'; onTab: (t: 'fills' | 'log') => void }) {
  const items: { id: 'fills' | 'log'; label: string }[] = [
    { id: 'fills', label: 'fills' },
    { id: 'log', label: 'ai log' },
  ]
  return (
    <div className="bg-background flex shrink-0 items-center gap-1 border-b px-3 py-1">
      {items.map((it) => (
        <button
          key={it.id}
          type="button"
          onClick={() => onTab(it.id)}
          aria-pressed={tab === it.id}
          className={cn(
            'focus-visible:ring-ring rounded-sm px-2 py-0.5 text-[10px] tracking-wide uppercase transition-colors focus-visible:ring-2 focus-visible:outline-none',
            tab === it.id
              ? 'bg-primary/15 text-primary'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          {it.label}
        </button>
      ))}
    </div>
  )
}

/**
 * What the models said about this book.
 *
 * Two logs, shown apart because they are two different powers over a trade: a
 * decider choosing a side on its own book, and the advisor panel refusing or
 * shrinking somebody else's. A rule-based run usually has only the second, an
 * `external` run only the first, and a run with neither says so rather than
 * showing an empty frame.
 *
 * Polled rather than streamed. These are written by processes outside this one,
 * at one entry per closed bar; a socket for something that moves every fifteen
 * minutes would be machinery for its own sake.
 */
function ReasoningSection({ runId, detail }: { runId: string | null; detail: PaperRunDetail | null }) {
  const [data, setData] = useState<Reasoning | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!runId) {
      setData(null)
      return
    }
    let alive = true
    const load = () => {
      api
        .paperReasoning(runId, 50)
        .then((r) => {
          if (!alive) return
          setData(r)
          setError(null)
        })
        .catch((e: unknown) => {
          if (!alive) return
          setError(e instanceof Error ? e.message : String(e))
        })
    }
    // Cleared first, so a slow answer for the previous book cannot land under
    // the heading of the one just opened.
    setData(null)
    setError(null)
    load()
    const timer = window.setInterval(load, 20_000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [runId])

  if (!runId) return null
  if (error) return <p className="text-destructive px-3 py-3 text-[12px]">Could not read the log: {error}</p>
  if (!data) return <div className="px-3 py-3"><Skeleton className="h-4 w-64" /></div>

  const empty = data.decisions.length === 0 && data.consultations.length === 0
  if (empty) {
    return (
      <p className="text-muted-foreground px-3 py-3 text-[12px]">
        No model has spoken about this book. Rule-based books carry an advisor
        transcript only once the panel has seen a pending trade; an AI book
        carries one from its first bar.
      </p>
    )
  }

  return (
    <div className="px-3 py-2">
      {data.decisions.length > 0 && (
        <>
          <SectionHead>
            decisions <TokenTotal decisions={data.decisions} />{' '}
            <span className="text-muted-foreground/70 normal-case">
              &mdash; one per closed bar, times in Vietnam (UTC+7). The model&rsquo;s own sentences quote UTC bar
              stamps, because that is what its prompt shows it.
            </span>
          </SectionHead>
          <ul className="mt-1 space-y-1">
            {data.decisions.map((d) => (
              <DecisionRow key={`${d.bar_time}-${d.at}`} d={d} outcome={outcomeOf(d, detail)} />
            ))}
          </ul>
        </>
      )}
      {data.consultations.length > 0 && (
        <>
          <SectionHead>
            advisor panel <span className="text-muted-foreground/70 normal-case">— may refuse or shrink, never choose a side</span>
          </SectionHead>
          <ul className="mt-1 space-y-1">
            {data.consultations.map((c) => (
              <ConsultationRow key={`${c.intent_id}-${c.at}`} c={c} />
            ))}
          </ul>
        </>
      )}
    </div>
  )
}

/** What the decisions on screen have cost between them. */
function TokenTotal({ decisions }: { decisions: Decision[] }) {
  let tokens = 0
  let cost = 0
  let billed = 0
  for (const d of decisions) {
    tokens += d.tokens_total ?? (d.tokens_in ?? 0) + (d.tokens_out ?? 0)
    if (d.cost_usd != null) {
      cost += d.cost_usd
      billed += 1
    }
  }
  if (!tokens) return null
  return (
    <span
      className="num text-muted-foreground/70 normal-case"
      title={
        billed
          ? `${billed} of these ${decisions.length} are billed per token; the rest run on a subscription and have no per-call price`
          : 'all of these run on a subscription — tokens are quota, not dollars'
      }
    >
      ({tokens.toLocaleString()}t{billed ? ` · $${cost.toFixed(4)}` : ' · on a plan'})
    </span>
  )
}

function SectionHead({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="text-muted-foreground mt-2 text-[10px] tracking-wide uppercase first:mt-0">{children}</h3>
  )
}

/** LONG / SHORT / NONE, coloured the way the fills table colours a side. */
function SidePill({ side }: { side: string }) {
  const up = side === 'LONG'
  const down = side === 'SHORT'
  return (
    <span
      className={cn(
        'num shrink-0 rounded-sm border px-1 text-[10px]',
        up && 'border-lc/40 bg-lc/10 text-lc',
        down && 'border-lp/40 bg-lp/10 text-lp',
        !up && !down && 'border-muted-foreground/30 text-muted-foreground',
      )}
    >
      {side.toLowerCase()}
    </span>
  )
}

/**
 * The trade a decision became, if it became one.
 *
 * A posted entry fills at the OPEN of the bar after the one it was decided on,
 * so the trade's entry time is the decision's bar time plus one timeframe.
 * Matched on that rather than on order, because the fills list is reversed for
 * display and a refused or stood-aside bar leaves no trade at all.
 */
function outcomeOf(d: Decision, detail: PaperRunDetail | null): BacktestTrade | null {
  if (!detail || d.side === 'NONE') return null
  const step = TF_MS[detail.run.tf]
  if (!step) return null
  return (detail.fills ?? []).find((f) => f.entryTime === d.bar_time + step) ?? null
}

/**
 * What one decision spent.
 *
 * A metered model gets tokens AND the dollars it cost. A subscription gets
 * tokens only — deliberately, because a plan call is not free, it draws on a
 * quota, and showing $0.00 beside it would claim something untrue. The owner
 * asked for exactly that split.
 *
 * `tokens_total` is the fallback for a provider that reports one number and no
 * breakdown: Codex prints a banner total and nothing else, and inventing an
 * input/output split it never gave would be worse than showing less.
 */
function TokenChip({ d }: { d: Decision }) {
  const total = d.tokens_total ?? ((d.tokens_in ?? 0) + (d.tokens_out ?? 0) || null)
  if (!total) return null
  const cached = d.tokens_cached ?? 0
  const parts = [
    d.tokens_in != null ? `${d.tokens_in.toLocaleString()} in` : null,
    cached ? `${cached.toLocaleString()} of them cached` : null,
    d.tokens_out != null ? `${d.tokens_out.toLocaleString()} out` : null,
  ].filter(Boolean)
  return (
    <span
      className="num text-muted-foreground/70 shrink-0 text-[10px]"
      title={
        (parts.length ? parts.join(' · ') : `${total.toLocaleString()} tokens, no split reported`) +
        (d.cost_usd != null ? '' : ' — on a subscription, so no per-call price')
      }
    >
      {total.toLocaleString()}t
      {d.cost_usd != null && <span className="text-caution"> ${d.cost_usd.toFixed(4)}</span>}
    </span>
  )
}

function DecisionRow({ d, outcome }: { d: Decision; outcome: BacktestTrade | null }) {
  const [open, setOpen] = useState(false)
  return (
    <li className="border-border/60 rounded-sm border">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="hover:bg-accent/40 focus-visible:ring-ring flex w-full items-start gap-2 px-2 py-1 text-left focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none"
      >
        <span
          className="num text-muted-foreground shrink-0 text-[11px]"
          title={`bar ${utcStamp(d.bar_time)} · answered ${utcStamp(d.at)}`}
        >
          {vnStamp(d.at)}
        </span>
        <SidePill side={d.side} />
        <span className="min-w-0 flex-1 text-[12px] leading-snug">{d.reason || <span className="text-muted-foreground">(no reason given)</span>}</span>
        {outcome && (
          <span
            className={cn('num shrink-0 rounded-sm px-1 text-[10px]', outcome.pnlUsd >= 0 ? 'bg-lc/15 text-lc' : 'bg-lp/15 text-lp')}
            title={`closed ${utcStamp(outcome.exitTime)} at ${outcome.exitPrice} — ${outcome.exitReason}`}
          >
            {signedR(outcome.r)}
          </span>
        )}
        <TokenChip d={d} />
        <span className="num text-muted-foreground/70 shrink-0 text-[10px]">
          {(d.latency_ms / 1000).toFixed(1)}s
        </span>
      </button>
      {open && (
        <div className="border-border/60 space-y-1 border-t px-2 py-1.5 text-[11px]">
          {outcome && (
            <p className="text-[11px]">
              <span className="text-muted-foreground">became: </span>
              <span className="num">{quote(outcome.entryPrice)}</span>
              <span className="text-muted-foreground"> → </span>
              <span className="num">{quote(outcome.exitPrice)}</span>
              <span className="text-muted-foreground"> at {vnStamp(outcome.exitTime)} · </span>
              <span className="text-muted-foreground">{outcome.exitReason.toLowerCase().replace(/_/g, ' ')} · </span>
              <span className={cn('num', outcome.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>
                {signedR(outcome.r)}
              </span>
              <span className="text-muted-foreground/60"> · held {Math.round(outcome.holdMs / 60000)} min</span>
            </p>
          )}
          <p className="text-muted-foreground">
            <span className="num">{d.model}</span> · decided {vnStamp(d.at)} ·{' '}
            {/* The state that matters is whether this reached a book, and why not. */}
            {d.dry_run
              ? 'dry run — nothing was posted'
              : d.refused_locally
                ? `refused by the desk: ${d.refused_locally}`
                : d.posted
                  ? 'posted; fills at the next bar\u2019s open'
                  : 'not posted'}
          </p>
          <pre className="text-muted-foreground/90 overflow-x-auto rounded-sm bg-black/30 p-1.5 text-[10px] whitespace-pre-wrap">
            {d.response || '(empty reply)'}
          </pre>
          <p className="text-muted-foreground/60 text-[10px]">
            prompt kept whole: {d.prompt_chars.toLocaleString()} characters, in
            data/paper/{'{'}run{'}'}/decisions.jsonl — a summary cannot be replayed against a changed prompt.
          </p>
        </div>
      )}
    </li>
  )
}

function ConsultationRow({ c }: { c: Consultation }) {
  const [open, setOpen] = useState(false)
  const cut = c.size_factor != null && c.size_factor < 1
  return (
    <li className="border-border/60 rounded-sm border">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="hover:bg-accent/40 focus-visible:ring-ring flex w-full items-start gap-2 px-2 py-1 text-left focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none"
      >
        <span className="num text-muted-foreground shrink-0 text-[11px]" title={utcStamp(c.at)}>{vnStamp(c.at)}</span>
        <span
          className={cn(
            'num shrink-0 rounded-sm border px-1 text-[10px]',
            c.size_factor === 0
              ? 'border-lp/40 bg-lp/10 text-lp'
              : cut
                ? 'border-caution/40 bg-caution/10 text-caution'
                : 'border-lc/40 bg-lc/10 text-lc',
          )}
        >
          {c.size_factor === 0 ? 'veto' : cut ? `\u00d7${c.size_factor?.toFixed(2)}` : 'allow'}
        </span>
        <span className="min-w-0 flex-1 text-[12px] leading-snug">{c.reason}</span>
        <span className="num text-muted-foreground/70 shrink-0 text-[10px]">{c.turns.length} agents</span>
      </button>
      {open && (
        <div className="border-border/60 space-y-1.5 border-t px-2 py-1.5">
          <p className="text-muted-foreground text-[11px]">
            {c.dry_run ? 'dry run — the verdict was not applied' : c.applied ? 'applied to the book' : 'not applied'} ·{' '}
            <span className="num">{c.intent_id}</span>
          </p>
          {c.turns.map((t) => (
            <div key={t.agent} className="border-border/40 border-l-2 pl-2">
              <p className="text-[11px]">
                <span className="num text-primary">{t.agent}</span>{' '}
                <span className="num text-muted-foreground/70">{t.model}</span>{' '}
                <span className="text-muted-foreground/60 num">
                  {(t.latency_ms / 1000).toFixed(1)}s
                  {t.size_factor != null && ` \u00b7 \u00d7${t.size_factor.toFixed(2)}`}
                </span>
              </p>
              <p className="text-muted-foreground text-[11px] leading-snug">{t.reason}</p>
            </div>
          ))}
          <p className="text-muted-foreground/60 text-[10px]">
            The verdict is the MIN of the panel, so one agent can refuse alone.
          </p>
        </div>
      )}
    </li>
  )
}

/**
 * Fed or stale. The dot pulses so a glance across ten rows finds the live ones,
 * and it stops pulsing when the reader's system asks for less motion.
 */
function StatusPill({ stale }: { stale: boolean }) {
  return (
    <span
      className={cn(
        'inline-flex w-fit items-center gap-1 rounded-sm border px-1.5 py-px text-[10px]',
        stale ? 'border-caution/30 bg-caution/10 text-caution' : 'border-lc/30 bg-lc/10 text-lc',
      )}
    >
      <span
        className={cn(
          'size-1.5 rounded-full',
          stale ? 'bg-caution' : 'bg-lc animate-pulse motion-reduce:animate-none',
        )}
        aria-hidden
      />
      {stale ? 'stale' : 'fed'}
    </span>
  )
}

/**
 * The price right now, and how old it is.
 *
 * The forming bar's close against the last closed bar's close: up green, down
 * red, with an arrow so the direction survives without colour. Underneath, the
 * age in seconds — because a price with no age is the thing that made this
 * screen look broken in the first place.
 *
 * With no live bar — no poller, or a market that is shut — the column is what
 * it was before: how long ago the last bar closed.
 */
function LiveCell({ run, now }: { run: PaperRun; now: number }) {
  const live = run.live
  if (!live) {
    return (
      <span className="num text-muted-foreground text-right">
        {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'no bar yet'}
      </span>
    )
  }
  const direction = liveDirection(live, run.last_bar_close)
  const spread = spreadOf(live)
  return (
    <span className="flex flex-col items-end leading-tight">
      <span
        className={cn(
          'num whitespace-nowrap transition-colors duration-300 motion-reduce:transition-none',
          DIRECTION_CLASS[direction + 1],
        )}
      >
        <span aria-hidden>{DIRECTION_MARK[direction + 1]}</span> {quote(live.close)}
      </span>
      <span className="num text-muted-foreground/80 text-[10px] whitespace-nowrap" title={spread ? `spread ${spread}` : undefined}>
        {liveAge(live.at, now)}
      </span>
    </span>
  )
}

/**
 * A trade that is decided but has not filled.
 *
 * Dashed, and never the same shape as an open position: the book is flat right
 * now and will not be after the next bar opens, and those are different facts.
 * Showing nothing at all for that bar — which is what the desk did until now —
 * made a committed book look idle for a full fifteen minutes.
 */
function PendingBadge({ pending }: { pending: NonNullable<PaperRun['pending']> }) {
  const long = pending.side === 'LONG'
  return (
    <span
      className={cn(
        'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border border-dashed px-1 py-px text-[10px]',
        long ? 'border-lc/50 text-lc/90' : 'border-lp/50 text-lp/90',
      )}
      title={`${pending.side} decided, fills at the next bar's open — ${pending.reason}`}
    >
      <span aria-hidden>{long ? '\u25b3' : '\u25bd'}</span>
      {pending.side.toLowerCase()}
      <span className="text-muted-foreground">waiting</span>
    </span>
  )
}

/**
 * Mark an open position at the live price.
 *
 * `unrealised_usd_at_last_close` is the engine's own number and is the one the
 * book will be judged on, but it is up to a bar old. Beside a price that is
 * moving, a figure that stale is the wrong one to show — so the live mark is
 * computed here from the tick when there is one, and the API's number is used
 * when there is not. The two agree at every bar close by construction.
 */
function markToLive(
  open: NonNullable<PaperRun['open']>,
  price: number | null | undefined,
): { usd: number; live: boolean } {
  if (price == null || !Number.isFinite(price) || !open.usd_per_point) {
    return { usd: open.unrealised_usd_at_last_close, live: false }
  }
  const dir = open.side === 'LONG' ? 1 : -1
  return { usd: (price - open.entry_price) * dir * open.usd_per_point, live: true }
}

/**
 * What the ACCOUNT holds for this book, and where it disagrees with the book.
 *
 * The disagreement is the point. A mirror is only worth running because the
 * two sides can differ, and the three ways they differ are each worth a
 * different word:
 *
 *  - the account holds the position   -> show it, with the broker's own P&L
 *  - the book wants one, account flat -> the mirror has not filled it, or was
 *                                        refused; say which
 *  - both flat                        -> "flat", which is agreement and needs
 *                                        no decoration
 *
 * A refusal is shown in full rather than summarised. The one that prompted
 * this badge - 100 lots against a $10k account - was a book bug, and a reader
 * who sees only "blocked" has to go and read a log to find that out.
 */
function BrokerBadge({ broker }: { broker: PaperBroker | null }) {
  if (!broker) return null
  const pos = broker.position
  if (pos) {
    const long = pos.side === 'LONG'
    const profit = pos.profit ?? 0
    return (
      <span
        className={cn(
          'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 py-px text-[10px]',
          long ? 'border-lc/45 bg-lc/10 text-lc' : 'border-lp/45 bg-lp/10 text-lp',
        )}
        title={`account holds ${pos.side} ${pos.lots} lots from ${pos.entry_price}, ticket ${pos.ticket}`}
      >
        <span aria-hidden>{long ? '▲' : '▼'}</span>
        {(pos.side ?? '').toLowerCase()} {pos.lots}
        <span className={profit >= 0 ? 'text-lc' : 'text-lp'}>
          {brokerMoney(profit, broker.currency)}
        </span>
      </span>
    )
  }
  if (broker.blocked) {
    return (
      <span
        className="num border-lp/45 bg-lp/10 text-lp mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px text-[10px]"
        title={broker.blocked}
      >
        refused: {broker.blocked}
      </span>
    )
  }
  // Sitting a trade out is the guard working, not a fault, so it is stated
  // quietly and in the muted palette. Showing it in the same red as a broker
  // refusal would teach the reader to ignore both.
  if (broker.standing_out) {
    return (
      <span
        className="num text-muted-foreground border-border mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px text-[10px]"
        title={`The mirror is not copying this trade: ${broker.standing_out}`}
      >
        sitting out
      </span>
    )
  }
  if (broker.book_side) {
    return (
      <span
        className="num text-muted-foreground border-border mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px text-[10px]"
        title={`the book is ${broker.book_side} ${broker.book_lots} lots; the account is flat`}
      >
        {broker.dry_run ? 'dry run' : 'not filled'} &middot; book wants {broker.book_side.toLowerCase()}{' '}
        {broker.book_lots}
      </span>
    )
  }
  return <span className="text-muted-foreground/60 mr-1 text-[10px]"> &middot; flat</span>
}

/** A compact open-position badge for a row in the books list. */
function OpenBadge({ run, tick }: { run: PaperRun; tick?: LiveBar }) {
  const open = run.open
  if (!open) return null
  const price = tick?.close ?? run.live?.close ?? run.last_bar_close
  const { usd } = markToLive(open, price)
  const long = open.side === 'LONG'
  return (
    <span
      className={cn(
        'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 py-px text-[10px]',
        long ? 'border-lc/45 bg-lc/10 text-lc' : 'border-lp/45 bg-lp/10 text-lp',
      )}
      title={`open ${open.side} ${open.lots} lots from ${open.entry_price}`}
    >
      <span aria-hidden>{long ? '\u25b2' : '\u25bc'}</span>
      {open.side.toLowerCase()}
      <span className={usd >= 0 ? 'text-lc' : 'text-lp'}>{accountMoney(usd, run)}</span>
    </span>
  )
}

/**
 * The open position, as a track from its stop to its target.
 *
 * The number that matters while a trade is on is not the entry price, it is
 * how close the price is to the two levels that will end it. A row of figures
 * makes the reader do that subtraction; a track does it for them, and the same
 * bar reads the same way on every book — a rule strategy, an AI book and its
 * coin control all size and exit under the same engine, so they all get this.
 *
 * Flat books keep a one-line "flat", because an empty frame where a position
 * would be is worse than a word.
 */
/**
 * The price a pending entry will actually fill at, once that is knowable.
 *
 * It fills at the OPEN of the bar after the one it was decided on. Before that
 * bar starts there is genuinely no such price and nothing should be drawn. But
 * the moment it starts forming, its open is already set and is EXACTLY the
 * fill — not an estimate — and the desk was hiding a number it already had.
 *
 * Claimed only when the forming bar is exactly one timeframe after the
 * decision. A stream that has skipped or is lagging must not have some other
 * bar's open presented as the fill.
 */
function pendingFill(
  pending: NonNullable<PaperRun['pending']>,
  live: LiveBar | null | undefined,
  tf: string,
): number | null {
  const step = TF_MS[tf]
  if (!step || !live || pending.decided_on == null) return null
  return live.time === pending.decided_on + step && Number.isFinite(live.open) ? live.open : null
}

function PositionBar({ run, live }: { run: PaperRun; live: LiveBar | null | undefined }) {
  const open = run.open
  if (!open) {
    // Flat and committed are different states and must not look alike. The
    // pending one carries no P&L on purpose: there is no entry price yet, so
    // every number a mark would need is missing, and inventing one would be
    // the only dishonest thing on this panel.
    const p = run.pending
    if (p) {
      const long = p.side === 'LONG'
      const fill = pendingFill(p, live, run.tf)
      return (
        <div className={cn('mt-2 rounded-sm border border-dashed px-2.5 py-2', long ? 'border-lc/40' : 'border-lp/40')}>
          <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
            <span className={cn('num rounded-sm border border-dashed px-1 text-[11px] font-medium', long ? 'border-lc/50 text-lc' : 'border-lp/50 text-lp')}>
              {p.side}
            </span>
            {fill != null ? (
              <span className="text-[11px]">
                <span className="text-muted-foreground">fills at </span>
                <span className="num text-[13px]">{quote(fill)}</span>
                <span className="text-muted-foreground/70"> — this bar&rsquo;s open, already set</span>
              </span>
            ) : (
              <span className="text-muted-foreground text-[11px]">decided — fills at the next bar&rsquo;s open, which has not started</span>
            )}
            <span className="num text-muted-foreground/70 ml-auto text-[10px]">
              on the {shortStamp(p.decided_on)} bar
            </span>
          </div>
          <div className="text-muted-foreground mt-1.5 flex flex-wrap gap-x-4 text-[10px]">
            <span className="num text-lp">
              stop {quote(p.stop)}
              {fill != null && p.stop != null && ` (${quote(Math.abs(fill - p.stop))} away)`}
            </span>
            <span className="num text-lc">
              target {quote(p.target)}
              {fill != null && p.target != null && ` (${quote(Math.abs(p.target - fill))} away)`}
            </span>
            {fill != null && (() => {
              const risk = Math.abs(fill - (p.stop ?? fill))
              if (!(risk > 0) || !run.contract_size) return null
              const lots = (run.equity * 0.01) / (risk * run.contract_size)
              const m = marginOf(lots, fill, run)
              return (
                <span className="num" title="one percent of equity against the distance to the stop, before the notional cap">
                  ≈{num(lots, 2)} lots{m ? ` · margin ${accountMoney(m.used, run, false)}` : ''}
                </span>
              )
            })()}
            {fill != null && p.stop != null && p.target != null && (
              <span className="num">
                R:R {num(Math.abs(p.target - fill) / Math.max(1e-9, Math.abs(fill - p.stop)), 2)}
              </span>
            )}
          </div>
          {p.reason && <p className="text-muted-foreground/80 mt-1.5 text-[11px] leading-snug">{p.reason}</p>}
        </div>
      )
    }
    return (
      <div className="mt-1.5 flex items-center gap-3 text-[11px]">
        <span className="text-muted-foreground text-[10px] tracking-wide uppercase">position</span>
        <span className="text-muted-foreground">flat</span>
      </div>
    )
  }

  const long = open.side === 'LONG'
  const price = live?.close ?? run.last_bar_close ?? open.entry_price
  const { usd, live: marked } = markToLive(open, price)
  const r = open.risk > 0 ? (price - open.entry_price) * (long ? 1 : -1) / open.risk : null

  // Left is ALWAYS the stop and right is ALWAYS the target, whichever way the
  // trade faces, so "left is bad, right is good" needs no thinking about.
  //
  // Anchoring on the stop rather than on the lower price is the whole point.
  // Ordering the track by price put a short's target on the left, while the
  // labels stayed stop-left/target-right: the marker for a position sitting
  // 2.4% from its stop was drawn hard against the right edge, under the word
  // "target". A chart that is merely unclear wastes a second; that one read as
  // the opposite of the truth, in the direction that loses money.
  const { stop, target } = open
  const span = stop != null && target != null ? target - stop : null
  const pos = (v: number) =>
    span && span !== 0 ? Math.min(100, Math.max(0, ((v - stop!) / span) * 100)) : null
  const atPrice = span ? pos(price) : null
  const atEntry = span ? pos(open.entry_price) : null

  return (
    <div className={cn('mt-2 rounded-sm border px-2.5 py-2', long ? 'border-lc/35 bg-lc/[0.06]' : 'border-lp/35 bg-lp/[0.06]')}>
      <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
        <span className={cn('num rounded-sm px-1 text-[11px] font-medium', long ? 'bg-lc/20 text-lc' : 'bg-lp/20 text-lp')}>
          {open.side}
        </span>
        <span className="num text-[13px]">{quote(open.entry_price)}</span>
        <span className="text-muted-foreground num text-[10px]">{num(open.lots, open.lots >= 100 ? 0 : 2)} lots</span>
        {(() => {
          const m = marginOf(open.lots, price, run)
          if (!m) return null
          return (
            <span
              className="text-muted-foreground/70 num text-[10px]"
              title={`margin used ${m.used.toFixed(2)} of ${run.account_currency}; a margin call comes at a level of 30%`}
            >
              margin {accountMoney(m.used, run, false)} ({m.pct.toFixed(2)}% · level{' '}
              {m.level > 9999 ? '>9999' : m.level.toFixed(0)}%)
            </span>
          )
        })()}
        <span className={cn('num ml-auto text-[15px] font-semibold', usd >= 0 ? 'text-lc' : 'text-lp')}>
          {accountMoney(usd, run)}
        </span>
        {r != null && (
          <span className={cn('num text-[11px]', r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(r)}</span>
        )}
      </div>

      {span ? (
        <>
          {/* stop ......... entry ... now ......... target */}
          <div className="relative mt-2 h-1.5 rounded-full bg-black/40">
            <div
              className="absolute inset-y-0 left-0 rounded-full bg-lp/30"
              style={{ width: `${atEntry ?? 0}%` }}
              aria-hidden
            />
            {atEntry != null && (
              <span className="bg-muted-foreground/70 absolute top-1/2 h-3 w-px -translate-y-1/2" style={{ left: `${atEntry}%` }} aria-hidden />
            )}
            {atPrice != null && (
              <span
                className={cn('absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-black/50', usd >= 0 ? 'bg-lc' : 'bg-lp')}
                style={{ left: `${atPrice}%` }}
                aria-hidden
              />
            )}
          </div>
          <div className="text-muted-foreground mt-1 flex justify-between text-[10px]">
            <span className="num text-lp">stop {quote(open.stop)}</span>
            <span className="num">{marked ? 'live' : 'at last close'} {quote(price)}</span>
            <span className="num text-lc">target {quote(open.target)}</span>
          </div>
        </>
      ) : (
        <div className="text-muted-foreground mt-1.5 text-[10px]">
          {open.stop == null && open.target == null
            ? 'self-managed: the strategy owns the exit, so there is no stop or target to sit between'
            : `stop ${quote(open.stop)} · target ${quote(open.target)} · the desk closes it at the maximum hold if neither is hit`}
        </div>
      )}

      <div className="text-muted-foreground/70 mt-1 text-[10px]">
        worst {signedR(open.mae)} · best {signedR(open.mfe)} · opened {shortStamp(open.entry_time)}
      </div>
    </div>
  )
}

/** Only the guards that actually fired, so an empty cell means a clean run. */
function GuardChips({ run }: { run: PaperRun }) {
  const closed = Object.entries(run.closed_by_guard).filter(([, n]) => n > 0)
  const refused = Object.values(run.skipped_by_guard).reduce((sum, n) => sum + n, 0)
  const chips: string[] = closed.map(([key, n]) => `${guardLabel(key)} ×${n}`)
  if (refused > 0) chips.push(`refused ×${refused}`)
  if (run.sized_down > 0) chips.push(`sized down ×${run.sized_down}`)
  if (chips.length === 0) {
    return <span className="text-muted-foreground/60 truncate text-[10px]">{run.guards ? 'none fired' : 'guards off'}</span>
  }
  return (
    <span className="flex min-w-0 flex-wrap gap-1">
      {chips.map((chip) => (
        <span key={chip} className="border-border text-muted-foreground num rounded-sm border px-1 py-px text-[10px]">
          {chip}
        </span>
      ))}
    </span>
  )
}

/* ------------------------------------------------------------- drilldown */

function Drilldown({
  detail,
  summary,
  broker,
  error,
  now,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail | null
  summary: PaperRun | null
  /** The selected account's record of this book, or null on the paper book. */
  broker: PaperBroker | null
  error: string | null
  now: number
  openFill: string | null
  onOpenFill: (key: string | null) => void
}) {
  if (error) {
    return (
      <div className="p-3">
        <Heading>Drill-down</Heading>
        <p className="text-destructive mt-1 text-[12px]">{error}</p>
      </div>
    )
  }
  if (!detail) {
    return (
      <div className="space-y-3 p-3">
        <Skeleton className="h-3 w-40" />
        <Skeleton className="h-[150px] w-full" />
        <Skeleton className="h-3 w-full" />
        <Skeleton className="h-3 w-5/6" />
      </div>
    )
  }

  const run = detail.run
  const stale = summary ? isStale(summary, now) : isStale(run, now)

  return (
    <div className="divide-border divide-y">
      <section className="px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="num text-[13px] font-medium">{run.id}</span>
          <StatusPill stale={stale} />
          <span className="text-muted-foreground num ml-auto text-[10px]">
            {run.market}:{run.tf} · last bar {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'none'}
          </span>
        </div>
        <LivePrice live={summary?.live ?? detail.live} lastClose={(summary ?? run).last_bar_close} now={now} />
        {run.label && <p className="text-muted-foreground mt-1 text-[11px] leading-snug">{run.label}</p>}
        {/* The two columns that left the runs table when it became a rail
            picker: both describe this one run rather than compare it to the
            others, so this is where they belonged all along. */}
        <PositionBar run={run} live={summary?.live ?? detail.live} />
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px]">
          <span className="text-muted-foreground text-[10px] tracking-wide uppercase">guards</span>
          <GuardChips run={run} />
        </div>
        <ConfigLine run={run} />
      </section>

      {broker ? (
        <AccountEquity broker={broker} />
      ) : (
        <section className="px-3 py-2">
          <Heading>Equity</Heading>
          <EquityCurve points={detail.equity_curve} />
          <p className="text-muted-foreground num mt-1 text-[10px]">
            {run.trades} closed{run.open ? ' · 1 open' : ''} · net {accountMoney(run.net_usd, run)} ·{' '}
            {run.profit_factor == null ? 'no PF yet' : `PF ${num(run.profit_factor)}`} ·{' '}
            {run.bars_seen} bars seen{run.gaps > 0 ? ` · ${run.gaps} gap${run.gaps === 1 ? '' : 's'}` : ''}
            {run.trades > 0 && run.trades < 30 && (
              <span className="text-caution"> · {run.trades} closed is too few to read as a result</span>
            )}
          </p>
        </section>
      )}

      <section className="px-3 py-2 xl:hidden">
        <Heading>
          Fills{' '}
          <span className="text-muted-foreground/70 num">
            {broker ? (broker.fills?.length ?? 0) : detail.fills.length}
          </span>
        </Heading>
        {broker ? (
          <BrokerFillsTable broker={broker} />
        ) : (
          <FillsTable detail={detail} openFill={openFill} onOpenFill={onOpenFill} />
        )}
      </section>

      <section className="px-3 py-2">
        <Heading>
          Events <span className="text-muted-foreground/70 num">{detail.events.length}</span>
          {/* Said out loud while an account is on screen: these are the BOOK's
              events - gaps in its feed, guards that fired, the run starting -
              and not the account's. */}
          {broker && <span className="text-muted-foreground/60 normal-case"> · paper book</span>}
        </Heading>
        <EventsList events={detail.events} />
      </section>
    </div>
  )
}

/**
 * The fills of the selected run, for the wide layout's left column.
 *
 * The same table the rail shows on a narrow screen; here it has the room
 * its seven columns want, under a runs table that rarely fills the height.
 */
/**
 * The live price, the size the drill-down has room for.
 *
 * The same number as the row above, read larger, with the quote it came from
 * and its age. The sentence under it is the whole point: this is the only
 * figure on the screen the bot has not acted on.
 */
function LivePrice({ live, lastClose, now }: { live: LiveBar | null; lastClose: number | null; now: number }) {
  if (!live) {
    return (
      <p className="text-muted-foreground mt-2 text-[11px]">
        No live price — nothing has posted a forming bar in the last 90 seconds.
      </p>
    )
  }
  const direction = liveDirection(live, lastClose)
  const spread = spreadOf(live)
  return (
    <div className="mt-2 flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <span
        className={cn(
          'num text-[22px] leading-none transition-colors duration-300 motion-reduce:transition-none',
          DIRECTION_CLASS[direction + 1],
        )}
      >
        <span aria-hidden>{DIRECTION_MARK[direction + 1]}</span> {quote(live.close)}
      </span>
      <span className="text-muted-foreground num text-[10px]">
        {live.bid != null && live.ask != null
          ? `bid ${quote(live.bid)} / ask ${quote(live.ask)}${spread ? ` · spread ${spread}` : ''}`
          : 'no quote'}
        {' · '}
        {liveAge(live.at, now)}
      </span>
      <span className="text-muted-foreground/70 text-[10px]">forming bar — not traded on</span>
    </div>
  )
}

function FillsSection({
  detail,
  openFill,
  onOpenFill,
  broker,
}: {
  detail: PaperRunDetail | null
  openFill: string | null
  onOpenFill: (key: string | null) => void
  /** The selected account's record of this book, or null on the paper book. */
  broker: PaperBroker | null
}) {
  if (!detail) {
    return (
      <div className="space-y-2 px-3 py-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="h-3 w-full" />
      </div>
    )
  }
  return (
    <section className="px-3 py-2">
      <Heading>
        Fills <span className="text-muted-foreground/70 num">{detail.run.id}</span>{' '}
        <span className="text-muted-foreground/70 num">
          {broker ? (broker.fills?.length ?? 0) : detail.fills.length}
        </span>
        {broker && <span className="text-muted-foreground/60 normal-case"> · on {broker.account}</span>}
      </Heading>
      {broker ? (
        <BrokerFillsTable broker={broker} />
      ) : (
        <FillsTable detail={detail} openFill={openFill} onOpenFill={onOpenFill} />
      )}
    </section>
  )
}

function Heading({ children }: { children: React.ReactNode }) {
  return <h2 className="text-muted-foreground mb-1 text-[10px] font-semibold tracking-wide uppercase">{children}</h2>
}

/** Strategy, parameters, filters and whether the guards are enforced — one line. */
function ConfigLine({ run }: { run: PaperRun }) {
  const params = Object.entries(run.params)
    .map(([k, v]) => `${k}=${Number.isInteger(v) ? v : num(v, 2)}`)
    .join(' ')
  return (
    <p className="text-muted-foreground num mt-1.5 text-[10px] leading-relaxed break-words">
      <span className="text-foreground/80">{run.strategy}</span>
      {params && <> · {params}</>}
      {run.filters.length > 0 && <> · {run.filters.join(', ')}</>}
      {' · '}
      <span className={run.guards ? 'text-lc' : 'text-caution'}>{run.guards ? 'guards on' : 'guards off'}</span>
      {' · '}
      {run.news.events_loaded} news events
      {run.news.next_blackout && (
        <>
          {' · next blackout '}
          <span className="text-caution">
            {run.news.next_blackout.name ?? run.news.next_blackout.currency} {zulu(run.news.next_blackout.time)}
          </span>
        </>
      )}
    </p>
  )
}

/* ------------------------------------------------------------------ chart */

/**
 * Indicator line colours, named rather than written out.
 *
 * Lightweight Charts paints on a canvas, where `var(--sp)` is not a colour, so
 * a token has to be resolved to whatever it currently computes to — but the
 * component still never carries a colour of its own. Lime is deliberately
 * absent: it is the one chrome accent (ui/DESIGN.md) and an indicator line is
 * data, not a control.
 */
const LINE_TOKENS = ['--chart-2', '--chart-3', '--chart-4', '--chart-5', '--caution']

/** What a palette token computes to right now. `gray` is a last resort no theme reaches. */
function tokenValue(name: string): string {
  if (typeof window === 'undefined') return 'gray'
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || 'gray'
}

/**
 * The candles the run stepped, its own indicators, and every fill on top.
 *
 * Nothing here is chosen for the run: `detail.indicators` is what the
 * strategy's definition declares it reads, and `overlay` on each one says
 * whether its series belongs over the candles or wants a pane of its own.
 */
function RunChart({
  detail,
  live,
  focus,
  broker,
}: {
  detail: PaperRunDetail | null
  live: LiveBar | null
  /** The fill picked in the table below, or null. */
  focus: BacktestTrade | null
  /** The selected account's record of this book, when the desk is showing an
   *  account rather than the paper book. */
  broker: PaperBroker | null
}) {
  // `bars` arrives as `[ms, o, h, l, c]` and `PriceChart` takes milliseconds
  // and divides, so the tuple goes straight across. (`series` times are
  // already seconds, which is what the chart wants there — the two halves of
  // the payload are in different units and neither is converted here.)
  const bars = useMemo<Bar[]>(
    () => (detail?.bars ?? []).map(([time, open, high, low, close]) => ({ time, open, high, low, close })),
    [detail],
  )

  // The forming candle, in the shape the chart takes. `PriceChart` appends it
  // past the last closed bar and ignores a frame older than one, so a stream
  // that has fallen behind draws nothing rather than a candle in the past.
  const forming = useMemo<Bar | null>(
    () =>
      live
        ? { time: live.time, open: live.open, high: live.high, low: live.low, close: live.close }
        : null,
    [live],
  )

  const indicators = useMemo<ActiveIndicator[]>(() => {
    const active: ActiveIndicator[] = []
    for (const entry of detail?.indicators ?? []) {
      // The wire sends fully-qualified outputs (`ema_21.ema`) and `PriceChart`
      // glues `key + '.' + output` back together, so the prefix comes off
      // here. Sliced by length rather than split on '.', because a float
      // parameter puts a dot inside the key itself: `keltner_20_10_1.5`.
      const prefix = `${entry.key}.`
      active.push({
        key: entry.key,
        id: entry.id,
        params: entry.params,
        outputs: entry.outputs.map((o) => (o.startsWith(prefix) ? o.slice(prefix.length) : o)),
        // Pane 0 is the candles. Every indicator whose own definition says it
        // does not belong over them gets a pane of its own, numbered from 1.
        pane: entry.overlay ? 0 : active.filter((e) => e.pane > 0).length + 1,
        color: tokenValue(LINE_TOKENS[active.length % LINE_TOKENS.length]),
      })
    }
    return active
  }, [detail])

  if (!detail) {
    return (
      <div className="space-y-2 px-3 py-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="w-full" style={{ height: CHART_H - 26 }} />
      </div>
    )
  }

  if (bars.length === 0) {
    return (
      <section className="px-3 py-2">
        <Heading>Chart</Heading>
        <p className="text-muted-foreground py-8 text-center text-[11px]">
          No bars to draw — the poller has fed this run nothing yet.
        </p>
      </section>
    )
  }

  return (
    <section className="flex h-full min-h-0 flex-col px-3 py-2">
      <Heading>
        Chart <span className="text-muted-foreground/70 num">{detail.run.market}:{detail.run.tf}</span>{' '}
        <span className="text-muted-foreground/70 num">{bars.length} bars</span>
        {focus && (
          <span className="text-primary num ml-2 normal-case">
            showing the {focus.direction.toLowerCase()} closed {shortStamp(focus.exitTime)} — click the row again to
            release
          </span>
        )}
      </Heading>
      <div className="border-border min-h-0 w-full flex-1 overflow-hidden rounded-sm border" style={{ minHeight: CHART_H - 60 }}>
        {/* Keyed by run: a new run brings a different set of panes, and
            remounting is cheaper to reason about than reconciling them. */}
        {/* No stop bands on the account's trades: the stop belongs to the
            book, and drawing the book's level around a broker's fill would mix
            the two records the switch exists to keep apart. */}
        <PriceChart
          key={detail.run.id}
          open={detail.run.open}
          pending={detail.run.pending}
          pendingFill={detail.run.pending ? pendingFill(detail.run.pending, live, detail.run.tf) : null}
          bars={bars}
          indicators={indicators}
          series={detail.series ?? {}}
          frame={null}
          showLevels={false}
          trades={broker ? accountTrades(broker) : detail.fills}
          showMarkers
          showZones={!broker}
          focus={focus}
          liveBar={forming}
        />
      </div>
      <p className="text-muted-foreground mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px]">
        {indicators.length === 0 ? (
          <span>This strategy reads no indicator — it trades the clock or the bar itself.</span>
        ) : (
          indicators.map((entry) => (
            <span key={entry.key} className="num inline-flex items-center gap-1">
              <span
                className="inline-block h-px w-3 align-middle"
                style={{ backgroundColor: entry.color }}
                aria-hidden
              />
              {entry.key}
              {entry.pane > 0 && <span className="text-muted-foreground/60">pane {entry.pane}</span>}
            </span>
          ))
        )}
        <span className="text-muted-foreground/70">
          {broker
            ? `marks are ${broker.account}'s own fills, at the prices it got — no stop bands, because the stop belongs to the book and not to the account.`
            : 'entries and exits are marked; the bands behind them are each fill’s stop and target.'}
        </span>
        <span className="text-muted-foreground/70">
          The bot decides on closed bars only; the last candle is still forming and is never traded on.
        </span>
      </p>
    </section>
  )
}

/* ---------------------------------------------------------- equity curve */

/**
 * The plot's own coordinate space. The margins are part of the `viewBox` so
 * the outermost label — a five-figure dollar amount on the left, a stamp at
 * each end — has room inside the box and nothing clips when the SVG is scaled
 * to the pane's width.
 */
const PLOT = { w: 700, h: 168, l: 62, r: 16, t: 12, b: 26 }

/**
 * A stepped equity curve, drawn by hand.
 *
 * Equity changes at a fill and holds between them, so the line steps rather
 * than slopes: a diagonal between two fills would draw money the book never
 * had. The curve is not sorted on the wire — warm-up trades close before the
 * run's own start stamp — so it is sorted here before anything is measured.
 */
/**
 * The account's balance through this book's trades, and what it came to.
 *
 * Built from the broker's own closed trades rather than from the paper book's
 * equity curve, and it starts from the balance the account had BEFORE them
 * (`balance - realised`), so the line is a real account balance and not a
 * cumulative P&L drawn as though it were one.
 *
 * No profit factor. It is a ratio of gross win to gross loss and the broker
 * reports neither; computing it from this handful of fills would be a
 * different number from the book's under the same name.
 */
function AccountEquity({ broker }: { broker: PaperBroker }) {
  const fills = (broker.fills ?? []).filter((f) => f.exitTime != null)
  const base = (broker.balance ?? 0) - (broker.realised ?? 0)
  const points: [number, number][] = []
  if (fills.length > 0) {
    const first = fills[0]
    points.push([(first.entryTime ?? first.exitTime) as number, base])
    let cum = base
    for (const f of fills) {
      cum += f.pnl ?? 0
      points.push([f.exitTime as number, cum])
    }
  }
  const open = broker.position?.profit ?? null
  return (
    <section className="px-3 py-2">
      <Heading>
        Equity <span className="text-muted-foreground/70 num normal-case">on {broker.account}</span>
      </Heading>
      {points.length === 0 ? (
        <p className="text-muted-foreground px-1 py-6 text-center text-[11px]">
          No curve yet — this account has closed no trade on this book.
        </p>
      ) : (
        <EquityCurve points={points} />
      )}
      <p className="text-muted-foreground num mt-1 text-[10px]">
        {broker.closed ?? 0} closed{broker.position ? ' · 1 open' : ''} · banked{' '}
        <span className={(broker.realised ?? 0) >= 0 ? 'text-lc' : 'text-lp'}>
          {brokerMoney(broker.realised, broker.currency)}
        </span>
        {open != null && (
          <>
            {' · open '}
            <span className={open >= 0 ? 'text-lc' : 'text-lp'}>{brokerMoney(open, broker.currency)}</span>
          </>
        )}
        {broker.dry_run && <span className="text-muted-foreground/60"> · dry run, nothing was sent</span>}
      </p>
    </section>
  )
}

/**
 * What the account actually filled, one row per completed trade.
 *
 * Fewer columns than the paper book's table and that is the point: there is no
 * R, no MAE and no MFE here, because a broker has no stop distance to measure
 * them against. Showing the columns with zeroes in them would read as a
 * measurement that nobody made.
 */
function BrokerFillsTable({ broker }: { broker: PaperBroker }) {
  const fills = [...(broker.fills ?? [])].reverse()
  if (fills.length === 0) {
    return (
      <p className="text-muted-foreground px-1 py-4 text-[11px]">
        This account has filled nothing on this book yet.
        {broker.dry_run && ' It is in dry run, so it never will until that is turned off.'}
      </p>
    )
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-[11px]">
        <thead className="text-muted-foreground text-[10px] tracking-wide uppercase">
          <tr className="border-b">
            <th className="py-1 pr-2 text-left font-medium">exit time (+07)</th>
            <th className="py-1 pr-2 text-left font-medium">side</th>
            <th className="py-1 pr-2 text-right font-medium">lots</th>
            <th className="py-1 pr-2 text-right font-medium">entry &rarr; exit</th>
            <th className="py-1 pr-2 text-left font-medium">reason</th>
            <th className="py-1 text-right font-medium">P&amp;L</th>
          </tr>
        </thead>
        <tbody>
          {fills.map((f, i) => (
            <tr key={`${f.entryTime}-${f.exitTime}-${i}`} className="border-b last:border-0">
              <td className="num py-1 pr-2">{vnStamp(f.exitTime)}</td>
              <td className={cn('num py-1 pr-2', f.direction === 'LONG' ? 'text-lc' : 'text-lp')}>
                {(f.direction ?? '').toLowerCase()}
              </td>
              <td className="num py-1 pr-2 text-right">{f.lots}</td>
              <td className="num py-1 pr-2 text-right">
                {quote(f.entryPrice)} &rarr; {quote(f.exitPrice)}
              </td>
              <td className="text-muted-foreground py-1 pr-2">{f.exitReason || '--'}</td>
              <td
                className={cn(
                  'num py-1 text-right',
                  (f.pnl ?? 0) > 0 && 'text-lc',
                  (f.pnl ?? 0) < 0 && 'text-lp',
                )}
              >
                {brokerMoney(f.pnl, broker.currency)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function EquityCurve({ points }: { points: [number, number][] }) {
  if (!points || points.length === 0) {
    return (
      <p className="text-muted-foreground px-1 py-6 text-center text-[11px]">
        No curve yet — the run has closed no trade.
      </p>
    )
  }

  const sorted = [...points].sort((a, b) => a[0] - b[0])
  const first = sorted[0]
  const last = sorted[sorted.length - 1]
  const start = first[1]
  const times = sorted.map((p) => p[0])
  const values = sorted.map((p) => p[1])
  const tMin = Math.min(...times)
  const tMax = Math.max(...times)
  let vMin = Math.min(...values)
  let vMax = Math.max(...values)
  // One point, or a book that has not moved: a zero range would divide by zero
  // and a flat line pinned to an edge reads as a bug. Give it a dollar either
  // way, and say nothing about a min and a max the book never had.
  const flat = !(vMax - vMin > 1e-9)
  if (flat) {
    vMin -= 1
    vMax += 1
  }

  const { w, h, l, r, t, b } = PLOT
  const boxW = l + w + r
  const boxH = t + h + b
  const bottom = t + h
  const x = (ms: number) => (tMax === tMin ? l + w / 2 : l + ((ms - tMin) / (tMax - tMin)) * w)
  const y = (v: number) => t + ((vMax - v) / (vMax - vMin)) * h

  let line = `M ${x(first[0]).toFixed(1)} ${y(first[1]).toFixed(1)}`
  for (let i = 1; i < sorted.length; i += 1) {
    const px = x(sorted[i][0]).toFixed(1)
    line += ` L ${px} ${y(sorted[i - 1][1]).toFixed(1)} L ${px} ${y(sorted[i][1]).toFixed(1)}`
  }
  const area = `${line} L ${x(last[0]).toFixed(1)} ${bottom} L ${x(first[0]).toFixed(1)} ${bottom} Z`

  const up = last[1] >= start
  const dollars = (v: number) => `$${Math.round(v).toLocaleString('en-US')}`

  return (
    <svg
      viewBox={`0 0 ${boxW} ${boxH}`}
      className={cn('w-full', up ? 'text-lc' : 'text-lp')}
      role="img"
      aria-label={`Equity from ${dollars(start)} to ${dollars(last[1])} over ${sorted.length} point${sorted.length === 1 ? '' : 's'}`}
    >
      {/* The plot's floor and left edge: a frame, not a grid. */}
      <path
        d={`M ${l} ${t} L ${l} ${bottom} L ${l + w} ${bottom}`}
        className="stroke-border"
        strokeWidth={1}
        fill="none"
      />
      {/* Where the run started. Everything above this line is profit. */}
      <line
        x1={l}
        x2={l + w}
        y1={y(start)}
        y2={y(start)}
        className="stroke-muted-foreground"
        strokeWidth={1}
        strokeDasharray="4 4"
        opacity={0.6}
      />
      <path d={area} fill="currentColor" opacity={0.12} stroke="none" />
      <path d={line} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round" />
      <circle cx={x(last[0])} cy={y(last[1])} r={3} fill="currentColor" />

      {/* y: the floor, the start, the ceiling — the two ends only when the
          book actually reached them. */}
      {!flat && (
        <text x={l - 6} y={t + 4} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
          {dollars(vMax)}
        </text>
      )}
      <text x={l - 6} y={y(start) + 3.5} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
        {dollars(start)}
      </text>
      {!flat && (
        <text x={l - 6} y={bottom} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
          {dollars(vMin)}
        </text>
      )}

      {/* x: first and last stamp, no ticks between — the steps carry the rest. */}
      <text x={l} y={boxH - 8} textAnchor="start" className="fill-muted-foreground" fontSize={10}>
        {shortStamp(tMin)}
      </text>
      <text x={l + w} y={boxH - 8} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
        {shortStamp(tMax)}
      </text>
    </svg>
  )
}

/* ------------------------------------------------------------------ fills */

const FILL_COLS = 'grid-cols-[96px_44px_56px_minmax(120px,1fr)_96px_72px_64px]'

/** Identifies a fill across polls: a run cannot open two trades on the same bar. */
const fillId = (fill: BacktestTrade, index: number) => `${fill.entryTime}-${fill.exitTime}-${index}`

/**
 * The run's closed trades, one per row, newest first.
 *
 * A row is a button: clicking it (or pressing Enter on it) opens the account of
 * that fill beneath the table, and clicking it again closes it. ↑/↓ walk the
 * rows the way they do in the runs table above.
 */
function FillsTable({
  detail,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail
  openFill: string | null
  onOpenFill: (key: string | null) => void
}) {
  const rowRefs = useRef<(HTMLButtonElement | null)[]>([])
  // Newest first, but keyed by the position the fill holds in the book, so the
  // key survives the reversal and the next poll.
  const rows = useMemo(
    () => detail.fills.map((fill, index) => ({ fill, key: fillId(fill, index) })).reverse(),
    [detail.fills],
  )
  const open = rows.find((row) => row.key === openFill) ?? null

  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, rows.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  if (rows.length === 0) {
    return (
      <p className="text-muted-foreground py-2 text-[11px]">
        None yet — the first live bar decides; the warm-up bars do not.
      </p>
    )
  }

  return (
    <>
      <div className="overflow-x-auto">
        <div className="min-w-[600px]" role="group" aria-label="Closed fills">
          <div
            className={cn(
              'text-muted-foreground grid items-center gap-2 border-b pb-1 text-[10px] tracking-wide uppercase',
              FILL_COLS,
            )}
          >
            <span>exit time (+07)</span>
            <span>side</span>
            <span className="text-right">lots</span>
            <span className="text-right">entry → exit</span>
            <span>exit reason</span>
            <span className="text-right">P&amp;L</span>
            <span className="text-right">R</span>
          </div>
          {rows.map(({ fill, key }, index) => {
            const isOpen = key === openFill
            return (
              <button
                key={key}
                type="button"
                ref={(el) => {
                  rowRefs.current[index] = el
                }}
                onClick={() => onOpenFill(isOpen ? null : key)}
                onKeyDown={(e) => onKeyDown(e, index)}
                aria-expanded={isOpen}
                aria-controls={`fill-account-${detail.run.id}`}
                className={cn(
                  'hover:bg-accent/60 focus-visible:ring-ring grid w-full items-center gap-2 border-b py-[3px] text-left text-[11px] transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none motion-reduce:transition-none',
                  FILL_COLS,
                  isOpen && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
                )}
              >
                <span className="num text-muted-foreground" title={`exit ${utcStamp(fill.exitTime)} · entry ${utcStamp(fill.entryTime)}`}>
                  {vnStamp(fill.exitTime)}
                </span>
                <span className={cn('num', fill.direction === 'LONG' ? 'text-lc' : 'text-lp')}>
                  {fill.direction.toLowerCase()}
                </span>
                <span className="num text-right">{num(fill.lots, fill.lots >= 100 ? 0 : 2)}</span>
                <span className="num text-right">
                  {quote(fill.entryPrice)} <span className="text-muted-foreground">→</span> {quote(fill.exitPrice)}
                </span>
                <span className="text-muted-foreground truncate">
                  {fill.exitReason.toLowerCase().replace(/_/g, ' ')}
                </span>
                <span className={cn('num text-right', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>
                  {signedUsd(fill.pnlUsd)}
                </span>
                <span className={cn('num text-right', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>
              </button>
            )
          })}
        </div>
      </div>
      <div id={`fill-account-${detail.run.id}`}>
        {open ? (
          <FillAccount fill={open.fill} run={detail.run} />
        ) : (
          <p className="text-muted-foreground/70 mt-1.5 text-[10px]">
            Pick a fill to read what the strategy saw, where its stop and target sat, and how it ended.
          </p>
        )}
      </div>
    </>
  )
}

/* ------------------------------------------------------- one fill, in words */

/** `2 h 15 min` — the scale a person holding a trade thinks in. */
const heldFor = (ms: number | null | undefined): string => {
  if (ms == null || !Number.isFinite(ms) || ms <= 0) return 'less than a minute'
  const minutes = Math.round(ms / 60_000)
  if (minutes < 1) return 'less than a minute'
  if (minutes < 60) return `${minutes} min`
  const hours = Math.floor(minutes / 60)
  const restMin = minutes % 60
  if (hours < 24) return restMin > 0 ? `${hours} h ${restMin} min` : `${hours} h`
  const days = Math.floor(hours / 24)
  const restH = hours % 24
  return restH > 0 ? `${days} d ${restH} h` : `${days} d`
}

/**
 * What ended the trade, in words.
 *
 * The guards spell themselves in SCREAMING_CASE and the engine's own exits are
 * `STOP` and `TARGET`. Anything else is the strategy describing its own rule —
 * `window closed`, say — and is quoted rather than paraphrased, because a
 * paraphrase of a sentence we did not write is a sentence we invented.
 */
const exitSentence = (reason: string): string => {
  switch (reason.trim().toUpperCase()) {
    case 'STOP':
      return 'Price reached the stop'
    case 'TARGET':
      return 'Price reached the target'
    case 'WEEKEND_FLAT':
      return 'The weekend guard closed it before Friday’s close'
    case 'NEWS_FLAT':
      return 'The news guard closed it before a release'
    case 'OPEN_LOSS_CAP':
      return 'The open-loss cap closed it at 2R'
    case 'DAILY_LOSS_LIMIT':
      return 'The daily loss limit closed the book, and this trade with it'
    default:
      return `The strategy’s own rule closed it — “${reason}”`
  }
}

/**
 * One fill, read out.
 *
 * Everything here is on the fill itself; nothing is modelled. `mae` and `mfe`
 * arrive already divided by the position's risk (see `BacktestTrade`), so they
 * are R and need no conversion — which is why the stop distance below is only
 * ever used for the *target's* R, never for the excursion's.
 */
function FillAccount({ fill, run }: { fill: BacktestTrade; run: PaperRun }) {
  const long = fill.direction === 'LONG'
  const side = long ? 'long' : 'short'
  const lots = num(fill.lots, fill.lots >= 100 ? 0 : 2)

  const stopAway = Math.abs(fill.entryPrice - fill.stop)
  // Prices come off the wire rounded to two decimals, so on a five-decimal
  // market a stop a few pips away can arrive equal to the entry. "0.00 away"
  // would be a lie; saying nothing about the distance in price is not.
  const stopKnown = Number.isFinite(stopAway) && stopAway > 0
  const targetAway = fill.target == null ? null : Math.abs(fill.target - fill.entryPrice)
  const targetR = targetAway != null && stopKnown ? targetAway / stopAway : null

  return (
    <div className="border-border bg-card/50 mt-2 space-y-1.5 rounded-sm border px-3 py-2 text-[12px] leading-relaxed">
      <p className="text-muted-foreground">
        {fill.reason ? (
          <>
            <Label>What it saw</Label> the strategy wrote “
            <span className="text-foreground">{fill.reason}</span>” as its reason for taking this one.
          </>
        ) : (
          <>
            <Label>What it saw</Label> the run recorded no entry sentence for this fill.
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>Entry</Label> it went{' '}
        <span className={cn('num', long ? 'text-lc' : 'text-lp')}>{side}</span>{' '}
        <span className="num text-foreground">{lots}</span> lots of{' '}
        <span className="num">{run.market}</span> at{' '}
        <span className="num text-foreground">{quote(fill.entryPrice)}</span>, on{' '}
        <span className="num">{shortStamp(fill.entryTime)}</span>.
      </p>

      <p className="text-muted-foreground">
        <Label>SL</Label> the stop sat at <span className="num text-foreground">{quote(fill.stop)}</span>
        {stopKnown ? (
          <>
            , <span className="num">{quote(stopAway)}</span> in price {long ? 'below' : 'above'} the entry
          </>
        ) : (
          <> (closer to the entry than the two decimals this feed reports prices in)</>
        )}
        . That distance <em className="not-italic">is</em> one R by construction — every R figure on this fill is a
        multiple of it.
        {fill.target == null && (
          <>
            {' '}
            Here it is only the sizing unit: this strategy exits on its own rule, so the stop says how big the trade
            was, not how it was meant to end.
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>TP</Label>{' '}
        {fill.target == null ? (
          <>no target — the strategy&rsquo;s own rule closes it, and nothing was set to take profit at.</>
        ) : (
          <>
            the target sat at <span className="num text-foreground">{quote(fill.target)}</span>
            {targetAway != null && (
              <>
                , <span className="num">{quote(targetAway)}</span> in price {long ? 'above' : 'below'} the entry
              </>
            )}
            {targetR != null && (
              <>
                {' '}
                — <span className="num text-foreground">{targetR.toFixed(2)}R</span> of reward against the 1R it was
                risking
              </>
            )}
            .
          </>
        )}
      </p>

      <p className="text-muted-foreground">
        <Label>What happened</Label> {exitSentence(fill.exitReason)}. It closed at{' '}
        <span className="num text-foreground">{quote(fill.exitPrice)}</span> on{' '}
        <span className="num">{shortStamp(fill.exitTime)}</span>, held{' '}
        <span className="num">{heldFor(fill.holdMs)}</span>, for{' '}
        <span className={cn('num', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>{signedUsd(fill.pnlUsd)}</span> —{' '}
        <span className={cn('num', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>.
      </p>

      <p className="text-muted-foreground">
        <Label>On the way</Label> it went as far as{' '}
        <span className="num text-lc">{signedR(fill.mfe)}</span> in favour and{' '}
        <span className="num text-lp">{signedR(fill.mae)}</span> against before it closed.
      </p>

      {run.trades < 30 && (
        <p className="text-caution border-border border-t pt-1.5 text-[11px]">
          This is one trade out of {run.trades} this run has closed. Fewer than 30 closed trades cannot be read as a result —
          neither this fill nor the run&rsquo;s total says whether the strategy works.
        </p>
      )}
    </div>
  )
}

/** The word that opens each sentence. Weight, not colour — lime is for controls. */
function Label({ children }: { children: React.ReactNode }) {
  return <span className="text-foreground font-medium">{children} — </span>
}

/* ----------------------------------------------------------------- events */

/** What each event line says, in the reader's words rather than the wire's. */
function eventLine(event: PaperEvent): string {
  switch (event.kind) {
    case 'started':
      return event.guards ? `guards: ${event.guards}` : 'guards off'
    case 'gap':
      return `${event.missing_bars ?? 0} bar${event.missing_bars === 1 ? '' : 's'} missing before this one`
    case 'refused':
      return `entry refused — ${guardLabel(event.reason ?? '')}`
    case 'guard_close':
      return `position closed — ${guardLabel(event.reason ?? '')}`
    case 'stopped':
      return `book closed at ${event.equity == null ? '—' : `$${Math.round(event.equity).toLocaleString('en-US')}`} over ${event.trades ?? 0} fills`
    default:
      return event.reason ?? ''
  }
}

const EVENT_TONE: Record<string, string> = {
  started: 'text-muted-foreground',
  gap: 'text-caution',
  refused: 'text-caution',
  guard_close: 'text-lp',
  stopped: 'text-lp',
}

function EventsList({ events }: { events: PaperEvent[] }) {
  if (events.length === 0) {
    return <p className="text-muted-foreground py-2 text-[11px]">Nothing but fills — no gap, no refusal.</p>
  }
  return (
    <ul className="space-y-0.5">
      {[...events].reverse().map((event, index) => (
        <li key={`${event.kind}-${event.time}-${index}`} className="grid grid-cols-[96px_78px_1fr] items-start gap-2 text-[11px]">
          <span className="num text-muted-foreground">{shortStamp(event.time)}</span>
          <span className={cn('num truncate', EVENT_TONE[event.kind] ?? 'text-muted-foreground')}>{event.kind}</span>
          <span className="text-muted-foreground leading-snug break-words">{eventLine(event)}</span>
        </li>
      ))}
    </ul>
  )
}
