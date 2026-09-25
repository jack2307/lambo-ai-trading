@echo off
rem  Start the REAL-MONEY executors for vantage-v10, detached from whoever
rem  asked. Same shape and the same reasons as run-traders.cmd beside it.
rem
rem  WHY THIS FILE EXISTS. Until 2026-09-25 the only way to start the live
rem  executors was to sit in RDP and run the launcher by hand, because the
rem  desk's notes said MT5 cannot be reached from an SSH session. That is
rem  true of STARTING a terminal and not of TALKING to one that is already
rem  running: measured the same day, `mt5.initialize()` from an SSH session
rem  returned True against the terminal in session 2 and read the account.
rem
rem  WHAT IT DOES NOT SOLVE ON ITS OWN, and why the task matters. Executors
rem  started directly over SSH DIE WHEN THE SSH SESSION CLOSES - Windows
rem  OpenSSH kills the process tree. Measured 2026-09-25: five executors
rem  launched, the session closed, `executors 0` a minute later, and one of
rem  them had already opened a real position (0.35 lots, ticket 597097500)
rem  which was then left with nobody mirroring the book's exit. The task IS
rem  the detachment; do not add a -Detached switch on top of it.
rem
rem  IT RUNS AS THE OWNER, NOT AS SYSTEM, and that is deliberate. The MT5
rem  terminal lives in the owner's interactive session and the Python bridge
rem  is reached through it. run-traders.cmd runs as SYSTEM because its job
rem  needs a metered API key SYSTEM can read; this one needs a terminal
rem  SYSTEM has no path to.
rem
rem  THE LAUNCHER STOPS EVERY EXECUTOR BEFORE IT STARTS ITS SELECTION. That
rem  is safe while the account is flat and is not safe while it is holding
rem  something this run would not restart - start_executors.ps1 refuses that
rem  case by itself and says what refusing costs. Do not pass -AllowOrphans
rem  from here.
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

rem  A TIMESTAMPED LOG, for the reason run-traders.cmd gives at length: a
rem  fixed name deadlocks, because the children inherit the handle and hold
rem  it for as long as they live.
for /f "tokens=1-6 delims=/:. " %%a in ("%DATE% %TIME%") do set "STAMP=%%c%%b%%a-%%d%%e%%f"
set "STAMP=%STAMP: =0%"
set "LOG=%LOGDIR%\exec-v10-%STAMP%.out"

>>"%LOG%" echo ==== start executors vantage-v10 %DATE% %TIME%

cd /d "%ROOT%"

powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\py\live\start_executors.ps1" -Live -AllowReal -Account vantage-v10 >>"%LOG%" 2>&1
set "RC=%ERRORLEVEL%"
>>"%LOG%" echo ==== end %DATE% %TIME% rc=%RC%
exit /b %RC%
