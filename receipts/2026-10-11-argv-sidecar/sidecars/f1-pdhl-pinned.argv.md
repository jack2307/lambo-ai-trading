# argv sidecar — pdhl-B

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
    argv[2] = --interval=5m
    argv[3] = --mode=hypotheses
    argv[4] = --batch-file=E:/rust/fd-argv-sidecar/docs/hypotheses/2026-09-13-pdhl.toml
    argv[5] = --seeds=20
    argv[6] = --to=2025-04-10
    argv[7] = --from=2022-06-16
    argv[8] = --data=E:/rust/flowdesk/data
    argv[9] = --config=E:/rust/fd-argv-sidecar/config
    argv[10] = --argv-sidecar=C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad/argvsc/f1/sc/pdhl-B.md

## 2. The window this run RESOLVED  (classes W1 / W2 / E1 — 26 rows)

    bars:         198716 of 1135389 in the file
    first bar:    2022-06-16 00:00 UTC   (1655337600000 ms)
    last bar:     2025-04-09 23:55 UTC   (1744242900000 ms)
    passed:       --from=2022-06-16 --to=2025-04-10
    bar file:     E:/rust/flowdesk/data\bars\XAUDUKA-5m.parquet
                  sha256 483b13c5ed31eaa14a1df1f5681e680df2e49c638f4a79b5fda508d0675e970b  (23331018 bytes, mtime 2026-09-13 15:44 UTC)
    bar digest:   sha256 f09dc35a81f681b8917bd4697a76419354ba9f8454d6d135d6874c3e2405ce6b
                  over the 198716 bars handed to the engine, in order,
                  (time, open, high, low, close, volume) little-endian, a tag
                  byte distinguishing a null volume from a literal 0.0.
                  This is the fingerprint of the exact input (class W1+C1: bars
                  rewritten INSIDE a pinned window). The parquet file is not
                  hashed — see the module doc.

## 3. replay — reproduces section 2 from THIS FILE ALONE

      C:\Users\Admin\AppData\Local\Temp\claude\e--nodejs-backcom-vantage\a5c7fd65-9276-457f-850d-25cb44e74c74\scratchpad\argvsc\bin\search-post.exe
      --market=xauduka
      --interval=5m
      --mode=hypotheses
      --batch-file=E:/rust/fd-argv-sidecar/docs/hypotheses/2026-09-13-pdhl.toml
      --seeds=20
      --data=E:/rust/flowdesk/data
      --config=E:/rust/fd-argv-sidecar/config
      --from=2022-06-16
      --to=2025-04-10

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

    [guards] = null — --guards was not passed, so no guard was installed.
    That is NOT every guard at zero: an unguarded run has no guard object, and
    `sized_down_by_guard` on such a row is null, not 0.

## 6. Binary and run

    commit:    6e38f0acc7cfa24f71581ac5207b07ec39526e56  (working tree dirty at build time)
               Captured by build.rs at BUILD time, which is what it is: if
               the only edit since the build was outside this crate, the
               `dirty` word can be stale. The commit is the reproduction base
               because two independent --release builds of one commit were
               measured producing byte-identical output (agent/hold-cap).
    run date:  2026-10-12 02:43 UTC
