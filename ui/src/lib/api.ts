/**
 * API client.
 *
 * Every route here is served today by the JavaScript prototype and will be
 * served tomorrow by the Rust `fd-api` binary at the same paths. The contract is
 * the seam that lets the UI be built before the port reaches phase 4; nothing in
 * this file may assume which process is answering.
 */

export type FlowClass = 'LC' | 'LP' | 'SC' | 'SP' | 'UNKNOWN'

export interface MarketInfo {
  id: string
  label: string
  barSymbol: string
  barSource: string
  optionsSource: string
  hasData: boolean
}

export interface IndicatorInfo {
  id: string
  name: string
  pane: 'overlay' | 'pane'
  params: Record<string, number | string>
  outputs: string[]
}

export interface StrategyInfo {
  id: string
  name: string
  description: string
  params: Record<string, number>
  grid: Record<string, number[]> | null
  needsOptions: boolean
}

export interface Catalog {
  markets: MarketInfo[]
  activeMarket: string
  timeframes: string[]
  indicators: IndicatorInfo[]
  strategies: StrategyInfo[]
  defaults: { timeframe: string }
  fillModel?: string
}

export interface Bar {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume?: number
}

/**
 * The candle still forming on the timeframe asked for, aggregated SERVER-SIDE.
 *
 * The client cannot build this honestly above the timeframe the books trade.
 * `useTicks` keeps only the newest bar per `market:tf` and no history, and the
 * stream only carries a `market:tf` some run actually trades — nothing trades
 * 4h. So a client-built 4h open would be the price when the tab was loaded:
 * wrong by up to four hours, different for two people on the same chart, and
 * shaped exactly like a real candle. The server builds it instead, anchored to
 * the last closed bar's own stamp from the exported file and aggregated from
 * the finest stored series that covers the period.
 */
export interface FormingBar {
  /** The bar's start, from the closed file's anchor — never epoch-bucketed. */
  time: number
  open: number
  high: number
  low: number
  close: number
  /** Which stored series it was aggregated from, e.g. `15m` inside a `4h`. */
  from_timeframe: string
  /**
   * The END of the last finer bar included — how far into this candle the high
   * and low are actually KNOWN.
   *
   * Built from 15m bars, the extremes can be a quarter-hour stale while the
   * tick stream's close runs ahead of them. A wick drawn past this point would
   * claim "this is the high so far" about a high that is fifteen minutes old,
   * which is a smaller version of the lie the whole field exists to prevent.
   * The chart draws up to here and says the rest is the last price only.
   */
  complete_to_ms: number
}

export interface BarsResponse {
  market: string
  symbol: string
  timeframe: string
  /** True when the source has no highs or lows and the bar is a flat close. */
  synthetic: boolean
  /**
   * True when this market has an upstream websocket. Markets without one are
   * snapshots and must not be shown with a live badge.
   */
  live: boolean
  bars: Bar[]
  stats: { bars: number; from: number; to: number; days: number; gaps: number }

  /*
   * EVERYTHING BELOW IS OPTIONAL BECAUSE THE SERVER MAY BE OLDER THAN THIS
   * CLIENT. Deploys here are staged and manual, so a desk can be running a UI
   * built today against an API built last week; these fields read as absent
   * then, and every surface that uses them says "not reported" rather than
   * inventing a value. See docs/decisions/2026-09-17-deploy-staleness.md.
   */

  /** The timeframe's period in ms. For labelling and staleness ONLY — bar
   *  times are the broker's and are not multiples of this from the epoch. */
  bar_ms?: number
  /** The last CLOSED bar's OPEN time — the same stamp as its bar in `bars`.
   *  `null` on a series with no closed bar yet. */
  last_closed_bar_ms?: number | null
  /**
   * Which file the bars were read from, and whether anything was resampled.
   *
   * `exported_at_ms` is IN HERE and not beside `bars`, which is where the
   * agreed contract put it and where this client first read it — silently, so
   * the export age simply never rendered rather than erroring. It belongs
   * here: it is that file's last write time, a property of the source and not
   * of the response, and reading it from the wrong level is a fact quietly
   * missing rather than a wrong one.
   */
  source?: {
    file: string
    timeframe: string
    /** True only for steps that divide an hour, where the broker's whole-hour
     *  offset makes the anchor irrelevant. `4h` and `1d` are refused instead
     *  of resampled, because their anchor is the broker's and not the epoch's. */
    resampled: boolean
    /** When that file was last written, epoch ms UTC. A stamp and not a
     *  duration, so the client ages it against its own clock rather than
     *  against however long the response spent in flight. */
    exported_at_ms?: number | null
  }
  /** `null` when no finer series covers the period; the chart then shows
   *  closed bars only and says so. */
  forming?: FormingBar | null
}

