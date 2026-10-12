# 2026-10-11 — RESULT: the target ceiling moves `PF_r`, and hands the trade to the clock

**Registration:** `docs/decisions/2026-10-11-reward-risk.md`, committed alone at
`885d021` before the first line of code.
**Instrument:** `1655950` (`--reward-risk=<ratio>` + `tests/reward_risk_flag.rs`).
**Branch:** `agent/reward-risk`, worktree `E:/rust/fd-reward-risk`, cut from `29c8635`.
**Receipts:** `receipts/2026-10-11-reward-risk/` (40 grid files, 11 control
files, `grid.csv` = 120 cells). Argv: `receipts/2026-10-11-reward-risk/ARGV.md`.

---

## 1. The one-sentence answer

**`PF_r` IS a function of `reward_risk` — on one of the three rows the flag can
reach, by up to 0.1620 of `PF_r`, in both windows, both guard arms and both swap
arms with the same shape — and the shipped `1.8` is close to the WORST value on
that row. But the function tops out at `PF_r 0.9380`, which is 0.262 short of the
gate, and the vacated TARGET share arrives at TIMEOUT rather than at STOP: the
target ceiling was real, and moving it hands the trade to the clock.**

## 2. The gate: 0 of 120 cells

    cells declared 120   ·   cells run 120   ·   cells through the gate 0

    cells with PF_r >= 1.200        0 / 120
    cells with E    >= +0.050 R     0 / 120
    best PF_r anywhere              0.9380   (config, W2, guards off, rr/ema-cross, reward_risk = 7.2,
                                              E -0.0367 R, n 1496, max_drawdown 69.66% — book intact)
    cells below the desk's 40 trades  0 / 120   (counted by hand from `grid.csv`; smallest n = 1209)

**Which leg binds: the PF leg, and only it.** `Lbar` is **0.2873 – 0.6144 R**
across all 120 cells and **0 cells** fall below 0.250 R, so by the identity
`E = Lbar × (PF_r − 1)` the expectancy leg is **redundant on every cell**
(addendum 6 §I). Reporting this as "failed three legs" would be wrong: it failed
one.

The identity itself held **120/120, worst residual 0.00008 R**.

## 3. F1 — DOES NOT FIRE. `PF_r` really does move.

Span of `PF_r` over the five declared ratios, per row-arm:

| row | W1 off | W1 on | W2 off | W2 on | arm B W1 off | arm B W2 off |
|---|---|---|---|---|---|---|
| **`ema-cross`** | **0.0957** | **0.0943** | **0.1607** | **0.1407** | **0.0964** | **0.1620** |
| `donchian-breakout` | 0.0354 | 0.0281 | 0.0333 | 0.0301 | 0.0354 | 0.0334 |
| `rsi-reversion` | 0.0240 | 0.0275 | 0.0479 | 0.0222 | 0.0242 | 0.0483 |

**8 of 24 row-arms reach the 0.050 threshold, and all eight of them are
`ema-cross`** — every arm it has. F1 required *every* family row in *both*
windows to fail to move; one row moves in both windows, both guard arms and both
swap arms, so **F1 does not fire and this is not window artefact #13**: the
registration's amendment clause demanded the same sign in both windows, and the
shape is the same curve in all eight.

The curve, `config` arm A (`PF_r` at reward_risk 0 / 0.9 / 1.8 / 3.6 / 7.2):

    ema-cross W1 off   0.8776  0.7879  0.8236  0.8465  0.8836
    ema-cross W1 on    0.8534  0.7591  0.7945  0.8264  0.8406
    ema-cross W2 off   0.9181  0.8119  0.7773  0.8885  0.9380
    ema-cross W2 on    0.8761  0.7887  0.7522  0.8611  0.8929

It is **U-shaped, with the minimum at the middle of the sweep and the maxima at
both ends.** Two things follow that the record did not have before:

