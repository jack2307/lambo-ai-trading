# Drive sunday-reopen.ps1's refusals with injected clocks and quote ages.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\sunday-reopen-selftest.ps1
#
# The runbook cannot be tested against the VPS - it talks to fd-api, to WMI and
# to a MetaTrader account with money in it, none of which exist here. So the
# two things it refuses ON are functions taking a time and a quote age, and
# this drives them. Nothing below reaches a network, a process or a file.
#
# It DOT-SOURCES the real script with -DefineOnly, so what is under test is the
# file that runs and not a copy of its logic - the same arrangement as
# -PlanOnly in deploy\start-desk-selftest.ps1, and the same reason: a selftest
# that re-implements the rule passes on the day the rule changes.
#
# The window table is deliberately the SAME table as section 13 of
# py\live\size_guard_selftest.py, instant for instant. Test-InWeekendWindow is
# a transcription of the executor's `in_weekend_window`, and the only way that
# stays true is for both to be pinned against the same row of hours. If one of
# these two files is ever edited alone, this is where it shows.
#
# What it pins, in the order the failures would matter:
#
#   1  the cut parses, and REFUSES what it cannot read rather than defaulting
#      to 20:45 - the same class of failure the backstop exists to catch.
#   2  the window, minute by minute: Friday `>=` the cut, all Saturday, Sunday
#      `<` 21:00 UTC. Both boundaries face the way the executor's do.
#   3  THE HOLIDAY CASE. A live poller posting a frozen weekend quote must
#      fail: the live bar is one second old and the last quote CHANGE is two
#      days old, and only the second of those says whether the market is open.
#      This is the check a clock cannot do and the reason the script asks twice.
#   4  the clock refuses BEFORE the feed, and -AllowStaleFeed overrides the
#      feed and never the clock.
#   5  ATR(14) on a series whose answer is known by hand.
#   6  the gap is found in the bars: the most recent 48-hour hole, not the
#      one-hour daily break, and signed the way price moved.
#   7  THE CUT COMES FROM THE REGISTRY. `[prices] weekend_flat` is read, an
#      absent key falls back to the executor's own default and says so, and a
#      malformed one REFUSES - loudly, with a non-zero exit, before any
#      executor is stopped. A backstop that silently reverts to a default is
#      the failure the whole decision note is about, so "it refuses" is the
#      property under test and not the parsing.

param(
    [string]$Script = '',
    [string]$Python = 'C:\Python39\python.exe',
    # Resolved in the body and NOT here. $PSScriptRoot is not reliably
    # populated while defaults are being evaluated - py\live\start_executors.ps1
    # carries the same note - and this file proved it the hard way: the default
    # came out empty, Join-Path threw, and section 7b was skipped in silence
    # while the run still printed "all green".
    [string]$Root = ''
)

$ErrorActionPreference = 'Continue'
$script:Fail = 0
$script:Ran = 0
function Check {
    param([string]$Name, [bool]$Ok, [string]$Detail = '')
    $script:Ran++
    if ($Ok) {
        Write-Host "  ok   $Name" -ForegroundColor DarkGray
    } else {
        $script:Fail++
        $line = "  FAIL $Name"
        if ($Detail) { $line += "   ($Detail)" }
        Write-Host $line -ForegroundColor Red
    }
}
function Section { param([string]$Name) Write-Host "`n== $Name" -ForegroundColor Cyan }

$here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
if (-not $Root) { $Root = Split-Path -Parent $here }
if (-not $Script) { $Script = Join-Path $here 'sunday-reopen.ps1' }

# EVERYTHING THIS FILE STILL NEEDS AFTER THE DOT-SOURCE IS COPIED ASIDE FIRST.
#
# Dot-sourcing a script runs its PARAM BLOCK in this scope, not only its
# functions - so `. sunday-reopen.ps1 -DefineOnly` rebinds every parameter it
# declares, and it declares `$Root = ''` and `$Python`. Measured here: section
# 7b's `Join-Path $Root` threw "argument is an empty string", the section was
# skipped, and the summary still printed "all green". It is the same class of
# collision as `$sat` overwriting `$SAT` below, from a different direction, and
# it is silent in exactly the same way.
$script:RepoRoot = $Root
$script:PythonExe = $Python
if (-not (Test-Path $Script)) {
    Write-Host "no script at $Script" -ForegroundColor Red
    exit 2
}
# Dot-sourced, so the functions land in THIS scope and so does
# $script:REOPEN_MIN_UTC, which Test-InWeekendWindow reads. -DefineOnly returns
# before the body, so nothing is started, nothing is read and nothing is sent.
. $Script -DefineOnly