export interface Metrics {
  trades: number
  winRate: number
  avgR: number
  profitFactor: number
  expectancy: number
  totalR: number
  netPnlUsd: number
  returnPct: number
  maxDrawdownUsd: number
  maxDrawdownPct: number
  sharpe: number
  avgMae: number
  avgMfe: number
  avgHoldMin: number
  exits: Record<string, number>
}

export interface BacktestTrade {
  direction: 'LONG' | 'SHORT'
  entryTime: number
  entryPrice: number
  exitTime: number
  exitPrice: number
  exitReason: string
  stop: number
  target: number | null
  lots: number
  pnlUsd: number
  r: number
  /**
   * Worst excursion while the trade was open, **in R** — the engine divides by
   * the position's risk before it rounds (`mae: position.mae / position.risk`),
   * so this is already a multiple of the stop distance and never a price.
   * Negative by construction.
   */
  mae: number
  /** Best excursion while the trade was open, **in R**. Positive by construction. */
  mfe: number
  holdMs: number
  reason: string
}

export interface BacktestResult {
  market: string
  strategy: string
  name: string
  timeframe: string
  params: Record<string, number>
  metrics: Metrics
  verdict: { promising: boolean; reasons: string[] }
  trades: BacktestTrade[]
  equityCurve: { time: number; equity: number }[]
  indicatorSpecs: { id: string; params?: Record<string, number> }[]
}

export interface LeaderboardRow {
  id: string
  name: string
  skipped?: string
  params?: Record<string, number>
  metrics?: Metrics
  trades?: number
}

export interface OptionsFrame {
  t: number
  spot: number
  bullRatio: number
  bullRatio15m: number
  netFlowVelocityNorm: number
  bigTradeImbalance: number
  clusters: { low: number; high: number; center: number; score: number; types: number; expirations: number }[]
  contexts: {
    symbol: string
    dte: number
    maxPain: number | null
    poc: number | null
    wSup: number | null
    wRes: number | null
    callBE: number | null
    putBE: number | null
    bullRatio: number
  }[]
}

export interface IndicatorPoint {
  time: number
  value: number
}

export interface ResearchRunRow {
  label: string
  base: string
  trades: number
  profitFactor: number
  expectancy: number
  nullP50: number | null
  nullP95: number | null
  percentile: number | null
  verdict: string
}

export interface ResearchRun {
  rows: ResearchRunRow[]
  survivors: string[]
  concluded: boolean
  modifiedAt: number
}

export interface ResearchDirection {
  base: string
  trades: number | null
  actualPf: number
  percentile: number | null
  outside: boolean
}

export interface ResearchHypothesis {
  id: string
  claim: string
  status: string
  registered: string
  inSample: string | null
  outOfSample: string | null
  batch: { label: string; base: string; why: string }[]
  runs: { inSample: ResearchRun | null; outOfSample: ResearchRun | null; direction: ResearchDirection[] }
  modifiedAt: number
}

/**
 * The bar still forming on a `market:tf`, as `POST /api/paper/tick` last
 * received it.
 *
 * Shown, never traded on. The paper loop decides on closed bars only, and the
 * server drops this after ninety seconds rather than let a dead feed be drawn
 * as a current price — so `live` being null is information, not a gap.
 */
export interface LiveBar {
  /** The bar's **open**, in epoch milliseconds — its bucket, not the read. */
  time: number
  open: number
  high: number
  low: number
  close: number
  volume?: number | null
  /** The quote at the read, when the source had one. */
  bid?: number | null
  ask?: number | null
  /** When the **server** received it, epoch ms. The age is measured from here. */
  at: number
}

