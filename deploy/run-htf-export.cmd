@echo off
rem  The scheduled task's action for the higher-timeframe bar export. Same
rem  shape and the same reasons as run-fd-api.cmd beside it - read that one
rem  for why this is a .cmd, why it is not PowerShell, and why the boundary
rem  line matters.
rem
rem  WHAT THIS EXISTS FOR. `GET /api/paper/htf` reads
rem  data\bars\XAUUSD-4h.parquet and -1d.parquet. `data\` is gitignored, so
rem  no commit can ever ship those files: a merge that brings the route and
rem  the exporter brings no bars, and the route then answers "not exported
rem  yet" correctly while the deploy reports success. This is the step that
rem  keeps them current, and deploy\update.ps1 refuses to report ready when
rem  the route says the bars are missing for a market the desk is trading.
rem
rem  WHY HOURLY AND NOT SIX TIMES A DAY. The broker's H4 candles close at
rem  01/05/09/13/17/21 UTC, and this machine's clock is not UTC. An hourly
rem  trigger at five past needs no knowledge of the offset - every H4 close
rem  is followed by an export within the hour - while six fixed local times
rem  would silently point at the wrong candles the first time anyone changed
rem  the timezone or the DST rule moved under them.
rem
rem  IT IS READ-ONLY AND IT MERGES. mt5_export.py calls initialize,
rem  symbol_select, symbol_info, copy_rates_from_pos and shutdown. It places
rem  no order, closes none, and needs no AutoTrading. Re-running it produces
rem  the same bar counts - verified by d1 - because it keys bars by time and
rem  merges rather than truncating, so a missed hour is repaired by the next
rem  run instead of leaving a hole.
rem
rem  THE DEMO TERMINAL, NAMED EXPLICITLY, AND NEVER THE CENT ONE. Without
rem  --terminal the MetaTrader5 package attaches to whichever terminal is
rem  already running, which is a coin toss on a machine with two. C:\MT5-cent
rem  holds the FUNDED account (config\accounts.toml); prices are identical on
rem  both - the standard and cent symbols differ in contract size, not in
rem  price - so there is no reason to touch the funded terminal and every
rem  reason not to.
rem
rem  XAUUSD AND NOT XAUUSD.sc, and the distinction is the terminal's not a
rem  preference: py\live\start_pollers.ps1 records that the cent account
rem  carries XAUUSD.sc and "standard XAUUSD etc - what the demo account
rem  carries". This attaches to the DEMO terminal, so the symbol has no
rem  suffix. mt5_export strips a .sc anyway when it names the file, so either
rem  spelling would land at XAUUSD-4h.parquet - it is symbol_select that
rem  cares, not the path.
rem
rem  AND IF THE SYMBOL IS WRONG, mt5_export LOGS "skipped" AND EXITS 0. That
rem  is a task which runs every hour, writes nothing, and reports success -
rem  so the parquet files are checked for here rather than assumed. The check
rem  is existence only; whether they are CURRENT is what
rem  deploy\update.ps1's readiness probe and the route's own age fields are
rem  for.
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
set "LOG=%LOGDIR%\htf-export.out"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

>>"%LOG%" echo(
>>"%LOG%" echo ==== htf export start %DATE% %TIME% ====

cd /d "%ROOT%"
"C:\Python39\python.exe" "%ROOT%\py\ingest\mt5_export.py" --symbols XAUUSD --timeframes H4,D1 --terminal "C:\MT5-demo\terminal64.exe" >>"%LOG%" 2>&1
set "RC=%ERRORLEVEL%"

rem  A zero exit is not evidence that anything was written. Say so in the log
rem  where the next person looks, and carry a non-zero code out so the task's
rem  own Last Run Result stops reading 0x0 while the desk has no bars.
if not exist "%ROOT%\data\bars\XAUUSD-4h.parquet" (
  >>"%LOG%" echo ==== NO XAUUSD-4h.parquet AFTER THIS RUN. The export wrote no H4 bars.
  >>"%LOG%" echo ==== Most likely the symbol is not XAUUSD on this terminal, or no terminal
  >>"%LOG%" echo ==== was attachable. mt5_export logs "symbol_select failed - skipped" and
  >>"%LOG%" echo ==== still exits 0, so read the lines above this one.
  set "RC=2"
)
if not exist "%ROOT%\data\bars\XAUUSD-1d.parquet" (
  >>"%LOG%" echo ==== NO XAUUSD-1d.parquet AFTER THIS RUN. Same causes as above.
  set "RC=3"
)
>>"%LOG%" echo ==== htf export end %DATE% %TIME% rc=%RC%
exit /b %RC%
