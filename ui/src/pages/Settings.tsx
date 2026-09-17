/**
 * Settings — the control room.
 *
 * One screen that answers "what is configured, what is running, and what can I
 * switch off from here", which until now was spread across a config file, a
 * launcher's command line, and whatever the Desk happened to show.
 *
 * WHAT THIS SCREEN WILL NOT DO, AND WHY
 *
 * It only ever moves things in the SAFE direction. It can stop a book; it
 * cannot grant an account permission to spend money, turn dry run off, or
 * raise a lot scale. Those three live in `config/accounts.toml` AND on the
 * launcher's command line, and both have to agree before a real order leaves
 * the machine.
 *
 * That is not timidity, it is the same rule the rest of the desk is built on:
 * a file can only ever make a run safer by itself, and making it riskier costs
 * a word typed by someone who is awake. This page is served over plain HTTP on
 * a loopback port with no authentication of any kind. Anything it could switch
 * on, anything that reached that port could switch on.
 *
 * So the riskier controls are shown as facts with the reason next to them,
 * never as inputs. A reader who wants to change one edits the file and
 * restarts the launcher, which is a deliberate act and leaves a record.
 */

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'

import { AdvisorCredentials } from '@/components/AdvisorCredentials'
import { GuardsPanel } from '@/components/GuardsPanel'
import { Panel, SectionHeader } from '@/components/dashboard/primitives'
import { Skeleton } from '@/components/ui/skeleton'
import { api, type BrokerAccount, type PaperRun } from '@/lib/api'
import { cn } from '@/lib/utils'

/** How old an account's newest snapshot may be and still read as connected. */
const ACCOUNT_STALE_MS = 45_000

function ago(at: number, now: number): string {
  if (!at) return 'never'
  const s = Math.max(0, Math.round((now - at) / 1000))
  if (s < 60) return `${s}s ago`
  if (s < 3600) return `${Math.round(s / 60)}m ago`
  return `${Math.round(s / 3600)}h ago`
}

