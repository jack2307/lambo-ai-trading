# Start (or restart) the LIVE side: one executor per book, per account.
#
#   powershell -NoProfile -File py\live\start_executors.ps1
#   powershell -NoProfile -File py\live\start_executors.ps1 -Account vantage-demo
#   powershell -NoProfile -File py\live\start_executors.ps1 -Account vantage-demo -Runs xau-ema
#   powershell -NoProfile -File py\live\start_executors.ps1 -Live
#
# It stops EVERY executor and starts only what the registry and the command
# line together name, so it REFUSES when that would walk away from an open
# real-money position - see -AllowOrphans. An account with `enabled = false`
# does not start, by name or otherwise.
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

    # Start anyway when a book that will NOT be restarted is holding a position
    # on a real-money account.
    #
    # This script stops EVERY executor and then starts only what the registry
    # and the command line together name, so any narrowing - `-Account`,
    # `-Runs`, a book dropped from `runs`, a STOP file - can leave a real
    # position on the account with nothing reconciling it. Nobody then mirrors
    # the book's exit and the trade runs to the broker's stop or target
    # instead, which is not the trade the book took.
    #
    # So that case refuses, and this is the word that overrides it. Refusing is
    # the right default because the alternative is silent: the orphan looks
    # exactly like a book that is simply flat. The override exists because
    # "wait until the account is flat" is not always the right answer either -
    # a book whose executor is already wedged needs restarting most when it is
    # holding something.
    [switch]$AllowOrphans,

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

