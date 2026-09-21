import { useEffect, useMemo, useState } from 'react'

import { Skeleton } from '@/components/ui/skeleton'
import { api, type BrokerAccount, type PaperRun } from '@/lib/api'
import { cn } from '@/lib/utils'

/**
 * Analytics — what each strategy has actually done, on the book on screen.
 *
 * The Desk answers "is this run alive and what did it just do". This answers
 * the other question: over everything that has closed, which strategy is
 * carrying the account and which is bleeding it. So it is terminal mode
 * (`ui/DESIGN.md`) — flat, dense, tables that fill the width — and not the
 * card grid an account-summary screen usually wears. A page whose job is to
 * let someone compare eleven numbers across ten strategies cannot spend its
 * width on padding.
 *
 * **It follows the sidebar's book switcher**, the same remembered choice the
 * Desk follows. On PAPER every figure comes from the books' own fills — the
 * rule executed perfectly at the bar's price, in dollars, with an R because
 * the book knows its stop. On an ACCOUNT every figure comes from what that
 * account's broker actually closed, in the account's own currency, and only
 * for the books mirrored into it. The two were one table once, and a page
 * that added a paper dollar to a real cent was the reason they are not.
 *
 * Every figure here is computed from closed trades in the browser. Nothing is
 * asked of the server that the Desk does not already ask: `/api/paper/status`
 * for the runs (which carries every account's fills), then one
 * `/api/paper/run/{id}` per run for its paper fills. That costs ten small
 * requests a minute and keeps the analytics honest — there is no second code
 * path that could disagree with the book.
 *
 * **It will look empty for a while, and that is the truth rather than a bug.**
 * The books close a handful of trades a day. A table of ten strategies over
 * thirteen trades is what thirteen trades look like, and every panel here
 * says how thin its own sample is rather than drawing a confident shape over
 * nothing.
 */

/** The books change on a bar, not on a tick: a minute is often enough. */
const REFRESH_MS = 60_000

/** Below this, a panel says the number is too thin to read instead of drawing it. */
const THIN_SAMPLE = 20

const MINUS = '−'

/** The sidebar's choice: `paper`, or an account's login. Same shape as `App`'s `Book`. */
type Book = 'paper' | number

/**
 * How money is written on this page — decided ONCE by the book on screen and
 * handed to every panel, so a paper dollar and a real cent can never share a
 * column. The unit is carried in the string, per
 * `docs/decisions/2026-09-17-unit-carrying.md`.
 */
interface Money {
  /** Signed, with the unit: `+$12.30` on paper, `+12.30 USC` on an account. */
  fmt: (v: number) => string
  /** The unit's own name, for headings. */
  unit: string
}

const PAPER_MONEY: Money = {
  fmt: (v) => {
    const sign = v < 0 ? MINUS : v > 0 ? '+' : ''
    return `${sign}$${Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 })}`
  },
  unit: '$',
}

const accountMoney = (currency: string | null): Money => {
  const unit = currency ?? '?'
  return {
    fmt: (v) => {
      const sign = v < 0 ? MINUS : v > 0 ? '+' : ''
      return `${sign}${Math.abs(v).toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 })} ${unit}`
    },
    unit,
  }
}

const pct = (v: number): string => `${(v * 100).toFixed(1)}%`

const signedR = (v: number): string => `${v < 0 ? MINUS : v > 0 ? '+' : ''}${Math.abs(v).toFixed(2)}R`

const hold = (ms: number): string => {
  if (!Number.isFinite(ms) || ms <= 0) return '—'
  const m = Math.round(ms / 60_000)
  if (m < 60) return `${m}m`
  const h = m / 60
  return h < 48 ? `${h.toFixed(1)}h` : `${(h / 24).toFixed(1)}d`
}

/** A signed duration difference, for "the account held 12m longer". */
const signedHold = (ms: number): string => {
  if (!Number.isFinite(ms) || Math.abs(ms) < 30_000) return '±0m'
  return `${ms < 0 ? MINUS : '+'}${hold(Math.abs(ms))}`
}

/** Colour a number by its sign, using the tape's own up/down tokens. */
const bySign = (v: number): string => (v > 0 ? 'text-lc' : v < 0 ? 'text-lp' : 'text-muted-foreground')

/** One bar of the book's timeframe, in ms — the window a mirrored fill is matched inside. */
const tfMs = (tf: string): number => {
  const m = /^(\d+)([mhd])$/i.exec(tf.trim())
  if (!m) return 15 * 60_000
  const n = Number(m[1])
  const unit = m[2].toLowerCase()
  return n * (unit === 'm' ? 60_000 : unit === 'h' ? 3_600_000 : 86_400_000)
}

/**
 * The hour of a stamp on a New York wall clock.
 *
 * Sessions in this repository are New York hours — `Workbench` spells them
 * that way and so do the research batches — so the bucket has to be read on
 * that clock and not on the browser's. `Intl` carries the daylight-saving
 * rule; a fixed offset would put a third of the year in the wrong session,
 * which is the fault the research loop logged twice (`2026-09-13-instrument-faults.md`).
 */
const NY_HOUR = new Intl.DateTimeFormat('en-US', {
  timeZone: 'America/New_York',
  hour: 'numeric',
  hour12: false,
})

const nyHour = (ms: number): number => {
  const parsed = Number(NY_HOUR.format(new Date(ms)))
  // `Intl` renders midnight as 24 in some engines.
  return parsed === 24 ? 0 : parsed
}

/**
 * The three sessions exactly as `Workbench` and the research batches define
 * them, in New York hours. **They overlap** — London and New York share
 * 08:00–11:00 — so a trade can belong to two, and the session table's rows
 * deliberately do not sum to the total. Inventing non-overlapping boundaries
 * here would make this page disagree with the filter the strategies are
 * actually run under, which is worse than a footnote.
 */
const SESSIONS: { label: string; from: number; to: number; title: string }[] = [
  { label: 'Asia', from: 18, to: 2, title: '18:00–02:00 New York' },
  { label: 'London', from: 3, to: 11, title: '03:00–11:00 New York' },
  { label: 'NY', from: 8, to: 16, title: '08:00–16:00 New York' },
]

const inSession = (h: number, from: number, to: number): boolean =>
  from < to ? h >= from && h < to : h >= from || h < to

/**
 * A closed trade, from either source, tagged with the book it came from.
 *
 * `money` is in whichever unit the page is on — the paper book's dollars or
 * the account's currency — and a list never mixes the two, because the page
 * builds one list per book on screen. `r`, `mae` and `mfe` are the paper
 * book's and `null` on an account: a broker has no stop distance, so a zero
 * there would read as a measurement rather than an absence.
 */