/** One paper run, as `/api/paper/status` reports it (snake_case on the wire). */
export interface PaperRun {
  id: string
  /** The sentence the run was registered under — why it is on the desk at all. */
  label?: string
  market: string
  tf: string
  strategy: string
  /**
   * Who has actually driven this book, for a run driven from outside the
   * process; `null` on every rule-based run and on an `external` run nobody
   * has posted to yet. Earned, never configured: the API writes it only when
   * an intent is accepted. `decisions` keyed by more than one name means the
   * book's net is not one decider's record.
   */
  decider: null | {
    last: string
    last_at: number
    /** Accepted ENTRIES per decider name. */
    decisions: Record<string, number>
    /** Bars a decider looked at and asked for nothing. */
    stood_aside: number
  }
  params: Record<string, number>
  filters: string[]
  guards: boolean
  started_at: number
  bars: number
  bars_seen: number
  warmup_bars: number
  last_bar_time: number | null
  /** The close of that bar — what the live price is read up or down against. */
  last_bar_close: number | null
  equity: number
  /**
   * What this book's account is denominated in, and how many of those units
   * make a dollar (100 on a cent account). EVERY money field on this object is
   * in USD; these two exist so the client can show the number the account
   * holder actually sees, without the conversion ever touching the arithmetic.
   */
  account_currency: string
  units_per_usd: number
  /** Account leverage (2000 = 1:2000) and units of underlying per lot. */
  leverage: number
  contract_size: number
  open: null | {
    side: 'LONG' | 'SHORT'
    /** The bar whose OPEN this fill is PRICED at. Not when the desk knew about
     *  it, and not when the account filled — see `learned_at` below and the
     *  broker row's `opened_at`. Three clocks, three fields, because one of
     *  them was standing for all three until 2026-09-17. */
    entry_time: number
    /**
     * The wall clock at which fd-api LEARNED the book holds this, which is a
     * BAR after `entry_time` by construction: the fill is priced at the open
     * of the bar stamped `entry_time`, and that bar is only posted once it
     * CLOSES. Null on a position open before the field existed, or reloaded
     * from an older state file — absent, not zero.
     */
    learned_at: number | null
    entry_price: number
    lots: number
    /**
     * Dollars per one unit of price movement (`lots x contract_size`), so the
     * position can be marked against the LIVE tick rather than a close that
     * may be fifteen minutes old.
     */
    usd_per_point: number
    stop: number | null
    target: number | null
    mae: number
    mfe: number
    risk: number
    unrealised_usd_at_last_close: number
  }
  /**
   * An entry decided at the last close and waiting for the next open.
   *
   * Between those two moments the trade is real — it will happen, at a price
   * nobody knows yet — so a book with one is not flat, it is committed.
   */
  pending: null | {
    side: 'LONG' | 'SHORT' | string
    stop: number | null
    target: number | null
    reason: string
    /** The bar whose close produced it; the fill is the NEXT bar's open. */
    decided_on: number | null
  }
  trades: number
  net_usd: number
  profit_factor: number | null
  skipped_by_guard: Record<string, number>
  closed_by_guard: Record<string, number>
  sized_down: number
  skipped_no_atr: number
  gaps: number
  news: {
    events_loaded: number
    next_blackout: null | { time: number; currency: string; impact: number; name?: string }
    /** The last event this run's guards could ever act on, and the days to it.
     *  A calendar is a finite list; the day after its last entry the news guard
     *  stops guarding without failing and without logging. */
    horizon?: number | null
    horizon_days?: number | null
    /** Which series runs out first — the thing to go and refresh. A long
     *  series masks a short one, so this is the minimum across names and not
     *  the last event on the file. */
    horizon_name?: string | null
  }
  /** The forming bar on this run's stream, or null when none arrived inside 90 s. */
  live: LiveBar | null
  /** The last ten closed trades. The whole book is on `/api/paper/run/{id}`. */
  last_fills: BacktestTrade[]
  /** On `/status` this is a *count*, kept light; the detail route carries the rows. */
  equity_curve?: number
  /**
   * The broker accounts this book is mirrored into, newest snapshot first —
   * empty when no executor has ever run it, and longer than one when the book
   * runs on several accounts at once.
   *
   * A present entry is NOT a connected one. The API is Rust and cannot ask a
   * MetaTrader terminal anything, so each is a file the executor writes every
   * poll and the API serves back; when the executor stops, the file stays. Read
   * `at` before believing any number in it.
   */
  /** Turned off by hand: no new entries. An open position is still managed
   *  to its stop and target — pausing is not abandoning. Deliberate, and not
   *  the same as a driver that died. */
  paused: boolean
  brokers: PaperBroker[]
  /**
   * The process driving this book, as it last reported — written every poll,
   * not every bar.
   *
   * `null` means nobody has ever written one (a rule-driven run, or a driver
   * older than the feature), which is NOT the same as stopped: the desk falls
   * back to the decider's bar gap in that case.
   */
  driver: null | {
    /** When the driver last said it was alive, epoch ms. */
    at: number
    model: string | null
    /** The book the process primarily drives — a control book names its
     *  model's run here, which is how the pair is known to stop together. */
    run: string | null
    pid: number | null
    /** The driver's own poll interval, so staleness is judged against the
     *  cadence it actually keeps rather than a number baked into the UI. */
    poll_s: number | null
  }
}

