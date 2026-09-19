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

import { HtfCard } from '@/components/HtfCard'
import { LevelsCard } from '@/components/LevelsCard'
import { useHtf } from '@/lib/useHtf'
import { usePriceLevels } from '@/lib/usePriceLevels'
import { PriceChart, type ActiveIndicator, type ChartLevel, type ChartTrade } from '@/components/PriceChart'
import { IndicatorPicker, useViewerIndicators } from '@/components/IndicatorPicker'
import { LevelToggles } from '@/components/LevelToggles'
import {
  LIVE_WINDOW_ATR,
  familyOf,
  foldSamePrice,
  harvestLevels,
  hiddenSentence,
  readLevelFamilies,
  selectLevels,
  writeLevelFamilies,
  type LevelFamily,
} from '@/lib/levels'
import { Skeleton } from '@/components/ui/skeleton'
import type { Book } from '@/App'
import { api, type BacktestTrade, type Bar, type BrokerEvent, type IndicatorPoint, type LiveBar, type PaperBroker, type PaperEvent, type PaperRun, type PaperRunDetail } from '@/lib/api'
import { TimeframeBar } from '@/components/TimeframeBar'
import { useChartBars } from '@/lib/useChartBars'
import {
  TF_MS,
  TF_WORD,
  barsBehindBy,
  isTimeframe,
  readTimeframe,
  snapToBar,
  writeTimeframe,
  type Timeframe,
} from '@/lib/timeframes'
import { clock, num, liveAge, since } from '@/lib/format'
import { toast } from 'sonner'
import { ClaudeMark, DeepSeekMark, OpenAIMark } from '@/components/BrandMarks'
import type { Consultation, Decision, Reasoning,
  HtfResponse,
  PriceLevelsResponse,
} from '@/lib/api'
import { APP_BAR_H } from '@/components/AppBar'
import { cn } from '@/lib/utils'


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

/** Fallback for a timeframe this table does not know: the old flat window. */
const STALE_MS_FALLBACK = 45 * 60_000

/** The selected run, remembered per browser. A reload should land where you were. */
const SELECTED_KEY = 'fd.desk.selected'

/** `/status` sends the last ten fills per run; a count from them can only be a floor. */
const LAST_FILLS_CAP = 10

/** Whether the open position is drawn, remembered per browser. */
const SHOW_OPEN_KEY = 'fd.desk.showOpen'

/** Whether the higher timeframe's levels are drawn, remembered per viewer. */
const SHOW_HTF_KEY = 'fd.desk.showHtf'

const readShowHtf = (): boolean => {
  try {
    // Absent means ON. They are thin dashed lines behind everything else, and
    // a level you did not know was there is the one that surprises you.
    return localStorage.getItem(SHOW_HTF_KEY) !== '0'
  } catch {
    return true
  }
}
const writeShowHtf = (on: boolean) => {
  try {
    localStorage.setItem(SHOW_HTF_KEY, on ? '1' : '0')
  } catch {
    /* private mode: the choice lasts the page */
  }
}

/** Whether the spent levels — pools already swept, blocks already broken —
 *  are drawn as well as the live ones. */
const SHOW_SPENT_KEY = 'fd.desk.levelsSpent'

const readShowSpent = (): boolean => {
  try {
    // Absent means OFF, and this is the one level switch that defaults off.
    // On the captured response of 2026-09-19 the route served 252 levels of
    // which 197 are spent: drawing them all is not a chart, it is a wash. It
    // is a VIEW and not a verdict — the count of what is hidden is on screen
    // beside this switch, and one press brings every one of them back.
    return localStorage.getItem(SHOW_SPENT_KEY) === '1'
  } catch {
    return false
  }
}
const writeShowSpent = (on: boolean) => {
  try {
    localStorage.setItem(SHOW_SPENT_KEY, on ? '1' : '0')
  } catch {
    /* private mode: the choice lasts the page */
  }
}

const readShowOpen = (): boolean => {
  try {
    // Absent means ON: the live position is the only thing on the chart that
    // can still cost anything, so it is drawn unless someone has said not to.
    return localStorage.getItem(SHOW_OPEN_KEY) !== '0'
  } catch {
    return true
  }
}
const writeShowOpen = (on: boolean) => {
  try {
    localStorage.setItem(SHOW_OPEN_KEY, on ? '1' : '0')
  } catch {
    /* private mode: the choice lasts the page */
  }
}

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
 * The paper book's money, which is US DOLLARS and stays that way.
 *
 * This was `accountMoney` and it CONVERTED paper USD into the account's units,
 * so a reader would see "the number the account holder actually sees". On a
 * desk that shows a paper book beside the real position mirroring it, that is
 * the wrong kindness. The owner's card rendered a 0.04-lot paper short as
 * "−12 USC" while the ACCOUNT's own position on the same book read "+2.72
 * USC" — one quantity appearing to disagree with itself, read as a broken
 * deploy by the person whose money it is. They are two different trades:
 * different entry, different size, one mirrored at `lot_scale`. Giving them a
 * common unit is what made them look comparable.
 *
 * So the conversion is DELETED rather than discouraged. A function that turns
 * paper into account units cannot be misapplied if it does not exist, and
 * `PaperRun.account_currency` / `units_per_usd` are carried on the wire and
 * now deliberately unused by this client.
 *
 * TWO DECIMALS, which is the half of 9d80cc6 that was right: the `signedUsd`
 * it replaced rounded to whole dollars, so a book risking 1% of USD 100
 * rendered every result as "−$0" or "+$1". That diagnosis stands; only its
 * remedy is reversed.
 *
 * THE MINUS SIGN HERE IS LOAD-BEARING, AND WHAT IT NOW MEANS HAS CHANGED.
 * This writes U+2212; `brokerMoney` below writes the ASCII hyphen
 * `toLocaleString` produces. While the old `accountMoney` converted, the tell
 * caught a multiplication by `units_per_usd`. Nothing converts any more, so it
 * marks something simpler and more useful: U+2212 is PAPER money in dollars,
 * ASCII is ACCOUNT money in the account's currency. On the position card the
 * two sit inches apart and are different trades, so which is which has to be
 * readable without reading the code.
 *
 * Whoever unifies these two - and it is a reasonable thing to want - has to
 * replace that check with something, not merely delete it.
 */
function paperMoney(usd: number | null | undefined, signed = true): string {
  if (usd == null || !Number.isFinite(usd)) return '—'
  const body = Math.abs(usd).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  const sign = signed ? (usd < 0 ? MINUS : '+') : usd < 0 ? MINUS : ''
  return `${sign}$${body}`
}

/** A signed dollar figure. The sign is the point, so it is never dropped. */
/*
 * `sharedUnit` stood here and went with the conversion it served. It decided
 * whether every run on screen agreed on an account unit, so a folded total
 * could be shown in it. Nothing converts any more, so there is nothing to
 * agree about: a total of paper money is paper money.
 */

/*
 * `signedUsd` stood here and is deliberately gone rather than left unused.
 *
 * It rendered whole dollars: `toFixed(0)`. On this desk a book risks 1% of a
 * USD 100 equity, so a typical result is well under a dollar and every row it
 * touched collapsed to "−$0" or "+$1". The open row shipped on 2026-09-17 used
 * `accountMoney` instead and read "−37 USC" beside closed rows reading "−$1" —
 * the same scale, a hundred apart, in one panel. Keeping the function would
 * leave the rounding for someone to reach for again.
 */

/**
 * The heading over a group of fill rows.
 *
 * The panel used to be one list with the open position as a tinted first row,
 * and its heading counted CLOSED fills only - so it read "6" over seven rows.
 * Splitting the list is what makes each number agree with what is under it.
 */
const GROUP_LABEL =
  'text-muted-foreground border-b py-1 fd-caption font-medium tracking-wide uppercase'

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
/* `liveAge` moved to lib/format.ts: the status strip shows the same age and
 * the two must not phrase it differently. */

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
function accountTrades(broker: PaperBroker | null): ChartTrade[] {
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
      // NOT `pnlUsd`. `BrokerFill.pnl` is the ACCOUNT's currency - USC on the
      // funded cent account - and `pnlUsd` means dollars to every other
      // producer and consumer of a `BacktestTrade`. Assigning one to the other
      // is how a real-money P&L ends up rendered a hundred times too large
      // under a dollar sign; `Analytics.tsx` already sums `pnlUsd` and prints
      // it that way, so the first feature to route these rows through it would
      // have done exactly that. NaN here for the same reason as `r` and `stop`
      // below: a zero would read as a measurement.
      pnlUsd: Number.NaN,
      pnlAccount: f.pnl ?? 0,
      r: Number.NaN,
      mae: Number.NaN,
      mfe: Number.NaN,
      holdMs: (f.exitTime as number) - (f.entryTime as number),
      reason: '',
    }))
}

/**
 * How long after the bar it is priced at the book LEARNED it holds a position,
 * compactly. `null` when the book has not said — absent, not zero, and a "+0m"
 * in that case would be a measurement nobody made.
 *
 * Rendered rather than left to the reader because the delay is STRUCTURAL and
 * was invisible: a fill is priced at the open of the bar stamped `entry_time`,
 * and that bar is only posted once it CLOSES, so the book cannot know before
 * `entry_time + one bar`. It was one bar on all six positions the funded
 * account took on 2026-09-17, every time. A number that is always the same for
 * a structural reason is exactly the number nobody checks, so it goes on the
 * screen instead of in a comment.
 */
function learnLag(entryTime: number, learnedAt: number | null | undefined): string | null {
  if (learnedAt == null || !Number.isFinite(learnedAt)) return null
  const ms = learnedAt - entryTime
  if (!Number.isFinite(ms)) return null
  const minutes = Math.round(ms / 60_000)
  if (Math.abs(minutes) < 90) return `${minutes >= 0 ? '+' : MINUS}${Math.abs(minutes)}m`
  return `${minutes >= 0 ? '+' : MINUS}${(Math.abs(minutes) / 60).toFixed(1)}h`
}

/**
 * The price an open position is marked against: the live tick when there is
 * one, else the last closed bar.
 *
 * One function, called by the chart heading and by both fills tables, so that
 * one position can never be marked at two different prices on one screen.
 * `livePrice` in the page above prefers a STREAMED tick over the polled
 * `run.live`, so a caller reaching for `run.live` directly would quietly show
 * a staler number than the chart beside it.
 */
function markPrice(
  live: LiveBar | null | undefined,
  run: { last_bar_close?: number | null } | null | undefined,
): number | null {
  const at = live?.close ?? run?.last_bar_close ?? null
  return at != null && Number.isFinite(at) ? at : null
}

/**
 * The live result of the open position on screen, formatted WITH ITS UNIT.
 *
 * A string and not a number, deliberately: this is the only place that decides
 * which currency an open result is in, so no caller can render one without it.
 *
 * The account's is the broker's own `profit` — already in the account's units
 * and already live. The paper book's is marked here through `usd_per_point`,
 * which is what that field exists for; `unrealised_usd_at_last_close` can be a
 * full bar old, and a result that lags the candle beside it is worse than none.
 *
 * The two never blend. An account is shown its own trade at its own price and
 * the paper book its own — that difference IS the slippage, which is the thing
 * the account switch exists to show.
 */
function openResult(
  broker: PaperBroker | null,
  run: PaperRun | null | undefined,
  mark: number | null,
): { label: string; positive: boolean } | null {
  if (broker) {
    const held = broker.position
    if (!held || held.profit == null || !Number.isFinite(held.profit)) return null
    return { label: brokerMoney(held.profit, broker.currency), positive: held.profit >= 0 }
  }
  const held = run?.open
  if (!held || mark == null) return null
  const usd = (mark - held.entry_price) * held.usd_per_point * (held.side === 'LONG' ? 1 : -1)
  if (!Number.isFinite(usd)) return null
  return { label: paperMoney(usd), positive: usd >= 0 }
}

/**
 * Money in the BROKER's currency, which is not the paper book's.
 *
 * No conversion, and nothing in this file converts any more. `profit`, `pnl`,
 * `balance` and `equity` on a `PaperBroker` are already in the account's own
 * units; `paperMoney` is for the BOOK's dollars and these are not those.
 *
 * The sign comes from `toLocaleString`, so it is an ASCII hyphen where
 * `paperMoney` writes U+2212 - see the note there. The two glyphs are now how
 * a reader tells ACCOUNT money from PAPER money where the position card puts
 * them inches apart.
 */
function brokerMoney(v: number | null | undefined, currency: string | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '--'
  const sign = v > 0 ? '+' : ''
  return `${sign}${v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${currency ?? ''}`.trim()
}

