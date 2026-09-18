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
#
#   AND THEREFORE NOT THE HTF EXPORT EITHER, which is the third task here and
#   the only one that does NOT run as SYSTEM. It talks to MetaTrader, so the
#   same rule that keeps the pollers out of session 0 keeps it out. See the
#   comment on its entry below: the principal is MEASURED from the running
#   terminal rather than named here, and this script refuses to register it
#   when it cannot measure one.
param(
    [string]$Root = '',
    # Only used to ask `py/live/accounts.py` which terminal serves prices.
    # The same default the launchers under py/live use.
    [string]$Python = 'C:\Python39\python.exe',
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

# `Principal` is 'system' or 'interactive', and `Trigger` is 'startup' or
# 'hourly'. Both were implicit while every task was the same shape; the export
# is neither, and an implicit field is how a third task quietly inherits the
# two decisions that do not apply to it.
# What a task's action is RIGHT NOW, as one comparable string.
#
# Used to answer "is this apply actually changing anything for this task",
# which decides whether its log is worth rolling aside. A task being
# re-registered with the action it already has is the common case - the
# operator is adding a third task, or re-running after a refusal - and there
# is nothing stale to roll in that case.
function Get-ActionSignature($task) {
    if (-not $task) { return $null }
    $a = @($task.Actions)[0]
    if (-not $a) { return $null }
    return (("{0}|{1}|{2}" -f $a.Execute, $a.Arguments, $a.WorkingDirectory)).Trim()
}

# Roll a log aside, and NEVER take the apply down with it.
#
# WHAT WENT WRONG, 2026-09-18 on the VPS. This ran as a loop over every task
# BEFORE any registration, and `Move-Item` on data\paper\logs\fd-api.out
# failed with "being used by another process" - because fd-api was running
# under its task and holding its own log open. `$ErrorActionPreference` is
# `Stop`, so that one failure aborted the whole apply: the XML exports had
# been written, nothing had been registered, and `flowdesk-htf-export` did not
# exist. One task's log file stopped three tasks from being defined.
#
# Two things were wrong and both are fixed here. It rolled unconditionally,
# including for tasks whose action was not changing and which therefore had
# nothing stale to roll; and it did it while the writer was still running.
# Registration with -Force stops the task, so this is now called AFTER the
# register, when the handle is gone.
#
# It still retries, because process exit and handle release are not the same
# instant, and it still returns rather than throws, because a log that could
# not be renamed is worth a line and is not worth an undefined task.
function Roll-LogAside([string]$path, [string]$stamp) {
    if (-not (Test-Path $path)) { return $true }
    foreach ($i in 1..5) {
        try {
            Move-Item -Path $path -Destination "$path.$stamp" -Force -ErrorAction Stop
            Note "rolled $(Split-Path $path -Leaf) aside as $(Split-Path "$path.$stamp" -Leaf)"
            return $true
        } catch {
            Start-Sleep -Milliseconds 400
        }
    }
    return $false
}

# THE EXPORT'S CADENCE AND ITS TIME LIMIT ARE ONE DECISION.
#
# Two adjacent numbers on purpose, because they are not independent: a run
# that can outlive its own period stops being a schedule and becomes a queue.
# `MultipleInstances IgnoreNew` then drops the starts it overlaps SILENTLY -
# the task's history shows successful runs and the gap never appears anywhere.
# So the limit must stay below the period, and the check below enforces it
# rather than trusting whoever edits these two lines next.
#
# WHY HOURLY TODAY AND NOT FIVE MINUTES. The owner asked for every timeframe
# on the chart and a5 asked for a 5-minute cadence. It cannot go there yet:
# `mt5_export.py` re-walks the FULL history on every run - nothing consults
# the stored file to decide where to resume - so six timeframes every five
# minutes is twelve full-history pulls an hour against the terminal that is
# also the price feed and also the funded account. `--days` does not help;
# it bounds how far back the walk goes, not how much the first call asks for.
#
# d1's `--since-stored` resumes from the newest stored bar with one bar of
# overlap, which makes a routine run cost minutes of data. Resolve-TaskAction
# REFUSES to register this task against a checkout that lacks it, so the
# dependency is enforced and not just described.
#
# WHEN IT LANDS AND HAS BEEN PROVEN ON THE VPS - read the first run's runtime
# out of data\paper\logs\bars-export.out rather than assuming it - change
# these two numbers together to 5 and 4. Not one of them.
$EXPORT_EVERY_MINUTES = 60
$EXPORT_LIMIT_MINUTES = 30
if ($EXPORT_LIMIT_MINUTES -ge $EXPORT_EVERY_MINUTES) {
    Write-Error ("the export's time limit ($EXPORT_LIMIT_MINUTES min) is not below its period " +
                 "($EXPORT_EVERY_MINUTES min). A run that outlives its period silently eats the " +
                 'next start under MultipleInstances=IgnoreNew, and the gap shows up nowhere.')
    exit 1
}

$TASKS = @(
    @{ Name = 'flowdesk-api';   Cmd = 'run-fd-api.cmd';   Log = 'fd-api.out';   What = 'the API and the client'
       Principal = 'system'; Trigger = 'startup' },
    @{ Name = 'flowdesk-watch'; Cmd = 'run-telegram.cmd'; Log = 'telegram.out'; What = 'the telegram watch'
       Principal = 'system'; Trigger = 'startup' },
    # THE HIGHER-TIMEFRAME BAR EXPORT, and it is the odd one out twice over.
    #
    # WHY IT EXISTS. `GET /api/paper/htf` reads data\bars\XAUUSD-4h.parquet
    # and -1d.parquet. `data\` is gitignored, so NO COMMIT CAN EVER SHIP
    # THOSE FILES - a merge that brings the route and the exporter brings no
    # bars, the route then answers "not exported yet" perfectly correctly, and
    # the deploy reports success. This task is what keeps them current;
    # deploy\update.ps1 refuses to report ready when the route says they are
    # missing for a market the desk trades.
    #
    # WHY NOT SYSTEM, which is the question that decides whether this works at
    # all. The answer is already written down twice in this repository and
    # both places say the same thing. deploy\start-desk.ps1: "Putting the
    # terminals in an RDP session and the rest under SYSTEM would be betting
    # that the IPC crosses a session boundary, which is a bet this desk does
    # not need to take." And py\ingest\mt5_export.py: "The terminal cannot be
    # launched from SSH at all - it lives in the owner's RDP session - so this
    # attaches to a terminal that is already up." What is PROVEN on this
    # machine is that the same USER reaches the terminal from a different
    # session - that is what the pollers do from the ssh session every day.
    # What is NOT proven is that a DIFFERENT user - SYSTEM, a different SID
    # entirely - reaches it at all. Registering this as SYSTEM would be a task
    # that registers cleanly and attaches to nothing, which is the shape this
    # desk keeps meeting.
    #
    # So: LogonType Interactive, as the account that owns the running
    # terminal, MEASURED at registration time rather than typed here. That is
    # the one configuration with evidence behind it - same user, and in fact
    # the same session the terminal is in. It requires the owner's session to
    # exist, which is already the operating rule for this machine
    # ("DISCONNECT the RDP session. Do not LOG OFF."), and if that session is
    # gone the desk is blind anyway and a missing export is the smaller
    # problem.
    #
    # WHY HOURLY RATHER THAN SIX FIXED TIMES. The broker's H4 candles close at
    # 01/05/09/13/17/21 UTC and this machine's clock is not UTC. An hourly
    # trigger at five past needs no knowledge of the offset; six local times
    # would point at the wrong candles the first time a timezone or a DST rule
    # moved underneath them. The export merges and is read-only, so running it
    # more often than strictly needed costs a few seconds and repairs any hour
    # that was missed.
    @{ Name = 'flowdesk-bars-export'; Cmd = 'run-bars-export.cmd'; Log = 'bars-export.out'
       What = 'the M1-D1 bar export the chart and the htf route read'
       Principal = 'interactive'; Trigger = 'hourly'; NeedsPrices = $true
       Timeframes = 'M1,M5,M15,H1,H4,D1'; BaseSymbol = 'XAUUSD' }
)

# WHICH terminal the desk reads bars from, per `config/accounts.toml`.
#
# Asked rather than hard-coded, and through `py/live/accounts.py --prices`
# rather than by parsing the TOML here - PowerShell has no TOML reader and a
# second parser would be a second answer to the same question. d1's key,
# landed 2026-09-18.
#
# Returns $null rather than refusing when the reader cannot answer, because
# this is only used to BREAK A TIE between two terminals running as the same
# user. The tie-break is a nicety; the owner measurement below is the thing
# that matters, and it must not start depending on a Python call that could
# fail on a machine where the tasks still need registering.
function Get-Prices([string]$root, [string]$python) {
    $reader = Join-Path $root 'py\live\accounts.py'
    if (-not (Test-Path $reader)) { return $null }
    try {
        $out = & $python $reader '--prices' 2>&1
        if ($LASTEXITCODE -ne 0) { return $null }
        $doc = ([string]::Join("`n", @($out))) | ConvertFrom-Json
        if ($doc.terminal) {
            # `symbol_suffix` is legitimately empty on a standard account, so
            # an absent one is a real answer and only `terminal` refuses.
            return @{ Terminal = [string]$doc.terminal; Suffix = [string]$doc.symbol_suffix }
        }
    } catch { }
    return $null
}

# A task's action arguments, and the reason it cannot have any.
#
# ONE function, called by the dry run AND by -Apply, because the first draft
# had the print and the registration compute the action separately: the dry
# run showed `cmd.exe /c "run-bars-export.cmd"` with no arguments while the
# apply would have registered it with three, and the precondition refusals
# below were invisible until you ran -Apply. A preview that does not preview
# the refusal is worse than no preview, because it is read as an all-clear.
#
# Returns @{ Args; Note; Refusal }. `Refusal` non-null means this task cannot
# be registered on this machine right now, and says why in a sentence meant
# to be printed.
function Resolve-TaskAction($t, [string]$root, [string]$python) {
    $script = Join-Path $root "deploy\$($t.Cmd)"
    $plain = "/c `"$script`""
    if (-not $t.NeedsPrices) { return @{ Args = $plain; Note = $null; Refusal = $null } }

    $pr = Get-Prices $root $python
    if (-not $pr) {
        return @{ Args = $plain; Note = $null; Refusal =
            'could not read [prices] from config\accounts.toml, so the terminal and symbol ' +
            'to export from are unknown. Guessing a terminal on a machine with two is how ' +
            "one account's contract ends up in the other account's file." }
    }

    # THE EXPORTER MUST BE ABLE TO RESUME, or this must not go on a clock.
    # Without `--since-stored` every run walks the FULL history: tolerable
    # hourly for two timeframes, and not tolerable on any cadence for six,
    # against the terminal that also carries the funded account. Checked
    # against the exporter's own --help rather than a commit hash, because
    # the question is what THIS checkout can do.
    $exporter = Join-Path $root 'py\ingest\mt5_export.py'
    if (-not (Test-Path $exporter)) {
        return @{ Args = $plain; Note = $null; Refusal = "missing $exporter" }
    }
    $help = & $python $exporter '--help' 2>&1
    if (-not (($help -join "`n") -match '--since-stored')) {
        return @{ Args = $plain; Note = $null; Refusal =
            'py\ingest\mt5_export.py has no --since-stored in this checkout, so every run ' +
            'would walk the whole history for every timeframe against the terminal that ' +
            'carries the funded account. Merge the exporter change first.' }
    }

    $sym = "$($t.BaseSymbol)$($pr.Suffix)"
    # THE WHOLE COMMAND IS WRAPPED IN ONE MORE PAIR OF QUOTES, and this is not
    # decoration. MEASURED 2026-09-18, and it was two live defects at once.
    #
    # Task Scheduler hands `Arguments` to cmd.exe as ONE string. cmd's rule
    # for `/c` is that when the remainder begins with a quote it strips the
    # FIRST and LAST quote characters and runs what is left - so
    #
    #     /c "script.cmd" "A" "B" "M1,M5"
    #
    # became  script.cmd" "A" "B" "M1,M5  and cmd answered "The filename,
    # directory name, or volume label syntax is incorrect", exit 1, in about
    # five seconds, having never started the wrapper. NO LOG AT ALL, because
    # the thing that writes the log was never reached. That was a5's second
    # defect, and its cause is this one.
    #
    # The same mangling shifted the quote parity further along the line, which
    # is why the hand-run - quoted correctly by a shell - still delivered only
    # `M1` as %3: cmd's numbered parameters split on COMMAS as well as spaces
    # once the quoting around them is gone. That was a5's first defect, and
    # its cause is also this one.
    #
    # Wrapping the whole remainder in another pair gives cmd something
    # harmless to strip and leaves the inner quotes intact. Verified on this
    # machine with a stub that echoes its argv: %1, %2 and %3 arrive whole,
    # commas included.
    $inner = "`"$script`" `"$($pr.Terminal)`" `"$sym`" `"$($t.Timeframes)`""
    return @{
        Args    = "/c `"$inner`""
        Note    = "exports $sym [$($t.Timeframes)] from $($pr.Terminal)"
        Refusal = $null
    }
}

