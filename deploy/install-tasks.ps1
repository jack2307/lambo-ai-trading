# The two scheduled tasks that keep the desk's server half alive, in the
# repository instead of in somebody's memory.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\install-tasks.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\install-tasks.ps1 -Apply
#
# Prints what it would do; -Apply registers. Same shape as everything else on
# this desk: a file describes, a word on the command line spends.
#
# WHY THIS FILE EXISTS. flowdesk-api and flowdesk-watch were created by hand on
# 2026-09-17 and existed nowhere else. That is two ways to lose a desk: nobody
# can review a definition that is not written down, and rebuilding the server
# means reconstructing it from memory. It also hid a real defect for a day -
# see the redirection note below.
#
# WHAT SHAPE THEY MUST KEEP, and each of these was paid for:
#
#   SYSTEM, LogonType ServiceAccount. A task running as the logged-on user
#   dies at logoff, and reads THAT USER's environment rather than machine
#   scope - which is what decides the advisor gate. deploy\update.ps1 warns
#   when it finds fd-api owned by anything else.
#
#   No console. A PowerShell wrapper owns one, and a console under a SYSTEM
#   task with no desktop is how fd-api died the first time this was set up.
#   The actions are cmd /c against the two .cmd files beside this one.
#
#   NOT MetaTrader, and not the pollers or the mirrors. Those need an
#   interactive session with a desktop - see the long argument at the top of
#   deploy\start-desk.ps1. Only fd-api and the watch belong under SYSTEM.
param(
    [string]$Root = '',
    # Register the tasks. Without it this prints the definitions and changes
    # nothing.
    [switch]$Apply
)

$ErrorActionPreference = 'Stop'

if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')

function Note($what) { Write-Host "   $what" -ForegroundColor DarkGray }

function Get-DeskTaskSafe([string]$name) {
    if (-not (Get-Command Get-ScheduledTask -ErrorAction SilentlyContinue)) { return $null }
    try { return Get-ScheduledTask -TaskName $name -ErrorAction Stop } catch { return $null }
}

$TASKS = @(
    @{ Name = 'flowdesk-api';   Cmd = 'run-fd-api.cmd';   Log = 'fd-api.out';   What = 'the API and the client' },
    @{ Name = 'flowdesk-watch'; Cmd = 'run-telegram.cmd'; Log = 'telegram.out'; What = 'the telegram watch' }
)

