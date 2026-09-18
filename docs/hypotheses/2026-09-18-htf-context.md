# 2026-09-18-htf-context: higher-timeframe facts as CONTEXT, and one structure-only rule on top of them

**Registered:** 2026-09-18, **before either book has seen a single bar.**
Everything below is a pre-commitment; nothing in it may be changed by what the
books turn out to say.
**Status:** RUNNING. Both books and their coins were created on the VPS at
**2026-09-18T04:07:34Z**, and that is when their record begins. The first
decision is the 04:15Z close.

The books existed as prompt variants and as launcher rows from 03:35Z and as
books only from 04:07:34Z; in between both traders polled a run id that did
not exist and decided nothing. Nothing in that gap is data — there are no
rows, not even NONE rows — but the start time is written here rather than
inferred from the first row, because "when did this campaign begin" should
not be answered by whichever bar happened to be first.
**Books:** `ai-xau-ds-ctx` (base, already running, untouched) as control,
against `ai-xau-ds-ctx-htf` (variant `htf-context`) and
`ai-xau-ds-ctx-htf-filter` (variant `htf-filter`), each with its own coin and
its own seed.
**Code:** `py/live/htf_context.py`, `py/live/ai_trader.py` (`VARIANTS`,
`HTF_RULE`), `crates/fd-api/src/htf.rs` (the route, written by d1),
`py/live/prompt_variant_selftest.py`.

## Two claims, and the second is built on the first

The first: *a model deciding from 15m bars alone decides differently when it
can also see what the H4 and D1 bars say.* That is `htf-context`, and it is a
claim about the **decision**, not about the higher timeframe — the facts could
change what the model does while predicting nothing.

The second: *constraining the model to the H4 swing structure improves the
book.* That is `htf-filter`, and it is the trader's proverb stated so it can
fail. It is the stronger claim and it is tested second, on top of the first,
because a filter on facts the model ignores would be a different experiment
from a filter on facts it reads.

## The prior, and it is not encouraging

Thirty-two registrations in `docs/decisions/` are closed and none survived out
of sample. The relevant one here is the 2026-09-14 synthesis: **four-year
trend signs existed at the 95th percentile on the first window and were gone
on the second.** Whatever is registered here starts from that.

A direct study of these four trend definitions against this desk's own 81
closed gold paper trades was run on 2026-09-18 (`STRATEGY BOOKS`, n=67 after
the coin controls are separated). It is the reason this file exists in the
shape it does, and its four findings are pre-committed here so that a result
cannot later be read against a rearranged prior.

### Finding 1 — the bar anchor decides the answer, and three of four definitions do not survive it

The broker's day starts 21:00 UTC in US summer and 22:00 in winter, so its H4
candles are 21/01/05/09/13/17Z. Aggregating on the Unix epoch instead builds
**different candles with different highs, lows and closes.** The first version
of this study used the epoch anchor. Recomputed on both, WITH-minus-AGAINST
mean R for the strategy books:

| definition | epoch anchor | broker anchor | verdict |
|---|---|---|---|
| structure (fractal-2 swings) | +0.552R (t +1.41) | +0.597R (t +1.33) | **stable** |
| EMA 21/55 | +0.054R (t +0.17) | −0.255R (t −0.80) | sign flip |
| prior-day midpoint | +0.454R (t +1.62) | **−0.811R (t −2.92)** | sign flip, and the only \|t\| > 2 on either anchor |
| Donchian(20) | +0.392R (t +1.22) | +0.003R (t +0.01) | collapses to nothing |

ADX(14) ≥ 25 over the same entries: **0 of 81** bars labelled TREND on the
epoch anchor, **73 of 81** on the broker's. A gate that never opens and a gate
that is open ninety percent of the time are not the same gate.

Structure versus prior-day-midpoint disagreement: **54% of entries on the
epoch anchor, 96% on the broker's** (78 of 81). The mechanism is visible in
what the definitions said: pdmid went from UP 23 / DOWN 58 to UP 59 / DOWN 22
when the day boundary moved.

