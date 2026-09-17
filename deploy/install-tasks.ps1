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
    Note "action    : cmd.exe /c `"$script`""
    Note "principal : SYSTEM, ServiceAccount, Highest"
    Note "trigger   : at startup"
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

# ---- roll the stale log aside, never append to it ----
#
# The existing data\paper\logs\fd-api.out was written by a wrapper-started
# process that has been dead for hours, and two people read it that evening as
# the live process's output. Appending the new stream to it would produce one
# file in which the top describes a process that no longer exists and nothing
# marks the join. Rolled aside with a timestamp and kept, the same rule the
# deploy uses for the binary and the client.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
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
    $action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument "/c `"$script`""
    $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
    $trigger = New-ScheduledTaskTrigger -AtStartup
    # ExecutionTimeLimit 0 because these are meant to run forever; the default
    # stops a task after three days, which is a desk that goes quiet on a
    # Wednesday for no reason anyone will connect to this setting.
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
        -StartWhenAvailable -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
    Register-ScheduledTask -TaskName $t.Name -Action $action -Principal $principal `
        -Trigger $trigger -Settings $settings -Force | Out-Null
    Write-Host "  registered $($t.Name)" -ForegroundColor Green
}

Write-Host ''
Write-Host 'Registered, and NOT started. Start them when you are ready:' -ForegroundColor Yellow
foreach ($t in $TASKS) { Write-Host "  Start-ScheduledTask -TaskName $($t.Name)" -ForegroundColor Yellow }
Write-Host 'Then read data\paper\logs\ - each start now writes a boundary line, so the' -ForegroundColor DarkGray
Write-Host 'version summary and the advisor-gate line are readable for the first time.' -ForegroundColor DarkGray
