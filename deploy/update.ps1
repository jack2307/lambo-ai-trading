# Pull, rebuild, restart. RUN THIS ON THE SERVER, every time there is a change.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1
#   powershell ... -File deploy\update.ps1 -ServerOnly -Staged C:\Windows\Temp\flowdesk-build
#   powershell ... -File deploy\update.ps1 -ServerOnly -Binary <exe> -Client <dist>
#   powershell ... -File deploy\update.ps1 -NoRestart
#   powershell ... -File deploy\update.ps1 -AllowOrphans
#
# THE SERVER HAS NO TOOLCHAIN. The VPS has git and nothing else - no cargo, no
# rustc - so `cargo build` can never run on the one machine this script exists
# for. The copy is therefore the primary path and the build is the local
# convenience, which is the opposite of how this file read until 2026-09-17.
# -Staged names a folder holding `fd-api.exe` and `dist\`; -Binary and -Client
# name them separately. With neither, and no cargo, the run REFUSES at the
# build step rather than restarting the old binary green.
#
# TWO MODES, AND THEY DIFFER IN WHAT THEY KILL.
#
#   (default)     the whole desk: executors, traders, pollers and fd-api stop,
#                 everything but the mirrors comes back. REFUSES while a
#                 real-money account is holding, because stopping the executors
#                 leaves that position with nobody mirroring the book's exit;
#                 -AllowOrphans is the override, and what refusing costs is
#                 printed where it happens rather than left for the reader.
#   -ServerOnly   fd-api and the watch, and NOTHING else. Executors, traders
#                 and pollers are never signalled. Nothing is orphaned, so an
#                 open position is NAMED and does not refuse.
#
# The two cannot be blended: -ServerOnly with a flag that only means something
# in the full mode is refused rather than interpreted, because a flag that is
# inert is indistinguishable from a flag that was understood.
#
# EVERY IRREVERSIBLE STAGE PRINTS WHAT IT IS ABOUT TO DO BEFORE DOING IT, and
# every artefact it replaces is rolled aside with a timestamp rather than
# deleted. See docs/decisions/2026-09-17-deploy-staleness.md: the incident that
# prompted that note was a 13 MB binary with tonight's mtime and last week's
# contents, and mtime is the number a reader instinctively trusts.
#
# The desk is stopped before anything is rebuilt and started again after, in an
# order that matters: the executors go down FIRST and come up LAST.
#
# THE REASON THIS FILE USED TO GIVE FOR THAT WAS WRONG, and it is corrected
# here rather than quietly replaced because it is the reason -ServerOnly looked
# dangerous. It said: "an executor talking to an fd-api that is mid-restart
# reads no book at all, and its rule for 'the book is flat and the account is
# not' is to close the position - so a restart with the mirrors still running
# would flatten every live trade on the way past."
#
# It does not. mt5_executor.py catches the failed read and never reaches the
# reconciler: `api-unreachable` logs, writes a snapshot, sleeps and continues,
# and `run-missing` does the same. A mirror cannot close a position on a book
# it could not read.
#
# What WOULD do it is narrower and still worth the ordering: an fd-api that
# answers, with the run present, and its book flat - a restarted process whose
# paper state came back incomplete. That is a live risk and it is why the full
# mode still stops the mirrors first. But it is not the unreachable window, and
# -ServerOnly is safe across the unreachable window for exactly that reason.
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
    [switch]$AllowOrphans,

    # Restart fd-api and the watch, and touch nothing else.
    #
    # For the deploy that moves the binary and the client while the mirrors go
    # on running the Python they started with. That is not a lesser version of
    # the full mode, it is a different intention: the full mode says "the desk
    # is going down and coming back", this one says "the server process is
    # being replaced underneath a desk that stays up".
    #
    # The executors are not signalled at all, so nothing is orphaned and an
    # open position does not refuse - it is named, because a person running a
    # deploy while the account is holding should see that on the screen even
    # when it is safe.
    #
    # What it does NOT do, stated because it is the thing someone will assume:
    # the running executors keep the Python they were launched with. A change
    # to mt5_executor.py is on disk and not in effect after this mode, and the
    # run prints which books are still on old code rather than leaving that to
    # be discovered.
    [switch]$ServerOnly,

    # A prebuilt fd-api.exe to install, instead of building one here.
    #
    # THIS IS THE NORMAL CASE ON THE SERVER, not a fallback. The VPS has git
    # and nothing else - no cargo, no rustc - so the build step can never run
    # on the one machine this script exists for. Every deploy there is a
    # binary built elsewhere and copied in, which makes the copy the primary
    # path and `cargo build` the local convenience.
    #
    # Until 2026-09-17 that case ended with `cargo not found` after the desk
    # had already been stopped, or - worse, under -SkipBuild - with the OLD
    # binary restarting green and every downstream check passing.
    [string]$Binary = '',

    # A prebuilt ui/dist to install, same reasoning. A directory, not a zip.
    [string]$Client = '',

    # One folder holding both, as `fd-api.exe` and `dist\`. Sugar for the pair
    # above, because the staged copy arrives as a pair and typing two paths
    # that must agree is a way to get one of them wrong.
    [string]$Staged = ''
)

$ErrorActionPreference = 'Stop'

# ---- the modes cannot be blended, and this is checked before anything moves ----
#
# Refused rather than ignored. A flag that is silently inert reads exactly like
# a flag that was understood, and the operator who typed -AllowOrphans believes
# they have authorised something. In -ServerOnly nothing is orphaned, so that
# authorisation has no referent: the honest answer is to stop and say so.
if ($ServerOnly -and $AllowOrphans) {
    Write-Host '-ServerOnly and -AllowOrphans contradict each other.' -ForegroundColor Red
    Write-Host '  -ServerOnly orphans nothing: the executors are never signalled, so there is' -ForegroundColor Red
    Write-Host '  nothing for -AllowOrphans to permit. If you meant the full deploy, drop' -ForegroundColor Red
    Write-Host '  -ServerOnly. If you meant the server only, drop -AllowOrphans.' -ForegroundColor Red
    exit 2
}
if ($ServerOnly -and $NoRestart) {
    Write-Host '-ServerOnly and -NoRestart contradict each other: -ServerOnly IS a restart,' -ForegroundColor Red
    Write-Host '  of fd-api and the watch. -NoRestart pulls and builds and starts nothing.' -ForegroundColor Red
    exit 2
}

# -Staged is the pair, so it cannot also be given piecemeal.
if ($Staged -and ($Binary -or $Client)) {
    Write-Host '-Staged already names both artefacts; do not also pass -Binary or -Client.' -ForegroundColor Red
    exit 2
}
if ($Staged) {
    $Binary = Join-Path $Staged 'fd-api.exe'
    $Client = Join-Path $Staged 'dist'
}

