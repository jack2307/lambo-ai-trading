import { useCallback, useEffect, useRef, useState } from 'react'

import { AppBar, BROKER_STALE_MS } from '@/components/AppBar'
import { Sidebar, readRail } from '@/components/Sidebar'
import { ErrorBanner } from '@/components/ErrorBanner'
import { Analytics } from '@/pages/Analytics'
import { Desk } from '@/pages/Desk'
import { Floor } from '@/pages/Floor'
import { Research } from '@/pages/Research'
import { Settings } from '@/pages/Settings'
import { Tape } from '@/pages/Tape'
import { Workbench } from '@/pages/Workbench'
import { Skeleton } from '@/components/ui/skeleton'
import { Toaster } from '@/components/ui/sonner'
import { api, type BrokerAccount, type Catalog } from '@/lib/api'
import { useTicks } from '@/lib/ticks'

export type View = 'desk' | 'analytics' | 'workbench' | 'tape' | 'research' | 'floor' | 'settings'

/**
 * Which side of the desk is on screen: what the strategies DECIDED, or what a
 * broker account actually did with those decisions.
 *
 * They are two different books and the difference is the measurement. The
 * paper book is the rule executed perfectly at the bar's price; the account is
 * the same rule after a spread, a slip and a fill. Showing them in one merged
 * view would make that difference unreadable, which is why this is a switch
 * and not a column.
 *
 * `paper` is the default and the fallback: it is the only one that is true
 * when nothing is connected. Any other value is an account's LOGIN - the one
 * identifier that is the same in the registry, in the snapshot and on the
 * broker's own screen, so the desk and the terminal can never be talking
 * about different accounts while agreeing on a name.
 */
export type Book = 'paper' | number

const VIEWS: View[] = ['desk', 'analytics', 'workbench', 'tape', 'research', 'floor', 'settings']

/**
 * The book on screen, remembered per viewer.
 *
 * A reload used to come back on paper however the desk was left, which on a
 * funded account means the first frame after every refresh is the wrong book -
 * the owner reloads, sees the paper figures, and has to re-pick his own
 * account to find out what his money is doing.
 *
 * Stored as `paper` or the account's LOGIN, which is the identifier the
 * registry, the snapshot and the broker's own screen all agree on. A remembered
 * login that no longer answers is NOT restored - see `AppBar`, where an account
 * that has stopped reporting drops back to paper on purpose, because a frozen
 * account view goes on looking current.
 */
const BOOK_KEY = 'fd.desk.book'

const readBook = (): Book | null => {
  try {
    const raw = localStorage.getItem(BOOK_KEY)
    if (!raw) return null
    if (raw === 'paper') return 'paper'
    const login = Number(raw)
    return Number.isFinite(login) && login > 0 ? login : null
  } catch {
    return null
  }
}

const writeBook = (book: Book) => {
  try {
    localStorage.setItem(BOOK_KEY, String(book))
  } catch {
    /* private mode: the choice lasts the page */
  }
}

function viewFromHash(): View {
  const hash = window.location.hash.replace('#', '') as View
  return VIEWS.includes(hash) ? hash : 'desk'
}

