import { useCallback, useEffect, useState } from 'react'

import { AppBar } from '@/components/AppBar'
import { ErrorBanner } from '@/components/ErrorBanner'
import { Dashboard } from '@/pages/Dashboard'
import { Tape } from '@/pages/Tape'
import { Workbench } from '@/pages/Workbench'
import { Skeleton } from '@/components/ui/skeleton'
import { Toaster } from '@/components/ui/sonner'
import { api, type Catalog } from '@/lib/api'

export type View = 'dashboard' | 'tape' | 'workbench'

function viewFromHash(): View {
  const hash = window.location.hash.replace('#', '')
  if (hash === 'tape' || hash === 'workbench') return hash
  return 'dashboard'
}

export default function App() {
  const [view, setView] = useState<View>(viewFromHash)
  const [catalog, setCatalog] = useState<Catalog | null>(null)
  const [market, setMarket] = useState<string>('')
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
      />

      {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}

      {!catalog ? (
        <div className="space-y-3 p-4">
          <Skeleton className="h-9 w-full" />
          <Skeleton className="h-[60vh] w-full" />
        </div>
      ) : view === 'dashboard' ? (
        <Dashboard catalog={catalog} market={market} onError={setError} />
      ) : view === 'workbench' ? (
        <Workbench catalog={catalog} market={market} onError={setError} />
      ) : (
        <Tape market={market} onError={setError} />
      )}

      <Toaster position="bottom-right" />
    </div>
  )
}