interface Closed {
  runId: string
  strategy: string
  market: string
  tf: string
  direction: 'LONG' | 'SHORT' | null
  entryTime: number
  entryPrice: number | null
  exitTime: number
  holdMs: number
  money: number
  /**
   * The owner's introducing-broker rebate on this trade's own spread, in the
   * same unit as `money`, BESIDE it and never inside it.
   *
   * `null` means the trade was never priced for one - no arrangement
   * recorded, or no spread the credit could be taken from. It is not a
   * rebate of zero, and the header counts the two separately.
   */
  rebate: number | null
  /**
   * `EXACT` when the credit came from the spread the trade actually paid,
   * `ESTIMATED` when it came from a configured spread standing in for one
   * nobody recorded, `null` when the trade was not priced.
   *
   * A paper trade is EXACT only if it RECORDED what it was charged. That
   * used to be every one of them — "the engine charged the configured
   * spread, so the credit is arithmetic on a known cost" — and the sentence
   * survived about a day. It assumed the configured spread today is the one
   * the trade paid, which is exactly the assumption that let a contract-size
   * correction restate a stored P&L by three orders of magnitude
   * (docs/decisions/2026-09-21-restated-pnl-contract-size.md). Trades closed
   * before that fix carry no basis and are ESTIMATED.
   *
   * On an ACCOUNT most are estimates too, because the spread at a real fill
   * was not recorded until 2026-09-21 and a stop the broker takes still
   * leaves none.
   */
  rebateBasis: 'EXACT' | 'ESTIMATED' | null
  r: number | null
  mae: number | null
  mfe: number | null
}

interface Stats {
  trades: number
  wins: number
  winRate: number
  net: number
  grossWin: number
  grossLoss: number
  profitFactor: number | null
  /** True when every trade carries an R — paper, in practice. */
  hasR: boolean
  meanR: number
  medianR: number
  bestR: number
  worstR: number
  /** The same four, in money, so an account without R still has a per-trade shape. */
  meanMoney: number
  medianMoney: number
  bestMoney: number
  worstMoney: number
  maxDrawdown: number
  meanHoldMs: number
  meanMae: number
  meanMfe: number
  longestWin: number
  longestLoss: number
  /**
   * The IB credit over these trades, in the list's money, and how it was
   * arrived at. `net` above is untouched by it: the owner asked for a credit
   * BESIDE the book and not inside it, so a reader can see how much of a
   * result is the strategy and how much is the commercial arrangement.
   *
   * `rebateExact + rebateEstimated + rebateUnpriced` is `trades`; a total
   * that silently dropped what it could not price would read as complete.
   */
  rebate: number
  rebateExact: number
  rebateEstimated: number
  rebateUnpriced: number
  /** Cumulative P&L after each trade, oldest first — the curve, from zero. */
  curve: { t: number; v: number }[]
}

const EMPTY: Stats = {
  trades: 0,
  wins: 0,
  winRate: 0,
  net: 0,
  grossWin: 0,
  grossLoss: 0,
  profitFactor: null,
  hasR: false,
  meanR: 0,
  medianR: 0,
  bestR: 0,
  worstR: 0,
  meanMoney: 0,
  medianMoney: 0,
  bestMoney: 0,
  worstMoney: 0,
  maxDrawdown: 0,
  meanHoldMs: 0,
  meanMae: 0,
  meanMfe: 0,
  longestWin: 0,
  longestLoss: 0,
  rebate: 0,
  rebateExact: 0,
  rebateEstimated: 0,
  rebateUnpriced: 0,
  curve: [],
}

const median = (xs: number[]): number => {
  if (xs.length === 0) return 0
  const ordered = [...xs].sort((a, b) => a - b)
  const mid = ordered.length >> 1
  return ordered.length % 2 ? ordered[mid] : (ordered[mid - 1] + ordered[mid]) / 2
}

/**
 * Every statistic this page shows, from one list of closed trades.
 *
 * Trades are sorted by **exit** time, because that is when the money moved and
 * therefore the order the equity curve and the streaks happen in. A book that
 * holds two positions at once can exit them out of entry order, and a curve
 * drawn on entry time would show a drawdown the account never had.
 *
 * `maxDrawdown` is on that cumulative curve from its own running peak, in
 * the list's money. It is a property of this group of trades alone, not of
 * the run's equity, so summing it across strategies means nothing and no row
 * does.
 */
function summarise(trades: Closed[]): Stats {
  if (trades.length === 0) return EMPTY
  const sorted = [...trades].sort((a, b) => a.exitTime - b.exitTime)

  let net = 0
  let grossWin = 0
  let grossLoss = 0
  let wins = 0
  let peak = 0
  let maxDrawdown = 0
  let run = 0
  let longestWin = 0
  let longestLoss = 0
  let holdSum = 0
  let maeSum = 0
  let mfeSum = 0
  let rebate = 0
  let rebateExact = 0
  let rebateEstimated = 0
  let rebateUnpriced = 0
  const rs: number[] = []
  const monies: number[] = []
  const curve: { t: number; v: number }[] = []
  const hasR = sorted.every((t) => t.r !== null)

  for (const t of sorted) {
    net += t.money
    if (t.money > 0) {
      grossWin += t.money
      wins += 1
      run = run > 0 ? run + 1 : 1
      longestWin = Math.max(longestWin, run)
    } else if (t.money < 0) {
      grossLoss += -t.money
      run = run < 0 ? run - 1 : -1
      longestLoss = Math.max(longestLoss, -run)
    }
    // A trade that closed at exactly zero breaks neither streak and starts
    // neither: it is not a win and not a loss, and pretending otherwise would
    // let a scratch inflate a run of seven into a run of fifteen.
    peak = Math.max(peak, net)
    maxDrawdown = Math.max(maxDrawdown, peak - net)
    holdSum += t.holdMs
    maeSum += t.mae ?? 0
    mfeSum += t.mfe ?? 0
    if (hasR) rs.push(t.r as number)
    // The credit is summed apart from the money and never added into `net`.
    // A trade with no credit is COUNTED rather than treated as a zero: the
    // three counts are what tell a reader whether the total beside them is
    // measured, estimated, or missing most of the book.
    if (t.rebate === null) rebateUnpriced += 1
    else {
      rebate += t.rebate
      if (t.rebateBasis === 'EXACT') rebateExact += 1
      else rebateEstimated += 1
    }
    monies.push(t.money)
    curve.push({ t: t.exitTime, v: net })
  }

  return {
    trades: sorted.length,
    wins,
    winRate: wins / sorted.length,
    net,
    grossWin,
    grossLoss,
    // A book that has lost nothing has no profit factor: gross win over zero
    // is not "infinitely good", it is a sample too small to have a ratio.
    profitFactor: grossLoss > 0 ? grossWin / grossLoss : null,
    hasR,
    meanR: hasR ? rs.reduce((a, b) => a + b, 0) / rs.length : 0,
    medianR: median(rs),
    bestR: hasR ? Math.max(...rs) : 0,
    worstR: hasR ? Math.min(...rs) : 0,
    meanMoney: net / sorted.length,
    medianMoney: median(monies),
    bestMoney: Math.max(...monies),
    worstMoney: Math.min(...monies),
    maxDrawdown,
    meanHoldMs: holdSum / sorted.length,
    meanMae: hasR ? maeSum / sorted.length : 0,
    meanMfe: hasR ? mfeSum / sorted.length : 0,
    longestWin,
    longestLoss,
    rebate,
    rebateExact,
    rebateEstimated,
    rebateUnpriced,
    curve,
  }
}

/** The paper books' fills, keyed by run — kept whichever book is on screen. */
type PaperFills = Map<string, Closed[]>

