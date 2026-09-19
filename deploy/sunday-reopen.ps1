# The Sunday reopen: look at the account, then - only if told to - restart the
# mirrors so the weekend backstop exists.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\sunday-reopen.ps1
#   powershell ... -File deploy\sunday-reopen.ps1 -Restart
#   powershell ... -File deploy\sunday-reopen.ps1 -Restart -AllowStaleFeed
#
# READ docs\decisions\2026-09-19-weekend-flat-never-fires.md FIRST. This script
# is the second half of that note and makes no sense without it. In short:
#
#   layer one   `flat_before_weekend_hhmm = 1640` in the engine. LIVE since
#               2026-09-19 11:32Z, in force across 27 runs, no restart needed.
#               It is bar-driven, and the whole finding is that a bar-driven
#               rule can fail silently - on 2026-09-18 the bar it needed never
#               arrived, because MetaTrader closes a bar only on a tick after
#               its boundary and the weekly close sends none.
#   layer two   `--weekend-flat HH:MM` in mt5_executor.py, default 20:45 UTC,
#               judged on the executor's own wall clock. Merged (d4a5066) and
#               NOT RUNNING: the five executors on the funded cent account
#               33705331 are still on the Python they were launched with. It
#               begins to exist at the next restart and not before.
#
# THE RESTART WAS DEFERRED ON PURPOSE, and this script is what un-defers it.
# Run on a shut market the backstop would try to close the two open positions
# every poll, fail every poll, and retry for two days. So the restart waits
# until after the Sunday reopen at 21:00 UTC - and "after 21:00 UTC" is a
# clock, which is not enough on its own. The reopen slips. A holiday Sunday
# has a clock and no tape. So this asks the feed as well, and refuses on
# either.
#
# WHAT IT DOES, in order, and it stops at step 3 unless -Restart is given:
#
#   1  the clock       is the executor's own weekend window in force right
#                      now? Same rule, same arithmetic as `in_weekend_window`.
#   2  the feed        has a quote actually arrived, and how long ago?
#   3  the account     what is open, in whose book, worth what - in the
#                      ACCOUNT's currency, which on 33705331 is USC and not
#                      USD - and what the weekend gap just did to it.
#   4  the restart     `start_executors.ps1 -Account <id> -AllowReal -Live`,
#                      detached, behind -Restart. THIS SENDS REAL ORDERS.
#   5  the proof       every executor back, the count right, each one having
#                      ADOPTED its position rather than opened a second one,
#                      and layer two actually in force.
#
# It does not decide anything. deploy\update.ps1 ends by saying that starting
# the mirrors is a decision somebody takes while reading the output rather than
# something a script does at three in the morning; this script is that output,
# written down, with the switch left off.
param(
    [string]$Root = '',

    # fd-api, on the machine. Port 8138 is the desk's, everywhere.
    [string]$Api = 'http://127.0.0.1:8138',

    # The account to look at and, with -Restart, to restart. The funded cent
    # account by name rather than "every enabled account": this runbook exists
    # for the one account that holds money, and a default that quietly widened
    # to the demo as well would be a different operation under the same word.
    [string]$Account = 'vantage-cent',

    # The market whose tape decides whether the week has started. The books on
    # this account are all gold; the feed question is asked of the instrument
    # that is about to be traded and not of the desk in general.
    [string]$Market = 'xauusd',

    # The weekend backstop's cut. AN OVERRIDE, NOT THE SOURCE.
    #
    # Left empty - the normal case - this reads `[prices] weekend_flat` out of
    # config\accounts.toml, the same key py\live\start_executors.ps1 passes to
    # every executor it starts. That is what the key is for: step 1 then asks
    # the same question the executors will, and step 5 compares the RUNNING
    # PROCESSES against the registry rather than against a number somebody
    # typed twice.
    #
    # It was a parameter default of '20:45' until that key existed, which was
    # the November DST trap in miniature - the same value in the launcher, in
    # the executor's argparse and here, none of the three able to see the
    # others. The DST change is now one edit in config\accounts.toml.
    #
    # Given, it overrides the registry for this run, and every line below says
    # which source the value came from.
    [string]$WeekendFlat = '',

    # How old the last QUOTE CHANGE may be before this refuses to call the
    # market open. See Get-FeedVerdict for why 180 and why the exact number
    # does not matter much.
    [int]$MaxQuoteAgeSec = 180,

    # Send real orders. Never the default; see the switch's own note at the
    # restart step.
    [switch]$Restart,

    # Proceed although the feed check failed.
    #
    # NOT an override of the weekend window - that one has no override, because
    # the executors would immediately enter their own window and thrash against
    # a shut market, which is the exact failure the deferral avoided. This
    # covers the other case: a mid-week emergency where the pollers are down,
    # the market IS open, and a real position needs a mirror more than it needs
    # a fresh quote. What it costs is that this script can no longer tell a
    # slipped reopen from a holiday, so the operator is asserting that instead.
    [switch]$AllowStaleFeed,

    # How long to wait for the launcher and then for each executor's first
    # snapshot, in seconds. The launcher itself sleeps 3 + 3.
    [int]$WaitSec = 180,

    [string]$Python = 'C:\Python39\python.exe',

    # Define the functions and run nothing. For deploy\sunday-reopen-selftest.ps1,
    # which dot-sources this file so that what is tested is the script that runs
    # and not a copy of its logic - the same arrangement as -PlanOnly in
    # deploy\start-desk-selftest.ps1.
    [switch]$DefineOnly
)

# ============================================================================
# The refusals, as functions, because they are the part that can be tested
# without a VPS
# ============================================================================

# `--weekend-flat` as minutes past midnight UTC, or $null when it is off.
#
# A transcription of `weekend_cut_minute` in py\live\mt5_executor.py, and it
# has to stay one: step 1 refusing on a different rule from the one the
# executors apply would be a runbook that disagrees with the thing it is
# running. The selftest pins the pair against the same table of instants.
#
# It THROWS rather than defaulting, for the reason the Python does: a mistyped
# value quietly becoming 20:45 is the same class of failure the backstop exists
# to catch. Rule 3 of docs\decisions\2026-09-17-unit-carrying.md - a conversion
# that cannot be established refuses, it does not fall back.
function Get-WeekendCutMinute {
    param([string]$Spec)
    $s = ([string]$Spec).Trim().ToLowerInvariant()
    if ($s -eq 'off' -or $s -eq 'none' -or $s -eq '') { return $null }
    $parts = $s.Split(':')
    if ($parts.Count -ne 2) { throw "-WeekendFlat: '$Spec' is not HH:MM (UTC) or 'off'" }
    $hour = 0; $minute = 0
    if (-not [int]::TryParse($parts[0], [ref]$hour)) { throw "-WeekendFlat: '$Spec' is not HH:MM (UTC) or 'off'" }
    if (-not [int]::TryParse($parts[1], [ref]$minute)) { throw "-WeekendFlat: '$Spec' is not HH:MM (UTC) or 'off'" }
    if ($hour -lt 0 -or $hour -gt 23 -or $minute -lt 0 -or $minute -gt 59) {
        throw "-WeekendFlat: '$Spec' is not a time of day"
    }
    return ($hour * 60 + $minute)
}

# Where the cut comes from, said out loud.
#
# The registry is the source and this is the only place that decides so. Three
# answers, and each one carries WHERE it came from, because "20:45" on a screen
# is worth very little without it: the whole defect this key closes was three
# copies of that number in three files.
#
#   the override    -WeekendFlat was typed. Validated here, so a typo stops the
#                   run rather than becoming a comparison nothing can satisfy.
#   the registry    `[prices] weekend_flat`. The normal case, and the same
#                   string start_executors.ps1 puts on every command line.
#   the default     the key is absent, so the launcher passes nothing and the
#                   executors keep argparse's own 20:45. Reported as the
#                   executor's default and NOT as the registry's, because the
#                   difference is exactly what a reader needs to know before
#                   editing a file that has no such key in it.
#
# $Prices is the object parsed from `accounts.py --prices`, or $null when it
# could not be read at all. Unresolved is set in that last case: the caller
# refuses on it rather than quietly proceeding on a default, because a cut this
# script cannot establish is a cut it cannot check the executors against.
function Resolve-WeekendFlat {
    param([string]$Override, $Prices, [string]$ExecutorDefault = '20:45')
    $r = [pscustomobject]@{
        Value        = $ExecutorDefault
        Source       = ''
        FromRegistry = $false
        Unresolved   = $false
    }
    if ($Override) {
        # Validated by the same function the window uses, so an override and a
        # registry value cannot be held to different standards.
        Get-WeekendCutMinute $Override | Out-Null
        $r.Value = $Override
        $r.Source = 'the -WeekendFlat override, ignoring the registry'
        return $r
    }
    if ($null -eq $Prices) {
        $r.Source = 'nowhere - config\accounts.toml could not be read'
        $r.Unresolved = $true
        return $r
    }
    $fromFile = ''
    if ($null -ne $Prices.weekend_flat) { $fromFile = ([string]$Prices.weekend_flat).Trim() }
    if ($fromFile) {
        Get-WeekendCutMinute $fromFile | Out-Null
        $r.Value = $fromFile
        $r.Source = 'config\accounts.toml [prices] weekend_flat'
        $r.FromRegistry = $true
        return $r
    }
    $r.Source = "the executor's own default - config\accounts.toml [prices] has no weekend_flat"
    return $r
}

