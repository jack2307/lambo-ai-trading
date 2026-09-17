# Start (or restart) the LIVE side: one executor per book, per account.
#
#   powershell -NoProfile -File py\live\start_executors.ps1
#   powershell -NoProfile -File py\live\start_executors.ps1 -Account vantage-demo
#   powershell -NoProfile -File py\live\start_executors.ps1 -Account vantage-demo -Runs xau-ema
#   powershell -NoProfile -File py\live\start_executors.ps1 -Live
#
# Paper and live are separate by construction and this script is the only thing
# that joins them. The paper desk decides; an executor MIRRORS one paper book
# into one MT5 account and does nothing else - no strategy logic lives here, so
# a live book can never disagree with the paper book it came from about what
# the rule said. What it can disagree about is the FILL, and that difference is
# the whole point of running it.
#
# WHAT RUNS WHERE IS config\accounts.toml, not this file and not the command
# line. Add an account there; name its books there. The command line only ever
# NARROWS what the registry already says - one account instead of all, a
# subset of its books - so that what is running can always be read off one
# file rather than reconstructed from whatever was typed.
param(
    # One account by id. Omitted, every enabled account in the registry starts.
    [string]$Account = '',

    # Mirror only these books of the account, instead of the ones it names.
    # A narrowing, never an addition: a book the account does not list is
    # refused rather than started, because the registry is the record of what
    # is live and a book started around it would not be in that record.
    [string[]]$Runs = @(),

    # Force dry run whatever the registry says. There is deliberately no
    # matching switch to force the opposite for everything: making a run safer
    # than the file says is free, making it riskier has to be said per account,
    # which is what -Live does.
    [switch]$DryRun,

    # Honour the registry's `dry_run = false`. Without this, every account runs
    # dry however it is configured - so the step from recording to trading is
    # always a deliberate word on the command line, never a file someone edited
    # a week ago.
    [switch]$Live,

    # Permit the accounts the registry marks `real_money = true`.
    #
    # Separate from -Live, and it has to be. -Live means "send orders", which
    # has meant a demo for weeks and costs nothing when it is wrong. This one
    # means "send orders that spend money". Folding it into -Live would carry
    # every habit built on a demo straight onto a funded account, which is the
    # habit most worth breaking.
    #
    # Without it a real-money account is SKIPPED, with a line saying so, and
    # the demo accounts in the same registry start exactly as before.
    [switch]$AllowReal,

    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = ''
)

# Resolved in the body, not in the param default: $PSScriptRoot is not reliably
# populated while defaults are being evaluated, and an empty one made
# Split-Path fail before the script had run a line.
if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent (Split-Path -Parent $here)
}
if (-not $Root) { $Root = (Get-Location).Path }

# `powershell -File script.ps1 -Runs a,b` hands the parameter over as the single
# string "a,b" - only a dot-sourced call binds it as an array. The documented
# invocation at the top of this file is the -File form, so it arrived as one
# run named "xau-ema,eur-hours" and was skipped as an unknown market. Split
# here so both forms mean the same thing.
$Runs = @($Runs | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

# Paper market -> the symbol that market trades on a STANDARD account.
#
# The paper desk quotes cent symbols (`XAUUSD.sc`) because the live Vantage
# account is a cent account. A demo account has no `.sc` book at all - checked
# against account 26108386, whose 1200 symbols contain XAUUSD and BTCUSD and no
# cent variant of either - so the mirror has to name the standard symbol. This
# table is the whole translation, and it is deliberately explicit: deriving it
# by stripping ".sc" would silently produce a tradable-looking symbol on some
# other broker where the cent book is spelled differently.
$SYMBOL_OF = @{
    'xauusd'  = 'XAUUSD'
    'xauduka' = 'XAUUSD'
    'gold'    = 'XAUUSD'
    'xagduka' = 'XAGUSD'
    'eurusd'  = 'EURUSD'
    'eurduka' = 'EURUSD'
    'btcusd'  = 'BTCUSD'
    'btc'     = 'BTCUSD'
}

# The suffix is the ACCOUNT's, and comes from the registry. The demo has none
# and gets the standard name; the real cent account sets '.sc' and gets the
# cent book, which is the one the paper desk has quoted all along. Without
# this the mirror would have named XAUUSD on an account that only lists
# XAUUSD.sc - the executor would have refused, safely, and every book would
# have sat flat for reasons nobody would have guessed at quickly.
function Resolve-Symbol([string]$run, [string]$suffix) {
    $state = Join-Path $Root "data\paper\$run\state.json"
    if (-not (Test-Path $state)) { return $null }
    $market = (Get-Content $state -Raw | ConvertFrom-Json).config.market
    if (-not $market) { return $null }
    $base = $SYMBOL_OF[[string]$market]
    if (-not $base) { return $null }
    return "$base$suffix"
}

# ---- the registry ----
$readerArgs = @('py/live/accounts.py')
if ($Account) { $readerArgs += "--id=$Account" }
$json = & $Python $readerArgs 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "account registry: $json"
    exit 1
}
# Assigned FIRST, then wrapped. `@($json | ConvertFrom-Json)` looks equivalent
# and is not: in PowerShell 5.1 ConvertFrom-Json emits a JSON array as ONE
# pipeline object, and @() around a pipeline does not unroll it - the result is
# a one-element array holding an Object[]. `foreach` then runs ONCE with every
# account at once, and since `$acct.id` on an array silently returns every id
# joined by a space, nothing fails: it started the second account's books
# against the first account's terminal and reported success.
#
# It was invisible with one account, which is how it got written. Measured
# 2026-09-16: form A gives Count=1 (Object[]), form B gives Count=2.
$parsed = $json | ConvertFrom-Json
$accounts = @($parsed)
foreach ($a in $accounts) {
    if (-not $a.id) {
        Write-Error 'account registry did not parse into account objects; refusing to guess'
        exit 1
    }
}
if ($accounts.Count -eq 0) {
    Write-Host "no enabled accounts in config\accounts.toml - nothing to start"
    exit 0
}