1. **The shipped `reward_risk = 1.8` is near the pessimal value on this row.**
   In W2 it is the worst of the five (0.7773 off / 0.7522 on) and `0` and `7.2`
   are both ~0.14–0.16 of `PF_r` better. Every published `Exits::Engine`
   derived-target measurement in `docs/decisions/` was taken at that value.
2. **The two ends are the same thing.** At `7.2` the target is reached on
   0.58–1.34% of trades, so a 7.2 R target is operationally "no target". The
   curve therefore has one maximum, at *no target*, and `0` and `7.2` are its
   two spellings. That is the setting `Exits::Engine` could not express before
   this flag.

`donchian-breakout` and `rsi-reversion` are flat. On `rsi-reversion` that was
predicted in the registration and the reason is in its own exit mix: **the
derived target was never its exit** — its own "RSI back to midline" rule closes
32–61% of its trades and TARGET closes 1.8–6.6%.

## 4. F2 — FIRES. The vacated share goes to the CLOCK, not to the stop.

`--exit-mix` at every ratio, as a share of each row's own exits. `ema-cross`,
`config`, moving from the shipped 1.8 to 0 (no target at all):

| arm | TARGET | STOP | TIMEOUT | other | mean hold |
|---|---|---|---|---|---|
| W1 off, rr 1.8 | 22.97% | 47.91% | 25.11% | 4.01% | 184.0 min |
| W1 off, rr 0 | **0%** | 51.55% | **45.78%** | 2.67% | **237.4 min** |
| change | **−22.97** | **+3.64** | **+20.67** | −1.34 | +53.4 min |
| W2 off, rr 1.8 | 21.12% | 52.13% | 23.47% | 3.27% | 168.3 min |
| W2 off, rr 0 | **0%** | 52.27% | **44.72%** | 3.01% | **212.8 min** |
| change | **−21.12** | **+0.14** | **+21.25** | −0.26 | +44.5 min |

**90% of the vacated target share in W1, and essentially 100% of it in W2,
arrives at TIMEOUT.** The stop barely moves. And the smoking gun is the hold:
with no target, the mean hold is **237.4 min against a cap of 240 min** — the
clock becomes the binding exit *immediately*, not eventually.

So the result is reported in the registration's words: **one ceiling swapped for
another.** `agent/hold-cap` approached the same wall from the other side and
measured `G_session` flat when the clock was lifted (trips ÷8.30 × G_trip ×9.90 =
×1.19, 8/12 combinations ≤1.00). This job lifts the target and the clock takes
the slack; that job lifts the clock and the stop takes it (TIMEOUT → STOP at 5/6
window pairs). The two measurements close on each other.

**And the falsifier's own conclusion now has all three of its terms:** stop
(`agent/stop-width`, `range-sizing`), target (here), clock (`agent/hold-cap`).
All three have now been moved on `Exits::Engine` and **none of them is what is
binding** — the best cell in this grid is still 0.262 of `PF_r` short of 1.200.
The one correction to the falsifier's wording is that it assumed `PF_r` would
*not* move; it does, by up to 0.1620, and that is the single most valuable number
this job produced. It is just not worth 0.262.

## 5. F3 — carry. Not measurable on 104 of 120 cells, and negative on the rest.

**104 of 120 cells printed `max_drawdown_pct > 100%`: the book blew** and
`PF_usd`, `net_usd` and arm-B `expectancy_net` are not read on them (addendum 6
§IV). The 16 survivors are all `ema-cross`; `donchian` and `rsi-reversion` blew
in every cell of the grid.

On arm B (`config-swap`, −0.83 USD per lot-night), `expectancy_net` is printed
on every cell because swap ≠ 0, and it is **more negative than arm A's `E` at
every single one**:

    ema-cross W2 off  rr=7.2   E -0.0368 R  ->  expectancy_net -0.077 R   (gap -0.040 R/trade)
    ema-cross W2 off  rr=1.8   E -0.1303 R  ->  expectancy_net -0.164 R   (gap -0.034 R/trade)
    ema-cross W2 on   rr=0.0   E -0.0747 R  ->  expectancy_net -0.104 R   dd 99.84%  <- readable
    ema-cross W2 on   rr=3.6   E -0.0804 R  ->  expectancy_net -0.110 R   dd 93.63%  <- readable
    ema-cross W2 on   rr=0.9   E -0.1007 R  ->  expectancy_net -0.126 R   dd 87.54%  <- readable