# 21:00 UTC, the Sunday reopen, as minutes past midnight. The same constant as
# WEEKEND_REOPEN_MIN_UTC in the executor and for the same reason: it is the
# instant the window ends, not an estimate of when the broker will quote.
$script:REOPEN_MIN_UTC = 21 * 60

# Is $NowUtc inside the executors' weekend window?
#
# Friday at or after the cut, the whole of Saturday, Sunday before 21:00 UTC.
# Half-open at both ends, exactly as `in_weekend_window` is: a poll landing on
# 21:00:00Z on Sunday is OUT of it, because equality has to fall somewhere and
# the side that lets the week start costs a missed poll where the other side
# costs a weekend hold.
#
# UTC, and not New York, for the reason the executor gives: a backstop is asked
# precisely when the things the first layer depends on are broken, so it
# depends on the fewest of them - no bar feed, no broker clock, no timezone
# table on a Python 3.9 that has no zoneinfo.
function Test-InWeekendWindow {
    param([datetime]$NowUtc, $CutMin)
    if ($null -eq $CutMin) { return $false }
    # .NET's DayOfWeek counts Sunday 0; Python's weekday() counts Monday 0.
    # Named rather than arithmetic'd between the two, because an off-by-one
    # here reads as a working script that refuses on the wrong day.
    $dow = $NowUtc.DayOfWeek
    $minute = $NowUtc.Hour * 60 + $NowUtc.Minute
    if ($dow -eq [System.DayOfWeek]::Friday)   { return ($minute -ge [int]$CutMin) }
    if ($dow -eq [System.DayOfWeek]::Saturday) { return $true }
    if ($dow -eq [System.DayOfWeek]::Sunday)   { return ($minute -lt $script:REOPEN_MIN_UTC) }
    return $false
}

# Has a quote actually arrived? Two clocks, and they answer different questions.
#
# THIS IS THE STEP A CLOCK CANNOT DO. The reopen slips; a holiday Sunday has a
# clock and no tape. But the trap is narrower than "is the feed up", and it is
# why both numbers are read:
#
#   $LiveAtMs    `/api/paper/status`'s live bar `at` - when THE SERVER received
#                a post. py\live\start_pollers.ps1 starts every poller with
#                `--tick-poll=1`, so this advances once a second while the
#                poller is alive, whatever the market is doing. Over a weekend
#                the terminal hands back the same frozen quote and the poller
#                posts it anyway: this clock stays fresh all weekend. It proves
#                the POLLER is alive. It does not prove the market is.
#   $LastTickMs  `/api/paper/m1`'s last accepted quote CHANGE. Its own doc
#                comment says a duplicate does not advance it. That is exactly
#                the property wanted here: a frozen weekend quote cannot move
#                it, so this is the one that proves the TAPE is running.
#
# Both, therefore, and the second is the decisive one. A missing live bar means
# the poller is dead (the API drops a live bar older than MAX_LIVE_AGE_MS =
# 90_000, so `null` is the API's own verdict, not this script's); a stale
# last_tick_ms with a fresh live bar is the holiday case precisely.
#
# WHY 180 SECONDS, and why the number is not delicate. The two populations this
# separates are about three orders of magnitude apart: at a real reopen gold
# prints continuously and last_tick_ms is seconds old, while a weekend's last
# change is the Friday close - by Sunday 21:00Z that is roughly 48 hours, or
# 172,800 seconds, which is 960x the threshold. Anything from 90 seconds to an
# hour would sort them. 180 was picked at the tight end of that range because
# the cost of being wrong is asymmetric and small either way: too tight is a
# refusal and a retry a minute later, too loose admits a frozen tape. It is two
# of the API's own 90-second liveness windows, so it cannot be tripped by a
# thin Sunday-evening book pausing through one of them.
#
# A quote from the FUTURE refuses rather than passing. It means the two clocks
# disagree, and when they disagree no age computed here means anything - which
# includes the age that would otherwise have said "fresh".
function Get-FeedVerdict {
    param(
        $LiveAtMs,
        $LastTickMs,
        [int64]$NowMs,
        [int]$MaxAgeSec,
        [string]$Unavailable = ''
    )
    $v = [pscustomobject]@{
        Ok         = $false
        Reason     = ''
        LiveAgeSec = $null
        TickAgeSec = $null
    }
    if ($Unavailable) {
        $v.Reason = "the m1 ring has no data for this market: $Unavailable"
        return $v
    }
    if ($null -eq $LastTickMs) {
        $v.Reason = 'no last_tick_ms on /api/paper/m1: the API has never accepted a quote CHANGE for this market'
        return $v
    }
    $tickAge = [math]::Round(($NowMs - [int64]$LastTickMs) / 1000.0, 1)
    $v.TickAgeSec = $tickAge
    if ($null -ne $LiveAtMs) {
        $v.LiveAgeSec = [math]::Round(($NowMs - [int64]$LiveAtMs) / 1000.0, 1)
    }
    if ($tickAge -lt -5) {
        $v.Reason = "the last quote change is $([math]::Abs($tickAge))s in the FUTURE: this machine's clock and the API's disagree, so no age here means anything"
        return $v
    }
    if ($null -ne $v.LiveAgeSec -and $v.LiveAgeSec -lt -5) {
        $v.Reason = "the live bar is $([math]::Abs($v.LiveAgeSec))s in the FUTURE: this machine's clock and the API's disagree, so no age here means anything"
        return $v
    }
    if ($null -eq $LiveAtMs) {
        $v.Reason = 'no live bar on /api/paper/status for this market: the API drops one older than 90s, so no poller has posted a quote in at least that long'
        return $v
    }
    if ($tickAge -gt $MaxAgeSec) {
        $v.Reason = "the last quote CHANGE was $($tickAge)s ago, older than the ${MaxAgeSec}s threshold - the poller is posting but the price is not moving, which is what a shut market looks like"
        return $v
    }
    if ($v.LiveAgeSec -gt $MaxAgeSec) {
        $v.Reason = "the live bar is $($v.LiveAgeSec)s old, older than the ${MaxAgeSec}s threshold"
        return $v
    }
    $v.Ok = $true
    $v.Reason = "the tape is running: last quote change $($tickAge)s ago, live bar $($v.LiveAgeSec)s old, threshold ${MaxAgeSec}s"
    return $v
}

# The two refusals as one answer, in the order they should be read: the clock
# first, because a market that is shut by the calendar makes the feed question
# moot, and a reader who sees "feed stale" on a Saturday would go looking for a
# dead poller.
function Get-ReopenVerdict {
    param([datetime]$NowUtc, $CutMin, $Feed, [switch]$AllowStaleFeed)
    $v = [pscustomobject]@{ Ok = $false; Reason = ''; Refusal = '' }
    if (Test-InWeekendWindow -NowUtc $NowUtc -CutMin $CutMin) {
        $v.Refusal = 'weekend-window'
        $v.Reason = "$($NowUtc.ToString('ddd HH:mm')) UTC is inside the executors' own weekend window; a restart here would have the backstop close into a shut market every poll"
        return $v
    }
    if (-not $Feed.Ok) {
        if ($AllowStaleFeed) {
            $v.Ok = $true
            $v.Reason = "the feed check FAILED and -AllowStaleFeed was given: $($Feed.Reason)"
            $v.Refusal = 'overridden'
            return $v
        }
        $v.Refusal = 'feed'
        $v.Reason = $Feed.Reason
        return $v
    }
    $v.Ok = $true
    $v.Reason = "out of the weekend window and $($Feed.Reason)"
    return $v
}