# Two copies of an executor on one book is two copies of every order. The
# reconciler is idempotent against the BOOK, not against another copy of
# itself: both would see a flat terminal, both would open, and the account
# would carry double the size the book asked for.
$mutex = New-Object System.Threading.Mutex($false, 'Global\flowdesk-executors')
if (-not $mutex.WaitOne(0)) {
    Write-Error 'another start_executors.ps1 is running; refusing to start a second set'
    exit 1
}

try {
    # Every executor, not just this account's. Starting one account must not
    # leave another account's mirrors running against a desk that has moved on
    # - and the registry, not the survivors, is the record of what should be up.
    Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 3

    $alive = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
    if ($alive.Count -gt 0) {
        Write-Error "$($alive.Count) executor(s) survived the stop; not starting more"
        exit 1
    }

    $started = 0
    $wanted = 0

    foreach ($acct in $accounts) {
        if (-not (Test-Path $acct.terminal)) {
            Write-Host "skipping $($acct.id) - no terminal at $($acct.terminal)"
            continue
        }

        $books = @($acct.runs)
        if ($Runs.Count -gt 0) {
            $unknown = @($Runs | Where-Object { $books -notcontains $_ })
            if ($unknown.Count -gt 0) {
                Write-Host "skipping $($acct.id) - $($unknown -join ', ') not listed for it in accounts.toml"
                continue
            }
            $books = $Runs
        }
        if ($books.Count -eq 0) {
            Write-Host "skipping $($acct.id) - it names no books"
            continue
        }

        # A real-money account is skipped unless it was asked for by name on
        # the command line. Skipped and not refused: the point is that starting
        # the demo mirrors must stay a one-word operation that cannot pick up a
        # funded account by accident.
        $real = [bool]$acct.real_money
        if ($real -and -not $AllowReal) {
            Write-Host "skipping $($acct.id) - real money, and -AllowReal was not given" -ForegroundColor Yellow
            continue
        }

        # Dry unless the registry says otherwise AND -Live was passed. -DryRun
        # overrides both, in the safe direction only.
        $dry = $DryRun -or $acct.dry_run -or (-not $Live)
        $mode = if ($dry) { 'DRY RUN' } else { 'LIVE' }
        if ($real) { $mode = "$mode - REAL MONEY" }
        Write-Host "$($acct.id): account $($acct.login) on $($acct.server) [$mode]" -ForegroundColor $(if ($real -and -not $dry) { 'Red' } else { 'Gray' })

        foreach ($run in $books) {
            # The kill switch the executor itself watches. Left in place here
            # rather than cleared: a book stopped by hand stays stopped until
            # someone deletes the file on purpose.
            $stop = Join-Path $Root "data\live\$($acct.id)\$run\STOP"
            if (Test-Path $stop) {
                Write-Host "  skipping $run - a STOP file is present; delete it to run this book live"
                continue
            }

            # A book whose market has no standard symbol is skipped rather than
            # guessed at. Sending an order on the wrong instrument is worse than
            # not sending one.
            $sym = Resolve-Symbol $run $acct.symbol_suffix
            if (-not $sym) {
                Write-Host "  skipping $run - no symbol known for its market; add it to SYMBOL_OF"
                continue
            }

            $wanted++
            # Everything one account writes about one book lives together,
            # stdout included. Two accounts mirroring the same book used to
            # share exec_<run>.out and overwrite each other's record.
            $here = Join-Path $Root "data\live\$($acct.id)\$run"
            New-Item -ItemType Directory -Force -Path $here | Out-Null

            # The terminal path is QUOTED. Start-Process joins this array with
            # spaces and quotes nothing, and the real account's terminal lives
            # in `C:\Program Files\MetaTrader 5\` - unquoted it reaches argparse
            # as two arguments and the executor exits on an unrecognised one.
            # The pollers hit exactly this on 2026-09-17 and took the feed down
            # with it; the demo path, C:\MT5-demo, has no space and never showed
            # it.
            $args = @('py/live/mt5_executor.py', "--run=$run", "--terminal=`"$($acct.terminal)`"",
                      "--login=$($acct.login)", "--account=$($acct.id)", "--symbol=$sym",
                      "--lot-scale=$($acct.lot_scale)")
            if ($dry) { $args += '--dry-run' }
            # Passed only for an account the registry marks, and only when the
            # command line asked. The executor checks the registry again for
            # itself; this is not the permission, only one third of it.
            if ($real) { $args += '--allow-real' }
            Start-Process -FilePath $Python -ArgumentList $args -WorkingDirectory $Root -WindowStyle Hidden `
                -RedirectStandardOutput (Join-Path $here 'exec.out') `
                -RedirectStandardError (Join-Path $here 'exec.err')
            Write-Host "  mirroring $run -> $sym at x$($acct.lot_scale)"
            $started++
        }
    }

    Start-Sleep -Seconds 3
    $running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
        Where-Object { $_.CommandLine -like '*mt5_executor.py*' })
    Write-Host "executors now running: $($running.Count) (launched $started of $wanted)"
    if ($running.Count -lt $wanted) {
        Write-Host "one or more exited immediately - read data\live\<account>\<book>\exec.err; the usual cause is"
        Write-Host "the terminal not being logged into the account the registry names, which the"
        Write-Host "executor refuses by design."
    }
}
finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
