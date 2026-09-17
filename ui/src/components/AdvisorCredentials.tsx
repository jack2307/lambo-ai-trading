/**
 * Advisor credentials — the two ways to pay for a verdict, on one panel.
 *
 * A metered provider bills per token against an API key. A plan provider
 * spends a subscription through a vendor CLI that is already signed in. The
 * advisor routes to one or the other from the model's name, so what is
 * configured here is the credential and never the routing.
 *
 * TWO THINGS THIS PANEL DELIBERATELY WILL NOT DO
 *
 * It will not sign a CLI in. A sign-in opens a browser and asks for a code to
 * be pasted back into a terminal, and no button on a web page can complete
 * that — it can only start a process that then hangs on a prompt nobody can
 * see. So the command is shown with a copy button, and the panel reads the
 * result back from the CLI afterwards.
 *
 * And it will not store a key for free. `Settings.tsx` states the rule: this
 * screen is served over plain HTTP on a loopback port with no authentication,
 * so anything it can switch on, anything reaching that port can switch on. An
 * API key is spending power. Storing one therefore costs the six-digit setup
 * code printed on the fd-api console at startup — reading it means standing
 * where the server runs. Removing a key costs nothing, because taking
 * capability away is the safe direction this screen already moves in freely.
 *
 * Nothing here ever displays key material. A stored key is shown masked
 * because that is all the API will say about it.
 */

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'

import { Panel, SectionHeader } from '@/components/dashboard/primitives'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { api, type CredentialRow, type CredentialsView } from '@/lib/api'
import { cn } from '@/lib/utils'

function Row({
  row,
  busy,
  onTest,
  onStore,
  onClear,
}: {
  row: CredentialRow
  busy: string | null
  onTest: (p: string) => void
  onStore: (p: string, key: string, code: string) => Promise<boolean>
  onClear: (p: string) => void
}) {
  const [open, setOpen] = useState(false)
  const [key, setKey] = useState('')
  const [code, setCode] = useState('')
  const working = busy === row.provider

  return (
    <div className="border-border/60 flex flex-col gap-2 border-b py-2.5 last:border-b-0">
      <div className="flex items-baseline gap-2">
        <span className="text-[12px] font-medium">{row.provider}</span>
        <span className="text-muted-foreground text-[10px] uppercase tracking-wide">{row.kind}</span>
        <span
          className={cn(
            'num rounded-sm px-1.5 py-0.5 text-[10px]',
            row.ready ? 'text-positive bg-positive/10' : 'text-muted-foreground bg-muted/40',
          )}
        >
          {row.ready ? 'ready' : 'not set'}
        </span>
        <span className="text-muted-foreground/70 ml-auto text-[10px]">{row.source}</span>
      </div>

      <p className="text-muted-foreground/80 text-[11px] leading-snug">{row.detail}</p>

      {row.env_overrides && (
        <p className="text-caution text-[10px] leading-snug">
          {row.env} is set in this process's environment and wins over anything stored in the file.
        </p>
      )}

      <div className="flex flex-wrap items-center gap-1.5">
        <Button size="sm" variant="outline" disabled={!row.ready || working} onClick={() => onTest(row.provider)}>
          {working ? 'calling…' : 'Test'}
        </Button>

        {row.kind === 'metered' && row.can_store && (
          <Button size="sm" variant="outline" onClick={() => setOpen((v) => !v)}>
            {open ? 'Cancel' : row.ready ? 'Replace key' : 'Enter key'}
          </Button>
        )}

        {row.kind === 'metered' && row.ready && (
          <Button size="sm" variant="ghost" disabled={working} onClick={() => onClear(row.provider)}>
            Remove
          </Button>
        )}

        {row.kind === 'plan' && row.login_command && (
          <>
            <code className="border-border bg-muted/40 rounded-sm border px-1.5 py-0.5 text-[11px]">
              {row.login_command}
            </code>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                void navigator.clipboard.writeText(row.login_command)
                toast.success('command copied — run it in a terminal')
              }}
            >
              Copy
            </Button>
          </>
        )}
      </div>

      {row.kind === 'plan' && row.verified === false && (
        <p className="text-muted-foreground/60 text-[10px] leading-snug">
          This repository has never completed a sign-in for this tool; the command comes from its
          documentation, not from a run here.
        </p>
      )}

      {open && (
        <form
          className="border-border/60 mt-1 flex flex-col gap-2 rounded-sm border p-2"
          onSubmit={(e) => {
            e.preventDefault()
            void onStore(row.provider, key, code).then((ok) => {
              if (ok) {
                setKey('')
                setCode('')
                setOpen(false)
              }
            })
          }}
        >
          <p className="text-muted-foreground/80 text-[10px] leading-snug">
            The key is proven with one real call before it is written, and refused if it does not
            answer — a stored key that fails would drop this agent from the panel with one line in a
            log. It is sent once and never read back.
          </p>
          <label className="flex flex-col gap-1">
            <span className="text-[11px] font-medium">API key</span>
            <input
              type="password"
              autoComplete="off"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={`the ${row.provider} key`}
              className="border-border bg-background h-7 rounded-sm border px-2 text-[12px]"
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-[11px] font-medium">Setup code</span>
            <input
              inputMode="numeric"
              autoComplete="off"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              placeholder="six digits"
              className="border-border bg-background num h-7 w-32 rounded-sm border px-2 text-[12px]"
            />
            <span className="text-muted-foreground/70 text-[10px] leading-snug">
              Printed on the fd-api console when it started. This port has no authentication, so the
              one control that can create spending power asks for something only the operator sees.
            </span>
          </label>
          <Button size="sm" type="submit" disabled={working || !key.trim() || !code.trim()}>
            {working ? 'proving…' : 'Prove and store'}
          </Button>
        </form>
      )}
    </div>
  )
}

