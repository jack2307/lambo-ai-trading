import type { View } from '@/App'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { MarketInfo } from '@/lib/api'
import { PRODUCT } from '@/lib/brand'
import { cn } from '@/lib/utils'

interface Props {
  view: View
  onViewChange: (view: View) => void
  markets: MarketInfo[]
  market: string
  onMarketChange: (market: string) => void
}

/**
 * The order is the working day: the book first, then the bench you change it
 * on, then the tape, then what the loop is proving, then the floor it happens
 * on. Desk is the default.
 */
const VIEWS: { id: View; label: string }[] = [
  { id: 'desk', label: 'Desk' },
  { id: 'workbench', label: 'Workbench' },
  { id: 'tape', label: 'Tape' },
  { id: 'research', label: 'Research' },
  { id: 'floor', label: 'Floor' },
]

/** The two screens that read one market at a time; the rest ignore the picker. */
const MARKET_VIEWS: View[] = ['workbench', 'tape']

export function AppBar({ view, onViewChange, markets, market, onMarketChange }: Props) {
  const needsMarket = MARKET_VIEWS.includes(view)
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

      <p className="text-muted-foreground ml-auto text-[11px]">
        paper only · no broker is connected
      </p>
    </header>
  )
}