export function Analytics({ book, accounts }: { book: Book; accounts: BrokerAccount[] }) {
  const [runs, setRuns] = useState<PaperRun[] | null>(null)
  const [paper, setPaper] = useState<PaperFills>(new Map())
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let alive = true

    const read = async () => {
      try {
        const status = await api.paperStatus()
        if (!alive) return
        setRuns(status.runs)

        // One detail request per run, in parallel. A run whose detail fails is
        // reported and skipped rather than blanking the page: nine books'
        // statistics are worth more than a clean error over all ten.
        const settled = await Promise.allSettled(
          status.runs.map(async (run) => {
            const detail = await api.paperRun(run.id, 1)
            return detail.fills.map<Closed>((fill) => ({
              runId: run.id,
              strategy: run.strategy,
              market: run.market,
              tf: run.tf,
              direction: fill.direction,
              entryTime: fill.entryTime,
              entryPrice: fill.entryPrice,
              exitTime: fill.exitTime,
              holdMs: fill.holdMs,
              money: fill.pnlUsd,
              // A paper book pays the spread the engine charged it — but only
              // a trade that RECORDED what it was charged can be called exact.
              // This read `rebateUsd === null ? null : 'EXACT'` and was true
              // for one day: from 2026-09-21 the engine carries the contract
              // size and the spread each trade was sized under, and every
              // trade closed before that carries neither. Calling those exact
              // would put the confident word on precisely the trades whose
              // basis is a guess at today's config — the restatement the
              // carrying exists to stop
              // (docs/decisions/2026-09-21-restated-pnl-contract-size.md).
              // `null` still means no arrangement is recorded at all, which
              // is not a rebate of nothing.
              rebate: fill.rebateUsd,
              rebateBasis:
                fill.rebateUsd === null ? null : fill.contractSize == null ? 'ESTIMATED' : 'EXACT',
              r: fill.r,
              mae: fill.mae,
              mfe: fill.mfe,
            }))
          }),
        )
        if (!alive) return
        const collected: PaperFills = new Map()
        const failed: string[] = []
        settled.forEach((result, i) => {
          if (result.status === 'fulfilled') collected.set(status.runs[i].id, result.value)
          else failed.push(status.runs[i].id)
        })
        setPaper(collected)
        setError(failed.length ? `no book for ${failed.join(', ')}` : null)
      } catch (e) {
        if (alive) setError((e as Error).message)
      } finally {
        if (alive) setLoading(false)
      }
    }

    void read()
    const timer = window.setInterval(() => void read(), REFRESH_MS)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  const account = typeof book === 'number' ? (accounts.find((a) => a.login === book) ?? null) : null

  /**
   * The books this page is about: every run on paper, and on an account only
   * the runs whose executor has ever reported into it. A run mirrored nowhere
   * has no business in an account's strategy table.
   */
  const scoped = useMemo<PaperRun[]>(() => {
    const all = runs ?? []
    if (typeof book !== 'number') return all
    return all.filter((r) => r.brokers.some((b) => b.login === book))
  }, [runs, book])

  /** The closed trades on screen, in the money of the book on screen. */
  const { trades, money, dropped } = useMemo(() => {
    if (typeof book !== 'number') {
      return { trades: [...paper.values()].flat(), money: PAPER_MONEY, dropped: 0 }
    }
    const list: Closed[] = []
    let dropped = 0
    let currency: string | null = account?.currency ?? null
    for (const run of scoped) {
      // Newest snapshot first on the wire; the one for THIS login is the row.
      const b = run.brokers.find((x) => x.login === book)
      if (!b) continue
      currency ??= b.currency
      for (const f of b.fills) {
        // A fill the broker could not date or price cannot be placed on a
        // curve or in a session; it is counted as dropped rather than parked
        // at zero, where it would read as a scratch.
        if (f.exitTime === null || f.entryTime === null || f.pnl === null) {
          dropped += 1
          continue
        }
        list.push({
          runId: run.id,
          strategy: run.strategy,
          market: run.market,
          tf: run.tf,
          direction: f.direction,
          entryTime: f.entryTime,
          entryPrice: f.entryPrice,
          exitTime: f.exitTime,
          holdMs: Math.max(0, f.exitTime - f.entryTime),
          money: f.pnl,
          // The executor's own figure, in the account's currency, with its
          // provenance beside it - most of these are estimated from the
          // configured spread because the one the fill actually paid was
          // never recorded. The header says how many.
          rebate: f.rebate,
          rebateBasis: f.rebate === null ? null : f.rebateBasis,
          r: null,
          mae: null,
          mfe: null,
        })
      }
    }
    return { trades: list, money: accountMoney(currency), dropped }
  }, [book, paper, scoped, account])

  /** One row per strategy, with the books that ran it folded together. */
  const byStrategy = useMemo(() => {
    const groups = new Map<string, Closed[]>()
    for (const t of trades) {
      const list = groups.get(t.strategy)
      if (list) list.push(t)
      else groups.set(t.strategy, [t])
    }
    // A run that has closed nothing still gets a row: "this strategy has
    // traded nothing" is a fact about the desk, and dropping it would make
    // the page quietly disagree with the Desk's ten.
    for (const run of scoped) if (!groups.has(run.strategy)) groups.set(run.strategy, [])
    return [...groups.entries()]
      .map(([strategy, list]) => ({
        strategy,
        books: scoped.filter((r) => r.strategy === strategy),
        stats: summarise(list),
        trades: list,
      }))
      .sort((a, b) => b.stats.net - a.stats.net || a.strategy.localeCompare(b.strategy))
  }, [trades, scoped])

  const overall = useMemo(() => summarise(trades), [trades])

  if (loading && !runs) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    )
  }

  if ((runs?.length ?? 0) === 0) {
    return (
      <div className="text-muted-foreground px-4 py-8 text-[13px]">
        <p className="text-foreground">No paper run is registered, so there is nothing to analyse.</p>
        <p className="mt-1">Start the books on the Desk first.</p>
      </div>
    )
  }

  if (typeof book === 'number' && scoped.length === 0) {
    return (
      <div className="flex min-h-0 flex-col">
        <TopStrip
          book={book}
          account={account}
          overall={overall}
          runs={scoped}
          money={money}
          error={error}
          dropped={dropped}
        />
        <div className="text-muted-foreground px-4 py-8 text-[13px]">
          <p className="text-foreground">No book has ever reported into account {book}.</p>
          <p className="mt-1">
            An executor writes its account's record every poll; until one runs against this login there is nothing
            the account has done. Paper figures are on the paper book, one switch away in the sidebar.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-col">
      <TopStrip
        book={book}
        account={account}
        overall={overall}
        runs={scoped}
        money={money}
        error={error}
        dropped={dropped}
      />
      <div className="min-h-0 flex-1 space-y-4 overflow-auto px-3 py-3">
        <StrategyTable rows={byStrategy} money={money} />
        {typeof book === 'number' && <BookVsAccount login={book} runs={scoped} paper={paper} account={trades} money={money} />}
        <Curves rows={byStrategy} money={money} />
        <div className="grid gap-4 xl:grid-cols-2">
          <SessionTable trades={trades} money={money} />
          <SymbolTable trades={trades} runs={scoped} money={money} />
        </div>
        <Distribution trades={trades} overall={overall} rows={byStrategy} money={money} onAccount={typeof book === 'number'} />
      </div>
    </div>
  )
}