A retraction is recorded with this: the epoch-anchor numbers were reported to
a5 as a result, including "the ADX gate would never open", and a5 made a
decision on them before the error was found. The corrected numbers are the
ones above.

### The bars Findings 1, 3 and 4 were measured on

Every number above comes from this file, on the desktop, fingerprinted so a
later reader can tell whether they are looking at the same bars rather than
the same path:

| file | bars | first | last | sha256 (first 16) |
|---|---|---|---|---|
| `data/bars/XAUUSD-15m.parquet` | 100,586 | 2022-06-16T10:30:00Z | 2026-09-17T13:00:00Z | `04ced3bbe30e78b9` |

`sha256:04ced3bbe30e78b9458ee87d4acf9847ecb8eddd13083785dc38349d2059ce20`,
window 2022-06-16 to 2026-09-17 inclusive UTC, and the file's own provenance:
`broker_symbol=XAUUSD.sc; server=VantageMarkets-Live 21; timeframe=M15;
exported_at=2026-09-17T13:23:48Z; contract_size=1.0`.

Recompute with `py -3.9 py/research/bars_fingerprint.py
data/bars/XAUUSD-15m.parquet --from 2022-06-16 --to 2026-09-17`.

The H4 and D1 series in Finding 1 are not fingerprinted separately because
they were not read: both were aggregated from this 15m file inside the
study, on the broker's anchor. The route's own `XAUUSD-4h.parquet` is a
different artefact and is not what these numbers came from.

**Why a digest and not the path.** The path names a different thing on each
machine: the VPS rewrites `data/bars/` every five minutes and the desktop's
copy is frozen. The digest is over the BARS - timestamps and OHLCV as bit
patterns - not the file, so it is unmoved by re-export and by `exported_at`,
and the window is bounded so that bars arriving later cannot invalidate a
reading that never saw them. See `docs/research/WHICH-BARS.md`.

### Finding 2 — this is why the rule keys on structure and on nothing else

a5's decision of 2026-09-18: **`htf-filter` keys on the STRUCTURE LABEL only.**
ADX and the efficiency ratio stay in the block as facts and stay out of the
rule. Finding 1 is the argument: structure is the one definition whose sign
and size held across the anchor correction, and a rule built on a definition
that flips with the bucketing is measuring the bucketing.

### Finding 3 — the sample holds one regime, so with/against is collinear with short/long

Across the 81 labelled entries on the broker anchor, structure said RANGE 55 /
DOWN 26 and **never once said UP**; EMA said MIXED 18 / DOWN 63; Donchian said
DOWN 56 / UP 1. On a sample like that, "WITH the trend" and "SHORT" are very
nearly the same variable, and no arithmetic separates them.

**Pre-committed consequence: no verdict on the with/against question is
permitted from a sample that contains only one regime.** This is a calendar
constraint, not an analysis choice — it is lifted when, and only when, the
accumulated sample contains entries under both an UP and a DOWN structure in
numbers worth comparing. Reporting a with/against effect before then would be
reporting the direction the book happened to trade.

### Finding 4 — NO-TREND was not the worst bucket, it was usually the best

On the broker anchor, the NO-TREND bucket returned +0.377R on the EMA
definition (against WITH +0.031R) and +0.622R on Donchian (against WITH
+0.061R) — the best bucket of the three in both cases. On structure it
returned +0.313R against WITH +0.325R, second by 0.012R. The prior-day
midpoint produces no NO-TREND bucket at all. The same ordering holds on the
epoch anchor.

So the honest prior for `htf-filter` is **not** that filtering to the trend
will help. On this desk's own trades so far, the bars the filter would have
removed were at least as good as the bars it would have kept. n is small, the
effect is not significant, and it is recorded here so that a positive result
is read as a surprise and a negative one is not read as a discovery.

## What is added to each prompt, and nothing else is

