# Pull, rebuild, restart. RUN THIS ON THE SERVER, every time there is a change.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1 -NoRestart
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1 -AllowOrphans
#
# It REFUSES while a real-money account is holding a position, because stopping
# the executors leaves that position with nobody mirroring the book's exit.
# -AllowOrphans is the override, and what refusing costs is printed where it
# happens rather than left for the reader to weigh.
#
# The desk is stopped before anything is rebuilt and started again after, in an
# order that matters: the executors go down FIRST and come up LAST. An executor
# talking to an fd-api that is mid-restart reads no book at all, and its rule
# for "the book is flat and the account is not" is to close the position - so a
# restart with the mirrors still running would flatten every live trade on the
# way past.
param(
    [string]$Root = '',
    [switch]$NoRestart,
    [switch]$SkipBuild,

    # Deploy anyway while a real-money account is holding a position.
    #
    # Stopping the executors leaves any open position with nobody reconciling
    # it: the book will decide to exit and nothing will carry that decision to
    # the broker, so the trade runs to its stop or its target instead. That is
    # not the trade the book took, and measuring exactly that difference is the
    # only reason the mirror exists.
    #
    # So a deploy taken over an open real position refuses, and this is the
    # word that overrides it. What refusing COSTS is written out at the point
    # where it happens, below; it is not free and it is not small.
    [switch]$AllowOrphans
)

$ErrorActionPreference = 'Stop'

# TLS 1.2, explicitly.
#
# Windows Server 2012 R2 and 2016 default .NET to TLS 1.0, which GitHub and
# python.org have both refused for years. Without this line every download
# below fails with "the underlying connection was closed" - a message that says
# nothing about protocols and sends people looking at firewalls.
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}
Set-Location $Root

function Step($what) { Write-Host "`n== $what" -ForegroundColor Cyan }
function Note($what) { Write-Host "   $what" -ForegroundColor DarkGray }