export function AdvisorCredentials() {
  const [view, setView] = useState<CredentialsView | null>(null)
  const [off, setOff] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  const load = useCallback(() => {
    api
      .advisorCredentials()
      .then((v) => {
        setView(v)
        setOff(null)
      })
      // A failure here is shown IN the panel and not only as a toast. The
      // route group is off by default on a live desk, and a panel that
      // answered that with a toast and then an endless skeleton would be the
      // same half-working shape this feature was switched off for: the reader
      // sees a loading state for a thing that is never going to load.
      .catch((e: Error) => setOff(e.message))
  }, [])

  useEffect(load, [load])

  const onTest = useCallback(
    (provider: string) => {
      setBusy(provider)
      api
        .advisorTest(provider)
        .then((r) => (r.ok ? toast.success(`${provider}: ${r.detail}`) : toast.error(`${provider}: ${r.detail}`)))
        .catch((e: Error) => toast.error(e.message))
        .finally(() => setBusy(null))
    },
    [],
  )

  const onStore = useCallback(
    async (provider: string, key: string, code: string) => {
      setBusy(provider)
      try {
        const r = await api.advisorStoreKey(provider, key, code)
        if (r.stored) toast.success(`${provider}: ${r.detail}`)
        else toast.error(`${provider}: ${r.detail}`)
        load()
        return Boolean(r.stored)
      } catch (e) {
        toast.error((e as Error).message)
        return false
      } finally {
        setBusy(null)
      }
    },
    [load],
  )

  const onClear = useCallback(
    (provider: string) => {
      setBusy(provider)
      api
        .advisorClearKey(provider)
        .then((r) => toast.success(`${provider}: ${r.detail}`))
        .catch((e: Error) => toast.error(e.message))
        .finally(() => {
          setBusy(null)
          load()
        })
    },
    [load],
  )

  return (
    <Panel className="flex flex-col gap-1 p-3">
      <SectionHeader
        title="Advisor credentials"
        subtitle="How a verdict is paid for: a metered key, or a plan through a signed-in CLI"
        action={
          <Button size="sm" variant="ghost" onClick={load}>
            Refresh
          </Button>
        }
      />

      {off ? (
        <p className="text-muted-foreground border-border mt-1 rounded-sm border px-2 py-1.5 text-[11px] leading-snug">
          {off}
        </p>
      ) : !view ? (
        <Skeleton className="h-24 w-full" />
      ) : (
        <>
          {!view.gitignored && (
            <p className="text-negative border-negative/40 mt-1 rounded-sm border px-2 py-1.5 text-[11px] leading-snug">
              {view.file} is not in .gitignore. Storing a key is refused until it is — a credential
              that a commit can carry is a credential that has left the machine.
            </p>
          )}
          <div className="mt-1 flex flex-col">
            {view.providers.map((p) => (
              <Row
                key={p.provider}
                row={p}
                busy={busy}
                onTest={onTest}
                onStore={onStore}
                onClear={onClear}
              />
            ))}
          </div>
          <p className="text-muted-foreground/60 mt-2 text-[10px] leading-snug">
            A plan is not billed per token, but it is billed: measured on 2026-09-17, the same
            trivial question cost 40 input tokens through the metered API and 23,148 through the CLI,
            because every CLI call reloads a whole system context. Use the plan to investigate and a
            key for a loop that asks on every bar.
          </p>
        </>
      )}
    </Panel>
  )
}