`htf-context` = base **plus one facts block** inside the existing
`MARKET CONTEXT — computed from the same bars, for convenience; none of it is
a signal` section, under that same framing. The coin clause is the **base**
one: this variant changes one thing, not two.

`htf-filter` = `htf-context` **plus exactly one sentence**, quoted here in
full and asserted verbatim by the selftest:

> ONE ADDITIONAL RULE FOR THIS BOOK - unlike the context above, this one is
> binding: do not propose LONG while the H4 swing structure label reads DOWN
> and do not propose SHORT while it reads UP; on RANGE, or whenever that label
> is absent, thin, stale or unavailable, both sides stay open.

The sentence names its own null behaviour deliberately. A rule that said only
"do not trade against the structure" would leave RANGE, an unconfirmed label
and an unreachable route undefined, and a model resolving those silently — three
different ways on three different bars — would make the book a mixture.

The block carries **no verdict**: the swing label is the only word in it, there
is no composite score, and `htf_context.py` computes nothing. Every number
comes off `GET /api/paper/htf` exactly as the route said it, because the
model and the Desk panel reading one implementation is the whole point of that
route.

### Two things the block must get right, and the selftest pins both

**Two ages, not one.** `confirmed_at_bar_ms` is not `computed_at_bar_ms`: a
fractal swing needs n bars after it before it is a swing, so on H4 the
structure label can be up to eight hours older than the ADX reading beside it.
The block prints both ages and says which is older and why. A block showing
one age tells the reader the label is as fresh as the facts, and it is not.

**`close_pct_of_prior_week_range` is not a 0–100 percentage.** Above 100 means
price left the prior week's range upward and below 0 downward, which is the
most informative thing the number ever says. The block renders it uncapped and
says so in its own text. A renderer that clamped it would read wrong exactly
when it mattered most. (Both flagged by d1, who wrote the route.)

**Three absence states, reported differently.** `unavailable` (the timeframe
is missing entirely), `thin` (the object is there and individual facts are null
because a warmup is not met), and `stale`. The route distinguishes the first
two on purpose and the block does not collapse them. `n/a` is the fourth value
of the log field, for the variants that never ask.

**Staleness is judged against the bar being decided, not the wall clock.** The
route reads a parquet; a stopped export would freeze these facts while the
desk went on trading, and a wall-clock threshold cannot tell a stopped export
from a weekend — on Monday morning the newest closed daily bar is Friday's,
sixty hours old and perfectly correct. The trader passes the timestamp of the
bar it is deciding on and staleness is the gap between the two, in H4 bars,
stale past 2.

An absent, thin or stale route **does not** fall back to the base prompt. That
would put context-absent decisions into a context-present book. The block says
which state it is in, and the `htf` field on every decision row records it, so
the bars that had the facts can be separated from the bars that did not.

## The cold-window confound, and the exact rule for it

**A new book starts with an EMPTY window.** `start_ai_runs.py` creates it,
the poller feeds it one bar, and the trader decides on that one bar while the
control — running for days — is deciding on forty. Observed in this book's
own first row: *"the lone 15m bar closed near its low…"*. At 15m it takes
about ten hours to reach forty.

So a difference measured over that stretch is partly the prompt and partly
the window, and nothing separates them after the fact unless it is decided
now. Raised by a5 from the first two rows, and written here before any
disagreement rate has been computed.

**THE RULE, pre-committed:** a decision row counts toward the disagreement
rate, the trade count, the win rate and every R figure **only when the book
saw a full window** — forty bars. Rows below that are excluded from the
headline numbers and reported separately as the warm-up, with their count
stated.

**It is measured, not counted.** The criterion is read out of the row's own
stored prompt, which carries `LAST <n> BARS of <market>:<tf>`, and the row is
excluded when `n < 40`. Checked 2026-09-18: that line parses from 55 of 55
decision rows in `ai-xau-ds-ctx`.

