# The introducing-broker rebate is a line beside the book, not a change to the cost model

**Written:** 2026-09-21
**Status:** a convention, adopted, and the code that implements it. Not a
hypothesis and not a measurement — it changes no verdict, no receipt and no
number that already existed. It says how the owner's IB credit is counted and
where it is allowed to appear.
**Scope:** `config/accounts.toml`, the paper books' `/api/paper/status`, the
account snapshots `py/live/mt5_executor.py` writes, and the Analytics screen.

## What was asked

The owner runs an introducing-broker business and is the IB on his own
accounts, so part of the spread his trading pays comes back to him. His
instruction: *"tinh them tien backcom se bang 45% cua spread"* — count the
Backcom money too, it will be 45% of the spread.

He was shown three ways to do it and chose **a separate credit line, applied
to live and to paper**. He explicitly did not choose folding it into the cost
model. That choice is the whole design and it is worth stating why it is the
right one: a rebate inside the spread makes every existing receipt
unreproducible and makes every future result a mixture of two things a reader
can no longer separate. Beside the book, a reader can always ask how much of
a result is the strategy and how much is the commercial arrangement.

So: **nothing that had a number has a different number because this shipped.**
`config/default.toml`'s `spread` still charges what the venue charges,
`net_usd`, `equity`, `pnl_usd` and `realised` all still mean what they meant,
and every document in `docs/decisions/` still reproduces. Two tests exist for
exactly that sentence and nothing else:
`the_rebate_is_a_separate_line_and_changes_nothing_that_existed` in
`crates/fd-api/tests/paper.rs`, and section 2 of `py/live/rebate_selftest.py`.

## Where the rate lives, and the argument against the other place

`config/accounts.toml`, in a top-level `[rebate]` table, `share_of_spread =
0.45`.

Not `config/default.toml`. That file is the COST MODEL: what the venue charges
anybody who trades this symbol, measured from the terminal. The rebate is not
a property of gold, of Vantage's spread or of any market — it is a term of the
owner's own IB agreement and it attaches to the login, exactly like
`real_money` and `lot_scale` already do in `accounts.toml`. A second desk
trading this same config with no IB link would pay the same spread and receive
nothing.

Top-level rather than per-account, and that is the part of the argument that
can be lost. There is one agreement today, and a paper book belongs to no
account at all — it is a market and a strategy, and it would have nowhere to
read a per-account rate from. So the rate sits beside `[prices]`, the other
table in that file that is a fact about the DESK. An account that negotiates
its own terms adds its own key later; nothing has to move for it.

`config/accounts.toml` also carries a commented-out `per_lot` form, with the
reason it is not implemented. An IB rebate is USUALLY quoted per standard lot
— the owner's own book is $10–12 per standard lot
(`2026-09-13-close-reopen-drift.md`) — and the two forms are not one number in
two hats. A share of the spread moves with the spread and pays nothing when
the spread is nothing; a per-lot figure is flat. A standard lot is 100× this
workspace's gold contract, so $11 per standard lot is $0.11 per lot here, and
typing the broker's own number into a file whose lots are ounces would be out
by a hundred. It is left unimplemented rather than guessed at, because an
invented term produces a confident figure and a confident wrong figure is
worse than no line at all.

## The arithmetic, on the account the desk actually trades

XAUUSD.sc on the funded cent account (33705331): contract 1 ounce, tick 0.01,
tick value 1.00 USC — so **one unit of price is 100 USC per lot**, which is
not the contract size and is the conversion the `notional_of` defect of
2026-09-17 got wrong. At the configured 0.28 spread and 0.07 lots:

| | |
|---|---|
| spread paid, round turn | 0.28 × 100 × 0.07 = **1.96 USC** |
| rebate at 45% | **0.88 USC** |
| as a share of the book's risk (~0.82 USD a trade) | **about 0.011 R** |

ROUND TURN, and the word decides the number. A position pays the spread once,
between the ask it enters at and the bid it leaves at; `apply_costs` charges
half on each side and the two halves are one spread. Taking 45% of each side
would be twice the truth.

## What the rebate means

On the recent-year screen `stoch-reversal` is 1297 trades, profit factor
0.912, expectancy **−0.044 R** (`2026-09-13-recent-year-screen.md`,
`docs/research/VERDICTS.md`). The rebate moves that to about **−0.033 R**, and
to about **−0.023 R** even if the 45% were paid per side rather than per round
turn.

