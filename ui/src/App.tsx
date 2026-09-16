import { useCallback, useEffect, useState } from 'react'

import { AppBar } from '@/components/AppBar'
import { ErrorBanner } from '@/components/ErrorBanner'
import { Analytics } from '@/pages/Analytics'
import { Desk } from '@/pages/Desk'
import { Floor } from '@/pages/Floor'
import { Research } from '@/pages/Research'
import { Tape } from '@/pages/Tape'
import { Workbench } from '@/pages/Workbench'
import { Skeleton } from '@/components/ui/skeleton'
import { Toaster } from '@/components/ui/sonner'
import { api, type Catalog } from '@/lib/api'

export type View = 'desk' | 'analytics' | 'workbench' | 'tape' | 'research' | 'floor'

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
 * when nothing is connected.
 */
export type Book = 'paper' | 'account'

const VIEWS: View[] = ['desk', 'analytics', 'workbench', 'tape', 'research', 'floor']

function viewFromHash(): View {
  const hash = window.location.hash.replace('#', '') as View
  return VIEWS.includes(hash) ? hash : 'desk'
}

export default function App() {
  const [view, setView] = useState<View>(viewFromHash)
  const [catalog, setCatalog] = useState<Catalog | null>(null)
  const [market, setMarket] = useState<string>('')
  const [book, setBook] = useState<Book>('paper')
  const [error, setError] = useState<string | null>(null)

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
    <div className="min-h-dvh">
      <AppBar
        view={view}
        onViewChange={changeView}
        markets={catalog?.markets ?? []}
        market={market}
        onMarketChange={setMarket}
        book={book}
        onBookChange={setBook}
      />

      {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}

      {/* The Desk, Analytics and the Floor read no catalog, so none waits on
          one: the default screen must not be held behind a request it does not
          use, and Analytics asks only the paper routes. */}
      {view === 'desk' ? (
        <Desk book={book} />
      ) : view === 'analytics' ? (
        <Analytics />
      ) : view === 'floor' ? (
        <Floor />
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
  )
}