# Wilder's ATR over `[time, open, high, low, close]` rows, oldest first.
#
# Computed here rather than read off `/api/paper/run/{id}`'s `series`, because
# that map's keys are built from the STRATEGY's own indicator specs - a
# different book has different keys, and a key that moves turns this number
# into a silent $null on the one morning it is wanted. Five numbers per bar are
# the same shape on every book.
#
# $null under Period+1 bars rather than a short-window answer: an ATR read on
# a slice shorter than its period is a different series from the one the
# strategy sized against, and the whole point of the figure below is to compare
# a gap against the stop the books actually used.
function Get-AtrWilder {
    param($Bars, [int]$Period = 14)
    $rows = @($Bars)
    if ($rows.Count -lt ($Period + 1)) { return $null }
    $tr = New-Object System.Collections.ArrayList
    for ($i = 1; $i -lt $rows.Count; $i++) {
        $h = [double]$rows[$i][2]; $l = [double]$rows[$i][3]; $pc = [double]$rows[$i - 1][4]
        $range = [math]::Max($h - $l, [math]::Max([math]::Abs($h - $pc), [math]::Abs($l - $pc)))
        [void]$tr.Add($range)
    }
    if ($tr.Count -lt $Period) { return $null }
    $sum = 0.0
    for ($i = 0; $i -lt $Period; $i++) { $sum += [double]$tr[$i] }
    $atr = $sum / $Period
    for ($i = $Period; $i -lt $tr.Count; $i++) {
        $atr = (($atr * ($Period - 1)) + [double]$tr[$i]) / $Period
    }
    return $atr
}

# The weekend gap, found in the bars themselves rather than asserted from a
# calendar.
#
# The most recent hole in the bar series of at least $MinGapHours, scanned from
# the newest end backwards. A calendar would be wrong in both directions: the
# broker's daily break is about an hour and must not count, and a Friday close
# that slipped or a Sunday that opened late moves the hole without moving the
# date. Twelve hours separates the two cleanly - the daily break is ~1h, the
# weekend is ~48h - and nothing on this instrument sits between them.
#
# Returns the close BEFORE the hole and the open AFTER it. The gap is signed in
# price: positive means the week opened ABOVE Friday's close, which pays a long
# and hurts a short.
function Get-WeekendGap {
    param($Bars, [double]$MinGapHours = 12.0)
    $rows = @($Bars)
    $g = [pscustomobject]@{
        Found        = $false
        PrevCloseMs  = $null
        PrevClose    = $null
        NextOpenMs   = $null
        NextOpen     = $null
        GapPoints    = $null
        HoleHours    = $null
        Index        = -1
    }
    if ($rows.Count -lt 2) { return $g }
    $needMs = [int64]($MinGapHours * 3600000)
    for ($i = $rows.Count - 1; $i -ge 1; $i--) {
        $delta = [int64]$rows[$i][0] - [int64]$rows[$i - 1][0]
        if ($delta -ge $needMs) {
            $g.Found = $true
            $g.Index = $i
            $g.PrevCloseMs = [int64]$rows[$i - 1][0]
            $g.PrevClose = [double]$rows[$i - 1][4]
            $g.NextOpenMs = [int64]$rows[$i][0]
            $g.NextOpen = [double]$rows[$i][1]
            $g.GapPoints = [math]::Round($g.NextOpen - $g.PrevClose, 3)
            $g.HoleHours = [math]::Round($delta / 3600000.0, 2)
            return $g
        }
    }
    return $g
}

if ($DefineOnly) { return }

# ============================================================================
# From here down nothing is tested locally, so nothing below decides anything
# ============================================================================

$ErrorActionPreference = 'Stop'

if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}
Set-Location $Root

# PATH, re-read from the registry, before anything needs a tool. Measured on
# the VPS 2026-09-17: over ssh the operator inherits sshd's environment, whose
# PATH predates the git and Python installs, so the tool is on the machine and
# not on the path of the process that needs it. deploy\update.ps1 died at `git
# pull` for exactly this and fixed it the same way.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')

$script:StartedUtc = (Get-Date).ToUniversalTime()
$logDir = Join-Path $Root 'data\paper\logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$script:LogPath = Join-Path $logDir ('sunday-reopen-' + $script:StartedUtc.ToString('yyyyMMdd-HHmmss') + 'Z.log')

# Everything this run says goes to the screen AND to a file, because the whole
# operation is "a person reads the output and decides", and the decision is
# worth more later than the scrollback of an ssh session that has closed. The
# launchers already write their own stdout under data\paper\logs\; this sits
# beside them under the same convention.
function Say { param([string]$Text = '', [string]$Colour = 'Gray')
    Write-Host $Text -ForegroundColor $Colour
    try { Add-Content -Path $script:LogPath -Value $Text -Encoding utf8 } catch { }
}
function Step { param([string]$What) Say '' ; Say "== $What" 'Cyan' }
function Note { param([string]$What) Say "   $What" 'DarkGray' }
function Warn { param([string]$What) Say "   $What" 'Yellow' }
function Good { param([string]$What) Say "   $What" 'Green' }
function Bad  { param([string]$What) Say "   $What" 'Red' }

function Refuse {
    param([string]$Headline, [string[]]$Detail = @())
    Say ''
    Say $Headline 'Red'
    foreach ($d in $Detail) { Say "  $d" 'Red' }
    Say ''
    Say 'NOTHING WAS STARTED, STOPPED OR SENT. The executors are exactly as they were.' 'Red'
    Say "This run is written to $script:LogPath" 'DarkGray'
    exit 1
}

# `$ErrorActionPreference = 'Stop'` makes Write-Error terminating, so an `exit`
# written after one never runs and the script dies where it stands. Same trap
# deploy\update.ps1 documents, same answer: a trap, so that an unanticipated
# failure still says where it stopped and whether it had already acted.
$script:Acted = $false
trap {
    Say ''
    Say "$($_.Exception.Message)" 'Red'
    if ($script:Acted) {
        Say 'THIS RUN HAD ALREADY RESTARTED THE EXECUTORS when it failed. They are running,' 'Red'
        Say 'and nothing has verified them. Check by hand:' 'Red'
        Say '  Get-CimInstance Win32_Process -Filter "name=''python.exe''" | Where-Object { $_.CommandLine -like ''*mt5_executor.py*'' } | Select-Object ProcessId, CommandLine' 'Red'
    } else {
        Say 'Nothing had been started or sent at the point this failed.' 'DarkGray'
    }
    exit 1
}

Say "flowdesk Sunday reopen - $($script:StartedUtc.ToString('yyyy-MM-dd HH:mm:ss'))Z"
Say "root $Root   account $Account   market $Market   api $Api"
Say "log  $script:LogPath"
if (-not $Restart) {
    Say 'REPORT ONLY. -Restart was not given, so nothing will be started.' 'DarkGray'
} else {
    Say '-Restart WAS GIVEN: if every check below passes, this will start mirrors that' 'Yellow'
    Say 'send REAL orders on a funded account.' 'Yellow'
}

# ---------------------------------------------------------------- 1, the clock
Step '1. the clock - is the executors'' own weekend window in force?'

# The registry FIRST, because the cut comes out of it and everything in this
# step is measured against the cut. accounts.py refuses a malformed
# weekend_flat, and that refusal is taken rather than swallowed: a cut this
# script cannot establish is a cut it cannot hold the executors to.
$prices = $null
$pricesErr = ''
$pj = & $Python (Join-Path $Root 'py\live\accounts.py') '--prices' 2>&1
if ($LASTEXITCODE -ne 0) {
    $pricesErr = ($pj | Out-String).Trim()
} else {
    try { $prices = $pj | ConvertFrom-Json } catch { $pricesErr = "could not parse accounts.py --prices: $($_.Exception.Message)" }
}
if ($pricesErr) {
    Refuse "config\accounts.toml [prices] would not read: $pricesErr" @(
        'The weekend cut lives in that file and nothing else on this machine knows it.',
        'Fix the registry and run this again. Nothing has been read from the account yet.')
}

# Kept before $WeekendFlat is overwritten with the resolved value: step 5 has
# to be able to say that a mismatch is the operator's override and not a fault.
$weekendOverride = $WeekendFlat
$flat = Resolve-WeekendFlat -Override $WeekendFlat -Prices $prices
if ($flat.Unresolved) {
    Refuse 'the weekend cut could not be established from the registry' @(
        'Refusing rather than assuming 20:45: a cut this script cannot establish is a cut',
        'it cannot check the running executors against, which is the only reason step 5 exists.')
}
$WeekendFlat = $flat.Value
$cutMin = Get-WeekendCutMinute $WeekendFlat
$nowUtc = (Get-Date).ToUniversalTime()
$inWindow = Test-InWeekendWindow -NowUtc $nowUtc -CutMin $cutMin

Note "now                $($nowUtc.ToString('ddd yyyy-MM-dd HH:mm:ss'))Z  (minute $($nowUtc.Hour * 60 + $nowUtc.Minute) of the UTC day)"
Note "the cut comes from $($flat.Source)"
if ($null -eq $cutMin) {
    Warn "the weekend backstop is '$WeekendFlat' - OFF. This script therefore has no weekend"
    Warn 'window to refuse on, and the feed check below is the only thing between you and a'
    Warn 'restart into a shut market. That is a real gap: read step 2 carefully.'
} else {
    Note "Friday cut         $WeekendFlat UTC (minute $cutMin)"
}
Note "Sunday reopen      21:00 UTC (minute $script:REOPEN_MIN_UTC)"
if ($inWindow) {
    Bad  'verdict            INSIDE the weekend window'
} else {
    Good 'verdict            outside the weekend window'
}

