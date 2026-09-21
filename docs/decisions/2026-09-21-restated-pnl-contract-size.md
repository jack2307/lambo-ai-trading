# `eur-hours` never made $177.51: a config correction restated a stored trade, and a trade now carries the contract size it was sized under

**Written:** 2026-09-21
**Status:** a defect, found and fixed, and a correction to the record of one
paper book. The convention half is an instance of
`2026-09-17-unit-carrying.md` and adds nothing new to it. No verdict, no
backtest, no receipt and no registration changes; one paper book's headline
figures do.
**Scope:** `crates/fd-backtest/src/engine.rs` (`Live`, `Trade`,
`close_position`), `crates/fd-api/src/paper.rs` and `dto.rs` (the status and
detail routes), the new `config/exclusions.toml`, and the paper book
`eur-hours`.

## The claim, plainly

**`eur-hours` reported +$176.85 on one trade, and +$177.51 over the book. It
was never a result.** The book made about eighteen cents on that trade. The
position it reports having held — 256.3 lots of EURUSD at 1.1546, $295,920 of
notional against $100 of equity, about 2,960× the account — is one it could
not have held and did not hold: the desk's own notional cap is 300%, and the
run's `sized_down` count is 0 because at the contract size it was actually
sized under the position was 296% and passed.

## What happened, and the two hours it happened in

| | |
|---|---|
| book started | 2026-09-16 **03:26 UTC** (`fills.jsonl`, the third `started` line; the two before it were a $10,000 book) |
| position opened | 2026-09-16 **07:00 UTC**, SHORT 256.3 lots at 1.1546, stop 1.1585 |
| commit cd92998 | 2026-09-16 15:53:49 +0700 = **08:53 UTC** — "fix EURUSD/XAGUSD contract sizes" |
| position closed | 2026-09-16 **15:00 UTC**, 1.15391, "window closed" |
| booked | `pnl_usd` **176.85**, into `equity` **276.85** |

Commit cd92998 changed `[markets.eurusd.trading] contract_size` from 1.0 to
the measured **1000.0**, and `[markets.xagduka.trading]` (the silver block,
`symbol = "XAGUSD.sc"`) and `[markets.eurduka.trading]` with it. Nothing about
that commit was wrong. 1000.0 is what `symbol_info("EURUSD.sc")
.trade_contract_size` returns on the account, and its message says so.

The position was **sized** under 1.0 and **booked** under 1000.0, with the
correction landing in the one hour and fifty-three minutes between.

## The arithmetic, both ways

Sizing is `lots = risk_usd / (risk x contract_size)` and P&L is
`points x lots x contract_size`. At **1.0**, on the book's own numbers:

    risk        1.1585 - 1.1546            = 0.0039     (39 pips, an ordinary EURUSD stop)
    risk_usd    1% of $100                 = $1.00
    lots        1.00 / (0.0039 x 1.0)      = 256.3      matches the record exactly
    notional    256.3 x 1.0 x 1.1546       = $295.92    against 300% of $100 = $300.00 -> passes
    points      1.1546 - 1.15391           = 0.00069
    P&L         0.00069 x 256.3 x 1.0      = $0.18

Every figure reconciles. At **1000.0** the same lots give
`0.00069 x 256.3 x 1000 = $176.85`, which is the number the book holds, and
`lots x risk x 1000 = $999.57` of risk on a $100 account, which is the
impossibility the record was carrying without saying so.

**`r` was right the whole time.** It is `points / risk`, the contract size
cancels out of it, and the trade records **0.1768 R**. A book reporting 0.18 R
and $176.85 on $100 of equity was stating two contradictory things at once,
and only the dollar one was read. That is the same disease as every entry in
`2026-09-17-unit-carrying.md`: the record agreed with the code, so nothing
downstream could contradict it.

## The defect is not the old config value

The old value was fixed in September and the fix was right. The defect is
that **a stored trade's lots were computed under one contract size and its
P&L under another**, and nothing in the record said which. cd92998's own
message argued the error "cancels out of every result … so the product is
invariant". That is true and it is true only while both readings are the SAME
reading. A trade that spans a correction has two, and then nothing cancels.

The same is true going the other way. `/api/paper/status` recomputed dollars
per point, and after 2026-09-21 the introducing-broker rebate, from
`state.trading_rules(&config.market)` at the moment of the request — so every
stored trade on the euro and on silver had those figures restated by the same
commit, silently, from a basis it never traded on. The rebate decision of the
same day (`2026-09-21-rebate-credit-line.md`) already names "the spread
repricing" as a change that is coming; without this fix that repricing would
have done to every credit what cd92998 did to this P&L.

## What is persisted, and what is not