export function Desk({ book, ticks, streaming, theme }: {
  book: Book
  /** The live stream, subscribed ONCE in `App` and handed down. Two
   *  subscriptions would be two answers a fraction of a second apart, on one
   *  screen, about one price. */
  ticks: Record<string, LiveBar>
  streaming: boolean
  /** The palette in force. Passed down only so the chart can be REMOUNTED on a
   *  change: `PriceChart` reads its colours from CSS custom properties once, at
   *  mount, so a theme switch would otherwise leave dark candles on a white
   *  page until the next navigation. */
  theme: 'light' | 'dark'
}) {
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

  // Flipped locally first so the switch answers the click, then confirmed by
  // the server's own reply rather than assumed: if the call fails the row goes
  // back to what it was, because a switch that shows "off" over a book that is
  // still trading is worse than one that feels slow.
  const togglePause = useCallback((id: string, paused: boolean) => {
    setRuns((prev) => prev?.map((r) => (r.id === id ? { ...r, paused } : r)) ?? prev)
    api
      .paperPause(id, paused)
      .then((res) => {
        setRuns((prev) => prev?.map((r) => (r.id === id ? { ...r, paused: res.paused } : r)) ?? prev)
        if (res.paused && res.holding) {
          toast.warning(`${id} stopped — its open position stays live`, {
            description: 'No new entries. The trade it is holding still runs to its stop or target.',
          })
        } else {
          toast.success(`${id} ${res.paused ? 'stopped' : 'started'}`)
        }
      })
      .catch((err: Error) => {
        setRuns((prev) => prev?.map((r) => (r.id === id ? { ...r, paused: !paused } : r)) ?? prev)
        toast.error(`Could not ${paused ? 'stop' : 'start'} ${id}`, { description: err.message })
      })
  }, [])

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
  // One poll for the whole page: the card NAMES a break level and the chart
  // DRAWS a line at it, so two fetches would have the desk arguing with itself
  // in the one place a reader compares them.
  const { htf, error: htfError } = useHtf(sorted.find((r) => r.id === activeId)?.market ?? '')

  // The price-bar levels, polled the same way and for the same reason: the
  // panel in the rail names a price and the chart draws a line at it, so the
  // two read ONE response. Both context reads are fetched here and handed
  // down rather than fetched where they are shown.
  const { levels: priceLevels, error: priceLevelsError } = usePriceLevels(
    sorted.find((r) => r.id === activeId)?.market ?? '',
  )

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
      style={{ height: `calc(100dvh - ${APP_BAR_H}px)` }}
    >
      <SummaryStrip runs={sorted} now={now} loading={runs === null} error={statusError} streaming={streaming} account={account} />

      {statusError && runs === null ? (
        <p className="text-destructive px-3 py-4 fd-body">
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
              <RunChart
                detail={live}
                live={livePrice}
                focus={focusFill}
                broker={activeBroker}
                theme={theme}
                htf={htf}
                levels={priceLevels}
                now={now}
              />
            </div>
            {/* The fills have seven columns and the rail has 460px, so they
                stay here where the width is. Capped at two fifths of the
                column: a long book must not push the chart off the screen the
                move above was made to give it. Below xl they are in the rail. */}
            <div className="hidden xl:flex xl:max-h-[40%] xl:shrink-0 xl:flex-col xl:overflow-hidden">
              <BottomTabs tab={bottomTab} onTab={setBottomTab} />
              <div className="min-h-0 flex-1 overflow-y-auto">
                {bottomTab === 'fills' ? (
                  <FillsSection detail={live} live={livePrice} openFill={pickedFill} onOpenFill={openFill} broker={activeBroker} />
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
              <RunsList
                runs={sorted}
                now={now}
                selected={activeId}
                onPick={pick}
                ticks={ticks}
                account={account}
                onToggle={togglePause}
              />
            </div>
            <div className="min-h-0 xl:flex-1 xl:overflow-y-auto">
              <Drilldown
                detail={live}
                summary={activeRun}
                broker={activeBroker}
                live={livePrice}
                error={detailError && detailError.id === activeId ? detailError.message : null}
                now={now}
                openFill={pickedFill}
                onOpenFill={openFill}
                htf={htf}
                htfError={htfError}
                levels={priceLevels}
                levelsError={priceLevelsError}
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
    <div className="text-muted-foreground flex h-8 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b px-3 fd-label">
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
              {paperMoney(stats.net)}
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
    <div className="text-muted-foreground min-h-0 flex-1 px-3 py-8 fd-body">
      <p className="text-foreground">No paper run is registered.</p>
      <p className="mt-1">Start the ten registered runs, then start the pollers that feed them closed bars:</p>
      <pre className="border-border bg-card text-foreground mt-3 w-fit rounded-md border px-3 py-2 font-mono fd-label leading-relaxed">
        <code>{'python py/live/start_runs.py\npowershell -File py\\live\\start_pollers.ps1'}</code>
      </pre>
      <p className="mt-2 fd-label">
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
  onToggle,
}: {
  runs: PaperRun[]
  now: number
  selected: string | null
  onPick: (id: string) => void
  /** Streamed forming bars by `market:tf`; newer than the row's own. */
  ticks: Record<string, LiveBar>
  /** Turn one book off or back on. */
  onToggle: (id: string, paused: boolean) => void
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
      <div className="text-muted-foreground bg-background sticky top-0 z-10 flex items-baseline gap-2 border-b px-3 py-1 fd-caption tracking-wide uppercase">
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
          <div
            key={run.id}
            className={cn(
              'hover:bg-accent/60 flex items-start gap-1 border-b pr-3 pl-2 transition-colors duration-100 last:border-0 motion-reduce:transition-none',
              isOn && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
              unmirrored && 'opacity-45',
              // A book that is switched off is dimmed as a whole. The badge
              // says which state it is in; this says it at a glance, down the
              // length of a list, without reading anything.
              run.paused && 'opacity-55',
            )}
          >
            <PowerSwitch run={run} onToggle={onToggle} />
            <button
              type="button"
              ref={(el) => {
                rowRefs.current[index] = el
              }}
              onClick={() => onPick(run.id)}
              onKeyDown={(e) => onKeyDown(e, index)}
              aria-pressed={isOn}
              className="focus-visible:ring-ring min-w-0 flex-1 py-[6px] text-left focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none"
            >
            <span className="flex items-center gap-2 text-xs">
              <StatusPill stale={stale} />
              {/* Said, not only implied by the dimming. `fed` beside it is
                  still true and still useful - the feed is fine; what is off
                  is the book - and the two facts are left as two. */}
              {run.paused && (
                <span className="border-border text-muted-foreground shrink-0 rounded-sm border px-1 fd-caption tracking-wide uppercase">
                  off
                </span>
              )}
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
                  {paperMoney(run.net_usd)}
                </span>
              )}
            </span>
            <span className="mt-0.5 flex items-end gap-2 fd-caption leading-tight">
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
          </div>
        )
      })}
    </div>
  )
}

/**
 * Turn one book off, or back on.
 *
 * Off means no NEW position. An open one is still managed to its stop and its
 * target, which is why the switch says so in its tooltip while a trade is
 * live: someone flicking it to stop trading has not stopped the trade they
 * already have, and finding that out later would be the worst way to learn it.
 *
 * Deliberately not a confirm dialog. Pausing is reversible, costs nothing and
 * takes effect on the next bar; a dialog in front of it would only teach
 * people to click through dialogs.
 */