/* ----------------------------------------------------------- top strip */

function TopStrip({
  book,
  account,
  overall,
  runs,
  money,
  error,
  dropped,
}: {
  book: Book
  account: BrokerAccount | null
  overall: Stats
  runs: PaperRun[]
  money: Money
  error: string | null
  dropped: number
}) {
  const onAccount = typeof book === 'number'
  /**
   * What the books on screen are HOLDING OUT of the figures beside this.
   *
   * Without this the page is worse than it was before exclusions existed: a
   * book whose only trade is excluded reads `0 closed, net $0` with nothing
   * saying why, and `eur-hours` — which reported +$177.51 until 2026-09-21 —
   * would simply go quiet. A number removed without its size is an assertion
   * the reader cannot check, so the size is published here and the reason is
   * one hover away. Paper only: `excluded` is a property of a book's own
   * record, and an account's rows are what the broker did.
   */
  const heldOut = ((): { trades: number; net: number; reasons: string[] } | null => {
    if (typeof book === 'number') return null
    let trades = 0
    let net = 0
    const reasons = new Set<string>()
    for (const run of runs) {
      const x = run.excluded
      if (!x) continue
      trades += x.trades
      net += x.net_usd
      for (const f of x.fills ?? []) if (f.excludedReason) reasons.add(f.excludedReason)
    }
    return trades > 0 ? { trades, net, reasons: [...reasons] } : null
  })()



  // The desk's age is the oldest book's start, not the newest: "uptime" is how
  // long this account has been running, and a book added yesterday does not
  // reset it. On an account it is the first thing the account ever did for a
  // book: the earliest snapshot is not kept, so the earliest fill stands in.
  const started = onAccount
    ? overall.curve.length
      ? Math.min(...overall.curve.map((p) => p.t))
      : null
    : runs.length
      ? Math.min(...runs.map((r) => r.started_at))
      : null
  const days = started ? (Date.now() - started) / 86_400_000 : 0

  // Balance is what the books hold now; capital is what they were opened
  // with, derived rather than assumed — each book's own equity less its own
  // net is its starting balance, so a book started at something other than the
  // configured default is still counted correctly. On an account the balance
  // is the broker's own figure and capital is that less what was realised
  // through the books on screen.
  const balance = onAccount ? (account?.balance ?? null) : runs.reduce((a, r) => a + r.equity, 0)
  const capital = onAccount
    ? balance === null
      ? null
      : balance - overall.net
    : runs.reduce((a, r) => a + (r.equity - r.net_usd), 0)
  const ret = capital !== null && balance !== null && capital > 0 ? balance / capital - 1 : 0

  const realMoney = account !== null && (account.real_money || account.demo === false)

  // How many of these trades carry a credit at all. Zero means no rebate
  // line is drawn - not a credit of nothing.
  const priced = overall.rebateExact + overall.rebateEstimated
  const rebateTitle = [
    `The introducing-broker credit on the spread these trades paid, in ${money.unit}.`,
    overall.rebateExact > 0
      ? `${overall.rebateExact} priced from the spread the trade actually paid.`
      : '',
    overall.rebateEstimated > 0
      ? `${overall.rebateEstimated} ESTIMATED from the configured spread, because the spread the fill actually paid was never recorded. On gold the configured 0.28 sits above every spread this desk has measured, so those run high.`
      : '',
    overall.rebateUnpriced > 0
      ? `${overall.rebateUnpriced} could not be priced at all and are in no total.`
      : '',
    'Counted beside the net and never inside it.',
  ]
    .filter(Boolean)
    .join(' ')

  return (
    <div className="text-muted-foreground flex h-8 shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b px-3 text-[11px]">
      {onAccount ? (
        <span
          className={cn('num', realMoney ? 'text-caution' : 'text-foreground')}
          title={
            realMoney
              ? 'A funded account. Every figure on this page is money the broker actually moved.'
              : 'A practice account. Broker figures, but no money.'
          }
        >
          {realMoney ? 'REAL' : 'DEMO'} {book}
          {account && <span className="text-muted-foreground"> · {account.id}</span>}
          <span className="text-muted-foreground"> · mirrors {runs.length} book{runs.length === 1 ? '' : 's'}</span>
        </span>
      ) : (
        <span className="num text-foreground">
          PAPER <span className="text-muted-foreground">· {runs.length} books</span>
        </span>
      )}
      <span className="num">
        balance{' '}
        <span className="text-foreground">
          {balance === null
            ? '—'
            : onAccount
              ? `${balance.toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 })} ${money.unit}`
              : `$${Math.round(balance).toLocaleString('en-US')}`}
        </span>
        {capital !== null && (
          <span className="text-muted-foreground">
            {' '}
            of{' '}
            {onAccount
              ? `${capital.toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 })}`
              : `$${Math.round(capital).toLocaleString('en-US')}`}
          </span>
        )}
      </span>
      <span
        className="num"
        title={`What the ${onAccount ? 'account' : 'books'} made over these closed trades, after the spread. BEFORE the introducing-broker rebate, which is the figure beside it.`}
      >
        net <span className={bySign(overall.net)}>{money.fmt(overall.net)}</span>
        <span className={cn('ml-1', bySign(ret))}>({ret >= 0 ? '+' : MINUS}{pct(Math.abs(ret))})</span>
      </span>
      {/* THE REBATE, AND THE BOOK NET OF IT - two figures, never one.
          The owner is the introducing broker on these accounts, so part of
          the spread this trading pays comes back to him. He asked for it as
          a credit BESIDE the book rather than as a change to the cost model,
          so `net` above is untouched and a reader can see how much of a
          result is the strategy and how much is the arrangement.

          Shown only when something was actually priced. Nothing here is a
          zero standing in for an absence: a desk with no arrangement
          recorded, or a book none of whose trades could be priced, shows no
          rebate at all rather than a credit of nought. */}
      {priced > 0 && (
        <>
          <span className="num" title={rebateTitle}>
            rebate <span className={bySign(overall.rebate)}>{money.fmt(overall.rebate)}</span>
            {/* The provenance, next to the figure and not in a tooltip -
                `ui/DESIGN.md`: a figure the data cannot fully support says so
                where it is read. Amber is this strip's existing word for "a
                caveat applies", the same one the undated-fills note wears;
                no new colour is introduced for it. */}
            {overall.rebateExact > 0 && (
              <span className="text-muted-foreground"> {overall.rebateExact} exact</span>
            )}
            {overall.rebateEstimated > 0 && (
              <span className="text-caution"> {overall.rebateEstimated} estimated</span>
            )}
            {overall.rebateUnpriced > 0 && (
              <span className="text-caution"> {overall.rebateUnpriced} unpriced</span>
            )}
          </span>
          <span
            className="num"
            title="The same trades with the rebate added. Published beside the net, never instead of it."
          >
            net + rebate{' '}
            <span className={bySign(overall.net + overall.rebate)}>
              {money.fmt(overall.net + overall.rebate)}
            </span>
          </span>
        </>
      )}
      <span className="num">
        {overall.trades} <span className="text-muted-foreground">closed</span>
      </span>
      {/* HELD OUT, AND SAID SO. Excluded is not deleted: the trades are still
          in each book's own fill list, and what they came to is printed here
          so that a count which dropped is visibly a count that dropped.
          Caution amber, this strip's existing word for "a caveat applies" -
          no new colour. */}
      {heldOut && (
        <span
          className="num text-caution"
          title={`Held out of every figure on this strip, and still in each book's own fills: ${heldOut.reasons.join('; ')}. Registered in config/exclusions.toml before these figures were read.`}
        >
          {heldOut.trades} excluded{' '}
          <span className="text-muted-foreground">{money.fmt(heldOut.net)} not counted</span>
        </span>
      )}
      <span className="num">
        win <span className="text-foreground">{overall.trades ? pct(overall.winRate) : '—'}</span>
      </span>
      <span className="num">
        PF{' '}
        <span className="text-foreground">
          {overall.profitFactor === null ? '—' : overall.profitFactor.toFixed(2)}
        </span>
      </span>
      <span className="num">
        peak-to-trough <span className="text-lp">{overall.trades ? money.fmt(-overall.maxDrawdown) : '—'}</span>
      </span>
      <span className="num">
        {onAccount ? 'first fill' : 'uptime'}{' '}
        <span className="text-foreground">
          {started === null ? '—' : days < 1 ? `${(days * 24).toFixed(0)}h` : `${days.toFixed(1)}d`}
          {started !== null && onAccount ? ' ago' : ''}
        </span>
      </span>
      {dropped > 0 && (
        <span className="text-caution" title="Fills the broker reported without a time or a result. Not counted anywhere.">
          {dropped} fill{dropped === 1 ? '' : 's'} undated
        </span>
      )}
      {error && <span className="text-caution ml-auto truncate">{error}</span>}
    </div>
  )
}

