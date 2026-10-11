# 2026-10-11 — reward/risk as a flag, and the middle one of the engine's three ceilings

**Registered:** commit time is authoritative. Written and committed ALONE,
before the first line of code on this branch.
**Branch:** `agent/reward-risk`, worktree `E:/rust/fd-reward-risk`, cut from
`29c8635`.
**Brief:** `E:/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + addenda 5, 6, 7, 8, 9,
10, 11. §IV.B of addendum 11 is the assignment.
**Status:** registered.

## 0. THREE corrections to the brief, before anything else (§8: the measurement wins)

### 0.1 `reward_risk` has NO per-market declaration path. `max_hold_ms` does.

Addendum 11 §VI told me to check this before building a new door, because
`hold-cap` found that `max_hold_ms` already had one. Read from the source, not
assumed:

    MarketTradingOverride (config.rs:338-386)  -- 20 fields, reward_risk NOT among them
    trading_rules_for     (engine.rs:368)      -- reward_risk: config.trading.reward_risk
    tests/trading_rules.rs:48                  -- asserts btc.reward_risk == gold.reward_risk,
                                                  with the comment that risk appetite is shared

So unlike `max_hold_ms` — which is `Option<i64>` per market and has been since
2026-09-24 — `reward_risk` is a single `f64` in the shared `[trading]` table and
is reachable from **nowhere else**. `[run]` in a batch file reaches it even less:
`batch_from_file` (`hypotheses.rs:103-105`) ignores every table that is not
`[[hypothesis]]`, by its own doc comment. **This job therefore builds the flag,
rather than making an existing door sweepable.**

### 0.2 `reward_risk = 1.8` was NOT what bounded `hold-cap`'s cells

Addendum 11 §IV.B says *"moi o 72h cua `hold-cap` van bi target 1,8R chan"*.
**All three of that job's family rows carry their own explicit target**, read
off the source before spending a cell:

    orb.rs:150-156            target = close +/- risk * p.get("riskReward"),  default riskReward 1.5
    screen.rs:35-42, 96       keltner-break via enter(.., p.get("riskReward")), default 1.5
    far_stop_break.rs:136-142 target = close +/- risk * p.get("riskReward"),  default riskReward 1.0

And the entry builder takes the explicit one first (`engine.rs:737`,
`Some(explicit) => Some(explicit)`), so `rules.reward_risk` is **never read** on
those rows. Consequences, both of which this record is bound by:

* The ceiling on `hold-cap`'s 72 h cells was **1.5 / 1.5 / 1.0 R from each
  row's own `riskReward` parameter**, not 1.8 R from `[trading]`. The TARGET
  share it watched rise 9.5-31.6 points is a real measurement; its attribution
  to `reward_risk` is wrong.
* That parameter is **already sweepable** — `overrides = { riskReward = X }` in
  a batch file — so for those three rows the axis this job is asked to open was
  never shut. `--reward-risk=` will be measured to change **0 trades** on them,
  and that is control #4 below.

⇒ The surface `--reward-risk=` can reach is the rows that enter with
`target: None` under `Exits::Engine`. Counted from the registry
(`builtin.rs:45-96`, 33 strategies): **seven**.

    ema-cross              builtin.rs:157,165   runnable on bars alone
    rsi-reversion          builtin.rs:230,239   runnable on bars alone
    donchian-breakout      builtin.rs:310,318   runnable on bars alone
    level-reversion        builtin.rs:460,467   needs_options() -- 0 trades, see §2
    flow-momentum          builtin.rs:605,613   needs_options() -- 0 trades, see §2
    flow-at-level          builtin.rs:702,710   needs_options() -- 0 trades, see §2
    companion-unconfirmed  companion_unconfirmed.rs:182   needs a companion market

Everything else in the registry is either `Exits::Strategy` (its stop and
target are a sizing unit, `engine.rs:772-777`) or supplies its own target.

### 0.3 And `Exits::Engine` CAN say "a stop and no target" — through this number

Addendum 11 §IV.B, and `max_hold_horizon.rs::a_no_target_entry_is_given_one`,
both say the engine cannot express it. Half of that is wrong the same way §0.1
was: `engine.rs:740` reads

    None if self_managed || stop.is_none() || rules.reward_risk == 0.0 => None

so `reward_risk = 0.0` **is** the expression — it has simply never been
reachable for one run without editing the table the two funded books on
33708517 read. `--reward-risk=0` makes defect 5 expressible per run, and
**`0` is the value in this sweep that the record has never measured at all.**

## 1. Hypothesis — one sentence

On the rows where the engine derives the target, `PF_r` is a function of
`reward_risk`, so the 1.8 R target — not the market — is what caps them; and the
value that has never been tried, `0` (a stop and the clock, no target), is where
that function is highest.

## 2. The pre-check, run BEFORE this registration, spending 0 gate cells

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (= `29c8635`, this
worktree's base), `--market=xauduka --mode=hypotheses --interval=15m
--exit-mix`, both windows pinned at both ends, both guard arms, default
parameters, config `config/` (swap 0). All six bar-runnable and options rows of
§0.2 in one batch file.

TARGET share is the quantity that decides whether the flag can bite at all:
TARGET is the only exit kind `reward_risk` places.

| row | win | guards | n | TARGET | TARGET % | STOP | TIMEOUT | own rule | `PF_r` | `E` (R) | `max_dd_pct` |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `ema-cross` | W1 | off | 1868 | 429 | **23.0%** | 895 | 469 | 75 | 0.8236 | −0.1019 | 161.40% blown |
| `ema-cross` | W1 | on | 1691 | 371 | **21.9%** | 790 | 406 | 68 | 0.7945 | −0.1175 | **83.31% survived** |
| `ema-cross` | W2 | off | 1619 | 342 | **21.1%** | 844 | 380 | 53 | 0.7773 | −0.1303 | 161.47% blown |
| `ema-cross` | W2 | on | 1768 | 363 | **20.5%** | 883 | 363 | 102 | 0.7522 | −0.1440 | 138.57% blown |
| `donchian-breakout` | W1 | off | 4242 | 483 | 11.4% | 810 | 2016 | 931 | 0.8734 | −0.0505 | 168.93% blown |
| `donchian-breakout` | W1 | on | 4117 | 442 | 10.7% | 740 | 1905 | 893 | 0.8665 | −0.0521 | 155.26% blown |
| `donchian-breakout` | W2 | off | 6287 | 520 | 8.3% | 600 | 1735 | 3431 | 0.8414 | −0.0536 | 207.65% blown |
| `donchian-breakout` | W2 | on | 6103 | 473 | 7.8% | 535 | 1630 | 3300 | 0.8435 | −0.0515 | 178.51% blown |
| `rsi-reversion` | W1 | off | 9273 | 183 | 2.0% | 3298 | 156 | 5635 | 0.7148 | −0.1133 | 333.12% blown |
| `rsi-reversion` | W1 | on | 9404 | 165 | 1.8% | 3255 | 98 | 5777 | 0.7176 | −0.1097 | 224.98% blown |
| `rsi-reversion` | W2 | off | 3468 | 228 | 6.6% | 1678 | 446 | 1116 | 0.8640 | −0.0710 | 190.76% blown |
| `rsi-reversion` | W2 | on | 4833 | 180 | 3.7% | 2028 | 316 | 2211 | 0.8106 | −0.0864 | 169.21% blown |
| `level-reversion` | both | both | **0** | — | **null** | — | — | — | null | null | NOT MEASURED |
| `flow-momentum` | both | both | **0** | — | **null** | — | — | — | null | null | NOT MEASURED |
| `flow-at-level` | both | both | **0** | — | **null** | — | — | — | null | null | NOT MEASURED |

**The pre-check falsifier — "the derived target is an exit on no row, so the
flag cannot bite" — DOES NOT FIRE.** `ema-cross` closes on the derived target on
20.5-23.0% of its trades in all four cells; `donchian-breakout` on 7.8-11.4%.
The cells are earned.

Five things this table says that the brief did not, and that bound the result:

1. **Eleven of twelve measurable cells blew their book** (`max_dd_pct > 100%`,
   addendum 6 §IV). The one survivor is `ema-cross` W1 guards-ON at 83.31% —
   and guards-ON is the only arm the owner permits (addendum 5 §D). So this axis
   has **one** cell in which a USD number may be read at all, and this record
   will not read `PF_usd`, `net_usd`, `G_trip` or `G_session` on the other
   eleven. Gate decisions are taken on `PF_r` and `E`, which are computed from
   `t.r = points/risk` and never read `lots`.
2. **`expectancy_net` is NOT lots-free.** `engine.rs:963` is
   `r_net = points/risk + (swap − commission)/risk_usd`, and `risk_usd` is the
   per-trade USD risk, which on a blown book is `min_lot`-clamped
   (`engine.rs:729`). So on the eleven blown cells arm B's `expectancy_net` is
   contaminated in the same direction `PF_usd` is. In arm A it is exact:
   `engine.rs:191` and the test at `engine.rs:1335` pin `r_net`
   **bit-identical** to `r` when swap and commission are 0. ⇒ **arm A is where
   carry reads zero as a measurement; arm B is reported with the contamination
   named.**
3. **`rsi-reversion` is a row where the derived target was never the exit** —
   its own "RSI back to midline" rule closes 32-61% of its trades and TARGET
   closes 1.8-6.6%. It stays in the family because the family rule is a code
   fact, not a threshold, but a flat reading there is not evidence about
   targets.
4. **`cap_lots` bites enormously in the permitted arm**, read from the printed
   `guards:` line which formats `sized_down_by_guard` (`hypotheses.rs:752`):
   `ema-cross` W1 1569/1691 = **92.8%** of entries sized down, W2 1521/1768 =
   **86.0%**; `rsi-reversion` W1 8812/9404 = **93.7%**; `donchian` W1
   1938/4117 = **47.1%**, W2 2387/6103 = **39.1%**. Both gate legs are blind to
   it (addendum 10 §III).
5. **The three options rows are `null`, not 0.** The receipt prints
   "max drawdown: NOT MEASURED — this cell took no position" and
   "Lbar: NOT MEASURED", which is the engine obeying `null != 0` itself.

## 3. Family membership rule — declared before the sweep

A row is in the family iff, **from the source**:

    exits() == Exits::Engine   AND   its Intent::Enter carries target: None

That is a code fact with no threshold in it, and it is why the family is the
seven rows of §0.2 rather than a selection. Of those seven, **three take
positions on bar data alone** and are the measured family:
**`ema-cross`, `rsi-reversion`, `donchian-breakout`.** The other four are
reported `null` with the reason, and cannot carry a result.

TARGET share from §2 is carried **beside** every row as a readability column,
not used as a filter: a row below 10% TARGET at `reward_risk = 1.8` is reported
with the words **the derived target was never its exit**.

## 4. Declared cells — the count BEFORE the numbers

**Part 1, instrument. Five controls; each must pass or no measurement is
published.**

1. **Default byte-identity, and the baseline is a COMMIT not a binary.** Five
   argv captured with (a) the shared `29c8635` binary, (b) an independent
   `--release` build of `29c8635` in this worktree's own `target-rr/`. Those two
   must agree byte for byte first — that is what makes the baseline the commit.
   Then (c) the post-change binary, **passed no flag**, must reproduce all five
   byte for byte. `diff -r` on the whole directory, not a spot check. If it
   fails at the last digit, the cause is found and the instrument patched — the
   standard is not relaxed to "approximately" (addendum 11 §VI, the BLAS GEMM
   case).
2. **A deliberately failing probe first.** `search.rs` holds a test that scans
   its own source for flag literals and demands each be declared in `ALWAYS` /
   `BY_MODE`. The flag's **reading code is written first**, the suite is run, and
   **that test is watched to FAIL**. Only then is the flag declared. Without
   that step a passing suite proves nothing (addendum 10 §VII).
3. **Near misses are denounced, garbage is a hard error.** `--rewardrisk=`,
   `--reward_risk=`, `--reward-risk-ratio=` must each be reported as not a flag
   this binary has. A value that is not a finite number, and a **negative**
   value — which would place the target on the losing side of entry and make a
   trade that can only lose — must fail the run, not be silently clamped.
4. **The explicit-target invariance, which is §0.2 turned into a measurement.**
   `orb`, `keltner-break` and `far-stop-break` supply their own target, so
   `--reward-risk=` must change **0 trades** on them at any value. Run at 1.8
   and at 0 and diffed. Its paired **positive control in the same run** is the
   derived-target family, which the same two values must move a great deal. A
   negative from a blind instrument is worthless.
5. **A deliberately BLIND probe, and the assertion that it is blind.** A fixture
   whose tape never reaches any target at any `reward_risk` reads invariant too
   — for a reason that is not the causal one. It is written, it is asserted to
   read invariant, and it is asserted to be **excluded** by the family rule of
   §3. Without it, "invariant" is not evidence of anything.

**And one thing that is NOT a control but a declared measurement, because
addendum 11 §VI says this class of flag moves the null:** `control.rs:150` —
the matched null enters with **`target: None`** and takes `Exits::Engine` by
default (`registry.rs:256`). So `--reward-risk=` **re-specifies the control
itself at every value**, on every row, including rows where the method changes
nothing. ⇒ **No percentile in this job is published across `reward_risk`
values**, and the null's own `p50` / `p95` / `count match` drift is measured and
printed as a number rather than assumed.

**Part 2, measurement. Values declared exactly, five of them:**

    0.0   -- NO TARGET AT ALL (engine.rs:740). Never measured in this record.
    0.9   -- half the shipped number
    1.8   -- the shipped number, config/default.toml:98. The control value.
    3.6   -- double
    7.2   -- quadruple

    arm A, swap = 0      (`config/`)      3 rows x 5 values x 2 windows x 2 guard arms = 60
    arm B, swap = -0.83  (`config-swap/`) 3 rows x 5 values x 2 windows x 2 guard arms = 60
                                                                     declared total = 120 cells
                                                                     in 40 runs of 3 rows each

Windows, both ends pinned on every single run (addendum 11 §VII):

    W1  --from=2010-06-01 --to=2018-06-01    expect 190,889 of 378,749 bars
    W2  --from=2018-06-01 --to=2026-06-01    expect 187,860 of 378,749 bars

Bar counts are printed by each run and compared; a run whose bar count differs
from its window's figure is not read.

`config-swap/` is in-tree (`74a49a9`): a copy of `config/` whose only change is
−0.83 USD per lot per night on `xauusd` and `xauduka`. Arm A is the desk's own
**measured** swap-free account, not a null. What stays `null` is a third thing —
the rate Dukascopy-era gold would have paid in 2010.

## 5. The quantities, defined before they are computed

Read off each row's own receipt, nothing re-derived from another job's numbers:

    PF_r, E, Lbar       from the printed Lbar identity line, 4 decimals
    identity check      E == Lbar x (PF_r - 1) on EVERY line, residual printed
    TARGET share        TARGET / sum(all exit kinds) off the `exits:` line
    where TARGET went   d(STOP share) and d(TIMEOUT share) per point of d(TARGET share)
    expectancy_net      arm A exact; arm B flagged contaminated on a blown row
    sized_down          read from the printed `guards:` line field, never grepped
    null drift          null p50, p95 and count match at each reward_risk value

`G_session` is **deliberately not computed** on the blown rows: it needs
`gross_usd`, which needs `lots`, which `min_lot`-clamping has destroyed on
eleven of twelve cells. Reporting it would be reporting a sizing artefact as a
signal result.

## 6. Falsifier — specific and firable

**F1 (the main one). FIRES** if, across the five declared values, `PF_r` moves
by less than **0.050** on every family row in **both** windows — i.e. the span
`max(PF_r) − min(PF_r)` over `reward_risk` fails to reach 0.050 anywhere.

Conclusion when it fires, in these words: **all three of the engine's ceilings
have now been measured — the stop (`agent/stop-width` and `range-sizing`), the
target (here), and the clock (`agent/hold-cap`) — and none of them is what is
binding.** The long-horizon layer closes for the third time, and this time it
closes the whole exit space of `Exits::Engine` rather than one parameter.

**F2 (the trade this axis actually makes).** If `PF_r` does move but the
`--exit-mix` shows the vacated TARGET share arriving mostly at **TIMEOUT**, the
result is reported as **one ceiling swapped for another**, in those words, and
is not a mechanism — it is the same finding `hold-cap` reached from the other
side.

**F3 (carry).** If some cell's `PF_r` rises at a `reward_risk` that lengthens
the hold, and arm B's `expectancy_net` at that cell is at or below arm A's at
`reward_risk = 1.8`, the result is reported as **an account perk, not a
strategy**, in those words. On a blown row this comparison is reported as
**not measurable** rather than as a number.

**F4 (instrument).** If byte-identity fails, **no measurement is published at
all** and this record says so.

**F5 (tool-blind).** If the explicit-target rows move at any `reward_risk`
value, §0.2 is wrong; stop and report rather than reinterpret.

## 7. How to read it

* **`PF_r` beside `PF_usd` on every row, units named**, and nobody's ratio
  quoted (addendum 11 §V.2 — nine measurements, three answers).
* **`E` from the `Lbar` identity line at 4 decimals**, not the 3-digit
  `expectancy` column (addendum 8 §IV).
* **`max_dd_pct` on every cell.** `> 100%` ⇒ the book blew; `PF_usd`, `net`,
  `G_*` and arm-B `expectancy_net` are not read on it.
* **`--exit-mix` at every value**, and the destination of the vacated TARGET
  share named.
* **Gate counted by hand: 40 trades, not the tool's `need 30`.** Below 40 is NO
  CONCLUSION.
* **No percentiles published** — the control is re-specified by this very flag
  (§4). The gate legs carry the verdict.
* **A cell that only survives with guards off is reported NOT TRADEABLE**, in
  those words.
* **No number in any `why =` prose** in the batch files (addendum 11 §VI).
* **The receipt prints a `reward/risk:` line ONLY when the flag is passed.**
  That is deliberate and it is the owner's decision #1 of addendum 11 §0 applied
  here: byte-invariance of a default run is kept, and the argv goes in a file
  beside the receipt. A flagged run can still never be read as a default one,
  because the line names itself and what it overrode.

## 8. What this job will not do

* Not touch `config/default.toml`'s `reward_risk` or `max_hold_ms`, `main`,
  `config/local.toml`, `config/accounts.toml`, `data-sealed/`, the VPS, the MT5
  terminal, or the two `collect.exe` processes (pids 5044 / 38720).
* Not build at debug, not `cargo test --workspace`; `--release -p fd-backtest`
  in this worktree's own `target-rr/`.
* Not re-open the four axes addendum 10 §I and addendum 11 §III closed, and not
  propose a slow signal expressed as many small trades.
* Not decide anything the brief does not answer. If the measurement needs a
  decision the owner has not given, it stops, writes it here, commits, pushes
  and reports.