function PowerSwitch({ run, onToggle }: { run: PaperRun; onToggle: (id: string, paused: boolean) => void }) {
  const off = run.paused
  const holding = !!run.open
  return (
    <button
      type="button"
      role="switch"
      aria-checked={!off}
      aria-label={`${off ? 'Start' : 'Stop'} ${run.id}`}
      title={
        off
          ? `${run.id} is off — it takes no new position. Click to start it.`
          : holding
            ? `${run.id} is running. Stopping it blocks new entries; the position it is holding stays open and is still managed to its stop and target.`
            : `${run.id} is running. Click to stop it taking new positions.`
      }
      onClick={() => onToggle(run.id, !off)}
      className={cn(
        'focus-visible:ring-ring mt-[9px] inline-flex h-[14px] w-[24px] shrink-0 items-center rounded-full border px-[2px] transition-colors focus-visible:ring-2 focus-visible:outline-none',
        off ? 'border-border bg-muted-foreground/15' : 'border-lc/50 bg-lc/25',
      )}
    >
      <span
        className={cn(
          'block size-[8px] rounded-full transition-transform',
          off ? 'bg-muted-foreground/70' : 'bg-lc translate-x-[10px]',
        )}
      />
    </button>
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
/**
 * The shortest name that still says which model drove the book.
 *
 * The chip used to print the decider's full id and let the row truncate it,
 * so `deepseek-flash` and `claude-opus-5` ended in an ellipsis on the narrow
 * rows — and an ellipsis in the one field that identifies WHO traded is that
 * field failing at its only job. Shortened here instead, deliberately, with
 * the full id kept in the title where it can still be read.
 *
 * Rules rather than a lookup table, because the deciders change: a new model
 * id has to shorten to SOMETHING sensible without anyone editing this. The
 * trailing word is preferred when there is one — `gpt-5.6-terra` is known
 * around here as terra — and a version number is never the answer.
 */
function shortDecider(name: string): string {
  // `codex/` is the route, not the model; it never survives into the label.
  const raw = name.toLowerCase().replace(/^codex\//, '')
  if (raw === 'coin') return 'coin'
  if (raw.startsWith('deepseek')) return 'deepseek'
  for (const family of ['opus', 'sonnet', 'haiku']) {
    if (raw.includes(family)) return family
  }
  const parts = raw.split(/[-_/]/).filter(Boolean)
  const tail = parts[parts.length - 1]
  // A trailing word names the model; a trailing number only versions it.
  if (parts.length > 1 && tail && tail.length <= 8 && !/^[\d.]+$/.test(tail)) return tail
  return parts[0] ?? raw
}

function DeciderTag({ run, silent = false }: { run: PaperRun; silent?: boolean }) {
  if (run.strategy !== 'external' && !run.decider) return null

  const names = Object.keys(run.decider?.decisions ?? {})
  const mixed = names.length > 1
  const last = run.decider?.last
  const coin = last === 'coin'
  const total = Object.values(run.decider?.decisions ?? {}).reduce((a, b) => a + b, 0)
  const house = coin || mixed ? null : houseOf(last)

  const text = last ? (coin ? 'coin' : shortDecider(last)) : 'idle'
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

  /*
   * ONE STYLE: an outline in the house hue, a coloured mark, and real text.
   *
   * It used to be a tinted fill carrying tinted TEXT — #F2C3AC on a 32% wash
   * of #D97757. That survives on a dark ground, where a lifted tint is
   * brighter than what is behind it, and dies on a light one, where the text
   * is a paler version of an already pale fill. The owner's report was that
   * it was unreadable and the measurement agreed.
   *
   * So colour and legibility are now carried by different things. The hue
   * goes on the border and the vendor mark, where it only has to be SEEN —
   * the non-text threshold, 3.0. The name is ordinary foreground text, which
   * is already measured against every ground the desk has. Nothing needs to
   * be tinted in order to look like it belongs to the family.
   *
   * The mark rather than a plain dot: same size, carries the hue just as
   * well, and says which house at a glance without the reader having to
   * decode a colour. The hue itself comes from a per-theme variable, so this
   * component never has to know which theme it is in.
   */
  const HOUSE = {
    claude: 'var(--house-claude)',
    deepseek: 'var(--house-deepseek)',
    openai: 'var(--house-openai)',
  } as const
  // A stopped model loses its house colour entirely. Dimming the brand tint
  // would still read as "this is the DeepSeek book, slightly faded"; dropping
  // it reads as "this book is not being driven", which is the true statement.
  const tone = silent ? null : house ? HOUSE[house] : null
  const skin = tone
    ? {
        borderColor: `color-mix(in oklab, ${tone} 55%, transparent)`,
        backgroundColor: `color-mix(in oklab, ${tone} 7%, transparent)`,
      }
    : undefined

  return (
    <span
      title={title}
      style={skin}
      className={cn(
        // `shrink-0` and `whitespace-nowrap`: a narrow row is allowed to
        // squeeze many things, and the name of who traded is not one of them.
        'mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 py-px align-middle fd-label tracking-wide whitespace-nowrap',
        skin && 'text-foreground',
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
      {/* The marks take `currentColor`, so the hue is set on them directly.
          The text beside them is deliberately NOT tinted, and letting the
          marks inherit it would have turned every one of them grey. */}
      {!silent && house === 'openai' && (
        <OpenAIMark className="size-[10px] shrink-0" style={{ color: tone ?? undefined }} />
      )}
      {!silent && house === 'claude' && (
        <ClaudeMark className="size-[10px] shrink-0" style={{ color: tone ?? undefined }} />
      )}
      {!silent && house === 'deepseek' && (
        <DeepSeekMark className="size-[10px] shrink-0" style={{ color: tone ?? undefined }} />
      )}
      {silent && <span aria-hidden>{'\u23f8'}</span>}
      {!silent && !house && !coin && <span aria-hidden>{mixed ? '\u26a0' : '\u25c6'}</span>}
      <span className="num">{text}</span>
      {/* Named, not implied. "stopped" beside the model is the one word
          that stops a reader taking the row's numbers as something still
          being added to. */}
      {silent && <span>stopped</span>}
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
            'focus-visible:ring-ring rounded-sm px-2 py-0.5 fd-caption tracking-wide uppercase transition-colors focus-visible:ring-2 focus-visible:outline-none',
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
  if (error) return <p className="text-destructive px-3 py-3 fd-body">Could not read the log: {error}</p>
  if (!data) return <div className="px-3 py-3"><Skeleton className="h-4 w-64" /></div>

  const empty = data.decisions.length === 0 && data.consultations.length === 0
  if (empty) {
    return (
      <p className="text-muted-foreground px-3 py-3 fd-body">
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
    <h3 className="text-muted-foreground mt-2 fd-caption font-medium tracking-wide uppercase first:mt-0">{children}</h3>
  )
}

/** LONG / SHORT / NONE, coloured the way the fills table colours a side. */
function SidePill({ side }: { side: string }) {
  const up = side === 'LONG'
  const down = side === 'SHORT'
  return (
    <span
      className={cn(
        'num shrink-0 rounded-sm border px-1 fd-caption',
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
      className="num text-muted-foreground/70 shrink-0 fd-caption"
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
        className="hover:bg-accent/60 focus-visible:ring-ring flex w-full items-start gap-2 px-2 py-1 text-left transition-colors duration-100 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none motion-reduce:transition-none"
      >
        <span
          className="num text-muted-foreground shrink-0 fd-label"
          title={`bar ${utcStamp(d.bar_time)} · answered ${utcStamp(d.at)}`}
        >
          {vnStamp(d.at)}
        </span>
        <SidePill side={d.side} />
        <span className="min-w-0 flex-1 fd-body leading-snug">{d.reason || <span className="text-muted-foreground">(no reason given)</span>}</span>
        {outcome && (
          <span
            className={cn('num shrink-0 rounded-sm px-1 fd-caption', outcome.pnlUsd >= 0 ? 'bg-lc/15 text-lc' : 'bg-lp/15 text-lp')}
            title={`closed ${utcStamp(outcome.exitTime)} at ${outcome.exitPrice} — ${outcome.exitReason}`}
          >
            {signedR(outcome.r)}
          </span>
        )}
        <TokenChip d={d} />
        <span className="num text-muted-foreground/70 shrink-0 fd-caption">
          {(d.latency_ms / 1000).toFixed(1)}s
        </span>
      </button>
      {open && (
        <div className="border-border/60 space-y-1 border-t px-2 py-1.5 fd-label">
          {outcome && (
            <p className="fd-label">
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
          <pre className="text-muted-foreground/90 overflow-x-auto rounded-sm bg-black/30 p-1.5 fd-caption whitespace-pre-wrap">
            {d.response || '(empty reply)'}
          </pre>
          <p className="text-muted-foreground/60 fd-caption">
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
        className="hover:bg-accent/60 focus-visible:ring-ring flex w-full items-start gap-2 px-2 py-1 text-left transition-colors duration-100 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none motion-reduce:transition-none"
      >
        <span className="num text-muted-foreground shrink-0 fd-label" title={utcStamp(c.at)}>{vnStamp(c.at)}</span>
        <span
          className={cn(
            'num shrink-0 rounded-sm border px-1 fd-caption',
            c.size_factor === 0
              ? 'border-lp/40 bg-lp/10 text-lp'
              : cut
                ? 'border-caution/40 bg-caution/10 text-caution'
                : 'border-lc/40 bg-lc/10 text-lc',
          )}
        >
          {c.size_factor === 0 ? 'veto' : cut ? `\u00d7${c.size_factor?.toFixed(2)}` : 'allow'}
        </span>
        <span className="min-w-0 flex-1 fd-body leading-snug">{c.reason}</span>
        <span className="num text-muted-foreground/70 shrink-0 fd-caption">{c.turns.length} agents</span>
      </button>
      {open && (
        <div className="border-border/60 space-y-1.5 border-t px-2 py-1.5">
          <p className="text-muted-foreground fd-label">
            {c.dry_run ? 'dry run — the verdict was not applied' : c.applied ? 'applied to the book' : 'not applied'} ·{' '}
            <span className="num">{c.intent_id}</span>
          </p>
          {c.turns.map((t) => (
            <div key={t.agent} className="border-border/40 border-l-2 pl-2">
              <p className="fd-label">
                <span className="num text-primary">{t.agent}</span>{' '}
                <span className="num text-muted-foreground/70">{t.model}</span>{' '}
                <span className="text-muted-foreground/60 num">
                  {(t.latency_ms / 1000).toFixed(1)}s
                  {t.size_factor != null && ` \u00b7 \u00d7${t.size_factor.toFixed(2)}`}
                </span>
              </p>
              <p className="text-muted-foreground fd-label leading-snug">{t.reason}</p>
            </div>
          ))}
          <p className="text-muted-foreground/60 fd-caption">
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
        'inline-flex w-fit items-center gap-1 rounded-sm border px-1.5 py-px fd-caption',
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
      <span className="num text-muted-foreground/80 fd-caption whitespace-nowrap" title={spread ? `spread ${spread}` : undefined}>
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
        'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border border-dashed px-1 py-px fd-caption',
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
          'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 py-px fd-caption',
          long ? 'border-lc/45 bg-lc/10 text-lc' : 'border-lp/45 bg-lp/10 text-lp',
        )}
        title={`account holds ${pos.side} ${pos.lots} lots from ${pos.entry_price}, ticket ${pos.ticket}. This is the TERMINAL's volume — the book's size times lot_scale${broker.lot_scale != null ? ` (x${broker.lot_scale})` : ''} — so it is not meant to equal the book's lots beside it.`}
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
        className="num border-lp/45 bg-lp/10 text-lp mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px fd-caption"
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
        className="num text-muted-foreground border-border mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px fd-caption"
        title={`The mirror is not copying this trade: ${broker.standing_out}`}
      >
        sitting out
      </span>
    )
  }
  if (broker.book_side) {
    return (
      <span
        className="num text-muted-foreground border-border mr-1 inline-flex shrink-0 items-center rounded-sm border px-1 py-px fd-caption"
        title={`the book is ${broker.book_side} ${broker.book_lots} lots (the BOOK's size, before lot_scale${broker.lot_scale != null ? ` x${broker.lot_scale}` : ''}); the account is flat`}
      >
        {broker.dry_run ? 'dry run' : 'not filled'} &middot; book wants {broker.book_side.toLowerCase()}{' '}
        {broker.book_lots}
      </span>
    )
  }
  return <span className="text-muted-foreground/60 mr-1 fd-caption"> &middot; flat</span>
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
        'num mr-1 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 py-px fd-caption',
        long ? 'border-lc/45 bg-lc/10 text-lc' : 'border-lp/45 bg-lp/10 text-lp',
      )}
      title={`open ${open.side} ${open.lots} lots from ${open.entry_price}`}
    >
      <span aria-hidden>{long ? '\u25b2' : '\u25bc'}</span>
      {open.side.toLowerCase()}
      <span className={usd >= 0 ? 'text-lc' : 'text-lp'}>{paperMoney(usd)}</span>
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

function PositionBar({ run, live, broker }: {
  run: PaperRun
  live: LiveBar | null | undefined
  /** The account mirroring this book, when one is selected. Its position is a
   *  DIFFERENT trade from the book's - own entry, `lot_scale` size - and the
   *  difference between them is the measurement the mirror exists for. */
  broker?: PaperBroker | null
}) {
  const open = run.open

  // ---- ACCOUNT MODE: his money leads, the book is the footnote ----
  //
  // Until 2026-09-18 this card led with the PAPER trade whenever a book was
  // selected, account or not, and put the account's position underneath as a
  // small line. The owner's answer to that was one sentence - "the problem is
  // I am on real money" - and it is the whole argument. He read a big "+11
  // USC" that was the book's, saw "+6.83 USC" on the chart that was his, and
  // asked why the desk was out of sync. Nothing was out of sync. The card was
  // answering a question he was not asking.
  //
  // So when an account is selected, the account's position is the card and the
  // book's is one line under it. Paper mode is untouched: there, the book IS
  // the subject.
  //
  // Nothing converts, in either direction. Account money is `brokerMoney` in
  // the account's currency with its ASCII minus; the book's line is
  // `paperMoney` in dollars with U+2212 and the word `book`.
  if (broker) {
    const held = broker.position
    const bookLine = (() => {
      if (open) {
        const bLong = open.side === 'LONG'
        const bPrice = live?.close ?? run.last_bar_close ?? open.entry_price
        const bUsd = markToLive(open, bPrice).usd
        const bR = open.risk > 0 ? ((bPrice - open.entry_price) * (bLong ? 1 : -1)) / open.risk : null
        return (
          <>
            <span className={cn('num', bLong ? 'text-lc' : 'text-lp')}>{open.side.toLowerCase()}</span>
            <span className="num">{num(open.lots, 2)} lots</span>
            <span className="num">from {quote(open.entry_price)}</span>
            <span className={cn('num', bUsd >= 0 ? 'text-lc' : 'text-lp')}>{paperMoney(bUsd)}</span>
            {bR != null && (
              <span className={cn('num', bR >= 0 ? 'text-lc' : 'text-lp')}>{signedR(bR)}</span>
            )}
            <span className="text-muted-foreground/70">the decision this trade mirrors</span>
          </>
        )
      }
      if (run.pending) {
        return <span className="text-muted-foreground">decided {run.pending.side.toLowerCase()} — fills at the next bar&rsquo;s open</span>
      }
      return <span className="text-muted-foreground">flat</span>
    })()

    // The state that costs money and had no words on this card: the book is in
    // a trade and the account is not. A refused join, a stop file, a mirror
    // that died. Silence here reads as agreement.
    const missedIt = !held && open != null

    const aLong = (held?.side ?? '') === 'LONG'
    const aPrice = held?.price_now ?? null
    const aRisk =
      held?.entry_price != null && held?.sl != null ? Math.abs(held.entry_price - held.sl) : null
    // The account's OWN R: its own entry against its own stop. Not the book's
    // risk borrowed - that would be the same mistake in a different unit.
    const aR =
      aRisk != null && aRisk > 0 && held?.entry_price != null && aPrice != null
        ? ((aPrice - held.entry_price) * (aLong ? 1 : -1)) / aRisk
        : null
    const aSpan = held?.sl != null && held?.tp != null ? held.tp - held.sl : null
    const at = (v: number | null | undefined) =>
      aSpan && aSpan !== 0 && v != null && held?.sl != null
        ? Math.min(100, Math.max(0, ((v - held.sl) / aSpan) * 100))
        : null

    return (
      <div
        className={cn(
          'mt-2 rounded-sm border px-3 py-2',
          held ? (aLong ? 'border-lc/35 bg-lc/[0.06]' : 'border-lp/35 bg-lp/[0.06]') : 'border-border',
        )}
      >
        <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
          <span className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">
            {broker.account}
          </span>
          {held ? (
            <>
              <span className={cn('num rounded-sm px-1 fd-label font-medium', aLong ? 'bg-lc/20 text-lc' : 'bg-lp/20 text-lp')}>
                {(held.side ?? '').toUpperCase()}
              </span>
              <span className="num fd-body">{quote(held.entry_price)}</span>
              <span className="text-muted-foreground num fd-caption">{num(held.lots ?? 0, 2)} lots</span>
              <span
                className={cn('num ml-auto fd-display font-semibold', (held.profit ?? 0) >= 0 ? 'text-lc' : 'text-lp')}
              >
                {brokerMoney(held.profit, broker.currency)}
              </span>
              {aR != null && (
                <span className={cn('num fd-label', aR >= 0 ? 'text-lc' : 'text-lp')}>{signedR(aR)}</span>
              )}
            </>
          ) : (
            <span className={cn('fd-label', missedIt ? 'text-lp font-medium' : 'text-muted-foreground')}>
              {missedIt
                ? 'flat — and the book is in a trade. This account is not mirroring it.'
                : 'flat'}
            </span>
          )}
        </div>

        {held && aSpan ? (
          <>
            {/* The ACCOUNT's own stop and target, not the book's. They are the
                same prices today - the executor sends the book's verbatim -
                and reading them off the broker is what keeps this true if that
                ever stops being so. */}
            <div className="relative mt-2 h-1.5 rounded-full bg-black/40">
              <span
                className="bg-muted-foreground/70 absolute top-1/2 h-3 w-px -translate-y-1/2"
                style={{ left: `${at(held.entry_price) ?? 0}%` }}
                aria-hidden
              />
              {at(aPrice) != null && (
                <span
                  className={cn('absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-black/50', (held.profit ?? 0) >= 0 ? 'bg-lc' : 'bg-lp')}
                  style={{ left: `${at(aPrice)}%` }}
                  aria-hidden
                />
              )}
            </div>
            <div className="text-muted-foreground mt-1 flex justify-between fd-caption">
              <span className="num text-lp">stop {quote(held.sl)}</span>
              <span className="num">now {quote(aPrice)}</span>
              <span className="num text-lc">target {quote(held.tp)}</span>
            </div>
          </>
        ) : held ? (
          <div className="text-muted-foreground mt-1.5 fd-caption">
            {held.sl == null && held.tp == null
              ? 'no stop or target on the account'
              : `stop ${quote(held.sl)} · target ${quote(held.tp)}`}
          </div>
        ) : null}

        {/* No worst/best here. The wire carries `mae`/`mfe` for the BOOK and
            not for the account, and borrowing the book's excursions onto the
            account's row would be the same error this card was just fixed
            for - a number belonging to one trade shown against another. */}
        {held?.opened_at != null && (
          <div className="text-muted-foreground/70 mt-1 fd-caption">
            filled {shortStamp(held.opened_at)}
            {broker.lot_scale != null ? ` · ×${broker.lot_scale} of the book's size` : ''}
          </div>
        )}

        <div className="mt-2 flex flex-wrap items-baseline gap-x-2 gap-y-1 border-t pt-1.5 fd-caption">
          <span className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">book</span>
          {bookLine}
        </div>
      </div>
    )
  }

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
        <div className={cn('mt-2 rounded-sm border border-dashed px-3 py-2', long ? 'border-lc/40' : 'border-lp/40')}>
          <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
            <span className={cn('num rounded-sm border border-dashed px-1 fd-label font-medium', long ? 'border-lc/50 text-lc' : 'border-lp/50 text-lp')}>
              {p.side}
            </span>
            {fill != null ? (
              <span className="fd-label">
                <span className="text-muted-foreground">fills at </span>
                <span className="num fd-body">{quote(fill)}</span>
                <span className="text-muted-foreground/70"> — this bar&rsquo;s open, already set</span>
              </span>
            ) : (
              <span className="text-muted-foreground fd-label">decided — fills at the next bar&rsquo;s open, which has not started</span>
            )}
            <span className="num text-muted-foreground/70 ml-auto fd-caption">
              on the {shortStamp(p.decided_on)} bar
            </span>
          </div>
          <div className="text-muted-foreground mt-1.5 flex flex-wrap gap-x-4 fd-caption">
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
                  ≈{num(lots, 2)} lots{m ? ` · margin ${paperMoney(m.used, false)}` : ''}
                </span>
              )
            })()}
            {fill != null && p.stop != null && p.target != null && (
              <span className="num">
                R:R {num(Math.abs(p.target - fill) / Math.max(1e-9, Math.abs(fill - p.stop)), 2)}
              </span>
            )}
          </div>
          {p.reason && <p className="text-muted-foreground/80 mt-1.5 fd-label leading-snug">{p.reason}</p>}
        </div>
      )
    }
    return (
      <div className="mt-1.5 flex items-center gap-3 fd-label">
        <span className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">position</span>
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
    <div className={cn('mt-2 rounded-sm border px-3 py-2', long ? 'border-lc/35 bg-lc/[0.06]' : 'border-lp/35 bg-lp/[0.06]')}>
      <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
        <span className={cn('num rounded-sm px-1 fd-label font-medium', long ? 'bg-lc/20 text-lc' : 'bg-lp/20 text-lp')}>
          {open.side}
        </span>
        <span className="num fd-body">{quote(open.entry_price)}</span>
        <span className="text-muted-foreground num fd-caption">{num(open.lots, open.lots >= 100 ? 0 : 2)} lots</span>
        {(() => {
          const m = marginOf(open.lots, price, run)
          if (!m) return null
          return (
            /* `m.used` is USD — `marginOf` divides a USD notional by leverage.
               The title said `${m.used} of <account currency>`, so the badge
               read "margin 22 USC" while its own tooltip read "margin used
               0.22 of USC". Both numbers are named now. */
            <span
              className="text-muted-foreground/70 num fd-caption"
              title={`margin used ${paperMoney(m.used, false)} (${m.used.toFixed(2)} USD); a margin call comes at a level of 30%`}
            >
              paper margin {paperMoney(m.used, false)} ({m.pct.toFixed(2)}% · level{' '}
              {m.level > 9999 ? '>9999' : m.level.toFixed(0)}%)
            </span>
          )
        })()}
        <span className={cn('num ml-auto fd-display font-semibold', usd >= 0 ? 'text-lc' : 'text-lp')}>
          {paperMoney(usd)}
        </span>
        {/* Named, because an account's own number may sit inches below it and
            the two are different trades. A bare figure here was read as the
            account's and as a contradiction. */}
        <span className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">paper</span>
        {r != null && (
          <span className={cn('num fd-label', r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(r)}</span>
        )}
      </div>

      {/* No account line here. This branch is PAPER MODE - no account is
          selected, so the book is the subject and there is no second trade to
          show. When an account IS selected the card returns above, led by the
          account's own position with the book as its footnote. */}

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
          <div className="text-muted-foreground mt-1 flex justify-between fd-caption">
            <span className="num text-lp">stop {quote(open.stop)}</span>
            <span className="num">{marked ? 'live' : 'at last close'} {quote(price)}</span>
            <span className="num text-lc">target {quote(open.target)}</span>
          </div>
        </>
      ) : (
        <div className="text-muted-foreground mt-1.5 fd-caption">
          {open.stop == null && open.target == null
            ? 'self-managed: the strategy owns the exit, so there is no stop or target to sit between'
            : `stop ${quote(open.stop)} · target ${quote(open.target)} · the desk closes it at the maximum hold if neither is hit`}
        </div>
      )}

      <div className="text-muted-foreground/70 mt-1 fd-caption">
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
    return <span className="text-muted-foreground/60 truncate fd-caption">{run.guards ? 'none fired' : 'guards off'}</span>
  }
  return (
    <span className="flex min-w-0 flex-wrap gap-1">
      {chips.map((chip) => (
        <span key={chip} className="border-border text-muted-foreground num rounded-sm border px-1 py-px fd-caption">
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
  live,
  htf,
  htfError,
  levels,
  levelsError,
  error,
  now,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail | null
  summary: PaperRun | null
  /** The selected account's record of this book, or null on the paper book. */
  broker: PaperBroker | null
  /** The forming bar, so an open row marks at the same price as the chart. */
  live: LiveBar | null
  error: string | null
  now: number
  openFill: string | null
  onOpenFill: (key: string | null) => void
  /** Higher-timeframe facts, polled once by the page. */
  htf: HtfResponse | null
  htfError: string | null
  /** The price-bar levels, the same response the chart is drawing from. */
  levels: PriceLevelsResponse | null
  levelsError: string | null
}) {
  if (error) {
    return (
      <div className="p-3">
        <Heading>Drill-down</Heading>
        <p className="text-destructive mt-1 fd-body">{error}</p>
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
          <span className="num fd-body font-medium">{run.id}</span>
          <StatusPill stale={stale} />
          <span className="text-muted-foreground num ml-auto fd-caption">
            {run.market}:{run.tf} · last bar {run.last_bar_time ? `${ago(run.last_bar_time, now)} ago` : 'none'}
          </span>
        </div>
        <LivePrice live={summary?.live ?? detail.live} lastClose={(summary ?? run).last_bar_close} now={now} />
        {run.label && <p className="text-muted-foreground mt-1 fd-label leading-snug">{run.label}</p>}
        {/* The two columns that left the runs table when it became a rail
            picker: both describe this one run rather than compare it to the
            others, so this is where they belonged all along. */}
        <PositionBar run={run} live={summary?.live ?? detail.live} broker={broker} />
        {/* Context, beside the book's own state rather than above it: the
            higher timeframe is something to weigh what this run is doing
            against, not an instruction about it. */}
        <HtfCard market={run.market} data={htf} error={htfError} />
        {/* The levels the chart is drawing, read out. The chart says WHERE
            they are and this says WHAT they are — age, state, and how far
            away in both units — which is the half a line on a canvas cannot
            carry. Under the higher timeframe because it is the finer read of
            the same idea: context to weigh the book against, never an
            instruction about it. */}
        <LevelsCard data={levels} error={levelsError} />
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 fd-label">
          <span className="text-muted-foreground fd-caption font-medium tracking-wide uppercase">guards</span>
          <GuardChips run={run} />
        </div>
        <ConfigLine run={run} />
      </section>

      {broker ? (
        <AccountEquity broker={broker} />
      ) : (
        <section className="px-3 py-2">
          <Heading>Equity</Heading>
          {/* The paper book's curve is USD — `lots x contract_size x price`
              and nothing else — so it says so. */}
          <EquityCurve
            points={detail.equity_curve}
            money={(v) => `$${Math.round(v).toLocaleString('en-US')}`}
          />
          <p className="text-muted-foreground num mt-1 fd-caption">
            {run.trades} closed{run.open ? ' · 1 open' : ''} · net {paperMoney(run.net_usd)} ·{' '}
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
          <FillsTable detail={detail} live={live} openFill={openFill} onOpenFill={onOpenFill} />
        )}
      </section>

      {/* Two logs, and the account picker chooses between them rather than
          relabelling one of them.

          The book's events are about the RULE - a gap in its feed, a guard
          firing, the run starting. The account's are about the EXECUTION - a
          position not adopted because the price had run, a size clipped to the
          broker's minimum, an order refused, AutoTrading off. None of the
          second list can happen on the paper side.

          Showing the first under an account's name, with a label saying so,
          was read as the two sides not being separate at all. That reading was
          right: a label is not a separation. */}
      {broker ? (
        <BrokerEventsSection run={detail.run.id} account={broker.account} />
      ) : (
        <section className="px-3 py-2">
          <Heading>
            Events <span className="text-muted-foreground/70 num">{detail.events.length}</span>
          </Heading>
          <EventsList events={detail.events} />
        </section>
      )}
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
      <p className="text-muted-foreground mt-2 fd-label">
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
          'num fd-display leading-none transition-colors duration-300 motion-reduce:transition-none',
          DIRECTION_CLASS[direction + 1],
        )}
      >
        <span aria-hidden>{DIRECTION_MARK[direction + 1]}</span> {quote(live.close)}
      </span>
      <span className="text-muted-foreground num fd-caption">
        {live.bid != null && live.ask != null
          ? `bid ${quote(live.bid)} / ask ${quote(live.ask)}${spread ? ` · spread ${spread}` : ''}`
          : 'no quote'}
        {' · '}
        {liveAge(live.at, now)}
      </span>
      <span className="text-muted-foreground/70 fd-caption">forming bar — not traded on</span>
    </div>
  )
}

function FillsSection({
  detail,
  live,
  openFill,
  onOpenFill,
  broker,
}: {
  detail: PaperRunDetail | null
  /** The forming bar, so an open row marks at the same price as the chart. */
  live: LiveBar | null
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
        {/* No count here any more. It counted closed fills only and sat above a
            list that also held the open one, so it read "6" over seven rows.
            Each group below carries its own, which is the number a reader can
            check against the rows they can see. */}
        Fills <span className="text-muted-foreground/70 num">{detail.run.id}</span>
        {broker && <span className="text-muted-foreground/60 normal-case"> · on {broker.account}</span>}
      </Heading>
      {broker ? (
        <BrokerFillsTable broker={broker} />
      ) : (
        <FillsTable detail={detail} live={live} openFill={openFill} onOpenFill={onOpenFill} />
      )}
    </section>
  )
}

function Heading({ children }: { children: React.ReactNode }) {
  return <h2 className="text-muted-foreground mb-1 fd-caption font-medium tracking-wide uppercase">{children}</h2>
}

/** Strategy, parameters, filters and whether the guards are enforced — one line. */
function ConfigLine({ run }: { run: PaperRun }) {
  const params = Object.entries(run.params)
    .map(([k, v]) => `${k}=${Number.isInteger(v) ? v : num(v, 2)}`)
    .join(' ')
  return (
    <p className="text-muted-foreground num mt-1.5 fd-caption leading-relaxed break-words">
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
/*
 * Stable empties, so switching off the traded timeframe does not hand the
 * chart a fresh `[]` and `{}` on every render and make it rebuild panes it
 * could have left alone.
 */
const EMPTY_INDICATORS: ActiveIndicator[] = []
const EMPTY_BARS: Bar[] = []
const EMPTY_SERIES: Record<string, IndicatorPoint[]> = {}
const EMPTY_LEVELS: ChartLevel[] = []

/**
 * The prior day's and the prior week's prices, appended to whatever the H4
 * structure produced.
 *
 * ABSENT IS ABSENT. Each of these five is `number | null` on the wire and
 * null is the ordinary state, not an error: the week's three are null until
 * the stored daily series covers a whole prior week, and they come back the
 * moment it does. A null drawn as 0 would be a line at the bottom of every
 * chart on the desk, in a colour that means "a level somebody watches".
 *
 * Separate from the H4 block, and not gated on it, because the two come from
 * different timeframes in the same response: a market with a daily export and
 * no four-hour one has these and nothing else, and folding them together
 * would have lost all five to an early return.
 */
function dailyLevels(out: ChartLevel[], htf: HtfResponse | null): ChartLevel[] {
  const d1 = htf?.d1
  if (!d1) return out
  const rows: [string, number | null, string][] = [
    ['prior day high', d1.prior_day_high, 'prior_day_high'],
    ['prior day low', d1.prior_day_low, 'prior_day_low'],
    ['prior week high', d1.prior_week_high, 'prior_week_high'],
    ['prior week low', d1.prior_week_low, 'prior_week_low'],
    ['prior week mid', d1.prior_week_mid, 'prior_week_mid'],
  ]
  for (const [label, price, kind] of rows) {
    if (price == null || !Number.isFinite(price)) continue
    out.push({ label, price, kind })
  }
  return out
}

/**
 * Where these candles came from, said on the chart.
 *
 * On the traded timeframe this is one clause, because there is nothing to
 * disclose: the bars came with the run. Off it, every fact that the traded
 * chart gets for free has to be stated — which file, how old the export is,
 * how many candles have closed that it does not have, and how far into the
 * forming candle the high and low are actually known.
 *
 * That last one is `complete_to_ms` and it is the reason this component
 * exists rather than a string. The server builds a forming 4h candle out of
 * closed 15m bars, so its extremes can be a quarter of an hour behind while
 * the close is current. A wick that is fifteen minutes old, drawn with no
 * remark, claims to be the high so far — a small lie of exactly the kind the
 * server-side forming bar was introduced to stop telling.
 */
function ChartSource({
  tf,
  ownTf,
  alt,
  bars,
  now,
}: {
  tf: Timeframe
  ownTf: boolean
  alt: ReturnType<typeof useChartBars>
  bars: Bar[]
  now: number
}) {
  if (ownTf) {
    return (
      <span className="text-muted-foreground/70">
        Candles are the run's own — the same series it decided on.
      </span>
    )
  }

  const data = alt.data
  const behind = barsBehindBy(bars, tf, now)
  const forming = data?.forming ?? null
  // Known only as far as the finer series reaches. One finer period of lag is
  // ordinary and not worth a remark; beyond that the wick is old enough to
  // mislead, and how old is the thing worth saying.
  const lag = forming && forming.complete_to_ms > 0 ? now - forming.complete_to_ms : 0
  const stale = forming != null && lag > TF_MS[tf] / 4

  return (
    <>
      <span className="text-muted-foreground/70">
        {TF_WORD[tf]} candles from the chart store
        {data?.source?.file ? ` (${data.source.file})` : ''}
        {/* `since` and not `liveAge`: an export is hours old on a 4h chart,
            and `liveAge` both prints raw seconds AND already ends in "ago",
            so this line used to read "exported 51404 s ago ago". */}
        {data?.source?.exported_at_ms != null
          ? `, exported ${since(data.source.exported_at_ms, now)} ago`
          : ''}
        {'. '}
        {/* "Indicators are hidden" was true when the book's were the only
            ones the chart could draw. It stopped being true the moment a
            viewer could add their own, and a caption that says nothing is
            drawn while lines are on screen is worse than no caption. */}
        The book's own indicators are hidden: a 15-minute average is not a {tf} average, and drawing one here
        under its own name would be wrong in a way nothing on screen could show. Lines you add yourself are
        computed for {tf} and drawn dashed.
      </span>
      {behind > 0 && (
        <span className="text-lp">
          {behind} {tf} {behind === 1 ? 'candle has' : 'candles have'} closed since this export — it is behind.
        </span>
      )}
      {forming && (
        <span className={stale ? 'text-lp' : 'text-muted-foreground/70'}>
          The forming candle is built from {forming.from_timeframe} bars: its high and low are known to{' '}
          {clock(forming.complete_to_ms)}, the close is live.
        </span>
      )}
      {!forming && !alt.loading && data != null && (
        <span className="text-muted-foreground/70">
          Closed bars only — nothing finer is stored behind this timeframe, so there is no honest partial
          candle to draw.
        </span>
      )}
    </>
  )
}

function RunChart({
  detail,
  live,
  focus,
  broker,
  theme,
  htf,
  levels,
  now,
}: {
  detail: PaperRunDetail | null
  live: LiveBar | null
  /** The desk's own clock, so the export's age ticks with everything else on
   *  the page instead of being read fresh during this component's render. */
  now: number
  /** The fill picked in the table below, or null. */
  focus: BacktestTrade | null
  /** The selected account's record of this book, when the desk is showing an
   *  account rather than the paper book. */
  broker: PaperBroker | null
  /** Only to key the chart, so a palette change remounts it. */
  theme: 'light' | 'dark'
  /** The same facts the card shows, so the line and the number agree. */
  htf: HtfResponse | null
  /** The price-bar levels, the same response the panel reads out. */
  levels: PriceLevelsResponse | null
}) {
  // `bars` arrives as `[ms, o, h, l, c]` and `PriceChart` takes milliseconds
  // and divides, so the tuple goes straight across. (`series` times are
  // already seconds, which is what the chart wants there — the two halves of
  // the payload are in different units and neither is converted here.)
  const runBars = useMemo<Bar[]>(
    () => (detail?.bars ?? []).map(([time, open, high, low, close]) => ({ time, open, high, low, close })),
    [detail],
  )

  /**
   * Which timeframe is on screen, and where its candles come from.
   *
   * `ownTf` is the one the book trades. On it the chart is the RECORD: the
   * bars, the indicators and the fills all came from the same series in the
   * same response, and nothing below has to be reconciled. Every other
   * timeframe is a VIEW, assembled from the chart store, and the things that
   * make the record trustworthy have to be re-established one at a time — the
   * indicators do not apply, the export has its own age, the forming candle is
   * built elsewhere. The two cases are kept apart here rather than blended,
   * because a chart that silently degrades is one nobody knows to distrust.
   */
  const [tf, setTf] = useState<Timeframe>(readTimeframe)
  const runTf = detail?.run.tf ?? null
  const market = detail?.run.market ?? null
  const ownTf = runTf != null && tf === runTf

  // Bumped when somebody picks the timeframe that is ALREADY selected, which
  // is how a failed one is retried: `setTf` to the same value is a no-op.
  const [retry, setRetry] = useState(0)
  const alt = useChartBars(market, tf, detail != null && !ownTf, retry)

  // Timeframes this market has refused, remembered for the selector. Earned
  // from an actual refusal rather than declared up front: a timeframe is only
  // known to be missing once the store has been asked for it.
  // A timeframe that starts answering is no longer missing — a store that was
  // empty at lunchtime and has an export by the evening should stop being
  // struck out — so this reads both ways and returns the set UNCHANGED when
  // nothing moved, which is what keeps a twenty-second poll that confirms what
  // is already known from re-rendering the chart.
  const [missing, setMissing] = useState<Set<string>>(() => new Set())
  useEffect(() => {
    if (!alt.error && !alt.data) return
    setMissing((previous) => {
      const known = previous.has(tf)
      if (alt.error && !known) return new Set(previous).add(tf)
      if (alt.data && known) {
        const next = new Set(previous)
        next.delete(tf)
        return next
      }
      return previous
    })
  }, [alt.data, alt.error, tf])

  // Memoised for its IDENTITY, not its cost. `?? []` mints a fresh array on
  // every render, and `bars` is a dependency of both snapping memos and a prop
  // on the chart — an unstable empty array would rebuild the whole series on
  // each tick while the store is still being read.
  const bars = useMemo(
    () => (ownTf ? runBars : (alt.data?.bars ?? EMPTY_BARS)),
    [ownTf, runBars, alt.data],
  )

  const pickTf = (next: Timeframe) => {
    if (next === tf) {
      // Same timeframe: this is a retry, not a change. Nothing to remember and
      // nothing to set - only the request to make again.
      setRetry((n) => n + 1)
      return
    }
    setTf(next)
    writeTimeframe(next)
  }

  // The forming candle, in the shape the chart takes. `PriceChart` appends it
  // past the last closed bar and ignores a frame older than one, so a stream
  // that has fallen behind draws nothing rather than a candle in the past.
  const forming = useMemo<Bar | null>(() => {
    if (ownTf) {
      return live
        ? { time: live.time, open: live.open, high: live.high, low: live.low, close: live.close }
        : null
    }

    // Off the traded timeframe there is no stream to build this from. The
    // client CANNOT do it honestly: `useTicks` keeps only the newest bar per
    // `market:tf` and no history, and it only carries a `market:tf` some run
    // actually trades — nothing trades 4h. A client-built 4h open would be the
    // price when the tab was loaded: wrong by up to four hours, different for
    // two people on the same chart, and shaped exactly like a real candle. So
    // it comes from the server, anchored to the closed file's own stamp.
    const f = alt.data?.forming
    if (!f) return null

    // The server aggregates the extremes from a finer stored series, so they
    // are only known as far as `complete_to_ms`, while the tick stream's close
    // is current. Take the newer close when the stream is inside this bucket,
    // and widen the extremes to include it — a price that has actually printed
    // is a real high or low, and leaving it outside would draw a close beyond
    // its own wick.
    const inBucket = live != null && live.time >= f.time
    const close = inBucket ? live.close : f.close
    return {
      time: f.time,
      open: f.open,
      high: Math.max(f.high, close),
      low: Math.min(f.low, close),
      close,
    }
  }, [ownTf, live, alt.data])

  const runIndicators = useMemo<ActiveIndicator[]>(() => {
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
        // Said explicitly although it is the default, because this is the
        // set the whole distinction is about: these are the lines the bot
        // READ, and the viewer's own are drawn beside them.
        source: 'book',
      })
    }
    return active
  }, [detail])

  /**
   * Indicators are drawn on the traded timeframe ONLY.
   *
   * A 21-period EMA of 15-minute closes is not a 21-period EMA of four-hour
   * closes; it is a different line that happens to share a name. Redrawing the
   * run's series against 4h candles would put a curve on the chart that is
   * labelled `ema_21`, looks plausible, and describes nothing on screen — and
   * because it would be visibly wrong to nobody, it would be believed. The
   * caption says they are hidden and why, which is the honest version of the
   * same information.
   */
  const indicators = ownTf ? runIndicators : EMPTY_INDICATORS

  const [showOpen, setShowOpen] = useState(readShowOpen)
  const [showHtf, setShowHtf] = useState(readShowHtf)

  /**
   * Every level the desk already computes, as lines.
   *
   * THE H4 SWINGS WERE THE ONLY ONES DRAWN UNTIL NOW. `/api/paper/htf` has
   * also been sending the prior day's high and low and the prior week's high,
   * low and midpoint on every poll since the HTF card shipped — five prices
   * the desk computed, read into the card as figures, and threw away here.
   * They are the levels a discretionary reader looks for first, and the chart
   * was silently the one screen that did not have them.
   *
   * Only the levels that exist. A null is ABSENT, never drawn at zero: the
   * prior week's numbers are null until the stored series is long enough, and
   * a line at 0.00 under gold at 4,381 would be a level nobody could even
   * read as a mistake. `break_level` is null on a RANGE — a range has no
   * single price whose break changes the label — and two swings can share a
   * bar when one outside bar was both a fractal high and a fractal low, so
   * nothing here assumes distinct stamps or a full set.
   *
   * `kind` is carried beside the label because that is what `familyOf` reads
   * to colour the line and to decide which switch turns it off.
   */
  const allLevels = useMemo<ChartLevel[]>(() => {
    const out: ChartLevel[] = []
    const st = htf?.h4?.structure
    if (!st) return dailyLevels(out, htf)
    const bl = st.break_level
    // THE BREAK LEVEL IS NOT AN INDEPENDENT PRICE. On UP it IS `last_low`; on
    // DOWN it IS `last_high` - by construction, because it points at whichever
    // swing the label hangs on. Drawing both would put two dashed lines at the
    // same price, in the one place this overlay is trying to be clearest.
    // So the swing is drawn ONCE, and named as the break when it is the break.
    const same = (a: number, b: number) => Math.abs(a - b) < 1e-9 * Math.max(1, Math.abs(a))
    const highIsBreak = bl != null && st.last_high != null && same(bl, st.last_high.price)
    const lowIsBreak = bl != null && st.last_low != null && same(bl, st.last_low.price)
    const side = (st.break_side ?? '').toLowerCase()
    if (st.last_high && !highIsBreak) out.push({ label: 'H4 high', price: st.last_high.price, kind: 'h4_high' })
    if (st.last_low && !lowIsBreak) out.push({ label: 'H4 low', price: st.last_low.price, kind: 'h4_low' })
    if (bl != null) {
      const what = highIsBreak ? 'H4 high · breaks' : lowIsBreak ? 'H4 low · breaks' : 'H4 breaks'
      // The kind follows the SWING it is, not the word in the label: a break
      // level is one of the two swings by construction, and `familyOf` must
      // put it in their family rather than in the neutral one.
      const kind = highIsBreak ? 'h4_high' : lowIsBreak ? 'h4_low' : 'h4_break'
      out.push({ label: `${what} ${side}`.trim(), price: bl, kind })
    }
    return dailyLevels(out, htf)
  }, [htf])

  /**
   * Which families of level are drawn, and the master switch over all of them.
   *
   * Two controls rather than one, and they do different jobs. `showHtf` is
   * the one that has always been here — it remembers, per viewer, that
   * somebody wanted this overlay off entirely — and the family switches are
   * new, for the reader who wants the week's extremes without the H4 swings.
   * The families are only offered while the master is on, because a switch
   * that cannot change what is on screen teaches a reader that none of them
   * can.
   */
  const [families, setFamilies] = useState<Set<LevelFamily>>(readLevelFamilies)

  /**
   * Whether the spent levels are drawn too.
   *
   * Its own switch rather than a sixth family, because SPENT is not a family
   * — a swept pool and a broken block come from two different rules — and
   * because state and provenance are two separate questions a reader asks.
   */
  const [showSpent, setShowSpent] = useState(readShowSpent)

  /** Every level `/api/paper/levels` sent, in this client's words. */
  const deskLevels = useMemo(() => harvestLevels(levels), [levels])

  /**
   * The window onto them: still live, near the close, family switched on.
   *
   * A VIEW, NOT A VERDICT — the rule is written where the filter is, in
   * `lib/levels.ts`, and the count of everything outside the window is on
   * screen beside the switches so the choice is auditable. Nothing here
   * ranks, scores or scores-by-another-name.
   */
  const selection = useMemo(
    () => selectLevels(deskLevels, { families, showSpent }),
    [deskLevels, families, showSpent],
  )

  /**
   * Every family that is in play, for the switches, and how many of each is
   * actually on the chart.
   *
   * TWO COUNTS AND NOT ONE. A switch exists while the response HAS levels of
   * that family — turning `gaps` off and having its switch disappear because
   * the window hid the last one is a control that vanishes under the pointer
   * — and the number beside it is what is drawn, which is usually smaller.
   */
  const levelCounts = useMemo(() => {
    const counts = new Map<LevelFamily, number>()
    const bump = (family: LevelFamily) => counts.set(family, (counts.get(family) ?? 0) + 1)
    for (const level of allLevels) bump(familyOf(level.kind ?? ''))
    for (const level of deskLevels) bump(level.family)
    return counts
  }, [allLevels, deskLevels])

  const htfLevels = useMemo<ChartLevel[]>(() => {
    if (!showHtf) return EMPTY_LEVELS
    const fromHtf = allLevels.filter((level) => families.has(familyOf(level.kind ?? '')))
    const fromRoute: ChartLevel[] = selection.drawn.map((level) => ({
      label: level.label,
      // The level's price, or a band's edge nearer the close. Never the
      // midpoint: see `DeskLevel.anchor`.
      price: level.anchor,
      kind: level.kind,
      bandLow: level.bandLow,
      bandHigh: level.bandHigh,
      spent: level.spent,
    }))
    // ONE PRICE IS ONE LINE. `/api/paper/htf` and `/api/paper/levels` both
    // report the prior day's high and the levels route reports it twice more
    // under two other names; unfolded, the chart drew four identical dashed
    // lines and four tags stacked 14px apart pretending to be four levels.
    // Nothing is dropped — the fold names all of them on one tag.
    return foldSamePrice([...fromHtf, ...fromRoute])
  }, [allLevels, families, selection, showHtf])

  /** What is drawn, per family, for the switch titles. */
  const drawnCounts = useMemo(() => {
    const counts = new Map<LevelFamily, number>()
    for (const level of htfLevels) {
      const family = familyOf(level.kind ?? '')
      counts.set(family, (counts.get(family) ?? 0) + 1)
    }
    return counts
  }, [htfLevels])

  /** "38 further levels, spent or further away, not drawn", or nothing to
   *  say. Never silent about a level the route sent and the chart did not
   *  draw — that is the debt the prompt block pays with the same sentence. */
  const hidden = showHtf ? hiddenSentence(selection) : null

  /**
   * The open position to draw, taken from whichever book is on screen.
   *
   * The paper book's and the account's are different trades at different
   * prices - that difference IS the slippage, and it is the thing the account
   * switch exists to show - so the chart draws the one whose record it is
   * showing and never a blend of the two.
   */
  const chartOpen = useMemo(() => {
    if (broker) {
      const held = broker.position
      if (!held || held.entry_price == null || !Number.isFinite(held.entry_price)) return null
      return { side: held.side ?? '', entry_price: held.entry_price, stop: held.sl, target: held.tp }
    }
    return detail?.run.open ?? null
  }, [broker, detail])

  /**
   * That position's live result, formatted WITH ITS UNIT before it leaves here.
   *
   * Two books, two currencies, and this is the only place that knows which is
   * on screen. The account's `profit` is the broker's own number in the
   * account's currency; the paper book's is marked here against the forming
   * bar, which is what `usd_per_point` exists for - the book's own
   * `unrealised_usd_at_last_close` can be a full bar old, and a P&L that lags
   * the candle it is drawn on is worse than none.
   *
   * Neither number reaches the chart bare. See
   * `docs/decisions/2026-09-17-unit-carrying.md`.
   */
  const openPnl = useMemo(
    () => openResult(broker, detail?.run, markPrice(live, detail?.run)),
    [broker, detail, live],
  )

  /**
   * The fills, moved onto candles that exist.
   *
   * A fill happens at an exact millisecond. On the traded timeframe that stamp
   * IS a bar on the series and the marker lands where it belongs, which is why
   * this has never mattered before. On any other timeframe there is no candle
   * at 13:17, and a marker whose time is not a time on the series is one the
   * chart discards — an entry that vanishes from the record with no error
   * anywhere.
   *
   * `snapToBar` searches the bars rather than computing a bucket, because this
   * broker's H4 candles start at 21:00 UTC and its daily candles with them —
   * the server runs UTC+3, moving to UTC+2 in winter. Arithmetic snapping is
   * three hours wrong, a different amount wrong in winter, and invents
   * Saturday buckets on a market that is shut. See `lib/timeframes.ts`.
   *
   * A fill older than the window drops rather than clamping to the first
   * candle: a marker parked on the left edge reads as "this happened here".
   * An entry and an exit inside the same four hours land on the same candle,
   * which is not a bug — they did.
   */
  const chartTrades = useMemo<ChartTrade[]>(() => {
    const source = broker ? accountTrades(broker) : (detail?.fills ?? [])
    if (ownTf) return source
    return source.flatMap((trade) => {
      const entryTime = snapToBar(trade.entryTime, bars)
      if (entryTime == null) return []
      const exitTime = snapToBar(trade.exitTime, bars)
      return [{ ...trade, entryTime, exitTime: exitTime ?? entryTime }]
    })
  }, [broker, detail, ownTf, bars])

  // The picked fill is matched inside the chart on its pair of stamps, so it
  // has to be moved by the same rule or the focus would stop matching the
  // trade it came from the moment the timeframe changed.
  const chartFocus = useMemo<ChartTrade | null>(() => {
    if (!focus) return null
    if (ownTf) return focus
    const entryTime = snapToBar(focus.entryTime, bars)
    if (entryTime == null) return null
    const exitTime = snapToBar(focus.exitTime, bars)
    return { ...focus, entryTime, exitTime: exitTime ?? entryTime }
  }, [focus, ownTf, bars])

  /**
   * THE VIEWER'S OWN LINES, and why they are allowed where the book's are not.
   *
   * The rule above stands: the run's `ema_21` is a fifteen-minute average and
   * is hidden the moment the candles underneath stop being fifteen-minute
   * ones. These are a different thing — asked for by the person looking, and
   * computed by the server for `tf`, the timeframe actually on screen. There
   * is no timeframe they could be mislabelled against, so they are drawn on
   * all of them, dashed, under the word `yours`.
   *
   * `paneBase` is how many panes the book's indicators already hold. Without
   * it the viewer's first pane would be the book's first pane, and an RSI
   * somebody added would share a box with the MACD the bot trades.
   */
  const paneBase = useMemo(() => indicators.filter((e) => e.pane > 0).length, [indicators])
  const viewer = useViewerIndicators(market, tf, theme, paneBase)

  /**
   * Drawn only when the server's answer is FOR the candles on screen.
   *
   * `/api/chart/indicators` echoes the timeframe it computed on. If that ever
   * stops matching what was asked for — a normalisation, an older route, a
   * reply that arrives after the selector moved — the lines are dropped and
   * the caption says so, rather than a 4h RSI being drawn over 15m candles
   * with nothing on screen able to show it.
   */
  const viewerOk = viewer.computedFor == null || viewer.computedFor === tf
  const viewerIndicators = viewerOk ? viewer.indicators : EMPTY_INDICATORS
  const chartIndicators = useMemo(
    () => (viewerIndicators.length === 0 ? indicators : [...indicators, ...viewerIndicators]),
    [indicators, viewerIndicators],
  )

  /**
   * One series record for both sets.
   *
   * Safe to merge only because the viewer's keys carry the `you:` prefix
   * (`IndicatorPicker`): with bare keys the book's `ema_21.ema` and a
   * viewer's `ema_21.ema` would be one entry, and whichever was written last
   * would be drawn twice under two different names.
   */
  const bookSeries = ownTf ? (detail?.series ?? EMPTY_SERIES) : EMPTY_SERIES
  const chartSeries = useMemo(
    () => ({ ...bookSeries, ...(viewerOk ? viewer.series : EMPTY_SERIES) }),
    [bookSeries, viewer.series, viewerOk],
  )

  if (!detail) {
    return (
      <div className="space-y-2 px-3 py-2">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="w-full" style={{ height: CHART_H - 26 }} />
      </div>
    )
  }

  if (bars.length === 0) {
    // Four different reasons the chart is empty, and only some are worth
    // getting up to look at. Before the selector there was one, so one
    // sentence was enough; collapsing them now would make "still loading" and
    // "this market has no daily series" the same screen.
    const why = ownTf
      ? 'No bars to draw — the poller has fed this run nothing yet.'
      : alt.loading
        ? `Reading the ${tf} series…`
        : alt.error
          ? `No ${tf} series stored for ${market ?? 'this market'} — ${alt.error}`
          : `No ${tf} bars came back for ${market ?? 'this market'}.`
    return (
      <section className="px-3 py-2">
        <Heading>
          Chart
          <TimeframeBar value={tf} onChange={pickTf} runTf={runTf} unavailable={missing} />
        </Heading>
        <p className="text-muted-foreground py-8 text-center fd-label">{why}</p>
        {!ownTf && !alt.loading && runTf != null && isTimeframe(runTf) && (
          <p className="text-muted-foreground/70 text-center fd-caption">
            The book trades {runTf}, and that chart is drawn from the run itself.
          </p>
        )}
      </section>
    )
  }

  return (
    <section className="flex h-full min-h-0 flex-col px-3 py-2">
      <Heading>
        Chart{' '}
        <span className="text-muted-foreground/70 num">
          {detail.run.market}:{tf}
        </span>{' '}
        <span className="text-muted-foreground/70 num">{bars.length} bars</span>
        <TimeframeBar value={tf} onChange={pickTf} runTf={runTf} unavailable={missing} />
        {/* Said once, near the selector, rather than left for the reader to
            infer from indicators that quietly stopped being drawn. */}
        {!ownTf && (
          <span className="text-muted-foreground/70 ml-2 normal-case">
            a view — this book trades {runTf}
          </span>
        )}
        {focus && (
          <span className="text-primary num ml-2 normal-case">
            showing the {focus.direction.toLowerCase()} closed {shortStamp(focus.exitTime)} — click the row again to
            release
          </span>
        )}
        {/* Shown whether or not a position is open, so the control does not
            appear and vanish under the pointer - and so that an empty chart
            can be read as "flat" rather than "switched off". */}
        <button
          type="button"
          onClick={() => {
            const next = !showOpen
            setShowOpen(next)
            writeShowOpen(next)
          }}
          aria-pressed={showOpen}
          className={cn(
            'hover:bg-accent focus-visible:ring-ring ml-2 rounded-sm border px-1.5 py-px fd-caption normal-case transition-colors focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
            showOpen ? 'border-primary/40 text-primary' : 'border-border text-muted-foreground',
          )}
          title={
            showOpen
              ? 'Hide the open position from the CHART. The fills list below still shows it.'
              : 'Draw the open position on the chart, with its stop, target and live result'
          }
        >
          on chart {showOpen ? 'on' : 'off'}
        </button>
        <button
          type="button"
          onClick={() => {
            const next = !showHtf
            setShowHtf(next)
            writeShowHtf(next)
          }}
          aria-pressed={showHtf}
          className={cn(
            'hover:bg-accent focus-visible:ring-ring ml-1 rounded-sm border px-1.5 py-px fd-caption normal-case transition-colors focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
            showHtf ? 'border-primary/40 text-primary' : 'border-border text-muted-foreground',
          )}
          title={
            showHtf
              ? 'Hide every level from the chart — the H4 structure and the prior day and week'
              : 'Draw the H4 swing highs, lows and break level, and the prior day and week extremes'
          }
        >
          {/* It said "H4 levels" while the H4 swings were the only ones
              drawn. The storage key is still `fd.desk.showHtf`: somebody who
              switched these off in that version should stay switched off,
              and renaming the key would silently turn them all back on. */}
          levels {showHtf ? 'on' : 'off'}
        </button>
        {showHtf && (
          <LevelToggles
            on={families}
            present={new Set(levelCounts.keys())}
            counts={levelCounts}
            drawn={drawnCounts}
            onChange={(next) => {
              setFamilies(next)
              writeLevelFamilies(next)
            }}
          />
        )}
        {showHtf && selection.hiddenSpent + selection.hiddenFar + selection.drawn.length > 0 && (
          <button
            type="button"
            onClick={() => {
              const next = !showSpent
              setShowSpent(next)
              writeShowSpent(next)
            }}
            aria-pressed={showSpent}
            className={cn(
              'hover:bg-accent focus-visible:ring-ring ml-1 rounded-sm border px-1.5 py-px fd-caption normal-case transition-colors focus-visible:ring-2 focus-visible:outline-none motion-reduce:transition-none',
              showSpent ? 'border-primary/40 text-primary' : 'border-border text-muted-foreground',
            )}
            title={
              showSpent
                ? 'Hide the spent levels — pools already swept and blocks already broken. They are drawn dotted and faded.'
                : 'Draw the spent levels too: pools already swept and blocks already broken. On the response of 2026-09-19 that was 197 of 252 levels.'
            }
          >
            spent {showSpent ? 'on' : 'off'}
          </button>
        )}
        {/* NEVER SILENT ABOUT WHAT IS NOT DRAWN. A chart that looks complete
            says the tape is tidier than it is; the count is the audit of the
            window, and the two switches beside it undo it. */}
        {hidden && (
          <span
            className="text-muted-foreground/70 num ml-2 normal-case"
            title={`Levels are drawn within ${LIVE_WINDOW_ATR} ATR(14) of the last close and only while they are live — a pool not yet swept, a block not yet broken, a gap not yet filled. It is a window onto the response, not a judgement about which levels matter: nothing here is ranked or scored.`}
          >
            {hidden}
          </span>
        )}
        <IndicatorPicker
          offered={viewer.offered}
          chosen={viewer.indicators}
          computedFor={viewer.computedFor}
          timeframe={tf}
          error={viewer.error}
          onAdd={viewer.add}
          onRemove={viewer.remove}
          onParam={viewer.setParam}
        />
        {showOpen && openPnl && (
          <span className={cn('num ml-2 normal-case', openPnl.positive ? 'text-lc' : 'text-lp')}>
            {broker ? 'account' : 'book'} P/L {openPnl.label}
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
          key={`${detail.run.id}:${theme}`}
          open={chartOpen}
          openPnl={openPnl}
          showOpen={showOpen}
          htfLevels={htfLevels}
          // The account's record has no pending entry - a pending is a book
          // decision, and the mirror only ever learns about it as an order.
          pending={broker ? null : detail.run.pending}
          pendingFill={!broker && detail.run.pending ? pendingFill(detail.run.pending, live, detail.run.tf) : null}
          bars={bars}
          indicators={chartIndicators}
          series={chartSeries}
          frame={null}
          showLevels={false}
          trades={chartTrades}
          showMarkers
          showZones={!broker}
          focus={chartFocus}
          liveBar={forming}
        />
      </div>
      <p className="text-muted-foreground mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 fd-caption">
        {/* THE WORD IS ON EVERY ENTRY, not on a heading over a group of them.
            Two sets of lines in the same five hues are told apart here the
            same way they are told apart on the chart: `book` is solid and is
            what the bot read, `yours` is dashed and is what somebody added
            to look at. A peer session spent today on a bug where two sources
            of figures were separated only by a caption elsewhere on the
            screen, and on screen they were identical. */}
        {indicators.length === 0 && (
          <span>
            {/* `runIndicators`, not `indicators`: off the traded timeframe
                the second is empty either way, and the caption used to say
                the strategy reads none while `ChartSource` said in the same
                paragraph that they were hidden. Both cannot be true. */}
            {runIndicators.length === 0
              ? 'This strategy reads no indicator — it trades the clock or the bar itself.'
              : `The book's own indicators are drawn on ${runTf} only, so none of them is on screen.`}
          </span>
        )}
        {chartIndicators.map((entry) => {
          const mine = entry.source === 'viewer'
          return (
            <span key={entry.key} className="num inline-flex items-center gap-1">
              <span
                className="inline-block h-0 w-3 border-t align-middle"
                style={{ borderColor: entry.color, borderTopStyle: mine ? 'dashed' : 'solid' }}
                aria-hidden
              />
              {/* The swatch is drawn in the line's own style so the chip can
                  be matched to the curve, and the word is there for anyone
                  who cannot separate a dash from a rule at three pixels. */}
              <span className={mine ? 'text-muted-foreground/60' : 'text-foreground/70'}>
                {mine ? 'yours' : 'book'}
              </span>
              {entry.name ?? entry.key}
              {entry.pane > 0 && <span className="text-muted-foreground/60">pane {entry.pane}</span>}
            </span>
          )
        })}
        {viewerIndicators.length > 0 && (
          <span className="text-muted-foreground/70">
            your lines are computed on {viewer.computedFor ?? tf}, the candles on screen — the bot read none of
            them.
          </span>
        )}
        {!viewerOk && (
          <span className="text-caution">
            your lines came back computed on {viewer.computedFor}, not {tf} — dropped rather than drawn over the
            wrong candles.
          </span>
        )}
        <span className="text-muted-foreground/70">
          {broker
            ? `marks are ${broker.account}'s own fills, at the prices it got — no stop bands, because the stop belongs to the book and not to the account.`
            : 'entries and exits are marked; the bands behind them are each fill’s stop and target.'}
        </span>
        <span className="text-muted-foreground/70">
          The bot decides on closed bars only; the last candle is still forming and is never traded on.
        </span>
        <ChartSource tf={tf} ownTf={ownTf} alt={alt} bars={bars} now={now} />
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
        <p className="text-muted-foreground px-1 py-6 text-center fd-label">
          No curve yet — this account has closed no trade on this book.
        </p>
      ) : (
        /* The ACCOUNT's own units, which is USC on the funded cent account.
           No conversion: `balance` and `pnl` arrive in the account's currency
           already. `paperMoney` is for the BOOK's dollars and would be the
           wrong formatter here, not merely the wrong scale. */
        <EquityCurve
          points={points}
          money={(v) => `${Math.round(v).toLocaleString('en-US')} ${broker.currency ?? ''}`.trim()}
        />
      )}
      <p className="text-muted-foreground num mt-1 fd-caption">
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
  // The position the account is holding RIGHT NOW, which is not a fill and is
  // the reason this table exists at all. It led the list from 2026-09-17: the
  // chart drew it and the table did not, so the panel a reader actually scans
  // for positions was the one place the live one could not be seen.
  const held = broker.position
  const result = openResult(broker, null, null)
  if (fills.length === 0 && !held) {
    return (
      <p className="text-muted-foreground px-1 py-4 fd-label">
        This account has filled nothing on this book yet.
        {broker.dry_run && ' It is in dry run, so it never will until that is turned off.'}
      </p>
    )
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full fd-label">
        <thead className="text-muted-foreground fd-caption tracking-wide uppercase">
          <tr className="border-b">
            {/* One header over two groups now, so it cannot say "exit": the
                closed rows show an exit stamp and the open row says `filled`
                and its own fill time, because it has no exit and printing one
                would invent the thing the reader came to check. Each cell
                labels its own clock. */}
            <th className="py-1 pr-2 text-left font-medium">time (+07)</th>
            <th className="py-1 pr-2 text-left font-medium">side</th>
            <th className="py-1 pr-2 text-right font-medium">lots</th>
            <th className="py-1 pr-2 text-right font-medium">entry &rarr; exit</th>
            <th className="py-1 pr-2 text-left font-medium">reason</th>
            <th className="py-1 text-right font-medium">P&amp;L</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td colSpan={6} className={GROUP_LABEL}>Open · {held ? 1 : 0}</td>
          </tr>
          {!held && (
            /* Said out loud rather than left blank. On a desk that has twice
               mistaken an absent record for an empty one, a missing section
               must not be the way "flat" is expressed. */
            <tr className="border-b">
              <td colSpan={6} className="text-muted-foreground py-1 fd-label">
                flat — this account holds nothing on this book right now
              </td>
            </tr>
          )}
          {held && (
            <tr className="border-primary/30 bg-primary/10 border-b shadow-[inset_2px_0_0_var(--primary)]">
              <td
                className="num text-muted-foreground py-1 pr-2 pl-1"
                title={
                  held.opened_at != null
                    ? `the ACCOUNT filled here, ${utcStamp(held.opened_at)} — still open. Not the bar the book priced its entry at, which is a bar earlier.`
                    : 'the executor could not read the broker clock, so the fill time is unknown'
                }
              >
                filled {vnStamp(held.opened_at)}
              </td>
              <td className={cn('num py-1 pr-2', (held.side ?? '') === 'LONG' ? 'text-lc' : 'text-lp')}>
                {(held.side ?? '').toLowerCase()}
              </td>
              <td className="num py-1 pr-2 text-right">{held.lots ?? '—'}</td>
              <td className="num py-1 pr-2 text-right">
                {quote(held.entry_price)} &rarr; {quote(held.price_now)}
              </td>
              <td className="py-1 pr-2">
                <span className="text-primary">open</span>
                <span className="text-muted-foreground">
                  {held.sl != null && ` · sl ${quote(held.sl)}`}
                  {held.tp != null && ` · tp ${quote(held.tp)}`}
                </span>
              </td>
              <td className={cn('num py-1 text-right', result?.positive ? 'text-lc' : 'text-lp')}>
                {result ? result.label : '—'}
              </td>
            </tr>
          )}
          <tr>
            <td colSpan={6} className={GROUP_LABEL}>Closed · {fills.length}</td>
          </tr>
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

/**
 * `points` are [ms, money] — and WHICH money depends on who is calling, which
 * is why the formatter is a prop and not a `$` in here.
 *
 * Until 2026-09-17 this hardcoded `$`. The paper book's curve is USD and that
 * was right; `AccountEquity` feeds it the ACCOUNT's balance and realised P&L,
 * which on the funded cent account are USC. So the axis of the real-money
 * curve read `$9,982` for an account holding 9,982 USC — ninety-nine dollars,
 * labelled as ten thousand. Same shape as `pnlUsd` carrying USC before 35b42c3
 * and as `notional_of` comparing dollars to cents: a component that formats
 * money it was not told the unit of. See
 * docs/decisions/2026-09-17-unit-carrying.md.
 */
function EquityCurve({ points, money }: { points: [number, number][]; money: (v: number) => string }) {
  if (!points || points.length === 0) {
    return (
      <p className="text-muted-foreground px-1 py-6 text-center fd-label">
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

  return (
    <svg
      viewBox={`0 0 ${boxW} ${boxH}`}
      className={cn('w-full', up ? 'text-lc' : 'text-lp')}
      role="img"
      aria-label={`Equity from ${money(start)} to ${money(last[1])} over ${sorted.length} point${sorted.length === 1 ? '' : 's'}`}
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
          {money(vMax)}
        </text>
      )}
      <text x={l - 6} y={y(start) + 3.5} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
        {money(start)}
      </text>
      {!flat && (
        <text x={l - 6} y={bottom} textAnchor="end" className="fill-muted-foreground" fontSize={10}>
          {money(vMin)}
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
  live,
  openFill,
  onOpenFill,
}: {
  detail: PaperRunDetail
  /** The forming bar, so the open row marks at the same price as the chart. */
  live: LiveBar | null
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

  // The position the book is holding right now. Not a fill, and the reason
  // this list was incomplete: a reader scanning fills for what is running
  // found only what had finished.
  const held = detail.run.open
  const mark = markPrice(live, detail.run)
  const result = openResult(null, detail.run, mark)
  // Signed the way the book measures: positive is in the trade's favour.
  const runningR =
    held && mark != null && held.risk > 0
      ? ((mark - held.entry_price) * (held.side === 'LONG' ? 1 : -1)) / held.risk
      : null

  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, rows.length - 1) : Math.max(index - 1, 0)
    rowRefs.current[next]?.focus()
  }

  if (rows.length === 0 && !held) {
    return (
      <p className="text-muted-foreground py-2 fd-label">
        None yet — the first live bar decides; the warm-up bars do not.
      </p>
    )
  }

  return (
    <>
      <div className="overflow-x-auto">
        <div className="min-w-[600px]" role="group" aria-label="Fills, grouped: open first, then closed">
          <div
            className={cn(
              'text-muted-foreground grid items-center gap-2 border-b pb-1 fd-caption tracking-wide uppercase',
              FILL_COLS,
            )}
          >
            {/* The closed group's column. The open group's first cell says
                `bar <t>` or `since <t>` and labels itself, because it is a
                different clock — see docs/decisions/2026-09-17-entry-lag.md. */}
            <span>time (+07)</span>
            <span>side</span>
            <span className="text-right">lots</span>
            <span className="text-right">entry → exit</span>
            <span>exit reason</span>
            <span className="text-right">P&amp;L</span>
            <span className="text-right">R</span>
          </div>
          <div className={GROUP_LABEL}>Open · {held ? 1 : 0}</div>
          {!held && (
            /* Said out loud rather than left blank — absence must not be how
               "flat" is expressed on this desk. */
            <div className="text-muted-foreground border-b py-1 fd-label">
              flat — this book holds no position right now
            </div>
          )}
          {held && (
            <div
              className={cn(
                'border-primary/30 bg-primary/10 grid items-center gap-2 border-b py-1 fd-label shadow-[inset_2px_0_0_var(--primary)]',
                FILL_COLS,
              )}
            >
              {/* Not a button: every other row opens a detail panel about a
                  trade that is finished, and this one has no result to open.
                  It is also why it is not keyboard-navigable with the rest —
                  arrow keys walk the closed fills, which is the list they
                  were built for. */}
              {/* Three different clocks can describe one entry and they are a
                  BAR apart by construction, not by jitter: the bar the fill is
                  priced at, the wall clock the book learned it at, and the
                  moment the account actually filled. This cell is the first.
                  It said `since`, which reads as the third and is the one word
                  that cannot be right for all of them. */}
              <span
                className="num text-muted-foreground whitespace-nowrap"
                title={
                  `the book's fill is priced at the OPEN of the bar stamped ${utcStamp(held.entry_time)}.` +
                  (held.learned_at != null
                    ? ` The book only learned it holds this at ${utcStamp(held.learned_at)}, when that bar closed.`
                    : ' The book has not said when it learned this — the field is absent, which is not the same as no delay.')
                }
              >
                bar {vnStamp(held.entry_time)}
                {learnLag(held.entry_time, held.learned_at) && (
                  <span className="text-muted-foreground/60"> {learnLag(held.entry_time, held.learned_at)}</span>
                )}
              </span>
              <span className={cn('num', held.side === 'LONG' ? 'text-lc' : 'text-lp')}>
                {held.side.toLowerCase()}
              </span>
              <span className="num text-right">{num(held.lots, held.lots >= 100 ? 0 : 2)}</span>
              <span className="num text-right">
                {quote(held.entry_price)} <span className="text-muted-foreground">→</span> {quote(mark)}
              </span>
              <span className="truncate">
                <span className="text-primary">open</span>
                <span className="text-muted-foreground">
                  {held.stop != null && ` · stop ${quote(held.stop)}`}
                  {held.target != null && ` · target ${quote(held.target)}`}
                </span>
              </span>
              <span className={cn('num text-right', result?.positive ? 'text-lc' : 'text-lp')}>
                {result ? result.label : '—'}
              </span>
              {/* The book's own unit, and it costs nothing to say: `risk` is
                  one R in price, so the running R is the move so far over it.
                  A dash here would read as "not measurable" when it is. */}
              <span className={cn('num text-right', (runningR ?? 0) >= 0 ? 'text-lc' : 'text-lp')}>
                {signedR(runningR)}
              </span>
            </div>
          )}
          <div className={GROUP_LABEL}>Closed · {rows.length}</div>
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
                  'hover:bg-accent/60 focus-visible:ring-ring grid w-full items-center gap-2 border-b py-1 text-left fd-label transition-colors last:border-0 focus-visible:ring-2 focus-visible:ring-inset focus-visible:outline-none motion-reduce:transition-none',
                  FILL_COLS,
                  isOpen && 'bg-primary/10 shadow-[inset_2px_0_0_var(--primary)]',
                )}
              >
                <span
                  className="num text-muted-foreground"
                  title={`exit ${utcStamp(fill.exitTime)} · entry priced at the bar stamped ${utcStamp(fill.entryTime)}`}
                >
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
                  {paperMoney(fill.pnlUsd)}
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
          <p className="text-muted-foreground/70 mt-1.5 fd-caption">
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
    <div className="border-border bg-card/50 mt-2 space-y-1.5 rounded-sm border px-3 py-2 fd-body leading-relaxed">
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
        <span className={cn('num', fill.pnlUsd >= 0 ? 'text-lc' : 'text-lp')}>{paperMoney(fill.pnlUsd)}</span> —{' '}
        <span className={cn('num', fill.r >= 0 ? 'text-lc' : 'text-lp')}>{signedR(fill.r)}</span>.
      </p>

      <p className="text-muted-foreground">
        <Label>On the way</Label> it went as far as{' '}
        <span className="num text-lc">{signedR(fill.mfe)}</span> in favour and{' '}
        <span className="num text-lp">{signedR(fill.mae)}</span> against before it closed.
      </p>

      {run.trades < 30 && (
        <p className="text-caution border-border border-t pt-1.5 fd-label">
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

/**
 * One account's record of one book: `data/live/<account>/<run>/executor.jsonl`.
 *
 * Fetched rather than carried on the run detail, because the detail does not
 * know which account is on screen and would otherwise have to send every
 * account's log to answer a question about one.
 */
function BrokerEventsSection({ run, account }: { run: string; account: string }) {
  const [events, setEvents] = useState<BrokerEvent[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    setEvents(null)
    setError(null)
    const load = () =>
      api
        .brokerEvents(run, account, 60)
        .then((r) => live && setEvents(r.events))
        .catch((e: Error) => live && setError(e.message))
    load()
    const t = setInterval(load, 15_000)
    return () => {
      live = false
      clearInterval(t)
    }
  }, [run, account])

  return (
    <section className="px-3 py-2">
      <Heading>
        Events <span className="text-muted-foreground/70 num">{events?.length ?? 0}</span>
        <span className="text-muted-foreground/60 num normal-case"> · on {account}</span>
      </Heading>
      {error ? (
        <p className="text-destructive py-2 fd-label">{error}</p>
      ) : !events ? (
        <Skeleton className="h-16 w-full" />
      ) : events.length === 0 ? (
        <p className="text-muted-foreground py-2 fd-label">
          This account has no record of {run} — nothing has mirrored it here.
        </p>
      ) : (
        <ul className="space-y-0.5">
          {[...events].reverse().map((event, index) => (
            <li
              key={`${event.kind}-${event.at}-${index}`}
              className="grid grid-cols-[96px_110px_1fr] items-start gap-2 fd-label"
            >
              <span className="num text-muted-foreground">{shortStamp(event.at)}</span>
              <span className={cn('num truncate', BROKER_EVENT_TONE[event.kind] ?? 'text-muted-foreground')}>
                {event.kind}
              </span>
              <span className="text-muted-foreground leading-snug break-words">{brokerEventLine(event)}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** Only the kinds worth colouring. Everything else stays quiet. */
const BROKER_EVENT_TONE: Record<string, string> = {
  'real-money': 'text-lp',
  refused: 'text-lp',
  blocked: 'text-lp',
  'autotrading-off': 'text-caution',
  'no-account': 'text-caution',
  'not-adopted': 'text-caution',
  clipped: 'text-caution',
  opened: 'text-lc',
  closed: 'text-lc',
}

/**
 * The line beside the kind.
 *
 * The executor writes whatever a kind needs, so the few kinds worth a sentence
 * get one and the rest fall back to their own fields. A fallback that prints
 * the JSON is better than one that prints nothing: a kind added to the
 * executor tomorrow still says something today.
 */
function brokerEventLine(event: BrokerEvent): string {
  const n = (k: string) => (typeof event[k] === 'number' ? (event[k] as number) : undefined)
  const s = (k: string) => (typeof event[k] === 'string' ? (event[k] as string) : undefined)
  switch (event.kind) {
    case 'not-adopted':
      return `${s('side') ?? ''} ${n('lots') ?? ''} — price ${n('drift_r')}R from the book's entry ${n('book_entry')}, limit ${n('limit')}R`.trim()
    case 'clipped':
      return `wanted ${n('wanted')}, sent ${n('sent')} — the broker's own volume grid`
    case 'refused':
      return s('reason') ?? s('error') ?? ''
    case 'real-money':
      return `account ${n('login')} on ${s('server') ?? '?'} — ${n('balance')} ${s('currency') ?? ''}, lot scale ${n('lot_scale')}`
    case 'started':
      return `${s('symbol') ?? ''} magic ${n('magic')} · lot scale ${n('lot_scale')}${event.dry_run ? ' · dry run' : ''}`
    case 'autotrading-off':
      return s('note') ?? 'AutoTrading is off in the terminal'
    default: {
      const rest = Object.entries(event).filter(([k]) => k !== 'at' && k !== 'kind')
      return rest.map(([k, v]) => `${k}=${typeof v === 'object' ? JSON.stringify(v) : String(v)}`).join(' ')
    }
  }
}

function EventsList({ events }: { events: PaperEvent[] }) {
  if (events.length === 0) {
    return <p className="text-muted-foreground py-2 fd-label">Nothing but fills — no gap, no refusal.</p>
  }
  return (
    <ul className="space-y-0.5">
      {[...events].reverse().map((event, index) => (
        <li key={`${event.kind}-${event.time}-${index}`} className="grid grid-cols-[96px_78px_1fr] items-start gap-2 fd-label">
          <span className="num text-muted-foreground">{shortStamp(event.time)}</span>
          <span className={cn('num truncate', EVENT_TONE[event.kind] ?? 'text-muted-foreground')}>{event.kind}</span>
          <span className="text-muted-foreground leading-snug break-words">{eventLine(event)}</span>
        </li>
      ))}
    </ul>
  )
}
