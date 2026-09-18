import { useEffect, useMemo, useState } from 'react'
import { Menu, Monitor, Moon, Sun } from 'lucide-react'

import type { Book, View } from '@/App'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { GuardsPanel } from '@/components/GuardsPanel'
import type { BrokerAccount, HtfResponse, LiveBar, MarketInfo } from '@/lib/api'
import { clock, liveAge, since } from '@/lib/format'
import type { Theme } from '@/lib/theme'
import {
  BIAS_RULE,
  biasGlyph,
  biasHue,
  biasTally,
  biasTint,
  biasWash,
  htfBias,
  structureGlyph,
  structureTint,
} from '@/lib/htfBias'
import { cn } from '@/lib/utils'

interface Props {
  view: View
  markets: MarketInfo[]
  market: string
  onMarketChange: (market: string) => void
  book: Book
  accounts: BrokerAccount[]
  isLive: (a: BrokerAccount) => boolean
  /** The live stream, subscribed once in `App`. */
  ticks: Record<string, LiveBar>
  streaming: boolean
  theme: Theme
  onThemeChange: (theme: Theme) => void
  /** Higher timeframe for the market this strip names. */
  htf: HtfResponse | null
  /** Narrow screens only: the sidebar is an overlay there and needs opening. */
  onOpenMenu: () => void
}

/**
 * How long an executor's snapshot stays believable.
 *
 * Exported because `App` owns the accounts poll now and applies the same rule:
 * an account whose mirror has stopped must not stay selected, and two
 * components disagreeing about what "stopped" means would be worse than
 * either answer.
 */
export const BROKER_STALE_MS = 45_000

/**
 * The strip's height, in pixels, exported so the pages that size themselves
 * against it cannot drift from it.
 *
 * `py-1.5` either side of a 24px control, plus the bottom border. It was 45
 * for the taller bar this replaced, written as a literal in BOTH Desk.tsx and
 * Workbench.tsx - two copies of a number describing a third file, which is the
 * arrangement that goes stale the first time anyone changes the padding. It
 * now lives beside the markup that decides it.
 */
export const APP_BAR_H = 37

/** A price, formatted as the Desk formats one. */
const quoteFmt = (v: number): string =>
  Math.abs(v) >= 1000
    ? v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
    : v.toFixed(Math.abs(v) >= 10 ? 2 : 4)

/** The two screens that read one market at a time; the rest ignore the picker. */
const MARKET_VIEWS: View[] = ['workbench', 'tape']

/**
 * The status strip. What the desk IS, not what to look at next.
 *
 * Navigation and the book switcher moved into the sidebar on 2026-09-18: a row
 * of seven pills said nothing about what any of them was for, and the book
 * switcher is the first decision of a visit rather than a control to reach for
 * afterwards. What is left here is state a reader wants visible on every
 * screen and never clicks: which market, what the broker side is doing, and
 * what the selected account is worth.
 */
/**
 * One timeframe's structure, as the strip shows it.
 *
 * A FACT, not a reading: `structure.label` is what the route publishes, from
 * a rule it names. The bias word beside it in the strip is this client's
 * summary and wears a glyph to say so.
 *
 * THE THREE ABSENCES ARE DIFFERENT AND ARE SAID DIFFERENTLY. `undefined` is a
 * server older than the H1 read — nothing is wrong, this desk is simply ahead
 * of its API. `null` is the timeframe's bars missing entirely, which is worth
 * going to look at. No `htf` at all is a market with no higher-timeframe read.
 * Collapsing them into one dash would make a deploy-order artefact look like
 * a data outage.
 */
function StructureWord({
  tf,
  htf,
  which,
  now,
}: {
  tf: 'H1' | 'H4'
  htf: HtfResponse | null
  which: 'h1' | 'h4'
  now: number
}) {
  const block = htf ? htf[which] : null
  // `h1` is optional on the wire; `h4` has always been sent.
  const unsent = which === 'h1' && htf != null && htf.h1 === undefined
  const sentence = htf?.unavailable_by_tf?.[which === 'h1' ? '1h' : '4h'] ?? htf?.unavailable ?? null

  if (!block) {
    return (
      <span
        className="text-muted-foreground/50"
        title={
          !htf
            ? 'no higher-timeframe reading for this market'
            : unsent
              ? `This desk shows ${tf}, but the API it is talking to does not send it yet.`
              : (sentence ?? `no ${tf} reading for this market`)
        }
      >
        {tf}: —
      </span>
    )
  }

  const s = block.structure
  const where =
    s.break_level != null && s.break_side
      ? ` Breaks ${s.break_side.toLowerCase()} ${s.break_level.toFixed(2)}.`
      : ''
  return (
    <span
      className={structureTint(s.label)}
      title={`${tf} structure ${s.label} by ${s.rule}.${
        s.confirmed_at_bar_ms != null
          ? ` Confirmed ${since(s.confirmed_at_bar_ms, now)} ago, on the bar that closed ${clock(s.confirmed_at_bar_ms)}Z.`
          : ''
      }${where} A published fact, not a summary.`}
    >
      <span aria-hidden>{structureGlyph(s.label)}</span> {tf}: {s.label.toLowerCase()}
    </span>
  )
}