# What the accounts are holding RIGHT NOW, read from the mirrors themselves.
#
# `data/live/<account>/<book>/broker.json` is written whole-then-renamed by
# each executor every poll, and it carries the position, the login and whether
# the account is a demo. It is the only view of the broker that does not
# involve talking to MetaTrader, which this script must not do: it starts the
# processes that own those terminals and has no business opening one itself.
#
# STALENESS IS THE POINT, not a caveat. A snapshot older than a minute means
# the executor that wrote it has already stopped, so the file says what was
# true when it died and not what is true now. Those are reported separately
# below, because "this stopped while holding" needs a person just as much as
# "this is about to be orphaned" does - it is the same position with nobody
# watching it, discovered later. What they do NOT do is refuse; see the
# predicate below for why.
#
# Absolute age, not a window relative to the other mirrors. The API's
# `mirroring` count is relative and is right to be - it asks "is this one
# behind its neighbours" - and reading it as an absolute once produced
# "mirroring 8/6". This asks a different question, "is this one alive", and it
# is asked at the one moment when the whole set may legitimately be down
# already, which is exactly when a relative measure answers nothing. Same
# reasoning and the same 60 seconds as MIRROR_STALE_MS in telegram_notify.py.
#
# Deliberately duplicated in deploy\update.ps1 rather than shared. A third
# file that both of them dot-source is one more thing to be missing on the
# server at the moment someone is trying to stop a trade, and this is twenty
# lines that read the same file the same way.
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
                # `demo` is the account's own word for itself, taken from
                # `account_info().trade_mode`, and it is better evidence than
                # the registry: the registry says what we meant to trade, this
                # says what the terminal was actually holding.
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
    # ---- pass one: decide everything, start nothing ----
    #
    # The plan is built BEFORE any executor is stopped, and that ordering is
    # the whole point of the split.
    #
    # It used to kill first and decide afterwards, which meant a run that was
    # going to be skipped anyway - a terminal that is not installed, a market
    # with no symbol - still cost every other book its process. Worse, there
    # was no moment at which this script knew both what was running and what
    # it was about to run, so it could not see that it was about to walk away
    # from an open position. It now does, and that check is below.
    $plan = @()

    # What join rule the executors this run starts will actually use.
    #
    # ASKED, NEVER STATED, and the difference is the whole reason this is four
    # lines of subprocess rather than one of string. A hardcoded "symmetric"
    # here would be a record that agrees with the code today and disagrees the
    # day someone flips the default - the same defect as a comment that was
    # true when it was written, which this repository spent 2026-09-17
    # removing from six places.
    #
    # So the executor is asked, and its answer is printed VERBATIM. The
    # sentence lives in exactly one place - mt5_executor.py, beside the flag
    # that decides it - and there is no second copy here to go stale. A third
    # mode could appear tomorrow and this line would print it without anyone
    # touching this file.
    #
    # `--print-join-mode` short-circuits before parse_args and before MT5 is
    # imported, so this costs one interpreter start and cannot reach the order
    # path. It honours any join flags it is given: nothing passes one today,
    # and if the mode ever becomes per-account in accounts.toml, whoever adds
    # it to $argv below must add it here too or this line starts lying.
    #
    # A FAILED ASK PRINTS THAT IT FAILED. It does not fall back to a guess -
    # an unavailable answer and a wrong answer look identical to a reader at
    # three in the morning, and only one of them is honest.
    # Captured WHOLE and then indexed, never piped into `Select-Object -First
    # 1`. That pipeline terminates the upstream command early and leaves
    # $LASTEXITCODE at -1 even when the query succeeded and printed the right
    # line - measured, not feared: the first version of this read the correct
    # sentence, saw -1, discarded it, and printed COULD NOT ASK every single
    # time. It fails in the safe direction, which is exactly why nobody would
    # have noticed: the fallback message looks deliberate.
    $joinMode = ''
    try {
        $out = & $Python (Join-Path $Root 'py\live\mt5_executor.py') '--print-join-mode' 2>&1
        if ($LASTEXITCODE -eq 0 -and $out) { $joinMode = [string](@($out)[0]) }
    } catch { $joinMode = '' }
    if ($joinMode) {
        Write-Host "  $joinMode" -ForegroundColor Cyan
    } else {
        Write-Host '  join rule: COULD NOT ASK the executor (--print-join-mode failed).' -ForegroundColor Yellow
        Write-Host '  Not guessing. Read join_check in py\live\mt5_executor.py before starting.' -ForegroundColor Yellow
    }

    foreach ($acct in $accounts) {
        # `enabled = false` means this account does not run. Checked HERE and
        # not in accounts.py, which is a reader: `--id` deliberately answers
        # for a disabled account so one can be inspected, and a reader that
        # hid it would be lying about the file it exists to report.
        #
        # It was checked in NEITHER place until 2026-09-17. `-Account <id>`
        # reaches accounts.py as `--id`, which skips the enabled filter, and
        # nothing here read the field - so `-Account vantage-demo -Live`
        # started the mirrors of an account that had been turned off precisely
        # because a second machine still holds a terminal logged into it. The
        # note in accounts.toml saying that could not happen was simply wrong,
        # and has been corrected there too.
        #
        # This is also the rule the header of this file already states: the
        # command line NARROWS what the registry says and never widens it.
        # `-Account` naming a disabled account was the one place that widened.
        if (-not $acct.enabled) {
            Write-Host "skipping $($acct.id) - enabled = false in accounts.toml" -ForegroundColor Yellow
            continue
        }

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

            # The terminal path is QUOTED. Start-Process joins this array with
            # spaces and quotes nothing, and the real account's terminal lives
            # in `C:\Program Files\MetaTrader 5\` - unquoted it reaches argparse
            # as two arguments and the executor exits on an unrecognised one.
            # The pollers hit exactly this on 2026-09-17 and took the feed down
            # with it; the demo path, C:\MT5-demo, has no space and never showed
            # it.
            #
            # `$argv` and not `$args`: `$args` is a PowerShell automatic
            # variable holding the parameters nothing bound. Assigning it works
            # at script scope, which is why this was fine, and stops working
            # silently the moment this block is moved into a function - at
            # which point `Start-Process -ArgumentList $args` would hand the
            # executor whatever the caller typed and nothing else.
            $argv = @('py/live/mt5_executor.py', "--run=$run", "--terminal=`"$($acct.terminal)`"",
                      "--login=$($acct.login)", "--account=$($acct.id)", "--symbol=$sym",
                      "--lot-scale=$($acct.lot_scale)")
            if ($dry) { $argv += '--dry-run' }
            # Passed only for an account the registry marks, and only when the
            # command line asked. The executor checks the registry again for
            # itself; this is not the permission, only one of the three checks
            # it makes - and see accounts.toml on why those three checks are
            # not three independent decisions on this path.
            if ($real) { $argv += '--allow-real' }

            # Everything one account writes about one book lives together,
            # stdout included. Two accounts mirroring the same book used to
            # share exec_<run>.out and overwrite each other's record.
            $plan += [pscustomobject]@{
                Account  = [string]$acct.id
                Run      = [string]$run
                Symbol   = $sym
                LotScale = $acct.lot_scale
                Here     = Join-Path $Root "data\live\$($acct.id)\$run"
                Argv     = $argv
            }
        }
    }

    $wanted = @($plan).Count

    # ---- the orphan check, between deciding and stopping ----
    #
    # Everything this script is about to kill that it is not about to start
    # again, and that is holding a position. See -AllowOrphans at the top for
    # why this refuses rather than warns.
    #
    # Read here and not earlier: the executors are still running, so their
    # snapshots are seconds old and say what the accounts hold right now.
    $keep = @{}
    foreach ($p in $plan) { $keep["$($p.Account)/$($p.Run)"] = $true }
    $orphans = @(Get-Holdings $Root | Where-Object { -not $keep["$($_.Account)/$($_.Run)"] })

    if ($orphans.Count -gt 0) {
        Write-Host ''
        Write-Host 'Holding a position, and NOT in what this would start:' -ForegroundColor Yellow
        foreach ($o in $orphans) {
            $what = "  $($o.Account)/$($o.Run): $($o.Side) $($o.Lots) lots, ticket $($o.Ticket)"
            if ($o.Real) { $what += ' [REAL MONEY]' }
            if ($o.Stale) { $what += "  (snapshot $([int]($o.AgeMs / 1000))s old - this mirror ALREADY stopped)" }
            Write-Host $what -ForegroundColor $(if ($o.Real) { 'Red' } else { 'Gray' })
        }
        # Only FRESH real-money holdings refuse, and that asymmetry is the
        # deliberate part rather than an oversight.
        #
        # A fresh snapshot means the executor is alive and the position is open
        # now: killing it is an orphan this script would be CREATING, and it is
        # the caller's to decide. A stale one means the mirror already stopped,
        # at some point that could be months ago - broker.json is never deleted
        # and `data/` is never cleaned - so the position it describes may have
        # closed long since. Refusing on that would make the launcher
        # permanently unusable on the strength of a file nobody tidies, and a
        # kill switch that cannot be reached is worse than a loud warning.
        #
        # So the stale ones are named, in the same list, and do not refuse.
        # They still need a person; they do not need this script to stop.
        $realOrphans = @($orphans | Where-Object { $_.Real -and -not $_.Stale })
        if ($realOrphans.Count -gt 0 -and -not $AllowOrphans) {
            Write-Host ''
            Write-Error ("$($realOrphans.Count) real-money position(s) would be left with nobody reconciling them. " +
                         'Nothing has been stopped. Either start those books too, or wait for the account to go ' +
                         'flat, or drop a STOP file to close them deliberately - and if you mean to leave them ' +
                         'open, say -AllowOrphans.')
            exit 1
        }
        Write-Host ''
    }

    # ---- pass two: stop everything, start the plan ----
    #
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
    foreach ($p in $plan) {
        New-Item -ItemType Directory -Force -Path $p.Here | Out-Null
        Start-Process -FilePath $Python -ArgumentList $p.Argv -WorkingDirectory $Root -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $p.Here 'exec.out') `
            -RedirectStandardError (Join-Path $p.Here 'exec.err')
        Write-Host "  mirroring $($p.Account)/$($p.Run) -> $($p.Symbol) at x$($p.LotScale)"
        $started++
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