# Refused HERE, before fd-api is touched, and the ordering is the same one
# Get-ReopenVerdict encodes: the clock first, because a market that is shut by
# the calendar makes every other question moot.
#
# It is also what the first run of this script did wrong. On Saturday
# 2026-09-19 05:24Z, with fd-api not running, it asked the API first and
# refused with "fd-api did not answer" - true, and the wrong answer: the
# reason not to restart was that it was Saturday, and a reader would have gone
# looking for a dead server instead of a calendar. So the same verdict
# function is asked twice, once with the feed stubbed out and once for real,
# rather than once at the end.
$clockOnly = Get-ReopenVerdict -NowUtc $nowUtc -CutMin $cutMin `
                               -Feed ([pscustomobject]@{ Ok = $true; Reason = 'not asked yet' })
if (-not $clockOnly.Ok) {
    Refuse "REFUSED on the clock: $($clockOnly.Reason)" @(
        "The window runs Friday from $WeekendFlat UTC, all Saturday, and Sunday until 21:00 UTC.",
        'There is deliberately no override for this one. An executor started inside its own',
        'weekend window closes every position it finds and refuses to open - into a market',
        'that is shut, so every close fails and it retries at every poll until Sunday.',
        'That is the failure the restart was deferred to avoid.',
        'Come back after 21:00 UTC on Sunday.')
}

# ----------------------------------------------------------------- 2, the feed
Step '2. the feed - has a quote actually arrived?'

function Get-Json { param([string]$Url, [int]$TimeoutSec = 10)
    return (Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec $TimeoutSec).Content | ConvertFrom-Json
}

$status = $null; $m1 = $null; $apiErr = ''
try {
    $ver = Get-Json "$Api/api/version" 5
    Note "fd-api             $($ver.version) $($ver.git_sha) built $($ver.built_at)"
    if ($ver.git_dirty) { Warn 'fd-api was built from a DIRTY tree; the hash names a commit whose contents it does not contain.' }
} catch {
    $apiErr = $_.Exception.Message
}
if (-not $apiErr) {
    try {
        $status = Get-Json "$Api/api/paper/status" 15
        $m1 = Get-Json "$Api/api/paper/m1?market=$Market&n=1" 15
    } catch { $apiErr = $_.Exception.Message }
}
if ($apiErr) {
    Refuse "fd-api at $Api did not answer: $apiErr" @(
        'Without it there is no way to ask whether the market is open, and no way to read',
        'what the books think they hold. Start fd-api (the flowdesk-api scheduled task) and',
        'run this again.')
}

# The live bar belongs to a run, not to the desk: it is stored per `market:tf`
# and served on every run of that pair. Any run on this market answers, so take
# the freshest - a run that was stopped carries an old one.
$marketRuns = @($status.runs | Where-Object { $_.market -eq $Market })
$liveAt = $null
foreach ($r in $marketRuns) {
    if ($r.live -and $r.live.at) {
        if ($null -eq $liveAt -or [int64]$r.live.at -gt $liveAt) { $liveAt = [int64]$r.live.at }
    }
}
$lastTick = $null
if ($null -ne $m1.last_tick_ms) { $lastTick = [int64]$m1.last_tick_ms }
$nowMs = [int64]((Get-Date).ToUniversalTime() - [datetime]'1970-01-01').TotalMilliseconds

$feed = Get-FeedVerdict -LiveAtMs $liveAt -LastTickMs $lastTick -NowMs $nowMs `
                        -MaxAgeSec $MaxQuoteAgeSec -Unavailable ([string]$m1.unavailable)

Note "runs on $Market     $($marketRuns.Count)"
if ($null -eq $liveAt) {
    Note 'live bar           none (the API drops one older than 90s)'
} else {
    Note "live bar           $([math]::Round(($nowMs - $liveAt) / 1000.0, 1))s old  (proves the POLLER is alive, not the market)"
}
if ($null -eq $lastTick) {
    Note 'last quote CHANGE  never'
} else {
    Note "last quote CHANGE  $([math]::Round(($nowMs - $lastTick) / 1000.0, 1))s ago  (proves the TAPE is running; a duplicate does not advance it)"
}
Note "duplicates $($m1.duplicates), dropped $($m1.dropped) on the m1 ring"
if ($feed.Ok) { Good "verdict            $($feed.Reason)" } else { Bad "verdict            $($feed.Reason)" }