export default function App() {
  const [view, setView] = useState<View>(viewFromHash)
  const [catalog, setCatalog] = useState<Catalog | null>(null)
  const [market, setMarket] = useState<string>('')
  const [book, setBook] = useState<Book>(() => readBook() ?? 'paper')
  // Whether the viewer has ever chosen. Read once, before the first render can
  // change it: if nothing is remembered, the desk picks the real-money account
  // for them (see `AppBar`), and that must not fight a choice they already made.
  const [autoPick] = useState(() => readBook() === null)
  const chooseBook = useCallback((next: Book) => {
    setBook(next)
    writeBook(next)
  }, [])
  const [error, setError] = useState<string | null>(null)
  // ONE tick source for the whole app.
  //
  // `useTicks` opens an EventSource, so calling it in two components would open
  // two connections to `/api/paper/stream` and give the strip and the chart two
  // answers a fraction of a second apart. Same rule as the accounts poll: one
  // subscription, one answer, handed down.
  const { ticks, connected: streaming } = useTicks()
  const [railed, setRailed] = useState<boolean>(readRail)
  const [drawerOpen, setDrawerOpen] = useState(false)

  // The registry, polled here rather than in the AppBar, because two chrome
  // components now read it: the sidebar's book card and the strip's equity
  // readout. One poll, one answer; two would be two answers a second apart.
  const [accounts, setAccounts] = useState<BrokerAccount[]>([])
  // Whether it has answered at ALL. An empty list before the first response is
  // not an empty registry, and both effects below would read it as one.
  const [accountsLoaded, setAccountsLoaded] = useState(false)
  const picked = useRef(false)

  useEffect(() => {
    let alive = true
    const tick = () => {
      api
        .paperAccounts()
        .then((res) => {
          if (!alive) return
          setAccounts(res.accounts)
          setAccountsLoaded(true)
        })
        // A failure here means no broker, which is what an empty list says.
        .catch(() => {
          if (!alive) return
          setAccounts([])
          setAccountsLoaded(true)
        })
    }
    tick()
    const timer = window.setInterval(tick, 5000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  const isLive = useCallback(
    (a: BrokerAccount) => a.at > 0 && Date.now() - a.at < BROKER_STALE_MS,
    [],
  )

  // Falling back is not cosmetic. If the executors stop while the desk shows an
  // account, every figure freezes at its last value and goes on looking
  // current; dropping to paper is the only reading that stays true unwatched.
  useEffect(() => {
    if (!accountsLoaded || typeof book !== 'number') return
    const still = accounts.find((a) => a.login === book)
    if (!still || !isLive(still)) setBook('paper')
  }, [book, accounts, accountsLoaded, isLive])

  // THE DEFAULT FOLLOWS THE MONEY. See the note on `autoPick`: with nothing
  // remembered the desk opens on the real-money account, and only when there
  // is exactly one enabled and reporting. Once, ever.
  useEffect(() => {
    if (!autoPick || picked.current || !accountsLoaded || book !== 'paper') return
    const real = accounts.filter((a) => a.real_money && a.enabled && isLive(a))
    if (real.length !== 1) return
    picked.current = true
    chooseBook(real[0].login)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoPick, accountsLoaded, accounts, book, isLive])

  const railTo = useCallback((next: boolean) => {
    setRailed(next)
    try {
      localStorage.setItem('fd.desk.sidebar', next ? '1' : '0')
    } catch {
      /* private mode: the choice lasts the page */
    }
  }, [])

  useEffect(() => {
    const onHashChange = () => setView(viewFromHash())
    window.addEventListener('hashchange', onHashChange)
    return () => window.removeEventListener('hashchange', onHashChange)
  }, [])

  useEffect(() => {
    api
      .catalog()
      .then((loaded) => {
        setCatalog(loaded)
        // A market with no cached tape is listed but never auto-selected: an
        // empty chart with no explanation is worse than a disabled option.
        const usable =
          loaded.markets.find((m) => m.id === loaded.activeMarket && m.hasData) ??
          loaded.markets.find((m) => m.hasData)
        setMarket(usable?.id ?? loaded.activeMarket)
      })
      .catch((err: Error) => setError(err.message))
  }, [])

  const changeView = useCallback((next: View) => {
    window.location.hash = next
    setView(next)
  }, [])

  return (
    <div className="flex min-h-dvh">
      <Sidebar
        view={view}
        onViewChange={changeView}
        book={book}
        onBookChange={chooseBook}
        accounts={accounts}
        railed={railed}
        onRailedChange={railTo}
        drawerOpen={drawerOpen}
        onDrawerOpenChange={setDrawerOpen}
        isLive={isLive}
      />

      <div className="flex min-w-0 flex-1 flex-col">
      <AppBar
        view={view}
        markets={catalog?.markets ?? []}
        market={market}
        onMarketChange={setMarket}
        book={book}
        accounts={accounts}
        isLive={isLive}
        ticks={ticks}
        streaming={streaming}
        onOpenMenu={() => setDrawerOpen(true)}
      />

      {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}

      {/* The Desk, Analytics and the Floor read no catalog, so none waits on
          one: the default screen must not be held behind a request it does not
          use, and Analytics asks only the paper routes. */}
      {view === 'desk' ? (
        <Desk book={book} ticks={ticks} streaming={streaming} />
      ) : view === 'analytics' ? (
        <Analytics />
      ) : view === 'floor' ? (
        <Floor />
      ) : view === 'settings' ? (
        <Settings />
      ) : !catalog ? (
        <div className="space-y-3 p-4">
          <Skeleton className="h-9 w-full" />
          <Skeleton className="h-[60vh] w-full" />
        </div>
      ) : view === 'research' ? (
        <Research catalog={catalog} market={market} onError={setError} />
      ) : view === 'workbench' ? (
        <Workbench catalog={catalog} market={market} onError={setError} />
      ) : (
        <Tape market={market} info={catalog.markets.find((m) => m.id === market)} onError={setError} />
      )}

      <Toaster position="bottom-right" />
      </div>
    </div>
  )
}
