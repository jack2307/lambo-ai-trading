# RESULT — the unit of the PF leg moves 3 of 452 published verdicts, one of them inside the decision record

**Date:** 2026-10-09
**Registration:** `docs/decisions/2026-10-09-pf-unit-audit.md`, committed alone
at `0a5b9fd` before the first engine run. The band was declared there.
**Branch:** `agent/pfr-audit` (cut from `agent/stop-width`).
**Binary:** `/e/rust/fd-stop-width/target-sw/release/search.exe`, built
2026-10-08 20:57 from `agent/stop-width` with the `lbar_line` patch. Nothing
was built on this axis.
**Receipt:** `receipts/pfr-audit-table.txt`.
**NO GATE.** Nothing here passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`. This
axis checks the ruler. No gate cell was spent.

## The answer

    cells re-run and paired to a published row      452
    verdicts that cross PF 1.200 when read in R       3      (0.7%)
      the record reads TOO HIGH (PF_usd >= 1.200, PF_r <  1.200)   1
      the record reads TOO LOW  (PF_usd <  1.200, PF_r >= 1.200)   2
      of the 3, inside docs/decisions/                            1

The falsifier declared in the registration — "if 0 verdicts flip, the record's
silence about the unit is harmless in practice" — **did NOT fire.** It is not
0. It is 3, and they are named below.

## The three rows, by name and direction

**THE RECORD READS TOO HIGH — 1 row.** This is the direction that matters,
because it is a published POSITIVE verdict that the R reading takes away:

    RECEIPT   hold/20-22-long   n=463   xauduka 15m   10,000 USD book
      published PF_usd 1.238  |  re-run PF_usd 1.2376  ->  PF_r 1.1818
      Lbar 0.6660 R, E +0.121 R      [a POSITIVE verdict was printed on it]

Read in USD it clears the PF leg by 0.038. Read in R it misses it by 0.018.
Same trades, same expectancy, same window — only the unit of the denominator.
It is a receipt row, not a decision row.

**THE RECORD READS TOO LOW — 2 rows:**

    DECISION  m15/eq-open-60m   n=316   btc 15m   10,000 USD book
      published PF_usd 1.192  |  re-run PF_usd 1.1919  ->  PF_r 1.2034
      Lbar 0.2630 R, E +0.053 R
      docs/decisions/2026-09-13-m15-check.md

    RECEIPT   hold/06-08-long   n=206   xauusd 15m
      CENT 100 USD book, min-lot clamped (defect 17)
      published PF_usd 1.196  |  re-run PF_usd 1.1925  ->  PF_r 1.2208
      Lbar 0.5343 R, E +0.118 R

## The one that sits in the decision record, read carefully

`docs/decisions/2026-09-13-m15-check.md` states in its **Outcome**:

> Every row fails the gate on its primary window; the closest is BTC's
> equity-open break at PF 1.192 (78th percentile, direction 72nd).

That row is `m15/eq-open-60m`, 316 trades. In R it prints **PF_r 1.2034**, so
on its PF leg it does **not** fail: 1.2034 >= 1.200. Its expectancy is
**+0.053 R >= +0.050 R** and its trade count is **316 >= 40**. So **read in R,
that row clears all three gate legs on that window**, while the decision
records it as the closest row that failed.

**What this does NOT overturn.** That decision rejected the row on **two**
grounds, and only one of them moves. The row sits at the **78th percentile
against a null whose p95 is 1.509** — the percentile leg is untouched by the
unit of the PF leg, and a percentile is not a gate (brief §4). The doc's own
Reading says "again short of both the gate and the noise"; after this
measurement it is short of **the noise**, not of the gate. The decision's
conclusion ("the timeframe is not the variable") does not depend on that row
passing or failing the PF leg, and the confirmation window was never opened,
so there is no second window to read. **One leg of one sentence in the
decision record is wrong; the decision is not.**

## The required identity check — it held, no second break found

    E = Lbar x (PF_r - 1)   on 452 of 452 cells within 1e-4 R
    largest |residual| anywhere                      0.00009 R

`agent/stop-width` measured 200/200 with a largest residual of 0.00008 R; mine
is 452/452 at 0.00009 R. **I did not find a second place the identity breaks.**
The known break stands alone: `engine.rs:1070-1076` mixing USD with R, which
is what makes the identity fail when it is read with `PF_usd`.

## The gap, and one place my numbers disagree with the brief

    |PF_usd - PF_r| on the same trade set, 452 paired cells
      median  0.0093      p90  0.0341      max  0.1343
    over EVERY cell of every re-run (653 cells)
      median  0.0090      p99  0.0802      max  0.3423

Widest anywhere: `close/fri-1630-1815` n=56 xauusd 15m,
**PF_usd 1.4635 -> PF_r 1.8058**.

**Addendum 7 A says the sign of the gap follows the sign of the book** — a
winning book flattered in USD, a losing book penalised. **Measured over 452
cells that is not what the gap does** (brief §8: my number wins, so here it
is):

    book DOWN (PF_usd <  1)   PF_usd flattered 249   penalised  98   equal 1
    book UP   (PF_usd >= 1)   PF_usd flattered  71   penalised  33

`PF_usd` reads **higher** than `PF_r` in 320 of 452 cells (71%) and it does so
in **both** books, 72% of losing books and 68% of winning ones. The USD
reading is biased upward, not signed by the book. (The 0.3640 figure that
addendum 7 quotes is the RANGE `PF_usd` travelled across ten stop widths, a
different quantity from the per-cell gap measured here, which is what decides
a flip. They are not in conflict; only the sign claim is.)

## The band was declared wide enough, and measured to be

Declared before any run: `PF_usd` in **[0.700, 1.700]**, half-width 0.500 =
1.37x the 0.3640 that `agent/stop-width` had measured.

    widest |PF_usd - PF_r| found anywhere in 653 cells   0.3423
    declared band half-width                             0.5000
    cells whose gap exceeds the half-width                    0

So on this evidence a published row sitting outside [0.700, 1.700] could not
have reached 1.200 either. The band is not merely declared — it is bounded by
measurement after the fact.

## Multiple-testing ledger: declared vs seen

    declared                                            seen
    commands to run                     88              88  (all exit 0)
    reproducible band keys             503             452 paired
    band rows                        4,845           4,845
    band rows in docs/decisions/       153              63 paired cells

Harvest numbers are mine, printed fresh (the record is a moving target: 9,858
rows / 110 refs at `floor-audit`, 10,076 / 118 at `gate-legs`):

    refs scanned                                       131
    distinct text blobs                              1,571
    table rows parsed                               16,788
    distinct printed rows                           10,802
    rows carrying BOTH a PF and an expectancy         6,589

`py/research/floor_audit_{harvest,census}.py` were **reused byte-identical**
from `agent/floor-audit` (md5 `141d375d2ecde4c0f47ab3caa50e9ae1`,
`dac30be534231686b48ae5930fe54642`) and `gate_legs_check.py` byte-identical
from `agent/gate-legs` (md5 `255e2cebfeceffd9269edbecc76b50bc`). No harvester
was written here. Continuity check: **all 1,053** entries of the frozen
`receipts/floor-audit-slot-rows.json` are still present in my fresh harvest, so
mine is a strict superset of what `floor-audit` read.

## Split by instrument and book (defect 17)

`config/default.toml` gives `xauusd`/`eurusd`/`btcusd` `starting_equity_usd =
100.0` — a cent book, so `raw_lots < min_lot 0.01` is clamped and `PF_usd`
there is the PF of a 0.01-lot book. The other markets take the 10,000 USD
default. The two are not the same quantity, so they are not pooled:

    market      book USD  cells  |gap| med  |gap| max  flips
    btc           10,000     18     0.0115     0.0575      1
    eurduka       10,000     12     0.0027     0.0058      0
    xagduka       10,000     12     0.0534     0.1222      0
    xauduka       10,000    258     0.0081     0.0802      1
    xauusd           100    152     0.0121     0.1343      1

The min-lot clamp does **not** remove the gap: `xauusd` carries the widest
paired gap of any market (0.1343) and one of the three flips. Clamping fixes
the lot size, but `pnl_usd` still weights each trade by that trade's own risk
distance while `r` weights every trade equally, so the two PFs still differ.

## THE BIGGEST THING I FOUND THAT WAS NOT THE QUESTION

**The record does not re-run to its own `PF_usd`, and parts of it no longer
produce the cell at all.** Same command, same batch file, same window, same
flags — only the paths moved onto this worktree:

    paired cells (trade count identical, so the same trade set)       452
      re-run PF_usd equal to published to print width (<= 0.0005)     295  (65.3%)
      |published PF_usd - re-run PF_usd| median                     0.0004
      |published PF_usd - re-run PF_usd| max                        0.1620

    published band keys whose command ran but which NO LONGER HAVE
    a cell with that trade count                                       51

The widest single drift, `tsmom/120d` n=70 `xagduka`, moved **PF_usd 1.248 ->
1.4100** on the same printed trade count. And of the 51 vanished cells, **34
sit in `docs/decisions/`** — whole families (`close/*`, `london/*`, `m15/*`,
`orb/*`, `vwap/*`, `fix/*`, `pdhl/*`, `hivol/*`, `lowvol/*`) now print a
different trade count than the decision that cites them, e.g. `fix/pm-after`
published n=578 where the same command now prints n=3078, and `m15/orb-60m`
published n=505 where it now prints n=2483.

This is a **reproducibility** defect, not a unit defect, and it is kept
strictly out of every flip count above. But it is larger than the thing I was
sent to measure: **a third of the re-run cells do not reproduce their own
published PF, and 51 published rows cannot be re-read at all.**

**Sensitivity, so the 51 are not an unquantified hole.** For each, the re-run
cell of the same label with the nearest trade count: 17 of 51 land within 5
trades, and **3 of those nearest cells sit on the other side of 1.200**. The
one that is genuinely near-identical:

    DECISION  close/1815-2200  published n=289 PF_usd 1.195
              vs nearest cell  n=292        PF_r 1.2061  (PF_usd 1.2185)

a 3-trade difference reading TOO LOW in the same direction as the other two —
a probable fourth flip that this axis cannot claim, because it is not the same
trade set. The other two (`close/fri-1630-1815`) are not comparable at all
(published PF 0.827 against a nearest cell at 2.1059 — a different arm).

## What was NOT measured, and why (null, not 0)

- **4,342 of the 4,845 band rows have no echoed command** in any blob they were
  found on; most are agent-written summary tables, which print a row but not
  the argv that made it. They are **unmeasured**, not shown to be unflippable.
  This is the real ceiling of this axis.
- **1,744 rows carrying a PF sit outside the declared band**, declared
  unmeasured in advance; the post-hoc gap bound (max 0.3423 < 0.500) is the
  evidence that they could not have flipped.
- **51 reproducible keys could not be paired** (above).
- **No percentile and no verdict word is read from a re-run.** `--seeds=1` was
  used after MEASURING that it leaves the method row and the entire `Lbar` line
  **byte-identical** to `--seeds=200` on
  `2026-09-13-recent-year-sessions.toml`; only `null p50`/`p95`, the percentile
  and the verdict word move, and none of those is read here. Every verdict side
  is read off the **published** row.
- **No drawdown criterion** (addendum 5 G: do not promise one on a
  non-drawdown job), although this binary prints it.
- `--config=` was normalised to this worktree's `config/`, so any published run
  that used a **copied** config dir (e.g. a `max_hold_ms` override) would not be
  reproduced faithfully. None of the 88 band commands echoed such a path; all
  echoed the default `config`.
- `data-sealed/` was **not** opened. `main`, `config/accounts.toml` and
  `config/local.toml` outside this worktree were not touched, `--data=` was read
  only, and the two `collect.exe` processes (pid 5044, 38720) were not touched.

## One operational note the desk should have

Two duplicate copies of my own driver were launched by accident early on and
could not be stopped from this session (the permission layer refused
`Stop-Process`, correctly — other agents' `search.exe` runs were live on the
same binary at the same time). They were left to run and wrote into a separate
directory that **no result above reads**; every number here comes from the
`--seeds=1` pass. Nothing was corrupted: runs are written to a `.part` file and
atomically renamed, and a run file is only read if it carries `# exit 0`.

## What would change this answer

- Re-running the 4,342 band rows that carry no argv would need each agent's own
  driver script recovered. That is the only way this axis gets from 452 cells to
  the whole band.
- The 51 vanished cells become readable only if the engine's trade-set drift is
  resolved first. That is a prior question to this one.
