# Start (or restart) the LIVE side: one executor per chosen book.
#
#   powershell -NoProfile -File py\live\start_executors.ps1 -Login 26108386
#   powershell -NoProfile -File py\live\start_executors.ps1 -Login 26108386 -DryRun
#   powershell -NoProfile -File py\live\start_executors.ps1 -Login 26108386 -Runs xau-ema,xau-macd-asia
#
# Paper and live are separate by construction and this script is the only thing
# that joins them. The paper desk decides; an executor MIRRORS one paper book
# into one MT5 account and does nothing else - no strategy logic lives here, so
# a live book can never disagree with the paper book it came from about what
# the rule said. What it can disagree about is the FILL, and that difference is
# the whole point of running it.
#
# Choosing what goes live is `$Runs` and nothing else. A book not named here is
# paper only.
param(
    # The demo account the terminal must be logged into. Required, and checked
    # by the executor against `account_info().login` before any order: a
    # terminal that is not the one you named is not the one you meant.
    [Parameter(Mandatory = $true)][int]$Login,

    # Which paper books to mirror. Order does not matter; each gets its own
    # process and its own magic number.
    [string[]]$Runs = @('xau-ema'),

    # Size relative to the paper book. 1.0 sends the book's own lots.
    [double]$LotScale = 1.0,

    [switch]$DryRun,
    [string]$Terminal = 'C:\MT5-demo\terminal64.exe',
    [string]$Symbol = 'XAUUSD.sc',
    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = ''
)

# Resolved in the body, not in the param default: $PSScriptRoot is not reliably
# populated while defaults are being evaluated, and an empty one made
# Split-Path fail before the script had run a line.
if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent (Split-Path -Parent $here)
}
if (-not $Root) { $Root = (Get-Location).Path }

if (-not (Test-Path $Terminal)) {
    Write-Error "no terminal at $Terminal"
    exit 1
}

# Two copies of an executor on one book is two copies of every order. The
# reconciler is idempotent against the BOOK, not against another copy of
# itself: both would see a flat terminal, both would open, and the account
# would carry double the size the book asked for.
$mutex = New-Object System.Threading.Mutex($false, 'Global\flowdesk-executors')
if (-not $mutex.WaitOne(0)) {
    Write-Error 'another start_executors.ps1 is running; refusing to start a second set'
    exit 1
}

try {
    Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 3

    $alive = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
    if ($alive.Count -gt 0) {
        Write-Error "$($alive.Count) executor(s) survived the stop; not starting more"
        exit 1
    }

    $logs = Join-Path $Root 'data\paper\logs'
    New-Item -ItemType Directory -Force -Path $logs | Out-Null

    foreach ($run in $Runs) {
        # The kill switch the executor itself watches. Clearing it here means a
        # book stopped by hand stays stopped until someone starts it on purpose.
        $stop = Join-Path $Root "data\paper\$run\STOP"
        if (Test-Path $stop) {
            Write-Host "skipping $run - a STOP file is present; delete it to run this book live"
            continue
        }

        $args = @('py/live/mt5_executor.py', "--run=$run", "--terminal=$Terminal",
                  "--login=$Login", "--symbol=$Symbol", "--lot-scale=$LotScale")
        if ($DryRun) { $args += '--dry-run' }
        Start-Process -FilePath $Python -ArgumentList $args -WorkingDirectory $Root -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $logs "exec_$run.out") `
            -RedirectStandardError (Join-Path $logs "exec_$run.err")
        $mode = if ($DryRun) { 'DRY RUN' } else { 'LIVE' }
        Write-Host "mirroring $run -> account $Login on $Symbol at x$LotScale [$mode]"
    }

    Start-Sleep -Seconds 3
    $running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
    Write-Host "executors now running: $($running.Count) (asked for $($Runs.Count))"
    if ($running.Count -lt $Runs.Count) {
        Write-Host "one or more exited immediately - read data\paper\logs\exec_*.out; the usual cause is"
        Write-Host "the terminal not being logged into account $Login, which the executor refuses by design."
    }
}
finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
