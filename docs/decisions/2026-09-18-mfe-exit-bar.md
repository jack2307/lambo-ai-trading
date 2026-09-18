# The exit bar was never counted, so every excursion this desk has measured was a lower bound

**Written:** 2026-09-18
**Status:** a correction, applied. It changes `mfe` and `mae` on every closed
trade and nothing else — no verdict, no result, no fill, no price.
**Scope:** `close_position` in `fd-backtest`, which every exit in the engine
and in the live book passes through, and the parity gate's treatment of two
fields.

## The defect

`track_excursion` runs in the `else` branch of the exit check: when a bar
closes a trade, the exit branch books it and returns, and that bar's excursion
is never recorded. So `mfe` and `mae` ended at the bar BEFORE the exit, always.

The record then disagreed with itself. `r > mfe` is impossible by definition —
`r` and `mfe` are the same distance measured to two different points, and the
exit is one of the points `mfe` is a maximum over — and the golden file is full
of it:

| | |
|---|---|
| golden trades (`tests/golden/btc-backtests.json`) | 112 |
| with `r > mfe` | **26 (23%)** |
| worst | `ema-cross`, r = 1.7826, **mfe = 0** |

A trade that made 1.78 R with a best excursion of zero. Every MFE-based
measurement this desk has ever made was a lower bound, and the ones that
matter are target exits, where the gap is largest by construction: the trade
leaves at its own best price and that price was the one not counted.

## It was reported as one path and it was four

The report was "check_exit runs before the excursion is tracked", which is
true and is one of the places a trade gets booked. The engine has four — the
stop/target/guard exit, a strategy signal exit, the end-of-data close, and the
run's own close — and `PaperBook::book` delegates to the same function, so the
live book adds three more.

Patching the reported one left `donchian-breakout[3]` on gold finishing at
−0.6126 R with a recorded worst excursion of −0.5925 R: the same impossibility,
in the other direction, on a path nobody had mentioned.

The correction lives in `close_position` instead. Every exit already passes
through it; that makes it the one place it cannot be forgotten, including by
whoever adds the fifth path.

**This is why the check was written as a property and not as a comparison.** A
test that asserted the reported symptom would have passed after the first
patch. `a_trade_never_ends_better_than_its_best_moment` runs the backtests and
asserts `mae <= r <= mfe` on every closed trade of both markets — 222 of them —
and it failed until all the paths were covered.

## To the exit price, not to the exit bar's range

The excursion the exit bar contributes is measured to the price the trade
actually left at and no further.

Reaching the exit price is a fact: the trade left there. The rest of that bar's
range may have happened after the position was already out, and intrabar order
is exactly what a bar does not record. Crediting a stopped-out long with its
exit bar's high would be inventing an excursion the position was not in for —
the same class of number as the three this desk removed on 2026-09-17 by
giving `open_position` and `Guards::calendar_refusal` the fields they actually
read.

It is one signed number and it lands in the right place without a branch: a
long stopped out left below its entry, so it extends `mae` and leaves `mfe`;
a long at target left above, so it extends `mfe`. The price used is the one
the trade is BOOKED at, costs included, so `r <= mfe` is exact and a trade
whose best moment was its exit reads `mfe == r`. 52 of the 222 do.

## The oracle has this bug, and the goldens are not edited

This is the point to read slowly, because it is the one that could be
misread.

`tests/golden/` is the JavaScript original's output. The port's plan makes it
read-only and frozen: it is the authority, and `CLAUDE.md` says a failing gate
is the result and is never re-tuned to pass. The oracle has this defect too —
that is where the 26 violating trades come from — and `parity.rs` compared
`mfe` and `mae` against those numbers directly.

So fixing this makes the gate fail against a file we are not allowed to
regenerate. Three ways out were weighed: exempt the two fields, replace the
comparison with a relation, or fix the oracle and re-export. The third would
change what "golden" means more than the second does, since the frozen oracle
is the only thing that makes the file evidence rather than a fixture.

**What was done:** equality on `mfe`/`mae` is replaced by a relation, and the
golden file is untouched.

- Rust's `mfe` must be **at least** the oracle's, and its `mae` **at most** —
  the fix adds one candidate to a max and to a min, so it can move them no
  other way, and a regression that shrank an excursion still fails.
- Rust's `r` must be **at most** its `mfe` and at least its `mae` — the
  definitional check the oracle itself violates.

**This is not the gate being loosened to pass.** Equality against a number
known to be wrong is weaker than it looks: it pins a value nobody can defend
and cannot express the property that matters. The relation keeps the oracle as
a BOUND and adds a check the old equality could not state at all. A change
that inflated an excursion past the trade's own result passes the old check
and fails this one.

## The precedent, and the cost of using it

This is the **second** deliberate divergence from the oracle. The first is
recorded in this project's memory: `fd-api` does not reproduce the Node API's
`stats`, which describes the source minute series rather than the array it
returns.

Two is a pattern and the pattern has a price, stated here rather than
discovered later: the golden files are now a file where most fields are the
oracle's and two are not, and that distinction lives in a comment on
`Diffs::check_excursion` and in this note. On 2026-09-17 this desk found five
comments that had stopped being true. Anyone adding a third divergence should
consider whether the frozen-oracle arrangement is still carrying its weight,
because the answer stops being obvious somewhere around here.