**`contract_size` and `spread`, on the position and on the closed trade.**
Recorded at sizing, carried through `close_position`, used for that trade's
P&L, its dollars per point, its mark against the live tick, and its rebate.
The market's current value is read **only** for a trade that predates the
field, and such a trade carries `null` rather than a fallback, so a reader and
the API can both tell. `null` is not `0` and it is not "the current one".

**`lot_step` is NOT persisted, and that is a decision rather than an
oversight.** `lot_step` acts exactly once, at sizing, to round `raw_lots`
down; its output is `lots`, which is already stored on the trade, and nothing
anywhere recomputes anything from it afterwards. Persisting it would add a
field with no reader — precisely the "rename sweep with no failing test behind
it" that `2026-09-17-unit-carrying.md` argues against. `min_lot` is out for
the same reason. If a future figure is ever derived from either *after* the
fill, that figure is what needs the field, and it should be added then with
the test that reads it.

`spread` IS persisted although the cost is already spent inside `entry_price`
and `exit_price`, because the rebate line reads a spread at the moment
somebody looks at the book, not at the moment the book paid it.

**Estimated, not blended.** A trade with no recorded basis is still priced —
a rebate line nobody can compute costs a line on a screen and does not gate a
decision that spends money, so rule 3 of the unit-carrying convention does not
bite here — but it is counted as `estimated`, beside `exact` and `unpriced`,
in the three counts the rebate work shipped on 2026-09-21 and never collapsed
into one. `estimated_trades` on a paper book used to be asserted as always
zero. It is not any more, and the claim that justified it ("the engine took
the configured spread out of the fill") was only ever true while the
configured spread was still the one that had been taken out.

## What is now true of `eur-hours`

The trade is **excluded, not deleted**, following the precedent of
`docs/hypotheses/2026-09-18-plan-entry.md` (amendment 2026-09-18 16:55Z),
where four books each took one bad trade from a process on the wrong binary
and the answer was a pre-committed exclusion written into the registrations:
*"A campaign whose first trade is quietly missing is worse than one that says
which trade it is not counting."*

`config/exclusions.toml` names the run, the entry time and the reason. The
trade stays in `trades.jsonl`, stays in `fills.jsonl`, and stays in the book's
own `last_fills` on `/api/paper/status` with `excludedReason` on it. What
changes is that `trades`, `net_usd` and `profit_factor` are computed without
it and the response carries an `excluded` block saying how many it left out,
what they came to, and what the equity would be without them.

An excluded trade also carries no rebate, and that is the point rather than a
side effect: the credit is `share x spread x lots x contract_size`, so on a
trade excluded for its size it is wrong by exactly the same factor — this one
priced at **$16.15** of credit on a $100 book. A desk that will not count a
trade's P&L must not print a confident credit beside it.

So the book's record reads: **one closed trade, taken 2026-09-16, excluded;
no counted trade, no counted P&L.** Not a loss and not a win — no result.
Served against a copy of the store on 2026-09-21 the route says exactly that:
`trades 0, net_usd 0.0, profit_factor null, equity 276.85`, with
`excluded: { trades 1, net_usd 176.85, equity_usd 100.00 }` and the fill
itself still in `last_fills`.

**Two things are deliberately NOT done.**

The stored `pnl_usd` of 176.85 is left where it is. It is what the process
booked, it is what `equity` was built from, and rewriting a number in a log to
what it should have said is the one repair this desk does not make. It is
excluded and labelled instead.

`eur-hours`'s ledger equity is left at **276.85**, and this has a live
consequence that is the owner's call and not mine: the book sizes its next
position at 1% of 276.85, so about $2.77 of risk on an account that started at
$100 — 2.8× the intended risk, on a market where 1% of equity buys a large
number of lots. `excluded.equity_usd` publishes what the ledger would be
without the trade (100.00 on the store read here). Restating a running book's
ledger, or restarting the book from $100, changes what it will do next and is
a decision about the book rather than about its report.

## Every contaminated book and trade

The sweep is over every `state.json` and `trades.jsonl` under `data/paper/`
(and `data/paper/_stray-launch-20260917/`), on the store read on 2026-09-21,
testing each stored trade two ways: is its market one cd92998 touched
(`eurusd`, `eurduka`, `xagduka`), and does
`lots x |entry - stop| x contract_size_now` — the risk the trade implies at
today's contract size — make sense against the book's equity.

**Sixteen books. Sixty-six stored trade rows — 41 on the main books and 25
on their shadows. One is contaminated.**

| book | market | trades | verdict |
|---|---|---|---|
| `eur-hours` | eurusd | 1 (+1 shadow) | **CONTAMINATED**, below |
| `ai-xau-ds-ctx`, `ai-xau-ds-ctx-coin`, `ai-xau-sol-ctx`, `ai-xau-sol-ctx-coin` | xauusd | 4 each | clean |
| `ai-xau-opus-ctx`, `ai-xau-opus-ctx-coin`, `xau-ict-5m` | xauusd | 0 | clean |
| `xau-box-5m` 8, `xau-keltner-asia` 8, `xau-stoch` 12, `xau-ema` 6, `xau-close` 4, `xau-evening` 4, `xau-macd-asia` 4, `xau-rsi2-ny` 2 | xauusd | as listed | clean |

The contaminated trade, in full:

| | |
|---|---|
| book | `eur-hours` (and its `shadow`, the same fill) |
| entry | 2026-09-16 07:00:00Z, SHORT, 1.1546, stop 1.1585 |
| exit | 2026-09-16 15:00:00Z, 1.15391, "window closed" |
| lots | 256.3 |
| `pnl_usd` as stored | **176.85** |
| P&L at the basis it was sized under | **$0.18** |
| `r` as stored | 0.1768 — correct, and the only figure that was |
| implied risk at today's contract size | **$999.57** on a $100 book |

Every gold book reconciles: contract size 1.0 for `xauusd` was measured
correct by cd92998 and unchanged by it, and each trade's implied risk sits at
$0.48–$0.99, which is 1% of a ~$100 book. Nothing on `eurduka` or `xagduka`
has ever been paper-traded. The two earlier `eur-hours` fills in `fills.jsonl`
(2026-09-14 and 2026-09-15, 25,651.98 and 25,355.19 lots on a $10,000 book)
were opened AND closed before 08:53 UTC on 2026-09-16, so both of their
readings were 1.0 and each is internally consistent; they belong to a book
epoch that was stopped and reset and they are not in `trades.jsonl`, so they
are in no figure the desk reports. Their lot numbers are the separate,
already-fixed defect cd92998 was written for.

**What this sweep could not see.** It is the store on this workstation, whose
newest paper trade is 2026-09-17. The desk that has been trading since then is
on the VPS (`103.19.29.194`, `C:\flowdesk\data\paper`), and the `eur-hours`
figure that started this — +$177.51 over **three** trades, the other two of
0.53 lots — is from there: two further fills taken AFTER the correction, sized
and booked at 1000.0 alike, and therefore clean. The sweep must be re-run
against that store before this document's count is claimed to be the desk's
count, and the exclusion file must be deployed there for the book's figures to
change.

## What would reopen this

- **A second trade found to span a config correction** — anywhere, on any
  market. The mechanism is now impossible for a trade closed after
  2026-09-21, so a new instance would mean a path that builds a `Trade`
  without going through `close_position`.
- **A figure derived from `lot_step` or `min_lot` after the fill.** That is
  the argument for persisting them, and it does not exist today.
- **The owner deciding what happens to `eur-hours`'s ledger.** Excluding the
  trade from the report does not change what the book sizes from.

## What this does not say

- **It does not say the euro's European hours is disconfirmed.** That
  registration lives on its backtests
  (`2026-09-14-fx-local-hours-sign.md`, `2026-09-14-fx-local-hours.md`) and
  nothing here touches them. cd92998's argument that the error cancels out of
  a completed BACKTEST is correct: a backtest reads one config from start to
  finish, so both readings are the same reading. What it says is that this
  book's live record is now empty rather than positive, and that anyone who
  read the Overview between 2026-09-16 and today saw a number that was not a
  result.
- **It does not change the cost model.** No configured number moved. 1000.0
  and 50.0 stay as measured, pinned by
  `the_euro_and_silver_contract_sizes_stay_as_measured`.
- **It does not claim the exclusion mechanism has been used before.** It is
  new code implementing an answer this desk reached in prose three days ago.
  An exclusion added because a result was disliked, rather than because a
  trade is defective, would be a deletion with extra steps, and the file says
  so at the top.
- **It does not say the desk's other restated figures have been found.**
  Dollars per point and the rebate were both being recomputed from current
  config for every trade on every market. They are correct now for trades
  that carry their basis, and remain estimates for ever on the sixty-six
  that do not.

## Gates

`cargo test --workspace` (all pass), `python py/live/rebate_selftest.py` (all
pass), `node ui/scripts/contract.check.mjs` (231 paths hold). New:
`crates/fd-backtest/tests/contract_size.rs` reproduces the eur-hours position
and closes it under the corrected config, asserting the trade books $0.18 and
not $176.85; `carried_basis` in `crates/fd-api/src/paper.rs` pins
exact-vs-estimated and parses the shipped exclusions file;
`a_trade_held_out_stays_in_the_record_and_leaves_the_headline_figures` in
`crates/fd-api/tests/paper.rs` runs one tape twice and asserts the trade is
out of the count, in the fills, and that the ledger is not restated.
