# argv sidecar — f2

A receipt prints the window it resolved but not the argv that asked for it, and
25 of the record's 34 unreproducible rows come from an argv recorded with an
empty `--from`/`--to`. This file is that argv, plus the four things an argv
alone does not pin. **Nothing here is printed into the receipt**: the receipt
beside this file is byte-for-byte what it was before this file existed, which is
why the owner chose a sidecar over a header line.

Written by `search.exe`. Registration: `docs/decisions/2026-10-11-argv-sidecar.md`.
Classes referenced below are those of `docs/decisions/2026-10-11-receipt-reconcile-result.md`.

## 1. argv, verbatim

    argv[0] = C:\Users\Admin\AppData\Local\Temp\claude\e--nodejs-backcom-vantage\a5c7fd65-9276-457f-850d-25cb44e74c74\scratchpad\argvsc\bin\search-post.exe
    argv[1] = --market=xauduka
    argv[2] = --mode=hypotheses
    argv[3] = --config=E:/rust/fd-argv-sidecar/config
    argv[4] = --data=E:/rust/flowdesk/data
    argv[5] = --interval=15m
    argv[6] = --batch-file=E:/rust/fd-argv-sidecar/docs/hypotheses/2026-09-13-pdhl.toml
    argv[7] = --exit-mix
    argv[8] = --guards
    argv[9] = --from=2018-06-01
    argv[10] = --to=2026-06-01
    argv[11] = --argv-sidecar=C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/argvsc/post-flag/sidecar/f2.md

## 2. The window this run RESOLVED  (classes W1 / W2 / E1 — 26 rows)

    bars:         187860 of 378749 in the file
    first bar:    2018-06-01 00:00 UTC   (1527811200000 ms)
    last bar:     2026-05-31 23:45 UTC   (1780271100000 ms)
    passed:       --from=2018-06-01 --to=2026-06-01
    bar file:     E:/rust/flowdesk/data\bars\XAUDUKA-15m.parquet
                  sha256 b3f52dfb5b4f42393e331412c4ab958500c283b18cc88caf7e48d336cae35f65  (8366863 bytes, mtime 2026-09-13 15:45 UTC)
    bar digest:   sha256 f327fcf0c0befab377c75e53b85bd242784395d73e0a87552b5f75ac0322b72f
                  over the 187860 bars handed to the engine, in order,
                  (time, open, high, low, close, volume) little-endian, a tag
                  byte distinguishing a null volume from a literal 0.0.
                  This is the fingerprint of the exact input (class W1+C1: bars
                  rewritten INSIDE a pinned window). The parquet file is not
                  hashed — see the module doc.

## 3. replay — reproduces section 2 from THIS FILE ALONE

      C:\Users\Admin\AppData\Local\Temp\claude\e--nodejs-backcom-vantage\a5c7fd65-9276-457f-850d-25cb44e74c74\scratchpad\argvsc\bin\search-post.exe
      --market=xauduka
      --mode=hypotheses
      --config=E:/rust/fd-argv-sidecar/config
      --data=E:/rust/flowdesk/data
      --interval=15m
      --batch-file=E:/rust/fd-argv-sidecar/docs/hypotheses/2026-09-13-pdhl.toml
      --exit-mix
      --guards
      --from=2018-06-01
      --to=2026-06-01

    `--to` is exclusive and day-granular, so it is the day AFTER the last
    bar. If a replay prints a different `bars:` or a different bar digest, the
    store moved under a pinned window — this file makes that DETECTABLE and
    cannot make it reproducible.

## 4. Batch file  (class P1 — 4 rows)

    path:    E:/rust/fd-argv-sidecar/docs/hypotheses/2026-09-13-pdhl.toml
    sha256 87f1f51a095fbf93d45e7a20b0ec6d401dc136b8a0e15b16fc6ea436bb668b18  (755 bytes, mtime 2026-10-11 13:22 UTC)

    The hash pins WHICH TEXT produced the rows. It does not pin which axis of a
    sweep won: `vwap/allday` declared stretchAtr 1.5 and `vwap/wide` declared
    2.5 and both printed 5,828 trades / PF 0.951 / -0.029R digit for digit,
    because the grid overwrote the preset. The engine denounces that itself and
    names the file — `hypotheses.rs:1347-1351` — so the receipt beside this
    file carries it.

## 5. Config  (class G1 — 1 row)

    --config=E:/rust/fd-argv-sidecar/config
    default.toml   sha256 28061f366715a20d14aae0fbf0367169e9ae0dd6f4b871c1c5fef33d19dcb87b  (25497 bytes, mtime 2026-10-11 13:22 UTC)
    local.toml     absent

    Only the files `Config::load` actually opens are hashed. `accounts.toml`
    and `exclusions.toml` sit in the same directory and this run never opened
    them, so they are NOT hashed and this file does not pretend otherwise.

    A whole-file digest is OVER-sensitive: a comment edit moves it. That is the
    safe direction — it can raise a false alarm, it cannot miss G1
    (`flat_before_weekend_hhmm` 1655 -> 1640 on 2026-09-19 took one row from
    336 trades to 0). A list of the keys the engine reads was rejected: it is
    the same shape as the defect the source-scanning flag test in `search.rs`
    exists to prevent.

### 5b. The resolved effective values, serialised from the live structs

    Printed from the objects the engine is about to use, after --spread=,
    --trail= and any per-market override. This CANNOT drift: a field added to
    either struct appears here without anyone remembering to add it.

    [trading_rules]
    account_currency = "USD"
    commission_per_lot = 0.0
    contract_size = 1.0
    fallback_atr_period = 14
    leverage = 1.0
    lot_step = 0.01
    max_hold_ms = 14400000
    min_lot = 0.01
    news_currencies = ["USD"]
    price_decimals = 2
    reward_risk = 1.8
    risk_per_trade_pct = 0.01
    spread = 0.28
    starting_equity_usd = 10000.0
    stop_atr = 1.2
    swap_long_per_lot = 0.0
    swap_short_per_lot = 0.0
    trail.activate_r = 1.0
    trail.distance_r = 1.0
    trail.enabled = false
    units_per_usd = 1.0

    [guards]
    cooldown_ms = 1800000
    daily_loss_limit_usd = 300.0
    flat_before_weekend_hhmm = 1640
    max_concurrent_positions = 1
    max_notional_pct_equity = 300.0
    max_open_loss_r = 2.0
    max_trades_per_day = 20
    news_currencies = ["USD"]
    news_flat_after_min = 30
    news_flat_before_min = 60
    news_min_impact = 3

## 6. Binary and run

    commit:    6e38f0acc7cfa24f71581ac5207b07ec39526e56  (working tree dirty at build time)
               Captured by build.rs at BUILD time, which is what it is: if
               the only edit since the build was outside this crate, the
               `dirty` word can be stale. The commit is the reproduction base
               because two independent --release builds of one commit were
               measured producing byte-identical output (agent/hold-cap).
    run date:  2026-10-12 02:21 UTC
