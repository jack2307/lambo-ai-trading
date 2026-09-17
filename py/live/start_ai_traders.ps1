# Start (or restart) the AI trader campaigns. READ docs/paper/AI-TRADER.md first.
#
#   powershell -NoProfile -File py\live\start_ai_traders.ps1
#   powershell -NoProfile -File py\live\start_ai_traders.ps1 -DryRun
#
# One process per model, each driving a matched pair of paper books: the
# model's side and a coin's. Nothing here can reach a broker — the route these
# post to accepts a side, a stop and a target for an `external` run and nothing
# else, and `py/live/mt5_executor.py` remains the only code in the repository
# that can send an order, on a demo account only.
#
# The seeds differ on purpose. With one seed both coins would flip identically,
# and on any bar where both models traded, the two campaigns would share a
# control's luck — the one thing a control may not do.
param(
    [switch]$DryRun,
    [string]$Python = 'C:\Python39\python.exe',
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),

    # Run only these campaigns, by run id (e.g. -Only ai-xau-ds-ctx,ai-xau-sol-ctx).
    # Empty means all of them. A campaign left out is not started - and since
    # this script stops every ai_trader first, leaving one out STOPS it.
    [string[]]$Only = @(),

    # Drive the model's book with no coin beside it.
    #
    # The owner asked for this on 2026-09-17, having decided the control was
    # not earning its place. What it costs is stated in ai_trader.py's
    # docstring and repeated once here because this is where someone turns it
    # on: without the coin, a campaign can say what it earned but not whether
    # the model earned it, because every long-gold book made money in a week
    # gold rose.
    [switch]$NoControl
)

$campaigns = @(
    # OpenAI's model through the account's ChatGPT PLAN, via the Codex CLI.
    # No OPENAI_API_KEY is involved. The `codex/` prefix is what routes it to
    # the plan — a bare `gpt-*` would still mean the metered API.
    #
    # The model is `gpt-5.6-sol` and NOT `gpt-5`: a ChatGPT account refuses
    # `gpt-5`, `gpt-5-codex` and `codex-mini-latest` outright ("not supported
    # when using Codex with a ChatGPT account"). This is the name Codex itself
    # defaults to on this plan, read from its own banner. That makes it a
    # DIFFERENT model from the one the API campaign ran, which is why that
    # campaign's books were closed rather than repointed.
    @{ model = 'codex/gpt-5.6-sol'; run = 'ai-xau-sol-ctx'; control = 'ai-xau-sol-ctx-coin'; seed = 7;  log = 'ai_trader_sol_ctx' },
    # claude-opus-5 goes through the account's PLAN, via the Claude Code CLI.
    # No API key is involved; see the `claude-cli` provider in advisor.py.
    @{ model = 'claude-opus-5'; run = 'ai-xau-opus-ctx'; control = 'ai-xau-opus-ctx-coin'; seed = 11; log = 'ai_trader_opus_ctx' },
    # DeepSeek's cheap model, straight at the metered API — no CLI, no plan.
    # Measured 2026-09-16: 1.5s and roughly a dollar a month for one 15m book,
    # because a direct call spends 2,853 tokens on the question where the CLI
    # route spends 26,509 on the same one. It is here to answer whether model
    # quality matters for this task at all; see the amendment in AI-TRADER.md.
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-ctx'; control = 'ai-xau-ds-ctx-coin'; seed = 23; log = 'ai_trader_ds' }
)

# `powershell -File script.ps1 -Only a,b` hands the parameter over as the
# single string "a,b"; only a dot-sourced call binds it as an array. Same trap
# and same fix as -Runs in start_executors.ps1, which learned it the same way.
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

if ($Only.Count) {
    $known = $campaigns | ForEach-Object { $_.run }
    $unknown = $Only | Where-Object { $known -notcontains $_ }
    if ($unknown) {
        # Refused rather than ignored. A typo that silently selects nothing
        # would stop every campaign and start none, and report success.
        Write-Error ("no such campaign: " + ($unknown -join ', ') + "; known: " + ($known -join ', '))
        exit 1
    }
    $campaigns = $campaigns | Where-Object { $Only -contains $_.run }
}
Write-Host ("campaigns: " + (($campaigns | ForEach-Object { $_.run }) -join ', '))
if ($NoControl) { Write-Host 'no coin: these books have nothing to be read against' -ForegroundColor Yellow }