Deliberately not "the first forty rows". Position is a proxy for window size
and a bad one — a restart, a poller outage or a missed bar makes the fortieth
row and the fortieth bar different things, and the proxy would then exclude
warm rows and admit cold ones while looking exactly as tidy. The prompt says
what the model actually saw, so that is what decides.

**Both arms are filtered by the same rule**, including the control's own rows
on those bars. A comparison in which one side is filtered is not a
comparison.


## What will be measured, in this order

**Stage 1 — disagreement rate. The cheap kill, and it runs first.**
On bars where both the control and `htf-context` produced a decision and the
`htf` field reads `ok`, the fraction on which the two chose differently
(counting NONE as a choice).

> **Pre-committed:** if `htf-context` disagrees with the control on **fewer
> than 10% of shared `ok` bars over the first 100 such bars**, the facts are
> not changing the decision, the first claim is answered NO, and **both books
> are stopped and the result recorded.** `htf-filter` is not run, because a
> filter on facts the model demonstrably ignores measures the filter alone and
> can be tested more cheaply later if anyone wants it.

**Stage 2 — trade count, then win rate.**
Entries taken, and win rate, for each book against the control over the same
bars. Reported before any R figure, because a book that barely trades produces
an R number with no sample behind it and the R number is the one people read
first.

> **Pre-committed MINIMUM:** if `htf-filter` takes **fewer than a third of the
> control's entries over the same 100 decided bars**, it is abandoned and the
> result recorded as "the rule closed the book". The control's measured rate
> as of 2026-09-17 is 7 entries in 56 decided bars (12.5%), so a third of it is
> roughly 4 entries per 100 bars. This is the `ai-xau-opus-ctx` failure mode
> — 79 real answers, zero entries — and it is declared in advance so that a
> silent book is a recorded result and not an ongoing experiment.

**Stage 3 — R per trade and total R against the control, each with its N.**
No R figure is reported without its trade count beside it.

Every stage is reported for `htf-context` and `htf-filter` separately, each
against the base control and against its own coin. The coins are what say
whether a difference is the prompt or the bars.

## What would make this wrong, stated now

- **Pseudo-replication.** Three books deciding on the same bars are not three
  independent samples. Comparisons are per shared bar, and the N reported is
  the number of shared bars, not the number of rows.
- **One regime.** Finding 3. No with/against verdict from a one-regime sample,
  whatever the books show.
- **The anchor.** Any later analysis that recomputes these facts must use the
  broker's anchor. Finding 1 is what an epoch anchor costs.
- **Small n.** Every number in Findings 1 and 4 comes from 67 strategy trades.
  None of it is significant and none of it is quoted here as if it were; it is
  a prior, and its job is to stop a small positive result being read as a
  discovery.

## What comes after this one

**Queued, not started: `htf-context-v2` = base + H1 **and** H4 facts**, same
block shape with an H1 row added. The question it would ask is the one this
registration cannot: *does the finer ladder change decisions beyond H4
alone?*

The owner asked for H1 in the facts on 2026-09-18 and d1 is adding an `h1`
section to the route. **The block these two books read does not change.**
They are registered on H4-only facts, and a prompt edited mid-run makes the
record two campaigns under one id — the rule this desk already applies to
models and to accounts. So v2 waits until this registration reaches its
stage-1 kill or passes it, and then runs as its own books with its own coins.

That the route gains `h1` while these books ignore it is the correct
outcome, not a gap. The route serves everything; a registration reads the
subset it named.

One thing that had to be fixed to keep that true: the block used to print the
route's own `unavailable` sentence into the prompt, and that sentence is the
join of every missing timeframe's reason. An `h1` section would have put H1's
wording inside a registered campaign's prompt with nobody editing this book.
The block now says those things in its own words and the route's text goes to
the record instead.

## Preconditions — why nothing is started

Both are blocking. Neither is a code defect any longer, and the second cannot
be fixed by a commit at all.