/* ------------------------------------------------------ strategy table */

/** Track widths; the wrapper's `min-w` is their sum plus the gaps. */
const STRAT_COLS =
  'grid-cols-[minmax(150px,1.4fr)_minmax(120px,1fr)_58px_86px_56px_58px_72px_72px_86px_66px_64px]'

function StrategyTable({
  rows,
  money,
}: {
  rows: { strategy: string; books: PaperRun[]; stats: Stats; trades: Closed[] }[]
  money: Money
}) {
  const onAccount = money !== PAPER_MONEY
  return (
    <section>
      <Heading
        note={`${rows.length} strateg${rows.length === 1 ? 'y' : 'ies'} · every closed trade, folded by strategy`}
      >
        By strategy
      </Heading>
      <div className="overflow-x-auto">
        <div className="min-w-[1040px]">
          <div
            className={cn(
              'text-muted-foreground grid gap-2 border-b px-2 py-1 text-[10px] tracking-wide uppercase',
              STRAT_COLS,
            )}
          >
            <span>Strategy</span>
            <span>Books</span>
            <span className="text-right">Trades</span>
            <span className="text-right">Net</span>
            <span className="text-right">Win</span>
            <span className="text-right">PF</span>
            {onAccount ? (
              <>
                <span className="text-right" title={`Mean profit and loss per closed trade, in ${money.unit}. A broker has no stop distance, so there is no R here.`}>
                  Mean
                </span>
                <span className="text-right">Median</span>
              </>
            ) : (
              <>
                <span className="text-right" title="Mean profit and loss in units of the trade's own risk.">
                  Mean R
                </span>
                <span className="text-right">Median R</span>
              </>
            )}
            <span className="text-right" title={`Deepest fall from this strategy's own running peak, in ${money.unit}.`}>
              Drawdown
            </span>
            <span className="text-right">Hold</span>
            <span className="text-right" title="Longest run of consecutive winners / losers.">
              Streak
            </span>
          </div>
          {rows.map(({ strategy, books, stats }) => (
            <div
              key={strategy}
              className={cn(
                'hover:bg-elevated/60 grid items-baseline gap-2 border-b px-2 py-1 text-[12px] last:border-b-0',
                STRAT_COLS,
              )}
            >
              <span className="num text-foreground truncate" title={strategy}>
                {strategy}
              </span>
              <span className="text-muted-foreground num truncate text-[11px]" title={books.map((b) => b.id).join(', ')}>
                {books.map((b) => b.id).join(', ') || '—'}
              </span>
              <span className="num text-right">{stats.trades || '—'}</span>
              <span className={cn('num text-right', bySign(stats.net))}>
                {stats.trades ? money.fmt(stats.net) : '—'}
              </span>
              <span className="num text-right">{stats.trades ? pct(stats.winRate) : '—'}</span>
              <span className="num text-right">
                {stats.profitFactor === null ? '—' : stats.profitFactor.toFixed(2)}
              </span>
              {stats.hasR ? (
                <>
                  <span className={cn('num text-right', bySign(stats.meanR))}>
                    {stats.trades ? signedR(stats.meanR) : '—'}
                  </span>
                  <span className={cn('num text-right', bySign(stats.medianR))}>
                    {stats.trades ? signedR(stats.medianR) : '—'}
                  </span>
                </>
              ) : (
                <>
                  <span className={cn('num text-right', bySign(stats.meanMoney))}>
                    {stats.trades ? money.fmt(stats.meanMoney) : '—'}
                  </span>
                  <span className={cn('num text-right', bySign(stats.medianMoney))}>
                    {stats.trades ? money.fmt(stats.medianMoney) : '—'}
                  </span>
                </>
              )}
              <span className="num text-lp text-right">{stats.trades ? money.fmt(-stats.maxDrawdown) : '—'}</span>
              <span className="num text-muted-foreground text-right">
                {stats.trades ? hold(stats.meanHoldMs) : '—'}
              </span>
              <span className="num text-right text-[11px]">
                {stats.trades ? (
                  <>
                    <span className="text-lc">{stats.longestWin}</span>
                    <span className="text-muted-foreground">/</span>
                    <span className="text-lp">{stats.longestLoss}</span>
                  </>
                ) : (
                  '—'
                )}
              </span>
            </div>
          ))}
        </div>
      </div>
      <Footnote>
        One row per strategy, not per book — a strategy several books run is folded into one row and its books are
        named beside it. A strategy that has closed nothing is listed rather than hidden, because a book that has not
        traded is a fact about the desk. Drawdown is measured on each strategy&rsquo;s own cumulative profit and loss
        from zero, so the column does not add up and is not meant to.
        {onAccount && (
          <>
            {' '}
            On an account the trades are the broker&rsquo;s own closed deals, in {money.unit}, commission and swap
            included, and only for the books mirrored into this login.
          </>
        )}
      </Footnote>
    </section>
  )
}

/* -------------------------------------------------- book vs account */

const MIRROR_COLS = 'grid-cols-[minmax(150px,1.2fr)_64px_92px_64px_92px_72px_84px_72px]'

/**
 * The account against the books it mirrors, one row per book.
 *
 * The paper book is the rule executed perfectly at the bar's price; the
 * account is what the broker did with it, a spread and a poll later. The gap
 * between the two is the executor's cost, and it is a number the owner asked
 * for by name. Fills are matched by run, side and an entry inside one bar of
 * the book's; anything unmatched is counted, not guessed.
 */
