import { useEffect, useState } from 'react'

import type { Book, View } from '@/App'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { api, type BrokerAccount, type MarketInfo } from '@/lib/api'
import { PRODUCT } from '@/lib/brand'
import { cn } from '@/lib/utils'

interface Props {
  view: View
  onViewChange: (view: View) => void
  markets: MarketInfo[]
  market: string
  onMarketChange: (market: string) => void
  book: Book
  onBookChange: (book: Book) => void
}

/**
 * How long an executor's snapshot stays believable.
 *
 * The executor polls every 15 s, so three missed looks is a stopped process
 * rather than a slow one. This matters more than it sounds: the snapshot is a
 * FILE, and a file does not disappear when the program that wrote it dies. An
 * account that is simply remembered must never be allowed to look connected,
 * because every number it carries would then be a past number presented as a
 * present one.
 */
const BROKER_STALE_MS = 45_000

/**
 * The order is the working day: the book first, then what the book has added
 * up to, then the bench you change it on, then the tape, then what the loop is
 * proving, then the floor it happens on. Desk is the default.
 */
const VIEWS: { id: View; label: string }[] = [
  { id: 'desk', label: 'Desk' },
  { id: 'analytics', label: 'Analytics' },
  { id: 'workbench', label: 'Workbench' },
  { id: 'tape', label: 'Tape' },
  { id: 'research', label: 'Research' },
  { id: 'floor', label: 'Floor' },
]

/** The two screens that read one market at a time; the rest ignore the picker. */
const MARKET_VIEWS: View[] = ['workbench', 'tape']

export function AppBar({ view, onViewChange, markets, market, onMarketChange, book, onBookChange }: Props) {
  const needsMarket = MARKET_VIEWS.includes(view)
  const [accounts, setAccounts] = useState<BrokerAccount[]>([])
  const [now, setNow] = useState(() => Date.now())

  // Its own poll rather than a share of the Desk's: this control is on every
  // screen, including the ones that never load a book, and an account summary
  // is a few hundred bytes.
  useEffect(() => {
    let alive = true
    const tick = () => {
      setNow(Date.now())
      api
        .paperAccounts()
        .then((res) => alive && setAccounts(res.accounts))
        // A failure here means no broker, which is exactly what an empty list
        // says. Nothing is thrown at the user for it.
        .catch(() => alive && setAccounts([]))
    }
    tick()
    const timer = window.setInterval(tick, 5000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  const live = accounts.filter((a) => now - a.at < BROKER_STALE_MS)
  const connected = live.length > 0

  // Falling back is not cosmetic. If the executors stop while the desk is
  // showing the account, every figure on screen freezes at its last value and
  // goes on looking current; dropping to paper is the only reading that stays
  // true without anyone watching.
  useEffect(() => {
    if (book === 'account' && !connected) onBookChange('paper')
  }, [book, connected, onBookChange])
  return (
    <header className="bg-card/80 sticky top-0 z-20 flex flex-wrap items-center gap-4 border-b px-4 py-2 backdrop-blur">
      <div className="flex items-baseline gap-2 text-[15px] font-semibold tracking-tight">
        <span className="bg-primary size-[7px] rounded-[2px]" aria-hidden />
        {PRODUCT}
      </div>

      <nav className="bg-background flex gap-1 rounded-full border p-[3px]" aria-label="Views">
        {VIEWS.map((entry) => (
          <Button
            key={entry.id}
            variant="ghost"
            size="sm"
            aria-current={view === entry.id ? 'page' : undefined}
            onClick={() => onViewChange(entry.id)}
            className={cn(
              'h-6 rounded-full px-3 text-xs font-medium transition-colors',
              view === entry.id ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground',
            )}
          >
            {entry.label}
          </Button>
        ))}
      </nav>

      {/* Hidden rather than disabled on the screens that read every market at
          once: a control that cannot change anything is noise, and the Desk's
          own rows already say which market each run trades. */}
      {needsMarket && (
        <label className="text-muted-foreground flex items-center gap-2 text-xs">
          market
          <Select value={market} onValueChange={onMarketChange}>
            <SelectTrigger size="sm" className="h-7 w-[150px] text-xs">
              <SelectValue placeholder="loading…" />
            </SelectTrigger>
            <SelectContent>
              {markets.map((entry) => (
                <SelectItem key={entry.id} value={entry.id} disabled={!entry.hasData}>
                  {entry.id}
                  {entry.hasData ? '' : ' — no data'}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
      )}

      <div className="ml-auto flex items-center gap-2">
        <div className="bg-background flex gap-1 rounded-full border p-[3px]" role="group" aria-label="Book">
          {(['paper', 'account'] as Book[]).map((id) => (
            <Button
              key={id}
              variant="ghost"
              size="sm"
              aria-pressed={book === id}
              disabled={id === 'account' && !connected}
              title={
                id === 'account' && !connected
                  ? 'no executor is reporting an account; start one with py/live/start_executors.ps1'
                  : undefined
              }
              onClick={() => onBookChange(id)}
              className={cn(
                'h-6 rounded-full px-3 text-xs font-medium transition-colors',
                book === id ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground',
              )}
            >
              {id}
            </Button>
          ))}
        </div>
        <BrokerCaption accounts={accounts} live={live} now={now} />
      </div>
    </header>
  )
}

/**
 * What the broker side is doing, in one line.
 *
 * Three states and they are deliberately not collapsed into two. "No broker"
 * and "a broker that has stopped answering" look identical on a toggle and are
 * opposite in meaning: the first is a desk nobody connected, the second is a
 * desk that WAS connected and whose mirror has died, possibly holding a
 * position nobody is now reconciling. The second one names how long it has
 * been quiet, because that is the number you act on.
 */
function BrokerCaption({
  accounts,
  live,
  now,
}: {
  accounts: BrokerAccount[]
  live: BrokerAccount[]
  now: number
}) {
  if (accounts.length === 0) {
    return <p className="text-muted-foreground text-[11px]">paper only &middot; no broker is connected</p>
  }
  if (live.length === 0) {
    const newest = accounts.reduce((a, b) => (a.at > b.at ? a : b))
    const mins = Math.round((now - newest.at) / 60000)
    return (
      <p className="text-lp text-[11px]">
        <span className="num">{newest.login}</span> &middot; mirror stopped{' '}
        {mins < 1 ? 'under a minute' : `${mins} min`} ago
      </p>
    )
  }
  const a = live[0]
  const money =
    a.equity == null
      ? null
      : `${a.equity.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${a.currency ?? ''}`.trim()
  return (
    <p className="text-muted-foreground text-[11px]">
      {/* "demo" is stated, not assumed. The executor refuses a real-money
          account outright, so anything else here would be a contradiction
          worth seeing rather than hiding. */}
      <span className={a.demo ? 'text-muted-foreground' : 'text-lp font-medium'}>
        {a.demo ? 'demo' : 'REAL'}
      </span>{' '}
      <span className="num">{a.login}</span>
      {a.server && <span className="text-muted-foreground/60"> &middot; {a.server}</span>}
      {money && <span className="num"> &middot; {money}</span>}
      {a.dry_run && <span className="text-muted-foreground/60"> &middot; dry run, nothing is sent</span>}
      {live.length > 1 && <span className="text-muted-foreground/60"> &middot; +{live.length - 1} more</span>}
    </p>
  )
}