# Two copies of this script running at once is not a nuisance, it is a
# corrupted campaign: each model would answer every bar twice, post twice, and
# the coin would flip twice off one seed. Kill-then-start does not protect
# against it — both copies kill nothing and then both start. So: one at a time,
# system-wide.
$mutex = New-Object System.Threading.Mutex($false, 'Global\flowdesk-ai-traders')
if (-not $mutex.WaitOne(0)) {
    Write-Error 'another start_ai_traders.ps1 is running; refusing to start a second set'
    exit 1
}

try {

# --------------------------------------------------- before anything is killed
#
# PATH is the OPENAI_API_KEY problem wearing different clothes, and unlike that
# one it did not go away with the key. Start-Process hands the child THIS
# process's environment, so a shell opened before Node was installed launches
# traders that cannot see `node` - and `codex.CMD` is a batch file whose first
# act is to run `node`. Measured 2026-09-17 on the server: `codex --version`
# exited 1 with '"node"' is not recognized, from a session whose PATH predated
# the install, and succeeded a line later once PATH was re-read. So re-read it
# from the registry, which is where the truth is.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [Environment]::GetEnvironmentVariable('Path', 'User')

# Then check that each model can be reached, HERE, above the kill. A campaign
# that cannot reach its model should leave the running one alone rather than
# replace it with nothing: the failure is otherwise invisible until a model is
# asked a question on a live bar, and by then this script has already reported
# that everything started.
#
# check_clis.py and not Get-Command, because the two machines resolve Codex
# differently and a PATH test gets one of them wrong; see its docstring.
Write-Host 'checking each model can be reached'
& $Python (@((Join-Path $Root 'py\live\check_clis.py')) + ($campaigns | ForEach-Object { $_.model })) |
    ForEach-Object { Write-Host "   $_" }
if ($LASTEXITCODE -ne 0) {
    Write-Error 'not starting; nothing was stopped'
    exit 1
}

Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*ai_trader.py*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 3

# Refuse to start on top of a survivor: a process the kill missed would double
# every decision from here on, and the campaign's own log would not show it.
$alive = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*ai_trader.py*' })
if ($alive.Count -gt 0) {
    Write-Error "$($alive.Count) ai_trader process(es) survived the stop; not starting more"
    exit 1
}

# No key is loaded here on purpose. Both campaigns now run on the account's
# own plans — Codex for OpenAI's model, Claude Code for Anthropic's — so
# nothing these processes do should be able to reach a metered API even by
# accident. The previous version of this script read OPENAI_API_KEY out of the
# User registry scope, because Start-Process hands the child THIS process's
# environment and a shell older than the key had never seen it. That problem
# is gone with the key.

$logs = Join-Path $Root 'data\paper\logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
foreach ($c in $campaigns) {
    $args = @('py/live/ai_trader.py', "--model=$($c.model)", '--market=xauusd', '--tf=15m',
              "--run=$($c.run)", "--control=$($c.control)", "--seed=$($c.seed)")
    if ($DryRun) { $args += '--dry-run' }
    # --control is still passed: the coin is still flipped off the same seed
    # so the sequence does not depend on whether a control book exists, and
    # switching the coin back on replays instead of diverging.
    if ($NoControl) { $args += '--no-control' }
    Start-Process -FilePath $Python -ArgumentList $args -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs "$($c.log).out") -RedirectStandardError (Join-Path $logs "$($c.log).err")
    $mode = if ($DryRun) { 'DRY RUN' } else { 'LIVE' }
    # Do not name a control that is not being written. The line read
    # "-> ai-xau-sol-ctx vs ai-xau-sol-ctx-coin" on a run started with
    # -NoControl, which is the log claiming a comparison that does not exist.
    $against = if ($NoControl) { 'no coin' } else { "vs $($c.control)" }
    Write-Host "started $($c.model) -> $($c.run) $against (seed $($c.seed)) [$mode]"
}

Start-Sleep -Seconds 2
$running = @(Get-CimInstance Win32_Process -Filter "name='python.exe'" |
    Where-Object { $_.CommandLine -like '*ai_trader.py*' })
Write-Host "ai_trader processes now running: $($running.Count) (expected $($campaigns.Count))"

}
finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}