/** What the broker's account holds for one book, as the executor last saw it. */
export interface PaperBroker {
  /** Which account's record this is — the registry id, or `login-<n>` for an
   *  executor started outside the registry. */
  account: string
  /** When the executor last looked, epoch ms. The freshness of everything else. */
  at: number
  login: number | null
  server: string | null
  /** False would be a real-money account, which the executor refuses to trade. */
  demo: boolean | null
  currency: string | null
  balance: number | null
  equity: number | null
  margin: number | null
  margin_free: number | null
  /** `null` on a flat account, rather than 0 — which would read as a stop-out. */
  margin_level: number | null
  symbol: string | null
  /**
   * What the TERMINAL says one lot of `symbol` is, for the account this row
   * describes. Not the same field as `PaperRun.contract_size`, which is what
   * the BOOK's market config says — and whether the two agree depends on the
   * account:
   *
   *   funded cent account, `XAUUSD.sc`  — both are 1.0. EQUAL. One lot is one
   *     ounce on each side, so nothing needs scaling between them.
   *   retired standard demo, `XAUUSD`   — the terminal said 100.0 against the
   *     book's 1.0. That is where the 100x came from.
   *
   * This said "the STANDARD symbol's contract size, which is 100x the cent
   * book's", which was true of the demo and is false of the account that holds
   * money. Corrected 2026-09-17 against both accounts' own `broker.json`.
   *
   * Nothing in `ui/src` reads this today. It is documented rather than removed
   * because the next reader to reach for it will be choosing between two
   * fields with one name, which is the trap in
   * `docs/decisions/2026-09-17-unit-carrying.md`.
   */
  contract_size: number | null
  /** How far the terminal's clock runs ahead of UTC, in ms, MEASURED from the
   *  terminal on the poll that wrote this rather than derived from a timezone.
   *  10_800_000 while Vantage is on +3h.
   *
   *  `null` means it could not be measured — no tick, or one too stale to be
   *  an offset — and that is also when the mirror refuses to open, so a null
   *  here is the reason a book is sitting out and not a missing detail. */
  server_offset_ms: number | null
  magic: number | null
  lot_scale: number | null
  /** True while the executor reconciles but sends nothing. */
  dry_run: boolean | null
  bid: number | null
  ask: number | null
  /** What the BOOK wanted when this snapshot was taken, beside what the
   *  account holds — the pair is the point, so that a row can say which of the
   *  two is ahead rather than just "out of sync". */
  book_side: string | null
  book_lots: number | null
  /** Banked on the account in its own currency, and over how many exits — the
   *  counterpart to the paper book's `net_usd` and `trades`. */
  realised: number | null
  closed: number | null
  /** The account's own closed trades, oldest first - what the BROKER did, as
   *  against the paper book's fills, which are the rule executed perfectly at
   *  the bar's price. */
  fills: BrokerFill[]
  position: null | {
    ticket: number | null
    side: string | null
    lots: number | null
    entry_price: number | null
    price_now: number | null
    sl: number | null
    tp: number | null
    /** In the ACCOUNT's currency — the broker's number, not the book's. */
    profit: number | null
    swap: number | null
    opened_at: number | null
  }
  /** Something is wrong and a person has to act — the broker refused the
   *  order, or a desk guard stopped one that was wrong by a factor. */
  blocked: string | null
  /** The mirror is deliberately sitting this trade out. Working as designed,
   *  and kept apart from `blocked` so an alert on one is not an alert on the
   *  other. */
  standing_out: string | null
  /** The account holds the book's SIDE but not its shape — more than one
   *  position on the book, or a volume that is not `book_lots * lot_scale`.
   *  Human-readable, because each one names which.
   *
   *  Reported and deliberately never corrected: trading the account back into
   *  shape would crystallise a result the book never took at a price it never
   *  saw. It clears itself when the book next goes flat.
   *
   *  A third string beside `blocked` and `standing_out`, not a fold into
   *  either: `blocked` is somebody must act now, `standing_out` is the guard
   *  working, and this is neither — wrong in a way nothing will fix on its
   *  own, and not an emergency.
   *
   *  Note before reading `book_lots` against `position.lots`: those two differ
   *  by `lot_scale` BY DESIGN (0.2 on the funded cent account), so they are
   *  not a drift check. This field is the drift check. */
  drift: string | null
}

/**
 * One trade the account actually completed.
 *
 * Deliberately not a `BacktestTrade`. A broker has no stop distance, so it has
 * no R, no MAE and no MFE, and a zero in those fields would read as a
 * measurement rather than as an absence.
 */
export interface BrokerFill {
  direction: 'LONG' | 'SHORT' | null
  entryTime: number | null
  entryPrice: number | null
  exitTime: number | null
  exitPrice: number | null
  lots: number | null
  /** `sl`/`tp` from MT5, or the executor's comment. Empty, never guessed. */
  exitReason: string | null
  /** In the account's currency, commission and swap included. */
  pnl: number | null
}

/**
 * One broker account, as `/api/paper/accounts` reports it: what
 * `config/accounts.toml` says it should be, merged with what is actually
 * running.
 *
 * `runs` is intent and `mirroring` is fact. An account configured but not
 * running comes back with `at: 0` and an empty `mirroring`, which is a state
 * worth showing — it is what a mirror that died overnight looks like.
 */