Write-Host ''
Write-Host "flowdesk scheduled tasks, root $Root" -ForegroundColor Cyan
foreach ($t in $TASKS) {
    $script = Join-Path $Root "deploy\$($t.Cmd)"
    $exists = $null
    try { $exists = Get-ScheduledTask -TaskName $t.Name -ErrorAction Stop } catch { }
    Write-Host ''
    Write-Host "  $($t.Name) - $($t.What)" -ForegroundColor White
    Note "action    : cmd.exe /c `"$script`"   (workdir $Root)"
    Note "principal : SYSTEM, ServiceAccount, Highest"
    Note "trigger   : at startup"
    Note "settings  : RestartCount=999 every 1m, ExecutionTimeLimit=0 (none), MultipleInstances=IgnoreNew"
    Note "stdout    : appended to data\paper\logs\$($t.Log), with a boundary line per start"
    if ($exists) { Note "currently : present, State=$($exists.State)" } else { Note 'currently : NOT REGISTERED' }
    if (-not (Test-Path $script)) { Write-Host "   MISSING   : $script" -ForegroundColor Red }
}

if (-not $Apply) {
    Write-Host ''
    Write-Host 'Nothing changed. Add -Apply to register these.' -ForegroundColor Yellow
    Write-Host 'Registering REPLACES the running definition and stops the task, so do it' -ForegroundColor Yellow
    Write-Host 'when the desk can lose its API for a moment - not while you are mid-deploy.' -ForegroundColor Yellow
    Write-Host ''
    Write-Host 'BEFORE YOU DO: FD_ADVISOR_PANEL MUST NOT BE SET on this machine until these' -ForegroundColor Yellow
    Write-Host 'tasks capture stdout. The advisor setup code exists only as a printed line,' -ForegroundColor Yellow
    Write-Host 'so turning the panel on enables all four routes with the only constructive' -ForegroundColor Yellow
    Write-Host 'one unusable - which is the exact state the gate exists to prevent, and it' -ForegroundColor Yellow
    Write-Host 'is invisible while the print goes nowhere. Capture first, then decide.' -ForegroundColor Yellow
    exit 0
}

# One stamp for the whole apply, set before anything uses it: the exported
# task XML and the rolled-aside logs then carry the SAME timestamp, so a
# rollback and the logs it belongs with are findable together.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'

# ---- export what is there BEFORE replacing it ----
#
# The rollback has to restore what was RUNNING, not what someone wrote down.
# The definitions on the server were read out by hand and reconciled into this
# file field by field, and that reconciliation found one value wrong
# (RestartCount 3 against the live 999) - which is exactly the reason not to
# trust a reconstruction as a rollback. `Export-ScheduledTask` returns the
# task's own XML, including any field nobody thought to check.
#
# Written before anything is replaced, named with the same timestamp as the
# rolled-aside logs, and the exact restore command is printed at the end.
$backup = Join-Path $Root "deploy\task-backup-$stamp"
$restorable = @()
foreach ($t in $TASKS) {
    if (-not (Get-DeskTaskSafe $t.Name)) { continue }
    New-Item -ItemType Directory -Force -Path $backup | Out-Null
    $xmlPath = Join-Path $backup "$($t.Name).xml"
    try {
        Export-ScheduledTask -TaskName $t.Name | Out-File $xmlPath -Encoding utf8 -ErrorAction Stop
        $restorable += $t.Name
        Note "exported the live definition of $($t.Name) to $(Split-Path $xmlPath -Leaf)"
    } catch {
        Write-Error ("could not export $($t.Name): $($_.Exception.Message). " +
                     'Refusing to replace a definition that cannot be restored.')
        exit 1
    }
}

# ---- roll the stale log aside, never append to it ----
#
# The existing data\paper\logs\fd-api.out was written by a wrapper-started
# process that has been dead for hours, and two people read it that evening as
# the live process's output. Appending the new stream to it would produce one
# file in which the top describes a process that no longer exists and nothing
# marks the join. Rolled aside with a timestamp and kept, the same rule the
# deploy uses for the binary and the client.
foreach ($t in $TASKS) {
    $log = Join-Path $Root "data\paper\logs\$($t.Log)"
    if (Test-Path $log) {
        $dest = "$log.$stamp"
        Move-Item -Path $log -Destination $dest -Force
        Note "rolled $($t.Log) aside as $(Split-Path $dest -Leaf)"
    }
}

foreach ($t in $TASKS) {
    $script = Join-Path $Root "deploy\$($t.Cmd)"
    if (-not (Test-Path $script)) { Write-Error "missing $script; not registering $($t.Name)"; continue }
    $action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument "/c `"$script`"" -WorkingDirectory $Root
    $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
    $trigger = New-ScheduledTaskTrigger -AtStartup
    # Every one of these is taken from the definitions running on the VPS,
    # read out on 2026-09-17 so this file reconciles with them rather than
    # overwriting them. The three that are load-bearing, in the operator's
    # words, are the first three:
    #
    #   RestartCount 999 / RestartInterval 1m - this is what has brought
    #     fd-api back after every death since the move to SYSTEM tasks. A
    #     first draft of this file had 3, which is a desk that stays down on
    #     the fourth crash of a bad afternoon and nobody knowing why.
    #   ExecutionTimeLimit 0 - the default stops a task after three days,
    #     which is a server that goes quiet on a Wednesday for a reason
    #     nobody will connect to this setting.
    #   MultipleInstances IgnoreNew - set EXPLICITLY rather than left to the
    #     platform default, because a definition kept in a repository is
    #     worth nothing if it only describes the settings someone bothered to
    #     name. A second instance would be a second process on port 8138.
    #
    # The battery settings look irrelevant on a rented server and are kept
    # because they are what is running; a reconciliation that quietly drops
    # fields is not a reconciliation.
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
        -StartWhenAvailable -ExecutionTimeLimit ([TimeSpan]::Zero) `
        -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) `
        -MultipleInstances IgnoreNew
    Register-ScheduledTask -TaskName $t.Name -Action $action -Principal $principal `
        -Trigger $trigger -Settings $settings -Force | Out-Null
    Write-Host "  registered $($t.Name)" -ForegroundColor Green
}

Write-Host ''
Write-Host 'Registered, and NOT started. Start them when you are ready:' -ForegroundColor Yellow
foreach ($t in $TASKS) { Write-Host "  Start-ScheduledTask -TaskName $($t.Name)" -ForegroundColor Yellow }
Write-Host ''
Write-Host 'PROVE IT TOOK - two lines, and read both:' -ForegroundColor Cyan
Write-Host '  Get-ScheduledTask flowdesk-api,flowdesk-watch | Select TaskName,State; Get-CimInstance Win32_Process -Filter "name=''fd-api.exe''" | Select ProcessId,SessionId,@{n=''Owner'';e={(Invoke-CimMethod $_ -MethodName GetOwner).User}}'
Write-Host '  Get-Content data\paper\logs\fd-api.out -Tail 6; (Invoke-WebRequest http://127.0.0.1:8138/api/version -UseBasicParsing).Content'
Write-Host ''
Write-Host '  The first must read State=Running and SessionId=0 with Owner=SYSTEM. A' -ForegroundColor DarkGray
Write-Host '  non-zero session is the task running as the logged-on user: it dies at' -ForegroundColor DarkGray
Write-Host '  logoff and reads the wrong environment for the advisor gate.' -ForegroundColor DarkGray
Write-Host '  The second must show a boundary line dated NOW, then the version summary,' -ForegroundColor DarkGray
Write-Host '  and /api/version must answer the commit you just installed.' -ForegroundColor DarkGray
Write-Host ''
Write-Host '  DO NOT judge fd-api by whether its log GROWS. It writes on events -' -ForegroundColor DarkGray
Write-Host '  startup, reloads, errors - so a healthy API is silent for hours at a' -ForegroundColor DarkGray
Write-Host '  time. An earlier draft of this file said "Length must grow" for it, and' -ForegroundColor DarkGray
Write-Host '  that sentence would have had an operator roll back a working server for' -ForegroundColor DarkGray
Write-Host '  being quiet - the dead-file defect and a quiet server look identical in' -ForegroundColor DarkGray
Write-Host '  Length alone, which is why the boundary line and /api/version are the' -ForegroundColor DarkGray
Write-Host '  test instead. (Reported from the VPS by a5, 2026-09-18.)' -ForegroundColor DarkGray
Write-Host '  GROWTH IS the test for flowdesk-watch and for the executors, which log' -ForegroundColor DarkGray
Write-Host '  on a clock: read telegram.out twice a minute apart and it must gain' -ForegroundColor DarkGray
Write-Host '  lines. A file that exists and never grows THERE is the old defect.' -ForegroundColor DarkGray
if ($restorable.Count -gt 0) {
    Write-Host ''
    Write-Host 'ROLLBACK, if the new action does not start. One command:' -ForegroundColor Yellow
    foreach ($n in $restorable) {
        Write-Host ("  Register-ScheduledTask -TaskName $n -Xml (Get-Content '" +
                    (Join-Path $backup "$n.xml") + "' -Raw) -Force") -ForegroundColor Yellow
    }
    Write-Host '  That restores the task EXACTLY as it was, from its own exported XML -' -ForegroundColor DarkGray
    Write-Host '  not from this file, and not from values anyone wrote down. Then' -ForegroundColor DarkGray
    Write-Host '  Start-ScheduledTask it. The logs rolled aside keep their timestamps.' -ForegroundColor DarkGray
}
