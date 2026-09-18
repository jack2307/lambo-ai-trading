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

    # Where python is. The same default the launchers under py/live use, and
    # named here because this script now asks the registry what to start.
    [string]$Python = 'C:\Python39\python.exe'
,

    # Print the launch plan and stop, starting nothing.
    #
    # For reading before you start the desk by hand, and for the selftest,
    # which needs to exercise the plan and the refusal without launching a
    # terminal or an API. It stops AFTER the missing-terminal check, so a
    # registry that names a path this machine does not have still exits
    # non-zero here - that check is the one most worth being able to run.
    [switch]$PlanOnly,

    # Read a registry other than `config/accounts.toml`. For the selftest,
    # which needs a registry describing terminals it created in a temp
    # directory; an empty value means the default, which is what every real
    # run uses.
    [string]$RegistryFile = ''
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

# THE LAUNCH PLAN COMES FROM THE REGISTRY, NOT FROM THIS FILE.
#
# Until 2026-09-18 the terminal paths were hard-coded here: `C:\MT5-demo` and
# `C:\MT5-live`. By then the desk had a funded account on `C:\MT5-cent` and
# `C:\MT5-live` did not exist on the server at all, so the script whose first
# line is "Start everything, in the order that matters" could not start the
# terminal the real money runs through - and would have looked like a clean
# start with the funded account simply absent. Nothing detected it because a
# launcher that starts three of four things exits 0.
#
# `config/accounts.toml` already knew. It names the terminal for every account
# and is the file you are told to edit when you add one. So this asks it,
# through `py/live/accounts.py`, which is the one reader - a second parser here
# would be a second answer to the same question, and the two would agree until
# the day they did not.
#
# The consequence worth having: add an account to the registry tomorrow and
# this starts its terminal without being edited.
$registry = Join-Path $Root 'py\live\accounts.py'
if (-not (Test-Path $registry)) { Write-Error "missing: $registry"; exit 1 }

function Read-Registry($argsList) {
    if ($RegistryFile) { $argsList = @('--file', $RegistryFile) + $argsList }
    $json = & $Python $registry @argsList 2>&1
    if ($LASTEXITCODE) { Write-Error ("accounts.py " + ($argsList -join ' ') + " failed: $json"); exit 1 }
    return ($json | ConvertFrom-Json)
}
$accounts = @(Read-Registry @('--all'))
$prices = Read-Registry @('--prices')

# Which terminal the bars come from. ONE key, read here and by the
# higher-timeframe export, so the 15m series a book decides on and the H4/D1
# series it reads for context cannot come from different terminals. The
# argument for the terminal it currently names - and the rule it overrules - is
# in `config/accounts.toml` beside the key, not repeated here.
#
# This replaces a `-WithLive` switch that chose between `C:\MT5-demo` and
# `C:\MT5-live`. Its reasoning was "a rented server does not need the real
# account's credentials on it", which was a good rule and was deliberately
# abandoned on 2026-09-17 when the owner funded the cent account and asked for
# it to trade from the VPS. The replacement controls are the three-key
# real-money lock in `mt5_executor.py`, the SYSTEM-task / RDP-session split,
# and SSH by key only. Recorded so the rule is not re-proposed as new.
$priceTerminal = $prices.terminal
# `start_pollers.ps1` takes a NAME for the symbol set rather than the suffix
# itself, so the registry's suffix is translated here. One place, and it is
# named rather than inlined so that a third symbol set makes this fail loudly
# instead of silently choosing 'standard'.
switch ($prices.symbol_suffix) {
    '.sc'   { $priceSymbols = 'cent' }
    ''      { $priceSymbols = 'standard' }
    default { Write-Error "[prices] symbol_suffix '$($prices.symbol_suffix)' is not one start_pollers.ps1 knows ('.sc' or empty)"; exit 1 }
}

# Every terminal that must be up: each enabled account's own, plus the price
# feed's. Deduplicated by path, because on this desk the price terminal is
# currently also an account's and starting it twice would be two processes
# fighting over one directory.
$plan = @{}
foreach ($a in $accounts) {
    if (-not $a.enabled) { continue }
    $plan[$a.terminal] = @{ path = $a.terminal; why = @() }
}
if (-not $plan.ContainsKey($priceTerminal)) { $plan[$priceTerminal] = @{ path = $priceTerminal; why = @() } }
foreach ($a in $accounts) {
    if ($a.enabled) {
        $plan[$a.terminal].why += ("$($a.id) login $($a.login)" + $(if ($a.real_money) { ' REAL MONEY' } else { '' }))
    }
}
$plan[$priceTerminal].why += 'prices'

# PRINTED BEFORE ANYTHING STARTS, so the operator reads the plan rather than
# inferring it from what came up. The desk has been started by hand at speed
# more than once, and "three of four things" is not visible in a scroll of
# green lines.
Note 'launch plan, from config/accounts.toml:'
foreach ($k in ($plan.Keys | Sort-Object)) {
    Note ("   " + $k + "  <- " + ($plan[$k].why -join ', '))
}
foreach ($a in $accounts) {
    if (-not $a.enabled) { Note ("   (disabled, not started: " + $a.id + " " + $a.terminal + ")") }
}

# A registry path that is not on this disk is a STOP, not a warning.
#
# It used to warn and continue, which is how a missing terminal became a desk
# that ran without one. The registry is the desk's own description of itself:
# if it names a terminal this machine does not have, either the file is wrong
# or the machine is, and both are things to fix before any money moves.
$absent = @($plan.Keys | Where-Object { -not (Test-Path $_) })
if ($absent.Count) {
    foreach ($m in $absent) { Warn ("registry names a terminal that is not on this disk: " + $m) }
    Write-Error 'refusing to start a partial desk; fix config/accounts.toml or install the terminal'
    exit 1
}

if ($PlanOnly) { Note 'plan only: nothing started'; exit 0 }

foreach ($k in ($plan.Keys | Sort-Object)) {
    $what = $plan[$k].why -join ', '
    $running = Get-Process terminal64 -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $k }
    if ($running) {
        Note ("already up: " + $what)
    } else {
        # `/portable` keeps each install's config and logs in its own
        # directory, which is what makes two terminals on one machine
        # independent rather than two views of one profile.
        #
        # `/config:` is passed when the terminal has an autologin file, and is
        # DERIVED rather than hard-coded. The old script passed it for the demo
        # terminal only, by path, so a second terminal with an autologin would
        # have started unauthorised and then answered every request with "no
        # such symbol" - which reads as a broker problem, not a launcher one.
        # Each install keeps its own under `config\autologin.ini` because
        # `/portable` puts it there.
        $targs = @('/portable')
        $autologin = Join-Path (Split-Path -Parent $k) 'config\autologin.ini'
        if (Test-Path $autologin) { $targs += "/config:$autologin" }
        Start-Process -FilePath $k -ArgumentList $targs
        Note ("started: " + $what + $(if (Test-Path $autologin) { ' (autologin)' } else { '' }))
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