# The clock is READ AGAIN rather than reused. Step 1's reading is minutes old
# by now - an unreachable fd-api times out slowly - and on a Friday those
# minutes can cross the cut. The cost of re-reading is nothing; the cost of not
# is a restart begun one minute inside the window.
$verdict = Get-ReopenVerdict -NowUtc ((Get-Date).ToUniversalTime()) -CutMin $cutMin `
                             -Feed $feed -AllowStaleFeed:$AllowStaleFeed
if (-not $verdict.Ok) {
    if ($verdict.Refusal -eq 'weekend-window') {
        Refuse "REFUSED on the clock: $($verdict.Reason)" @(
            "The window runs Friday from $WeekendFlat UTC, all Saturday, and Sunday until 21:00 UTC.",
            'There is deliberately no override for this one. An executor started inside its own',
            'weekend window closes every position it finds and refuses to open - into a market',
            'that is shut, so every close fails and it retries at every poll until Sunday.',
            'That is the failure the restart was deferred to avoid.',
            'Come back after 21:00 UTC on Sunday.')
    }
    Refuse "REFUSED on the feed: $($verdict.Reason)" @(
        'A clock is not enough. The reopen slips, and a holiday Sunday has a clock and no tape.',
        'If the pollers are down, start them: py\live\start_pollers.ps1.',
        'If the market really is open and the feed is the problem, -AllowStaleFeed proceeds and',
        'says so in the log - but then nothing here can tell a slipped reopen from a holiday.')
}
if ($verdict.Refusal -eq 'overridden') {
    Warn "PROCEEDING WITH A FAILED FEED CHECK because -AllowStaleFeed was given: $($feed.Reason)"
}

# -------------------------------------------------------------- 3, the account
Step '3. the account - what is open, in whose book, worth what'

# The registry, read the way every launcher reads it. `--id` answers for a
# disabled account on purpose, so `enabled` is checked here rather than assumed.
$acctJson = & $Python 'py/live/accounts.py' "--id=$Account" 2>&1
if ($LASTEXITCODE -ne 0) { Refuse "account registry: $acctJson" }
# Assigned first, then wrapped: `@($x | ConvertFrom-Json)` does not unroll a
# JSON array in PowerShell 5.1 and yields a one-element array holding an
# Object[]. Measured 2026-09-16, and it silently started one account's books
# against another's terminal - see the same note in py\live\start_executors.ps1.
$parsedAcct = $acctJson | ConvertFrom-Json
$acct = @($parsedAcct)[0]
if (-not $acct -or -not $acct.id) { Refuse "config\accounts.toml has no account '$Account'" }

Note "$($acct.id): $($acct.label)"
Note "login $($acct.login) on $($acct.server), terminal $($acct.terminal)"
Note "real_money $($acct.real_money), enabled $($acct.enabled), dry_run $($acct.dry_run), lot_scale $($acct.lot_scale), suffix '$($acct.symbol_suffix)'"
$registryBooks = @($acct.runs)
Note "registry names $($registryBooks.Count) book(s): $($registryBooks -join ', ')"
if (-not $acct.enabled) {
    Warn 'enabled = false: start_executors.ps1 will skip this account entirely, by name or otherwise.'
}

# What the account holds RIGHT NOW, read from the mirrors themselves.
#
# `data/live/<account>/<book>/broker.json` is written whole-then-renamed every
# poll and carries the position, the login and the account's own currency. It
# is the only view of the broker that does not involve talking to MetaTrader,
# which this script must not do: the executors own those terminals.
#
# Deliberately a THIRD copy of this reader rather than something shared with
# py\live\start_executors.ps1 and deploy\update.ps1, and less arbitrarily so
# than those two: this one reads the entry, the stop, the unrealised profit and
# the currency, which neither of the others needs. A common function would be
# the union of three different questions. The shared reasoning still applies -
# a fourth file that all of them dot-source is one more thing to be missing on
# the server at the moment someone is trying to stop a trade.
#
# STALENESS IS REPORTED AND IS THE POINT. A snapshot older than 60s (the same
# bound as MIRROR_STALE_MS in telegram_notify.py) means the executor that wrote
# it has stopped, so the file says what was true when it died.
function Get-AccountHoldings {
    param([string]$RootPath, [string]$AccountId)
    $dir = Join-Path $RootPath "data\live\$AccountId"
    $out = @()
    if (-not (Test-Path $dir)) { return $out }
    $nowMs = [int64]((Get-Date).ToUniversalTime() - [datetime]'1970-01-01').TotalMilliseconds
    foreach ($runDir in @(Get-ChildItem $dir -Directory -ErrorAction SilentlyContinue)) {
        $p = Join-Path $runDir.FullName 'broker.json'
        if (-not (Test-Path $p)) { continue }
        $snap = $null
        try { $snap = Get-Content $p -Raw -ErrorAction Stop | ConvertFrom-Json } catch { continue }
        if (-not $snap) { continue }
        $at = 0
        if ($snap.at) { $at = [int64]$snap.at }
        $out += [pscustomobject]@{
            Account   = $AccountId
            Run       = $runDir.Name
            AtMs      = $at
            AgeSec    = [math]::Round(($nowMs - $at) / 1000.0, 1)
            Stale     = (($nowMs - $at) -gt 60000)
            # The account's own word for itself, from account_info().trade_mode.
            Real      = ($snap.demo -eq $false)
            Currency  = $snap.currency
            Balance   = $snap.balance
            Equity    = $snap.equity
            LotScale  = $snap.lot_scale
            DryRun    = $snap.dry_run
            BookSide  = $snap.book_side
            BookLots  = $snap.book_lots
            Blocked   = $snap.blocked
            StandOut  = $snap.standing_out
            Drift     = $snap.drift
            HasPos    = ($null -ne $snap.position)
            Ticket    = $(if ($snap.position) { [int64]$snap.position.ticket } else { $null })
            Side      = $(if ($snap.position) { $snap.position.side } else { $null })
            Lots      = $(if ($snap.position) { $snap.position.lots } else { $null })
            Entry     = $(if ($snap.position) { $snap.position.entry_price } else { $null })
            PriceNow  = $(if ($snap.position) { $snap.position.price_now } else { $null })
            Stop      = $(if ($snap.position) { $snap.position.sl } else { $null })
            Target    = $(if ($snap.position) { $snap.position.tp } else { $null })
            Profit    = $(if ($snap.position) { $snap.position.profit } else { $null })
            Swap      = $(if ($snap.position) { $snap.position.swap } else { $null })
            OpenedAt  = $(if ($snap.position) { $snap.position.opened_at } else { $null })
        }
    }
    return $out
}

$holdings = @(Get-AccountHoldings $Root $Account)
$open = @($holdings | Where-Object { $_.HasPos })

$ccy = 'the account currency'
$anyCcy = @($holdings | Where-Object { $_.Currency } | Select-Object -First 1)
if ($anyCcy.Count -gt 0) { $ccy = $anyCcy[0].Currency }
$balLine = @($holdings | Sort-Object AtMs -Descending | Select-Object -First 1)
if ($balLine.Count -gt 0 -and $null -ne $balLine[0].Balance) {
    # The unit is on the line with the number, not assumed by the reader. On
    # 33705331 this is USC, where 100 USC = USD 1, and a reader who takes it for
    # USD is out by a hundred. docs\decisions\2026-09-17-unit-carrying.md rule 1.
    Note "balance $($balLine[0].Balance) $ccy, equity $($balLine[0].Equity) $ccy, as of $($balLine[0].AgeSec)s ago"
}

if ($open.Count -eq 0) {
    Good "$Account is FLAT: no mirror on this account reports a position."
} else {
    Say ''
    Say "   $($open.Count) position(s) open on $Account, in ${ccy}:" 'Yellow'
}
foreach ($h in $open) {
    $line = "   $($h.Run): $($h.Side) $($h.Lots) lots, ticket $($h.Ticket), entry $($h.Entry)"
    if ($null -ne $h.Stop) { $line += ", stop $($h.Stop)" } else { $line += ', NO STOP' }
    if ($null -ne $h.Target) { $line += ", target $($h.Target)" }
    Say $line 'Yellow'
    $pl = "      unrealised $($h.Profit) $ccy"
    if ($null -ne $h.Swap) { $pl += " (swap $($h.Swap) $ccy)" }
    $pl += ", price now $($h.PriceNow)"
    Say $pl 'Yellow'
    # The two lot figures are NOT in the same unit and a reader comparing them
    # directly is wrong on every real-money book: `book_lots` is the BOOK's
    # size and `lots` is the TERMINAL's volume, which is the book's times
    # `lot_scale`. The scale travels with the numbers it converts.
    Say "      the book wants $($h.BookSide) $($h.BookLots) lots x lot_scale $($h.LotScale) = $([math]::Round([double]$h.BookLots * [double]$h.LotScale, 2)) on the terminal" 'DarkGray'
    if ($h.Stale) { Say "      SNAPSHOT IS $($h.AgeSec)s OLD - the executor that wrote it has already stopped" 'Red' }
    if ($h.Drift) { Say "      DRIFT: $($h.Drift)" 'Red' }
    if ($h.Blocked) { Say "      BLOCKED: $($h.Blocked)" 'Red' }
    if ($h.StandOut) { Say "      standing out: $($h.StandOut)" 'DarkGray' }
    if (-not $h.Real) { Say '      this snapshot says demo, not real money' 'DarkGray' }
}

# What the BOOKS think they hold, which is a different question and the one
# that decides what the restarted executor will do: the reconciler compares the
# account against the book, so a book that is flat while the account holds is a
# position about to be closed, and a book that holds while the account is flat
# is a position about to be opened or adopted.
Say ''
Say '   what each book itself thinks it holds:' 'Gray'
$bookState = @{}
foreach ($b in $registryBooks) {
    $run = @($status.runs | Where-Object { $_.id -eq $b })
    if ($run.Count -eq 0) {
        Say "   $($b): NOT ON THE DESK - fd-api has no run with this id" 'Red'
        continue
    }
    $r = $run[0]
    $bookState[$b] = $r
    if ($r.open) {
        $stopTxt = 'no stop'
        if ($null -ne $r.open.stop) { $stopTxt = "stop $($r.open.stop)" }
        Say "   $($b): $($r.open.side) $($r.open.lots) lots from $($r.open.entry_price), $stopTxt  [book equity $($r.equity) USD]" 'Gray'
    } else {
        Say "   $($b): flat" 'DarkGray'
    }
    if ($r.paused) { Say "      the book is PAUSED: new entries are off, an open position is still managed" 'Yellow' }
}

# Any book holding on the account that the registry does NOT name is a position
# a restart would orphan - start_executors.ps1 refuses on exactly this, and
# saying it here means the refusal is not a surprise a hundred lines later.
$orphanRisk = @($open | Where-Object { $registryBooks -notcontains $_.Run -and -not $_.Stale })
if ($orphanRisk.Count -gt 0) {
    Say ''
    foreach ($o in $orphanRisk) {
        Bad "$($o.Run) holds $($o.Side) $($o.Lots) lots and is NOT in the registry's runs for $Account."
    }
    Bad 'start_executors.ps1 will REFUSE on this (its orphan check) unless -AllowOrphans is given.'
    Bad 'This script does not pass -AllowOrphans. Decide what that position is for first.'
}

# --------------------------------------------------- 3b, the gap that just happened
Step '3b. the weekend gap - what the hole in the tape cost'

# Read off the bars of a book on this market, not off a calendar: the hole is
# found in the series (see Get-WeekendGap). 200 bars of 15m is just over two
# days, which covers Friday's session and the warmup the ATR needs.
$gapBook = $null
foreach ($b in $registryBooks) {
    if ($bookState.ContainsKey($b) -and $bookState[$b].market -eq $Market) { $gapBook = $b; break }
}
if (-not $gapBook) {
    Warn "no book on $Market in this account's registry entry; skipping the gap."
} else {
    $detail = $null
    $detailUrl = "$Api/api/paper/run/" + $gapBook + '?bars=200'
    try { $detail = Get-Json $detailUrl 20 } catch { }
    if (-not $detail -or -not $detail.bars) {
        Warn "could not read bars from /api/paper/run/$gapBook; skipping the gap."
    } else {
        $bars = @($detail.bars)
        Note "$($bars.Count) bars of $($bookState[$gapBook].tf) from $gapBook"
        $gap = Get-WeekendGap -Bars $bars
        if (-not $gap.Found) {
            Warn 'no hole of 12h or more in this window: either the week has not broken yet in these'
            Warn 'bars, or the window is too short. Nothing to measure.'
        } else {
            # The ATR is taken over the bars BEFORE the hole, on purpose. The
            # stop these books sized against was set on Friday - the decision
            # note measures 1.2 x ATR(14) = 11.93 on 2026-09-18 - and an ATR
            # that includes the gap bar answers a different question, one whose
            # answer is always "the gap was about one ATR".
            $pre = @($bars[0..($gap.Index - 1)])
            $atr = Get-AtrWilder -Bars $pre -Period 14
            $fri = ([datetime]'1970-01-01').AddMilliseconds($gap.PrevCloseMs)
            $mon = ([datetime]'1970-01-01').AddMilliseconds($gap.NextOpenMs)
            Note "last bar before the hole  $($fri.ToString('ddd yyyy-MM-dd HH:mm'))Z  close $($gap.PrevClose)"
            Note "first bar after it        $($mon.ToString('ddd yyyy-MM-dd HH:mm'))Z  open  $($gap.NextOpen)"
            Note "the hole itself           $($gap.HoleHours) hours with no bar"
            $sign = 'UP'
            if ($gap.GapPoints -lt 0) { $sign = 'DOWN' }
            Say "   the gap                   $($gap.GapPoints) points $sign" 'Yellow'
            if ($null -eq $atr) {
                Warn 'ATR(14): not enough bars before the hole to compute one; the gap is in points only.'
            } else {
                $inAtr = [math]::Round([math]::Abs($gap.GapPoints) / $atr, 2)
                Say "   ATR(14) before the hole   $([math]::Round($atr, 2)) points" 'Yellow'
                Say "   the gap in ATR units      $inAtr x ATR" 'Yellow'
                Note "the books size their stop at 1.2 x ATR, so that is about $([math]::Round([math]::Abs($gap.GapPoints) / ($atr * 1.2), 2)) stops."
                Note 'A stop does not work across a gap: a position gapped through fills AT THE GAP,'
                Note 'which is the whole reason the weekend rule exists.'
            }
            # Direction against each open position, named rather than left to
            # the reader's arithmetic at three in the morning.
            foreach ($h in $open) {
                $hurt = 'in its favour'
                if (($h.Side -eq 'LONG' -and $gap.GapPoints -lt 0) -or ($h.Side -eq 'SHORT' -and $gap.GapPoints -gt 0)) {
                    $hurt = 'AGAINST it'
                }
                Say "      $($h.Run) is $($h.Side): the gap went $hurt. Its unrealised is $($h.Profit) $ccy, which is the terminal's own number and already contains this." 'Yellow'
                if ($null -ne $h.Stop) {
                    $through = $false
                    if ($h.Side -eq 'LONG' -and $gap.NextOpen -lt [double]$h.Stop) { $through = $true }
                    if ($h.Side -eq 'SHORT' -and $gap.NextOpen -gt [double]$h.Stop) { $through = $true }
                    if ($through) {
                        Bad "$($h.Run): the week opened THROUGH its stop ($($h.Stop)). It filled at the gap, not at the level - check the deal history for what it actually got."
                    }
                }
            }
        }
    }
}

# ------------------------------------------------------ 4, what happens next
Step '4. the restart'

$running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" -ErrorAction SilentlyContinue |
             Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
Note "$($running.Count) executor process(es) running right now"
foreach ($p in $running) {
    $runId = ''
    if ($p.CommandLine -match '--run=([^\s]+)') { $runId = $Matches[1] }
    $hasFlag = 'no --weekend-flat on the command line'
    if ($p.CommandLine -match '--weekend-flat[= ]([^\s]+)') { $hasFlag = "--weekend-flat $($Matches[1])" }
    Note "  pid $($p.ProcessId)  $runId  ($hasFlag)"
}

$launcher = Join-Path $Root 'py\live\start_executors.ps1'
$cmdline = "powershell -NoProfile -ExecutionPolicy Bypass -File py\live\start_executors.ps1 -Account $Account -AllowReal -Live"

if (-not $Restart) {
    Say ''
    Say '   -Restart was NOT given, so this stops here.' 'Cyan'
    Say '   Nothing has been started, stopped or sent.' 'Cyan'
    Say ''
    Say '   What -Restart would run, detached, and what it costs:' 'Yellow'
    Say "     $cmdline" 'Yellow'
    Say '     -AllowReal permits the account the registry marks real_money = true.' 'Yellow'
    Say '     -Live honours its dry_run = false. Together they mean ORDERS THAT SPEND MONEY.' 'Yellow'
    Say '     It stops EVERY executor first, including the demo''s, and starts back only what' 'Yellow'
    Say '     the registry and that command line together name.' 'Yellow'
    Say ''
    Say "   Read the three checks above, then run the same command with -Restart." 'Cyan'
    Say "   This run is written to $script:LogPath" 'DarkGray'
    exit 0
}

# THE POINT OF NO RETURN.
#
# Everything above is a read. From here the executors are stopped and started
# and real orders become possible, so the state to compare against is captured
# FIRST - while the old executors are still alive and their snapshots are
# seconds old, which is the same reason start_executors.ps1 reads its holdings
# between deciding and stopping.
$before = @{}
foreach ($h in $holdings) {
    $jsonl = Join-Path $Root "data\live\$Account\$($h.Run)\executor.jsonl"
    $len = 0
    if (Test-Path $jsonl) { $len = (Get-Item $jsonl).Length }
    $before[$h.Run] = [pscustomobject]@{
        Ticket = $h.Ticket
        Side   = $h.Side
        Lots   = $h.Lots
        Entry  = $h.Entry
        AtMs   = $h.AtMs
        Bytes  = $len
    }
}
$restartMs = [int64]((Get-Date).ToUniversalTime() - [datetime]'1970-01-01').TotalMilliseconds
Say ''
Say "   STARTING THE MIRRORS ON $Account. This sends real orders." 'Red'
Say "   $cmdline" 'Red'
foreach ($k in $before.Keys) {
    if ($null -ne $before[$k].Ticket) {
        Say "   before: $k holds ticket $($before[$k].Ticket) ($($before[$k].Side) $($before[$k].Lots) from $($before[$k].Entry))" 'Red'
    }
}
$script:Acted = $true

# Detached through Win32_Process.Create, and this is not decoration.
#
# Measured 2026-09-18 17:00Z: all eight AI traders were found dead minutes
# after a deploy. They had been started by a launcher running inside an
# ssh-invoked PowerShell, and when that parent tree went away the traders went
# with it - the desk lost ten minutes of bars and nothing said so. It is the
# same fault that put fd-api and the watch on SYSTEM scheduled tasks: on this
# machine anything tied to a session or a console dies with it. A WMI-created
# child is parented by WMI and not by sshd.
#
# py\live\start_ai_traders.ps1 -Detached is the pattern; this copies it,
# including the concatenation. PowerShell escapes a quote inside a
# double-quoted string with a BACKTICK, not a backslash, and the backslash form
# parses as a literal backslash that ENDS the string - a fault that only shows
# when the command runs.
$launcherOut = Join-Path $logDir 'sunday-reopen-start_executors.out'
$inner = "Set-Location '$Root'; & powershell -NoProfile -ExecutionPolicy Bypass -File '$launcher' -Account '$Account' -AllowReal -Live *> '$launcherOut'"
$cmd = 'powershell.exe -NoProfile -NoLogo -ExecutionPolicy Bypass -Command "' + $inner + '"'
$create = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = $cmd; CurrentDirectory = $Root }
if ($create.ReturnValue -ne 0) {
    $script:Acted = $false
    Refuse "Win32_Process.Create refused with $($create.ReturnValue). Nothing was stopped and nothing started."
}
$launcherPid = $create.ProcessId
Note "detached launcher pid $launcherPid; its output goes to $launcherOut"

# Wait for the launcher to finish. It sleeps 3 seconds after the kill and 3
# after the starts, so a clean run is ten to twenty seconds; $WaitSec is the
# bound, not the expectation.
$deadline = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $deadline) {
    $still = @(Get-CimInstance Win32_Process -Filter "ProcessId=$launcherPid" -ErrorAction SilentlyContinue)
    if ($still.Count -eq 0) { break }
    Start-Sleep -Seconds 3
}
$stillThere = @(Get-CimInstance Win32_Process -Filter "ProcessId=$launcherPid" -ErrorAction SilentlyContinue)
if ($stillThere.Count -gt 0) {
    Warn "the launcher (pid $launcherPid) is still running after ${WaitSec}s. Verifying anyway; read $launcherOut."
}

Say ''
Say '   --- what the launcher said ---' 'DarkGray'
if (Test-Path $launcherOut) {
    foreach ($line in (Get-Content $launcherOut)) { Say "   | $line" 'DarkGray' }
} else {
    Bad "the launcher wrote no output to $launcherOut. That is itself a failure - read it by hand."
}
Say '   --- end ---' 'DarkGray'

# What the launcher SAYS it started, taken from its own lines rather than
# re-derived. Its skips (a STOP file, a market with no symbol, a book the
# registry does not list) are decisions this script must not duplicate: two
# places computing the expected set is two places to be wrong, and only one of
# them ran the terminals.
$launcherStarted = @()
if (Test-Path $launcherOut) {
    foreach ($line in (Get-Content $launcherOut)) {
        if ($line -match 'mirroring\s+([^/]+)/(\S+)\s+->') { $launcherStarted += $Matches[2] }
    }
}

# ------------------------------------------------------------- 5, the proof
Step '5. the proof - did they come back, adopt, and get layer two?'

$script:FailCount = 0
function Fails { param([string]$What) $script:FailCount++ ; Bad "FAIL  $What" }
function Passes { param([string]$What) Good "ok    $What" }

# 5a - the processes.
Start-Sleep -Seconds 5
$after = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" -ErrorAction SilentlyContinue |
           Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
Say ''
Note "$($after.Count) executor process(es) after the restart; the launcher said it started $($launcherStarted.Count); the registry names $($registryBooks.Count)"
if ($after.Count -eq $launcherStarted.Count -and $after.Count -gt 0) {
    Passes "every book the launcher started has a process ($($after.Count))"
} else {
    Fails "the launcher started $($launcherStarted.Count) and $($after.Count) are running - read $launcherOut and data\live\$Account\<book>\exec.err"
}
if ($launcherStarted.Count -ne $registryBooks.Count) {
    Warn "the registry names $($registryBooks.Count) book(s) and the launcher started $($launcherStarted.Count)."
    $missing = @($registryBooks | Where-Object { $launcherStarted -notcontains $_ })
    foreach ($m in $missing) {
        Warn "  $m was NOT started. The usual causes are a STOP file at data\live\$Account\$m\STOP"
        Warn "  or data\paper\$m\STOP, or no symbol for its market. The launcher's own lines above say which."
    }
} else {
    Passes "the count matches the registry's mirrored books ($($registryBooks.Count))"
}

# Each expected book has exactly one process, by --run and --account. One book
# with two processes is two copies of every order, and the reconciler is
# idempotent against the BOOK and not against a second copy of itself.
foreach ($b in $launcherStarted) {
    $mine = @($after | Where-Object { $_.CommandLine -like "*--run=$b*" -and $_.CommandLine -like "*--account=$Account*" })
    if ($mine.Count -eq 1) { Passes "$b has one executor (pid $($mine[0].ProcessId))" }
    elseif ($mine.Count -eq 0) { Fails "$b has NO executor - read data\live\$Account\$b\exec.err" }
    else { Fails "$b has $($mine.Count) executors. TWO COPIES OF EVERY ORDER. Stop them now." }
}

# 5b - layer two, which is the whole point of the exercise.
#
# THE STRONG CHECK IS THE COMMAND LINE, and it only became possible when the
# cut moved into the registry. py\live\start_executors.ps1 now reads `[prices]
# weekend_flat` itself and appends `--weekend-flat=<value>` to every `$argv`,
# so the number the owner typed in one file is visible on every running
# process and this can compare the two directly.
#
# BEFORE THAT KEY IT COULD NOT. The launcher passed no weekend flag at all,
# the executors ran on argparse's default, the flag appeared nowhere in
# Get-CimInstance, and the best available proof was a chain of inference. That
# chain is kept below, because it is still the only proof available when the
# registry has NO key - which is how an older registry behaves on purpose:
#
#   1  the file on disk declares --weekend-flat, asked of the executor itself
#      through --help, which reaches argparse without importing MetaTrader5;
#   2  the process was created AFTER that file was last written, so it is
#      running that file and not the one it was launched with in September.
#
# So: with a registry key, a missing or different value on a command line is a
# FAILURE. Without one, the chain is all there is and the absence is reported
# as the weaker evidence it is.
Say ''
if ($flat.FromRegistry) {
    Note "expecting --weekend-flat=$WeekendFlat on every command line, from $($flat.Source)"
} else {
    Warn "the cut comes from $($flat.Source), so the launcher passed no flag and the"
    Warn 'command lines cannot be checked against a file. Falling back to the weaker proof:'
    Warn 'the source declares the flag, and each process is newer than the source.'
}
if ($weekendOverride) {
    Warn "-WeekendFlat $weekendOverride was given. It changes what THIS SCRIPT expects and"
    Warn 'nothing else: start_executors.ps1 was run without it and passed whatever the'
    Warn 'registry says. A mismatch below is therefore between you and the file, not a fault.'
}
$helpText = ''
try { $helpText = (& $Python (Join-Path $Root 'py\live\mt5_executor.py') '--help' 2>&1 | Out-String) } catch { $helpText = '' }
if ($helpText -match '--weekend-flat') {
    $declared = ''
    if ($helpText -match '--weekend-flat[\s\S]{0,400}?\(default:\s*([0-9]{2}:[0-9]{2})\)') { $declared = $Matches[1] }
    if ($declared) {
        Passes "mt5_executor.py on disk declares --weekend-flat, default $declared UTC"
        # The default is only the operative value when nothing passes one. With
        # a registry key the two are SUPPOSED to be able to differ - that is
        # what the key buys, a cut that changes in November without touching
        # Python - so comparing them here would manufacture a warning on a
        # correct desk.
        if (-not $flat.FromRegistry -and $declared -ne $WeekendFlat) {
            Warn "nothing passes a value, and the executor's default $declared is not the $WeekendFlat"
            Warn 'this run expects. Put the cut in config\accounts.toml [prices] weekend_flat.'
        }
    } else {
        Passes 'mt5_executor.py on disk declares --weekend-flat (its help text does not name a default)'
    }
} else {
    Fails 'mt5_executor.py on disk has NO --weekend-flat. Layer two does not exist in this checkout,'
    Bad   '      so the restart bought nothing: the account is protected only by the engine''s'
    Bad   '      bar-driven guard, which is the layer that has already been shown to fail silently.'
    Bad   '      git log --oneline -- py/live/mt5_executor.py  and look for d4a5066.'
}
$execMtime = (Get-Item (Join-Path $Root 'py\live\mt5_executor.py')).LastWriteTimeUtc
foreach ($p in $after) {
    $runId = ''
    if ($p.CommandLine -match '--run=([^\s]+)') { $runId = $Matches[1] }
    $startedAt = $null
    try { $startedAt = $p.CreationDate } catch { }
    if ($null -eq $startedAt) {
        Warn "$runId (pid $($p.ProcessId)): no creation time from WMI, so it cannot be proved to be running the current file."
        continue
    }
    $startedUtc = $startedAt.ToUniversalTime()
    if ($startedUtc -gt $execMtime) {
        Passes "$runId started $($startedUtc.ToString('HH:mm:ss'))Z, after mt5_executor.py was last written $($execMtime.ToString('yyyy-MM-dd HH:mm:ss'))Z - it is running the current file"
    } else {
        Fails "$runId started $($startedUtc.ToString('yyyy-MM-dd HH:mm:ss'))Z, BEFORE mt5_executor.py was last written $($execMtime.ToString('yyyy-MM-dd HH:mm:ss'))Z - this process survived the restart and is on the OLD code"
    }
    # The direct check. `--weekend-flat=20:45` is how the launcher writes it;
    # the space form is accepted too, because an executor started by hand is a
    # thing that happens and reads identically to the terminal.
    $onLine = ''
    if ($p.CommandLine -match '--weekend-flat[= ]([^\s]+)') { $onLine = $Matches[1] }
    if ($onLine) {
        if ($onLine -eq $WeekendFlat) {
            Passes "$runId carries --weekend-flat=$onLine, which is what the registry says"
        } else {
            Fails "$runId carries --weekend-flat=$onLine and the expected value is $WeekendFlat. The running process and the file disagree about when the account goes flat."
        }
    } elseif ($flat.FromRegistry) {
        Fails "$runId carries NO --weekend-flat although config\accounts.toml sets it to $WeekendFlat. It is running on argparse's default, so the registry's value is not in force - the launcher on this box is older than the key."
    } else {
        Note "$runId carries no --weekend-flat, as expected with no key in the registry: it is on the executor's own default."
    }
}

