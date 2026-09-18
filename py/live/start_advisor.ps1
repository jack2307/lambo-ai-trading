# Start (or restart) the advisor panel. READ docs/paper/ADVISOR.md first.
#
#   powershell -NoProfile -File py\live\start_advisor.ps1 -PlanOnly
#   powershell -NoProfile -File py\live\start_advisor.ps1
#   powershell -NoProfile -File py\live\start_advisor.ps1 -Apply     # see below
#
# One process. It polls /api/paper/pending for runs whose next bar would open a
# trade, asks three agents, and takes the MINIMUM of their size factors. The
# only thing it can return is a number in [0, 1]: refuse at 0, shrink between,
# full size at 1. It cannot choose a direction, move a stop or a target, set a
# price, enlarge a trade, or open one the strategy did not ask for - not by
# prompt instruction but because there is no field in the request in which any
# of those could be expressed, and `crates/fd-backtest/src/paper.rs` has a test
# for each refusal. That asymmetry is why this is safe to run at all: the worst
# a broken or hostile panel can do is stop the desk trading.
#
# WHY THIS FILE EXISTS
#
# It did not, and that was the whole fault. `advisor.py` last ran on the
# DESKTOP on 2026-09-17; on the VPS no process ran it, no log existed, and no
# launcher started it - `start_ai_traders.ps1` mentions it only in a comment.
# The owner saw stale ADVISOR PANEL entries in the AI log and asked why the
# agent was not working. Nothing was lost: the declared default when the
# advisor is absent is that the intent fills exactly as the strategy decided,
# and that default is by construction rather than by a timeout, so the desk
# behaved correctly the whole time. But "correct because the feature was
# missing" is only distinguishable from "correct because the feature agreed"
# by reading the logs, and there were none to read.
#
# WHAT IS NOT VERIFIED, SAID HERE RATHER THAN DISCOVERED LATER
#
# The CLI route under a launcher has been proven on the VPS for the TRADERS,
# not for this process. `start_ai_traders.ps1` starts `ai_trader.py`, which
# reaches the same two CLIs through the same resolver, and that works there.
# This file re-reads PATH the same way and checks the same binaries the same
# way, so the plumbing is the same plumbing - but no advisor process has ever
# been started by a launcher on that machine, and `check_clis.py` deliberately
# does not prove a CLI is LOGGED IN. So a clean start here means the panel
# began, not that it will get answers. The first consultation in
# data\paper\logs\advisor.out is what proves the second, and it is worth
# watching for once rather than assuming.
#
# Known to be different from the traders on that machine: `gpt-5.6-sol` is
# retired and refused there ("not supported when using Codex with a ChatGPT
# account"), which is why the news agent's default below is `gpt-5.6-terra`.
# The panel's last desktop run was risk:claude-opus-5, news:codex/gpt-5.6-sol,
# arbiter:claude-opus-5 - so that agent would have had no model on the VPS.
param(
    # DROP --dry-run. Read the red block this prints before using it.
    [switch]$Apply,

    # Print the plan and exit, having started, stopped and checked nothing.
    # This is the only mode that is safe to run casually, and it returns
    # before the kill.
    [switch]$PlanOnly,

    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),

    [string]$Api = 'http://127.0.0.1:8138',

    # Seconds between polls. 20 and not the 30 the desktop ran, because a paper
    # book decides on the close of bar N and fills at the open of bar N+1: on a
    # 5m book the whole window the advisor lives in is five minutes, and a 30s
    # poll can spend a sixth of it not having looked yet.
    [double]$Poll = 20,

    # AGENT=MODEL, repeatable, passed through to advisor.py unchanged.
    #
    # advisor.py has ONE default model for ALL THREE agents - `DEFAULT_MODEL =
    # "claude-opus-5"` - and no per-role default; checked in the source rather
    # than inferred from how it was last invoked. `claude-opus-5` resolves to
    # the `claude-cli` provider, which is the account's PLAN through the Claude
    # Code CLI and carries no API key, because `claude-cli` is listed before
    # `anthropic` in PROVIDERS and claims the `claude` prefix.
    #
    # So this default moves ONE agent and leaves risk and arbiter on the plan:
    # news goes to `codex/gpt-5.6-terra`, which is the ChatGPT plan through the
    # Codex CLI (the `codex/` prefix is what routes it there; a bare `gpt-*`
    # would mean the metered API). Its predecessor `gpt-5.6-sol` is retired and
    # refused on the VPS.
    #
    # Naming a model here that needs a key rather than a plan is allowed and
    # will use one. That is a metered bill, not a plan quota.
    [string[]]$AgentModel = @('news=codex/gpt-5.6-terra'),

    # No model calls at all; the arithmetic control. Useful for proving the
    # poll, the routes and the log path without spending a single call.
    [switch]$RulesOnly
)