# Sources are checked HERE, before the pull and long before anything is
# stopped. A path typed wrong should cost nothing; discovering it after the
# desk is down costs the desk.
if ($Binary) {
    if (-not (Test-Path $Binary -PathType Leaf)) {
        Write-Host "-Binary: no file at $Binary" -ForegroundColor Red
        exit 2
    }
}
if ($Client) {
    if (-not (Test-Path $Client -PathType Container)) {
        Write-Host "-Client: no directory at $Client (a built ui\dist, not a zip)" -ForegroundColor Red
        exit 2
    }
    if (-not (Test-Path (Join-Path $Client 'index.html'))) {
        Write-Host "-Client: $Client has no index.html, so it is not a built client" -ForegroundColor Red
        exit 2
    }
}

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

# PATH, re-read from the registry, before anything needs a tool.
#
# Measured on the VPS 2026-09-17: the first real run of this script died at
# `git pull` with "The term 'git' is not recognized". Over ssh the operator
# inherits sshd's environment, and sshd's PATH predates the Git install, so
# git is on the machine and not on the path of the process that needs it.
# Same defect the AI-trader launcher hit with node and fixed the same way -
# the registry is where the truth is, a session's copy is a snapshot.
#
# It failed at the safest point it could have, with the plan and the open
# positions printed and nothing stopped, which is luck rather than design:
# the same missing tool a few lines later is a desk that is down with no
# binary installed.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host 'git is not on PATH even after re-reading it from the registry.' -ForegroundColor Red
    Write-Host '  Nothing has been stopped. Install git, or put it on the machine PATH.' -ForegroundColor Red
    exit 2
}

function Step($what) { Write-Host "`n== $what" -ForegroundColor Cyan }
function Note($what) { Write-Host "   $what" -ForegroundColor DarkGray }

# Stages this run still owes, once it has done something it cannot undo.
#
# An exit after the stop leaves the desk mid-deploy, and the operator's first
# question is not why it stopped - the error says that - but WHAT DID NOT
# HAPPEN. On 2026-09-17 a refusal at the readiness check skipped the watch
# restart and nothing said so; the watch was restarted by hand once someone
# noticed. So every exit after the point of no return lists what it owed and
# did not do.
$script:Owed = @()
function Owe([string[]]$what) { $script:Owed = $what }
function Paid([string]$what) { $script:Owed = @($script:Owed | Where-Object { $_ -ne $what }) }
function Show-Owed {
    if ($script:Owed.Count -gt 0) {
        Write-Host ''
        Write-Host 'THIS RUN STOPPED PART WAY. It did NOT do the following, and they are' -ForegroundColor Yellow
        Write-Host 'not going to happen on their own:' -ForegroundColor Yellow
        foreach ($o in $script:Owed) { Write-Host "  - $o" -ForegroundColor Yellow }
    }
}
function Stop-Here([string]$why, [int]$code = 1) {
    Write-Host ''
    Write-Host $why -ForegroundColor Red
    Show-Owed
    exit $code
}

# And the same for everything that stops this script WITHOUT going through
# Stop-Here, which is most of it.
#
# `$ErrorActionPreference = 'Stop'` makes every `Write-Error` a TERMINATING
# error, so the `exit 1` written after each one never executes and the script
# dies where it stands. That was fine while the only question was whether it
# stopped. It is not fine now that the answer to "what did it skip" has to be
# on the screen, and a trap covers the failures nobody anticipated as well as
# the ten that were written out - which is the point, since the refusal that
# skipped the watch restart on 2026-09-17 was one nobody had anticipated.
trap {
    Write-Host ''
    Write-Host "$($_.Exception.Message)" -ForegroundColor Red
    Show-Owed
    exit 1
}

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
    if (Get-DeskTask $API_TASK) { Stop-DeskTask $API_TASK 'fd-api' }
    else { Get-Process fd-api -ErrorAction SilentlyContinue | Stop-Process -Force }
    Start-Sleep -Seconds 3
}

# ------------------------------------------------- scheduled tasks, not children
#
# On the VPS fd-api and the watch run as SYSTEM scheduled tasks, and they are
# there BECAUSE a session-bound process did not survive: anything started from
# a console or an SSH session died with it. So this script must drive the
# tasks, not spawn children.
#
# It would have been a green failure, which is the worst kind. Starting fd-api
# with Start-Process there passes the hash check, prints a clean deploy, and
# leaves a process that dies at logoff while the scheduled task believes it is
# not running. Nothing on the screen would have said so.
#
# There is a second reason and it is the advisor gate. FD_ADVISOR_PANEL is read
# from the PROCESS's environment. A task running as SYSTEM reads machine scope,
# which is what b68a4f6 was designed against; a child of the operator's session
# inherits the OPERATOR's environment, so a stray variable in one shell would
# switch the panel on for the desk.
$API_TASK = 'flowdesk-api'
$WATCH_TASK = 'flowdesk-watch'

function Get-DeskTask([string]$name) {
    # $null when the ScheduledTasks module is absent as well as when the task
    # is: both mean "this machine does not run it as a task", which is the
    # question the caller is asking.
    if (-not (Get-Command Get-ScheduledTask -ErrorAction SilentlyContinue)) { return $null }
    try { return Get-ScheduledTask -TaskName $name -ErrorAction Stop } catch { return $null }
}

function Stop-DeskTask([string]$name, [string]$procName, [string]$cmdLike = '') {
    # Stop the task, then kill any survivor. Both, in that order: stopping the
    # task asks Task Scheduler to end its instance, and a process that ignores
    # that would still be holding the exe when the copy runs.
    #
    # `$cmdLike` IS NOT OPTIONAL IN PRACTICE FOR ANYTHING NAMED python.
    # The first version of this took a bare process name and the watch call
    # passed 'python'. On the server that resolves to every python.exe there
    # is - the five executors, the AI traders, the pollers - and the survivor
    # sweep would have killed all of them, in the mode whose entire promise is
    # that it does not signal an executor. Caught by reading it back rather
    # than by running it, which on that machine would have been five orphaned
    # mirrors and an open real position.
    try { Stop-ScheduledTask -TaskName $name -ErrorAction Stop } catch { }
    Start-Sleep -Seconds 2
    $alive = @(Get-CimInstance Win32_Process -Filter "name='$procName.exe'" -ErrorAction SilentlyContinue |
               Where-Object { (-not $cmdLike) -or ($_.CommandLine -like $cmdLike) })
    if ($alive.Count -gt 0) {
        Note "$name left $($alive.Count) process(es) matching$(if ($cmdLike) { " $cmdLike" } else { " $procName.exe" }); stopping them"
        $alive | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 2
    }
}

