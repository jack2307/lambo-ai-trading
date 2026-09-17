# Pull, rebuild, restart. RUN THIS ON THE SERVER, every time there is a change.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1 -NoRestart
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
    [switch]$SkipBuild
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
    foreach ($m in '*mt5_executor.py*', '*ai_trader.py*', '*advisor.py*', '*telegram_notify*', '*mt5_bars.py*', '*spread_log*') {
        Get-CimInstance Win32_Process -Filter "name='python.exe'" |
            Where-Object { $_.CommandLine -like $m } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    Get-Process fd-api -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 3
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
}