/** The guard values in force, as numbers a person reads and types. */
export interface GuardValues {
  /** Not editable: the reconciler, the mirror and the desk are all built on
   *  one position per book, so a control for it would be a switch with
   *  nothing behind it. */
  max_concurrent_positions: number
  max_trades_per_day: number
  daily_loss_limit_usd: number
  cooldown_min: number
  max_open_loss_r: number
  max_notional_pct_equity: number
  flat_before_weekend_hhmm: number
  news_flat_before_min: number
  news_flat_after_min: number
  news_min_impact: number
}

/**
 * Which guards the desk has taken over from the config file.
 *
 * Every field optional, so an edit says what it changed and nothing else — a
 * whole-struct payload would silently pin the fields nobody touched to
 * whatever the form happened to be holding.
 */
export type GuardEdit = Partial<Omit<GuardValues, 'max_concurrent_positions'>>

export interface GuardsView {
  /** In force now: the file's values with the desk's edits on top. */
  effective: GuardValues
  /** What `config/default.toml` holds, so "back to the file" needs no memory. */
  configured: GuardValues
  edited: GuardEdit
  /** How many books are running under these right now. */
  guarded_runs: number
}

/**
 * One line of an account's `executor.jsonl`.
 *
 * Deliberately loose. The executor writes a `kind` and whatever that kind
 * needs — `not-adopted` carries a drift and a limit, `clipped` carries two
 * volumes, `refused` carries a retcode — and a type that enumerated every
 * shape would have to be edited before the desk could show a new one. The
 * screen renders `kind` plus whatever fields came with it.
 */
export interface BrokerEvent {
  at: number
  kind: string
  [field: string]: unknown
}

export interface BrokerAccount {
  /** The registry id, or `login-<n>` for an executor started outside it. */
  id: string
  label: string
  login: number
  server: string | null
  /** False when no `[[account]]` block claims this login — someone started an
   *  executor by hand. Shown rather than hidden. */
  configured: boolean
  enabled: boolean
  /** Books the registry says this account should mirror. */
  runs: string[]
  demo: boolean | null
  currency: string | null
  balance: number | null
  equity: number | null
  margin: number | null
  margin_level: number | null
  /** True only when EVERY reporting book here is in dry run. */
  dry_run: boolean
  /**
   * What the registry PERMITS, not what is happening.
   *
   * An account can be `real_money` and flat, or `real_money` and dry. The flag
   * says only that nothing in the configuration stands between this account
   * and an order that costs money — worth saying plainly on a screen where a
   * funded account and a practice one otherwise look identical.
   *
   * It is not the permission itself. `mt5_executor.py` reads the same file and
   * additionally demands `--allow-real` on its own command line, so this
   * cannot be granted from a browser and showing it here grants nothing.
   */
  real_money: boolean
  /** Terminal volume = the book's lots × this, from the registry. */
  lot_scale: number
  /** Newest snapshot across this account's books; 0 when none ever reported. */
  at: number
  /** Books whose executor is actually reporting into this account. */
  mirroring: string[]
  /** How many of those hold a position on the account. */
  positions: number
}

/**
 * One line of a run's `fills.jsonl`, less its `trade` lines: what happened to
 * the run that was not a fill.
 *
 * `kind` is `started`, `gap`, `refused`, `guard_close` or `stopped`, and each
 * carries only the fields its kind writes — hence every extra field optional.
 */
export interface PaperEvent {
  kind: string
  time: number
  /** `refused` and `guard_close`: the guard's own label, e.g. `NEWS_FLAT`. */
  reason?: string
  /** `gap`: bars the feed skipped before this one. */
  missing_bars?: number
  /** `started`: the guard sentence and the news file, as the server describes them. */
  guards?: string | null
  news?: string | null
  /** `stopped`: the book as it was closed. */
  trades?: number
  equity?: number
  net_usd?: number
  trade?: BacktestTrade
}

/**
 * One indicator a paper run's strategy actually reads, as the strategy's own
 * definition declares it — not a guess made from the strategy's name.
 */
export interface PaperIndicator {
  id: string
  /** Instance key, e.g. `ema_21`, `macd_12_26_9`, `keltner_20_10_1.5`. */
  key: string
  params: Record<string, number>
  /**
   * Fully-qualified series keys — the instance key, a dot, then the output
   * name (`ema_21.ema`, `macd_12_26_9.histogram`). `ActiveIndicator.outputs`
   * wants the bare output name instead, because `PriceChart` rebuilds the
   * qualified key itself.
   */
  outputs: string[]
  /** True when the series belongs over the candles; false when it wants a pane. */
  overlay: boolean
}

/** `GET /api/paper/run/{id}` — one run's book, narration and recent bars. */
export interface PaperRunDetail {
  run: PaperRun
  /** The same forming bar as `run.live`, beside the bars the chart draws. */
  live: LiveBar | null
  /** `[epoch ms, equity]`, one point per closed trade plus the opening balance.
   *  Not sorted: warm-up trades close before the run's own start stamp. */
  equity_curve: [number, number][]
  fills: BacktestTrade[]
  events: PaperEvent[]
  /** `[time, open, high, low, close]`, oldest first. Times are **milliseconds**. */
  bars: [number, number, number, number, number][]
  /** The indicators the run's strategy reads, in the order it declares them. */
  indicators: PaperIndicator[]
  /** Qualified output key → points. Times are **seconds**, unlike `bars`. */
  series: Record<string, IndicatorPoint[]>
}

