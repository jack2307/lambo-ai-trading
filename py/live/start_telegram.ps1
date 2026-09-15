# Start (or restart) the Telegram watcher.
#
#   powershell -NoProfile -File py\live\start_telegram.ps1
#   powershell -NoProfile -File py\live\start_telegram.ps1 -Test
#
# One at a time, system-wide. Two copies is not merely noisy here: Telegram's
# `getUpdates` hands each update to whichever poller asks first and then
# forgets it, so a second copy makes commands vanish at random rather than
# duplicate. The mutex is the fix, not a nicety.
#
# The bot reads and nothing else. It never calls a write route, and the token
# lives in config\local.toml which is gitignored and is never passed on a
# command line — a command line is visible in the process list.
param(
    [switch]$Test,
    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

if ($Test) {
    & $Python (Join-Path $Root 'py\live\telegram_notify.py') --test
    exit $LASTEXITCODE
}

$mutex = New-Object System.Threading.Mutex($false, 'Global\flowdesk-telegram')
if (-not $mutex.WaitOne(0)) {
    Write-Error 'another start_telegram.ps1 is running; refusing to start a second watcher'
    exit 1
}

try {
    Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*telegram_notify.py*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 3

    $alive = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*telegram_notify.py*' })
    if ($alive.Count -gt 0) {
        Write-Error "$($alive.Count) telegram watcher(s) survived the stop; not starting another"
        exit 1
    }

    $logs = Join-Path $Root 'data\paper\logs'
    New-Item -ItemType Directory -Force -Path $logs | Out-Null
    Start-Process -FilePath $Python -ArgumentList @('py/live/telegram_notify.py', '--poll=30') `
        -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs 'telegram.out') `
        -RedirectStandardError (Join-Path $logs 'telegram.err')
    Start-Sleep -Seconds 3
    Write-Host 'telegram watcher started (reads the desk; answers one chat only)'
}
finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
