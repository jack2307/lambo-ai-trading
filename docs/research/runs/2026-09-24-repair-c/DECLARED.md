# Task C — the declared set, committed before any horizon was run

Registration: `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`, task C.
This file is committed **before** the 24 h / 72 h / 168 h runs exist, so the set
cannot have been chosen after seeing which rows were interesting. The 4 h rows
already exist twice over (see below) and are the reproduction, not a selection.

## The method set — all eight rows of one published batch, none dropped

`docs/research/designs/2026-09-23-designed-1-contrast.toml`: eight
`far-stop-break` configurations, declared and published on 2026-09-23 as the
cost-term contrast. Chosen because:

- **Every one is `Exits::Engine`** (`far_stop_break.rs`, `fn exits`), so every
  one of them was force-closed at four hours whatever its logic intended. That
  is exactly the population the cap acted on.
- It is a **published receipt with its command line in its first line**
  (`docs/research/runs/2026-09-23-designed-1-cost-term/xauusd-guarded.txt`), so
  the before/after reproduction is against a number someone already printed.
- It is the batch whose own finding was about the horizon: it measured that the
  gross profit factor needed for net 1.20 falls from 1.32 at four hours to 1.21
  at a week. This is the direct test of that arithmetic.
- It contains the record's most horizon-sensitive engine-exit row,
  `struct-80-f14` at PF 1.065 / 100th percentile, **and** its own control arms.
  All eight are reported at all four horizons. Nothing is dropped for being
  dull and nothing is added for being interesting.

## The horizons

4 h (14,400,000 ms — the default, no override), 24 h (86,400,000),
72 h (259,200,000), 168 h (604,800,000).

## The two run families, per horizon

**Family 1 — the published path.** Fixed parameters over the whole window, the
count-calibrated matched null, guards on:

```
search --market=xauusd --interval=15m --data=E:/rust/flowdesk/data-sealed \
  --mode=hypotheses --batch-file=docs/research/designs/2026-09-23-designed-1-contrast.toml \
  --fixed --seeds=200 --from=2022-06-16 --to=2025-09-23 --guards --exit-mix \
  [--config=<dir with the override>]
```

Reports trades, PF, expectancy, matched-null p50/p95, percentile, the achieved
count match, guard activity, the exit mix and the gate verdict.

**Family 2 — both nulls.** The walk-forward path, which is the only one that
runs the direction null beside the matched one. `--rebate-share=0` is a credit
of nothing, so the gross and net columns are identical by construction and the
cost model is untouched:

```
search ... --mode=rescore --seeds=200 --direction-samples=1000 --rebate-share=0 --guards
```

Family 2's trade counts are per-fold out-of-sample and are **not** Family 1's
whole-window counts. The two are not interchangeable and are reported side by
side, not merged.

## How the horizon is stated

A copy of `config/` with one added `local.toml`, and nothing else:

```toml
[markets.xauusd.trading]
max_hold_ms = 86_400_000   # or 259_200_000 / 604_800_000
```

`config/` itself is never edited, `config/accounts.toml` is never touched, and
the shipped default stays at four hours. Every receipt prints its own
`max hold:` line naming the value and where it came from.

## Pre-committed readings

- **No threshold moves.** 30 trades / PF 1.20 / 0.05R; both nulls at the 95th.
- **A horizon that cannot reach 30 trades is unscoreable, not promising.**
- Percentiles are reported with their achieved count match beside them. The
  matched null is not cost-matched and the direction null is not count-matched;
  PF and expectancy carry the verdict.
- **If no verdict changes at any horizon the claim is refuted** and the cap was
  never binding in practice. That is the result, not a failure.