function BookVsAccount({
  login,
  runs,
  paper,
  account,
  money,
}: {
  login: number
  runs: PaperRun[]
  paper: PaperFills
  account: Closed[]
  money: Money
}) {
  const rows = useMemo(
    () =>
      runs.map((run) => {
        const p = paper.get(run.id) ?? []
        const a = account.filter((t) => t.runId === run.id)
        const b = run.brokers.find((x) => x.login === login) ?? null
        const bar = tfMs(run.tf)
        const usedPaper = new Set<number>()
        let matched = 0
        let adverseSum = 0
        let holdDiffSum = 0
        for (const t of a) {
          let best: { i: number; gap: number } | null = null
          p.forEach((q, i) => {
            if (usedPaper.has(i) || q.direction !== t.direction) return
            const gap = Math.abs(q.entryTime - t.entryTime)
            if (gap <= bar && (!best || gap < best.gap)) best = { i, gap }
          })
          if (!best) continue
          const q = p[(best as { i: number }).i]
          usedPaper.add((best as { i: number }).i)
          matched += 1
          if (t.entryPrice !== null && q.entryPrice !== null) {
            // Positive means the account got the worse price: paid more on a
            // long, sold lower on a short.
            adverseSum += t.direction === 'SHORT' ? q.entryPrice - t.entryPrice : t.entryPrice - q.entryPrice
          }
          holdDiffSum += t.holdMs - q.holdMs
        }
        return {
          run,
          paperCount: p.length,
          paperNet: p.reduce((s, t) => s + t.money, 0),
          paperR: p.reduce((s, t) => s + (t.r ?? 0), 0),
          accountCount: a.length,
          accountNet: a.reduce((s, t) => s + t.money, 0),
          matched,
          notTaken: Math.max(0, p.length - matched),
          extra: Math.max(0, a.length - matched),
          drift: matched ? adverseSum / matched : null,
          holdDiff: matched ? holdDiffSum / matched : null,
          at: b?.at ?? 0,
          open: b?.position ?? null,
        }
      }),
    [runs, paper, account, login],
  )

  return (
    <section>
      <Heading note="the rule at the bar's price, beside what the broker did with it">Book against account</Heading>
      <div className="overflow-x-auto">
        <div className="min-w-[860px]">
          <div
            className={cn(
              'text-muted-foreground grid gap-2 border-b px-2 py-1 text-[10px] tracking-wide uppercase',
              MIRROR_COLS,
            )}
          >
            <span>Book</span>
            <span className="text-right" title="Closed trades on the paper book.">Paper</span>
            <span className="text-right" title="The paper book's net, in dollars and in R.">Paper net</span>
            <span className="text-right" title="Closed deals on this account for this book.">Account</span>
            <span className="text-right" title={`The account's realised result for this book, in ${money.unit}.`}>Account net</span>
            <span className="text-right" title="Paper entries with no account deal inside one bar of them, on the same side / account deals with no paper entry.">
              Missed / extra
            </span>
            <span className="text-right" title="Mean entry drift on matched pairs, in price. Positive is the account getting the worse price.">
              Entry drift
            </span>
            <span className="text-right" title="Mean hold difference on matched pairs: account minus paper.">Hold Δ</span>
          </div>
          {rows.map((r) => (
            <div
              key={r.run.id}
              className={cn(
                'hover:bg-elevated/60 grid items-baseline gap-2 border-b px-2 py-1 text-[12px] last:border-b-0',
                MIRROR_COLS,
              )}
            >
              <span className="num text-foreground truncate" title={r.run.label ?? r.run.id}>
                {r.run.id}
                {r.open && (
                  <span className="text-muted-foreground ml-1.5 text-[10px]" title="The account holds a position on this book now; it is not in any count here until it closes.">
                    open {r.open.side ?? '?'}
                  </span>
                )}
              </span>
              <span className="num text-right">{r.paperCount || '—'}</span>
              <span className={cn('num text-right', bySign(r.paperNet))}>
                {r.paperCount ? (
                  <>
                    {PAPER_MONEY.fmt(r.paperNet)}
                    <span className="text-muted-foreground ml-1 text-[10px]">{signedR(r.paperR)}</span>
                  </>
                ) : (
                  '—'
                )}
              </span>
              <span className="num text-right">{r.accountCount || '—'}</span>
              <span className={cn('num text-right', bySign(r.accountNet))}>
                {r.accountCount ? money.fmt(r.accountNet) : '—'}
              </span>
              <span className="num text-right">
                {r.paperCount || r.accountCount ? (
                  <>
                    <span className={r.notTaken ? 'text-caution' : 'text-muted-foreground'}>{r.notTaken}</span>
                    <span className="text-muted-foreground">/</span>
                    <span className={r.extra ? 'text-caution' : 'text-muted-foreground'}>{r.extra}</span>
                  </>
                ) : (
                  '—'
                )}
              </span>
              <span className={cn('num text-right', r.drift === null ? 'text-muted-foreground' : r.drift > 0 ? 'text-lp' : 'text-lc')}>
                {r.drift === null ? '—' : `${r.drift < 0 ? MINUS : '+'}${Math.abs(r.drift).toFixed(2)}`}
              </span>
              <span className="num text-muted-foreground text-right">{r.holdDiff === null ? '—' : signedHold(r.holdDiff)}</span>
            </div>
          ))}
        </div>
      </div>
      <Footnote>
        Paper net is in the book&rsquo;s dollars and account net in the account&rsquo;s {money.unit}; they sit side by
        side to be compared in shape, never added. A paper entry counts as missed when no account deal on the same
        side opened within one bar of it — the mirror joined late, the guard stood it out, or the terminal refused.
        Entry drift is the mean price the account gave up on the pairs that did match: the spread, the poll, and the
        one-bar lag, in one number.
      </Footnote>
    </section>
  )
}

/* ------------------------------------------------------- equity curves */

function Curves({ rows, money }: { rows: { strategy: string; stats: Stats }[]; money: Money }) {
  const withTrades = rows.filter((r) => r.stats.trades > 0)
  return (
    <section>
      <Heading note="cumulative profit and loss from zero, by exit time">Equity</Heading>
      {withTrades.length === 0 ? (
        <Empty>No book has closed a trade yet, so there is no curve to draw.</Empty>
      ) : (
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-5">
          {withTrades.map(({ strategy, stats }) => (
            <figure key={strategy} className="border-border bg-card rounded-sm border px-2 py-1.5">
              <figcaption className="flex items-baseline justify-between gap-2 text-[11px]">
                <span className="num text-foreground truncate" title={strategy}>
                  {strategy}
                </span>
                <span className={cn('num shrink-0', bySign(stats.net))}>{money.fmt(stats.net)}</span>
              </figcaption>
              <Spark points={stats.curve} />
              <p className="text-muted-foreground num mt-0.5 text-[10px]">
                {stats.trades} trade{stats.trades === 1 ? '' : 's'}
              </p>
            </figure>
          ))}
        </div>
      )}
    </section>
  )
}

/**
 * A step line, because money moves when a trade closes and not between.
 *
 * Deliberately axis-free: at this size a tick label is unreadable and the
 * number that matters is printed beside the caption. The zero line is drawn
 * because the sign of the curve is the whole question.
 */