export function Settings() {
  const [accounts, setAccounts] = useState<BrokerAccount[] | null>(null)
  const [runs, setRuns] = useState<PaperRun[] | null>(null)
  const [now, setNow] = useState(() => Date.now())
  const [busy, setBusy] = useState<string | null>(null)

  const load = useCallback(() => {
    api
      .paperAccounts()
      .then((r) => setAccounts(r.accounts))
      .catch((e: Error) => toast.error(e.message))
    api
      .paperStatus()
      .then((r) => setRuns(r.runs))
      .catch((e: Error) => toast.error(e.message))
    setNow(Date.now())
  }, [])

  useEffect(() => {
    load()
    const t = setInterval(load, 10_000)
    return () => clearInterval(t)
  }, [load])

  const toggle = useCallback(
    (id: string, paused: boolean) => {
      setBusy(id)
      // Optimistic, then corrected by the reply. The switch has to answer the
      // click immediately or a reader clicks it twice, and a book toggled
      // twice is a book that ends up where it started while looking broken.
      setRuns((prev) => prev?.map((r) => (r.id === id ? { ...r, paused } : r)) ?? prev)
      api
        .paperPause(id, paused)
        .then((r) => {
          setRuns((prev) => prev?.map((x) => (x.id === r.id ? { ...x, paused: r.paused } : x)) ?? prev)
          toast.success(
            r.paused
              ? r.holding
                ? `${r.id} off — it takes no new position; the one it holds is still managed to its stop`
                : `${r.id} off`
              : `${r.id} on`,
          )
        })
        .catch((e: Error) => {
          toast.error(e.message)
          load()
        })
        .finally(() => setBusy(null))
    },
    [load],
  )

  if (!accounts || !runs) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    )
  }

  const real = accounts.filter((a) => a.real_money || a.demo === false)
  const rest = accounts.filter((a) => !real.includes(a))

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-4">
      <header>
        <h1 className="text-lg font-semibold">Settings</h1>
        <p className="text-muted-foreground mt-1 text-xs">
          What is configured, what is running, and what can be switched off from here.
        </p>
      </header>

      {real.length > 0 && (
        <section className="space-y-2">
          <SectionHeader title="Real money" subtitle="Accounts the registry permits to send orders that cost money." />
          <div className="space-y-2">
            {real.map((a) => (
              <AccountCard key={a.login} account={a} now={now} />
            ))}
          </div>
        </section>
      )}

      <section className="space-y-2">
        <SectionHeader title="Practice accounts" subtitle="Demo, and anything reporting without a registry entry." />
        {rest.length === 0 ? (
          <Panel className="text-muted-foreground p-3 text-xs">None.</Panel>
        ) : (
          <div className="space-y-2">
            {rest.map((a) => (
              <AccountCard key={a.login} account={a} now={now} />
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <SectionHeader
          title="Books"
          subtitle="Off means the book takes no NEW position. A position it already holds is still managed to its stop and target — off is not abandoned."
        />
        <Panel className="divide-border divide-y">
          {runs.map((run) => (
            <BookRow key={run.id} run={run} accounts={accounts} busy={busy === run.id} onToggle={toggle} />
          ))}
        </Panel>
      </section>

      <section className="space-y-2">
        <SectionHeader title="Guards" subtitle="In force across every book that runs with guards on." />
        <GuardsPanel />
      </section>

      <section className="space-y-2">
        <AdvisorCredentials />
      </section>

      <section className="space-y-2">
        <SectionHeader title="Not changeable here" subtitle="And the reason, rather than a disabled input." />
        <Panel className="space-y-3 p-3 text-xs">
          {[
            {
              what: 'Permission to trade real money',
              where: 'real_money in config/accounts.toml, and --allow-real on the launcher',
              why: 'Two places have to agree, one edited once and one typed now. This page is served over plain HTTP on a loopback port with no authentication, so anything it could switch on, anything reaching that port could switch on.',
            },
            {
              what: 'Dry run on or off',
              where: 'dry_run in the registry, and -Live on the launcher',
              why: 'The step from recording to trading is meant to be a deliberate word on a command line, never a file somebody edited a week ago.',
            },
            {
              what: 'Lot scale',
              where: 'lot_scale in the registry',
              why: 'On a funded account this is the number that decides what being wrong costs. It is worth the walk to the file.',
            },
            {
              what: 'Which books an account mirrors',
              where: 'runs in the registry',
              why: 'The registry is the record of what is live. A book started around it would be trading and not written down anywhere.',
            },
          ].map((row) => (
            <div key={row.what} className="grid gap-1 sm:grid-cols-[14rem_1fr]">
              <div className="font-medium">{row.what}</div>
              <div className="space-y-1">
                <div className="num text-muted-foreground">{row.where}</div>
                <p className="text-muted-foreground">{row.why}</p>
              </div>
            </div>
          ))}
        </Panel>
      </section>
    </div>
  )
}

function AccountCard({ account, now }: { account: BrokerAccount; now: number }) {
  const live = account.at > 0 && now - account.at < ACCOUNT_STALE_MS
  const isReal = account.demo === false || account.real_money
  // Intent and fact are two different things and the card keeps them apart:
  // `runs` is what the registry says this account should mirror, `mirroring`
  // is which executors are actually reporting into it right now.
  const missing = account.runs.filter((r) => !account.mirroring.includes(r))

  return (
    <Panel className={cn('space-y-3 p-3', isReal && 'border-lp/60')}>
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span className="font-medium">{account.label}</span>
        <span className="num text-muted-foreground text-xs">
          {account.login}
          {account.server ? ` · ${account.server}` : ''}
        </span>
        <div className="ml-auto flex flex-wrap items-center gap-1">
          {isReal && <Tag tone="danger">real money</Tag>}
          {account.demo === true && <Tag>demo</Tag>}
          {!account.configured && <Tag tone="warn">not in the registry</Tag>}
          {!account.enabled && <Tag>disabled</Tag>}
          {account.dry_run ? <Tag tone="warn">dry run</Tag> : <Tag tone="danger">sending orders</Tag>}
          <Tag tone={live ? 'ok' : 'warn'}>{live ? 'connected' : `last heard ${ago(account.at, now)}`}</Tag>
        </div>
      </div>

      <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs sm:grid-cols-4">
        <Fact label="Balance" value={money(account.balance, account.currency)} />
        <Fact label="Equity" value={money(account.equity, account.currency)} />
        <Fact
          label="Margin level"
          value={account.margin_level == null ? '—' : `${Math.round(account.margin_level)}%`}
        />
        <Fact label="Lot scale" value={`×${account.lot_scale}`} />
      </dl>

      <div className="text-xs">
        <span className="text-muted-foreground">Mirroring </span>
        <span className="num">
          {account.mirroring.length}/{account.runs.length || '—'}
        </span>
        {account.positions > 0 && (
          <span className="text-muted-foreground"> · {account.positions} holding a position</span>
        )}
        {missing.length > 0 && (
          // Named, not counted. "6/8" sends the reader to guess which two;
          // this is the line that says a mirror died overnight.
          <div className="text-lp mt-1">
            not reporting: <span className="num">{missing.join(', ')}</span>
          </div>
        )}
      </div>
    </Panel>
  )
}

function BookRow({
  run,
  accounts,
  busy,
  onToggle,
}: {
  run: PaperRun
  accounts: BrokerAccount[]
  busy: boolean
  onToggle: (id: string, paused: boolean) => void
}) {
  const off = run.paused
  const holding = !!run.open
  // Which accounts the registry sends this book to, so a row makes clear
  // whether switching it off stops a paper record or a funded position.
  const onReal = accounts.some((a) => (a.real_money || a.demo === false) && a.runs.includes(run.id))

  return (
    <div className="flex items-center gap-3 px-3 py-2 text-xs">
      <button
        type="button"
        role="switch"
        aria-checked={!off}
        aria-label={`${off ? 'Start' : 'Stop'} ${run.id}`}
        disabled={busy}
        onClick={() => onToggle(run.id, !off)}
        title={
          off
            ? `${run.id} is off — it takes no new position. Click to start it.`
            : holding
              ? `${run.id} is on and holding a position. Switching it off stops NEW entries; the open one is still managed to its stop and target.`
              : `${run.id} is on. Click to stop new entries.`
        }
        className={cn(
          'focus-visible:ring-ring relative h-5 w-9 shrink-0 rounded-full border transition-colors focus-visible:ring-2 focus-visible:outline-none',
          off ? 'bg-muted border-border' : 'bg-lc/70 border-lc',
          busy && 'opacity-50',
        )}
      >
        <span
          className={cn(
            'bg-background absolute top-[2px] h-3.5 w-3.5 rounded-full transition-all',
            off ? 'left-[2px]' : 'left-[18px]',
          )}
        />
      </button>

      <span className="num min-w-0 flex-1 truncate">{run.id}</span>

      <span className="text-muted-foreground shrink-0">
        {run.market}:{run.tf}
      </span>

      {holding && <Tag tone="warn">holding</Tag>}
      {onReal && <Tag tone="danger">real</Tag>}
      {off && <Tag>off</Tag>}
    </div>
  )
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="num">{value}</dd>
    </div>
  )
}

function money(n: number | null, currency: string | null): string {
  if (n == null) return '—'
  return `${n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}${currency ? ` ${currency}` : ''}`
}

function Tag({ children, tone = 'plain' }: { children: React.ReactNode; tone?: 'plain' | 'ok' | 'warn' | 'danger' }) {
  return (
    <span
      className={cn(
        'shrink-0 rounded-sm border px-1 py-px text-[9px] tracking-wide uppercase',
        tone === 'plain' && 'border-border text-muted-foreground',
        tone === 'ok' && 'border-lc/60 text-lc',
        // caution, not sp: sp is the teal of a short put and means nothing
        // here, while caution is the amber the rest of the desk already uses
        // for "look at this".
        tone === 'warn' && 'border-caution/60 text-caution',
        tone === 'danger' && 'border-lp/60 text-lp',
      )}
    >
      {children}
    </span>
  )
}