# A UTC instant on a known week. 2026-09-17 is a Thursday, so Friday is the
# 18th - the Friday of the incident this whole runbook comes from - Saturday
# the 19th, Sunday the 20th and Monday the 21st.
function At { param([int]$Day, [int]$Hour, [int]$Minute, [int]$Second = 0)
    return (New-Object datetime (2026, 9, $Day, $Hour, $Minute, $Second, [System.DateTimeKind]::Utc))
}
$THU = 17; $FRI = 18; $SAT = 19; $SUN = 20; $MON = 21

# ---------------------------------------------------------------------------
Section '1 - the cut parses, or refuses'

$CUT = Get-WeekendCutMinute '20:45'
Check '--weekend-flat 20:45 is minute 1245 UTC' ($CUT -eq 1245) "$CUT"
Check '00:00 is a time and not a falsy off' ((Get-WeekendCutMinute '00:00') -eq 0)
Check '23:59 is minute 1439' ((Get-WeekendCutMinute '23:59') -eq 1439)
Check "'off' disables it" ($null -eq (Get-WeekendCutMinute 'off'))
Check "'OFF' too, whatever the case" ($null -eq (Get-WeekendCutMinute 'OFF'))
Check "'none' disables it" ($null -eq (Get-WeekendCutMinute 'none'))
Check 'an empty value disables it' ($null -eq (Get-WeekendCutMinute ''))
Check 'whitespace around it does not matter' ((Get-WeekendCutMinute '  20:45 ') -eq 1245)

# A mistyped value must NOT quietly become the default. Rule 3 of
# docs\decisions\2026-09-17-unit-carrying.md: a conversion that cannot be
# established refuses, it does not fall back.
foreach ($bad in @('2045', '20:45:00', '24:00', '20:60', 'half past eight', '-1:00', '20:xx')) {
    $threw = $false
    try { Get-WeekendCutMinute $bad | Out-Null } catch { $threw = $true }
    Check "'$bad' throws rather than quietly becoming 20:45" $threw
}

# ---------------------------------------------------------------------------
Section '2 - the window, minute by minute, in UTC'

$table = @(
    @{ d = $THU; h = 20; m = 45; want = $false; why = 'Thursday at the same minute is a trading night' },
    @{ d = $THU; h = 23; m = 59; want = $false; why = 'Thursday midnight is not the weekend' },
    @{ d = $FRI; h = 0;  m = 0;  want = $false; why = 'Friday morning' },
    @{ d = $FRI; h = 20; m = 44; want = $false; why = 'Friday one minute before the cut' },
    @{ d = $FRI; h = 20; m = 44; s = 59; want = $false; why = '...and 59 seconds, still before it' },
    @{ d = $FRI; h = 20; m = 45; want = $true;  why = 'Friday exactly on the cut' },
    @{ d = $FRI; h = 20; m = 46; want = $true;  why = 'Friday after the cut' },
    @{ d = $FRI; h = 23; m = 59; want = $true;  why = 'Friday midnight' },
    @{ d = $SAT; h = 0;  m = 0;  want = $true;  why = 'Saturday opens' },
    @{ d = $SAT; h = 12; m = 0;  want = $true;  why = 'Saturday midday' },
    @{ d = $SAT; h = 20; m = 44; want = $true;  why = "Saturday at a minute that is inside Friday's cut" },
    @{ d = $SAT; h = 23; m = 59; want = $true;  why = 'Saturday closes' },
    @{ d = $SUN; h = 0;  m = 0;  want = $true;  why = 'Sunday morning' },
    @{ d = $SUN; h = 20; m = 59; want = $true;  why = 'Sunday one minute before the reopen' },
    @{ d = $SUN; h = 21; m = 0;  want = $false; why = 'Sunday exactly at the reopen' },
    @{ d = $SUN; h = 21; m = 0; s = 30; want = $false; why = '...and half a minute past it' },
    @{ d = $SUN; h = 23; m = 59; want = $false; why = 'Sunday night, the week is running' },
    @{ d = $MON; h = 0;  m = 0;  want = $false; why = 'Monday' },
    @{ d = $MON; h = 20; m = 45; want = $false; why = 'Monday at the same minute as the cut' }
)
foreach ($row in $table) {
    $sec = 0
    if ($row.ContainsKey('s')) { $sec = $row.s }
    $when = At $row.d $row.h $row.m $sec
    $got = Test-InWeekendWindow -NowUtc $when -CutMin $CUT
    $word = 'out'
    if ($row.want) { $word = 'in' }
    Check "$($when.ToString('ddd HH:mm'))Z $($word): $($row.why)" ($got -eq $row.want) "got $got"
}

