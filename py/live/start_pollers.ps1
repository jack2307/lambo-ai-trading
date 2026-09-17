# Start (or restart) the read-only MT5 bar pollers that feed the paper runs.
#
#   powershell -NoProfile -File py\live\start_pollers.ps1 [-Warm 120]
#
# One process per stream; logs under data\paper\logs\. Stops any poller
# already running first, so this is safe to re-run after a reboot or a
# fd-api restart (the runs ignore bars they have seen and take the newer
# ones, so a re-sent warm-up loses nothing). Nothing here can trade: the
# poller calls initialize, symbol_info, copy_rates_from_pos and shutdown.
param(
    [int]$Warm = 120,
    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    # Which terminal the prices come from. Empty lets the MetaTrader5 package
    # pick, which is fine on a machine with one terminal and a coin flip on a
    # machine with two.
    [string]$Terminal = ''
)

# $Root is computed, not given, and a wrong one does not announce itself: the
# pollers would start with the wrong working directory, fail to find
# py/live/mt5_bars.py, and leave four empty log files. Measured 2026-09-17,
# when a bad edit to the param block above turned $Root into an array and this
# script started nothing at all while reporting four successful starts.
if (-not (Test-Path (Join-Path $Root 'py\live\mt5_bars.py'))) {
    throw "Root does not look like the flowdesk repo: '$Root' (no py\live\mt5_bars.py)"
}

$streams = @(
    @{ symbol = 'XAUUSD.sc'; market = 'xauusd'; tf = 'M15'; log = 'mt5_bars_xau_m15'; warm = $Warm },
    @{ symbol = 'XAUUSD.sc'; market = 'xauusd'; tf = 'M5';  log = 'mt5_bars_xau_m5';  warm = ($Warm * 3) },
    @{ symbol = 'BTCUSD.sc'; market = 'btcusd'; tf = 'M15'; log = 'mt5_bars_btc_m15'; warm = $Warm },
    @{ symbol = 'EURUSD.sc'; market = 'eurusd'; tf = 'M15'; log = 'mt5_bars_eur_m15'; warm = $Warm }
)

Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*mt5_bars.py*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2

$logs = Join-Path $Root 'data\paper\logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
foreach ($s in $streams) {
    $args = @('py/live/mt5_bars.py', "--symbol=$($s.symbol)", "--market=$($s.market)", "--tf=$($s.tf)", "--warm=$($s.warm)", '--poll=5', '--tick-poll=1')
    # Named only when given. A machine with one terminal has nothing to choose
    # between; a machine with two does, and the cent symbols quoted here live
    # on the live account only.
    if ($Terminal) { $args += "--terminal=$Terminal" }
    Start-Process -FilePath $Python -ArgumentList $args -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs "$($s.log).out") -RedirectStandardError (Join-Path $logs "$($s.log).err")
    Write-Host "started $($s.symbol) $($s.tf) -> $($s.market) (warm $($s.warm))"
}
