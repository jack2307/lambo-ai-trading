# One page saying whether the desk is up, for a person who has just logged in.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\status.ps1
#
# READ-ONLY. It starts nothing, stops nothing and writes nothing. Reach for it
# first, every time, before deciding anything is wrong.
#
# It exists because the answer was spread across five commands nobody could
# remember at speed, and because two failures on 2026-09-21 were invisible
# precisely until somebody compared a process start time against a log
# timestamp. Those two comparisons are printed here by default.
$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $PSScriptRoot
$bad = 0
function Say($s) { Write-Host $s }
function Warn($s) { $script:bad++; Write-Host $s -ForegroundColor Yellow }

Say ''
Say '================ SERVER ================'
try {
    $v = Invoke-RestMethod http://127.0.0.1:8138/api/version -TimeoutSec 15
    $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
    $head = (& git -C $Root rev-parse HEAD 2>$null)
    Say ("  API up, built from " + $v.git_hash.Substring(0,7))
    if ($head) {
        $head = $head.Trim()
        if ($head -ne $v.git_hash) {
            Warn ("  repo is at " + $head.Substring(0,7) + " - the running binary is OLDER than the source")
            Say  '     harmless if nothing in crates/ or ui/ changed; otherwise redeploy'
        }
    }
} catch { Warn ("  API DOWN: " + $_.Exception.Message) }

Say ''
Say '================ WHAT IS RUNNING ================'
$py = @(Get-CimInstance Win32_Process -Filter "name='python.exe'")
$traders   = @($py | Where-Object { $_.CommandLine -like '*ai_trader.py*' })
$executors = @($py | Where-Object { $_.CommandLine -like '*mt5_executor*' })
$pollers   = @($py | Where-Object { $_.CommandLine -like '*mt5_bars*' })
$terms     = @(Get-CimInstance Win32_Process -Filter "name='terminal64.exe'")
Say ("  traders {0}   executors {1}   pollers {2}   MT5 terminals {3}" -f $traders.Count, $executors.Count, $pollers.Count, $terms.Count)
if ($traders.Count -eq 0) { Warn '  NO TRADERS - the paper books are deciding nothing' }
if ($pollers.Count -eq 0) { Warn '  NO POLLERS - no bars are arriving; everything downstream is blind' }

foreach ($x in $executors) {
    $a  = if ($x.CommandLine -match '--account=(\S+)') { $Matches[1] } else { '?' }
    $ls = if ($x.CommandLine -match '--lot-scale=(\S+)') { $Matches[1] } else { '?' }
    Say ("  REAL MONEY executor: {0} at x{1}  pid {2}" -f $a, $ls, $x.ProcessId)
}
if ($executors.Count -eq 0) { Say '  no executor - nothing can send an order (this is the normal state after a reboot)' }

$oldest = $traders | Sort-Object CreationDate | Select-Object -First 1
if ($oldest) { Say ("  oldest trader started " + $oldest.CreationDate) }

Say ''
Say '================ THE MONEY ================'
$liveDir = Join-Path $Root 'data\live'
if (Test-Path $liveDir) {
    foreach ($acct in Get-ChildItem $liveDir -Directory) {
        foreach ($book in Get-ChildItem $acct.FullName -Directory) {
            $bj = Join-Path $book.FullName 'broker.json'
            if (-not (Test-Path $bj)) { continue }
            $j = Get-Content $bj -Raw | ConvertFrom-Json
            $age = [int](((Get-Date).ToUniversalTime() - (Get-Date '1970-01-01')).TotalSeconds - $j.at/1000)
            # Only the books an executor is actually driving are worth a line;
            # a directory is never deleted, so most of these are history.
            if ($age -gt 600) { continue }
            $pos = if ($j.position) { "$($j.position.side) $($j.position.lots) lots, ticket $($j.position.ticket)" } else { 'flat' }
            Say ("  {0}/{1}  login {2}  {3} {4}  {5}" -f $acct.Name, $book.Name, $j.login, $j.balance, $j.currency, $pos)
            if (Test-Path (Join-Path $book.FullName 'STOP')) { Say '     STOP file present - this book will not trade here' }
        }
    }
}

Say ''
Say '================ PAPER ================'
try {
    $s = Invoke-RestMethod http://127.0.0.1:8138/api/paper/status -TimeoutSec 40
    $now = ((Get-Date).ToUniversalTime() - (Get-Date '1970-01-01')).TotalMilliseconds
    $stale = @($s.runs | Where-Object { $_.live -and ($now - $_.live.at) -gt 300000 })
    Say ("  {0} books, {1} holding a position" -f @($s.runs).Count, @($s.runs | Where-Object { $_.open }).Count)
    if ($stale.Count) { Warn ("  {0} book(s) have a feed over 5 minutes old" -f $stale.Count) }
    foreach ($r in @($s.runs | Where-Object { $_.excluded })) {
        Say ("  {0}: holding out {1} trade(s), {2} not counted" -f $r.id, $r.excluded.trades, $r.excluded.net_usd)
    }
} catch { Warn ("  paper status unreadable: " + $_.Exception.Message) }

Say ''
Say '================ BOOT TASKS ================'
foreach ($n in 'flowdesk-api','flowdesk-watch','flowdesk-traders','flowdesk-bars-export') {
    $t = Get-ScheduledTask -TaskName $n -ErrorAction SilentlyContinue
    if (-not $t) { Warn ("  {0}: NOT REGISTERED" -f $n); continue }
    Say ("  {0,-22} {1}" -f $n, $t.State)
}
Say '  (executors and the Opus/Terra traders are deliberately NOT here - see docs\VAN-HANH.md)'

Say ''
if ($bad) { Write-Host "$bad thing(s) worth looking at above." -ForegroundColor Yellow }
else { Write-Host 'Nothing is complaining.' -ForegroundColor Green }
Say ''