/** One idea, before it is a hypothesis. `docs/research/SCOUTING.md`. */
export interface Proposal {
  id: string
  title: string
  proposed_at: number
  scout: {
    mechanism: string
    why_unarbitraged: string
    data_needed: string[]
    falsifier_sketch: string
    closest_known: string
  }
  verdicts: {
    role: string
    verdict: string
    at: number
    note: string
    evidence: string[]
    instrument: string
    sample: string
  }[]
  /** Derived by the server from the verdicts; no agent can write it. */
  status: 'proposed' | 'shortlisted' | 'registered' | 'rejected'
  registered_as: string | null
  file: string
}

export interface Research {
  updatedAt: number
  hypotheses: ResearchHypothesis[]
  backlog: { open: { title: string; note: string }[]; closed: { title: string; note: string }[] }
  decisions: { file: string; date: string; title: string }[]
  scouting: Proposal[]
}

/** A failed request carries the server's message, not a status code alone. */
export class ApiError extends Error {}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init)
  let body: unknown
  try {
    body = await response.json()
  } catch {
    throw new ApiError(`${path} returned ${response.status} with no JSON body`)
  }
  if (body && typeof body === 'object' && 'error' in body) {
    throw new ApiError(String((body as { error: unknown }).error))
  }
  if (!response.ok) throw new ApiError(`${path} returned ${response.status}`)
  return body as T
}

const post = <T,>(path: string, payload: unknown) =>
  request<T>(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })


/* ------------------------------------------------------------ higher timeframe */

/**
 * The H4/D1 facts the Desk shows as CONTEXT.
 *
 * Every field is always present in the JSON; `null` means the fact could not
 * be computed and is never the same as a zero. `bars_since_new_high: 0` means
 * THIS bar made a new high — a measurement — while `null` means the 20-bar
 * window is not full yet. `ema21_slope_sign: 0` is a genuine flat.
 *
 * Shapes agreed with the route's author before either side was written, and
 * checked against real output rather than inferred from a description — the
 * last time a client assumed a shape from prose it read the wrong hash.
 */
export interface HtfLevel {
  price: number
  bar_ms: number
}

export interface HtfStructure {
  /** Closed set. Switch on it exhaustively. */
  label: 'UP' | 'DOWN' | 'RANGE'
  /** The rule that produced it, e.g. `fractal(2)`. Shown, because a label
   *  without its rule is a stronger claim than the rule supports. */
  rule: string
  /**
   * The bar that CONFIRMED the newest swing — not the bar the swing happened
   * on. A fractal(2) needs two bars after it, so on H4 the label can be up to
   * eight hours older than the facts beside it. The card ages the LABEL from
   * this and the facts from `computed_at_bar_ms`, because they are two
   * different ages and one stamp cannot carry both.
   */
  confirmed_at_bar_ms: number | null
  /** Swing points. Two of these can share a `bar_ms`: one outside bar can be
   *  both a fractal high and a fractal low. Do not assume they are distinct. */
  last_high: HtfLevel | null
  prior_high: HtfLevel | null
  last_low: HtfLevel | null
  prior_low: HtfLevel | null
  /** The price whose break would change the label. `null` on RANGE — a range
   *  has no single such level, and inventing one would claim a precision the
   *  rule does not have. */
  break_level: number | null
  break_side: 'BELOW' | 'ABOVE' | null
}

export interface HtfH4 {
  /** The CLOSED H4 bar these facts describe, UTC epoch ms. */
  computed_at_bar_ms: number
  /** Wall clock when the route computed them, UTC epoch ms. */
  computed_at_ms: number
  timeframe: string
  /** The bar length in ms — read from the object whose stamps are being aged
   *  rather than from a sibling that describes where the bars came from. */
  bar_ms: number
  structure: HtfStructure
  ema21: number | null
  ema55: number | null
  /** -1 | 0 | +1 over the last three closed bars; 0 is a genuine flat. */
  ema21_slope_sign: number | null
  ema55_slope_sign: number | null
  /** Quote units — published so the denominator of `dist_ema21_atr` is
   *  visible. A ratio whose denominator cannot be checked is not checkable. */
  atr14: number | null
  /** Signed, positive when the close is ABOVE the EMA. In ATRs. */
  dist_ema21_atr: number | null
  adx14: number | null
  plus_di14: number | null
  minus_di14: number | null
  /** Kaufman efficiency ratio, 0..1, unitless. */
  efficiency_20: number | null
  donchian20: {
    upper: number | null
    lower: number | null
    bars_since_new_high: number | null
    bars_since_new_low: number | null
  }
  last_close: number | null
}