# 5c - adoption, per book.
#
# ADOPTION IS SILENT BY CONSTRUCTION, and that is why the check is shaped like
# this. When the book holds and the account holds the same side and size, the
# reconciler's third branch calls drift_from_book, which returns None and logs
# NOTHING: no order is sent, so there is no row to find. The rows that exist
# are the ones that would mean the opposite:
#
#   started        the restart boundary. Every check below is "after this row".
#   joining        the mirror OPENED a position. After a restart on a book that
#                  was already holding, this is a SECOND position - the failure
#                  this whole step exists to catch.
#   order          the order actually sent. `action` says which.
#   not-adopted    the executor refused to adopt: the book opened it too long
#                  ago (--max-adopt-bars) or the price has drifted too far
#                  (--max-join-r). Not a failure of the restart, but the book
#                  will now sit out a live position, which a person must know.
#   already-taken / refused-unknown-history
#                  failing closed while it cannot read the server clock or the
#                  deal history. Expected in the first polls; a worry if it
#                  persists.
#
# So: the `started` row with NO `joining` and no opening `order` after it, and
# the ticket in broker.json unchanged across the restart. Both halves, because
# either alone can be fooled - a silent log by an executor that never started,
# an unchanged ticket by a snapshot nobody rewrote.
Say ''
Say '   adoption, per book:' 'Cyan'
foreach ($b in $launcherStarted) {
    $dir = Join-Path $Root "data\live\$Account\$b"
    $snapPath = Join-Path $dir 'broker.json'
    $jsonlPath = Join-Path $dir 'executor.jsonl'

    # Wait for this executor's FIRST snapshot after the restart. Without it
    # there is nothing to compare: an unchanged broker.json is the old file.
    $waitUntil = (Get-Date).AddSeconds([math]::Min($WaitSec, 90))
    $snap = $null
    while ((Get-Date) -lt $waitUntil) {
        if (Test-Path $snapPath) {
            try { $snap = Get-Content $snapPath -Raw -ErrorAction Stop | ConvertFrom-Json } catch { $snap = $null }
            if ($snap -and [int64]$snap.at -gt $restartMs) { break }
        }
        Start-Sleep -Seconds 5
        $snap = $null
    }
    if (-not $snap) {
        Fails "$($b): no broker.json newer than the restart after 90s. The executor is not writing snapshots - read data\live\$Account\$b\exec.err"
        continue
    }

    # No entry in $before at all is a THIRD state, not "was flat": it means
    # this account has no broker.json for the book, so no mirror has ever run
    # it here and there is nothing this check can compare against. Said out
    # loud, because silently folding it into "was flat" would let a first-ever
    # start report an adoption it never made.
    $hadRecord = $before.ContainsKey($b)
    if (-not $hadRecord) {
        Note "$($b): no broker.json existed before the restart - this account has never mirrored this book, so there was nothing to adopt."
    }
    $wasTicket = $null
    if ($hadRecord) { $wasTicket = $before[$b].Ticket }
    $nowTicket = $null
    if ($snap.position) { $nowTicket = [int64]$snap.position.ticket }

    # The rows this executor wrote since the restart.
    $rows = @()
    if (Test-Path $jsonlPath) {
        foreach ($line in (Get-Content $jsonlPath -ErrorAction SilentlyContinue)) {
            if (-not $line.Trim()) { continue }
            $row = $null
            try { $row = $line | ConvertFrom-Json } catch { continue }
            if ($null -ne $row.time -and [int64]$row.time -ge $restartMs) { $rows += $row }
        }
    }
    $startedRows = @($rows | Where-Object { $_.kind -eq 'started' })
    $joins = @($rows | Where-Object { $_.kind -eq 'joining' })
    $opens = @($rows | Where-Object { $_.kind -eq 'order' -and "$($_.action)" -like 'open *' })
    $closes = @($rows | Where-Object { $_.kind -eq 'order' -and "$($_.action)" -like 'close *' })
    $refusedAdopt = @($rows | Where-Object { $_.kind -eq 'not-adopted' })
    $failedSends = @($rows | Where-Object { $_.kind -eq 'order-failed' -or $_.kind -eq 'order-partial' })
    $unknownHist = @($rows | Where-Object { $_.kind -eq 'refused-unknown-history' -or $_.kind -eq 'already-taken' })
    $refusals = @($rows | Where-Object { $_.kind -eq 'refused' -or $_.kind -eq 'refused-size' -or $_.kind -eq 'refused-margin' })

    if ($startedRows.Count -eq 0) {
        Fails "$($b): no 'started' row in executor.jsonl since the restart. It did not get past the three-key lock - read the 'refused' rows and exec.err."
        foreach ($r in $refusals) { Bad "        refused: $($r.reason)" }
        continue
    }
    $st = $startedRows[-1]
    $stAt = ([datetime]'1970-01-01').AddMilliseconds([int64]$st.time)
    Note "$($b): started row at $($stAt.ToString('HH:mm:ss'))Z - login $($st.login) on $($st.server), $($st.balance) $($st.currency), $($st.symbol), dry_run $($st.dry_run), lot_scale $($st.lot_scale)"
    if ($st.dry_run) {
        Fails "$($b): started with dry_run TRUE. It is reconciling and sending nothing, so the account has no mirror."
    }

    if ($null -ne $wasTicket) {
        # It was holding. The only good outcome is the same ticket.
        if ($null -eq $nowTicket) {
            Fails "$($b): held ticket $wasTicket before the restart and is FLAT now."
            foreach ($c in $closes) { Bad "        closed by: $($c.action) retcode $($c.retcode) at $($c.price)" }
            Bad   '        If no close row is above, the BROKER closed it (a stop or a target through the gap).'
        } elseif ([int64]$nowTicket -eq [int64]$wasTicket) {
            if ($joins.Count -eq 0 -and $opens.Count -eq 0) {
                Passes "$($b): ADOPTED ticket $wasTicket - the 'started' row is the last order-path row it wrote, no 'joining' and no opening 'order' after it, and broker.json still names the same ticket."
            } else {
                Fails "$($b): keeps ticket $wasTicket but ALSO wrote $($joins.Count) 'joining' and $($opens.Count) opening 'order' row(s) since the restart. That is a second position."
                foreach ($j in $joins) { Bad "        joining: $($j.side) $($j.lots) at $($j.filling_at) (book entry $($j.book_entry))" }
                foreach ($o in $opens) { Bad "        order:   $($o.action) retcode $($o.retcode) ticket $($o.ticket) at $($o.price)" }
            }
        } else {
            Fails "$($b): held ticket $wasTicket and now holds $nowTicket. THIS IS A DIFFERENT POSITION - it re-entered instead of adopting."
            foreach ($j in $joins) { Bad "        joining: $($j.side) $($j.lots) at $($j.filling_at) (book entry $($j.book_entry))" }
            foreach ($o in $opens) { Bad "        order:   $($o.action) retcode $($o.retcode) ticket $($o.ticket) at $($o.price)" }
            Bad   '        Close one of them by hand. Two positions on one book is two copies of the trade.'
        }
    } else {
        # It was flat. A position now is a NEW trade, which is the mirror doing
        # its job - and is still named, because a new real-money trade in the
        # first minute of a restart is a thing a person should see rather than
        # find later.
        if ($null -eq $nowTicket) {
            Passes "$($b): flat before, flat now - nothing to adopt."
        } else {
            Warn "$($b): was flat and now holds ticket $nowTicket ($($snap.position.side) $($snap.position.lots) from $($snap.position.entry_price))."
            Warn '  That is a NEW entry, not an adoption. The book signalled and the mirror followed.'
            foreach ($j in $joins) { Warn "        joining: $($j.side) $($j.lots) at $($j.filling_at) (book entry $($j.book_entry), drift $($j.drift_r)R)" }
        }
    }

    foreach ($n in $refusedAdopt) {
        Warn "$($b): not-adopted - $($n.reason)"
        Warn '  The book holds and this account will not follow it. It sits out until the book goes flat.'
    }
    foreach ($u in $unknownHist) {
        Note "$($b): $($u.kind) - failing closed until the server clock or the deal history comes back. Expected for the first polls."
    }
    foreach ($f in $failedSends) {
        Fails "$($b): $($f.kind) - $($f.action) retcode $($f.retcode) $($f.error)"
    }
    if ($snap.blocked) { Fails "$($b): blocked - $($snap.blocked)" }
    if ($snap.drift) { Fails "$($b): drift - $($snap.drift)" }
    if ($snap.standing_out) { Note "$($b): standing out - $($snap.standing_out)" }
}