F3's "an account perk, not a strategy" clause **does not apply**, because no cell
rises above the gate in either arm — there is no perk to attribute. What carry
does do is **blow the book harder**: the same cell reads `max_drawdown_pct`
161.47% in arm A and 192.85% in arm B. And **the ranking survives carry**: 7.2
is still the best of the five ratios after financing.

⚠️ **Only 3 of 20 arm-B `ema-cross` cells have an intact book**, so 17 of them
carry an `expectancy_net` that is contaminated by `min_lot` clamping
(`engine.rs:729` feeding `risk_usd` in `engine.rs:963`). Those are printed with
the contamination named and are **not** used for the ranking.

What stays **`null`**: the rate Dukascopy-era gold would actually have paid in
2010. Arm A is the desk's own *measured* swap-free account and arm B is the
generic rate; neither is a null, and the third thing was never measurable here.

## 6. The instrument. All five controls passed, and one of them taught something.

**C1 — byte-identity, and the baseline is a COMMIT.**

    two INDEPENDENT --release builds of 29c8635, five argv:   diff -r clean, 5/5
    post-change binary, NO FLAG, same five argv:              diff -r clean, 5/5

The two 29c8635 binaries differ in size (9,732,411 vs 9,732,413 bytes) and
produce byte-identical output, so the reproducible unit is the commit rather than
a binary. **F4 does not fire; the measurement is publishable.** Nothing was
relaxed to "approximately".

**C2 — the probe was watched to FAIL before the flag was declared.** The
reading code went in first; `cargo test --release -p fd-backtest --bin search`
then reported

    flag_tests::every_flag_the_source_reads_is_declared_in_the_table ... FAILED
    these flags are READ by search.rs and not DECLARED in ALWAYS/BY_MODE: ["reward-risk"]

with the other 12 tests passing, so the scanner was not vacuous. `reward-risk`
was added to `ALWAYS` only after that.

**C3 — near misses denounced, bad values are hard errors.**

    --rewardrisk=3        "** --rewardrisk is not a flag this binary has; it changed nothing **"   exit 0
    --reward_risk=3       denounced likewise                                                       exit 0
    --reward-risk-ratio=3 denounced likewise                                                       exit 0
    --reward-risk=        Error: wants a number, 0 meaning no target at all, got ``                exit 1
    --reward-risk=abc     Error: wants a number ...                                                exit 1
    --reward-risk=-1.8    Error: cannot be negative: -1.8 would put the target on the losing side  exit 1
    --reward-risk=nan     Error: wants a finite number ...                                         exit 1
    --reward-risk=1.8e400 Error: wants a finite number ...   (overflow to inf is caught)           exit 1

And the known `arg()` first-match behaviour is now **visible rather than
silent**: `--reward-risk=3.6 --reward-risk=7.2` uses 3.6 and the header line says
`--reward-risk=3.6`.

**C4 — the explicit-target invariance, which is the brief correction measured.**
`orb`, `keltner-break`, `far-stop-break` at no-flag / 0 / 1.8 / 7.2:

    ex/orb60     n 1072  PF 1.020  expectancy +0.010   identical at all four values
    ex/keltner   n 2082  PF 0.935  expectancy -0.028   identical at all four values
    ex/far-stop  n 3052  PF 0.920  expectancy -0.020   identical at all four values

**0 trades change.** `--reward-risk=` cannot reach them, because each supplies
its own target from its own `riskReward` parameter. **F5 does not fire** — it
would have fired if these rows had moved, and §0.2 of the registration would then
have been wrong. The paired positive control in the same measurement is the
derived-target family of §3, which the same two values move by up to 0.1620.

Self-managed rows (`tsmom`, `quiet-swing`, `session-hold`,
`intraday-momentum`): the whole receipt is byte-identical at no-flag / 0 / 1.8 /
7.2 apart from the header line the flag adds — **method AND null unchanged**.

