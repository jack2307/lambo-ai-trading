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
if (-not (Test-Path $registry)) { Write-Host "missing: $registry" -ForegroundColor Red; exit 1 }

function Read-Registry($argsList) {
    if ($RegistryFile) { $argsList = @('--file', $RegistryFile) + $argsList }
    $json = & $Python $registry @argsList 2>&1
    if ($LASTEXITCODE) { Write-Host ("accounts.py " + ($argsList -join ' ') + " failed: $json") -ForegroundColor Red; exit 1 }
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
    default { Write-Host "[prices] symbol_suffix '$($prices.symbol_suffix)' is not one start_pollers.ps1 knows ('.sc' or empty)" -ForegroundColor Red; exit 1 }
}

# Every terminal that must be up: each enabled account's own, plus the price
# feed's. Deduplicated by path, because on this desk the price terminal is
# currently also an account's and starting it twice would be two processes
# fighting over one directory.
$plan = @{}
foreach ($a in $accounts) {
    if (-not $a.enabled) { continue }
    $plan[$a.terminal] = @{ path = $a.terminal; why = @(); autologin = $false }
}
if (-not $plan.ContainsKey($priceTerminal)) { $plan[$priceTerminal] = @{ path = $priceTerminal; why = @(); autologin = $false } }
foreach ($a in $accounts) {
    if ($a.enabled) {
        $plan[$a.terminal].why += ("$($a.id) login $($a.login)" + $(if ($a.real_money) { ' REAL MONEY' } else { '' }))
        # Opt-in, from the registry. See the `autologin` key's comment there:
        # deriving this from "the file exists" would hand an unread autologin
        # to the terminal carrying real money, which the old launcher never
        # did.
        if ($a.autologin) { $plan[$a.terminal].autologin = $true }
    }
}
$plan[$priceTerminal].why += 'prices'

# Decided here rather than at launch, so the plan printed below SAYS whether a
# terminal will be handed an autologin file. It was decided nine lines lower
# to begin with, past the `-PlanOnly` exit, which meant the selftest could not
# see the one decision on this path that touches a funded terminal. A choice an
# operator cannot read before it happens is a choice nobody reviews.
foreach ($k in @($plan.Keys)) {
    $f = Join-Path (Split-Path -Parent $k) ('config' + [char]92 + 'autologin.ini')
    $plan[$k].autologinFile = $f
    $plan[$k].autologinPresent = (Test-Path $f)
    $plan[$k].autologinUse = ($plan[$k].autologin -and $plan[$k].autologinPresent)
}

# PRINTED BEFORE ANYTHING STARTS, so the operator reads the plan rather than
# inferring it from what came up. The desk has been started by hand at speed
# more than once, and "three of four things" is not visible in a scroll of
# green lines.
Note 'launch plan, from config/accounts.toml:'
foreach ($k in ($plan.Keys | Sort-Object)) {
    $note = ''
    if ($plan[$k].autologinUse) { $note = '  [autologin]' }
    elseif ($plan[$k].autologin) { $note = '  [autologin asked for, file missing]' }
    elseif ($plan[$k].autologinPresent) { $note = '  [autologin present, NOT used]' }
    Note ("   " + $k + "  <- " + ($plan[$k].why -join ', ') + $note)
}
foreach ($a in $accounts) {
    if ($a.enabled) { continue }
    # A disabled account whose terminal is the PRICE feed still has its
    # terminal started, and saying "not started" beside a plan line that starts
    # it is two true sentences that read as nonsense together.
    if ($plan.ContainsKey($a.terminal)) {
        Note ("   (disabled: " + $a.id + " - its terminal starts anyway, for prices)")
    } else {
        Note ("   (disabled, not started: " + $a.id + " " + $a.terminal + ")")
    }
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
    Write-Host 'refusing to start a partial desk; fix config/accounts.toml or install the terminal' -ForegroundColor Red
    exit 1
}

if ($PlanOnly) { Note 'plan only: nothing started'; exit 0 }