function Test-Detached([string]$procName) {
    # Will the thing that is answering survive the operator logging off?
    #
    # THE FIRST VERSION ANSWERED THIS WITH "is its session different from
    # mine", AND THAT WAS WRONG OVER SSH. sshd's children run in session 0, so
    # a correctly task-started fd-api - parent svchost.exe, owner SYSTEM,
    # session 0 - matched the caller's session exactly and was refused as
    # session-bound. Measured on the VPS 2026-09-17: the evidence printed on
    # the line above the refusal was right and the verdict was wrong, and
    # because it exited there the watch restart never ran.
    #
    # So the verdict is now the two facts that actually decide it:
    #
    #   OURS      - the process is a descendant of this script, so it dies
    #               with this script whatever its session says. Unambiguous,
    #               and the only thing that hard-refuses.
    #   SERVICE   - the owner is a service account. A task running as SYSTEM
    #               survives a logoff; one running as the logged-on user does
    #               not, and that is the misconfiguration worth warning about.
    #
    # Sessions are still REPORTED, because "session 0, parent svchost.exe" is
    # what a person reads to confirm it by eye. They are just no longer the
    # verdict.
    $me = (Get-CimInstance Win32_Process -Filter "ProcessId=$PID").SessionId
    $p = @(Get-CimInstance Win32_Process -Filter "Name='$procName.exe'" -ErrorAction SilentlyContinue)
    if ($p.Count -eq 0) { return @{ found = $false } }
    $it = $p[0]
    $parent = 'gone'
    try {
        $pp = Get-CimInstance Win32_Process -Filter "ProcessId=$($it.ParentProcessId)" -ErrorAction Stop
        if ($pp) { $parent = $pp.Name }
    } catch { }
    $owner = '?'
    try { $owner = (Invoke-CimMethod -InputObject $it -MethodName GetOwner).User } catch { }

    # Walk the parent chain looking for this script. Bounded, because a pid is
    # reused and a cycle would otherwise hang a deploy.
    $ours = $false
    $cur = $it.ParentProcessId
    for ($hop = 0; $hop -lt 8 -and $cur; $hop++) {
        if ($cur -eq $PID) { $ours = $true; break }
        try { $cur = (Get-CimInstance Win32_Process -Filter "ProcessId=$cur" -ErrorAction Stop).ParentProcessId }
        catch { break }
    }
    return @{
        found     = $true
        pid       = $it.ProcessId
        session   = $it.SessionId
        mySession = $me
        ours      = $ours
        service   = ($owner -in @('SYSTEM', 'LOCAL SERVICE', 'NETWORK SERVICE'))
        parent    = $parent
        owner     = $owner
    }
}

function Stop-Server {
    # -ServerOnly's stop: fd-api alone. The watch is not stopped here because
    # it is restarted at the end, which both replaces its code AND leaves it
    # alive across the window where fd-api is down - the window worth
    # watching. Same reasoning as the full mode's list above.
    if (Get-DeskTask $API_TASK) {
        Stop-DeskTask $API_TASK 'fd-api'
    } else {
        Get-Process fd-api -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Seconds 3
    }
}

# ---------------------------------------------------------------- staleness
#
# Everything below exists because of one incident and one general fact.
#
# The incident: a build made with `cargo` missing from PATH left a plausible
# 13 MB fd-api.exe in target/release. It was from before the day's changes.
# The general fact: `Copy-Item` stamps the DESTINATION, so a binary built three
# days ago and copied tonight carries tonight's mtime. mtime is worse than no
# evidence, because it is the number a reader instinctively checks and it says
# "fresh" exactly when the file is not.
#
# See docs/decisions/2026-09-17-deploy-staleness.md.

function Hash-Of([string]$path) {
    if (-not (Test-Path $path)) { return $null }
    try { return (Get-FileHash -Path $path -Algorithm SHA256).Hash.Substring(0, 12) } catch { return $null }
}

function Roll-Aside([string]$path, [string]$stamp) {
    # COPIED aside, not moved. The note says an old artefact is rolled aside
    # and never deleted; a move would also satisfy that and would leave the
    # tree without a binary or without a client if the build then failed, which
    # trades one silent failure for a louder one. A copy costs disk and keeps
    # both properties.
    if (-not (Test-Path $path)) { return $null }
    $dest = "$path.$stamp"
    try {
        Copy-Item -Path $path -Destination $dest -Recurse -Force -ErrorAction Stop
        return $dest
    } catch {
        Write-Host "   could not roll $path aside: $($_.Exception.Message)" -ForegroundColor Yellow
        return $null
    }
}

function Assert-BundleReferenced([string]$dist) {
    # The client is read from disk at runtime, so a stale ui/dist is served
    # forever and is internally consistent while it does it - index.html
    # references its own hashed bundle and that file is there, so nothing 404s.
    #
    # Check the REFERENCED name and never a directory listing. The hand check
    # done on the night this was written grepped the oldest index-*.js in the
    # directory, because `ls | head -1` is not "newest" and a dozen generations
    # are kept. The reference is the only name that means anything.
    $index = Join-Path $dist 'index.html'
    if (-not (Test-Path $index)) { return @{ ok = $false; why = "no index.html in $dist" } }
    $html = Get-Content $index -Raw
    $m = [regex]::Matches($html, '(?:src|href)="(?<p>[^"]*?/assets/[^"]+?\.(?:js|css))"')
    if ($m.Count -eq 0) { return @{ ok = $false; why = "index.html references no /assets/ bundle" } }
    $missing = @()
    $seen = @()
    foreach ($x in $m) {
        $rel = $x.Groups['p'].Value.TrimStart('/')
        $seen += $rel
        if (-not (Test-Path (Join-Path $dist $rel))) { $missing += $rel }
    }
    if ($missing.Count -gt 0) {
        return @{ ok = $false; why = "index.html references $($missing -join ', '), which $($dist) does not contain" }
    }
    return @{ ok = $true; refs = $seen }
}

function Wait-ForVersion([string]$expected, [int]$seconds = 30) {
    # Readiness is the hash, not a 200.
    #
    # The old probe polled /api/paper/status and accepted any answer. A stale
    # binary answers that endpoint perfectly - it is the same endpoint it has
    # always served - so the probe could not fail for the reason it exists.
    #
    # FAILS CLOSED. /api/version is being added on another branch and is not on
    # main yet. Until it is, this returns "missing" and the caller stops: an
    # absent check must not read as a passed one, which is the whole thesis of
    # the note this comes from.
    $sawSomething = $false
    foreach ($i in 1..$seconds) {
        Start-Sleep -Seconds 1
        try {
            $r = Invoke-WebRequest -Uri 'http://127.0.0.1:8138/api/version' -UseBasicParsing -TimeoutSec 2
            $v = $r.Content | ConvertFrom-Json
            return @{ state = 'answered'; body = $v }
        } catch {
            $code = $null
            try { $code = [int]$_.Exception.Response.StatusCode } catch { }
            if ($code -eq 404 -or $code -eq 501) {
                # The process is up and does not have the route. That is a
                # different fact from "not listening yet" and stops the wait.
                return @{ state = 'missing'; code = $code }
            }
            if ($code) { $sawSomething = $true }
        }
    }
    return @{ state = $(if ($sawSomething) { 'erroring' } else { 'silent' }) }
}