function Stop-Desk {
    # Executors first. See the note at the top of this file.
    #
    # `telegram_notify` is NOT in this list, and its absence is the point. It
    # is the only thing watching while everything else is down, and stopping it
    # here put the alarm out for the whole pull-stop-build window - minutes, in
    # which the executors are already dead and any open position has nobody
    # reconciling it. That is precisely the window someone needs to be told
    # about.
    #
    # It is pure Python with no build artefact, so there is nothing to rebuild
    # it for, and `start_telegram.ps1` at the end of this script stops whatever
    # is running before starting a fresh copy - so a watcher left alive here is
    # still replaced by the new code, just later.
    #
    # What it costs: two messages per deploy. The watcher will see fd-api go
    # away and say "the desk stopped answering", then "the desk is answering
    # again". Both are edge-triggered (`api_down` in telegram_notify.py), so it
    # is two lines and not a storm, and they are a true account of what
    # happened.
    foreach ($m in '*mt5_executor.py*', '*ai_trader.py*', '*advisor.py*', '*mt5_bars.py*', '*spread_log*') {
        Get-CimInstance Win32_Process -Filter "name='python.exe'" |
            Where-Object { $_.CommandLine -like $m } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    Get-Process fd-api -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 3
}

# What the accounts are holding RIGHT NOW, read from the mirrors themselves.
#
# Deliberately duplicated from py\live\start_executors.ps1 rather than shared.
# A third file that both of them dot-source is one more thing to be missing on
# the server at the moment someone is trying to stop a trade, and this is
# twenty lines that read the same file the same way. If it changes, change it
# in both - the comment there says the same.
function Get-Holdings([string]$root) {
    $live = Join-Path $root 'data\live'
    if (-not (Test-Path $live)) { return @() }
    $nowMs = [int64]((Get-Date).ToUniversalTime() - [datetime]'1970-01-01').TotalMilliseconds
    $out = @()
    foreach ($acctDir in @(Get-ChildItem $live -Directory -ErrorAction SilentlyContinue)) {
        foreach ($runDir in @(Get-ChildItem $acctDir.FullName -Directory -ErrorAction SilentlyContinue)) {
            $p = Join-Path $runDir.FullName 'broker.json'
            if (-not (Test-Path $p)) { continue }
            try { $snap = Get-Content $p -Raw -ErrorAction Stop | ConvertFrom-Json } catch { continue }
            if (-not $snap -or -not $snap.position) { continue }
            $at = 0
            if ($snap.at) { $at = [int64]$snap.at }
            $out += [pscustomobject]@{
                Account = $acctDir.Name
                Run     = $runDir.Name
                Real    = ($snap.demo -eq $false)
                Side    = $snap.position.side
                Lots    = $snap.position.lots
                Ticket  = $snap.position.ticket
                AgeMs   = $nowMs - $at
                Stale   = (($nowMs - $at) -gt 60000)
            }
        }
    }
    return $out
}

# ------------------------------------------------------------- open positions
#
# Before the pull, before anything. A deploy that is going to be refused should
# leave the server exactly as it found it - including its commit, so that what
# is running and what is checked out still match while someone decides what to
# do.
#
# Skipped under -NoRestart, which stops nothing and therefore orphans nothing.
#
# ON STALENESS, because it decides what this does rather than merely how it
# reads. A FRESH snapshot means the executor is alive and the position is open
# now; killing it is an orphan this script would be creating, so that refuses.
# A STALE one means the mirror already stopped, at some point that may have
# been months ago - broker.json is never deleted and `data/` is never cleaned -
# so the position it describes may have closed long since. Refusing on that
# would make the desk permanently un-deployable on the strength of a file
# nobody tidies. So stale holdings are NAMED and do not refuse.
#
# Absolute age and not a window relative to the other mirrors, unlike the API's
# `mirroring` count: the question here is "is this one alive", and at deploy
# time the whole set may legitimately be down already, which is exactly when a
# relative measure says nothing. Same reasoning as MIRROR_STALE_MS in
# telegram_notify.py, and the same 60 seconds.
if (-not $NoRestart) {
    $holding = @(Get-Holdings $Root)
    if ($holding.Count -gt 0) {
        Step 'open positions'
        foreach ($h in $holding) {
            $what = "$($h.Account)/$($h.Run): $($h.Side) $($h.Lots) lots, ticket $($h.Ticket)"
            if ($h.Real) { $what += ' [REAL MONEY]' }
            if ($h.Stale) { $what += "  (snapshot $([int]($h.AgeMs / 1000))s old - this mirror ALREADY stopped)" }
            Write-Host "   $what" -ForegroundColor $(if ($h.Real) { 'Red' } else { 'Gray' })
        }
        $atRisk = @($holding | Where-Object { $_.Real -and -not $_.Stale })
        if ($atRisk.Count -gt 0 -and -not $AllowOrphans) {
            Write-Host ''
            Write-Host 'This deploy stops the executors and does not start them again, so those' -ForegroundColor Yellow
            Write-Host 'position(s) would be left with nobody mirroring the book''s exit. Refusing.' -ForegroundColor Yellow
            Write-Host ''
            Write-Host 'WHAT THIS COSTS, so it is a decision and not a wall: the desk is now' -ForegroundColor DarkGray
            Write-Host 'un-deployable until the account goes flat, and on a 15m book with a four-' -ForegroundColor DarkGray
            Write-Host 'hour maximum hold that can be most of a session. A fix that has to wait' -ForegroundColor DarkGray
            Write-Host 'for a trade to close is a fix that is not deployed, and if the thing being' -ForegroundColor DarkGray
            Write-Host 'deployed is what makes the desk safe, waiting is the more dangerous half.' -ForegroundColor DarkGray
            Write-Host 'Three ways past it, in the order worth trying:' -ForegroundColor DarkGray
            Write-Host '  1. Wait for flat. Right when nothing is urgent.' -ForegroundColor DarkGray
            Write-Host '  2. Close on purpose: drop data\live\<account>\<book>\STOP, let the' -ForegroundColor DarkGray
            Write-Host '     executor close and exit, then deploy. The exit is then a decision' -ForegroundColor DarkGray
            Write-Host '     in the record rather than a trade nobody managed.' -ForegroundColor DarkGray
            Write-Host '  3. -AllowOrphans, and go and watch the account yourself.' -ForegroundColor DarkGray
            exit 1
        }
    }
}

# --------------------------------------------------------------------- pull
Step 'git pull'
$before = git rev-parse HEAD
git pull --ff-only
if ($LASTEXITCODE -ne 0) {
    # Never merge or rebase unattended on a machine that trades. A conflict
    # here means the server has local changes, and resolving those blind is how
    # a desk ends up running something nobody wrote.
    Write-Error 'git pull failed. If the server has local edits, deal with them by hand.'
    exit 1
}
$after = git rev-parse HEAD
if ($before -eq $after) {
    Note 'already up to date'
} else {
    Note "$($before.Substring(0,7)) -> $($after.Substring(0,7))"
    git --no-pager log --oneline "$before..$after" | ForEach-Object { Note "  $_" }
}

if ($NoRestart -and $SkipBuild) { Write-Host "`nnothing else asked for"; exit 0 }

# --------------------------------------------------------------------- stop
if (-not $NoRestart) {
    Step 'stopping the desk'
    Stop-Desk
    Note 'executors, traders, pollers and fd-api down'
}

# -------------------------------------------------------------------- build
if (-not $SkipBuild) {
    Step 'building'
    $cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
    if (-not $cargo) { $cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe" }
    if (-not (Test-Path $cargo)) { Write-Error 'cargo not found; run bootstrap.ps1 -WithToolchain'; exit 1 }
    & $cargo build --release -p fd-api
    if ($LASTEXITCODE -ne 0) { Write-Error 'cargo build failed'; exit 1 }
    Note 'fd-api built'

    Push-Location ui
    # `npm ci` and not `npm install`: it installs exactly the lockfile, so the
    # server cannot quietly end up on different dependency versions from the
    # machine the change was tested on.
    npm ci --silent
    $ok = $LASTEXITCODE -eq 0
    if ($ok) { npm run build; $ok = $LASTEXITCODE -eq 0 }
    Pop-Location
    if (-not $ok) { Write-Error 'ui build failed'; exit 1 }
    Note 'client built'
}

# -------------------------------------------------------------------- start
if (-not $NoRestart) {
    Step 'starting the desk'
    $logs = Join-Path $Root 'data\paper\logs'
    New-Item -ItemType Directory -Force -Path $logs | Out-Null

    Start-Process -FilePath (Join-Path $Root 'target\release\fd-api.exe') -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs 'fd-api.out') -RedirectStandardError (Join-Path $logs 'fd-api.err')
    Note 'fd-api starting; waiting for it to answer'

    # Waited for rather than slept past. Everything below posts to it, and a
    # poller that starts against a socket nobody is listening on logs an error
    # and carries on as though the desk were simply quiet.
    $ready = $false
    foreach ($i in 1..30) {
        Start-Sleep -Seconds 1
        try {
            Invoke-WebRequest -Uri 'http://127.0.0.1:8138/api/paper/status' -UseBasicParsing -TimeoutSec 2 | Out-Null
            $ready = $true
            break
        } catch { }
    }
    if (-not $ready) { Write-Error 'fd-api did not answer within 30s; read data\paper\logs\fd-api.err'; exit 1 }
    Note 'fd-api answering'

    foreach ($s in 'start_pollers.ps1', 'start_ai_traders.ps1', 'start_telegram.ps1') {
        $p = Join-Path $Root "py\live\$s"
        if (Test-Path $p) {
            Note "running $s"
            powershell -NoProfile -ExecutionPolicy Bypass -File $p | Out-Null
        }
    }

    Write-Host "`nThe mirrors are NOT started." -ForegroundColor Yellow
    Write-Host 'Start them yourself, once you have looked at the desk:' -ForegroundColor Yellow
    Write-Host '  powershell -NoProfile -File py\live\start_executors.ps1 -DryRun' -ForegroundColor Yellow
    Write-Host '  powershell -NoProfile -File py\live\start_executors.ps1 -Live' -ForegroundColor Yellow
    Write-Host 'Sending real orders after an unattended rebuild should be a decision,' -ForegroundColor DarkGray
    Write-Host 'not something a script did while nobody was reading the output.' -ForegroundColor DarkGray

    # Said LAST because it is the thing to act on, and last is what stays on
    # the screen. The check at the top of this script already refused unless
    # -AllowOrphans was given, so reaching here holding something means someone
    # chose it - and the one way that choice goes wrong is being forgotten
    # while the build scrolled past.
    $stillOpen = @($holding | Where-Object { $_.Real -and -not $_.Stale })
    if ($stillOpen.Count -gt 0) {
        Write-Host ''
        Write-Host "$($stillOpen.Count) REAL position(s) have had no mirror since this deploy began:" -ForegroundColor Red
        foreach ($h in $stillOpen) {
            Write-Host "  $($h.Account)/$($h.Run): $($h.Side) $($h.Lots) lots, ticket $($h.Ticket)" -ForegroundColor Red
        }
        Write-Host 'Until an executor is running for each of those, the book''s exit reaches' -ForegroundColor Red
        Write-Host 'nobody and the trade ends at the broker''s stop or target instead.' -ForegroundColor Red
    }
}
