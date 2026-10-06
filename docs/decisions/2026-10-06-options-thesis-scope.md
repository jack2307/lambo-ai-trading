# 2026-10-06-options-thesis-scope: the original thesis has a tape, and the thing it does not have is bars

**Written:** 2026-10-06
**Status:** SURVEY. No code written, nothing built, no backtest run, no API
called. Every number below was read off disk or out of source.
**Agent:** n4, branch `agent/n4`, worktree `/e/rust/fd-b4`
**Axis:** gold options flow — max pain, whale levels, break-even, clusters,
profile: the claim this project was built to test.
**There is no gate result in this document and no percentile, because nothing
was measured.** §11 item 2 of the brief is answered "no gate — this axis
measured nothing."

## 0. The one-paragraph answer

The premise this survey was handed — *no tape exists, the thesis was never
loaded with data* — is **half right and the wrong half is the actionable
one.** The thesis has never been measured; the record already says so twice in
the desk's own voice. But the tape **is on disk**: 33,785 gold option prints
and 44,843 BTC prints. `ls data/timeline* data/options*` found nothing because
that is not where the tape lives, and the binary printed `timeline: none`
because it was pointed at `xauusd`, a market whose config says
`options_source = "none"` and which would print that line over a perfect tape.

The real blocker is one nobody has written down: **the gold tape has no
tradable bars to be measured against.** The only bar series that covers it,
`GC-1m`, has `open == high == low == close` on **11,813 of 11,813 bars** and
zero volume on every one. And the tape's calendar — 2026-09-06 to 2026-09-17 —
holds **zero prints inside either of the brief's two windows.**

So: **conditionally open, and one action is urgent.** The collector has been
off since 2026-09-17; every hour it stays off is an hour of tape that cannot
be bought back at any price.

## 1. What was written, and where — counted, not estimated

### Rust (`crates/fd-engine`, 10 modules)

| module | lines | mechanism | ported from |
|---|---|---|---|
| `engine.rs` | 523 | snapshot assembly, per-expiration contexts | `src/core/engine.js` (186) |
| `levels.rs` | 376 | 9 level types → confluence clusters, scored | `src/core/levels.js` (157) |
| `profile.rs` | 376 | strike profile, POC / VAH / VAL, value area | `src/core/profile.js` (173) |
| `whale.rs` | 334 | whale support / resistance | `src/core/whale.js` (182) |
| `breakeven.rs` | 331 | call / put break-even | `src/core/breakeven.js` (162) |
| `flow.rs` | 297 | bull/bear premium, bias, net-flow velocity | `src/core/flow.js` (141) |
| `maxpain.rs` | 278 | max pain, payout curve, positioning table | `src/core/maxpain.js` (126) |
| `bigtrades.rs` | 267 | big-print selection, quantiles, footprints | `src/core/bigtrades.js` (96) |
| `price_levels.rs` | 2,091 | **bars, not options** — swings, FVG, order blocks | new 2026-09-19 |
| `lib.rs` | 47 | | |

Options analytics in Rust: **2,782 lines across 8 modules.** `price_levels.rs`
is the one module in the crate whose input is bars, and its own doc comment
says so.

### The port is 1:1. Nothing is JS-only.

The prototype's 13 `src/core/*.js` modules (5,145 lines across the whole JS
`src/`) each have a Rust counterpart — the eight above plus `classify.js` →
`fd-core/classify.rs`, `premium.js` → `fd-core/market.rs`, `trade.js` →
`fd-core/types.rs`, `instruments.js`, `signal.js`. Fidelity is not asserted, it
is tested: `crates/fd-engine/tests/parity.rs` (470 lines, 7 tests) checks
snapshots, profiles and models against JS-exported oracles for **both** gold
and btc, and `tests/golden/` holds those 27 oracle files on disk right now
(`gold-timeline.json` 4.9 MB, `gold-snapshot.json`, `gold-profiles.json`, …).

**So the answer to "which mechanism is only in JS" is: none.** The port
is complete. That is a real asset and it has been sitting unused.

### What is declared but NOT implemented — in either language

