# 2026-09-15-monthend-fix-slope: a month of US equity outperformance signs the euro's move through the last month-end fix

**Registered:** (commit time is authoritative) — before the instrument exists
and before a single month-end window is read
**Status:** registered
**Instrument:** `scripts/monthend_fix_slope.py` — does not exist yet; written
after this file is committed
**Came from:** `docs/research/scouting/2026-09-15-monthend-hedge-rebalance.json`,
the one proposal of four that cleared both gates (historian `NOVEL`,
feasibility `TESTABLE`)

## The mechanism, and it is a mechanism rather than a signal

A euro-based institution holding US equities hedges the currency with short
dollar forwards sized to the portfolio's value, and **its investment management
agreement says when to resize: at month end, at the benchmark.**

So when US equities outperform euro-area equities through a month, that
holder's dollar exposure has grown past what its hedge covers and it must sell
more dollars. The hedge ratio is a policy number in a contract, not a view. Two
different constraints, one sign.

Three things make this a forced flow rather than an opinion:

- **The size is not private.** It is a function of the month's equity return,
  which everybody can see, and of the stock of cross-border holdings, which
  changes slowly.
- **The execution venue is contractual.** The client's mark and the client's
  fill must be the same number, so the order is deliberately price-insensitive
  and the executor is instructed *not* to seek a better price.
- **The forcer cannot stop.** A fund that stops resizing its hedge at the
  benchmark is out of compliance with its own mandate. Unlike an informed
  trader, it cannot wait for a better day.

## Claim

On `eurduka` 15-minute bars, 2010-06 → 2026-05, the **slope** of the
15:45–16:15 London EURUSD return on the month's US-minus-euro-area equity
return is **positive** across the last-business-day-of-month observations.

Positive means: US outperformance precedes euro strength through the fix.
**No mirror is available** — a negative slope is not a finding with the sign
flipped, it is this hypothesis failing.

## Falsifier

All of it declared here, before `scripts/monthend_fix_slope.py` is written.

**The conditioning variable.** Month-to-date total return of the S&P 500 minus
that of the EURO STOXX 50, measured through **the close prior to the last
business day** — the information a treasurer actually has when the order is
sized.

> **Amendment made at registration, not after:** free daily index data is
> **price** return, not total return. Dividends are excluded on both legs. The
> difference between the two is a slow, roughly-offsetting drift on a monthly
> horizon and not a monthly signal, and the same substitution is made on both
> sides of the subtraction — but it is a substitution, it is recorded here
> rather than in a footnote later, and the record must call the variable what
> it is.

**The statistic.** The ordinary-least-squares slope of the 15:45–16:15 London
EURUSD return on that variable, across the ~190 month-ends of 2010-06 → 2026-05.

**It dies if any one of these holds:**

1. **The slope is ≤ 0.**
2. **It fails the 95th percentile** of a null that permutes the equity return
   across month-ends. That null preserves the unconditional month-end drift and
   tests only the conditioning — which is the whole point, since an
   unconditional drift would show up as an intercept and must not be allowed to
   masquerade as a slope. **100,000 draws**, declared here: fault 11 in
   `docs/decisions/2026-09-13-instrument-faults.md` is a gate placed at a
   precision its estimator could not deliver, and 1,000 draws have a standard
   error of 0.74 percentage points against a gate written at 5.
3. **Three placebos, each fatal on its own.**
   - **Day.** The same regression on the **penultimate** business day must not
     produce the same slope. If it does, the day is not the variable and this
     is monthly equity momentum leaking into the euro.
   - **Window.** The same regression on **13:45–14:15 London** — inside
     European hours, containing no benchmark — must not produce it. This one is
     mandatory rather than decorative: `docs/decisions/2026-09-14-fx-local-hours.md`
     established a real unconditional euro drift through European hours and the
     fix window sits inside it. That closed effect is the most likely confound
     and it must be shown to be the **intercept**, not the slope.
   - **Sign.** The identical machinery on a month-to-date return of **gold**
     instead of equities. A slope there of comparable size means the
     conditioning variable is "risk appetite" and the hedging story is
     decoration.

