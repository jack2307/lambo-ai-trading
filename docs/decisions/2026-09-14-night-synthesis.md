# A second night of the loop: seven registrations, zero survivors, three real clocks worth the spread, four instrument faults

**Date:** 2026-09-13 21:00 → 2026-09-14 12:00 (unattended, on the owner's
instruction to keep going until something came out)
**Question:** With sixteen years of Dukascopy bars on three assets (gold,
silver, EURUSD; 2010-06 → 2026-05) and the loop's discipline — long window
primary, a second window as confirmation, a count- and hold-matched random
null, the method's own sides permuted, three veto reviews — is there any
pre-registrable claim on this feed at Vantage cost that survives its
second window?
**Outcome:** No. Seven registrations ran end to end and every one is
closed; the running total is 25. What came out is not a strategy but a
shape, seen now on three assets and five mechanisms: **a real drift, at the
95th percentile or better of every null on its first window, worth about
the spread, and gone or unresolvable on its second.** Four faults in the
instrument were found by the reviews and fixed with tests before the next
run; two of them had flattered every earlier session-hold receipt by a
small, stated amount and changed no verdict.

## The ledger

| registration | claim | first window | second window | verdict |
|---|---|---|---|---|
| `2026-09-13-close-reopen-drift` | gold long 16:30 → 18:30 NY across the close | 1,355 holds PF 1.25, 100th of both nulls; nets zero to 2022 | 282 holds PF 1.30, 96th | fails the 0.05R gate as a unit; the drift is $0.10–0.20 an ounce a session |
| `2026-09-13-friday-weekend-hold` | gold Friday 16:30 → Sunday 18:30 | 2018–25: 336 holds PF 2.06, 100th (the closed record) | 2010–18, the window the adversary named: 400 holds PF 0.98, 92nd, 2014–18 negative every year | closed for good |
| `2026-09-13-tsmom-silver` | 20/60/120-day sign on silver, rebalanced 16:15 | 2010–18: 20d PF 1.34, 100th/98th (one of three); 120d inverted at the 0th | 2018–26: 20d PF 1.03, 63rd/62nd | closed; with gold, two assets |
| `2026-09-14-tsmom-eurusd` | 60-day sign on EURUSD, one lookback, the adversary's terms | 2010–18: 108 trades PF 1.81, 100th of the sized null, **85th on direction**; one 356-day trade = 71% of net | not opened | closed; three assets, one shape |
| `2026-09-14-intraday-momentum` | gold's first half hour sets its last hour (Gao et al. 2018), new method | 2010–18: PF 0.61 on 2,022, 40th on direction; the window loses the spread | not opened | closed; the day-so-far sign reversed at the 1st, post hoc |
| `2026-09-14-fx-local-hours` | the euro falls in European hours (Breedon–Ranaldo 2013) | 2010–18: 2,087 holds, 100th/99th, −6,150 pips in eight years, eight of nine years vs trend; PF 1.07 at 1.4 pips, 1.16 at zero | not opened; the gate needed −0.7 pips of spread | closed on the gate |
| `2026-09-14-fx-local-hours-sign` | the same, as a sign claim on the unread window, two conditions named in advance | 2018–26: 67th on direction, excess −0.10 pips a hold (t = −0.16) | — | closed; the clock stopped |

Every number is in `docs/research/runs/<id>/` and quoted in the record
named in the first column.

## The shape

Five mechanisms, three assets, one pattern, stated as the adversary stated
it on the EURUSD record and checked on the others:

- **On the first window the effect is real by every test the loop has.**
  Close-reopen 100th of both nulls on 1,355 sessions; the weekend hold
  100th on 336; silver's 20-day sign 100th/98th; the euro's European hours
  100th/99th on 2,087 holds with the paper's mechanism visible in the
  hours before the US data. These are not selection artefacts: each was
  registered with a falsifier before the run, the nulls are matched to
  hours, count and hold, and the direction null is the method's own trades.
- **The effect is worth about what it costs to touch.** Gold's close
  drift: $0.10–0.20 an ounce against $0.28. The euro's hours: 3 pips
  against 1.4, and a profit factor of 1.16 at zero spread. Silver's and
  EURUSD's trend signs: three to five trades of profit in eight years,
  which no null of a hundred trades can resolve. The registered gate —
  5% of a daily range per hold, net — was never reachable by a clock
  hold, and three registrations said so before running.
- **On the second window it is gone or unresolvable.** The weekend hold
  on 2010–18: negative five years running. Silver's 20-day sign on
  2018–26: a coin flip. The euro's hours on 2018–26: a tenth of a pip. The
  one exception — gold's close drift keeps its sign on 2025–26 — is the
  one that was never worth anything.

The reading the loop can defend: on a 24-hour CFD feed at retail cost,
the persistent clock effects in the literature exist at the size the
literature reports, and that size is the spread; the trend effects exist
as a few long holds per decade, and a decade is what it takes to see one.
Nothing tested tonight carries a retail-tradable edge, and the loop has
now said so on 25 registrations across two nights with the tests getting
stricter each time.