function Spark({ points }: { points: { t: number; v: number }[] }) {
  const W = 220
  const H = 46
  if (points.length === 0) return <div style={{ height: H }} />

  const xs = points.map((p) => p.t)
  const vs = [0, ...points.map((p) => p.v)]
  const tMin = Math.min(...xs)
  const tMax = Math.max(...xs)
  let vMin = Math.min(...vs)
  let vMax = Math.max(...vs)
  if (!(vMax - vMin > 1e-9)) {
    vMin -= 1
    vMax += 1
  }
  // Breathing room, or the extreme of every curve is drawn along the border of
  // its own card and reads as clipped rather than as a high.
  const pad = (vMax - vMin) * 0.12
  vMin -= pad
  vMax += pad
  const x = (t: number) => (tMax === tMin ? W / 2 : ((t - tMin) / (tMax - tMin)) * W)
  const y = (v: number) => H - ((v - vMin) / (vMax - vMin)) * H

  let d = `M 0 ${y(0).toFixed(1)}`
  let prev = 0
  for (const p of points) {
    const px = x(p.t).toFixed(1)
    d += ` L ${px} ${y(prev).toFixed(1)} L ${px} ${y(p.v).toFixed(1)}`
    prev = p.v
  }
  const last = points[points.length - 1].v
  const zeroY = y(0)
  const inside = zeroY >= 0 && zeroY <= H

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      // Stretch rather than letterbox: the default `xMidYMid meet` centres a
      // 220-wide drawing inside whatever the card is and leaves the curve
      // floating in the middle of empty space.
      preserveAspectRatio="none"
      className="mt-1 block w-full"
      style={{ height: H }}
      role="img"
      aria-hidden
    >
      {inside && (
        <line x1={0} y1={zeroY} x2={W} y2={zeroY} className="stroke-border" strokeWidth={1} strokeDasharray="2 2" />
      )}
      {/* `vectorEffect` keeps the stroke 1.5px after the non-uniform stretch;
          without it the vertical segments would be drawn much thinner than the
          horizontal ones. */}
      <path
        d={d}
        fill="none"
        strokeWidth={1.5}
        vectorEffect="non-scaling-stroke"
        className={last >= 0 ? 'stroke-lc' : 'stroke-lp'}
      />
    </svg>
  )
}

/* ------------------------------------------------- session and symbol */

/** The per-trade column: R where every trade has one, money otherwise. */
function PerTrade({ stats, money }: { stats: Stats; money: Money }) {
  if (!stats.trades) return <span className="num text-right">—</span>
  return stats.hasR ? (
    <span className={cn('num text-right', bySign(stats.meanR))}>{signedR(stats.meanR)}</span>
  ) : (
    <span className={cn('num text-right', bySign(stats.meanMoney))}>{money.fmt(stats.meanMoney)}</span>
  )
}

function SessionTable({ trades, money }: { trades: Closed[]; money: Money }) {
  const rows = useMemo(
    () =>
      SESSIONS.map((s) => {
        const inside = trades.filter((t) => inSession(nyHour(t.entryTime), s.from, s.to))
        return { ...s, stats: summarise(inside) }
      }),
    [trades],
  )
  const outside = useMemo(
    () =>
      summarise(
        trades.filter((t) => {
          const h = nyHour(t.entryTime)
          return !SESSIONS.some((s) => inSession(h, s.from, s.to))
        }),
      ),
    [trades],
  )
  const perTrade = money === PAPER_MONEY ? 'Mean R' : 'Mean'

  return (
    <section>
      <Heading note="by the hour the trade was entered, on a New York clock">By session</Heading>
      {trades.length === 0 ? (
        <Empty>No closed trade to place in a session yet.</Empty>
      ) : (
        <Grid head={['Session', 'Trades', 'Net', 'Win', perTrade]}>
          {[...rows, { label: 'Outside', title: 'No named session covers this hour', stats: outside }].map((r) => (
            <div key={r.label} className="contents">
              <span className="num text-foreground" title={r.title}>
                {r.label}
              </span>
              <span className="num text-right">{r.stats.trades || '—'}</span>
              <span className={cn('num text-right', bySign(r.stats.net))}>
                {r.stats.trades ? money.fmt(r.stats.net) : '—'}
              </span>
              <span className="num text-right">{r.stats.trades ? pct(r.stats.winRate) : '—'}</span>
              <PerTrade stats={r.stats} money={money} />
            </div>
          ))}
        </Grid>
      )}
      <Footnote>
        These are the same windows the Workbench offers and the research batches use, and London and New York
        <strong className="text-foreground/80"> overlap between 08:00 and 11:00</strong> — a trade entered then is
        counted in both rows, so the rows do not sum to the total. Inventing tidier boundaries here would make this page
        disagree with the filter the strategies actually run under.
      </Footnote>
    </section>
  )
}

function SymbolTable({ trades, runs, money }: { trades: Closed[]; runs: PaperRun[]; money: Money }) {
  const rows = useMemo(() => {
    const groups = new Map<string, Closed[]>()
    for (const t of trades) {
      const key = `${t.market}:${t.tf}`
      const list = groups.get(key)
      if (list) list.push(t)
      else groups.set(key, [t])
    }
    for (const r of runs) {
      const key = `${r.market}:${r.tf}`
      if (!groups.has(key)) groups.set(key, [])
    }
    return [...groups.entries()]
      .map(([key, list]) => ({ key, stats: summarise(list) }))
      .sort((a, b) => b.stats.trades - a.stats.trades || a.key.localeCompare(b.key))
  }, [trades, runs])
  const perTrade = money === PAPER_MONEY ? 'Mean R' : 'Mean'

  return (
    <section>
      <Heading note="by the instrument and timeframe the book steps">By instrument</Heading>
      <Grid head={['Instrument', 'Trades', 'Net', 'Win', perTrade]}>
        {rows.map((r) => (
          <div key={r.key} className="contents">
            <span className="num text-foreground">{r.key}</span>
            <span className="num text-right">{r.stats.trades || '—'}</span>
            <span className={cn('num text-right', bySign(r.stats.net))}>
              {r.stats.trades ? money.fmt(r.stats.net) : '—'}
            </span>
            <span className="num text-right">{r.stats.trades ? pct(r.stats.winRate) : '—'}</span>
            <PerTrade stats={r.stats} money={money} />
          </div>
        ))}
      </Grid>
    </section>
  )
}

/* ------------------------------------------- streaks and distribution */

/** R buckets, chosen so a full stop loss and a clean win each land in one. */
const R_BUCKETS: { label: string; lo: number; hi: number }[] = [
  { label: '≤ −2R', lo: -Infinity, hi: -2 },
  { label: '−2 … −1R', lo: -2, hi: -1 },
  { label: '−1 … 0R', lo: -1, hi: 0 },
  { label: '0 … +1R', lo: 0, hi: 1 },
  { label: '+1 … +2R', lo: 1, hi: 2 },
  { label: '≥ +2R', lo: 2, hi: Infinity },
]

