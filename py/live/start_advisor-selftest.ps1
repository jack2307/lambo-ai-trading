# Exercise start_advisor.ps1 without starting, stopping or calling anything.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File py\live\start_advisor-selftest.ps1
#
# It runs the REAL launcher with -PlanOnly, so what is tested is the file that
# runs rather than a copy of its logic, and it reads start-desk.ps1's own
# syntax tree for the one property that cannot be tested by running anything.
#
# WHAT IT PINS
#
#   1  -PlanOnly reaches no side effect. Proved from the SYNTAX TREE, not by
#      observing that a run happened to be harmless: every Stop-Process,
#      Start-Process, Move-Item and New-Item must sit after the line -PlanOnly
#      exits on. This check is what makes running -PlanOnly safe at all, and
#      the launcher it guards is one nobody on this desk is allowed to run
#      casually - so the guard is asserted rather than assumed.
#   2  start-desk.ps1 starts the panel, and starts it WITHOUT -Apply. A reboot
#      may start the advisor; a reboot may never start it applying verdicts to
#      real entries. That is a claim made in a comment in start-desk.ps1, and a
#      claim in a comment is not enforced by anything.
#   3  a bad agent name refuses with exit 1 - and refuses in the plan, which is
#      BEFORE the kill. A typo found after the kill costs the running panel.
#   4  -Apply says out loud that it reaches real orders, so the warning cannot
#      be quietly softened into something nobody reads.
#   5  no python process appears or disappears across the whole selftest.
#
# WHAT IT DOES NOT PIN, stated so a green run is not read as more than it is:
# nothing here proves the panel works. It never calls a model, never starts
# advisor.py, and never touches fd-api. `check_clis.py` is not even reached,
# because -PlanOnly returns above it. A green selftest means the launcher's
# plan and its refusals are right; the first consultation in advisor.out is
# still the only thing that proves the panel answers.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$launcher = Join-Path $root 'py\live\start_advisor.ps1'
$startDesk = Join-Path $root 'deploy\start-desk.ps1'

$fails = 0
function Check($name, $ok, $detail = '') {
    if ($ok) {
        Write-Host ("  PASS  " + $name) -ForegroundColor Green
    } else {
        Write-Host ("  FAIL  " + $name) -ForegroundColor Red
        if ($detail) { Write-Host ("        " + $detail) -ForegroundColor DarkGray }
        $script:fails++
    }
}
function Parse($path) {
    $t = $null; $e = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$t, [ref]$e)
    if ($e.Count) { throw ("parse errors in " + $path + ": " + ($e[0].Message)) }
    return $ast
}

$pythonBefore = @(Get-CimInstance Win32_Process -Filter "name='python.exe'").Count

# 1 -------------------------------------------------- -PlanOnly is a dead end
Write-Host "`n-PlanOnly reaches no side effect"
$ast = Parse $launcher
$guardEnd = ($ast.FindAll({ param($n)
        $n -is [System.Management.Automation.Language.IfStatementAst] -and $n.Extent.Text -match '\$PlanOnly'
    }, $true) | ForEach-Object { $_.Extent.EndLineNumber } | Measure-Object -Maximum).Maximum
Check '-PlanOnly guard exists' ($null -ne $guardEnd -and $guardEnd -gt 0)

$sideEffects = @('Stop-Process', 'Start-Process', 'Move-Item', 'New-Item', 'Remove-Item')
$early = @($ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true) |
    Where-Object { $sideEffects -contains $_.GetCommandName() } |
    Where-Object { $_.Extent.StartLineNumber -le $guardEnd })
Check 'every side effect sits after the -PlanOnly exit' ($early.Count -eq 0) `
    (($early | ForEach-Object { $_.GetCommandName() + ' at line ' + $_.Extent.StartLineNumber }) -join '; ')

# `$args` is an automatic variable; assigning to it works at script scope and
# silently does not inside a function. This repo has paid for that once.
$argsAssign = @($ast.FindAll({ param($n)
        $n -is [System.Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -eq '$args'
    }, $true))
Check 'nothing assigns to the automatic $args' ($argsAssign.Count -eq 0)

# 2 ------------------------------------------- a reboot cannot start it applied
Write-Host "`nstart-desk.ps1 starts the panel, and never applying"
$deskText = (Get-Content $startDesk -Raw)
$deskAst = Parse $startDesk
$entries = @($deskAst.FindAll({ param($n)
        $n -is [System.Management.Automation.Language.HashtableAst] -and $n.Extent.Text -match 'start_advisor\.ps1'
    }, $true))
