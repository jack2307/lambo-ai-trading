# 2026-09-24-what-the-record-cannot-see: repair the instruments before searching again

**Registered:** (commit time is authoritative) — before any agent is briefed and
before any corrected number exists
**Status:** open

## Where this came from, and why it is not a fifth angle

The owner's instruction after the designed-methods programme closed with zero
submissions: *"Bắt chúng nó nghiên cứu tiếp đi"* — keep them researching.

`docs/hypotheses/2026-09-23-designed-methods.md` pre-committed that if all eight
hypotheses failed, **the programme closes rather than spawning a fifth angle**, and
`docs/decisions/2026-09-24-designed-methods.md` closed it. That pre-commitment stands
and this registration does not break it: **not one of the four tasks below searches for
an edge.** Every one of them repairs or measures an instrument that the record's own
conclusions depend on.

The closing record named three of them itself:

> The three repairs above are worth more than another search: a null that controls for
> the instrument's own drift would change what this desk is able to believe, and the
> absence of one means every long-biased result in the record is unread. That, and the
> `max_hold_ms` cap, which has silently bounded every engine-exit measurement ever
> taken here.

This is not a detour from what the owner wants. Until the drift control exists, a
profitable-looking long-biased gold method **cannot be distinguished from buy-and-hold**
— so hunting more edges now would produce numbers nobody can read. Repairing the
instruments is the fastest route to an answer that means something, not a delay before
one.

## What is claimed, per task

Four independent claims, one per agent. Each is refutable on its own and none depends
on another's result.

### Task A — the drift-controlled null

**Claim.** A null that carries the instrument's own drift changes the verdict on at
least one published percentile in `docs/decisions/`.

**Why it is needed.** Both existing nulls take random or flipped sides and so carry
drift of about zero. Gold's unconditional drift is **+0.3946 ATR20 per five sessions
(t = +6.74)** on the recent window, and the price went from 1,200 to 3,700 over the data
on file. A long-biased multi-day gold method therefore clears both nulls on its side
ratio alone. This is not hypothetical: an agent measured a long-only weekly hold at
**+0.179 ATR8 per week, t = +3.26**, recorded that it *would clear the gate*, and refused
it because it is buy-and-hold wearing weekend flats.

**Falsifier.** Build the control, then re-run every published percentile whose method
has a side ratio outside 40–60% long. If no verdict changes, the claim is refuted and
the defect, while real, is inert on this record — which is itself worth knowing and is
published as such.

**What must be decided in the open, not silently.** There is more than one defensible
control and the choice changes the answer, so the agent states its choice and its
reason *before* running, and reports at least one alternative it rejected:
a side-ratio-matched random entry; a long-only random entry; a block bootstrap that
preserves drift and destroys timing. A control that removes the drift the method was
exposed to is not a control, and a control that hands the method its own side sequence
is not one either.

### Task B — the cost-matched null

**Claim, direction already fixed.** Registered yesterday in
`docs/hypotheses/2026-09-24-cost-matched-null.md`, which states the expected direction
before any corrected number: rows above 1.5 ATR **fall**, `vwap-fade` at 1.0 **rises**,
and **anything at exactly 1.5 does not move at all**.

**Falsifier.** A row at `stopAtr = 1.5` whose percentile moves. That means the fix
touched something other than the stop, and the work stops until it is explained. Also
refuting: any change to a gate figure, or a cost match achieved while the count match
drifts out of the 0.25 band — `matched_rate` must be recalibrated *after* the stop is
set, not before.

### Task C — what the four-hour cap has been costing

**Claim.** The `max_hold_ms` cap of four hours materially changed at least one verdict
in the record.

**Why it is needed.** `trading_rules_for` applies four hours to every market with no
override path, so **every `Exits::Engine` method ever measured here was force-closed
after at most 16 fifteen-minute bars, whatever its logic intended.** The one agent that
wanted multi-day holds had to use `Exits::Strategy` and take an UNMEASURED percentile
in exchange. Nobody has ever measured what the cap costs.

**Falsifier.** Make the cap overridable per market **without changing what any existing
receipt measured** — the default stays four hours, so every published number is
reproducible unchanged, and that reproduction is part of the task. Then re-measure a
declared set of engine-exit methods at 4h / 24h / 72h / 168h and report the verdict at
each. If no verdict changes at any horizon, the cap was never binding in practice and
the claim is refuted.