# Tasks this file used to define and no longer does. `-Apply` exports each
# one's XML into the same backup folder as the replacements and then removes
# it, so a rename is reversible exactly like an edit is.
#
# `flowdesk-htf-export` became `flowdesk-bars-export` on 2026-09-18 when the
# export stopped being about the higher timeframes - the owner asked for every
# timeframe on the chart, so it pulls M1 through D1 and "htf" named only the
# two it started with. Leaving the old one registered would run the old
# wrapper hourly beside the new one: two processes attaching to the same
# terminal, writing the same files, on two schedules.
$RETIRED = @(
    @{ Name = 'flowdesk-htf-export'; Why = 'renamed to flowdesk-bars-export (M1-D1, not just H4/D1)' }
)

# WHO owns the MetaTrader terminal, measured rather than assumed.
#
# Named here instead of in the table because a principal typed into a file is
# a claim about a machine, and this file is read on more than one. If no
# terminal is running there is nothing to attach to and nothing to measure, so
# this returns $null and the caller REFUSES to register the export rather than
# guessing at a username.
function Get-TerminalPrincipal {
    # EVERY terminal, not the first one found. This machine runs two - the
    # cent terminal the pollers and the export read, and a demo terminal - and
    # taking whichever CIM listed first would make the principal depend on
    # enumeration order. Measured 2026-09-18: both are session 2 under one
    # owner, so they agree and the answer is the same either way. If they ever
    # DO disagree, that is a machine whose terminals run as different users
    # and there is no single right answer to guess at, so this returns nothing
    # and the caller refuses.
    $found = @()
    foreach ($proc in @(Get-CimInstance Win32_Process -Filter "name='terminal64.exe'" -ErrorAction SilentlyContinue)) {
        try {
            $o = Invoke-CimMethod -InputObject $proc -MethodName GetOwner -ErrorAction Stop
        } catch { continue }
        if ($o.ReturnValue -ne 0 -or -not $o.User) { continue }
        $who = if ($o.Domain) { "$($o.Domain)\$($o.User)" } else { "$($o.User)" }
        $found += @{ User = $who; Pid = $proc.ProcessId; Session = $proc.SessionId; Path = $proc.ExecutablePath }
    }
    if ($found.Count -eq 0) { return $null }
    $owners = @($found | ForEach-Object { $_.User } | Sort-Object -Unique)
    if ($owners.Count -gt 1) {
        Write-Host "   two terminals run as different users ($($owners -join ', ')); cannot choose one" -ForegroundColor Red
        return $null
    }
    # One owner, so any of them gives the right answer - but REPORT the one
    # the export actually talks to, which is the terminal `[prices]` names.
    # On the VPS on 2026-09-18 this picked the demo terminal by enumeration
    # order and was correct by luck: both run as the same user. Enumeration
    # order is not a reason, and a printed principal that names a different
    # terminal than the task will attach to is a line an operator has to
    # reconcile.
    $p = Get-Prices $Root $Python
    $preferred = if ($p) { $p.Terminal } else { $null }
    if ($preferred) {
        $match = @($found | Where-Object { $_.Path -and ($_.Path -ieq $preferred) })
        if ($match.Count) { return $match[0] }
        Write-Host "   note: [prices] names $preferred, which is not running; reporting another terminal's owner (same user)" -ForegroundColor Yellow
    }
    return $found[0]
}

