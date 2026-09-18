@echo off
rem  The scheduled task's action for the bar export. Same shape and the same
rem  reasons as run-fd-api.cmd beside it - read that one for why this is a
rem  .cmd, why it is not PowerShell, and why the boundary line matters.
rem
rem  WAS run-htf-export.cmd. Renamed when the export stopped being about the
rem  higher timeframes: the owner asked for every timeframe on the chart, so
rem  this now pulls M1 through D1 and "htf" described only the two it started
rem  with. install-tasks.ps1 -Apply unregisters the old `flowdesk-htf-export`
rem  task and says so in its report.
rem
rem  WHAT THIS EXISTS FOR. `GET /api/paper/htf` reads
rem  data\bars\<SYMBOL>-4h.parquet and -1d.parquet, and the chart reads the
rem  rest. `data\` is gitignored, so no commit can ever ship those files: a
rem  merge that brings the route and the exporter brings no bars, and the
rem  route then answers "not exported yet" correctly while the deploy reports
rem  success. This is the step that keeps them current, and
rem  deploy\update.ps1 refuses to report ready when the route says the bars
rem  are missing for a market the registry mirrors.
rem
rem  EVERY ARGUMENT COMES FROM THE TASK, NOT FROM THIS FILE.
rem
rem    %1  terminal   the price terminal, from [prices] in accounts.toml
rem    %2  symbols    broker symbols, comma-separated, suffix included
rem    %3+ timeframes MT5 names - one comma-separated argument, or
rem                   several separate ones, which are rejoined below
rem
rem  install-tasks.ps1 resolves them through `py\live\accounts.py --prices`
rem  at REGISTRATION time and writes them into the task's action. Two
rem  consequences, and both are deliberate:
rem
rem    `Get-ScheduledTask flowdesk-bars-export | Select -Expand Actions` shows
rem    exactly what will run, so an operator can see the terminal and the
rem    symbols without opening this file or the registry.
rem
rem    Changing [prices] therefore needs `install-tasks.ps1 -Apply` re-run as
rem    well as a poller restart. That is stated beside the key and in the
rem    script's report, because a binding you have to know about is a binding
rem    somebody will not know about.
rem
rem  Nothing is defaulted here. A missing argument is a task that was
rem  registered wrong or a hand-run with no arguments, and guessing a
rem  terminal on a machine with two is how the desk ends up exporting one
rem  account's contract into the other account's file.
rem
rem  READ-ONLY, AND IT MERGES. mt5_export calls initialize, symbol_select,
rem  symbol_info, copy_rates_from_pos and shutdown. No order, none closed, no
rem  AutoTrading - the same access the pollers have had against this terminal
rem  since day one. `--since-stored` resumes from the newest bar already in
rem  each file with one bar of overlap, so a routine run asks for minutes of
rem  data and a missing file still backfills. `--days` is deliberately NOT
rem  passed with it: both bound the pull and the tighter one would win
rem  silently. `--days` stays for hand runs, where a person means "give me
rem  this much".
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
set "LOG=%LOGDIR%\bars-export.out"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

rem  THE BOUNDARY LINE IS WRITTEN BEFORE ANYTHING CAN FAIL, including the
rem  argument handling below. a5 asked for this after a task run that exited
rem  1 in five seconds and left NO log at all.
rem
rem  Being honest about what it does and does not buy: that particular run
rem  never reached this file - cmd mangled the command line and could not
rem  find the script - so no echo here would have left a trace. What it does
rem  cover is every failure from this point on, which is the argument
rem  handling, the python lookup and the exporter itself. A log that starts
rem  is a log that tells you how far it got.
>>"%LOG%" echo(
>>"%LOG%" echo ==== bars export start %DATE% %TIME% ====

set "TERMINAL=%~1"
set "SYMBOLS=%~2"

rem  THE TIMEFRAME LIST, ACCEPTED IN EITHER SHAPE.
rem
rem  cmd splits numbered parameters on COMMAS as well as spaces, so a list
rem  that loses its quotes anywhere on the way here arrives as %3 %4 %5...
rem  instead of one argument. The task's action is now quoted so that it
rem  arrives whole (see Resolve-TaskAction), and this rejoins it anyway:
rem  a hand-run that types the list unquoted then works too, and the two
rem  invocations cannot behave differently.
set "TIMEFRAMES=%~3"
if not "%~4"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~4"
if not "%~5"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~5"
if not "%~6"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~6"
if not "%~7"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~7"
if not "%~8"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~8"
if not "%~9"=="" set "TIMEFRAMES=%TIMEFRAMES%,%~9"

if "%TERMINAL%"=="" goto :noargs
if "%SYMBOLS%"==""  goto :noargs
if "%TIMEFRAMES%"=="" goto :noargs
goto :run

:noargs
>>"%LOG%" echo ==== REFUSING: called with no terminal/symbols/timeframes.
>>"%LOG%" echo ==== This wrapper takes all three from the task's action, which
>>"%LOG%" echo ==== install-tasks.ps1 fills in from [prices] in accounts.toml.
>>"%LOG%" echo ==== Re-run: powershell -File deploy\install-tasks.ps1 -Apply
>>"%LOG%" echo ==== By hand:  run-bars-export.cmd "C:\MT5-cent\terminal64.exe" "XAUUSD.sc" M1 M5 M15 H1 H4 D1
exit /b 20

:run
>>"%LOG%" echo ==== terminal %TERMINAL%
>>"%LOG%" echo ==== symbols %SYMBOLS%  timeframes %TIMEFRAMES%

cd /d "%ROOT%"
"C:\Python39\python.exe" "%ROOT%\py\ingest\mt5_export.py" --symbols "%SYMBOLS%" --timeframes "%TIMEFRAMES%" --since-stored --terminal "%TERMINAL%" >>"%LOG%" 2>&1
set "RC=%ERRORLEVEL%"

rem  EXIT CODES, AND WHOSE THEY ARE.
rem
rem    0    everything asked for was written
rem    1    the exporter wrote nothing
rem    2    the exporter wrote SOME of what was asked for
rem    10   one or more expected parquet files are missing after this run
rem    20   this wrapper was called without its arguments
rem
rem  0, 1 and 2 are mt5_export's; 10 and 20 are this file's, numbered clear
rem  of them so a Last Run Result says which layer objected.
rem
rem  ONE CODE FOR "A FILE IS MISSING" AND NOT ONE PER FILE. An earlier draft
rem  used 10/11/12 for no-H4 / no-D1 / neither, which was already the wrong
rem  shape at two timeframes and does not survive six. WHICH file is a log
rem  question and the log names every one of them; the exit code answers
rem  "does this need a human", which has one bit in it.
rem
rem  PARTIAL IS FATAL, and that is a decision this file is entitled to make
rem  and the exporter is not. d1 left it here on the grounds that the
rem  exporter cannot know what the files are for. The route needs H4 and D1,
rem  the chart needs the rest, and a partial reported as success reads green
rem  forever while one series quietly stops updating.
rem
rem  WHAT THE EXISTENCE CHECK CANNOT SEE: a file that exists and has STOPPED
rem  being updated passes it every time. It protects the first run of a new
rem  timeframe. After that the exit code is the only thing watching, which is
rem  why it had to be fixed in the exporter rather than worked around here.
set "MISSING="
for %%T in (1m 5m 15m 1h 4h 1d) do (
  if not exist "%ROOT%\data\bars\XAUUSD-%%T.parquet" (
    >>"%LOG%" echo ==== MISSING data\bars\XAUUSD-%%T.parquet after this run
    set "MISSING=1"
  )
)
if defined MISSING (
  >>"%LOG%" echo ==== At least one expected file is absent. Most likely the symbol is
  >>"%LOG%" echo ==== not right for this terminal - mt5_export logs "symbol_select failed
  >>"%LOG%" echo ==== - skipped" and now exits 1 for that, but read the lines above.
  set "RC=10"
)
>>"%LOG%" echo ==== bars export end %DATE% %TIME% rc=%RC%
exit /b %RC%