**C5 — the blind probe, which found a second thing.** A fixture on a flat tape
reads **outcome-invariant** at every ratio (not a trade, not a dollar, not an
exit moves — every exit is the clock) while being squarely inside the family rule.
That is invariance for a non-causal reason, and it is why "it did not move" is
never evidence on its own. Writing it exposed a distinction the record did not
have:

> **Outcome-invariant and byte-invariant are not the same claim.** At
> `reward_risk = 0` the engine stops *recording* a target it was never going to
> reach, so the `target` field of every trade changes from `Some(..)` to `None`
> while entry, exit, R, dollars and exit kind are bit-identical. A diff of
> receipts cannot tell that apart from a behaviour change. The explicit-target
> and self-managed rows above are invariant in **both** senses; the blind probe
> only in the weaker one.

**Suite:** `cargo test --release -p fd-backtest` — **201 passed, 0 failed**
across 22 test binaries (the 5 new ones among them).

## 7. THE NULL MOVES WITH THE FLAG — so no percentile is published

This was declared in the registration as a measurement, not a hope, and it is
confirmed with numbers. `control.rs:150` enters with `target: None` and
`RandomEntry` takes the default `Exits::Engine` (`registry.rs:256`), so the
control is **re-specified at every ratio**.

How far the control's own line moves across the five ratios, same row, same
window, same arm:

    config W1 off ema-cross    null p50 0.777..0.829 (d 0.052)   null p95 0.831..0.913 (d 0.082)
    config W1 off rsi-reversion  null p50 0.775..0.835 (d 0.060)   null p95 0.831..0.902 (d 0.071)
    config W1 off donchian     null p50 0.805..0.843 (d 0.038)   null p95 0.850..0.906 (d 0.056)
    null trade count, donchian W2 off:   4,414 .. 5,820   (x1.32 across the axis)

**The control's p95 moves 0.082 on a row whose method moves 0.0957.** The line
moves 86% as far as the thing being measured against it. ⇒ **no percentile in
this job is published**, and the gate legs carry the whole verdict.

And a refinement of addendum 11 §VI, measured here: **the drift is not
universal, it depends on which control the row is given.**
`hypotheses.rs:1167` sets `drift = exits() == Exits::Strategy && grid_empty`;
a drift row gets `RandomHold`, which enters with **no stop**, so
`engine.rs:740`'s `stop.is_none()` branch refuses to derive a target and the
control is immune. Measured: self-managed receipts byte-identical across ratios,
engine-exit receipts not. So:

    Exits::Engine row  -> control is RandomEntry (ATR stop) -> derives a target -> NULL MOVES
    drift row          -> control is RandomHold (no stop)   -> no target        -> null frozen

**`--seeds=` moves the control and never the method**, measured on the same
argv: method columns byte-identical at `--seeds=200` and `--seeds=20`, while
null p50 moved 0.886 → 0.899, 0.901 → 0.922 and 0.885 → 0.896. Another reason a
percentile on this axis is not a measurement.

## 8. `[run]` in the batch file is INERT for this flag — measured, not asserted

`docs/hypotheses/2026-10-11-reward-risk-runinert.toml` carries
`[run] reward_risk = 9.0` on purpose. A run against it is **byte-identical** to
the same run against the file without the key, and prints **no**
`reward/risk:` header line. `batch_from_file` (`hypotheses.rs:103-105`)
deserialises only `[[hypothesis]]`, so the key is accepted without a complaint
and obeyed by nobody — the same inert layer `[run] null_sides` and
`[run] max_hold_ms` sit in.

## 9. Everything else the brief told me to print

* **`PF_usd` beside `PF_r`, measured on MY data and nobody's ratio quoted:**
  `PF_usd` reads **LOWER** than `PF_r` on **73/120 cells (60.8%)** and higher on
  47. **0 cells** have the two on opposite sides of the 1.200 line, and the
  largest `|gap|` is **0.1229**. That is a tenth measurement of a quantity that
  has now given at least four different answers; it is reported, not
  generalised.