Still negative. The rebate rescues nothing, and the risk role already said the
general form of this on 2026-09-13: *"Rebate never pays for trading; only the
edge does."* This line exists so the owner can see money he is genuinely owed,
not so a losing book can be re-read as a winning one.

## How each side computes it

**Paper (Rust).** Every closed trade has a known spread — the configured one,
which is what the engine actually charged the fill — known lots and a known
contract size, so `0.45 × spread × lots × contract_size` is exact arithmetic
on a known cost. `/api/paper/status` carries a `rebate` object on each run
beside the untouched `net_usd`, and each `TradeDto` carries its own
`rebateUsd` beside its untouched `pnlUsd`.

**The account (Python).** `history_of` sums `realised` from the deals matched
by magic and now computes the credit beside it, into `broker.json`'s `rebate`
and each fill's own `rebate`. The money conversion is `tick_value / tick_size`
read from the symbol, never `contract_size`.

## The honest difficulty, and it is not solved, it is labelled

**The desk did not record the spread at the moment of a real fill.**
`broker.json` carries `bid` and `ask` as of the SNAPSHOT, which is whenever
the poll happened to run and has nothing to do with when a trade filled. So
for every trade already in history the credit can only be worked out from the
CONFIGURED spread — and that is an estimate that runs high: the spread logger
has measured XAUUSD.sc at a median of 0.210 against a configured 0.280
(`config/default.toml`, the block above `[markets.xauusd]`), so an estimate on
this market is about a quarter above the truth.

Two things follow, and both are in the code rather than in this paragraph.

**Going forward the spread is captured at the order.** `send()` — the one
choke point every order this process sends passes through — reads the quote
immediately before `order_send` and keeps it against the deal ticket the
broker returns, in `data/live/<account>/<run>/spreads.json`. Keyed by deal
rather than by position, because a deal ticket is what both ends hand over and
"position id equals the opening order's ticket" is a coincidence of this
broker. It survives a restart, which is why a restarted executor does not turn
every exact trade back into an estimate.

**What it still cannot know is labelled, not averaged.** A stop or a target
fires with no order from this desk and no quote to read; so does the fill of a
pending order placed with `--mirror-pending`. Those exits stay estimates for
good. So every figure carries three counts —

    exact      priced from the spread the trade actually paid
    estimated  priced from the configured spread, because none was recorded
    unpriced   no usable basis; credited nothing, and COUNTED

— and they are never collapsed into one number. An average of a measured value
and a guessed one is a guessed value wearing a measurement's clothes, and this
desk treats a field that mixes the two without saying which as a defect
(`2026-09-17-unit-carrying.md`, rule 5: the unit — and here the provenance —
appears in the record and not only in the code). On a book that mostly exits
at its stop, `estimated` will stay the larger number, and the screen says so
where the figure is read rather than in a tooltip.

`exact + estimated + unpriced` equals the trade count; `contract.check.mjs`
asserts it on a served sample, because a total that silently dropped what it
could not price would read as complete.

## Units

Every paper figure is USD and named `*_usd`, beside the `account_currency` and
`units_per_usd` the run already carries. Every account figure is in the
ACCOUNT's currency — USC on the funded cent account — and `broker.json`'s
rebate block carries `currency` in the record beside the numbers. The two are
never summed: the Analytics page builds one list per book on screen and
decides its `Money` once, which is the rule that already stopped a paper
dollar being added to a real cent.

## What would reopen this

- **The owner supplying the real per-lot figure.** That is the likelier form
  of the agreement, it is a different number from a share of the spread, and
  the commented block in `config/accounts.toml` is where it goes.
- **The spread repricing.** The configured 0.28 is above every spread the
  logger has ever observed on XAUUSD.sc. When that amendment is written, every
  ESTIMATED credit on this desk changes with it — which is an argument for
  making the exact count grow, not for repricing sooner.
- **A second account on different terms**, which is when `[rebate]` stops
  being top-level.

## What this does not say

- It does not say the rebate makes anything profitable. See above: −0.044 R
  becomes −0.033 R.
- It does not say the account's rebate figure is measured. Most of it is an
  estimate today and the counts say so on every surface that shows it.
- It does not claim the credit has been received. This is what the terms say
  the desk is owed on the spread it paid; nothing here reconciles against a
  payment, and a rebate statement from the broker is the only thing that
  could.
- It says nothing about the IB business's own rebate income from clients.
  That is `E:\nodejs\backcom-vantage`'s subject, not this desk's.
