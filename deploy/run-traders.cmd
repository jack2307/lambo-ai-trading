@echo off
rem  The scheduled task's action for the AI traders. Same shape and the same
rem  reasons as run-fd-api.cmd beside it - read that one for why this is a
rem  .cmd and not PowerShell.
rem
rem  WHAT THIS IS FOR. Before 2026-09-21 nothing brought the traders back
rem  after a reboot. flowdesk-api and flowdesk-watch had boot triggers; the
rem  traders, the pollers and the executors did not, so a restart left the
rem  desk answering HTTP and deciding nothing, with no alarm anywhere. The
rem  owner asked for "boot mot nua" - half a boot: the paper side comes back
rem  by itself, the money side does not.
rem
rem  DEEPSEEK ONLY, AND THAT IS A PERMISSION BOUNDARY AND NOT A PREFERENCE.
rem  This task runs as SYSTEM. `deepseek-flash` is metered and its key is in
rem  config\local.toml, which SYSTEM can read. `claude-opus-5` and
rem  `codex/gpt-5.6-terra` are keyless - they reach their models through the
rem  owner's own plans, authenticated inside the Administrator profile, which
rem  SYSTEM has no path to. start_ai_traders.ps1 checks every model can be
rem  reached BEFORE it kills anything, so a task running all nine would fail
rem  that check and start nothing. -Model names the model rather than seven
rem  run ids so a DeepSeek book added later is picked up and Opus and Terra
rem  never are.
rem
rem  Those two come back when the owner does. They need his session anyway.
rem
rem  NO -Detached. The task IS the detachment; a second one would spawn a
rem  child the task cannot see and report success before it had started.
rem  start_ai_traders.ps1 says this at the switch itself.
rem
rem  IT REFUSES IF TRADERS ARE ALREADY RUNNING, and that guard is the whole
rem  reason this file is safe to have. start_ai_traders.ps1 STOPS EVERY
rem  trader before starting its selection, so this action run by hand in the
rem  middle of a session would kill Opus and Terra and not bring them back.
rem  At boot there is nothing running and the guard passes. Anywhere else it
rem  declines and says so. Exit 0, not an error: "the desk is already up" is
rem  the task succeeding at its job, and a red Last Run Result would train
rem  somebody to ignore it.
setlocal
set "ROOT=%~dp0.."
set "LOGDIR=%ROOT%\data\paper\logs"
if not exist "%LOGDIR%" mkdir "%LOGDIR%"

rem  A TIMESTAMPED LOG, because a fixed one deadlocks. Start-Process hands a
rem  child the parent's handles, so the traders hold whatever file this
rem  redirects into for as long as they live, and the next run cannot open it
rem  - the redirect fails before a line executes and the caller is told it
rem  started. That cost two days of silently-dead restarts on 2026-09-21; see
rem  CLAUDE.md and start_ai_traders.ps1's own note.
for /f "tokens=1-6 delims=/:. " %%a in ("%DATE% %TIME%") do set "STAMP=%%c%%b%%a-%%d%%e%%f"
set "STAMP=%STAMP: =0%"
set "LOG=%LOGDIR%\boot-traders-%STAMP%.out"

>>"%LOG%" echo ==== boot traders start %DATE% %TIME%

cd /d "%ROOT%"

rem  THE GUARD IS IN THE POWERSHELL, NOT HERE, AND THAT IS THE SECOND
rem  ATTEMPT. The first counted the traders with a `for /f` wrapped around a
rem  PowerShell one-liner, and cmd ate the quotes inside it: the count came
rem  back empty, this file read it as zero while nine traders were running,
rem  went ahead, and killed the two campaigns the -Model filter cannot
rem  restart. Measured 2026-09-21 21:23; restored a minute later, and no bar
rem  closed in the gap. -OnlyIfNoneRunning does the same check where
rem  counting a process needs no escaping, and where the script that does
rem  the killing is the one that decides not to.
powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\py\live\start_ai_traders.ps1" -Model deepseek-flash -OnlyIfNoneRunning >>"%LOG%" 2>&1
set "RC=%ERRORLEVEL%"
>>"%LOG%" echo ==== boot traders end %DATE% %TIME% rc=%RC%
exit /b %RC%