`levels.rs:23-32` declares **9** `LevelType`s. `levels_from_context`
(`levels.rs:191-202`) emits all 9. **Eight are computed. The ninth is not:**

```
crates/fd-engine/src/engine.rs:284        gamma_wall: None,
src/core/engine.js:129                    gammaWall: null, // requires a Greeks pipeline; phase 2
```

The gamma wall is plumbed end to end — it has a type, a weight (0.8), a
string, a cluster slot — and it is hardcoded absent in both implementations.
**§3 below shows it can never be computed from the gold feed at any effort.**

And **5 of the 9 level types carry `is_experimental() == true`**
(`levels.rs:52-58`) — "the formula behind this level has never been
reproduced": `CallBe`, `PutBe`, `Wsup`, `Wres`, `GammaWall`. `levels.rs:7-10`
says the cluster scoring coefficients "are **this project's own construction**
and encode untested hypotheses."

### Which mechanisms a strategy can actually read

Exactly **4** strategies return `needs_options() == true`
(`crates/fd-strategy/src/builtin.rs`, asserted by its own test at :805):

| id | reads | lines |
|---|---|---|
| `level-reversion` | `options.clusters` (score, low, high, center) | 402-477 |
| `maxpain-magnet` | `contexts.first().max_pain`, `.dte` | 489-551 |
| `flow-momentum` | `bull_ratio_15m`, `net_flow_velocity_norm` | 553-620 |
| `flow-at-level` | clusters + `bull_ratio` + a reclaim close | 624-… |

The same four, with the same ids and the same grids, exist in
`src/strategy/builtin.js:151/180/209/237`.

**Mechanisms no strategy reads directly:** value area (`vah`/`val`),
`call_be`/`put_be`, `w_sup`/`w_res`, `bigTradeImbalance`. They reach
`level-reversion` and `flow-at-level` **only pooled into one cluster score**.
This matters for §4: a cluster result is unattributable by construction, and
5 of the 9 things pooled into it are formulas the repo itself marks unverified.

They are also 6 of the 24 named features in `crates/fd-features/src/lib.rs:30`
(`pocDistAtr`, `wSupDistAtr`, `wResDistAtr`, `callBeDistAtr`, `putBeDistAtr`,
`bigTradeImbalance`) — the identical name list appears in the prototype's
`data/datasets/dataset.json` `featureNames`.

### A standing caveat the crate enforces by naming

`fd-engine/src/lib.rs:17-21`: **"There is no open interest here. Neither feed
publishes it."** What this crate calls max pain is derived from *traded*
positioning, and the type is `maxpain::PositioningTable` rather than an OI
table so the difference cannot be quietly forgotten. Any claim about "max
pain" here is a claim about a flow proxy, not the textbook quantity.

## 2. The data — exact filenames and schema

### Why the glob found nothing, and why the binary said `timeline: none`

Two separate causes, both benign, neither of them "no data".

**(a) Wrong path.** There is no `data/timeline*` or `data/options*` and there
never was. `crates/fd-store/src/store.rs:1-18` defines the layout:

```
data/<tape_id>/tape/date=YYYY-MM-DD/part-NNNNNN.parquet
```

Parquet has no append, so each flush is a new part inside its UTC day; reads
de-duplicate by venue trade id, and *that* — not the writer — is what makes
the store idempotent.

**(b) `xauusd` has no options by config, not by accident.**
`config/default.toml:344`:

```toml
options_source = "none"
```

`Market::tape_id()` (`crates/fd-core/src/market.rs:183-189`) returns `None` for
`OptionsSource::None`. `load_timeline` (`search.rs:358-366`) hits that `?` and
returns `None` **before it touches the disk**. So `--market=xauusd` prints
`timeline: none — options strategies will be skipped` and `tape: 0 prints, 0
frames` over a perfect tape. The 25 lines of config comment above it give the
reason, and it is the right reason: the COMEX GC tape's strikes do not sit on
spot XAUUSD prices.

### What is actually on disk, measured