* **`sized_down_by_guard` read from the printed `guards:` line**
  (`hypotheses.rs:752`), never grepped. On guards-ON `ema-cross` the notional
  cap sizes down **86.0 – 93.5% of entries** at every ratio, and both gate legs
  are blind to it. Guards-off cells are **`null`, not 0**.
* **Both window ends pinned on all 40 runs**, and the bar count printed by every
  one of them: **W1 = 190,889 bars, W2 = 187,860** — one value each, so no
  comparison here is across different tape.
* **No number appears in any `why =` prose** in the four batch files.
* **`wrong_side_stop` was not read**: there is no counter for it on this tree
  (addendum 8 §III.2), so it is `null`, not 0.
* **A cell that only survives with guards off**: not applicable — the only
  intact books here include both arms, and `ema-cross` W1 guards-ON at
  `reward_risk = 0` (`max_drawdown 75.59%`) is intact in **the arm the owner
  permits**, which is the one thing in this grid that would have been reportable
  as a candidate had its `PF_r` been 0.35 higher.

## 10. What was NOT measured, and why

1. **`G_session`** — deliberately not computed. It needs `gross_usd`, which
   needs `lots`, and `min_lot` clamping destroyed `lots` on 104 of 120 cells.
   Publishing it would be publishing a sizing artefact.
2. **Four of the seven rows the flag can reach.** `level-reversion`,
   `flow-momentum` and `flow-at-level` all `needs_options()` and took **0
   trades** on both 8-year windows — the receipt itself prints
   "max drawdown: NOT MEASURED — this cell took no position" and "Lbar: NOT
   MEASURED", which is `null`, not 0. `companion-unconfirmed` needs a companion
   market and was not run. **So this axis was measured on 3 of the 7 rows that
   exist, and the 4 missing ones are `null`.**
3. **Whether `reward_risk` matters on a row that is not losing money.** Every
   cell in this grid has `PF_r < 1`. On a losing book a wider target is
   mechanically better because it stops converting unrealised gains into
   realised small wins; whether the same curve shape holds on a profitable row
   **cannot be answered from this grid**, because the record contains no
   profitable derived-target `Exits::Engine` row to run it on.
4. **L3.** `reward_risk` is a multiple of risk, so this job moved L2's ceiling,
   not L3's. Addendum 11 §IV.A's point stands untouched: a target that is a
   **price level** is still the only unexplored shape, and `--reward-risk=0`
   now makes it expressible for the first time (a strategy can supply a level
   while the engine adds no target of its own).

## 11. Disk and infrastructure

    df -h /e   before  14 GB free      after  13 GB free      (floor is 5 GB)

`target-rr/` is 1.0 GB, `--release` only, `-p fd-backtest` only. **Nothing was
deleted.** The overnight dip and recovery is the pagefile, as addendum 11 §IX
says. Both `collect.exe` processes (pids 5044 and 38720) were verified alive
after the one `taskkill` this job issued (which targeted an already-exited
`search.exe` and reported "not found").

**One environment note worth recording:** a `search.exe` started with
`nohup ... &` *inside* a foreground Bash call is reaped when that call's shell
exits, which made `--mode=costs` look like a crash three times (it stopped at
exactly the same strategy each time, with an empty stderr). Run in the
foreground it exits **0**. The fixture was re-cut to a one-year window for cost
and the five fixtures are what `ARGV.md` records.

---

## 12. One line for the next brief

> **`reward_risk` is now sweepable (`--reward-risk=`, CLI only, default
> byte-invariant), `1.8` is near the pessimal value on the one row it moves, and
> the whole axis is worth up to +0.1620 of `PF_r` — which leaves the best cell
> 0.262 short of the gate while handing 90–100% of the vacated target share to
> the 4 h clock. All three `Exits::Engine` ceilings have now been moved and none
> of them is what is binding. The 1.8 R target was a real ceiling and it was not
> the wall.**