**This is the task with a live blast radius.** `max_hold_ms` is read by the paper loop
and by `mt5_executor`, so a careless change reaches two funded books on account
33708517. The default must not move, the config change must be additive, and the agent
does not deploy anything.

### Task D — is gold the wrong instrument for this cost model

**Claim.** At least one instrument on this desk's feed has a materially lower
`spread / stop` at a realistic stop than XAUUSD does.

**Why it is needed.** Every one of the four designed angles died on the same quantity,
and that quantity is a property of the **instrument and its spread**, not of any rule.
Nobody has measured it across instruments. Two figures already in the record show the
question is live and the answer is not obvious: gold's configured 0.28 costs **0.67% of
R** at a 41.57-point multi-day stop but **14.0%** at a 2.00-point intraday one; and
silver, whose configured spread is a tenth of gold's at 0.021, pays **2.70% of R** — four
times gold's share — because its stops are proportionally tighter.

**Falsifier.** Measure, for every instrument with usable history, the distribution of
`spread / stop` at a declared stop rule, and report it as one table. If gold is already
at or near the cheapest, the claim is refuted and the record gains the strongest
statement it can make: the cost problem is not an instrument-selection problem, and no
further instrument screening is warranted.

**This is a measurement, not a search.** It ranks nothing by profitability, proposes no
method, and its output is a cost table. An agent that comes back with a promising
strategy has done the wrong task.

## What is pre-committed across all four

1. **Every corrected number is published beside the original**, whichever direction it
   moves, and nothing is quietly replaced.
2. **No threshold moves.** The gate stays 30 trades / PF 1.2 / 0.05R and both nulls
   stay at the 95th. A repair that arrives with a relaxed bar is not a repair.
3. **Gate figures belong to the method, not its control.** If PF, expectancy or a trade
   count changes in tasks A or B, something other than the null was touched and the
   work stops.
4. **Nothing touches the funded books.** `ai-xau-ds-ctx` and `xau-macd-asia` on account
   33708517 are not reconfigured, not stopped and not deployed to by this programme.
   Whether `xau-macd-asia` keeps running after failing its gate on the most recent three
   months is a standing decision for the owner and is not taken here.
5. **The withheld year stays sealed.** `data-sealed/` ends 2025-09-23 and XAUUSD 15m
   2025-09-23 → 2026-09-17 remains unread. **No task here spends it**, and none needs
   to: tasks A, B and C re-measure windows the record already published, and task D is
   a cost table. The seal is worth one honest programme and this is not it.
6. **A defect found is published, not smoothed over.** Eight were found in two days and
   all eight are in the record. The same applies to a defect found in a repair.
7. **A task that refutes its own claim has succeeded.** "The defect is real but inert on
   this record" is a result, and reporting it is worth more than manufacturing a
   verdict change.

## Multiplicity

**There is no multiplicity to declare, because nothing here is a search.** No task
proposes a trading method, no task selects a winner from a field, and no task is scored
against a percentile. Tasks A and B change how existing numbers are read; C and D
measure quantities nobody has measured. The count of hypotheses in the edge-hunting
sense is **zero**, and that is the point of running this instead of a fifth angle.

## What would make this whole programme wrong

- A repaired null that cannot reproduce an unaffected row unchanged.
- A per-market hold cap whose default silently differs from four hours.
- Any of the four tasks coming back with a proposed trading method.
- A corrected percentile that flatters a construct this desk likes and was checked by
  nobody. That is the temptation the count-match repair was registered against on
  2026-09-23 and it has not gone away.

## What happens next, stated now

If the drift control changes verdicts, **the record is re-read before anything else is
searched for** — and some of what this desk believes will turn out to have been
buy-and-hold. If the hold cap turns out to have been binding, the low-frequency space
has never actually been measured here and becomes the honest place to look next, with
its own registration and against the sealed year. If neither moves and gold is already
the cheapest instrument on the feed, then explanation 2 is not merely supported, it is
closed, and the desk's question stops being "which rule" and becomes "these costs, at
this frequency, on this instrument, cannot be beaten by anything measurable here."
