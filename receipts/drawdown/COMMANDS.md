# `agent/drawdown` receipts — 2026-10-07

Every run below used `E:/rust/fd-drawdown/target/release/search.exe`, built from
`agent/drawdown` (which is `agent/instr-repair` plus the drawdown printer), and
the **live** store `--data=E:/rust/flowdesk/data`. `data-sealed` was not opened.
News came from `E:/rust/flowdesk/data/news/events.parquet` (747 events,
2010-01-08 → 2027-12-08) in every run, as each receipt's own `news:` line says.

`--exit-mix` on every run, so each row states whether its own rule fired.

| receipt | command (after `search.exe`) |
|---|---|
| `orb-ny-in-sample.txt` | `--market=xauusd --interval=5m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-orb-ny.toml --seeds=200 --exit-mix` |
| `london-range-in-sample.txt` | `--market=xauusd --interval=5m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-london-range.toml --seeds=200 --exit-mix` |
| `recent-year-hours-in-sample.txt` | `--market=xauusd --interval=15m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-recent-year-hours.toml --seeds=200 --from=2025-09-13 --to=2026-09-12 --exit-mix` |
| `recent-year-hours-oos-xauduka.txt` | `--market=xauduka --interval=15m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-recent-year-hours.toml --seeds=200 --from=2022-06-16 --to=2025-04-10 --exit-mix` |
| `recent-year-sessions-in-sample.txt` | `--market=xauusd --interval=15m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-recent-year-sessions.toml --seeds=200 --from=2025-09-13 --to=2026-09-12 --exit-mix` |
| `sessions-5m-oos-xauduka.txt` | `--market=xauduka --interval=5m --mode=hypotheses --batch-file=docs/hypotheses/2026-09-13-recent-year-sessions-5m.toml --seeds=200 --from=2022-06-16 --to=2025-04-10 --exit-mix` |
| `close-reopen-in-sample-fixed.txt` | `--market=xauduka --interval=15m --mode=hypotheses --fixed --batch-file=docs/hypotheses/2026-09-13-close-reopen-drift.toml --seeds=300 --from=2018-06-16 --to=2025-04-10 --exit-mix` |
| `tsmom-2-in-sample-fixed.txt` | `--market=xauduka --interval=15m --mode=hypotheses --fixed --batch-file=docs/hypotheses/2026-09-13-tsmom-2.toml --seeds=200 --from=2018-06-16 --to=2025-04-10 --exit-mix` |
| `tsmom-silver-in-sample-fixed.txt` | `--market=xagduka --interval=15m --mode=hypotheses --fixed --batch-file=docs/hypotheses/2026-09-13-tsmom-silver.toml --seeds=300 --from=2010-06-01 --to=2018-06-15 --exit-mix` |
| `tsmom-eurusd-in-sample-fixed.txt` | `--market=eurduka --interval=15m --mode=hypotheses --fixed --batch-file=docs/hypotheses/2026-09-14-tsmom-eurusd.toml --seeds=300 --from=2010-06-01 --to=2018-06-15 --exit-mix` |
| `ict-m1-in-sample.txt` | `--market=xauusd --interval=1m --mode=hypotheses --batch=ict-m1 --seeds=200 --to=2026-09-12 --exit-mix` |
| `designed-1-struct80-live.txt` | `--market=xauusd --interval=15m --mode=hypotheses --batch-file=docs/research/designs/2026-09-23-designed-1-contrast.toml --fixed --seeds=200 --guards --from=2022-06-16 --to=2025-09-23 --exit-mix` |
| `designed-1-struct80-era2-h72-live.txt` | same batch, `--from=2023-06-16 --to=2024-06-16 --guards --fixed --seeds=200 --exit-mix --config=<copy of config/ with `max_hold_ms = 259_200_000`>` |
| `rescore-smoke-orb.txt` | `--market=xauusd --interval=5m --mode=rescore --batch-file=docs/hypotheses/2026-09-13-orb-ny.toml --seeds=20 --direction-samples=50 --rebate-share=0.5 --exit-mix` — a smoke test of the second printer, not a measurement |
| `tsmom-gold-in-sample.txt` | **incomplete and not read.** `--market=xauduka --interval=5m --batch-file=docs/hypotheses/2026-09-13-tsmom.toml …` was started on the wrong receipt (that batch's published rows are all fails) and stopped mid-run. Kept only so the directory does not look edited. |

Two published rows could **not** be re-run as published and were re-run on the
live store instead, which is said in the decision record and in §R0 of it:
`struct-80` / `struct-80-f14` ran originally on `data-sealed` with a 72 h hold
config that no longer exists. Their figures reproduced exactly anyway.

`collect.exe --market=gold` (pid 5044) and `--market=btc` (pid 38720) were not
touched, and `/e/rust/flowdesk/data` was read only.
