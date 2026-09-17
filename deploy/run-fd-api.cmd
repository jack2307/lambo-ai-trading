@echo off
rem  The scheduled task's action for fd-api, and the only reason it exists is
rem  that the task had NO redirection and fd-api's stdout went nowhere.
rem
rem  Found 2026-09-17, after a deploy whose checklist said "read the startup
rem  line": the version summary and the advisor-gate line were both
rem  unreadable by construction. `data\paper\logs\fd-api.out` held 990 bytes
rem  written by an earlier wrapper-started process, and two people read it
rem  that evening as the live process's output. A file that looks like a log
rem  and describes a dead process is worse than no file at all.
rem
rem  WHY A .CMD AND NOT REDIRECTION IN THE TASK'S ARGUMENT FIELD. The argument
rem  would be one string carrying nested quotes around two paths, which is the
rem  classic way to get a task that registers cleanly and runs nothing. Here
rem  the quoting is in a file, in the repository, where it can be read and
rem  fixed.
rem
rem  WHY CMD AND NOT POWERSHELL. A PowerShell wrapper owns a console, and a
rem  console under a SYSTEM task with no desktop is how fd-api died the first
rem  time this desk was set up. cmd /c under a service account creates no
rem  window and no desktop dependency.
rem
rem  Paths come from this file's own location, so a checkout anywhere works
rem  and C:\flowdesk is not hardcoded.
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
set "LOG=%LOGDIR%\fd-api.out"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

rem  The boundary. Appending without one produces a file in which nobody can
rem  tell where the current process started, which is the half of the problem
rem  that survives adding redirection.
>>"%LOG%" echo(
>>"%LOG%" echo ==== fd-api start %DATE% %TIME% ====

cd /d "%ROOT%"
"%ROOT%\target\release\fd-api.exe" >>"%LOG%" 2>&1