function Distribution({
  trades,
  overall,
  rows,
  money,
  onAccount,
}: {
  trades: Closed[]
  overall: Stats
  rows: { strategy: string; stats: Stats }[]
  money: Money
  onAccount: boolean
}) {
  const buckets = useMemo(() => {
    const counts = R_BUCKETS.map(() => 0)
    for (const t of trades) {
      if (t.r === null) continue
      const r = t.r
      const i = R_BUCKETS.findIndex((b) => r >= b.lo && r < b.hi)
      if (i >= 0) counts[i] += 1
    }
    return counts
  }, [trades])
  const most = Math.max(1, ...buckets)

  // On an account there is no R to bucket, so every closed deal is drawn as
  // its own bar in exit order: the shape of one trade, trade by trade.
  const deals = useMemo(() => [...trades].sort((a, b) => a.exitTime - b.exitTime), [trades])
  const biggest = Math.max(1e-9, ...deals.map((t) => Math.abs(t.money)))

  const worstStreak = rows.reduce(
    (acc, r) => (r.stats.longestLoss > acc.n ? { n: r.stats.longestLoss, strategy: r.strategy } : acc),
    { n: 0, strategy: '' },
  )

  return (
    <section>
      <Heading note={onAccount ? 'what a single deal has looked like, on this account' : 'what a single trade has looked like, across every book'}>
        Distribution and streaks
      </Heading>
      {trades.length === 0 ? (
        <Empty>Nothing has closed, so there is no distribution.</Empty>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <div>
            {onAccount ? (
              <>
                <div className="space-y-0.5">
                  {deals.map((t, i) => (
                    <div
                      key={`${t.runId}-${t.exitTime}-${i}`}
                      className="grid grid-cols-[minmax(0,1fr)_1fr_78px] items-center gap-2 text-[11px]"
                      title={`${t.runId} · ${t.direction ?? '?'} · ${new Date(t.exitTime).toLocaleString()}`}
                    >
                      <span className="num text-muted-foreground truncate">{t.runId}</span>
                      <span className="bg-elevated relative h-3 overflow-hidden rounded-[2px]">
                        <span
                          className={cn('absolute top-0 block h-full', t.money < 0 ? 'right-1/2 bg-lp/70' : 'left-1/2 bg-lc/70')}
                          style={{ width: `${(Math.abs(t.money) / biggest) * 50}%` }}
                        />
                      </span>
                      <span className={cn('num text-right', bySign(t.money))}>{money.fmt(t.money)}</span>
                    </div>
                  ))}
                </div>
                <Footnote>
                  One bar per closed deal, oldest first, drawn from a shared zero. The broker reports no stop
                  distance, so there is no R on an account: the size of a bar is money, commission and swap included.
                </Footnote>
              </>
            ) : (
              <>
                <div className="space-y-1">
                  {R_BUCKETS.map((b, i) => (
                    <div key={b.label} className="grid grid-cols-[86px_1fr_36px] items-center gap-2 text-[11px]">
                      <span className="num text-muted-foreground">{b.label}</span>
                      <span className="bg-elevated h-3 overflow-hidden rounded-[2px]">
                        <span
                          className={cn('block h-full', b.hi <= 0 ? 'bg-lp/70' : 'bg-lc/70')}
                          style={{ width: `${(buckets[i] / most) * 100}%` }}
                        />
                      </span>
                      <span className="num text-right">{buckets[i] || ''}</span>
                    </div>
                  ))}
                </div>
                <Footnote>
                  R is the trade&rsquo;s profit in units of its own risk, as the engine sized it. A book whose stop is
                  a sizing unit rather than an order can exceed −1R, which is why the first bucket exists.
                </Footnote>
              </>
            )}
          </div>

          <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 self-start text-[12px]">
            <Stat label="Longest winning run" value={`${overall.longestWin}`} />
            <Stat label="Longest losing run" value={`${overall.longestLoss}`} />
            <Stat
              label="Worst run on one strategy"
              value={worstStreak.n ? `${worstStreak.n}` : '—'}
              note={worstStreak.strategy}
            />
            <Stat label="Mean hold" value={hold(overall.meanHoldMs)} />
            {overall.hasR ? (
              <>
                <Stat label="Best trade" value={signedR(overall.bestR)} tone={bySign(overall.bestR)} />
                <Stat label="Worst trade" value={signedR(overall.worstR)} tone={bySign(overall.worstR)} />
                <Stat
                  label="Mean excursion against"
                  value={signedR(overall.meanMae)}
                  tone="text-lp"
                  note="how far a trade went the wrong way before it closed"
                />
                <Stat
                  label="Mean excursion for"
                  value={signedR(overall.meanMfe)}
                  tone="text-lc"
                  note="how far it went the right way before it closed"
                />
              </>
            ) : (
              <>
                <Stat label="Best deal" value={money.fmt(overall.bestMoney)} tone={bySign(overall.bestMoney)} />
                <Stat label="Worst deal" value={money.fmt(overall.worstMoney)} tone={bySign(overall.worstMoney)} />
                <Stat label="Mean deal" value={money.fmt(overall.meanMoney)} tone={bySign(overall.meanMoney)} />
                <Stat
                  label="Excursions"
                  value="—"
                  note="the broker reports where a deal closed, not how far it wandered first"
                />
              </>
            )}
          </dl>
        </div>
      )}
      {trades.length > 0 && trades.length < THIN_SAMPLE && (
        <p className="text-caution mt-2 text-[11px]">
          {trades.length} closed {onAccount ? 'deal' : 'trade'}{trades.length === 1 ? '' : 's'}{' '}
          {onAccount ? 'on this account' : 'across every book'}. Nothing on this page is a measurement yet — a win rate
          over {trades.length} trades has a margin of roughly ±
          {Math.round((100 / Math.sqrt(trades.length)) * 0.5)} points, which is wider than any difference it could show.
        </p>
      )}
    </section>
  )
}

/* -------------------------------------------------------------- bits */

function Stat({
  label,
  value,
  tone,
  note,
}: {
  label: string
  value: string
  tone?: string
  note?: string
}) {
  return (
    <div className="border-border border-b pb-1">
      <dt className="text-muted-foreground text-[10px] tracking-wide uppercase">{label}</dt>
      <dd className={cn('num text-[13px]', tone ?? 'text-foreground')}>
        {value}
        {note && <span className="text-muted-foreground ml-1.5 text-[10px] normal-case">{note}</span>}
      </dd>
    </div>
  )
}

function Grid({ head, children }: { head: string[]; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(90px,1.4fr)_58px_92px_56px_80px] gap-2 text-[12px]">
      {head.map((h, i) => (
        <span
          key={h}
          className={cn('text-muted-foreground border-b pb-1 text-[10px] tracking-wide uppercase', i > 0 && 'text-right')}
        >
          {h}
        </span>
      ))}
      {children}
    </div>
  )
}

function Heading({ children, note }: { children: React.ReactNode; note?: string }) {
  return (
    <h2 className="mb-1.5 flex flex-wrap items-baseline gap-x-2 text-[11px] tracking-wide uppercase">
      <span className="text-foreground">{children}</span>
      {note && <span className="text-muted-foreground text-[10px] normal-case">{note}</span>}
    </h2>
  )
}

function Footnote({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-foreground mt-1.5 max-w-[80ch] text-[10px] leading-relaxed">{children}</p>
}

function Empty({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-foreground border-border rounded-sm border border-dashed px-3 py-6 text-[11px]">{children}</p>
}