export interface HtfD1 {
  computed_at_bar_ms: number
  computed_at_ms: number
  timeframe: string
  /** The NOMINAL 86,400,000. The broker's day is not always 24 hours — it
   *  moves at the daylight-saving changeover — so this ages a stamp and must
   *  not be used for arithmetic between two of them. */
  bar_ms: number
  prior_day_high: number | null
  prior_day_low: number | null
  prior_day_bar_ms: number | null
  prior_week_high: number | null
  prior_week_low: number | null
  prior_week_mid: number | null
  prior_week_start_ms: number | null
  /** NOT clamped to 0..100. Above 100 is price out of the prior week's range
   *  upward, below 0 downward, and those are the most informative things it
   *  ever says — so it must never be drawn as a bar that stops at the ends. */
  close_pct_of_prior_week_range: number | null
  last_close: number | null
}

export interface HtfSource {
  file: string
  bars: number
  timeframe: string
}

export interface HtfResponse {
  market: string
  /** `null` only when the stored bars for that timeframe are missing
   *  ENTIRELY. A timeframe present but too short returns the object with its
   *  stamps set and the individual facts null — "no data" and "not enough
   *  yet" are different states and the card must not say one for the other. */
  /**
   * The one-hour read, in exactly the H4 shape.
   *
   * OPTIONAL ON THE WIRE because this client ships before the route that
   * sends it. Absent reads as "this server predates H1" and the card says so,
   * rather than showing an empty row that looks like a missing market.
   */
  h1?: HtfH4 | null
  h4: HtfH4 | null
  d1: HtfD1 | null
  h1_source?: HtfSource | null
  h4_source: HtfSource | null
  d1_source: HtfSource | null
  /**
   * One sentence, written to be displayed, when a timeframe is absent.
   *
   * This cannot say WHICH once there are three timeframes, and the mixed case
   * is the normal one: the daily export can be stale while the hourly is
   * fresh. A single sentence sitting under a row that rendered perfectly is a
   * caption confidently wrong about the thing above it.
   */
  unavailable: string | null
  /**
   * The same sentence, per timeframe, keyed as the route names them (`1h`,
   * `4h`, `1d`). A missing key means that timeframe is fine.
   *
   * Read in preference to `unavailable` and shown against the ROW it belongs
   * to. When it is absent entirely — an older server — the card falls back to
   * the single sentence and puts it under the card rather than under any one
   * row, which is the most it can honestly claim about it.
   */
  unavailable_by_tf?: Record<string, string> | null
}

export const api = {
  catalog: () => request<Catalog>('/api/chart/catalog'),

  bars: (market: string, tf: string, n?: number) =>
    request<BarsResponse>(
      `/api/chart/bars?market=${encodeURIComponent(market)}&tf=${encodeURIComponent(tf)}` +
        (n ? `&n=${n}` : ''),
    ),

  levels: (market: string) =>
    request<{ frame: OptionsFrame | null; frames?: number; live?: boolean }>(
      `/api/chart/levels?market=${encodeURIComponent(market)}`,
    ),

  leaderboard: (market: string, tf: string) =>
    request<{ market: string; timeframe: string; rows: LeaderboardRow[]; note: string }>(
      `/api/chart/leaderboard?market=${encodeURIComponent(market)}&tf=${encodeURIComponent(tf)}`,
    ),

  indicators: (market: string, tf: string, specs: { id: string; params?: Record<string, number> }[]) =>
    post<{ timeframe: string; series: Record<string, IndicatorPoint[]> }>('/api/chart/indicators', {
      market,
      tf,
      specs,
    }),

  backtest: (
    market: string,
    tf: string,
    strategy: string,
    params: Record<string, number>,
    filters: string[] = [],
    guards = false,
    range: { from?: string; to?: string } = {},
  ) => post<BacktestResult>('/api/chart/backtest', { market, tf, strategy, params, filters, guards, from: range.from || undefined, to: range.to || undefined }),

  research: () => request<Research>('/api/research'),
  paperStatus: () => request<{ runs: PaperRun[] }>('/api/paper/status'),

  paperAccounts: () => request<{ accounts: BrokerAccount[] }>('/api/paper/accounts'),

  guards: () => request<GuardsView>('/api/paper/guards'),

  /** Higher-timeframe context for one market. Read-only, and shown as context
   *  rather than as a signal. */
  htf: (market: string) => request<HtfResponse>(`/api/paper/htf?market=${encodeURIComponent(market)}`),

  setGuards: (edit: GuardEdit) => post<GuardsView>('/api/paper/guards', edit),

  paperPause: (run: string, paused: boolean) =>
    post<{ id: string; paused: boolean; holding: boolean }>('/api/paper/pause', { run, paused }),

  /** One run in full. `bars` is how many recent bars to send back with it. */
  paperRun: (id: string, bars = 120) =>
    request<PaperRunDetail>(`/api/paper/run/${encodeURIComponent(id)}?bars=${bars}`),

  /**
   * What ONE account did with one book — a different log from what the book
   * did, and the reason account mode used to show the paper book's events
   * with a label instead of the account's own.
   *
   * The book's events are about the rule: a gap in its feed, a guard firing.
   * These are about the execution: a position not adopted because the price
   * had run, a size clipped to the broker's minimum, an order refused,
   * AutoTrading off. None of it can happen on the paper side.
   */
  brokerEvents: (run: string, account: string, limit = 50) =>
    request<{ run: string; account: string; events: BrokerEvent[] }>(
      `/api/paper/broker-events/${encodeURIComponent(run)}?account=${encodeURIComponent(account)}&limit=${limit}`,
    ),

  /** What the models said about one book. Newest first. */
  paperReasoning: (id: string, limit = 50) =>
    request<Reasoning>(`/api/paper/reasoning/${encodeURIComponent(id)}?limit=${limit}`),

  /**
   * How the advisor pays for a verdict. Masks only — no call here ever returns
   * key material, and the one that creates spending power costs the setup code
   * printed on the fd-api console at startup.
   */
  advisorCredentials: () => request<CredentialsView>('/api/advisor/credentials'),

  /** One real call, to prove a credential. Spends a little, on purpose. */
  advisorTest: (provider: string, model?: string) =>
    post<CredentialProbe>('/api/advisor/credentials/test', { provider, model }),

  advisorStoreKey: (provider: string, key: string, setupCode: string) =>
    post<CredentialWrite>('/api/advisor/credentials', { provider, key, setup_code: setupCode }),

  advisorClearKey: (provider: string) =>
    post<CredentialWrite>('/api/advisor/credentials/clear', { provider }),
}

