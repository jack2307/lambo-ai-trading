# 2026-09-15-nfp-cross-asset: does the pre-NFP hour exist on instruments nobody has looked at?

**Registered:** (commit time is authoritative) — before any bar of `xagduka`
or `eurduka` is read for this or any other news question
**Status:** registered
**Instrument:** `scripts/news_drift.py`, unchanged since `436582c`, plus
`scripts/friday_cells.py` for the small cell. **No new code.**
**Parent:** `docs/decisions/2026-09-14-pre-nfp-drift.md`

## Why this is the last question worth asking about this hour

Gold's 07:30 → 08:30 New York hour before the US Employment Situation is the
only claim in twenty-eight registrations that survived its falsifier. Two
attempts to explain it away have now failed and made it narrower rather than
weaker:

- **Not macro news in general.** US Personal Income and PCE, 199 releases
  collected, 90 of them on Fridays with no employment report: matched on the
  day of the month the release adds −0.409 $/oz at a Welch t of −0.50, medians
  −0.24 against −0.26. Nothing.
- **Not employment news in general.** Canada's Labour Force Survey, 201
  releases, with a control inside the release itself — Statistics Canada moved
  it from 07:00 to 08:30 New York on 2012-04-05 for administrative reasons.
  At 08:30, on Fridays with no US report: +0.622 mean, 53.6% of hours **up**,
  against +0.023 and 50.8% on Fridays with no jobs report at all. Nothing, and
  the wrong sign.

So the claim is now narrower than the parent stated it: the US Employment
Situation specifically. And everything supporting it has been measured on
**one instrument**, `xauduka`, whose 2010–2026 bars have been read for this
family a dozen times.

`xagduka` (silver) and `eurduka` (the euro) have been on disk since
2026-09-13 and have **never been read for any news question at all**. That is
the only genuinely out-of-sample data this loop still owns for this claim, and
this registration spends it.

## Claim

On `xagduka` and `eurduka`, 2010-06-01 → 2026-05-31, the same hour before the
same release falls, by the same three numbers the parent declared for gold.

The mechanism offered, not shown: the month's most anticipated US number is
the one books are squared into, so the dollar is bought and what is priced in
dollars is sold. The euro is the instrument that can tell those two apart — if
the hour is dollar strength, EURUSD falls too; if it is something gold-specific,
EURUSD does not move and the metals do.

## Falsifier

Per instrument, from
`python scripts/news_drift.py <market> 2010-06-01 2026-05-31 --permute="US Employment Situation (NFP):-60:0" --draws=1000`:

1. The actual mean sits **at or below the 5th percentile** of 1000 date
   permutations holding the weekday and the New York minute fixed.
2. The share of release hours that were **up is ≤ 42%** — the parent's own
   threshold, and gold reads 33.1% on 178 Fridays against 53.1% on quiet ones.
3. The mean move is **negative** in the instrument's own units.

All three, on an instrument, is that instrument confirming. Any one failing is
that instrument refusing.

**Condition 1 carries a known fault and is kept anyway, for comparability.**
The permutation null holds the weekday and the minute and lets the **day of
the month** float, and gold's turn-of-month Fridays are not flat — that is the
amendment already filed against the parent. So condition 1 cannot separate the
release from the turn of the month on any instrument. It is retained because
it is the number the parent was judged on and the comparison must be exact.
Conditions 2 and 3 are absolute and do not depend on the null.

**Reported before it is read, and NOT gated:** the confound-free cell. The
release lands on a month's second Friday about fifteen times in this span, on
days 8–14, where no turn-of-month tilt can reach it. Gold reads −1.734 with
**20.0% up on 25 days** there, against +0.258 and 52.8% on 142 quiet Fridays
on the same days of the month. That cell will be printed for both instruments
and read as direction only: **on 25 observations a 42% gate would pass 21% of
the time by chance**, which is not a test and will not be called one.

## Sample needed, and the power, declared

- `xagduka`: **184** of the 191 releases have bars at both ends of the window.
- `eurduka`: **189** of 191.

Both counted before this file was written, from bar coverage and the calendar
only. At those counts a 42% up-rate is an exact two-sided sign test at about
p = 0.03, so condition 2 is a real gate with real power — which is the thing
the last two registrations in this thread did not have and the reason this one
is worth running.

**The multiplicity:** two instruments, three conditions each, one look. No
window is split and none is held back, because neither instrument has been
read at all — the whole span is out of sample. If both instruments confirm,
that is two independent-ish reads; silver is not very independent of gold and
is described that way below.

## Data

- `xagduka:15m` and `eurduka:15m`, 2010-06-01 → 2026-05-31, fetched
  2026-09-13 from Dukascopy, never read for a news question. Spreads measured
  read-only against the live Vantage terminal: silver $0.021, EURUSD 0.00014.
- The calendar: `data/news/events.csv`, scheduled layer, the same 191 releases
  the parent used. **Not** the extended calendar — this is the parent's test
  repeated, and changing the event set would change the question.

## What each outcome means

- **Both confirm** → the hour is a property of the release and not of gold.
  With the euro confirming, it is dollar strength; the record says so and the
  claim finally has a mechanism rather than a correlation. It is still worth
  about $46 a year on gold and still inside the bot's own news blackout, and
  **no outcome of this registration is a paper run or a strategy.**
- **Silver confirms, the euro does not** → the effect is the metals complex,
  not the dollar. Silver is correlated with gold at roughly 0.8 on daily
  moves, so this is the weakest of the informative outcomes: it may be one
  effect seen twice.
- **The euro confirms, silver does not** → the effect is the dollar, and
  gold's version of it is incidental. That would be the most interesting
  outcome and the least expected.
- **Neither confirms** → gold's pre-NFP hour is a fact about gold's 2010–2026
  bars and not about the release. The parent's record is amended to say the
  claim did not replicate on the only out-of-sample instruments available, and
  the thread ends there. **This is the outcome the loop's own history says to
  expect**, and it is the reason the question is being asked.