Say ''
if ($script:FailCount -eq 0) {
    Say "All checks passed. $($after.Count) executor(s) on $Account, layer two in force from $WeekendFlat UTC on Friday." 'Green'
    Say 'Next Friday the account flattens twice over: the engine at 1640 New York on the' 'Green'
    Say '20:30Z bar, and the executor''s wall clock at 20:45Z whatever the feed does.' 'Green'
} else {
    Say "$($script:FailCount) CHECK(S) FAILED. The mirrors ARE running - this did not undo anything." 'Red'
    Say 'Read the FAIL lines above. Nothing here stops an executor; that is a decision too:' 'Red'
    Say ("  powershell -NoProfile -File py\live\start_executors.ps1 -Account $Account -DryRun") 'Red'
    Say ("  or drop data\live\$Account\<book>\STOP to stop one book on this account.") 'Red'
}
Say ''
Say 'STILL NEEDS A HUMAN, whatever the checks said:' 'Cyan'
Say '  - the two positions the weekend was entered with: adopted is not the same as wanted.' 'Cyan'
Say '  - the November DST change: --weekend-flat becomes 21:45, in start_executors.ps1 and' 'Cyan'
Say '    in this script''s -WeekendFlat on the same day. See the dated trap in the decision note.' 'Cyan'
Say "This run is written to $script:LogPath" 'DarkGray'
if ($script:FailCount -gt 0) { exit 1 }
exit 0
