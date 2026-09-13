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
  mae: number
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

export interface Research {
  updatedAt: number
  hypotheses: ResearchHypothesis[]
  backlog: { open: { title: string; note: string }[]; closed: { title: string; note: string }[] }
  decisions: { file: string; date: string; title: string }[]
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
}