export function AppBar({ view, markets, market, onMarketChange, book, accounts, isLive, ticks, streaming, theme, onThemeChange, htf, onOpenMenu }: Props) {
  const needsMarket = MARKET_VIEWS.includes(view)
  const [now, setNow] = useState(() => Date.now())

  // A second hand for the caption's "quiet for N s". Nothing is fetched here;
  // `App` owns the poll.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  // The freshest stream for the market this app is pointed at.
  //
  // Keyed `market:tf`, and a market can carry several timeframes, so the
  // newest reading wins - they are the same price seen at different
  // resolutions, and the oldest of them is the one that looks like a stall.
  //
  // It is the MARKET's price and not the book's: the strip shows the same
  // thing on paper and on an account, because a spread does not belong to a
  // book. The label names the market it is quoting so it can never be read as
  // a price for something else.
  const quote = useMemo(() => {
    let best: { stream: string; bar: LiveBar } | null = null
    for (const [stream, bar] of Object.entries(ticks)) {
      if (!stream.startsWith(`${market}:`)) continue
      if (!best || bar.at > best.bar.at) best = { stream, bar }
    }
    return best
  }, [ticks, market])

  const spread =
    quote?.bar.bid != null && quote?.bar.ask != null ? quote.bar.ask - quote.bar.bid : null

  const live = accounts.filter(isLive)
  const chosen = typeof book === 'number' ? (accounts.find((a) => a.login === book) ?? null) : null

  return (
    <header className="bg-card/80 sticky top-0 z-20 flex flex-wrap items-center gap-3 border-b px-3 py-1.5 backdrop-blur">
      {/* Only where the sidebar is an overlay. Above lg it is in the flow and
          has its own collapse control, so a second one here would be two
          buttons for one thing. */}
      <button
        type="button"
        onClick={onOpenMenu}
        aria-label="Open menu"
        className="text-muted-foreground hover:bg-accent hover:text-foreground flex size-6 items-center justify-center rounded-sm transition-colors duration-100 motion-reduce:transition-none lg:hidden"
      >
        <Menu className="size-4" />
      </button>

      {/* market · last · spread · age. The market is named, so this can never
          be read as a price for whatever the Desk happens to be showing.
          Absence is stated: no tick for this market says so rather than
          leaving a gap that reads as a quiet market. */}
      <div className="flex items-baseline gap-2">
        <span className="text-muted-foreground fd-caption">{market || '—'}</span>
        {quote ? (
          <>
            <span className="num fd-body tabular-nums">{quoteFmt(quote.bar.close)}</span>
            {spread != null ? (
              <span
                className="text-muted-foreground num fd-caption tabular-nums"
                title="ask minus bid, in the market's quote units"
              >
                spread {spread.toFixed(spread < 1 ? 2 : 1)}
              </span>
            ) : (
              <span className="text-muted-foreground/60 fd-caption" title="this feed sends no bid/ask">
                no spread
              </span>
            )}
            {/* The dot pulses only while ticks are arriving. A chart that has
                silently stopped updating is indistinguishable from a quiet
                market, and this is the one thing on the strip that tells them
                apart - so it must not animate when the stream is down. */}
            <span
              className={cn(
                'size-1.5 rounded-full',
                streaming ? 'bg-lc motion-safe:animate-pulse' : 'bg-muted-foreground/50',
              )}
              title={streaming ? 'stream connected' : 'stream down'}
              aria-hidden
            />
            <span className="text-muted-foreground num fd-caption tabular-nums">
              {liveAge(quote.bar.at, now)}
            </span>
          </>
        ) : (
          <span className="text-muted-foreground/60 fd-caption">
            {streaming ? 'no tick yet' : 'stream down'}
          </span>
        )}
      </div>

      {/* The read, from every screen: two FACTS and then one SUMMARY.
          Unavailable says so rather than defaulting to RANGE, which would be
          a reading nobody made.

          The order is the ladder — hour, then four-hour, then the word. When
          the two disagree, the disagreement IS the information, and there is
          deliberately no third word averaging them: an "aligned/divergent"
          score would answer a question nobody here has measured. */}
      {(() => {
        const bias = htfBias(htf?.h4)
        const lamps = bias
          ? bias.votes.map((v) => `${v.name} ${v.vote ?? 'no vote'}`).join(' · ')
          : ''
        const conf = htf?.h4?.structure.confirmed_at_bar_ms ?? null
        return (
          <span className="border-border num inline-flex items-center gap-1 rounded-sm border px-1.5 py-px fd-caption tabular-nums">
            <StructureWord tf="H1" htf={htf} which="h1" now={now} />
            <span className="text-muted-foreground/40" aria-hidden>
              ·
            </span>
            <StructureWord tf="H4" htf={htf} which="h4" now={now} />
            {bias && (
              <>
                {/* Dropped FIRST when the strip is tight: the two structures
                    are facts the desk publishes, the word is this client's
                    summary of one of them. A summary is the right thing to
                    lose when there is no room for everything. */}
                <span className="text-muted-foreground/40 hidden sm:inline" aria-hidden>
                  ·
                </span>
                {/* A faint pill in the bias hue, matching the card's wash
                    so one colour means one thing across the page. Nothing on
                    RANGE — `biasHue` returns null and the word sits on the
                    strip's own ground, which is what "no reading" should look
                    like. The alpha is the same measured token the card uses;
                    the word is its own hue on a wash of that hue, which is
                    the pair that caps how strong either can be. */}
                <span
                  style={
                    biasHue(bias.word)
                      ? {
                          backgroundColor: `color-mix(in oklab, ${biasHue(bias.word)} ${biasWash(bias)}, transparent)`,
                        }
                      : undefined
                  }
                  className={cn('hidden rounded-sm px-1 sm:inline', biasTint(bias.word))}
                  title={`H4 bias ${bias.word} — ${biasTally(bias)}. ${lamps}. ${
                    conf != null
                      ? `Confirmed ${since(conf, now)} ago, on the bar that closed ${clock(conf)}Z. `
                      : ''
                  }${BIAS_RULE} A summary, not a signal.`}
                >
                  <span aria-hidden>{biasGlyph(bias.word)}</span> {bias.word.toLowerCase()}
                </span>
              </>
            )}
          </span>
        )
      })()}

      {needsMarket && (
        <label className="text-muted-foreground flex items-center gap-2 text-[11px]">
          market
          <Select value={market} onValueChange={onMarketChange}>
            <SelectTrigger size="sm" className="h-6 w-[140px] text-[11px]">
              <SelectValue placeholder="loading…" />
            </SelectTrigger>
            <SelectContent>
              {markets.map((entry) => (
                <SelectItem key={entry.id} value={entry.id} disabled={!entry.hasData}>
                  {entry.id}
                  {!entry.hasData && ' — no tape'}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
      )}

      <div className="ml-auto flex items-center gap-3">
        {/* The selected account's equity, in ITS unit and never converted.
            Here because it is the number a person glances at from any screen,
            and the one they would otherwise leave the page to find. */}
        {chosen && (
          <span
            className="num text-[12px] tabular-nums"
            title={`${chosen.label} equity${chosen.real_money ? ' — REAL MONEY' : ''}`}
          >
            <span className={cn(chosen.real_money ? 'text-primary' : 'text-muted-foreground')}>
              {chosen.equity != null
                ? `${Math.round(chosen.equity).toLocaleString('en-US')} ${chosen.currency ?? ''}`.trim()
                : '—'}
            </span>
          </span>
        )}
        <GuardsPanel />
        <BrokerCaption accounts={accounts} live={live} chosen={chosen} now={now} />
        {/* Three states, not a switch. `system` is a real choice - follow the
            machine at dusk - and a boolean cannot say it. */}
        <div className="border-border flex items-center rounded-sm border p-px" role="group" aria-label="Theme">
          {([
            ['dark', Moon, 'Dark'],
            ['light', Sun, 'Light'],
            ['system', Monitor, 'Follow the system'],
          ] as [Theme, typeof Moon, string][]).map(([id, Icon, label]) => (
            <button
              key={id}
              type="button"
              onClick={() => onThemeChange(id)}
              aria-pressed={theme === id}
              title={label}
              aria-label={label}
              className={cn(
                'flex size-5 items-center justify-center rounded-[3px] transition-colors duration-100 motion-reduce:transition-none',
                theme === id
                  ? 'bg-accent text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Icon className="size-3" />
            </button>
          ))}
        </div>
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
