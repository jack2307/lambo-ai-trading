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
    # gpt-5 goes through the metered OpenAI API (OPENAI_API_KEY).
    @{ model = 'gpt-5';         run = 'ai-xau';      control = 'ai-xau-coin';      seed = 7;  log = 'ai_trader_gpt5' },
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

# Start-Process hands the child THIS process's environment, not the registry.
# A shell started before the key was saved to the User scope has never seen it,
# and the campaign then dies at startup saying the key is missing while
# `setx` swears it is set. Read it from the registry when the process lacks it.
if (-not $env:OPENAI_API_KEY) {
    $env:OPENAI_API_KEY = [Environment]::GetEnvironmentVariable('OPENAI_API_KEY', 'User')
}

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
