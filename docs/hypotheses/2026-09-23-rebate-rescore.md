# 2026-09-23-rebate-rescore: does the introducing-broker rebate move anything across the line

**Registered:** (commit time is authoritative) — before any run
**Status:** open

## Where this came from

The owner asked for a scalping strategy built to maximise the Backcom
rebate: "lãi thấp nhưng tối ưu được Backcom". The arithmetic was done before
any design work and says that idea loses money, so it was not built. The
rebate is 45% of the round-turn spread and the spread is paid in full, so
**every trade keeps 55% of the spread as a cost**. At 0.7 lots, a hundred
trades a day with no edge pays 1,470 USC and gets 662 back: 808 USC down a
day, about 24% of a 100,000 USC account a month. Churn cannot farm it.

Worse for the specific idea: cost and rebate are both fixed in points, so
dividing by a tighter stop makes both larger as a fraction of risk. The
break-even edge is **5.8% of R at a 2-point stop** against **1.0% at a
12-point stop**. Scalping is the most cost-sensitive style there is, and a
45% refund does not reverse that.

What the rebate does do is **halve the bar**: 10.5% of R becomes 5.8% at two
points, 1.8% becomes 1.0% at twelve. That is a real change and it is the
only honest version of the question — not "trade more to earn rebate" but
"does the lower bar admit anything the record already rejected".

## Claim

Among the constructs this desk has already closed, at least one crosses
**both** its profit-factor gate and its matched null when the rebate is
credited, on the same data and the same folds it was closed on.

The five nearest the line, from `ui/src/lib/verdicts.ts`:

| construct | PF as closed | trades |
|---|---|---|
| `keltner-break` | 1.052 | 533 |
| `ema-cross` | 1.033 | 787 |
| `rsi-reversion` | 1.011 | 1,247 |
| `macd-cross` | 1.010 | 923 |
| `donchian-breakout` | 0.972 | 2,088 |

Four of these already have PF above 1 and were closed on the null, not on
the money. So the claim is mostly about the null: a credit that lifts every
trade equally lifts the null too, and whether the gap widens is the whole
question.

## Falsifier

Per construct, on the primary year, walk-forward 4 folds, with the rebate
credited as a separate line and the gross left untouched:

- **PF net of rebate ≥ 1.2**, the registry's standing `min_profit_factor`,
  **and**
- **≥ 95th percentile of the matched null**, the null carrying the same
  rebate, **and**
- **≥ 95th on the direction null**, likewise.

All three, or the construct stays closed. A construct that passes is a
**candidate**, not a result, and its long-window number is reported beside
it whatever it says.

**The null must carry the rebate too.** A credit applied to the strategy and
not to its control is not a test, it is an accounting error that flatters
every row equally. If the implementation cannot put the rebate in the null,
this registration fails and reports that, rather than reporting a percentile
that means nothing.

## Multiplicity

29 constructs carry a recorded profit factor; the five above are the ones
named in advance and the run covers all 29. At the 95th percentile, **one or
two of 29 pass by luck**. So:

- one or two passes is **the expected yield of noise** and closes the
  program;
- a pass is only interesting if it also clears the long window, which is a
  second, unpaid-for test;
- the record will state how many passed against how many were expected.

## Data

- Primary: `xauusd:15m`, `2025-09-13 → 2026-09-12` — the recent Vantage year,
  the criterion the owner made primary on 2026-09-13.
- Context, reported and never a gate: `xauduka:15m`, `2022-06-16 → 2025-04-10`.
- Spread: the configured **0.28**, unchanged, because every receipt in
  `docs/decisions/` was measured at it. The logger's measured median is 0.21
  and the difference is noted in `config/default.toml`; using 0.21 here
  would flatter every row against a record measured at 0.28 and would be a
  second change riding in on this one.
- Rebate: `[rebate] share_of_spread = 0.45` from `config/accounts.toml`, one
  whole round-turn spread per trade, the same arithmetic the live desk uses
  (`RebateTerms::on`).

## What each outcome means

- **Nothing crosses.** The likeliest result and a useful one: it closes
  "optimise for Backcom" as a strategy question with a number, and the
  rebate goes back to being what it is — a discount on costs that makes the
  existing books slightly cheaper to run.
- **One or two cross.** Indistinguishable from luck at this width. Recorded,
  not promoted, and not put on the funded account.
- **Three or more cross, or one crosses and also clears the long window.**
  Worth a paper book and a fresh registration. Not worth real money on this
  evidence alone.
- **A construct crosses only because the null was not given the rebate.**
  Not a result at all. See the falsifier.

## What this does NOT test

Whether a purpose-built high-frequency strategy could earn more rebate than
it pays. The arithmetic above says it cannot at 45%, and that is a statement
about the cost model rather than about any particular rule.

**Corrected the same day, before any run.** This paragraph first said that a
share "above about 60%" would change the sign of that arithmetic. That is
false and the check that produced it is trivial: the net cost of a round
turn is `(1 − share) × spread`, which is positive at every share below 100%.
At 60% the desk still pays 40% of the spread on every trade; at 90% it pays
10%. Churn with no edge loses money at **any** share short of a full
refund, and a full refund would only make it free, never profitable. The
number that can change the sign is not the share but a **per-lot** rebate,
which is not proportional to the spread and can exceed it — the owner's own
book pays $10–12 per standard lot
(`docs/decisions/2026-09-13-close-reopen-drift.md`), and a standard lot is
100× this workspace's gold contract. If the IB terms ever become per-lot on
this account, the arithmetic is different in kind and this is worth asking
again. Under a share of the spread it is not.