# ------------------------------------------------- the htf route's own inputs
#
# WHY THIS CHECK EXISTS, and it is not about the route being correct.
#
# `GET /api/paper/htf` reads data\bars\<SYMBOL>-4h.parquet and -1d.parquet.
# `data\` is gitignored, so NO COMMIT CAN SHIP THOSE FILES. A merge that
# brings the route, the exporter and the two prompt variants brings no bars.
# The route then answers "not exported yet" - correctly, in a 200, in a
# sentence written to be displayed - and every other check in this script
# passes, because every other check is about the binary. The deploy reports
# success and the first anyone learns of it is a decided bar with no context
# in it, hours later, in a book whose whole purpose was the context.
#
# That is a step whose only enforcement is somebody remembering. This turns it
# into a deploy-time stop.
#
# THREE ANSWERS, AND ONLY ONE OF THEM IS A FAILURE.
#
#   404/501  - the binary predates the route. That is an older server, not a
#              missing export, and refusing on it would block every deploy of
#              a commit that does not have the route yet. Noted, never fatal.
#   h4 null  - the route is there and has no bars. THIS is the failure, and it
#              is the one no other check can see.
#   h4 there - the bars are present. Their AGE is the route's business and the
#              block's, not this script's.
#
# WHICH MARKETS. The ones the account registry mirrors, resolved to markets by
# asking the desk - not a list written here, which would become a thing to
# remember to update the first time a market was added, i.e. exactly the class
# of failure this check exists to remove. It is also why the scope is the
# REGISTRY and not every loaded book: `eur-hours` is a paper book on eurusd
# that nobody mirrors and nobody has exported H4 for, and scoping to every
# book would refuse every deploy forever over a market no real money touches.
function Test-HtfReadiness([string]$root) {
    $acct = Join-Path $root 'config\accounts.toml'
    if (-not (Test-Path $acct)) {
        Note 'no config\accounts.toml, so no mirrored runs to check the htf route for'
        return $true
    }
    # The run ids the registry mirrors. Read with a small state machine rather
    # than a TOML parser, which PowerShell 5.1 does not have: inside a
    # `runs = [` block, take each quoted string until the closing bracket.
    $runs = @{}
    $inRuns = $false
    foreach ($line in (Get-Content $acct)) {
        $t = $line.Trim()
        if ($t -match '^runs\s*=\s*\[') { $inRuns = $true; continue }
        if ($inRuns) {
            if ($t -match '^\]') { $inRuns = $false; continue }
            if ($t -match "^[`"']([^`"']+)[`"']") { $runs[$Matches[1]] = $true }
        }
    }
    if ($runs.Count -eq 0) {
        Note 'the registry mirrors no runs, so there is no market to check the htf route for'
        return $true
    }

    # id -> market, from the desk itself.
    $markets = @{}
    try {
        $st = (Invoke-WebRequest -Uri 'http://127.0.0.1:8138/api/paper/status' -UseBasicParsing -TimeoutSec 5).Content | ConvertFrom-Json
        foreach ($r in @($st.runs)) { if ($runs.ContainsKey("$($r.id)") -and $r.market) { $markets["$($r.market)"] = $true } }
    } catch {
        Note "could not read /api/paper/status to resolve run markets ($($_.Exception.Message)); htf readiness NOT checked"
        return $true
    }
    if ($markets.Count -eq 0) {
        Note 'none of the mirrored runs is loaded, so there is no market to check the htf route for'
        return $true
    }

    $bad = @()
    foreach ($m in $markets.Keys) {
        $code = $null
        $body = $null
        try {
            $r = Invoke-WebRequest -Uri "http://127.0.0.1:8138/api/paper/htf?market=$m" -UseBasicParsing -TimeoutSec 5
            $body = $r.Content | ConvertFrom-Json
        } catch {
            try { $code = [int]$_.Exception.Response.StatusCode } catch { }
            if ($code -eq 404 -or $code -eq 501) {
                Note "this fd-api has no /api/paper/htf (HTTP $code) - it predates the route. Not a missing export; continuing."
                return $true
            }
            Note "could not reach /api/paper/htf for $m ($($_.Exception.Message)); htf readiness NOT checked"
            return $true
        }
        if ($null -eq $body.h4) {
            $bad += [pscustomobject]@{ Market = $m; Why = "$($body.unavailable)" }
        } else {
            Note "htf route has H4 bars for $m"
            if ($null -eq $body.d1) { Write-Host "   WARNING: $m has H4 but no D1 ($($body.unavailable))" -ForegroundColor Yellow }
        }
    }
    if ($bad.Count -eq 0) { return $true }

    Write-Host ''
    Write-Host 'THE HTF ROUTE HAS NO BARS FOR A MARKET THIS DESK MIRRORS.' -ForegroundColor Red
    foreach ($b in $bad) {
        Write-Host "  $($b.Market): $($b.Why)" -ForegroundColor Red
    }
    Write-Host '  This is NOT a code problem and a merge will not fix it. data\ is' -ForegroundColor Yellow
    Write-Host '  gitignored, so the bars ship with nothing - they have to be exported' -ForegroundColor Yellow
    Write-Host '  on THIS machine, once, against the running demo terminal:' -ForegroundColor Yellow
    Write-Host ''
    Write-Host '    C:\Python39\python.exe py\ingest\mt5_export.py --symbols XAUUSD ' -NoNewline -ForegroundColor Cyan
    Write-Host '--timeframes H4,D1 --terminal "C:\MT5-demo\terminal64.exe"' -ForegroundColor Cyan
    Write-Host ''
    Write-Host '  That is the same command deploy\run-htf-export.cmd runs hourly once' -ForegroundColor Yellow
    Write-Host '  install-tasks.ps1 -Apply has registered flowdesk-htf-export. Run it by' -ForegroundColor Yellow
    Write-Host '  hand first: if the symbol is wrong on this terminal, mt5_export logs' -ForegroundColor Yellow
    Write-Host '  "skipped" and still exits 0, so read its output rather than its code.' -ForegroundColor Yellow
    Write-Host '  Until the bars exist, any book on an htf prompt variant decides every' -ForegroundColor Yellow
    Write-Host '  bar with an apology where its context should be - which is a' -ForegroundColor Yellow
    Write-Host '  context-absent campaign wearing a context-present id.' -ForegroundColor Yellow
    return $false
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
$holding = @()

# ---- what this run will and will not touch, before it touches anything ----
#
# Printed rather than implied. The sentence that prompted -ServerOnly was "the
# executors do NOT restart", said about a script whose first act was to kill
# them: the intention and the script disagreed and nothing on the screen would
# have said so. This is the screen saying so.
Step 'what this run will touch'
$running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" -ErrorAction SilentlyContinue |
             Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
$artefact = if ($Binary -or $Client) {
    $bits = @()
    if ($Binary) { $bits += "binary from $Binary" }
    if ($Client) { $bits += "client from $Client" }
    'COPY ' + ($bits -join ' and ')
} elseif ($SkipBuild) {
    'NEITHER - -SkipBuild, so whatever binary and client are already here'
} elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
    'BUILD here with cargo'
} else {
    'NOTHING CAN - no cargo and no -Binary; this run will refuse at the build step'
}

# How each service will be driven, named per service because getting this
# wrong is silent: a session-bound fd-api passes every check and dies at logoff.
$apiHow = if (Get-DeskTask $API_TASK) { "scheduled task '$API_TASK'" } else { 'Start-Process (SESSION-BOUND: dies when this session ends)' }
$watchHow = if (Get-DeskTask $WATCH_TASK) { "scheduled task '$WATCH_TASK'" } else { 'py\live\start_telegram.ps1 (SESSION-BOUND)' }

if ($ServerOnly) {
    Note 'MODE: -ServerOnly'
    Note '  will stop and restart : fd-api, the telegram watch'
    Note "    fd-api via          : $apiHow"
    Note "    the watch via       : $watchHow"
    Note "  will NOT touch        : $($running.Count) executor(s), the AI traders, the pollers"
    Note "  artefacts             : $artefact"
    # Named by PROCESS and not by file. This sentence used to say "a change to
    # mt5_executor.py is on disk and not in effect", which was true of the
    # change in flight when it was written and immediately misleading when the
    # next one landed in mt5_bars.py: a reader could infer that the pollers
    # were current because only the executor was named. The pull prints the
    # actual list below, from git, so nothing here has to guess which.
    Note '  none of those three pick up this pull. They keep the Python they were'
    Note '  launched with, and anything it changes for them takes effect at their'
    Note '  next restart, which this mode does not perform.'
} elseif ($NoRestart) {
    Note 'MODE: -NoRestart'
    Note '  will stop and restart : nothing'
    Note '  will do               : pull, and build unless -SkipBuild'
} else {
    Note 'MODE: full desk'
    Note "  will STOP             : $($running.Count) executor(s), AI traders, pollers, fd-api"
    Note '  will restart          : fd-api, pollers, AI traders, the telegram watch'
    Note "    fd-api via          : $apiHow"
    Note "    the watch via       : $watchHow"
    Note '  will NOT restart      : the executors - that stays a decision, by hand'
    Note "  artefacts             : $artefact"
}

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
        # -ServerOnly names them and proceeds. It signals no executor, so no
        # position here is being orphaned BY THIS RUN - and a check that
        # refuses when it is not protecting anything teaches the operator to
        # reach for the override, which is how the override stops meaning
        # anything. Named anyway: a person deploying while the account holds
        # money should see that, safe or not.
        if ($atRisk.Count -gt 0 -and $ServerOnly) {
            Write-Host ''
            Write-Host "$($atRisk.Count) real position(s) are open. -ServerOnly does not touch their" -ForegroundColor Yellow
            Write-Host 'executors, so they stay mirrored throughout and this is NOT a refusal.' -ForegroundColor Yellow
            Write-Host 'fd-api does go down for a moment: the mirrors will log api-unreachable,' -ForegroundColor DarkGray
            Write-Host 'hold what they have and reconcile again when it answers. They do not' -ForegroundColor DarkGray
            Write-Host 'close a position on an unreadable book - that path logs and sleeps.' -ForegroundColor DarkGray
        }
        if ($atRisk.Count -gt 0 -and -not $ServerOnly -and -not $AllowOrphans) {
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

    # WHAT THIS PULL CHANGED FOR PROCESSES NOBODY IS RESTARTING, asked of git
    # rather than claimed.
    #
    # The Python under py\live\ is read at process start and never again, so a
    # pull changes those files on disk while the running executors, traders
    # and pollers go on executing what they were launched with. The plan block
    # says that as a property of the mode; this says WHICH FILES, tonight,
    # from the commit range that actually landed - so the list cannot go stale
    # and cannot be wrong about a change nobody anticipated.
    #
    # It is a report and never a refusal: deploying code that takes effect at
    # the next restart is the normal, intended shape of -ServerOnly.
    $pyChanged = @(git --no-pager diff --name-only "$before..$after" -- py/live 2>$null |
                   Where-Object { $_ -match '\.py$' })
    if ($pyChanged.Count -gt 0) {
        Write-Host ''
        Write-Host "  this pull changed $($pyChanged.Count) file(s) under py\live\:" -ForegroundColor Yellow
        foreach ($c in $pyChanged) { Write-Host "    $c" -ForegroundColor Yellow }
        # The list is from git; the mapping from file to process is NOT, and
        # is not attempted. An earlier draft printed these under "running
        # processes will not pick up" and was wrong about telegram_notify.py -
        # the watch IS restarted by -ServerOnly, so that one does take effect.
        # A file-to-process table here would be a second copy of a fact the
        # script already knows exactly, and would be wrong the first time
        # someone adds a process. So it states what it restarts and leaves the
        # reader to join them, which cannot go stale.
        Write-Host '  Python is read at process start and never again. This run restarts only' -ForegroundColor DarkGray
        Write-Host '  the processes named in the plan above; every other process keeps the code' -ForegroundColor DarkGray
        Write-Host '  it was launched with, whatever this pull changed. Restarting the rest is a' -ForegroundColor DarkGray
        Write-Host '  separate decision with its own checks - the executors especially.' -ForegroundColor DarkGray
    }
}

if ($NoRestart -and $SkipBuild) { Write-Host "`nnothing else asked for"; exit 0 }

# ------------------------------------------- the staged artefacts, BEFORE the stop
#
# Everything here needs only two things: the files that were copied over, and
# the commit the pull just landed on. Both exist by this line, so both checks
# run while the desk is still up.
#
# They used to run inside the install block, after the stop. That is the
# principle the operator stated after the first real deploy - a refusal at the
# readiness line with fd-api already down is a worse place to learn it than
# here - applied one layer in: the cheapest signal that the wrong artefact was
# staged fired with the desk already stopped, when it could fire while
# everything is still running and cost nothing but an exit.
if ($Binary -or $Client) {
    Step 'the staged artefacts, checked before anything stops'

    # VERSION, written by pack.ps1. Its `git_hash` is the TREE cargo was
    # pointed at; `binary_contains_hash` is whether the exe's own bytes
    # corroborate that. Both are reported and only the first is enforced,
    # because a false corroboration is pack.ps1's problem to have caught and
    # this is the second line of defence, not the first.
    $verFile = if ($Staged) { Join-Path $Staged 'VERSION' } `
               elseif ($Binary) { Join-Path (Split-Path $Binary -Parent) 'VERSION' } else { $null }
    if ($verFile -and (Test-Path $verFile)) {
        try { $ver = Get-Content $verFile -Raw | ConvertFrom-Json } catch { $ver = $null }
        if ($ver) {
            Note "VERSION: built from $($ver.git_hash) at $($ver.built_at_utc)"
            if ($ver.git_dirty) {
                Write-Host '   the staged binary was built from a DIRTY tree; its hash names a' -ForegroundColor Yellow
                Write-Host '   commit whose contents it does not contain.' -ForegroundColor Yellow
            }
            if ($ver.PSObject.Properties.Name -contains 'binary_contains_hash' -and -not $ver.binary_contains_hash) {
                Write-Host '   pack.ps1 could NOT find that commit string inside the exe.' -ForegroundColor Yellow
                Write-Host '   Treat the hash below as a claim rather than a fact.' -ForegroundColor Yellow
            }
            if ("$($ver.git_hash)" -ne $after) {
                Stop-Here ("the staged artefacts were built from $($ver.git_hash) and this pull " +
                           "landed on $after. Nothing has been stopped and nothing copied. " +
                           'Rebuild at the pulled commit, or pull to the commit they were built ' +
                           'from. This is the check that used to fire after the desk was down.')
            }
            Note 'VERSION matches the pulled HEAD'
        } else {
            Note "VERSION at $verFile could not be parsed; falling back to the byte scan"
        }
    } else {
        Note 'no VERSION file beside the staged artefacts (packed before pack.ps1 emitted one)'
    }

    # The byte scan, moved here from the install block. A heuristic, and it
    # says so: version.rs bakes the commit in with env!() so it sits in
    # .rodata as a plain literal, but a different toolchain or a packer could
    # hide it. It never gates - /api/version after the start is the check -
    # and it is here because it can save stopping the desk for the wrong file.
    if ($Binary) {
        try {
            $probe = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Binary))
            if ($probe.Contains($after)) {
                Note "the staged binary contains $($after.Substring(0,12)) - consistent with this commit"
            } else {
                Write-Host "   WARNING: $after does not appear in the staged binary." -ForegroundColor Yellow
                Write-Host '   It may be built from another commit. Heuristic only; /api/version decides.' -ForegroundColor Yellow
            }
        } catch {
            Note 'could not scan the staged binary for its commit; /api/version will decide'
        }
    }

    # The client, checked the same way and for the same reason: index.html
    # must reference bundles the folder actually contains, and finding that
    # out after the old dist has been rolled aside is finding it out late.
    if ($Client) {
        $stagedBundle = Assert-BundleReferenced $Client
        if (-not $stagedBundle.ok) {
            Stop-Here ("the staged client is not serveable: $($stagedBundle.why). " +
                       'Nothing has been stopped and nothing copied.')
        }
        foreach ($r in $stagedBundle.refs) { Note "staged index.html references $r, present" }
    }
}

# --------------------------------------------------------------------- stop
#
# BEFORE the build, in both restart modes, and that ordering is a staleness
# check rather than tidiness: a running .exe cannot be overwritten on Windows.
# With fd-api up, the link step fails, and in a hand-typed sequence that line
# scrolls past and the OLD binary restarts green with every downstream check
# passing. Stopping first turns a silent stale deploy into a build error.
$exe = Join-Path $Root 'target\release\fd-api.exe'
$dist = Join-Path $Root 'ui\dist'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$exeBefore = Hash-Of $exe

if (-not $NoRestart) {
    # The point of no return. From here an exit leaves the desk mid-deploy,
    # so every exit below goes through Stop-Here and names what it owed.
    if ($ServerOnly) {
        Owe @('install the staged artefacts', 'start fd-api', 'verify the running version', 'restart the telegram watch')
        Step 'stopping fd-api (and nothing else)'
        Stop-Server
        Note 'fd-api down; executors, traders and pollers untouched'
    } else {
        Owe @('install or build the artefacts', 'start fd-api', 'verify the running version',
              'restart the pollers', 'restart the AI traders', 'restart the telegram watch')
        Step 'stopping the desk'
        Stop-Desk
        Note 'executors, traders, pollers and fd-api down'
    }
}

# ------------------------------------------------------------ install: copy
#
# The copy is now a first-class path and not a fallback, because on the VPS it
# is the ONLY path: that machine has git and nothing else. `cargo build` is the
# local convenience; this is the deploy.
#
# It runs after the stop above, which is the ordering that matters - a running
# .exe cannot be overwritten on Windows, and a failed Copy-Item in a hand-typed
# sequence scrolls past while the old binary restarts green.
if ($Binary -or $Client) {
    Step 'installing the staged artefacts'
    if (Get-Process fd-api -ErrorAction SilentlyContinue) {
        Write-Error 'fd-api is still running; a copy over a running exe fails and the old one restarts green. Nothing was copied.'
        exit 1
    }
    if ($Binary) {
        $srcHash = Hash-Of $Binary
        Note "source  $Binary  sha $srcHash"
        # The commit scan and the VERSION comparison used to live here. They
        # moved ABOVE the stop, because both need only the staged files and
        # the pulled hash, and a refusal that costs nothing is worth more than
        # the same refusal after the desk is down.
        $rolled = Roll-Aside $exe $stamp
        if ($rolled) { Note "previous binary kept at $(Split-Path $rolled -Leaf)" }
        New-Item -ItemType Directory -Force -Path (Split-Path $exe -Parent) | Out-Null
        try {
            Copy-Item -Path $Binary -Destination $exe -Force -ErrorAction Stop
        } catch {
            Write-Error "copying the binary FAILED: $($_.Exception.Message). The old binary is still in place and has NOT been started."
            exit 1
        }
        # Two genuinely different files now, so this is the real check that
        # was degenerate while the build wrote in place.
        $dstHash = Hash-Of $exe
        if ($srcHash -ne $dstHash) {
            Write-Error "the copy did not land: source sha $srcHash, destination sha $dstHash. Do not start this."
            exit 1
        }
        Note "installed: destination sha $dstHash matches the source"
    }
    if ($Client) {
        $rolledDist = Roll-Aside $dist $stamp
        if ($rolledDist) { Note "previous client kept at $(Split-Path $rolledDist -Leaf)" }
        if (Test-Path $dist) {
            # The destination MUST be gone before the copy. `Copy-Item -Recurse`
            # onto an existing directory copies the source INTO it - the result
            # is ui\dist\dist, index.html is not where fd-api looks, and the
            # desk serves nothing while every step above reported success.
            #
            # And it may only be removed once the roll-aside has actually
            # produced a copy. "Never deleted" is not satisfied by deleting it
            # after a failed backup, so a failed roll-aside stops the run with
            # the old client still in place.
            if (-not $rolledDist) {
                Write-Error "could not roll the existing $dist aside, so it will not be removed. The old client is intact and nothing was installed."
                exit 1
            }
            Remove-Item -Path $dist -Recurse -Force
        }
        try {
            Copy-Item -Path $Client -Destination $dist -Recurse -Force -ErrorAction Stop
        } catch {
            Write-Error "copying the client FAILED: $($_.Exception.Message). The previous client is at $rolledDist."
            exit 1
        }
        Note "client installed from $Client"
    }
    Paid 'install the staged artefacts'
    Paid 'install or build the artefacts'
}

# -------------------------------------------------------------------- build
if (-not $SkipBuild -and -not $Binary) {
    Step 'building'
    if ($NoRestart) {
        # fd-api was not stopped, so the link will fail if it is running. Said
        # before the attempt rather than diagnosed after it.
        if (Get-Process fd-api -ErrorAction SilentlyContinue) {
            Write-Error 'fd-api is running and -NoRestart did not stop it; the link step cannot overwrite a running exe. Stop it, or drop -NoRestart.'
            exit 1
        }
    }
    $cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
    if (-not $cargo) { $cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe" }
    # THE REFUSAL, and it is the one that matters most on the server.
    #
    # No toolchain and no staged binary means there is no way to produce a
    # current one. Before today this fell through to `cargo not found` after
    # the desk was already stopped, or - under -SkipBuild - to the OLD binary
    # restarting green with every downstream check passing, which is the
    # incident the staleness note was written about.
    if (-not (Test-Path $cargo)) {
        Write-Host ''
        Write-Host 'No toolchain here and no staged binary given, so nothing can produce a' -ForegroundColor Red
        Write-Host 'current fd-api.exe. NOTHING WAS BUILT and nothing was copied.' -ForegroundColor Red
        Write-Host '  This is the normal state of the VPS: it has git and nothing else.' -ForegroundColor Yellow
        Write-Host '  Build on a machine that has cargo, copy the pair over, then:' -ForegroundColor Yellow
        Write-Host '    update.ps1 -ServerOnly -Staged C:\Windows\Temp\flowdesk-build' -ForegroundColor Yellow
        Write-Host '  or name them separately with -Binary and -Client.' -ForegroundColor Yellow
        Write-Host '  -SkipBuild restarts whatever binary is already here, deliberately.' -ForegroundColor DarkGray
        exit 1
    }
    $rolled = Roll-Aside $exe $stamp
    if ($rolled) { Note "previous binary kept at $(Split-Path $rolled -Leaf)" }
    $rolledDist = Roll-Aside $dist $stamp
    if ($rolledDist) { Note "previous client kept at $(Split-Path $rolledDist -Leaf)" }
    & $cargo build --release -p fd-api
    if ($LASTEXITCODE -ne 0) { Write-Error 'cargo build failed'; exit 1 }
    if (-not (Test-Path $exe)) { Write-Error "cargo reported success but $exe does not exist"; exit 1 }
    $exeAfter = Hash-Of $exe
    # Built in place: cargo links straight into target\release, so source and
    # destination are one file and this is before-and-after rather than the
    # two-file comparison. The two-file version is in the copy branch above,
    # which is the branch the server actually takes.
    Note "fd-api built in place: $exeBefore -> $exeAfter"
    if ($exeBefore -and $exeAfter -eq $exeBefore -and $before -ne $after) {
        # Not fatal - a pull that touched only Python or config legitimately
        # leaves the binary identical - but it must be said, because the same
        # observation with a Rust change in the pull is the stale-binary bug.
        Write-Host '   NOTE: the pull moved HEAD and the binary is byte-identical.' -ForegroundColor Yellow
        Write-Host '   Fine if the change was Python or config; wrong if it was Rust.' -ForegroundColor Yellow
    }

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

# The client is checked whether or not this run built it, because -SkipBuild is
# the short path to serving last week's bundle and the check costs nothing.
Step 'client bundle'
$bundle = Assert-BundleReferenced $dist
if (-not $bundle.ok) {
    Write-Error "ui/dist is not serveable: $($bundle.why)"
    exit 1
}
foreach ($r in $bundle.refs) { Note "index.html references $r, present" }

# A run that restarts nothing still has to say which build is answering.
#
# The note's finding 4: -SkipBuild and -NoRestart turn off the step that would
# have made the artefact current while leaving every downstream check green. So
# the version is REPORTED here even when this run will not restart anything -
# reported and not enforced, because nothing was deployed and there is nothing
# to refuse.
if ($NoRestart) {
    Step 'what is currently answering'
    $cur = Wait-ForVersion $after 3
    if ($cur.state -eq 'answered') {
        $h = "$($cur.body.git_hash)"
        $tag = if ($h -eq $after) { 'matches the tree' } else { "DOES NOT match the tree ($after)" }
        Note "fd-api is on $($h.Substring(0, [Math]::Min(12, $h.Length))) built $($cur.body.built_at_utc) - $tag"
        if ($cur.body.git_dirty) { Note 'and it was built from a dirty tree' }
    } elseif ($cur.state -eq 'missing') {
        Note 'fd-api is up but has no /api/version - it predates the version stamp'
    } else {
        Note 'fd-api is not answering; nothing was restarted, so that is how it was found'
    }
}

# -------------------------------------------------------------------- start
if (-not $NoRestart) {
    Step 'starting the desk'
    $logs = Join-Path $Root 'data\paper\logs'
    New-Item -ItemType Directory -Force -Path $logs | Out-Null

    # The task, where there is one. See the block above Stop-Server for why
    # this must not be a child process on the server.
    $viaTask = [bool](Get-DeskTask $API_TASK)
    if ($viaTask) {
        Start-ScheduledTask -TaskName $API_TASK
        Note "started scheduled task '$API_TASK'; waiting for it to report its version"
    } else {
        Start-Process -FilePath (Join-Path $Root 'target\release\fd-api.exe') -WorkingDirectory $Root -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $logs 'fd-api.out') -RedirectStandardError (Join-Path $logs 'fd-api.err')
        Note 'fd-api started as a child of this session; waiting for it to report its version'
    }

    # Readiness is the HASH, not a 200. See Wait-ForVersion.
    $v = Wait-ForVersion $after 30
    if ($v.state -eq 'missing') {
        Write-Error ("fd-api is up but has no /api/version (HTTP $($v.code)). That endpoint is " +
                     'the only check that can tell a fresh binary from a stale one, so this ' +
                     'run FAILS CLOSED rather than reporting a deploy it cannot verify. It ' +
                     'lands on main tonight; until then, verify by hand and deploy with the ' +
                     'staged sequence. fd-api IS running.')
        exit 1
    }
    if ($v.state -ne 'answered') {
        Write-Error "fd-api did not report a version within 30s ($($v.state)); read data\paper\logs\fd-api.err"
        exit 1
    }
    # Field names read from crates/fd-api/src/version.rs, not from a summary of
    # it: `git_hash`, `git_dirty`, `built_at_ms`, `built_at_utc`. GIT_HASH is a
    # full 40-character sha or the literal "unknown" - never empty, which is
    # what makes an equality test safe here.
    $got = "$($v.body.git_hash)"
    $builtAt = "$($v.body.built_at_utc)"
    if ($got -eq 'unknown') {
        Write-Error ("fd-api reports git_hash 'unknown': git was unreachable when this binary " +
                     "was compiled, so nothing can say what source it contains. Built $builtAt. " +
                     'Rebuild it somewhere git works; do not deploy a binary that cannot name itself.')
        exit 1
    }
    Note "fd-api answering, built from $($got.Substring(0, [Math]::Min(12, $got.Length))) at $builtAt"
    if ($got -ne $after) {
        # Both hashes and the build time, then the two causes, because the
        # message is the whole value of the check. And NEVER a rebuild from
        # here: a deploy that quietly fixes what it just caught destroys the
        # evidence of how the server got into this state.
        Write-Host ''
        Write-Host 'THE RUNNING BINARY IS NOT THE COMMIT THIS DEPLOY PULLED.' -ForegroundColor Red
        Write-Host "  tree HEAD after the pull : $after" -ForegroundColor Red
        Write-Host "  binary reports built from: $got" -ForegroundColor Red
        Write-Host "  binary built at          : $builtAt" -ForegroundColor Red
        Write-Host '  Two causes, and they need different answers:' -ForegroundColor Yellow
        Write-Host '   1. it was never rebuilt - cargo missing from PATH, or -SkipBuild.' -ForegroundColor Yellow
        Write-Host '      The binary on disk is whatever was there before the pull.' -ForegroundColor Yellow
        Write-Host '   2. the write failed because the old exe was still running, so the' -ForegroundColor Yellow
        Write-Host '      link could not overwrite it and the old one started again green.' -ForegroundColor Yellow
        Write-Host '  Not rebuilding from here on purpose: fixing this automatically would' -ForegroundColor DarkGray
        Write-Host '  erase which of the two happened, and that is the thing worth knowing.' -ForegroundColor DarkGray
        exit 1
    }
    # WHO is answering, not just what. A session-bound fd-api passes every
    # check above - right hash, right build time, clean startup - and dies at
    # logoff, which is the green failure this whole block exists to prevent.
    $who = Test-Detached 'fd-api'
    if (-not $who.found) {
        Note 'answering, but no fd-api process is visible to report on (it may be running as another user)'
    } else {
        Note ("answering process pid $($who.pid), session $($who.session) (this script: $($who.mySession)), " +
              "parent $($who.parent), owner $($who.owner)")
        if ($viaTask) {
            $st = (Get-DeskTask $API_TASK).State
            if ("$st" -ne 'Running') {
                Stop-Here "scheduled task '$API_TASK' reports State=$st after being started. Something is answering on 8138 but the task is not running it."
            }
            Note "scheduled task '$API_TASK' State=Running"
            if ($who.ours) {
                # Unambiguous: it is our own descendant, so it dies with this
                # script whatever its session or owner says.
                Stop-Here ("'$API_TASK' is reported Running but the answering fd-api is a DESCENDANT of " +
                           'this script, so it will die when this session ends. Something started it ' +
                           'outside the task. fd-api IS running.')
            }
            if ($who.owner -eq '?') {
                # Three states, not two. GetOwner() can simply fail - measured
                # on services.exe, which returns an empty user - and "could
                # not read it" is not "it is a user account". Saying the
                # second when the first is true is the mistake the snapshot
                # made with `demo: false`.
                Note 'could not read the process owner; the task principal is worth an eye'
            } elseif (-not $who.service) {
                # Warned, not refused, and the ssh case is why. sshd's
                # children run in session 0, so a session comparison cannot
                # separate a SYSTEM task from the caller there, and a refusal
                # on weak evidence costs the stages below it - which is
                # exactly what happened on the first real run.
                Write-Host "   WARNING: fd-api is owned by '$($who.owner)', not a service account." -ForegroundColor Yellow
                Write-Host '   A task running as the logged-on user dies at logoff and reads that' -ForegroundColor Yellow
                Write-Host '   user''s environment rather than machine scope, which decides the' -ForegroundColor Yellow
                Write-Host '   advisor gate. Check the task principal. Continuing.' -ForegroundColor Yellow
            }
        } elseif ($who.ours) {
            Write-Host '   NOTE: fd-api is a child of this session and will stop when it ends.' -ForegroundColor Yellow
            Write-Host '   There is no flowdesk-api scheduled task on this machine; that is fine' -ForegroundColor DarkGray
            Write-Host '   locally and is not how the server should run.' -ForegroundColor DarkGray
        }
    }
    Paid 'start fd-api'
    Paid 'verify the running version'

    # After the version is proven and before anything is started against it.
    # A refusal here leaves fd-api RUNNING and correct - the binary is fine,
    # its inputs are not - so this does not go through Stop-Here, which is for
    # states where the desk has been left half-stopped.
    if (-not (Test-HtfReadiness $Root)) {
        Write-Host ''
        Write-Host 'fd-api is running and is the right commit. NOT reporting ready.' -ForegroundColor Red
        Show-Owed
        exit 1
    }
    if ($v.body.git_dirty) {
        # Named, never refused, and -48 is right about why: this is the state
        # most likely to be running during an incident, and a check that
        # refuses it means the person firefighting cannot deploy. A dirty build
        # has a hash that is true and a content that is not; the deploy's job
        # is to say which state the server is in, not to have an opinion.
        Write-Host '   WARNING: built from a DIRTY tree. The hash above names a commit whose' -ForegroundColor Yellow
        Write-Host '   contents this binary does not contain. Running, and said out loud.' -ForegroundColor Yellow
    }

    # -ServerOnly restarts the watch and nothing else. The pollers and traders
    # were never stopped, so starting them here would start a SECOND copy of
    # each - the mutex in start_pollers would refuse, noisily, which is a
    # failure invented by this script rather than found by it.
    # The watch goes through its task where one exists, for the same reason
    # fd-api does: start_telegram.ps1 spawns a child of this session, and the
    # watch is on a task precisely because that did not survive.
    if (Get-DeskTask $WATCH_TASK) {
        # Scoped by command line, NEVER by 'python' alone: the executors, the
        # traders and the pollers are all python.exe on this machine.
        Stop-DeskTask $WATCH_TASK 'python' '*telegram_notify*'
        Start-ScheduledTask -TaskName $WATCH_TASK
        Start-Sleep -Seconds 2
        $wst = (Get-DeskTask $WATCH_TASK).State
        Note "scheduled task '$WATCH_TASK' restarted, State=$wst"
        if ("$wst" -ne 'Running') {
            Write-Host "   WARNING: '$WATCH_TASK' is $wst, so nothing is watching the desk." -ForegroundColor Yellow
        }
        Paid 'restart the telegram watch'
        $toRun = if ($ServerOnly) { @() } else { @('start_pollers.ps1', 'start_ai_traders.ps1') }
    } else {
        $toRun = if ($ServerOnly) { @('start_telegram.ps1') } else { @('start_pollers.ps1', 'start_ai_traders.ps1', 'start_telegram.ps1') }
    }
    foreach ($s in $toRun) {
        $p = Join-Path $Root "py\live\$s"
        if (Test-Path $p) {
            Note "running $s"
            powershell -NoProfile -ExecutionPolicy Bypass -File $p | Out-Null
        }
        switch ($s) {
            'start_pollers.ps1'    { Paid 'restart the pollers' }
            'start_ai_traders.ps1' { Paid 'restart the AI traders' }
            'start_telegram.ps1'   { Paid 'restart the telegram watch' }
        }
    }

    if ($ServerOnly) {
        $still = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" -ErrorAction SilentlyContinue |
                   Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
        Write-Host ''
        Write-Host "$($still.Count) executor(s) ran throughout and were not signalled." -ForegroundColor Green
        Write-Host 'They are still on the Python they were LAUNCHED with, not what was just' -ForegroundColor Yellow
        Write-Host 'pulled. A change to mt5_executor.py needs start_executors.ps1 to take' -ForegroundColor Yellow
        Write-Host 'effect, and that is a separate decision with its own orphan check.' -ForegroundColor Yellow
        exit 0
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
