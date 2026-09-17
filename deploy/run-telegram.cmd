@echo off
rem  The scheduled task's action for the telegram watch. Same shape and the
rem  same reasons as run-fd-api.cmd beside it - read that one for why this is
rem  a .cmd, why it is not PowerShell, and why the boundary line matters.
rem
rem  The watch task had the same defect: its action was the bare interpreter
rem  with no redirection, so a watcher that failed to start, or started and
rem  complained, did so into nothing.
rem
rem  The token is NOT on this command line and must never be. It lives in
rem  config\local.toml, which is gitignored, and telegram_notify.py reads it
rem  from there - a command line is visible in the process list to anyone who
rem  can run `tasklist`.
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
set "LOG=%LOGDIR%\telegram.out"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

>>"%LOG%" echo(
>>"%LOG%" echo ==== telegram watch start %DATE% %TIME% ====

cd /d "%ROOT%"
"C:\Python39\python.exe" "%ROOT%\py\live\telegram_notify.py" --poll=30 >>"%LOG%" 2>&1