| store | parts | days | rows | unique prints | window (UTC) | premium |
|---|---|---|---|---|---|---|
| `data/gold/tape` | 509 | 11 | 33,785 | **33,785** (0 dupes) | 2026-09-06 22:09:20 → 2026-09-17 13:46:11 | $371.6M |
| `data/btc/tape` | 1,455 | 7 | 55,324 | **44,843** (10,481 dupes, 18.9%) | 2026-09-11 05:36:36 → 2026-09-17 13:51:58 | $177.0M |
| `data/xauusd/tape` | 0 | 0 | 0 | 0 | — | — |
| `data/btcusd/tape` | 0 | 0 | 0 | 0 | — | — |

The two empty ones are correct: `btcusd` sets `tape = "btc"` and reuses that
store; `xauusd` has no options source at all.

Gold, per UTC day — and the day-of-week pattern is clean (Sunday evening open,
Mon–Fri, Saturday absent):

| day | prints | contracts | premium | source |
|---|---|---|---|---|
| 09-06 Sun | 65 | 5 | $0.5M | migrated |
| 09-07 Mon | 1,204 | 11 | $13.4M | migrated 913 / feed 291 |
| 09-08 Tue | 2,147 | 11 | $30.2M | migrated 1,731 / feed 416 |
| 09-09 Wed | 2,935 | 11 | $42.4M | migrated 2,261 / feed 674 |
| 09-10 Thu | 3,926 | 11 | $52.3M | migrated 2,990 / feed 936 |
| 09-11 Fri | 5,030 | 11 | $57.0M | migrated 3,811 / feed 1,219 |
| 09-13 Sun | 83 | 10 | $1.7M | feed |
| 09-14 Mon | 4,744 | 11 | $45.4M | feed |
| 09-15 Tue | 3,532 | 10 | $31.3M | feed |
| 09-16 Wed | 6,711 | 9 | $66.8M | feed |
| 09-17 Thu | 3,408 | 8 | $30.6M | feed |

**The two halves are not the same instrument of measurement.** Sept 6–11 is
`migrated` (the JS prototype's JSON, through `fd-store/src/bin/migrate-json.rs`,
2 parts per day). Sept 13–17 is live `reference-feed` polling (83–139 parts per
day). Only **Sept 14–17, 18,395 prints, 4 days** is uniformly live-collected.
Any split of this tape into two windows splits it across that seam.

### The schema the loader expects