function Note($m) { Write-Host "   $m" -ForegroundColor DarkGray }

# `powershell -File script.ps1 -AgentModel a=b,c=d` hands the parameter over as
# the single string "a=b,c=d"; only a dot-sourced call binds it as an array.
# Same trap and same fix as -Only in start_ai_traders.ps1 and -Runs in
# start_executors.ps1, both of which learned it the same way.
$AgentModel = @($AgentModel | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

# ------------------------------------------------------------------ the plan
#
# Printed BEFORE anything is checked, killed or started, because a launcher
# that reports what it did is read after the fact and a launcher that says what
# it is about to do can be stopped.

# The panel as advisor.py will resolve it: every agent on DEFAULT_MODEL unless
# an -AgentModel overrides it. Derived here only to PRINT it - advisor.py is
# still the one that decides, and this list is reconstructed from its source
# rather than duplicated as a second source of truth.
$defaultModel = 'claude-opus-5'
$panel = [ordered]@{ risk = $defaultModel; news = $defaultModel; arbiter = $defaultModel }
foreach ($pair in $AgentModel) {
    $bits = $pair -split '=', 2
    if ($bits.Count -ne 2) {
        Write-Host "-AgentModel wants AGENT=MODEL, got '$pair'" -ForegroundColor Red
        exit 1
    }
    if (-not $panel.Contains($bits[0])) {
        # Refused here as well as in advisor.py. A typo would otherwise be
        # found only after the kill, which is the one place a failure costs
        # something: the running panel would already be gone.
        Write-Host ("no agent '" + $bits[0] + "'; the panel is " + ($panel.Keys -join ', ')) -ForegroundColor Red
        exit 1
    }
    $panel[$bits[0]] = $bits[1]
}

$mode = if ($Apply) { 'APPLY' } else { 'DRY RUN' }
Write-Host ''
Write-Host "advisor panel plan [$mode]"
foreach ($k in $panel.Keys) { Note ("{0,-8} {1}" -f $k, $panel[$k]) }
Note ("poll     every ${Poll}s against $Api")
if ($RulesOnly) { Note 'models   NOT CALLED (-RulesOnly: the arithmetic control)' }
$logs = Join-Path $Root 'data\paper\logs'
Note ("log      " + (Join-Path $logs 'advisor.out'))

if ($Apply) {
    # Red, and specific about what it reaches. The generic warning is the one
    # nobody reads.
    Write-Host ''
    Write-Host '  -Apply: VERDICTS WILL BE POSTED AND WILL CHANGE REAL ENTRIES.' -ForegroundColor Red
    Write-Host '  A cut shrinks the lots of a live entry; a 0 refuses it outright.' -ForegroundColor Red
    Write-Host '  This is not confined to paper. Since 2026-09-17 mt5_executor.py can' -ForegroundColor Red
    Write-Host '  mirror a paper book to the FUNDED cent account (three-key lock:' -ForegroundColor Red
    Write-Host '  --allow-real, --account, and real_money in config/accounts.toml), so' -ForegroundColor Red
    Write-Host '  a verdict that shrinks a mirrored book shrinks a real order. The' -ForegroundColor Red
    Write-Host '  closing section of docs/paper/ADVISOR.md still said "paper only" long' -ForegroundColor Red
    Write-Host '  after that stopped being true; it is corrected in the same commit as' -ForegroundColor Red
    Write-Host '  this file.' -ForegroundColor Red
    Write-Host ''
    Write-Host '  Whether the panel ever runs applied is the OWNER''S decision and a new' -ForegroundColor Red
    Write-Host '  campaign generation, not a flag flip: every advised book stops being' -ForegroundColor Red
    Write-Host '  comparable to the record it has been building. As of this file no' -ForegroundColor Red
    Write-Host '  verdict has ever been applied - 86 consultations on disk, 85 marked' -ForegroundColor Red
    Write-Host '  dry_run and one a deliberate probe, and applied:false on every one.' -ForegroundColor Red
    Write-Host ''
} else {
    Note 'dry run: decides, prints and logs locally; posts nothing, so no book changes'
}

if ($PlanOnly) {
    Note 'plan only: nothing checked, nothing stopped, nothing started'
    exit 0
}

# Two panels at once is not a nuisance, it is a corrupted record: both would
# answer the same intent, both would post, and `advice.jsonl` would carry two
# verdicts for one bar with no way to tell which the book used. Kill-then-start
# does not protect against it - both copies kill nothing and then both start.
# So: one at a time, system-wide.
$mutex = New-Object System.Threading.Mutex($false, 'Global\flowdesk-advisor')
if (-not $mutex.WaitOne(0)) {
    Write-Host 'another start_advisor.ps1 is running; refusing to start a second panel' -ForegroundColor Red
    exit 1
}

try {

# --------------------------------------------------- before anything is killed
#
# PATH from the registry, for the reason start_ai_traders.ps1 does it:
# Start-Process hands the child THIS process's environment, so a shell opened
# before Node was installed launches a panel that cannot see `node` - and
# `codex.CMD` is a batch file whose first act is to run `node`. Measured
# 2026-09-17 on the server: `codex --version` exited 1 with '"node"' is not
# recognized, from a session whose PATH predated the install, and succeeded a
# line later once PATH was re-read.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')

# Then check the models are reachable HERE, ABOVE THE KILL. A panel that cannot
# reach its models should leave the running one alone rather than replace it
# with nothing.
#
# It matters more here than it does for a trader, and in the opposite
# direction. A trader that cannot reach its model stops deciding and the
# campaign visibly stalls. An advisor that cannot reach its models keeps
# running and every agent returns "agent unavailable" at factor 1.0 - which is
# indistinguishable in the book from a panel that looked carefully and approved
# every trade. The desktop log already contains a run of exactly those lines.
# So the check is the difference between a dead panel and a silently permissive
# one.
#
# check_clis.py and not Get-Command, because the two machines resolve Codex
# differently and a PATH test gets one of them wrong; see its docstring. It
# proves the binary RUNS, not that it is logged in - that limit is its own and
# is stated in its docstring too.
if ($RulesOnly) {
    Note 'skipping the model check: -RulesOnly calls no models'
} else {
    Write-Host 'checking each model can be reached'
    $models = @($panel.Values | Sort-Object -Unique)
    & $Python (@((Join-Path $Root 'py\live\check_clis.py')) + $models) |
        ForEach-Object { Write-Host "   $_" }
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'not starting; nothing was stopped' -ForegroundColor Red
        exit 1
    }
}

Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*advisor.py*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 3

# Refuse to start on top of a survivor, for the reason the mutex exists: two
# panels answering one intent is a record nobody can read back.
$alive = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*advisor.py*' })
if ($alive.Count -gt 0) {
    Write-Host "$($alive.Count) advisor process(es) survived the stop; not starting another" -ForegroundColor Red
    exit 1
}

# ------------------------------------------------------------------- the logs
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$out = Join-Path $logs 'advisor.out'
$err = Join-Path $logs 'advisor.err'

# THE BOUNDARY LINE CANNOT GO IN THE LOG ITSELF, and this was checked rather
# than assumed: `Start-Process -RedirectStandardOutput` TRUNCATES the file it
# is given. Measured 2026-09-18 - a line written to the file immediately before
# the call was gone immediately after, replaced by the child's first output. So
# a boundary written there would be destroyed by the very start it was meant to
# mark, silently, leaving a log that looks like one continuous run.
#
# Two things instead, and neither can be truncated by a start:
#
#   1. The previous log is ROLLED ASIDE, never overwritten and never deleted.
#      `advisor.out` becomes `advisor.out.<UTC stamp>`. The panel's reasoning
#      is the only record of what it thought, and this desk does not delete
#      history - it rolls it.
#   2. An append-only `advisor.launches.log` gets one line per start: when,
#      which mode, the panel, and the name of the file the previous run's
#      output was rolled into. That is the boundary, and it survives because
#      nothing redirects into it.
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$rolled = @()
foreach ($f in @($out, $err)) {
    if ((Test-Path $f) -and ((Get-Item $f).Length -gt 0)) {
        $to = "$f.$stamp"
        # Move-Item refuses to overwrite without -Force, which is what is
        # wanted: a name collision means two starts in one second and the
        # second must not eat the first's log.
        Move-Item -Path $f -Destination $to -ErrorAction Stop
        $rolled += (Split-Path -Leaf $to)
    }
}

$launchLine = ("{0}  mode={1}  poll={2}s  api={3}  panel={4}  rolled={5}" -f `
    (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'),
    $mode, $Poll, $Api,
    (($panel.Keys | ForEach-Object { "$_=$($panel[$_])" }) -join ' '),
    $(if ($rolled.Count) { $rolled -join ',' } else { 'none' }))
# Appended through .NET with an explicit BOM-LESS encoder, not `Add-Content
# -Encoding utf8`. Windows PowerShell 5.1 writes a BOM for `utf8`, and it goes
# in at the head of the FILE - so the first launch record of the desk's life
# would carry three invisible bytes in front of its timestamp, and every grep
# for a line starting with a date would miss exactly that one. This repo has
# already lost an afternoon to a BOM: `Set-Content -Encoding UTF8` wrote one
# into a TOML file, tomli refused it, and a selftest went green because every
# check was refusing for that reason instead of the reason it was testing.
$launchLog = Join-Path $logs 'advisor.launches.log'
[System.IO.File]::AppendAllText($launchLog, $launchLine + [Environment]::NewLine,
                                (New-Object System.Text.UTF8Encoding($false)))
if ($rolled.Count) { Note ('previous log rolled aside: ' + ($rolled -join ', ')) }

# ----------------------------------------------------------------- the start
#
# `$argv`, not `$args`: `$args` is an automatic variable. Assigning to it
# happens to work at script scope and does not inside a function, and this repo
# has already paid for that once (9ee54c8, start_executors.ps1).
$argv = @('py/live/advisor.py', "--api=$Api", "--poll=$Poll")
if (-not $Apply) { $argv += '--dry-run' }
if ($RulesOnly) { $argv += '--rules-only' }
foreach ($pair in $AgentModel) { $argv += @('--agent-model', $pair) }

Start-Process -FilePath $Python -ArgumentList $argv -WorkingDirectory $Root -WindowStyle Hidden `
    -RedirectStandardOutput $out -RedirectStandardError $err

Start-Sleep -Seconds 2
$running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*advisor.py*' })
Write-Host "advisor processes now running: $($running.Count) (expected 1)"
if ($running.Count -ne 1) {
    Write-Host "read $err" -ForegroundColor Red
    exit 1
}
Write-Host "started [$mode]; its first consultation in advisor.out is what proves the models answer"

}
finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