## What was learned about the instrument

Four faults, all found by a review reading a receipt against code, all
fixed with tests the same night (`2026-09-13-instrument-faults.md`,
addendum):

1. The sizing range counted Sunday evenings as days (R 6–9% small).
2. The hold null guessed half the lookback, then held the mean; now it
   draws each hold from the method's realised log-normal distribution.
3. The hold null exited one bar late — across the halt and the weekend
   when that bar was the day's last — so every session-hold receipt's
   sized-null column was slightly generous; the direction null decided
   every one of those rows the same way, and the records say so.
4. Filters gate entries only; a self-managed method exits on the Sunday
   reopen bar.

And one about the tests themselves: for a fixed-window hold the sized
random-hold null and the permuted-sides null are one test in two spellings
(the adversary on close-reopen; confirmed to two decimals on
intraday-momentum once fault 3 was fixed), so those records count one null,
not two. For a trend-following hold whose profit is three trades, the
direction null is saturated — it has about one answer in eight — and a
test that cannot resolve a result is not one the result can pass.

## What is left

- **The guards.** Every receipt in this repository is unguarded; the risk
  role has blocked every promotion on that and on the absence of an
  unrealised-loss cap, a notional cap and a weekend rule for self-managed
  holds. Nothing can be proposed for paper until those exist in code with
  tests, and nothing tonight earned a proposal anyway.
- **Two sign claims of no trading value**, in the backlog at low priority:
  gold's intraday reversal (one extreme of six draws, 64% one year) and
  the local-hours effect on another pair.
- **Options flow at levels**, the reason the project exists, still
  waiting on a tape long enough to test.
- **The different program**: time-series momentum as the literature
  measures it — daily bars, futures, a diversified book, the 12-month
  sign — is a research program with its own data and cost model, not a
  row here.

## What this does not say

- It does not say the owner's business needs an edge. A bot that trades
  the client's chosen size with a stop, a daily loss limit and no weekend
  exposure is a product whose value is discipline, and the receipts are
  the honest thing to show a client who asks what it earns: at this cost,
  what the market gives, minus the spread.
- It does not say the loop is finished. It says the registry of
  literature-backed, data-available claims on this feed is nearly empty,
  and that adding rows without a new reason is the search the nulls exist
  to catch.

## Amendment, 2026-09-19: "zero survivors" is exact about strategies and imprecise as written

The title and the outcome line of this record say zero survivors. On the seven
registrations in the ledger above that is exact and nothing below changes it.
But later the same night a registration that is not in that ledger passed its
falsifier, and this record has never pointed at it.

**What the other document says.** `docs/hypotheses/2026-09-14-pre-nfp-drift.md`
was registered with three numbers before its test window was opened, and all
three hold on that window: the 3.2nd percentile of a clock-matched Friday null
against a gate of the 5th, a mean of −1.536 $/oz against a gate of −0.40, and
35.6% of hours up against a gate of 42%, on n = 90 releases. Its own status line
records the pass, and `docs/decisions/2026-09-14-pre-nfp-drift.md` opens: *"The
claim survives its falsifier — the first registration in this loop to do so."*

**Which is right.** Both, in different words, and the distinction is worth more
than a corrected sentence:

- **No tradable strategy survived, and that is what this record measured.** The
  pre-NFP registration declares its own profit-factor gate **unreachable in
  advance** — the effect is worth roughly $46 a year on a $10,000 account by the
  decision's arithmetic, against a $0.28 spread — and states in the
  registration, before the window was read, that *"no outcome of this
  registration is a paper run or a strategy"*. Its decision adds that the
  repository's own news guard blacks out 07:30–09:00 New York, so the hour it
  names is one the bot already refuses to trade. On this record's own criterion
  — a retail-tradable edge at Vantage cost — the count of survivors is still
  zero, on 2026-09-14 and today.
- **One registration did survive its falsifier**, and this file must not be
  read as saying none ever has. It is a **sign claim**, not a strategy; the
  registration says so in as many words ("this is a sign claim, not a
  strategy") and that is the whole of the difference between the two sentences.

The title stands as the record of what was measured on those seven. What is
corrected here is the reading, not the number.

**One other thing about the count.** "The running total is 25" is a count at
2026-09-14 12:00, where this record's window closes. `2026-09-14-volman-box`
closed on the afternoon of the same day and `2026-09-14-pre-nfp-drift` that
night, which is why `2026-09-14-pre-nfp-drift.md` can say "twenty-six
registrations closed before this one" without contradicting the 25 above. The
arithmetic is set out in section 6 of `docs/research/VERDICTS.md`.

*Written 2026-09-19 by the records session, from the two documents named.
Nothing above is recomputed; every number is quoted from the file it is
attributed to.*
