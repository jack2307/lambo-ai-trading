import { useEffect, useState } from 'react'

import type { Book, View } from '@/App'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { GuardsPanel } from '@/components/GuardsPanel'
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

  const isLive = (a: BrokerAccount) => a.at > 0 && now - a.at < BROKER_STALE_MS
  const live = accounts.filter(isLive)
  const chosen = typeof book === 'number' ? (accounts.find((a) => a.login === book) ?? null) : null

  // Falling back is not cosmetic. If the executors stop while the desk is
  // showing an account, every figure on screen freezes at its last value and
  // goes on looking current; dropping to paper is the only reading that stays
  // true without anyone watching. The same applies to an account that vanishes
  // from the registry while it is selected.
  useEffect(() => {
    if (typeof book !== 'number') return
    const still = accounts.find((a) => a.login === book)
    if (!still || !(still.at > 0 && Date.now() - still.at < BROKER_STALE_MS)) onBookChange('paper')
  }, [book, accounts, onBookChange])
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
        <GuardsPanel />
        <div className="bg-background flex items-center gap-1 rounded-full border p-[3px]" role="group" aria-label="Book">
          <Button
            variant="ghost"
            size="sm"
            aria-pressed={book === 'paper'}
            onClick={() => onBookChange('paper')}
            className={cn(
              'h-6 rounded-full px-3 text-xs font-medium transition-colors',
              book === 'paper' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground',
            )}
          >
            paper
          </Button>

          {/* One account is a button; several is a picker. The common case
              stays a two-way switch you can hit without reading, and the list
              only appears once there is something to choose between. */}
          {accounts.length <= 1 ? (
            <Button
              variant="ghost"
              size="sm"
              aria-pressed={typeof book === 'number'}
              disabled={accounts.length === 0 || !isLive(accounts[0])}
              title={accountHint(accounts[0])}
              onClick={() => accounts[0] && onBookChange(accounts[0].login)}
              className={cn(
                'h-6 rounded-full px-3 text-xs font-medium transition-colors',
                typeof book === 'number' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground',
              )}
            >
              {accounts[0]?.label ?? 'account'}
            </Button>
          ) : (
            <Select
              value={typeof book === 'number' ? String(book) : ''}
              onValueChange={(v) => onBookChange(Number(v))}
            >
              <SelectTrigger
                size="sm"
                className={cn(
                  'h-6 rounded-full border-0 px-3 text-xs font-medium',
                  typeof book === 'number' ? 'bg-accent text-foreground' : 'text-muted-foreground',
                )}
              >
                <SelectValue placeholder="account" />
              </SelectTrigger>
              <SelectContent>
                {accounts.map((a) => (
                  // An account that is defined but not reporting is listed and
                  // not selectable: hiding it would make a mirror that died
                  // look like an account nobody ever set up.
                  <SelectItem key={a.login} value={String(a.login)} disabled={!isLive(a)}>
                    {a.label}
                    {!isLive(a) && ' \u2014 not running'}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>
        <BrokerCaption accounts={accounts} live={live} chosen={chosen} now={now} />
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
/** Why the account button is unavailable, said where the reader's hand is. */
function accountHint(a: BrokerAccount | undefined): string | undefined {
  if (!a) return 'no account is defined - add an [[account]] block to config/accounts.toml'
  if (a.at === 0) return 'this account has never reported - start its mirror with py/live/start_executors.ps1'
  return undefined
}

function BrokerCaption({
  accounts,
  live,
  chosen,
  now,
}: {
  accounts: BrokerAccount[]
  live: BrokerAccount[]
  /** The account currently on screen, when one is. It is described in
   *  preference to any other: a caption about a different account than the one
   *  being read is worse than no caption. */
  chosen: BrokerAccount | null
  now: number
}) {
  if (accounts.length === 0) {
    return (
      <p className="text-muted-foreground text-[11px]" title="Add an [[account]] block to config/accounts.toml">
        paper only &middot; no broker is configured
      </p>
    )
  }
  const reported = accounts.filter((a) => a.at > 0)
  if (live.length === 0 && !chosen) {
    // Configured-and-never-started is a different thing from was-running-and-
    // stopped, and only the second is a problem to go and look at.
    if (reported.length === 0) {
      return (
        <p className="text-muted-foreground text-[11px]" title={accounts.map((a) => a.label).join(', ')}>
          {accounts.length} account{accounts.length === 1 ? '' : 's'} configured &middot; none running
        </p>
      )
    }
    const newest = reported.reduce((a, b) => (a.at > b.at ? a : b))
    const mins = Math.round((now - newest.at) / 60000)
    return (
      <p className="text-lp text-[11px]">
        <span className="num">{newest.login}</span> &middot; mirror stopped{' '}
        {mins < 1 ? 'under a minute' : `${mins} min`} ago
      </p>
    )
  }
  const a = chosen ?? live[0]
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
      {/* An account trading without an entry in config/accounts.toml is the
          exact thing the registry exists to make visible, so it is said on the
          bar rather than left to be noticed. */}
      {!a.configured && (
        <span className="text-caution" title="No [[account]] block claims this login. Add one to config/accounts.toml.">
          {' '}
          &middot; not in the registry
        </span>
      )}
      {!chosen && live.length > 1 && (
        <span className="text-muted-foreground/60"> &middot; +{live.length - 1} more</span>
      )}
      {/* Mirrored against intended. The registry names the books this account
          should carry; this says how many of them are actually reporting, so a
          mirror that quietly failed to start is visible without opening a log. */}
      {chosen && chosen.runs.length > 0 && (
        <span
          className={cn(
            chosen.mirroring.length < chosen.runs.length ? 'text-caution' : 'text-muted-foreground/60',
          )}
        >
          {' '}
          &middot; {chosen.mirroring.length}/{chosen.runs.length} books
        </span>
      )}
    </p>
  )
}