# Off is off on every one of them, which is the property the flag sells.
$anyOn = $false
foreach ($d in @($FRI, $SAT, $SUN, $MON)) {
    for ($h = 0; $h -lt 24; $h++) {
        if (Test-InWeekendWindow -NowUtc (At $d $h 0) -CutMin $null) { $anyOn = $true }
    }
}
Check 'with the cut off no instant is in the window' (-not $anyOn)

# A different cut moves the FRIDAY boundary and nothing else - which is what
# the November DST change will do, and the reason it is one typed value.
$early = Get-WeekendCutMinute '21:45'
Check 'a 21:45 cut takes Friday from 21:45Z' (
    (Test-InWeekendWindow -NowUtc (At $FRI 21 45) -CutMin $early) -and
    (-not (Test-InWeekendWindow -NowUtc (At $FRI 21 44) -CutMin $early)))
Check '...and leaves Saturday and the Sunday reopen exactly where they were' (
    (Test-InWeekendWindow -NowUtc (At $SAT 3 0) -CutMin $early) -and
    (-not (Test-InWeekendWindow -NowUtc (At $SUN 21 0) -CutMin $early)))

# ---------------------------------------------------------------------------
Section '3 - the feed: a clock is not a tape'

$NOW = 1000000000000   # an arbitrary epoch-ms "now"; only the differences matter
$SEC = 1000
$MAX = 180

# THE HOLIDAY CASE, and the reason this script asks two questions.
#
# py\live\start_pollers.ps1 starts every poller with --tick-poll=1, so it
# offers the API a quote once a second whatever the market is doing. Over a
# weekend MetaTrader hands back the same frozen quote and the poller posts it
# anyway: /api/paper/status's live bar is one second old all weekend long.
# /api/paper/m1's last_tick_ms is the last accepted quote CHANGE and its own
# doc comment says a duplicate does not advance it, so that one still points at
# Friday. A script that trusted the live bar would call a holiday Sunday open.
$frozen = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 48 * 3600 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check 'a live poller posting a FROZEN weekend quote is refused' (-not $frozen.Ok) $frozen.Reason
Check '...and the reason names the quote change, not the poller' ($frozen.Reason -like '*quote CHANGE*')