$startedAny = $false
foreach ($k in ($plan.Keys | Sort-Object)) {
    $what = $plan[$k].why -join ', '
    $running = Get-Process terminal64 -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $k }
    if ($running) {
        Note ("already up: " + $what)
    } else {
        $startedAny = $true
        # `/portable` keeps each install's config and logs in its own
        # directory, which is what makes two terminals on one machine
        # independent rather than two views of one profile.
        #
        # `/config:` IS OPT-IN, FROM THE REGISTRY, AND THE DEFAULT IS OFF.
        #
        # The first version of this derived it — pass `/config:` if the install
        # has a `config\autologin.ini`. That is one line and it is wrong on
        # this desk. `/portable` gives EVERY install that directory, any
        # terminal ever configured through the GUI may have such a file, and
        # nobody has read the one on the funded install. So the derivation
        # would have handed an unread autologin to the terminal carrying real
        # money, which the launcher it replaced never did: either it names a
        # different account and the terminal comes up on the wrong login, or it
        # names the same one and forces a re-login on the terminal that is also
        # the price feed — the exact failure `config/accounts.toml` warns about,
        # on a machine with no second terminal to fall back to. Found by b5
        # reviewing the change.
        #
        # The thing the derivation was for is still worth having: a terminal
        # that silently starts unauthorised answers every request with "no such
        # symbol", which reads as a broker problem. So an unused autologin is
        # WARNED about rather than acted on. Saying it out loud costs nothing
        # and logging somebody in does not.
        $targs = @('/portable')
        if ($plan[$k].autologinUse) { $targs += ("/config:" + $plan[$k].autologinFile) }
        Start-Process -FilePath $k -ArgumentList $targs
        Note ("started: " + $what + $(if ($plan[$k].autologinUse) { ' (autologin)' } else { '' }))
    }
}
# Only when something actually started. Restarting after a crash the terminals
# survived is the common case, and twenty seconds of nothing there reads as the
# script having hung.
if ($startedAny) {
    Note 'give them a moment to authorise before the pollers ask for bars'
    Start-Sleep -Seconds 20
}

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
if (-not $ready) { Write-Host 'fd-api did not answer within 30s; read data\paper\logs\fd-api.err' -ForegroundColor Red; exit 1 }
Note 'answering on 127.0.0.1:8138'

# ------------------------------------------------------------ the rest
Step 'pollers, traders, watch'
# Their output is shown, not swallowed. Measured 2026-09-17: start_pollers.ps1
# was silently starting nothing - the output that said so went to Out-Null, and
# the desk ran for an hour with three traders and no prices reaching them.
Note "prices: $priceTerminal, $priceSymbols symbols$(if ($prices.fill_on_open) { ', fill on open' })"
$launchers = @(
    @{ script = 'start_pollers.ps1'; args = @('-Terminal', $priceTerminal, '-Symbols', $priceSymbols) + $(if ($prices.fill_on_open) { @('-FillOnOpen') } else { @() }) },
    @{ script = 'start_ai_traders.ps1'; args = @() },
    # The advisor panel, AFTER the traders because it advises them, and with
    # NO ARGUMENTS ON PURPOSE - which means dry run, because `-Apply` is a
    # switch and a switch not passed is off.
    #
    # That is the whole safety property of this line, and it is worth saying
    # rather than leaving as an absence: a reboot can start the panel, and a
    # reboot can never start it APPLIED. Applied verdicts shrink or refuse real
    # entries on every mirrored book; whether that ever happens is the owner's
    # decision and a new campaign generation, so it is something a person types
    # once, deliberately, and not a state this machine can come up in. Do not
    # add `-Apply` here. If applied running is ever agreed it belongs in the
    # registry beside the other opt-ins, where granting it costs a reviewable
    # edit.
    #
    # This line was missing entirely until 2026-09-18, which is why the panel
    # had never run on this machine: no process, no log, and a comment in
    # start_ai_traders.ps1 as the only trace that it was meant to exist.
    @{ script = 'start_advisor.ps1'; args = @() },
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
    # `$argv`, not `$args`: `$args` is an automatic variable and assigning to
    # it happens to work at script scope and does not inside a function. The
    # same trap cost this repo a fix in `start_executors.ps1` (9ee54c8); it was
    # latent here and is not worth leaving for the next person who wraps this
    # block in a function.
    $argv = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $Root 'py\live\start_executors.ps1'))
    if ($MirrorsLive) { $argv += '-Live' } else { $argv += '-DryRun' }
    & powershell $argv
}

Write-Host "`nRemember: DISCONNECT this RDP session, do not LOG OFF." -ForegroundColor Green
Write-Host 'Logging off destroys the session and everything in it, and the desk' -ForegroundColor DarkGray
Write-Host 'goes blind with no error anywhere - the feed just stops.' -ForegroundColor DarkGray
