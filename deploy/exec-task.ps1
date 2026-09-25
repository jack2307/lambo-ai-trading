# Register `flowdesk-exec-v10` and run it, so the live executors start
# DETACHED from the shell that asked.
#
# Run on demand only - no trigger. The desk's rule is "boot mot nua": the
# paper side comes back by itself after a reboot and the money side does not,
# so this task must never acquire a boot trigger. It exists so that starting
# the money side does not require sitting in RDP, not so that it happens
# without anybody deciding.
#
# THE PRINCIPAL IS THE OWNER AND NOT SYSTEM. The MT5 terminal lives in the
# owner's interactive session; SYSTEM has no path to it. `LogonType Interactive`
# matches what install-tasks.ps1 uses for the tasks that need that session.
[CmdletBinding()]
param(
    [string]$TaskName = 'flowdesk-exec-v10',
    [string]$Root = 'C:\flowdesk',
    # Register and report without running, for checking the registration on a
    # day when starting executors would be the wrong thing to do.
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'

$action = New-ScheduledTaskAction -Execute (Join-Path $Root 'deploy\run-exec-v10.cmd')

# THE IDENTITY IS ASKED FOR, NOT ASSEMBLED. `"$env:USERDOMAIN\$env:USERNAME"`
# is the obvious line and it fails here: over SSH this box reports
# USERDOMAIN=WORKGROUP, and `WORKGROUP\administrator` maps to no SID -
# `Register-ScheduledTask` answers "No mapping between account names and
# security IDs was done", which names the symptom and not the cause. The
# machine is standalone, so the domain part has to be the COMPUTER name.
# GetCurrent().Name returns exactly what the account is called, whichever
# shell is asking.
$user = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $TaskName -Action $action -Principal $principal `
    -Settings $settings -Description 'Start the REAL MONEY executors for vantage-v10, on demand' `
    -Force | Out-Null
Write-Host "registered $TaskName as $user (Interactive, on demand, no trigger)"

if ($NoRun) { Write-Host 'not run (-NoRun)'; return }

Start-ScheduledTask -TaskName $TaskName
Write-Host "started $TaskName - it launches the executors and exits; they outlive it"