/** One provider's row. `masked` is the most a credential store may ever say. */
export interface CredentialRow {
  provider: string
  /** `metered` bills per token against a key; `plan` spends a subscription. */
  kind: 'metered' | 'plan'
  source: string
  ready: boolean
  detail: string
  masked: string
  /** False for a plan, and for a provider that declares no config section. */
  can_store: boolean
  /** The command to run in a terminal. A browser cannot complete a sign-in. */
  login_command: string
  env?: string
  env_overrides?: boolean
  verified?: boolean
}

export interface CredentialsView {
  file: string
  gitignored: boolean
  providers: CredentialRow[]
  setup_code_required: boolean
}

export interface CredentialProbe {
  provider: string
  ok: boolean
  detail: string
}

export interface CredentialWrite {
  provider: string
  stored?: boolean
  removed?: boolean
  detail: string
}

/** One decision an outside decider made, as its own log recorded it. */
export interface Decision {
  at: number
  /** The bar it decided on; the fill, if any, was the NEXT bar's open. */
  bar_time: number
  model: string
  side: 'LONG' | 'SHORT' | 'NONE' | string
  reason: string
  /** The reply as received, whole — what the summary can be checked against. */
  response: string
  latency_ms: number
  posted: boolean
  /** Set when the desk refused the decision before it reached a book. */
  refused_locally: string
  dry_run: boolean
  /**
   * Size of the prompt that produced this, in characters. The prompt itself is
   * deliberately NOT sent: ~3.7KB a bar, ninety-six bars a day. It stays in
   * `data/paper/<run>/decisions.jsonl`, which is the replayable record.
   */
  prompt_chars: number
  /**
   * What the call spent, as the provider reported it. `tokens_total` alone
   * where a provider gives only one number; the split where it exists; null
   * where it reported nothing, rather than a zero nobody measured.
   */
  tokens_in: number | null
  tokens_cached: number | null
  tokens_out: number | null
  tokens_total: number | null
  /**
   * Dollars, for a model billed per token. **Null for a subscription** — a plan
   * call is not free, it draws on a quota, and $0.00 beside it would be a lie.
   */
  cost_usd: number | null
}

/** One agent's turn in an advisor consultation. */
export interface Turn {
  agent: string
  model: string
  reason: string
  response: string
  latency_ms: number
  size_factor: number | null
}

/** One consultation of the advisor panel over a pending intent. */
export interface Consultation {
  at: number
  intent_id: string
  applied: boolean
  dry_run: boolean
  size_factor: number | null
  reason: string
  turns: Turn[]
}

/**
 * The two logs, kept apart because they are two different powers over a trade:
 * a decider choosing a side on its OWN book, and the advisor panel refusing or
 * shrinking somebody ELSE's trade.
 */
export interface Reasoning {
  run: string
  decisions: Decision[]
  consultations: Consultation[]
}
