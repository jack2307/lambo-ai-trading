# Exercise start-desk.ps1's launch plan without starting anything.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\start-desk-selftest.ps1
#
# It runs the REAL script with -PlanOnly against temporary registries, so what
# is tested is the file that runs and not a copy of its logic. -PlanOnly stops
# after the missing-terminal check and before any Start-Process, so nothing
# here launches a terminal, an API or a poller.
#
# What it pins, in the order the defects were found on 2026-09-18:
#
#   1  the plan comes from config/accounts.toml, so a terminal in the registry
#      is started and one that is not there is not. The old script hard-coded
#      C:\MT5-demo and C:\MT5-live and could not start the funded C:\MT5-cent
#      at all - a launcher that starts three of four things and exits 0.
#   2  a registry path that is not on this disk REFUSES. It used to warn and
#      continue, which is how a missing terminal becomes a desk running without
#      one.
#   3  a disabled account is named and not started, rather than silently absent.
#   4  the price terminal comes from [prices] and is started even when no
#      enabled account claims it.
#   5  an unknown symbol_suffix refuses instead of quietly choosing 'standard'.

param(
    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Continue'
$script:Fail = 0
function Check($name, $ok) {
    if ($ok) { Write-Host "  ok   $name" -ForegroundColor DarkGray }
    else { $script:Fail++; Write-Host "  FAIL $name" -ForegroundColor Red }
}

# A registry naming terminals under $dir, which the test creates or does not.
# WITHOUT A BOM, and this is not a detail.
#
# `Set-Content -Encoding UTF8` on Windows PowerShell 5.1 writes EF BB BF at the
# start of the file, and `tomli` refuses it: "Invalid statement (at line 1,
# column 1)". The first version of this selftest used it, so every registry it
# wrote was unparseable, every run refused for that reason, and the two checks
# that assert a REFUSAL passed - a green test proving nothing about the thing
# it names. Verified 2026-09-18 by reading the bytes back.
function Write-Registry($path, $body) {
    [System.IO.File]::WriteAllText($path, $body, (New-Object System.Text.UTF8Encoding($false)))
}

function Run-Plan($registryPath) {
    # The real script, with the registry pointed at a temporary file through
    # accounts.py's own --file flag. `2>&1` so a refusal's error text is part
    # of the captured output rather than lost to the host.
    $out = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'start-desk.ps1') `
        -Root $Root -Python $Python -PlanOnly -RegistryFile $registryPath 2>&1 | Out-String
    return @{ text = $out; code = $LASTEXITCODE }
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("desk-selftest-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    # Two fake terminals that DO exist, and one path that never will.
    $termA = Join-Path $tmp 'MT5-a\terminal64.exe'
    $termB = Join-Path $tmp 'MT5-b\terminal64.exe'
    $termGone = Join-Path $tmp 'MT5-gone\terminal64.exe'
    # A third that exists and has no `config` directory at all, for the
    # autologin-asked-for-but-absent case. It has to be a terminal NO other
    # account claims: `accounts.py` refuses a registry where two accounts share
    # a terminal, which is correct - two executors on one account is two copies
    # of every order - and which made the first version of this case fail for
    # that reason rather than the one it names.
    $termC = Join-Path $tmp 'MT5-c\terminal64.exe'
    foreach ($t in @($termA, $termB, $termC)) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $t) | Out-Null
        Set-Content -Path $t -Value 'not really a terminal' -Encoding ASCII
    }

    $account = @"
[prices]
terminal = '$termA'
symbol_suffix = ".sc"

[[account]]
id = "a"
label = "A"
login = 1
server = "S"
terminal = '$termA'
real_money = false
enabled = true

[[account]]
id = "b"
label = "B"
login = 2
server = "S"
terminal = '$termB'
real_money = true
enabled = false
"@
    $reg = Join-Path $tmp 'ok.toml'
    Write-Registry $reg $account
    $r = Run-Plan $reg
    Write-Host 'a registry the machine matches'
    Check 'exits 0' ($r.code -eq 0)
    Check 'the enabled account''s terminal is in the plan' ($r.text -match [regex]::Escape($termA))
    Check 'the disabled one is named as not started' ($r.text -match 'disabled, not started: b')
    Check 'and its terminal is NOT in the plan line' (-not ($r.text -match ([regex]::Escape($termB) + '\s+<-')))
    Check 'the price feed is labelled' ($r.text -match 'prices')
    Check 'nothing was started' ($r.text -match 'plan only: nothing started')

    # The defect this whole change exists for: a terminal the registry names
    # and the machine does not have.
    $missing = $account -replace [regex]::Escape($termB), $termGone -replace 'enabled = false', 'enabled = true'
    $reg2 = Join-Path $tmp 'missing.toml'
    Write-Registry $reg2 $missing
    $r2 = Run-Plan $reg2
    Write-Host 'a registry naming a terminal this disk does not have'
    Check 'refuses rather than starting a partial desk' ($r2.code -ne 0)
    Check 'and names the path it could not find' ($r2.text -match [regex]::Escape($termGone))

    # A price terminal no account claims must still be started: the feed does
    # not have to be an account's.
    $priceOnly = $account -replace "terminal = '$([regex]::Escape($termA))'\s*\r?\nsymbol_suffix", "terminal = '$termB'`nsymbol_suffix"
    $reg3 = Join-Path $tmp 'priceonly.toml'
    Write-Registry $reg3 $priceOnly
    $r3 = Run-Plan $reg3
    Write-Host 'a price terminal that is nobody''s account'
    Check 'exits 0' ($r3.code -eq 0)
    Check 'the price terminal is in the plan anyway' ($r3.text -match ([regex]::Escape($termB) + '\s+<-'))

    # AUTOLOGIN IS OPT-IN. The decision is made while the plan is built and
    # printed with it, so this can see it; it used to be made past -PlanOnly,
    # which is why the first version of this file could not test the one
    # decision on this path that touches a funded terminal.
    New-Item -ItemType Directory -Force -Path (Join-Path (Split-Path -Parent $termA) 'config') | Out-Null
    Set-Content -Path (Join-Path (Split-Path -Parent $termA) ('config' + [char]92 + 'autologin.ini')) -Value 'Login=1' -Encoding ASCII
    $r5 = Run-Plan $reg
    Write-Host 'an autologin file the registry has not opted into'
    Check 'is reported, not used' ($r5.text -match 'autologin present, NOT used')
    Check 'and the plan does not claim it will be passed' (-not ($r5.text -match '\[autologin\]'))

    $optIn = $account -replace 'id = "a"', "id = `"a`"`nautologin = true"
    $reg5 = Join-Path $tmp 'optin.toml'
    Write-Registry $reg5 $optIn
    $r6 = Run-Plan $reg5
    Write-Host 'the same file with the registry opting in'
    Check 'the plan says it will be passed' ($r6.text -match '\[autologin\]')

    # Asked for and absent: say so rather than start silently unauthorised.
    $optInB = $optIn -replace [regex]::Escape($termA), $termC
    $reg6 = Join-Path $tmp 'optin-missing.toml'
    Write-Registry $reg6 $optInB
    $r7 = Run-Plan $reg6
    Write-Host 'autologin asked for where there is no file'
    Check 'the plan says the file is missing' ($r7.text -match 'file missing')

    # An unknown suffix is a stop, not a default. 'standard' would quietly ask
    # a cent terminal for symbols it does not carry.
    $oddSuffix = $account -replace 'symbol_suffix = "\.sc"', 'symbol_suffix = ".xx"'
    $reg4 = Join-Path $tmp 'suffix.toml'
    Write-Registry $reg4 $oddSuffix
    $r4 = Run-Plan $reg4
    Write-Host 'a symbol suffix the pollers do not know'
    Check 'refuses instead of choosing one' ($r4.code -ne 0)
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ''
if ($script:Fail) { Write-Host "$script:Fail CHECK(S) FAILED" -ForegroundColor Red; exit 1 }
Write-Host 'all checks passed' -ForegroundColor Green
exit 0
