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
    # The model name is one of the plan's own and NOT `gpt-5`: a ChatGPT
    # account refuses `gpt-5`, `gpt-5-codex` and `codex-mini-latest` outright
    # ("not supported when using Codex with a ChatGPT account"). That made the
    # plan campaign a DIFFERENT model from the one the API campaign ran, which
    # is why that campaign's books were closed rather than repointed - the same
    # rule that retires the entry below.
    #
    # RETIRED 2026-09-17, left here rather than deleted so the file still says
    # what ran:
    #   @{ model = 'codex/gpt-5.6-sol'; run = 'ai-xau-sol-ctx'; control = 'ai-xau-sol-ctx-coin'; seed = 7; log = 'ai_trader_sol_ctx' },
    #
    # On the VPS every call comes back 400, "The 'gpt-5.6-sol' model is not
    # supported when using Codex with a ChatGPT account" - on the SAME account
    # and the SAME CLI version (0.148.0-alpha.15) that answers on the desktop.
    # What differs is the binary and the credential: the desktop goes through
    # the Codex desktop app with an auth.json predating today, the VPS logged
    # in fresh through the npm CLI. The campaign has no model on the machine it
    # runs on, and 99 decisions is where it stops.
    #
    # STOPPED rather than repointed, for the reason the API campaign was:
    # `ai-xau-sol-ctx` carries an equity curve, a trade list and 99 decisions
    # that mean "this is what gpt-5.6-sol did". Another model under that id
    # makes every number in it a blend of two models under one name - intact
    # and meaningless.
    #
    # And they ARE two models, which was checked rather than assumed. The
    # account's own list, %USERPROFILE%\.codex\models_cache.json, fetched
    # 2026-09-17T05:00Z by client 0.148.0, holds six models and carries BOTH
    # `gpt-5.6-sol` (GPT-5.6-Sol, priority 4) and `gpt-5.6-terra`
    # (GPT-5.6-Terra, priority 7) as separate live entries, beside
    # `gpt-5.6-luna`. A rename cannot put two names in one list at one moment.
    #
    # Read the new campaign's results knowing this: by the vendor's own
    # ordering terra ranks BELOW sol. It is a substitution, not an upgrade, and
    # the two books are not comparable to each other.
    @{ model = 'codex/gpt-5.6-terra'; run = 'ai-xau-terra-ctx'; control = 'ai-xau-terra-ctx-coin'; seed = 31; log = 'ai_trader_terra_ctx' },
    # claude-opus-5 goes through the account's PLAN, via the Claude Code CLI.
    # No API key is involved; see the `claude-cli` provider in advisor.py.
    # STOPPED 2026-09-18 by the owner, to stop spending Opus plan quota on a
    # book whose answer is known: NONE on every one of ~100 real bars. The
    # book and its coin were stopped via /api/paper/stop (final.json kept);
    # the row is kept here, commented, so the id is never reused - starting
    # it again would be a new campaign under a new id, not a resumption.
    # It was the base control for docs/hypotheses/2026-09-17-prompt-coin-penalty.md;
    # its rate (0 entries) is established, the registration records the stop.
    # @{ model = 'claude-opus-5'; run = 'ai-xau-opus-ctx'; control = 'ai-xau-opus-ctx-coin'; seed = 11; log = 'ai_trader_opus_ctx' },
    # THE SAME MODEL ON THE SAME BARS WITH ONE SENTENCE REMOVED, and it is a
    # second campaign rather than an edit to the row above for the reason
    # terra is not sol: an id has to keep meaning one thing, and here the
    # comparison between the two books IS the experiment.
    #
    # WHY. `ai-xau-opus-ctx` has never once entered. 79 real answers out of 99
    # rows on 2026-09-17 - the other 20 were CLI auth failures - every one
    # NONE, with a coherent reason each time. On the same bars and the same
    # prompt deepseek-flash entered 7 of 77 and gpt-5.6-sol 6 of 94. The
    # suspect is the opening sentence's second half, which tells the model
    # that a trade it is not confident in is WORSE than no trade: for a
    # cautious instruction-follower that makes standing aside the
    # literal-safe answer every bar, and a book that never trades measures
    # nothing.
    #
    # `no-coin-penalty` removes that clause and adds nothing in its place.
    # Registered at docs/hypotheses/2026-09-17-prompt-coin-penalty.md with the
    # decision rule and the falsifier written down before the first bar.
    #
    # Seed 17 and not 11 on purpose: a shared seed would give the two
    # campaigns the same coin sequence, and then neither has a control.
    @{ model = 'claude-opus-5'; run = 'ai-xau-opus-ctx-b'; control = 'ai-xau-opus-ctx-b-coin'; seed = 17; log = 'ai_trader_opus_ctx_b'; promptVariant = 'no-coin-penalty' },
    # DeepSeek's cheap model, straight at the metered API — no CLI, no plan.
    # Measured 2026-09-16: 1.5s and roughly a dollar a month for one 15m book,
    # because a direct call spends 2,853 tokens on the question where the CLI
    # route spends 26,509 on the same one. It is here to answer whether model
    # quality matters for this task at all; see the amendment in AI-TRADER.md.
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-ctx'; control = 'ai-xau-ds-ctx-coin'; seed = 23; log = 'ai_trader_ds' },
    # THE SAME MODEL ON THE SAME BARS WITH ONE BLOCK ADDED: options-flow
    # positioning from the COMEX tape, inside the market-context section and
    # under its framing - context, not a signal. `ai-xau-ds-ctx` above keeps
    # running unchanged and is the control.
    #
    # deepseek-flash and not Opus, deliberately. This asks whether the context
    # changes decisions, so it needs a model that already makes some: ds
    # entered 7 of 77 on the base prompt where Opus entered none. Running it
    # on a model that never trades would measure nothing twice.
    #
    # It is a different claim from the twenty-five registrations that tested
    # levels of this kind as mechanical rules and found nothing out of sample.
    # Those asked whether the levels predict price. This asks whether they
    # change what a model decides - which can be true whether or not they
    # predict anything, and is cheap to measure as a disagreement rate.
    # Registered at docs/hypotheses/2026-09-17-otl-context.md.
    #
    # THE FEED CAN FAIL AND THAT IS RECORDED, NOT PAPERED OVER. When it is
    # unreachable or stale the block says so in the prompt and the row carries
    # `otl`, so the book's bars can be split into context-present and
    # context-absent. Falling back to the base prompt would mix base decisions
    # into this book's numbers with nothing able to separate them.
    #
    # Seed 29: distinct from 23, or the two ds campaigns share one coin.
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-ctx-otl'; control = 'ai-xau-ds-ctx-otl-coin'; seed = 29; log = 'ai_trader_ds_otl'; promptVariant = 'otl-context' },
    # THE HIGHER-TIMEFRAME PAIR. Registered at
    # docs/hypotheses/2026-09-18-htf-context.md before either book saw a bar,
    # and they are a PAIR on purpose: `htf-context` is the base prompt plus one
    # facts block, `htf-filter` is htf-context plus exactly one sentence. Two
    # claims, and the second is only interesting if the first survives - so
    # they are read in that order, and the registration says that a
    # disagreement rate under 10% over the first 100 shared bars closes the
    # first claim and stops both.
    #
    # Same model as `ai-xau-ds-ctx` above, which is their control, because a
    # variant tested against a different model's book measures the model.
    #
    # THEY NEED BARS THAT NO COMMIT SHIPS. Both fetch GET /api/paper/htf,
    # which reads data\bars\XAUUSD-4h.parquet and -1d.parquet. `data\` is
    # gitignored: the bars come from the flowdesk-htf-export task
    # (deploy\install-tasks.ps1), and deploy\update.ps1 refuses to report
    # ready without them. If they are missing these two books still run and
    # every decision carries `htf: unavailable` - which is a recorded result
    # and not a silent one, but it is not the experiment.
    #
    # Seeds 37 and 41: distinct from each other and from 7, 11, 17, 23, 29 and
    # 31. Two books sharing a seed share a coin's luck on every bar where both
    # traded, which is the one thing a control may not do.
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-ctx-htf'; control = 'ai-xau-ds-ctx-htf-coin'; seed = 37; log = 'ai_trader_ds_htf'; promptVariant = 'htf-context' },
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-ctx-htf-filter'; control = 'ai-xau-ds-ctx-htf-filter-coin'; seed = 41; log = 'ai_trader_ds_htf_filter'; promptVariant = 'htf-filter' },
    # THE STAGED-ENTRY PAIR, stages 1 and 2 of
    # docs/plans/2026-09-18-staged-ai-entry.md. Registered at
    # docs/hypotheses/2026-09-18-plan-entry.md and 2026-09-18-plan-trigger.md
    # before either book saw a bar.
    #
    # `plan` answers with an entry type and price - "wait for 4331" - instead
    # of a side that fills at the next open; the order fills from the desk's
    # tick feed and is cancelled unfilled, AS A ROW, after the bars the model
    # named. `ai-xau-ds-ctx` above is its control: same model, same bars, the
    # market answer. `plan-trigger` is the same prompt (the selftest pins the
    # two byte-identical) plus a question every minute while an order waits:
    # TRIGGER, WAIT or CANCEL, thinking off, on the decision prompt re-sent
    # unchanged so the provider serves it from cache. `plan` is ITS control:
    # the rule-only fill against the model at the trigger.
    #
    # Same model as the rest of the ds family for the reason the htf pair
    # gives: a variant tested against a different model's book measures the
    # model. Their coins take the same entry TYPE at mirrored distances, so
    # the control shares the fill mechanics and never gets the fast question.
    #
    # Seeds 43 and 47: distinct from each other and from 7, 11, 17, 23, 29,
    # 31, 37 and 41, for the reason every row above gives.
    #
    # The funded account does NOT follow these. `mt5_executor.py --mirror-
    # pending` exists and is off; it stays off until the stage-1 comparison
    # has 30 trades over two windows, which the registration says in advance.
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-plan'; control = 'ai-xau-ds-plan-coin'; seed = 43; log = 'ai_trader_ds_plan'; promptVariant = 'plan' },
    @{ model = 'deepseek-flash'; run = 'ai-xau-ds-plan-trigger'; control = 'ai-xau-ds-plan-trigger-coin'; seed = 47; log = 'ai_trader_ds_plan_trigger'; promptVariant = 'plan-trigger' }
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
    # Passed only when a campaign names one, so every existing row keeps the
    # trader's own default - `base`, which renders byte-identical to the
    # prompt these books have always run. A campaign that says nothing about
    # its prompt gets the prompt it has always had.
    if ($c.promptVariant) { $args += "--prompt-variant=$($c.promptVariant)" }
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
    # The variant is on the line because two rows now name the same model and
    # the only thing separating them is the prompt. A start line that says
    # "claude-opus-5" twice and nothing else is a log nobody can read back.
    $variant = if ($c.promptVariant) { " prompt=$($c.promptVariant)" } else { ' prompt=base' }
    Write-Host "started $($c.model) -> $($c.run) $against (seed $($c.seed))$variant [$mode]"
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
