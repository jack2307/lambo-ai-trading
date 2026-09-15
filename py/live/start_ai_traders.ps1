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
    [string]$Root = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
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
    @{ model = 'codex/gpt-5.6-sol'; run = 'ai-xau-sol'; control = 'ai-xau-sol-coin'; seed = 7;  log = 'ai_trader_sol' },
    # claude-opus-5 goes through the account's PLAN, via the Claude Code CLI.
    # No API key is involved; see the `claude-cli` provider in advisor.py.
    @{ model = 'claude-opus-5'; run = 'ai-xau-opus'; control = 'ai-xau-opus-coin'; seed = 11; log = 'ai_trader_opus5' }
)

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
    Start-Process -FilePath $Python -ArgumentList $args -WorkingDirectory $Root -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $logs "$($c.log).out") -RedirectStandardError (Join-Path $logs "$($c.log).err")
    $mode = if ($DryRun) { 'DRY RUN' } else { 'LIVE' }
    Write-Host "started $($c.model) -> $($c.run) vs $($c.control) (seed $($c.seed)) [$mode]"
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
