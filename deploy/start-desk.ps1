# Start everything, in the order that matters. RUN THIS IN THE RDP SESSION.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\start-desk.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\start-desk.ps1 -WithMirrors
#
# WHY THIS IS NOT A SERVICE, AND NOT A SCHEDULED TASK RUNNING AS SYSTEM
#
# MetaTrader is a GUI application. In a session with no desktop it starts, fails
# to create its windows, and dies before it ever reaches the network - measured
# 2026-09-17 on a server, where the log showed "Window MDI create failed" and
# then no Network lines at all. That is true of every Windows version; it is not
# a property of any particular one.
#
# So MetaTrader must live in an interactive session. And the Python that talks
# to it reaches it over local IPC, which is simplest and most reliable from the
# SAME session. Putting the terminals in an RDP session and the rest under
# SYSTEM would be betting that the IPC crosses a session boundary, which is a
# bet this desk does not need to take.
#
# The consequence, and it is the operating rule for the whole machine:
#
#     DISCONNECT the RDP session. Do not LOG OFF.
#
# Disconnecting leaves the session running and everything in it alive. Logging
# off destroys the session and every process in it, and the desk goes blind
# without a single error anywhere - the feed simply stops.
param(
    [string]$Root = '',
    # Start the mirrors too. Off by default: sending real orders should be a
    # decision made while reading the output, not the tail end of a startup
    # script.
    [switch]$WithMirrors,
    [switch]$MirrorsLive,

    # Also start the live terminal, and read prices from it.
    #
    # Off by default, which is a decision about this machine and not a
    # preference: a rented server does not need the real account's
    # credentials on it. The demo account quotes the same gold, the same
    # bitcoin and the same euro - the standard symbol and the cent symbol
    # differ in contract size, not in price - so prices come from the demo
    # terminal and nothing about the books changes.
    #
    # Pass this once the live terminal here has actually been logged in.
    # Until then it would start, sit unauthorised, and the pollers would ask
    # it for XAUUSD.sc and be told there is no such symbol.
    [switch]$WithLive
)

$ErrorActionPreference = 'Stop'
if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}
Set-Location $Root

function Step($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Note($m) { Write-Host "   $m" -ForegroundColor DarkGray }
function Warn($m) { Write-Host "   $m" -ForegroundColor Yellow }

# ----------------------------------------------------------- the terminals
Step 'MetaTrader'
if (-not $env:SESSIONNAME) {
    Warn 'No session name: this looks like an SSH or service session, which has no'
    Warn 'desktop. MetaTrader will not start here. Run this from RDP.'
}
# Where the bars come from, decided here and passed to the poller, so the one
# place that knows which terminal is logged in is also the place that names
# the symbols that terminal carries.
if ($WithLive) {
    $priceTerminal = 'C:\MT5-live\terminal64.exe'
    $priceSymbols = 'cent'
} else {
    $priceTerminal = 'C:\MT5-demo\terminal64.exe'
    $priceSymbols = 'standard'
}

$terminals = @(
    @{ path = 'C:\MT5-demo\terminal64.exe'; args = @('/portable', '/config:C:\MT5-demo\config\autologin.ini'); what = 'demo (execution' + $(if ($WithLive) { '' } else { ' and prices' }) + ')' }
)
if ($WithLive) {
    $terminals += @{ path = 'C:\MT5-live\terminal64.exe'; args = @('/portable'); what = 'live (prices)' }
}
foreach ($t in $terminals) {
    if (-not (Test-Path $t.path)) { Warn ("missing: " + $t.path); continue }
    $running = Get-Process terminal64 -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $t.path }
    if ($running) {
        Note ("already up: " + $t.what)
    } else {
        Start-Process -FilePath $t.path -ArgumentList $t.args
        Note ("started: " + $t.what)
    }
}
Note 'give them a moment to authorise before the pollers ask for bars'
Start-Sleep -Seconds 20

# --------------------------------------------------------------- the API
Step 'fd-api'
$logs = Join-Path $Root 'data\paper\logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
if (Get-Process fd-api -ErrorAction SilentlyContinue) {
    Note 'already up'
} else {
    Start-Process -FilePath (Join-Path $Root 'target\release\fd-api.exe') -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs 'fd-api.out') -RedirectStandardError (Join-Path $logs 'fd-api.err')
}

# Waited for, not slept past. Everything below posts to it, and a poller that
# starts against a socket nobody is listening on logs one error and then
# behaves exactly as though the market were quiet.
$ready = $false
foreach ($i in 1..30) {
    Start-Sleep -Seconds 1
    try {
        Invoke-WebRequest -Uri 'http://127.0.0.1:8138/api/paper/status' -UseBasicParsing -TimeoutSec 2 | Out-Null
        $ready = $true; break
    } catch { }
}
if (-not $ready) { Write-Error 'fd-api did not answer within 30s; read data\paper\logs\fd-api.err'; exit 1 }
Note 'answering on 127.0.0.1:8138'

# ------------------------------------------------------------ the rest
Step 'pollers, traders, watch'
# Their output is shown, not swallowed. Measured 2026-09-17: start_pollers.ps1
# was silently starting nothing - the output that said so went to Out-Null, and
# the desk ran for an hour with three traders and no prices reaching them.
Note "prices: $priceTerminal, $priceSymbols symbols"
$launchers = @(
    @{ script = 'start_pollers.ps1'; args = @('-Terminal', $priceTerminal, '-Symbols', $priceSymbols) },
    @{ script = 'start_ai_traders.ps1'; args = @() },
    @{ script = 'start_telegram.ps1'; args = @() }
)
foreach ($l in $launchers) {
    $p = Join-Path $Root ("py\live\" + $l.script)
    if (-not (Test-Path $p)) { Warn ("missing: " + $l.script); continue }
    Note $l.script
    & powershell (@('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $p) + $l.args) 2>&1 |
        ForEach-Object { Write-Host "     $_" -ForegroundColor DarkGray }
    if ($LASTEXITCODE) { Warn ($l.script + " exited $LASTEXITCODE") }
}

# --------------------------------------------------------------- mirrors
Step 'mirrors'
if (-not $WithMirrors) {
    Note 'not started (pass -WithMirrors)'
    Note 'Before you do: make sure no OTHER machine is mirroring the same account.'
    Note 'Each launcher holds a system-wide mutex, which stops two copies on one'
    Note 'machine and nothing at all across two. Two executors on one account is'
    Note 'two copies of every order.'
} else {
    $args = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $Root 'py\live\start_executors.ps1'))
    if ($MirrorsLive) { $args += '-Live' } else { $args += '-DryRun' }
    & powershell $args
}

Write-Host "`nRemember: DISCONNECT this RDP session, do not LOG OFF." -ForegroundColor Green
Write-Host 'Logging off destroys the session and everything in it, and the desk' -ForegroundColor DarkGray
Write-Host 'goes blind with no error anywhere - the feed just stops.' -ForegroundColor DarkGray