$open = Get-FeedVerdict -LiveAtMs ($NOW - 2 * $SEC) -LastTickMs ($NOW - 3 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check 'a running tape passes' $open.Ok $open.Reason
Check '...and reports both ages' (($open.TickAgeSec -eq 3) -and ($open.LiveAgeSec -eq 2))

# The threshold itself, from both sides. The populations it separates are three
# orders of magnitude apart, so the exact value is not delicate - but it has to
# be APPLIED, and an off-by-one here is a script that never refuses.
$justIn = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 179 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
$justOut = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 181 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check '179s inside a 180s threshold passes' $justIn.Ok
Check '181s outside it refuses' (-not $justOut.Ok) $justOut.Reason

# A dead poller: the API drops a live bar older than 90s, so null is its own
# verdict and not this script's.
$noPoller = Get-FeedVerdict -LiveAtMs $null -LastTickMs ($NOW - 5 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check 'no live bar on /api/paper/status refuses even with a recent tick' (-not $noPoller.Ok) $noPoller.Reason

# A fresh fd-api that has never been posted to at all.
$never = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs $null -NowMs $NOW -MaxAgeSec $MAX
Check 'no last_tick_ms at all refuses' (-not $never.Ok) $never.Reason
$unavail = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs $null -NowMs $NOW -MaxAgeSec $MAX -Unavailable 'no ticks yet'
Check "the ring's own 'unavailable' sentence is carried through" (
    (-not $unavail.Ok) -and ($unavail.Reason -like '*no ticks yet*')) $unavail.Reason

# Clock skew. When the two clocks disagree no age computed here means anything,
# INCLUDING the age that would otherwise have said fresh - so this refuses
# rather than passing on a number it cannot trust.
$future = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW + 600 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check 'a quote from the FUTURE refuses rather than reading as fresh' (-not $future.Ok) $future.Reason
Check '...and says the clocks disagree' ($future.Reason -like '*disagree*')
$skew = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 2 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
Check 'a couple of seconds of jitter is not called skew' $skew.Ok

# ---------------------------------------------------------------------------
Section '4 - the verdict: the clock first, and only the feed can be overridden'

$goodFeed = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 2 * $SEC) -NowMs $NOW -MaxAgeSec $MAX
$deadFeed = Get-FeedVerdict -LiveAtMs ($NOW - 1 * $SEC) -LastTickMs ($NOW - 48 * 3600 * $SEC) -NowMs $NOW -MaxAgeSec $MAX

# The verdicts are named `v...` and not `$sat` / `$mon`, and this is not
# style. POWERSHELL VARIABLE NAMES ARE CASE-INSENSITIVE: the first draft
# assigned `$sat = Get-ReopenVerdict ...` and thereby overwrote the day
# constant `$SAT = 19`, so the next `At $SAT 12 0` was handed a PSCustomObject
# where an int was wanted. It failed loudly here, which is luck - the same
# collision between `$mon` and `$MON` would have moved an instant rather than
# thrown, and a green table would have been pinning the wrong days.
$vSaturday = Get-ReopenVerdict -NowUtc (At $SAT 12 0) -CutMin $CUT -Feed $goodFeed
Check 'Saturday refuses even with a perfect feed' (-not $vSaturday.Ok) $vSaturday.Reason
Check '...on the clock, named as such' ($vSaturday.Refusal -eq 'weekend-window')

$vSundayEarly = Get-ReopenVerdict -NowUtc (At $SUN 20 59) -CutMin $CUT -Feed $goodFeed
Check 'Sunday 20:59Z still refuses' (-not $vSundayEarly.Ok) $vSundayEarly.Reason

# THE OVERRIDE DOES NOT REACH THE CLOCK. An executor started inside its own
# weekend window closes every position it finds into a shut market and retries
# at every poll, which is precisely the failure the restart was deferred to
# avoid - so there is nothing to override.
$vSaturdayForced = Get-ReopenVerdict -NowUtc (At $SAT 12 0) -CutMin $CUT -Feed $goodFeed -AllowStaleFeed
Check '-AllowStaleFeed does NOT get past Saturday' (-not $vSaturdayForced.Ok) $vSaturdayForced.Reason
Check '...and is still refused on the clock' ($vSaturdayForced.Refusal -eq 'weekend-window')

$vHoliday = Get-ReopenVerdict -NowUtc (At $SUN 21 30) -CutMin $CUT -Feed $deadFeed
Check 'a holiday Sunday - past the reopen, no tape - refuses' (-not $vHoliday.Ok) $vHoliday.Reason
Check '...on the feed' ($vHoliday.Refusal -eq 'feed')

$vHolidayForced = Get-ReopenVerdict -NowUtc (At $SUN 21 30) -CutMin $CUT -Feed $deadFeed -AllowStaleFeed
Check '-AllowStaleFeed gets past a dead feed once the clock is clear' $vHolidayForced.Ok
Check '...and the log says the check FAILED and was overridden' (
    ($vHolidayForced.Refusal -eq 'overridden') -and ($vHolidayForced.Reason -like '*FAILED*')) $vHolidayForced.Reason

$vReopen = Get-ReopenVerdict -NowUtc (At $SUN 21 1) -CutMin $CUT -Feed $goodFeed
Check 'Sunday 21:01Z with a running tape proceeds' $vReopen.Ok $vReopen.Reason
$vMidweek = Get-ReopenVerdict -NowUtc (At $MON 9 0) -CutMin $CUT -Feed $goodFeed
Check 'a mid-week restart proceeds' $vMidweek.Ok

# The day constants must still be integers here. If a future edit reintroduces
# the collision above, this is the line that says so in one word rather than
# leaving a whole table quietly testing the wrong instants.
Check 'the day constants survived this section (no case-insensitive collision)' (
    ($THU -is [int]) -and ($FRI -is [int]) -and ($SAT -is [int]) -and
    ($SUN -is [int]) -and ($MON -is [int]))

# ---------------------------------------------------------------------------
Section '5 - ATR(14), Wilder'

# A series with a constant true range of exactly 2.0: every bar is 2 wide and
# closes in the middle of the next, so the seed average and every smoothed
# value are 2.0. If the smoothing or the seed is wrong this is not 2.
$flat = @()
for ($i = 0; $i -lt 40; $i++) { $flat += ,@((1000 + $i), 100.0, 101.0, 99.0, 100.0) }
$a = Get-AtrWilder -Bars $flat -Period 14
Check 'a constant 2.0 true range gives ATR 2.0' ([math]::Abs($a - 2.0) -lt 1e-9) "$a"

Check 'fewer bars than the period + 1 gives $null, not a short-window answer' (
    $null -eq (Get-AtrWilder -Bars @($flat[0..13]) -Period 14))
Check 'exactly period + 1 bars is enough' (
    $null -ne (Get-AtrWilder -Bars @($flat[0..14]) -Period 14))

# One wide bar must LIFT the average and not replace it: Wilder's smoothing
# gives the newest bar weight 1/14, so a single 30-point bar on a 2.0 series
# moves the answer by (30 - 2) / 14 = 2.0, to 4.0 exactly.
$spike = @($flat[0..29])
$spike += ,@(1030, 100.0, 115.0, 85.0, 100.0)
$b = Get-AtrWilder -Bars $spike -Period 14
Check 'one 30-point bar lifts a 2.0 ATR to exactly 4.0 (weight 1/14)' ([math]::Abs($b - 4.0) -lt 1e-9) "$b"

# ---------------------------------------------------------------------------
Section '6 - the weekend gap, found in the bars and not on a calendar'

# 15-minute bars: a session, a one-hour daily break, another session, a
# 48-hour hole, then the new week five points lower.
$M15 = 900000
$bars = @()
$t = 0
for ($i = 0; $i -lt 40; $i++) { $bars += ,@($t, 4380.0, 4381.0, 4379.0, 4380.0); $t += $M15 }
$t += 3600000                                    # the broker's daily break, one hour
for ($i = 0; $i -lt 40; $i++) { $bars += ,@($t, 4387.0, 4388.0, 4386.0, 4387.0); $t += $M15 }
$fridayCloseMs = $t - $M15
$t += 48 * 3600000                               # the weekend
$bars += ,@($t, 4382.0, 4383.0, 4381.0, 4382.0)  # the new week opens five points lower

$gap = Get-WeekendGap -Bars $bars
Check 'the hole is found' $gap.Found
Check 'it is the 48-hour one and not the one-hour daily break' ($gap.HoleHours -eq 48.25) "$($gap.HoleHours)"
Check "Friday's close is the bar before it" ($gap.PrevClose -eq 4387.0) "$($gap.PrevClose)"
Check '...at the right instant' ($gap.PrevCloseMs -eq $fridayCloseMs) "$($gap.PrevCloseMs) vs $fridayCloseMs"
Check "the new week's open is the bar after it" ($gap.NextOpen -eq 4382.0) "$($gap.NextOpen)"
Check 'the gap is signed the way price moved: down five points' ($gap.GapPoints -eq -5.0) "$($gap.GapPoints)"

# A gap UP is positive, which is the half that pays a long.
$up = @($bars[0..($bars.Count - 2)])
$up += ,@($t, 4400.0, 4401.0, 4399.0, 4400.0)
$gapUp = Get-WeekendGap -Bars $up
Check 'a gap up is positive' ($gapUp.GapPoints -eq 13.0) "$($gapUp.GapPoints)"

# The MOST RECENT hole, when a window happens to contain two. Scanning forwards
# would answer with last week's gap on any window longer than a week.
$two = @()
$t2 = 0
for ($i = 0; $i -lt 10; $i++) { $two += ,@($t2, 4300.0, 4301.0, 4299.0, 4300.0); $t2 += $M15 }
$t2 += 48 * 3600000
for ($i = 0; $i -lt 10; $i++) { $two += ,@($t2, 4350.0, 4351.0, 4349.0, 4350.0); $t2 += $M15 }
$t2 += 48 * 3600000
$two += ,@($t2, 4370.0, 4371.0, 4369.0, 4370.0)
$gapTwo = Get-WeekendGap -Bars $two
Check 'with two holes in the window it answers with the most recent' ($gapTwo.PrevClose -eq 4350.0) "$($gapTwo.PrevClose)"

# No hole at all is an answer, not an exception: a mid-week run has none.
$none = @($bars[0..39])
Check 'a window with no hole reports Found = false rather than guessing' (
    -not (Get-WeekendGap -Bars $none).Found)
Check 'a single bar does not throw' (-not (Get-WeekendGap -Bars @($bars[0])).Found)

# ---------------------------------------------------------------------------
Section '7a - where the cut comes from, with the registry injected'

# The resolver, driven with the object `accounts.py --prices` would have
# produced. No file, no Python: this is the decision, not the parsing.
$regSet = Resolve-WeekendFlat -Override '' -Prices ([pscustomobject]@{ weekend_flat = '21:45' })
Check 'a registry value is used' ($regSet.Value -eq '21:45') $regSet.Value
Check '...and is reported as coming FROM the registry' $regSet.FromRegistry
Check '...and the source names the file and the key' ($regSet.Source -like '*accounts.toml*weekend_flat*') $regSet.Source

# ABSENT IS NOT OFF and is not an error: the launcher passes nothing and the
# executor keeps argparse's own default, which is how every registry written
# before this key existed behaves.
$regNone = Resolve-WeekendFlat -Override '' -Prices ([pscustomobject]@{ terminal = 'x' })
Check 'an absent key falls back to the executor default' ($regNone.Value -eq '20:45') $regNone.Value
Check '...and does NOT claim to come from the registry' (-not $regNone.FromRegistry)
Check "...and says so in words, so nobody edits a key that is not there" (
    $regNone.Source -like "*executor's own default*") $regNone.Source

$regEmpty = Resolve-WeekendFlat -Override '' -Prices ([pscustomobject]@{ weekend_flat = '' })
Check 'an empty value reads as absent, not as off' (
    ($regEmpty.Value -eq '20:45') -and (-not $regEmpty.FromRegistry)) $regEmpty.Value

$regOff = Resolve-WeekendFlat -Override '' -Prices ([pscustomobject]@{ weekend_flat = 'off' })
Check "the word 'off' is carried through" ($regOff.Value -eq 'off') $regOff.Value
Check '...and turns the window off entirely' (
    $null -eq (Get-WeekendCutMinute $regOff.Value))

$regOver = Resolve-WeekendFlat -Override '22:00' -Prices ([pscustomobject]@{ weekend_flat = '20:45' })
Check 'the override beats the registry' ($regOver.Value -eq '22:00') $regOver.Value
Check '...and says it is ignoring the registry' ($regOver.Source -like '*override*') $regOver.Source

# A registry that could not be read at all is a THIRD answer, and the caller
# refuses on it. Defaulting to 20:45 here would mean checking the executors
# against a number this script invented, which is the one thing step 5 must
# never do.
$regGone = Resolve-WeekendFlat -Override '' -Prices $null
Check 'an unreadable registry is Unresolved rather than defaulted' $regGone.Unresolved

# A malformed value must throw wherever it arrives from.
foreach ($badVal in @('20.45', '2045', '25:00', 'saturday')) {
    $threwReg = $false
    try { Resolve-WeekendFlat -Override '' -Prices ([pscustomobject]@{ weekend_flat = $badVal }) | Out-Null } catch { $threwReg = $true }
    Check "a registry value of '$badVal' throws rather than resolving" $threwReg
    $threwOv = $false
    try { Resolve-WeekendFlat -Override $badVal -Prices $null | Out-Null } catch { $threwOv = $true }
    Check "an override of '$badVal' throws too - same standard for both" $threwOv
}

# ---------------------------------------------------------------------------
Section '7b - the registry itself, through the real accounts.py'

# WITHOUT A BOM, and this is not a detail. `Set-Content -Encoding UTF8` on
# Windows PowerShell 5.1 writes EF BB BF at the start of the file and `tomli`
# refuses it with "Invalid statement (at line 1, column 1)" - so every registry
# written that way is unparseable, every case refuses, and the cases that
# ASSERT a refusal pass while proving nothing. deploy\start-desk-selftest.ps1
# learned this on 2026-09-18 by reading the bytes back.
function Write-Registry { param([string]$Path, [string]$Body)
    [System.IO.File]::WriteAllText($Path, $Body, (New-Object System.Text.UTF8Encoding($false)))
}

$accountsPy = Join-Path $script:RepoRoot 'py\live\accounts.py'
# A SKIP IS COUNTED AND PRINTED, never silent. The first run of this section
# skipped itself through a broken $Root and the summary still said "all green" -
# which is a selftest lying in the one direction that matters.
$script:Skipped = 0
if (-not (Test-Path $script:PythonExe) -or -not (Test-Path $accountsPy)) {
    $script:Skipped++
    Write-Host "  SKIPPED: no $($script:PythonExe) or no $accountsPy on this machine." -ForegroundColor Yellow
    Write-Host '  These checks need the real reader; the injected ones in 7a still ran.' -ForegroundColor Yellow
} else {
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("sunday-reopen-selftest-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        function Read-Prices { param([string]$Body)
            $p = Join-Path $tmp 'accounts.toml'
            Write-Registry $p $Body
            $out = & $script:PythonExe $accountsPy '--file' $p '--prices' 2>&1 | Out-String
            return @{ text = $out; code = $LASTEXITCODE }
        }
        # A terminal is required by the reader, so every body below carries one.
        $head = "[prices]`nterminal = 'C:\MT5-cent\terminal64.exe'`nsymbol_suffix = `".sc`"`n"

        $r1 = Read-Prices ($head + "weekend_flat = `"20:45`"`n")
        Check 'accounts.py --prices returns the key' (
            ($r1.code -eq 0) -and ((($r1.text | ConvertFrom-Json).weekend_flat) -eq '20:45')) $r1.text

        $r2 = Read-Prices $head
        Check 'an absent key returns the empty string, meaning "pass nothing"' (
            ($r2.code -eq 0) -and ((($r2.text | ConvertFrom-Json).weekend_flat) -eq '')) $r2.text

        $r3 = Read-Prices ($head + "weekend_flat = `"off`"`n")
        Check "'off' survives the round trip" (
            ($r3.code -eq 0) -and ((($r3.text | ConvertFrom-Json).weekend_flat) -eq 'off')) $r3.text

        # Canonicalised, so two spellings of one instant cannot read as two
        # settings in two logs.
        $r4 = Read-Prices ($head + "weekend_flat = `"8:5`"`n")
        Check '"8:5" is canonicalised to "08:05"' (
            ($r4.code -eq 0) -and ((($r4.text | ConvertFrom-Json).weekend_flat) -eq '08:05')) $r4.text

        # THE PROPERTY THIS SECTION EXISTS FOR. A malformed value must stop the
        # launcher, not become 20:45 behind everybody's back.
        foreach ($badToml in @('weekend_flat = "20.45"', 'weekend_flat = "2045"',
                               'weekend_flat = "25:00"', 'weekend_flat = "20:61"',
                               'weekend_flat = 2045', 'weekend_flat = true')) {
            $rb = Read-Prices ($head + $badToml + "`n")
            Check "$badToml exits non-zero rather than falling back" ($rb.code -ne 0) "code $($rb.code): $($rb.text)"
            Check "...and says which key and which value" (
                ($rb.text -like '*weekend_flat*')) $rb.text
        }

        # And the reader still refuses what it always refused, so this key has
        # not weakened the one next to it.
        $rNoTerm = Read-Prices "[prices]`nweekend_flat = `"20:45`"`n"
        Check 'a [prices] with no terminal still refuses' ($rNoTerm.code -ne 0) $rNoTerm.text
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------
Write-Host ''
if ($script:Skipped -gt 0) {
    Write-Host "$($script:Skipped) section(s) were SKIPPED - the count below does not cover them." -ForegroundColor Yellow
}
if ($script:Fail -eq 0) {
    Write-Host "$($script:Ran) checks, all green." -ForegroundColor Green
    exit 0
}
Write-Host "$($script:Fail) of $($script:Ran) checks FAILED." -ForegroundColor Red
exit 1