1. **The route is not on `main`.** `GET /api/paper/htf` exists on branch
   `htf-facts` (three commits: `82aa047` the route and the exporter, `fa0b5f3`
   the record path, `aa71630` a comment) and nowhere else. Until it is merged
   and a binary carrying it is deployed, every call is a connection error and
   both books would run as base-with-an-apology-block.

2. **The bars the route needs exist in exactly one worktree, and no commit can
   ship them.** `data/` is gitignored (`.gitignore:8`, `/data/*`) and therefore
   per-worktree. d1 exported real H4 and D1 for XAUUSD into
   `E:/rust/flowdesk-htf/data/bars/` — 6,727 H4 bars from 2022-05-16 and 1,122
   D1, verified here, with bar opens landing on 01/05/09/13/17/21Z and 21Z
   respectively, which is the broker's anchor and not the epoch's. They are not
   in the main worktree and they are not on the VPS. Merging `htf-facts` ships
   the route and the exporter; **it ships no bars.**

   So the deploy carries an operational step that no commit can express: run
   `py/ingest/mt5_export.py --symbols XAUUSD --timeframes H4,D1` once on the
   VPS against the running terminal, after which the route answers. That step
   belongs in the deploy runbook, not in a diff, and it is the kind of step
   that gets skipped precisely because a green merge looks complete.

Until both are satisfied, starting either book would produce a campaign whose
every row read `htf: unavailable` — a context-absent book wearing a
context-present id, which is the exact failure this registration's own design
rules out everywhere else.

### Correction, recorded rather than quietly fixed

The first version of this section said the exporter could not produce H4 or D1
at all, and that the route's own "not exported yet" message named a command
that `py/ingest/mt5_export.py` would reject. **That was wrong, and it was wrong
in a way worth naming:** the route was read at `82aa047` and the exporter was
read in the main worktree. `82aa047` changes three files and the exporter is
one of them — on that branch `TIMEFRAMES` already carries `H4: ("4h", 14400)`
and `D1: ("1d", 86400)`, with `WINDOW_DAYS` extended by the same
86,400-bars-per-request identity. The remedy the route prints can be followed
as of the branch and could not be as of main.

Verifying a claim against the code is only verification if it is the same tree
the claim was made about. Corrected 2026-09-18 after d1 pointed it out.

## What the record carries

`fa0b5f3` adds `htf_bar_time` to the intent and opened lines on both fill
paths — the bar's open and its close, since a fill can happen at either. The
name is deliberately not the route's `computed_at_bar_ms`: it records what the
decision SAW, which is a different thing from what the route says now, and a
shared name would go wrong the first time someone compared a stored intent
against a fresh call. It is `null` when the decider read no context, and the
test asserts that case explicitly — a missing fetch must not look like a fetch
that found nothing, or the two arms of this experiment are a mixture.

Alongside it, the `htf` field on every decision row written by this trader
records the state the block was in: `ok`, `thin <fields>`, `stale <n> bars`,
`unavailable`, or `n/a`. Stage 1 below counts only bars where it reads `ok`.

## Signed

Written by Claude Opus 5 (session `backcom-vantage`, 2026-09-18) against the
route written by d1 and the scoping decision made by a5. Field names, units
and the two rendering traps were verified against `crates/fd-api/src/htf.rs`
at `82aa047`, not against the message describing it.


## Amendment 2026-09-18 ~16:00Z - layout and hold funnel, not a new book

From the traders' restart after this stamp, every row carries
`prompt_layout: "cache-v2"`: the SAME sentences in a different order (rules
and answer shape first, slow blocks next, bars last) so DeepSeek's prefix
cache can hit. Rows before it are layout v1 (the field is absent). The hold
question is now asked on events (`hold_trigger` on `hold_check` rows,
`hold_skip` rows for the bars not asked) with thinking off; it was and is
advisory, so no book's result moves. Both are stage 3-4 of
`docs/plans/2026-09-18-staged-ai-entry.md`. If the split by `prompt_layout`
shows a different entry rate, that is a finding about the order and belongs
here, not hidden by pooling the two.
