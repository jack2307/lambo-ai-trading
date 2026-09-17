# The five bugs this desk shipped in units are one bug, and the trap list is not the cure

**Written:** 2026-09-17
**Status:** a convention, adopted. Not a hypothesis and not a measurement — it
changes no verdict and no result. It says how a number carries its unit in this
repo, and why prose alone has already been shown not to work.
**Scope:** every number that is money, a price, a duration, a timestamp, a rate
or a size, at the point where it crosses between two systems.

## Why

Five defects were found in one day, 2026-09-17, four of them on the path that
sends orders to a funded account. Three were the same disease:

| Where | Compared | Against | Result |
|---|---|---|---|
| `notional_of` | USD (`lots x contract x price`) | USC (`account_info().equity`) | the notional ceiling admitted 1000x while reading `10.0` |
| `already_taken` | UTC (the book's `entry_time`) | broker-server time (`d.time_msc`) | a guard that never once matched in its whole life |
| `send()` | — | — | `volume` logged the REQUESTED size under a name that reads as filled |

In none of these was the code merely wrong. **The record agreed with it.** The
ceiling printed a bare ratio with no currency beside it; `already_taken` logged
neither clock; every order line called the requested size `volume`. So nothing
downstream could contradict the code, and all three survived review by people
looking straight at them. A record that is confidently wrong is worse than one
that is missing, because a missing record sends you to check and a wrong one
does not.

## The part that should have been noticed sooner

`CLAUDE.md` already lists four instances of this same disease, as four
unrelated bullets in "Traps this codebase has already paid for":

- **CLAUDE.md:43** — `symbol_info.swap_long` is in POINTS, not dollars
  (`swap_mode=1`). A number that reads as money and is not.
- **CLAUDE.md:46** — the MT5 account currency is USC; `contract_size` 1.0 on
  `XAUUSD.sc` is one ounce, so per-lot figures are per ounce and a standard lot
  is 100x. This is *exactly* the `notional_of` bug, written down before it was
  written.
- **CLAUDE.md:51** — the OTL feed renders UTC+7 including its `+00:00`-suffixed
  expirations. A field that does not merely omit its zone but asserts the wrong
  one.
- **CLAUDE.md:58** — MT5 returns bar times on the broker's clock, UTC+3/+2.

Four bullets, one disease, and nobody had said so. That is the first half of
the finding.

The second half is harder: **writing the trap down did not prevent it.**
CLAUDE.md:58 says MT5 times are on the broker's clock. `already_taken` compared
`d.time_msc` against a UTC stamp anyway, and the module's own `history_of`
docstring warns about the same distinction twenty lines above the code that
ignored it. The warning was in the file, in the right file, near the right
code, and the bug shipped and then sat inert for its entire life.

So prose is necessary and is not sufficient. What has actually held is the
remedy applied to the one instance that has never regressed.

## What has worked, three times, without being named

Three separate places in this repo arrived independently at the same answer:

- `config/default.toml` + `crates/fd-ingest/src/otl.rs` — the feed offset is
  data (`utc_offset_hours = 7`), measured rather than derived, and pinned by a
  test at `otl.rs:340` whose name says *"the workspace config carries the
  measured feed offset"* and whose assertion message is `"utc_offset_hours must
  stay 7 until re-measured"`.

  That test is the single best piece of evidence in this document, and it is
  worth reading its comment rather than this paraphrase. The offset **was**
  lost once after being fixed — during a commit split — and a collector wrote
  *15,000 mis-stamped prints* before anyone read the log line. The test was
  added in response. It has not come back since, which is the only claim of
  that kind anywhere in this repo: every other instance of this disease was
  written down in prose and recurred anyway.
- `crates/fd-core/src/config.rs` + `PaperRun` — `account_currency` and
  `units_per_usd` travel with every run, so the client can show the account's
  own number without the conversion ever entering the arithmetic.
- `py/live/mt5_executor.py` (2026-09-17) — the server clock offset is measured
  from the terminal at runtime, refuses rather than guesses when it cannot be
  read, and is written into `broker.json` every poll.

Three arrivals at one rule is a convention the codebase is already trying to
have. This writes it down.

## The convention

**1. The name carries the unit, or a field beside it does.**

`starting_equity_usd`, `units_per_usd`, `utc_offset_hours`, `swap_usd`,
`*_ms` — these are the existing good examples and there is nothing new to
learn from them. The rule is only interesting where a second unit for the same
quantity actually exists in this repo, which today means:

    USD vs USC          money on a cent account
    UTC vs server time  Vantage is UTC+3 in NY summer, UTC+2 in winter
    UTC vs UTC+7        the OTL feed
    points vs dollars   swap, and anything read from symbol_info
    per ounce vs per standard lot    contract_size 1.0 vs 100
    lots vs units       volume against contract size

Where a name cannot carry it, the unit rides in a sibling field
(`account_currency` next to the money, `server_offset_ms` next to the times) and
both go into whatever record a human will read.

**2. A conversion is measured, not derived, wherever the source can be asked.**

`utc_offset_hours = 7` came from a shift scan, not from knowing the feed is
Vietnamese. The MT5 clock offset is read from the terminal, not from anchoring
Vantage to New York DST — the New York derivation is right today and is still a
belief about which zone a broker keeps. A derivation is a second thing that can
be wrong, silently, later.

**3. A conversion that cannot be established REFUSES. It does not fall back.**

A default is how a unit error becomes invisible: `getattr(acc, "equity", 0.0)`
turned a terminal that was not answering into an equity of zero and skipped two
guards. Where the conversion gates a decision that spends money, not knowing
must stop the decision.

**4. Every conversion is pinned by a test at the boundary, and the test says
what would reopen it.**

This is the half that prose does not do. `otl.rs:351` is the model: the
assertion does not merely check a number, it says *"must stay 7 until
re-measured"*, so a reader who changes it knows what they are claiming. A
convention with no failing test is a comment, and CLAUDE.md:58 is the proof
that a comment does not hold.

**5. The unit appears in the log line, not only in the code.**

All three of today's were survivable-by-review only because the record was as
wrong as the code. Money is logged with its currency beside it; times are
logged with the offset that was used to compare them.

## What this does not say

- It is not a request to rename existing fields. A sweep renaming every
  `price` in the repo would be a large diff with no failing test behind it,
  which is the opposite of what this document argues for. The rule binds at
  boundaries and when code is next touched.
- It does not say the trap list in CLAUDE.md is useless. It says the trap list
  is where you find out the disease exists, and a test is what stops it.
- It does not claim five instances is the whole set. A survey accompanies this
  document; the survey is a snapshot, and the convention is what outlives it.
- It says nothing about correctness of any result. No verdict, backtest or
  receipt in `docs/` is affected — every one of the five was in live plumbing,
  not in the engine.

## What would reopen this

A second instance appearing in code written AFTER this date, with a test at the
boundary, would mean rule 4 is not sufficient and the convention needs a
stronger mechanism than a test — a type, probably, which is cheap in Rust and
expensive in Python and was deliberately not proposed here.