Write-Host ''
Write-Host "flowdesk scheduled tasks, root $Root" -ForegroundColor Cyan
foreach ($t in $TASKS) {
    $script = Join-Path $Root "deploy\$($t.Cmd)"
    $exists = $null
    try { $exists = Get-ScheduledTask -TaskName $t.Name -ErrorAction Stop } catch { }
    Write-Host ''
    Write-Host "  $($t.Name) - $($t.What)" -ForegroundColor White
    $res = Resolve-TaskAction $t $Root $Python
    Note "action    : cmd.exe $($res.Args)   (workdir $Root)"
    if ($res.Note) { Note "exports   : $($res.Note)" }
    if ($res.Refusal) {
        Write-Host "   WOULD REFUSE: $($res.Refusal)" -ForegroundColor Red
    }
    if ($t.Principal -eq 'interactive') {
        $own = Get-TerminalPrincipal
        if ($own) {
            Note "principal : $($own.User), Interactive, Highest  (measured from terminal64.exe pid $($own.Pid), session $($own.Session))"
            Note "            $($own.Path)"
        } else {
            Write-Host '   principal : CANNOT BE DETERMINED - no terminal64.exe is running.' -ForegroundColor Red
            Write-Host '               This task attaches to MetaTrader over local IPC and must run' -ForegroundColor Red
            Write-Host '               as the account that owns it. Start the terminal in the RDP' -ForegroundColor Red
            Write-Host '               session first; -Apply will refuse this task until then.' -ForegroundColor Red
        }
        Note "trigger   : every $EXPORT_EVERY_MINUTES min at five past, from the next such minute"
        Note "settings  : ExecutionTimeLimit=${EXPORT_LIMIT_MINUTES}m, MultipleInstances=IgnoreNew, StartWhenAvailable"
    } else {
        Note "principal : SYSTEM, ServiceAccount, Highest"
        Note "trigger   : at startup"
        Note "settings  : RestartCount=999 every 1m, ExecutionTimeLimit=0 (none), MultipleInstances=IgnoreNew"
    }
    Note "stdout    : appended to data\paper\logs\$($t.Log), with a boundary line per start"
    if ($exists) { Note "currently : present, State=$($exists.State)" } else { Note 'currently : NOT REGISTERED' }
    if (-not (Test-Path $script)) { Write-Host "   MISSING   : $script" -ForegroundColor Red }
}