`fd_store::tape_schema()` (`crates/fd-store/src/schema.rs`), **25 columns read
positionally** (`tape.rs:107-145` — positional "so that a schema drift shows up
as a type error on the very first read instead of as a column of nulls"):

```
id string NOT NULL                 timestamp timestamp[ms,UTC] NOT NULL
symbol string NOT NULL             instrument string
underlying string NOT NULL         expiration timestamp[ms,UTC] NOT NULL
dte double NOT NULL                strike double NOT NULL
option_type string NOT NULL        trade_price double NOT NULL
contracts double NOT NULL          bid double / ask double
aggressor_side string NOT NULL     flow_class string NOT NULL
premium_usd double NOT NULL        underlying_price double NOT NULL
exchange string                    sequence_id string
implied_volatility double          flag_block/sweep/multi_leg/spread bool NOT NULL
source string NOT NULL
```

From that, `load_timeline` (`search.rs:358-382`) builds frames via
`fd_backtest::timeline::build_timeline` at `options_step_ms = 300_000`
(5 minutes, `config/default.toml:79`), skipping any frame whose snapshot has no
contexts. A `Frame` carries `t, spot, bull_ratio, bull_ratio_15m,
net_flow_velocity_norm, big_trade_imbalance, clusters, contexts`.

### JS-era artifacts: present, and one of them is a trap

| file | content |
|---|---|
| `data/timelines/timeline-1788757760000-1789185599000-300000.json` | 1,427 frames |
| `data/timelines/timeline-1789104996063-1789191240277-300000.json` | 288 frames (btc) |
| `data/datasets/dataset.json` | 1,354 rows, 24 `featureNames` |
| `data/reference/btc-options.json`, `chart-{G2MU6,OG3U6,OGV6,OGX6}.json` | raw payload cache |

**The trap:** the gold timeline filename starts at `1788757760000` =
2026-09-07 05:09:20Z. The corrected tape starts at `1788732560000` =
2026-09-06 22:09:20Z. The difference is **exactly 25,200,000 ms = 7 hours.**
These files are on the **uncorrected UTC+7 feed clock**; they predate the
2026-09-12 offset fix (`docs/decisions/2026-09-12-otl-timestamps.md`,
pinned by `otl.rs:342-351`). **They must not be read as UTC.** The corrected
oracle is the one already in this repo: `tests/golden/gold-timeline.json`,
1,427 frames, 2026-09-06 22:09:20Z → 2026-09-11 20:58:28Z, with
`tests/golden/gold-meta.json` declaring tape count 11,770. The live store has
since grown from 11,770 to 33,785.

## 3. Can the data be got, and what does it cost

### Where the gold thesis actually gets its data — and it is not CME

```
config/default.toml:627-628   [sources.reference]  base_url = "https://live.otldata.com"
```

`crates/fd-ingest/src/otl.rs` (376 lines, fully ported): "Public,
unauthenticated JSON endpoints of an observed dashboard. It exists … as a data
source while no direct CME feed is wired up, and as the calibration target for
the level formulas this project has not been able to verify independently."

**Money cost: $0.** No key, no subscription, no signup. *No endpoint was called
for this survey.*

The brief's suspicion is correct and worth stating plainly: **Deribit has no
gold.** `crates/fd-ingest/src/deribit.rs` serves BTC/ETH only. The entire gold
options thesis rests on one free third-party dashboard, and the repo's own rule
is that the feed's published `max_pain`, break-even and whale levels are
**calibration targets, never inputs** (`otl.rs:9-13`) — "feeding a site's own
answer back into this project's analytics would turn a comparison into a copy."

### The cost is not money. It is calendar time, and it is being spent right now.

`crates/fd-ingest/src/bin/collect.rs:1-11` — the title line is the finding:

> **"Accumulate the options tape, because nobody will sell it to us later.**
> Deribit's public trade endpoint reaches back about a day. That is not a
> paging bug and no amount of retrying widens it: history that is not captured
> as it happens is gone. So the options-derived strategies cannot be researched
> over anything longer than a day *unless a process like this has been
> running*."

And for gold (`collect.rs:18-24`): "the OTL feed publishes a rolling window of
a few days per contract, so the gold collector *polls*". `collect.rs:26`:
**"Intended to run for weeks."**

**The collector last wrote at 2026-09-17 13:46. Today is 2026-10-06. Nineteen
days of tape are gone and cannot be recovered at any price.** Nothing on disk
is newer. This is the single most consequential fact in the survey, and it is
the only one with a deadline.

### Three things the tape does not contain, measured column by column

**(a) No implied volatility and no quotes on gold ⇒ the gamma wall is
impossible, not merely unimplemented.**

| column | gold (33,785 rows) | btc (55,324 rows) |
|---|---|---|
| `implied_volatility` | **100.0% null** | 23.3% null (present on the live rows) |
| `bid` | **100.0% null** | 100.0% null |
| `ask` | **100.0% null** | 100.0% null |

A gamma wall needs gamma; gamma needs IV or a quote to invert. The gold feed
publishes neither. §1's `gamma_wall: None` is therefore **not a to-do item** —
it is the correct value, and it will stay correct until a different gold feed
exists. *Recording this closes the "phase 2 Greeks pipeline" note that has
stood in both codebases since the prototype.*

**(b) The venue flags carry no signal.** All four are `NOT NULL` and on gold
all four are **false on all 33,785 prints**:

| flag | gold | btc |
|---|---|---|
| `flag_block` | 0 true | 29 true (0.05%) |
| `flag_sweep` | 0 true | **0 true** |
| `flag_multi_leg` | 0 true | 1,641 true (3.0%) |
| `flag_spread` | 0 true | 0 true |

So "block flow" and "sweep flow" as *flags* are not in the data. **This does
not kill the whale mechanisms**, and the distinction matters: `bigtrades.rs`
and `whale.rs` select on **premium**, not on flags. At gold's configured floor
of $100,000 (`config/default.toml:299`), **368 prints (1.09%) qualify, about 35
a trading day**; premium distribution p50 $5,380 / p95 $38,520 / max $779,000.
Whale levels are computable. "Sweep detection" is not.

**(c) One schema defect.** Gold's `underlying` is `NOT NULL` and holds the
empty string `''` on **100%** of prints. Harmless today — nothing downstream
groups by it — but a `NOT NULL` column carrying `''` is a column that will read
as populated to the next person who checks.

## 4. The blocker nobody had written down: there are no bars

This is the finding that changes the plan, and it was not in the brief.

### The one bar series that covers the gold tape is degenerate

`gold` reads `bars/GC-1m.parquet` (`bar_symbol = "GC"`). It covers
2026-09-06 22:08 → 2026-09-17 13:44, 11,813 bars — an **exact** match to the
tape, **11,811 of 11,813 bars (100.0%) inside the tape window.** Coverage is
perfect. The bars are not.

> **11,813 of 11,813 bars have `open == high == low == close`. 100.00%.
> `volume` is 0 on every bar.**

`otl.rs:21-22` says so in its own voice: *"**Closes only** on the underlying
series, so a one-minute bar from it has `open == high == low == close`."*

Three consequences, none of them survivable for a gated measurement:

1. A bar with no range **cannot cover a stop.** The engine's documented rule —
   "stop wins when a bar covers both stop and target"
   (`tests/golden/gold-manifest.json`) — can never engage. Every fill and every
   exit lands on a close, and **no adverse intrabar excursion is ever charged.**
   That bias runs in the method's favour.
2. True range collapses to `|close − prev_close|`, so ATR is a close-to-close
   statistic and every `stopAtr` is sized off it. Measured ATR(14) on GC-1m:
   p10 0.586, **p50 1.007**, p90 1.807, mean 1.175.
3. At gold's spread of 0.3 (`config/default.toml:289`), **cost/R = 19.9% at
   1.5 ATR, 14.9% at 2.0 ATR, 9.9% at 3.0 ATR.** Brief §7 puts gold at 15m /
   1.5 ATR at **4.04%**. The 1m horizon is **4.9× more expensive** — worse than
   silver's 17.32%, which §7 calls the expensive instrument.

And there is no second interval: `search.rs:56-57` reads
`bars/<bar_symbol>-<interval>.parquet` directly, with **no resampling**, and
`GC-1m.parquet` is the only `GC-*` file on disk. `--market=gold` has exactly
one horizon, and it is the worst one.

### Every other configuration trades bars for coverage

Measured, tape window ∩ bars, with the median in-window true range:

| market / bars | bars total | in tape | % | ATR p50 | cost/R @1.5 ATR |
|---|---|---|---|---|---|
| `gold` / GC-1m | 11,813 | **11,811** | 100.0% | 0.800 | **25.0%** + 100% degenerate |
| `gold` / XAUUSD-15m | 100,586 | 786 | 0.8% | 7.405 | **2.70%** ← cheapest |
| `gold` / XAUUSD-5m | 101,242 | **1,841** | 1.8% | 4.160 | 4.81% |
| `gold` / XAUUSD-1m | 100,000 | 6,731 | 6.7% | 1.670 | 11.98% |
| `btcusd` / BTCUSD-15m | 100,798 | 425 | 0.4% | 116.92 | 9.72% |
| `btcusd` / BTCUSD-5m | 100,860 | 1,277 | 1.3% | 63.73 | 17.84% |
| `btc` / BTCUSDT-5m | 218,880 | 448 | 0.2% | 42.47 | 7.85% |
| `btc` / BTCUSDT-15m | 70,080 | **114** | 0.2% | 103.31 | 3.23% |

That last row reproduces, to the bar, the sentence in
`docs/decisions/2026-09-12-technical-baselines.md`: *"the tape covers 114 of
70,080 bars (0.16%)."* **The number has not moved in 24 days.** It is the same
tape against the same bars.

The XAUUSD rows are **not currently runnable** — `options_source = "none"`
blocks them — which brings us to the one claim in the config worth re-examining.

### The basis is real, is smaller than the config implies, and is measurable

`config/default.toml:340-347` justifies `options_source = "none"` on `xauusd`:

> "Spot XAUUSD trades tens of dollars below GC (the basis), wider than the
> strike spacing the levels come from, so attaching the gold tape here would
> place every level on the wrong price. Basis-adjusted levels are a research
> task; until then this market is technical-only."

**Measured on 6,718 overlapping minutes (GC-1m close − XAUUSD-1m close):**

```
mean  +43.70    sd 1.91
p10    41.26    p50 43.97    p90 45.78
min     9.00    max 47.61
drift  quartile means 45.66 -> 44.19 -> 43.59 -> 41.35  (monotone, -4.3 over 10 days)
```

The config is right about the **level** and that is the easy part: +43.70 is a
constant and a constant is correctable. What decides the research task is the
**residual after correction**, and that is **sd 1.91 USD against a gold cluster
floor of 5.0 USD** (`config/default.toml:302`) — about 38% of the minimum
cluster width. Because the basis *drifts monotonically*, a single constant
offset would be wrong by 4.3 USD across the window while a **rolling**
correction would not.

**So the config's claim is true of a constant offset and not true of a rolling
one.** "Basis-adjusted levels are a research task" is correct, and this
measurement bounds it: it is a specified, finite task, not a dead end.

### The hard stop: the brief's gates cannot be reached in any configuration

Brief §4 requires two windows, A = 2025-07-01…2025-10-01 and
B = 2025-04-01…2025-07-01. **The tape holds zero prints in either.** The only
tape that exists is 2026-09-06…2026-09-17.

Brief §5 requires ≥ 40 trades **per window**. The widest honest split of 10.6
trading days is two ~5-day halves — and §2 showed that seam falls exactly
between the `migrated` half and the live-polled half.

> **This axis cannot be put through the brief's gates today, in any
> configuration. That is why this document declares no gate and publishes no
> percentile.** Brief §9's warning applies in advance: with one short window of
> a trending instrument, a percentile would measure September 2026 gold, not a
> mechanism.

## 5. What the record already said, and it was right

The owner's framing — *the thesis has never been measured* — is correct, and
the desk had already established it. Quoting rather than re-deriving:

- `docs/decisions/2026-09-12-technical-baselines.md`, on all four options
  strategies: *"Those have never been tested: the tape covers 114 of 70,080
  bars (0.16%), and all four returned zero out-of-sample trades. **Not
  rejected — unasked.**"*
- `docs/decisions/2026-09-14-night-synthesis.md`: *"Options flow at levels, the
  reason the project exists, still waiting on a tape long enough to test."*
- `docs/hypotheses/2026-09-17-otl-context.md`, amendment of 2026-09-19, which
  audited every closed registration: *"**Not one of them reads the option
  tape.**"* That amendment also corrects its own file's claim of a
  twenty-five-body graveyard, and the correction is the right prior for this
  document: *"**An untested construct is not a ruled-out one, and it is not a
  promising one either.**"*

The only receipt the four ids appear in is a **side effect** of sweeping a
trailing stop across the whole strategy table
(`docs/research/runs/2026-09-15-trail-exploratory/trail-off.txt`):

| strategy | trades | win% | PF | expectancy |
|---|---|---|---|---|
| `maxpain-magnet` | **9** | 66.7% | 3.298 | +0.522 |
| `level-reversion` | **12** | 41.7% | 0.989 | +0.002 |
| `flow-momentum` | **3** | 0.0% | 0.000 | −1.010 |
| `flow-at-level` | **2** | 0.0% | 0.000 | −1.031 |

Four rows, **26 trades in total**, against a 40-trade floor; no null was run;
no registration named them. The otl-context amendment's verdict on the first
row is the one to keep: *"`maxpain-magnet`'s PF of 3.298 on nine trades is the
exact number this desk exists to not believe."* Brief §5's own warning is the
same number from the other side: the same rule, same window, gave PF 1.753 on
14 trades and PF 0.682 on 178.

One adjacent measurement exists and it is not encouraging:
`docs/decisions/2026-09-12-logistic-does-not-generalise.md` fitted a logistic
model over the 24 features — including `max_pain`, `poc`, `w_sup`, `w_res` —
and got gold AUC **0.77 in-sample → 0.52 walk-forward** on 974 rows, BTC
**0.95 → 0.24** on 169. Its own reading is the careful one: *"The useful
conclusion is not that options flow fails — it is that this feature set is
easily memorised."*

## 6. The first falsifiable measurement — a pre-registration, ready to use

**Not to be run until §7 item 1 is done.** Registered now so that it cannot be
written after seeing a number.

**Hypothesis.** On the single tape that exists, option-derived confluence
clusters mark prices at which gold spot mean-reverts, at a rate distinguishable
from a cost-matched, location-matched null.

**The cheap pre-check that comes first, and can close the axis on arithmetic.**
Before any strategy is run, build the timeline from `data/gold/tape` and count,
over the **1,841 in-tape XAUUSD-5m bars**, the number of bars whose close sits
within 0.35 ATR of a cluster of score ≥ 3. That count **is the ceiling on the
trade count**, because all four strategies refuse to enter while
`ctx.position.is_some()`.

> **If the ceiling is below 40, the gate is unreachable and this axis closes on
> arithmetic — no backtest, no null, no percentile.**

That is one build and one run, with no strategy and no null. It is the cheapest
possible route to a NO and it is why it goes first.

**Falsifiers, both of which can fire.**
1. *Ceiling < 40 entries in the one available window.* → Closed for want of
   tape. Recorded, and nobody re-asks until the collector has banked months.
   **This is the expected outcome, stated in advance.**
2. *Ceiling ≥ 40, but the entry locations are indistinguishable from prices
   drawn at random on the same strike grid, matched on distance-to-ATR.* → The
   cluster is a restatement of "price is somewhere", not a level.

**Null must match the mechanism (brief §9).** The cluster rule enters at a
**price location**, so the null must also enter at a price location: clusters
of the same count and width placed at random strikes on the same grid. A coin
flip would not be a control for this rule. **If that null cannot be built,
report the gate alone and publish no percentile** — and say so.

**Multiple-testing budget, declared before the first run.**
Pre-check: 1 run × 1 row = **1 cell.**
Compare pass, if the pre-check permits: 1 market-config × 4 strategies × 2
guard arms (`--guards` and not, per brief §8) = **8 cells.**
**Declared total: 9 cells.** Any widening is a new registration.

**How to read it.** Gate PF ≥ 1.200 **and** expectancy ≥ +0.050R **and** ≥ 40
trades, with `--exit-mix` always on and **the method's own rule verified to
have fired** (brief §8's `tsmom/120d` trap: PF 2.236 and `SURVIVES` while its
own rule fired zero times). Guards reported on both arms.
**One window only exists. A pass is "survived one window", which brief §4
says is not a result.** Declared here so it cannot be read otherwise later.

**What this does not test.** Not max pain alone, not whale levels alone, not
break-even alone — the cluster pools 9 level types and 5 of them are
`is_experimental`. Attribution needs per-level-type runs, which is a later
registration, and the pooled result cannot stand in for it.

## 7. Effort, straight, because the base rate is 0/5 programmes

| # | action | effort | prerequisite |
|---|---|---|---|
| 1 | **Restart the gold + btc collectors** | **minutes** | none |
| 2 | Pre-check: frame count and entry ceiling on existing tape | **2–4 hours** | one build |
| 3 | Basis-adjusted `gold-spot` market (rolling GC−XAUUSD, levels shifted at build) | **1–2 days** | (2) says the arithmetic permits it |
| 4 | A measurement that can pass §4's two windows | **6–10 weeks of collector uptime**, then hours | (1) running continuously |
| 5 | Gamma wall | **impossible on this feed, at any effort** | a different gold feed |

**Item 1 is the recommendation and it does not wait on this document's
conclusions.**

```
cargo +stable-x86_64-pc-windows-gnu run --release -p fd-ingest --bin collect -- --market=gold
cargo +stable-x86_64-pc-windows-gnu run --release -p fd-ingest --bin collect -- --market=btc
```

No money, no signup, no new code, no decision about the thesis. It is a pure
option: 19 days are already unrecoverable, and the cost of the next 19 is the
same. Every other item on this list is worth less than this one.

**On item 4, the honest arithmetic.** 40 trades per window × 2 windows, at the
entry rates the only existing receipt showed (9, 12, 3, 2 trades over a tape of
this length), needs on the order of **3–6 months of tape**, not 11 days. There
is no vendor and no shortcut: §3 established that the history cannot be bought.
**Nobody should fund item 3 or 4 on the strength of 11 days of tape** — brief
§1's base rate is 0/5 programmes, and an unmeasured thesis is not a promise.

## 8. Verdict: conditionally OPEN, with one part closed in writing

**OPEN**, because the obstacle turned out to be bars and calendar, not the
absence of prints — and both are addressable:
- 33,785 gold prints and 44,843 BTC prints are on disk, de-duplicated, with a
  tested 25-column schema and a JS-parity-checked engine behind them;
- the port is complete, so there is no code to write before measuring;
- the free source still exists and the collector is one command;
- the basis blocker is **measured** here (sd 1.91 against a 5.0 cluster floor,
  monotone drift) and is a bounded task rather than an unknown.

**CLOSED IN WRITING, so nobody re-asks:**

1. **The gamma wall cannot be computed from the gold feed.** `implied_volatility`
   is 100% null and `bid`/`ask` are 100% null across all 33,785 prints. The
   `// requires a Greeks pipeline; phase 2` note in both codebases is hereby
   answered: not phase 2 — **not possible on this feed.** `gamma_wall: None` is
   the correct value.
2. **Sweep and block detection by venue flag is not available.** All four flags
   are false on all gold prints; BTC has 29 blocks and 0 sweeps in 55,324.
   Premium-threshold whale selection **does** work (368 gold prints over
   $100k, ~35/day) and is the only whale mechanism that should be claimed.
3. **The brief's two-window gate is unreachable for this axis in 2026-10.** The
   tape holds zero prints in windows A and B. Any receipt claiming otherwise is
   reading a different market's bars with no tape attached.
4. **`--market=xauusd` will always print `timeline: none`.** It is config, not
   data. Reaching the gold tape from spot bars requires the new market in item 3
   of §7 — not a flag.

**What is NOT closed:** whether option-derived levels predict gold. This survey
did not measure it and could not. Per §5's own standard: **not rejected —
unasked**, and now with the reason written down to the filename.

## 9. Multiple-testing ledger

Declared for this job: **zero cells.** It is a survey; nothing was measured,
nothing was swept, no gate was read, no percentile computed.
Examined: **zero (run, row) pairs.** No build, no backtest, no API call.
The pre-registration in §6 declares **9 cells** for a future job and has not
spent any of them.

**Nothing was loosened after seeing a result**, because no result was produced.
The one threshold this document *proposes* (ceiling ≥ 40 entries) is the
brief's own §5 floor, unchanged.

## 10. Reproducing every number here

Read-only, no build. Scripts were written to the session scratchpad with the
`n4_` prefix (brief §10) and are not committed; each is a dozen lines of
pyarrow and the paths are given above.

- tape rows / dupes / window / premium: `pq.ParquetFile` over
  `data/{gold,btc}/tape/date=*/*.parquet`, union of `id`
- OHLC degeneracy: `data/bars/GC-1m.parquet`, count `o==h==l==c`
- cost/R: median in-window true range × `stopAtr`, against
  `config/default.toml` `spread`
- basis: inner join of `GC-1m` and `XAUUSD-1m` on `time`, 6,718 minutes
- null fractions / flags: `null_count` per column over every part
- `data-sealed/` was **not** opened, not read and not counted (brief §10)

`df -h /e`: **25G available before, 25G after** — nothing was built and nothing
was written outside this worktree.
