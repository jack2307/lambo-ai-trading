import { Button } from '@/components/ui/button'

interface Props {
  message: string
  onDismiss: () => void
}

/**
 * Failures surface here rather than in a console or a swallowed catch. A
 * dashboard that silently keeps showing stale numbers after a request failed is
 * worse than one that says so.
 */
export function ErrorBanner({ message, onDismiss }: Props) {
  return (
    <div
      role="alert"
      className="border-bear/40 bg-bear/10 mx-4 mt-3 flex items-start gap-3 rounded-md border px-4 py-2 text-sm"
    >
      <span className="flex-1">{message}</span>
      <Button variant="ghost" size="sm" className="h-6 px-2 text-xs" onClick={onDismiss}>
        Dismiss
      </Button>
    </div>
  )
}