foreach ($r in $RETIRED) {
    $old = Get-DeskTaskSafe $r.Name
    if ($old) {
        Write-Host ''
        Write-Host "  $($r.Name) - WILL BE REMOVED" -ForegroundColor Yellow
        Note "reason    : $($r.Why)"
        Note "currently : present, State=$($old.State)"
        Note 'its XML is exported to the backup folder first, so this is reversible'
    }
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

# ---- tasks this file no longer defines ----
#
# Backed up the same way a replacement is and then removed. A rename that
# left the old task registered would run the old wrapper on its own schedule
# beside the new one: two processes attaching to the same terminal and
# writing the same files, which is worse than either alone and would look
# like the export working.
# NOT `$retired`. PowerShell variable names are CASE-INSENSITIVE, so
# `$retired = @()` assigned to the very `$RETIRED` this loop then iterates -
# emptying it first and running the body ZERO times, silently, with no line
# printed and no error. `flowdesk-htf-export` was still registered after
# -Apply on the VPS for exactly that reason, and would have kept running
# hourly beside its own replacement: two writers on the same files.
$removedTasks = @()
foreach ($r in $RETIRED) {
    $old = Get-DeskTaskSafe $r.Name
    if (-not $old) { continue }
    New-Item -ItemType Directory -Force -Path $backup | Out-Null
    $xmlPath = Join-Path $backup "$($r.Name).xml"
    try {
        Export-ScheduledTask -TaskName $r.Name | Out-File $xmlPath -Encoding utf8 -ErrorAction Stop
    } catch {
        # Not fatal, but not silent either: removing a definition that was
        # never captured is the one step here with no way back.
        Write-Host "  NOT removing $($r.Name): its XML could not be exported ($($_.Exception.Message))." -ForegroundColor Red
        Write-Host '  Refusing to delete a definition that cannot be restored.' -ForegroundColor Red
        $skipped += "$($r.Name) (could not export its XML, so it was left registered)"
        continue
    }
    try {
        Unregister-ScheduledTask -TaskName $r.Name -Confirm:$false -ErrorAction Stop
        Write-Host "  removed $($r.Name) - $($r.Why)" -ForegroundColor Yellow
        $removedTasks += $r.Name
    } catch {
        Write-Host "  could not remove $($r.Name): $($_.Exception.Message)" -ForegroundColor Red
        $skipped += "$($r.Name) (could not be unregistered)"
    }
}

# ---- what gets registered, what gets its log rolled, and what could not ----
#
# THE LOG IS ROLLED AFTER THE REGISTER, NOT BEFORE, and only when this apply
# actually changes that task's action. See Roll-LogAside for the incident that
# taught both halves. The original reason for rolling at all still stands: the
# first fd-api.out was written by a process that had been dead for hours and
# two people read it as live output. But that was a transition from an action
# with no redirection to one with it - a task whose action is unchanged is
# already writing a log with a boundary line per start, and rolling it would
# discard readable history to solve a problem that no longer exists.
$registered = @()
$skipped = @()
$unrolled = @()
foreach ($t in $TASKS) {
    $script = Join-Path $Root "deploy\$($t.Cmd)"
    if (-not (Test-Path $script)) {
        # NOT Write-Error. `$ErrorActionPreference` is `Stop`, so Write-Error
        # terminates the script and the `continue` that used to follow it was
        # unreachable - one missing .cmd would have aborted the whole apply,
        # the same shape as the log-handle failure this function was written
        # for. Named, skipped, reported at the end.
        Write-Host "  NOT registering $($t.Name): missing $script" -ForegroundColor Red
        $skipped += "$($t.Name) (missing $($t.Cmd))"
        continue
    }
    # The same call the dry run made, so what was previewed is what is
    # registered - arguments, refusals and all.
    $res = Resolve-TaskAction $t $Root $Python
    if ($res.Refusal) {
        Write-Host "  NOT registering $($t.Name): $($res.Refusal)" -ForegroundColor Red
        $skipped += "$($t.Name) ($($res.Refusal.Split('.')[0]))"
        continue
    }
    $cmdArgs = $res.Args
    if ($res.Note) { Note "$($t.Name) $($res.Note)" }
    $newSig = (("{0}|{1}|{2}" -f 'cmd.exe', $cmdArgs, $Root)).Trim()
    $action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument $cmdArgs -WorkingDirectory $Root
    # Read BEFORE the register replaces it. If the action is unchanged there
    # is nothing stale in that task's log and it is left alone.
    $wasRunning = $null -ne (Get-DeskTaskSafe $t.Name)
    $oldSig = Get-ActionSignature (Get-DeskTaskSafe $t.Name)
    if ($t.Principal -eq 'interactive') {
        $own = Get-TerminalPrincipal
        if (-not $own) {
            # REFUSED, not defaulted. Falling back to SYSTEM here would
            # register a task that runs every hour and attaches to nothing,
            # and its Last Run Result would read 0x0 while the desk had no
            # bars - the exact green failure the rest of this file exists to
            # prevent.
            Write-Host "  NOT registering $($t.Name): no terminal64.exe is running, so the account" -ForegroundColor Red
            Write-Host '  it must run as cannot be measured. Start the demo terminal in the RDP' -ForegroundColor Red
            Write-Host '  session and run this again; the other tasks above are unaffected.' -ForegroundColor Red
            $skipped += "$($t.Name) (no terminal64.exe running, so its account could not be measured)"
            continue
        }
        $principal = New-ScheduledTaskPrincipal -UserId $own.User -LogonType Interactive -RunLevel Highest
        # Five past the hour, every hour, starting at the next one. An
        # explicit finite duration rather than [TimeSpan]::MaxValue: the
        # object builds either way, but MaxValue serialises to
        # P99999999DT23H59M59S and is the kind of value a scheduler service
        # is entitled to reject at registration. Ten years is not indefinite
        # and is long enough that the machine will be gone first.
        $start = (Get-Date).Date.AddHours((Get-Date).Hour).AddHours(1).AddMinutes(5)
        $trigger = New-ScheduledTaskTrigger -Once -At $start `
            -RepetitionInterval (New-TimeSpan -Minutes $EXPORT_EVERY_MINUTES) `
            -RepetitionDuration ([TimeSpan]::FromDays(3650))
    } else {
        $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
        $trigger = New-ScheduledTaskTrigger -AtStartup
    }
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
    if ($t.Principal -eq 'interactive') {
        # Deliberately NOT the settings above. RestartCount 999 every minute
        # is right for a server that must never stay down and wrong for an
        # hourly batch job: a terminal that is not up would be retried a
        # thousand times an hour, filling the log with the same failure. It
        # runs, it succeeds or it does not, and the next hour tries again.
        # ExecutionTimeLimit is 30 minutes rather than none, because an export
        # that hangs on a terminal that stopped answering should die before
        # the next one is due.
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
            -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Minutes $EXPORT_LIMIT_MINUTES) `
            -MultipleInstances IgnoreNew
    } else {
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
            -StartWhenAvailable -ExecutionTimeLimit ([TimeSpan]::Zero) `
            -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) `
            -MultipleInstances IgnoreNew
    }
    Register-ScheduledTask -TaskName $t.Name -Action $action -Principal $principal `
        -Trigger $trigger -Settings $settings -Force | Out-Null
    Write-Host "  registered $($t.Name)" -ForegroundColor Green
    $registered += $t.Name

    # NOW the log, with the task stopped by the register above and its writer
    # gone. Only when the action changed, or when there was no task before -
    # an unchanged action is already writing a log with boundary lines, and
    # rolling it would throw away readable history for nothing.
    $log = Join-Path $Root "data\paper\logs\$($t.Log)"
    if ((-not $wasRunning) -or ($oldSig -ne $newSig)) {
        if (-not (Roll-LogAside $log $stamp)) {
            Write-Host "   could not roll $($t.Log) aside - something still holds it open." -ForegroundColor Yellow
            Write-Host '   The task IS registered. The new stream will APPEND, so the boundary' -ForegroundColor Yellow
            Write-Host '   line is what separates this process output from the last one.' -ForegroundColor Yellow
            $unrolled += $t.Log
        }
    } else {
        Note "$($t.Log) left alone: this task's action is unchanged, so nothing in it is stale"
    }
}

