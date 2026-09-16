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
    entry_time: number
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
  /** On `/status` these two are *counts*, kept light; the detail route carries the rows. */
  equity_curve?: number
  events?: number
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

export const api = {
  catalog: () => request<Catalog>('/api/chart/catalog'),

  bars: (market: string, tf: string) =>
    request<BarsResponse>(`/api/chart/bars?market=${encodeURIComponent(market)}&tf=${encodeURIComponent(tf)}`),

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

  /** One run in full. `bars` is how many recent bars to send back with it. */
  paperRun: (id: string, bars = 120) =>
    request<PaperRunDetail>(`/api/paper/run/${encodeURIComponent(id)}?bars=${bars}`),

  /** What the models said about one book. Newest first. */
  paperReasoning: (id: string, limit = 50) =>
    request<Reasoning>(`/api/paper/reasoning/${encodeURIComponent(id)}?limit=${limit}`),
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