Check 'start-desk.ps1 has a launcher entry for start_advisor.ps1' ($entries.Count -eq 1) `
    ("found " + $entries.Count)
if ($entries.Count -eq 1) {
    $entry = $entries[0].Extent.Text
    # The entry's own text, not the whole file: -Apply may legitimately appear
    # in a comment elsewhere, and matching the file would pass or fail for the
    # wrong reason.
    Check 'that entry passes no -Apply' (-not ($entry -match '-Apply')) $entry
    Check 'that entry passes no arguments at all' ($entry -match 'args\s*=\s*@\(\s*\)') $entry
}
# And the panel is started after the traders it advises.
$iTraders = $deskText.IndexOf("start_ai_traders.ps1'; args")
$iAdvisor = $deskText.IndexOf("start_advisor.ps1'; args")
Check 'the panel is launched after the traders' ($iTraders -gt 0 -and $iAdvisor -gt $iTraders)

# 3 ------------------------------------------------ refusals happen in the plan
Write-Host "`nrefusals land before anything is stopped"
$null = & powershell -NoProfile -ExecutionPolicy Bypass -File $launcher -PlanOnly -AgentModel 'newz=gpt-4o' 2>&1
Check 'an unknown agent name exits 1' ($LASTEXITCODE -eq 1) ("exit " + $LASTEXITCODE)
$null = & powershell -NoProfile -ExecutionPolicy Bypass -File $launcher -PlanOnly -AgentModel 'nonsense' 2>&1
Check 'an -AgentModel without AGENT=MODEL exits 1' ($LASTEXITCODE -eq 1) ("exit " + $LASTEXITCODE)

# A comma-joined value must split: `powershell -File x.ps1 -AgentModel a=b,c=d`
# hands the parameter over as one string, and only a dot-sourced call binds it
# as an array. Untreated, the whole string becomes one agent name and refuses.
$split = & powershell -NoProfile -ExecutionPolicy Bypass -File $launcher -PlanOnly -AgentModel 'news=codex/gpt-5.6-terra,risk=gpt-4o' 2>&1
Check 'a comma-joined -AgentModel splits into two' `
    ($LASTEXITCODE -eq 0 -and ($split -join "`n") -match 'risk\s+gpt-4o' -and ($split -join "`n") -match 'news\s+codex/gpt-5\.6-terra')

# 4 ------------------------------------------------- the plan says what it is
Write-Host "`nthe plan is legible, and -Apply is loud"
$dry = (& powershell -NoProfile -ExecutionPolicy Bypass -File $launcher -PlanOnly 2>&1) -join "`n"
Check 'the default plan is a DRY RUN' ($dry -match '\[DRY RUN\]')
Check 'the default plan names all three agents' `
    ($dry -match 'risk\s' -and $dry -match 'news\s' -and $dry -match 'arbiter\s')
Check 'risk and arbiter default to the plan model' `
    (([regex]::Matches($dry, 'claude-opus-5')).Count -ge 2) $dry
Check 'nothing was started' ($dry -match 'nothing stopped, nothing started')

$applied = (& powershell -NoProfile -ExecutionPolicy Bypass -File $launcher -PlanOnly -Apply 2>&1) -join "`n"
Check '-Apply announces itself as APPLY' ($applied -match '\[APPLY\]')
Check '-Apply says verdicts change REAL entries' ($applied -match 'REAL ENTRIES')
Check '-Apply names the funded mirror rather than warning in the abstract' `
    ($applied -match 'mt5_executor' -and $applied -match 'cent account')
Check '-Apply still starts nothing under -PlanOnly' ($applied -match 'nothing stopped, nothing started')

# 5 ------------------------------------------------------- nothing moved at all
Write-Host "`nthe selftest itself is inert"
$pythonAfter = @(Get-CimInstance Win32_Process -Filter "name='python.exe'").Count
Check 'no python process appeared or disappeared' ($pythonAfter -eq $pythonBefore) `
    ("before " + $pythonBefore + ", after " + $pythonAfter)
Check 'no log directory was created' (-not (Test-Path (Join-Path $root 'data\paper\logs\advisor.launches.log')))

Write-Host ''
if ($fails) {
    Write-Host ("$fails check(s) failed") -ForegroundColor Red
    exit 1
}
Write-Host 'all checks passed - and a green run means the plan and the refusals are right,' -ForegroundColor Green
Write-Host 'not that the panel works. Nothing here called a model.' -ForegroundColor DarkGray