# SAID BEFORE THE INSTRUCTIONS, because the instructions assume every task
# exists. An apply that registered two of three used to look exactly like one
# that registered three: the loop printed a green line per success and the
# failure had already taken the script down.
Write-Host ''
if ($skipped.Count) {
    Write-Host "$($skipped.Count) task(s) were NOT registered:" -ForegroundColor Red
    foreach ($m in $skipped) { Write-Host "  $m" -ForegroundColor Red }
    Write-Host '  Fix the cause and run -Apply again. Re-registering the others is' -ForegroundColor DarkGray
    Write-Host '  harmless: their actions will be unchanged, so their logs are left alone.' -ForegroundColor DarkGray
}
if ($removedTasks.Count) {
    Write-Host "$($removedTasks.Count) task(s) were REMOVED: $($removedTasks -join ', ')" -ForegroundColor Yellow
    foreach ($n in $removedTasks) {
        Write-Host ("  put it back with: Register-ScheduledTask -TaskName $n -Xml (Get-Content '" +
                    (Join-Path $backup "$n.xml") + "' -Raw) -Force") -ForegroundColor DarkGray
    }
}
if ($unrolled.Count) {
    Write-Host "$($unrolled.Count) log(s) could not be rolled aside: $($unrolled -join ', ')" -ForegroundColor Yellow
    Write-Host '  Their tasks are registered. Read from the newest boundary line down.' -ForegroundColor DarkGray
}
if ($registered.Count -eq 0) {
    Write-Host 'NOTHING was registered.' -ForegroundColor Red
    exit 1
}

Write-Host ''
Write-Host "Registered $($registered.Count) task(s), and NOT started. Start them when you are ready:" -ForegroundColor Yellow
foreach ($n in $registered) { Write-Host "  Start-ScheduledTask -TaskName $n" -ForegroundColor Yellow }
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

# A PARTIAL APPLY IS NOT A SUCCESS, and the exit code has to say so now that
# one is possible. Before this, any failure took the script down and the exit
# code was right by accident; now the failures are survivable and reported, so
# the code has to be set deliberately or an operator with two of three tasks
# registered gets a clean 0.
if ($skipped.Count) { exit 1 }
exit 0