## The power, stated before the window is opened

This is the number the last twenty-five registrations here kept discovering
afterwards, and feasibility measured it in advance:

| | |
|---|---|
| observations | **192** month-ends, 2010-06 → 2026-05 |
| window coverage | mean **29.95** of the 30 minutes; minimum 25 |
| window standard deviation | **14.3 bp** |
| mean absolute move | 11.3 bp |
| round trip (1.4 pips) | **1.21 bp** |
| 80% power at the 95th percentile needs | r ≥ 0.20, i.e. **≈ 2.9 bp per 1 sd** of the equity variable |

So this sample resolves only an effect **at least 2.4× the round trip**. An
effect worth exactly its own cost sits at t ≈ 1.2 and is invisible here.

**The 1.2–2.9 bp band is therefore declared UNDERPOWERED in advance, not
"no".** If the slope lands there, the record says the sample could not resolve
it — which is a different sentence from a refutation, and writing it now is the
only way that distinction survives contact with a disappointing number.

Two month-ends (2013-03-29, 2024-03-29) are Good Fridays with 25 of 30
minutes. **They are kept**, declared here; the run reports the figure with and
without them and the headline is the 192.

## Data

- `eurduka:15m` 2010-06-01 → 2026-05-31, on disk, fetched 2026-09-13. Every
  one of the 192 month-ends carries the 15:45–16:15 London window, and both
  placebo windows are 192/192 covered, so nothing here can fail for want of
  bars.
- `xauduka:15m`, same span, for the gold sign placebo.
- Daily closes for the S&P 500 and the EURO STOXX 50 — **not on disk when this
  was registered**, being collected into `data/equity/` with a provenance
  header. If either index cannot be obtained the registration is **withdrawn,
  not weakened**: a one-sided conditioning variable is a different hypothesis.

## The multiplicity, and the sibling this must not be read apart from

**One cell.** One window, one variable, one direction, three placebos. There is
no grid here and none will be added.

But the historian found the thing that governs how this must be read:
**`2026-09-15-etf-creation-settlement` is the same mechanism in a different
suit.** Both say a publicly observable number creates a mandated,
price-insensitive order of computable size that must execute inside a known
benchmark window — LBMA 15:00 in one, WM 16:00 in the other. They share a
closest closed relative, `docs/decisions/2026-09-13-london-fix.md`, which found
no unconditional drift at the 74th–84th percentile, and they share a single
point of failure: **whether a benchmark window at retail spread carries
anything at all once conditioned.**

So, declared now: a negative here is **not** independent evidence for running
the ETF one, and two positives would **not** be two findings. They are one
programme and the second must carry the first's multiplicity. That is how the
adversary treated the three `fx-local-hours` rows — "the rows are one degree of
freedom" — and it applies here for the same reason.

## What each outcome means

- **Slope positive, past the null, past all three placebos, ≥ 2.9 bp** → the
  desk has a conditioned benchmark-window effect that its own sample can
  resolve. It is still one cell of one programme and the ETF sibling inherits
  its multiplicity.
- **Slope positive but in the 1.2–2.9 bp band** → underpowered. Not a
  refutation. The record says so and names what sample would settle it.
- **Any placebo reproduces the slope** → closed, and the placebo names what it
  really was: monthly momentum, the closed European-hours drift, or risk
  appetite.
- **Slope ≤ 0, or inside the null** → closed, and the ETF sibling loses most of
  its prior with it.

**No outcome of this registration is a paper run or a strategy.** At 192
observations a year apart in twelve, the best case is about sixteen trades a
year on a euro pair at 1.4 pips. It is a question about whether mandated flow
prints, and the answer is worth having either way.
