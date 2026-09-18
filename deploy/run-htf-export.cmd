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
rem  THE SAME SERIES THE BOOKS TRADE, WHICH IS THE CENT ONE. Without
rem  --terminal the MetaTrader5 package attaches to whichever terminal is
rem  already running, which is a coin toss on a machine with two. Named
rem  explicitly, and named as the CENT terminal on purpose.
rem
rem  An earlier draft of this file pointed at the demo terminal and the
rem  unsuffixed symbol, reasoning that the funded terminal should not be
rem  touched without cause. That was wrong on the facts. The four pollers on
rem  the VPS run --symbol=XAUUSD.sc against C:\MT5-cent\terminal64.exe
rem  (read out read-only from the server, 2026-09-18), so the 15m bars every
rem  book decides on ARE the cent symbol's from the cent terminal. And
rem  config\default.toml says the same thing from the other end: the xauusd
rem  market is labelled "Vantage XAUUSD.sc CFD".
rem
rem  So H4 and D1 must come from that series too. The two symbols quote the
rem  same price and differ in contract size, so bars from the demo terminal
rem  would be very nearly right - which is worse than plainly wrong, because
rem  "the higher timeframe disagrees with the chart by a tick sometimes" is a
rem  discrepancy nobody would trace. One series, one source.
rem
rem  READ-ONLY ON THE FUNDED TERMINAL, which is the house rule and is not
rem  bent here. mt5_export calls initialize, symbol_select, symbol_info,
rem  copy_rates_from_pos and shutdown. It sends no order, closes none, and
rem  needs no AutoTrading - the same access the pollers have had against this
rem  terminal since day one.
rem
rem  XAUUSD.sc GOES IN AND XAUUSD-4h.parquet COMES OUT. mt5_export strips the
rem  suffix when it names the file (`symbol.split(".")[0]`, unless
rem  --keep-suffix), and htf.rs resolves the file as `{spec.bar_symbol}-4h`
rem  where default.toml sets bar_symbol = "XAUUSD". Both ends checked. The
rem  consequence to remember when verifying: the FILENAME is identical
rem  whichever symbol is passed, so the file existing proves nothing about
rem  which series is in it - only the skip line in the log does.
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
"C:\Python39\python.exe" "%ROOT%\py\ingest\mt5_export.py" --symbols XAUUSD.sc --timeframes H4,D1 --terminal "C:\MT5-cent\terminal64.exe" >>"%LOG%" 2>&1
set "RC=%ERRORLEVEL%"

rem  EXIT CODES, AND WHOSE THEY ARE.
rem
rem    0      everything asked for was written
rem    1      the exporter wrote nothing
rem    2      the exporter wrote SOME of what was asked for
rem    10     this wrapper found no H4 parquet afterwards
rem    11     ... no D1 parquet
rem    12     ... neither
rem
rem  0, 1 and 2 are mt5_export's (d1, merged at bac3c4b); 10, 11 and 12 are
rem  this file's, numbered clear of them so a Last Run Result says which layer
rem  objected. Three codes and not two because the first draft had the D1
rem  check overwrite the H4 one: with both files missing it exited 11, "no
rem  D1", while the more serious half went unnamed in the code a pager would
rem  show. Caught by running the batch against a stub exporter rather than by
rem  reading it. Before that merge the exporter returned 0 whatever happened -
rem  a skipped symbol, or nothing written at all - which is why the two
rem  existence checks below were written and why they stay.
rem
rem  PARTIAL IS FATAL HERE, and that was a decision rather than a default.
rem  d1 left the call to this file on the grounds that the exporter cannot
rem  know what the files are for. This task asks for exactly two timeframes
rem  and the route needs both: if H4 keeps working while D1 stops, a partial
rem  reported as success reads green forever while the daily levels quietly
rem  go stale - the same defect one level down. So 2 is passed straight out
rem  as a failure and the hour's log says which half was missing.
rem
rem  WHAT THE EXISTENCE CHECKS BELOW CANNOT SEE, stated so nobody trusts them
rem  further than they go: a file that exists and has STOPPED being updated
rem  passes them every time. They protect the first run. After that the exit
rem  code is the only thing watching, which is why it had to be fixed rather
rem  than worked around here.
rem  The flags are set inside the blocks and READ outside them. %VAR% inside a
rem  parenthesised block expands when the block is parsed, not when it runs,
rem  so a code chosen in there from a variable set in there reads the old
rem  value. `if defined` is evaluated at execution and is safe; these three
rem  lines are outside the blocks anyway, which is the form that cannot be
rem  got wrong later.
set "NOH4="
set "NOD1="
if not exist "%ROOT%\data\bars\XAUUSD-4h.parquet" (
  >>"%LOG%" echo ==== NO XAUUSD-4h.parquet AFTER THIS RUN. The export wrote no H4 bars.
  >>"%LOG%" echo ==== Most likely the symbol is not XAUUSD.sc on this terminal, or no terminal
  >>"%LOG%" echo ==== was attachable. mt5_export logs "symbol_select failed - skipped" and
  >>"%LOG%" echo ==== now exits 1 for this, but an older build of it exits 0, so read the
  >>"%LOG%" echo ==== lines above this one rather than trusting the code alone.
  set "NOH4=1"
)
if not exist "%ROOT%\data\bars\XAUUSD-1d.parquet" (
  >>"%LOG%" echo ==== NO XAUUSD-1d.parquet AFTER THIS RUN. Same causes as above.
  set "NOD1=1"
)
if defined NOH4 if defined NOD1 set "RC=12"
if defined NOH4 if not defined NOD1 set "RC=10"
if not defined NOH4 if defined NOD1 set "RC=11"
>>"%LOG%" echo ==== htf export end %DATE% %TIME% rc=%RC%
exit /b %RC%
