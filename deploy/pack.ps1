# Build the desk and pack what a server actually needs. RUN THIS AT HOME.
#
#   powershell -NoProfile -File deploy\pack.ps1
#
# Produces deploy\flowdesk-<date>.zip, about 45 MB. Copy it to the VPS (drag it
# over an RDP session is fine), unpack it, and run bootstrap.ps1 there.
#
# WHAT IS DELIBERATELY LEFT OUT
#
#   target\           31 GB of Rust build tree. The server needs the 12 MB exe,
#                     not the toolchain that made it. Backtest sweeps - the only
#                     thing that wants many cores - stay at home, where that
#                     tree already is.
#   ui\node_modules   266 MB to produce 3 MB of bundle. Same argument.
#   data\bars         394 MB of historical Parquet. Research reads it; the live
#                     desk does not - a paper run with no stored bars warms up
#                     from its poller instead. It can be copied later if the
#                     server ever needs to backtest, which it should not.
#   data\paper        The live books. NOT packed here on purpose: it is the one
#                     thing that changes every bar, so it is copied at cutover
#                     and not hours beforehand. See the note at the end.
#   config\local.toml Secrets. Copied by hand, once, so they never sit in a zip
#                     on two machines.
param(
    [string]$Root = '',
    [string]$Out = ''
)

if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}
Set-Location $Root

# Resolved rather than assumed. `cargo` is on PATH in a shell that has sourced
# the rustup env and in no other, and the windows-gnu target additionally needs
# the WinLibs binutils for `as.exe` - a bare `cargo build` in a plain
# PowerShell fails with a linker error that says nothing about either.
function Resolve-Tool([string]$name, [string[]]$candidates) {
    $found = (Get-Command $name -ErrorAction SilentlyContinue).Source
    if ($found) { return Split-Path -Parent $found }
    foreach ($c in $candidates) { if (Test-Path (Join-Path $c "$name.exe")) { return $c } }
    return $null
}

$cargoDir = Resolve-Tool 'cargo' @("$env:USERPROFILE\.cargo\bin")
if (-not $cargoDir) { Write-Error 'cargo not found; install rustup or add it to PATH'; exit 1 }
$winlibs = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = if (Test-Path $winlibs) { "$cargoDir;$winlibs;$env:PATH" } else { "$cargoDir;$env:PATH" }

# THE TREE IS MEASURED HERE, BEFORE ANYTHING IS BUILT OR WRITTEN.
#
# It used to be measured at the end, after this script had created
# `deploy\staged-<stamp>\` inside the repo - so `git status --porcelain`
# counted pack's own output and EVERY VERSION said `git_dirty: true`. A
# warning that is always on is a warning nobody reads, and this one printed
# "the tree had uncommitted changes, so this binary contains something the
# hash above does not name" on every clean pack.
#
# It was also inconsistent with the binary. `build.rs` runs its own
# `git status --porcelain` during the cargo build below, before any output
# folder exists, so the exe's baked `FD_GIT_DIRTY` was correct while the
# VERSION file beside it was not - and `update.ps1` warns off the binary's
# value, so the two disagreed with nobody comparing them.
#
# Measured at the same moment build.rs measures it, the two agree by
# construction. `.gitignore` now also covers `deploy/staged-*` and
# `deploy/task-backup-*`, which stops a LEFTOVER folder from a previous run
# dirtying the tree that cargo is about to read - that one reached the binary.
$head = (git rev-parse HEAD).Trim()
$porcelain = @(git status --porcelain | Where-Object { $_ })
$dirty = [bool]$porcelain.Count

Write-Host 'building fd-api...'
& (Join-Path $cargoDir 'cargo.exe') build --release -p fd-api
if ($LASTEXITCODE -ne 0) { Write-Error 'cargo build failed'; exit 1 }

Write-Host 'building the client...'
Push-Location ui
npm run build
$uiOk = $LASTEXITCODE -eq 0
Pop-Location
if (-not $uiOk) { Write-Error 'ui build failed'; exit 1 }

$stamp = Get-Date -Format 'yyyyMMdd-HHmm'
$stage = Join-Path $env:TEMP "flowdesk-$stamp"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

# The exe, the client, the scripts, the config, the research record.
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'target\release') | Out-Null
Copy-Item 'target\release\fd-api.exe' (Join-Path $stage 'target\release') -Force
foreach ($d in 'py', 'config', 'docs', 'deploy') {
    Copy-Item $d (Join-Path $stage $d) -Recurse -Force
}
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'ui') | Out-Null
Copy-Item 'ui\dist' (Join-Path $stage 'ui\dist') -Recurse -Force

# Two files never travel in the zip, for two different reasons.
#
# `local.toml` holds the Telegram token and the DeepSeek key. It is gitignored
# for the same reason it is skipped here: a secret that sits in an archive on
# two machines is a secret in two more places than it needs to be.
#
# `guards.toml` is this machine's RUNTIME override of the risk rules. Letting
# it travel would start the server under rules that `config/default.toml` does
# not state and nobody chose for it - the desk records every guard change into
# every book's file precisely so the rules can never move unannounced, and
# shipping the override layer would move them by copy instead.
foreach ($drop in 'config\local.toml', 'config\guards.toml') {
    $p = Join-Path $stage $drop
    if (Test-Path $p) {
        Remove-Item $p -Force
        Write-Host "$drop excluded"
    }
}

# The account registry names THIS machine's terminals. Left in so its comments
# travel, but renamed so nobody starts executors against paths that do not
# exist on the server and wonders why nothing happens.
$acct = Join-Path $stage 'config\accounts.toml'
if (Test-Path $acct) {
    Move-Item $acct (Join-Path $stage 'config\accounts.toml.from-home') -Force
}

$zip = if ($Out) { $Out } else { Join-Path $Root "deploy\flowdesk-$stamp.zip" }
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
Remove-Item $stage -Recurse -Force

$mb = (Get-Item $zip).Length / 1MB

# ------------------------------------------------------------ the staged pair
#
# The zip is for standing a server UP. This folder is for UPDATING one, and it
# exists because the server has no toolchain: the VPS has git and nothing else,
# so every deploy there is a binary built here and copied over, and
# `update.ps1 -Staged <folder>` is the only path that installs it.
#
# WHY A VERSION FILE. Finding 8 of the staleness note: the artefacts that
# decide what the desk DOES - the binary and the client - are the two that
# `git pull` cannot touch, and a dated zip filename is an invitation to ship
# the wrong one. mtime cannot help, because a copy stamps the destination. So
# the folder carries the commit it was built from, and update.ps1 can compare
# that to the pulled HEAD BEFORE it stops anything.
#
# THE HASH IS THE TREE'S, AND THE FILE SAYS SO. `git rev-parse HEAD` describes
# the checkout that cargo was pointed at, not the bytes that came out. Those
# are the same thing only if the build actually ran and actually wrote. So the
# exe is also SCANNED for that hash - version.rs bakes it in with env!(), so it
# sits in .rodata as a plain literal - and `binary_contains_hash` records
# whether the bytes corroborate the claim. A false there is the stale-binary
# case caught at pack time instead of at readiness.
# `-Out` names the zip; the staged pair goes beside it rather than always
# under `deploy\`. It used to ignore -Out entirely, so asking for the output
# somewhere else moved half of it and left the half that matters - the folder
# update.ps1 -Staged actually installs from - in the repo.
$stagedParent = if ($Out) { Split-Path -Parent $Out } else { Join-Path $Root 'deploy' }
if (-not $stagedParent) { $stagedParent = Join-Path $Root 'deploy' }
$staged = Join-Path $stagedParent "staged-$stamp"
if (Test-Path $staged) { Remove-Item $staged -Recurse -Force }
New-Item -ItemType Directory -Force -Path $staged | Out-Null

$exe = Join-Path $Root 'target\release\fd-api.exe'
Copy-Item $exe (Join-Path $staged 'fd-api.exe') -Force
Copy-Item (Join-Path $Root 'ui\dist') (Join-Path $staged 'dist') -Recurse -Force
# update.ps1 travels with the pair so the operator copies ONE folder and the
# script that installs it cannot be a different vintage from the artefacts.
Copy-Item (Join-Path $Root 'deploy\update.ps1') (Join-Path $staged 'update.ps1') -Force

$corroborated = $false
try {
    $bytes = [IO.File]::ReadAllBytes($exe)
    $corroborated = [Text.Encoding]::ASCII.GetString($bytes).Contains($head)
} catch { }
$version = [ordered]@{
    git_hash              = $head
    git_dirty             = $dirty
    # A true flag that does not say what it is about is a flag nobody can
    # act on. The first few paths are enough to tell "I forgot to commit
    # something" from "a build artefact is untracked".
    git_dirty_files       = @($porcelain | Select-Object -First 10)
    built_at_utc          = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    binary_sha256         = (Get-FileHash -Path $exe -Algorithm SHA256).Hash
    binary_contains_hash  = $corroborated
    bundle                = (Get-ChildItem (Join-Path $staged 'dist') -Filter 'index.html' | Select-Object -First 1).Name
    packed_by             = 'deploy/pack.ps1'
}
$version | ConvertTo-Json | Out-File (Join-Path $staged 'VERSION') -Encoding utf8

Write-Host ''
Write-Host ("packed {0} ({1:N0} MB)" -f $zip, $mb)
Write-Host ("staged {0}" -f $staged)
Write-Host ("  git_hash   {0}" -f $head)
Write-Host ("  git_dirty  {0}" -f $dirty) -ForegroundColor $(if ($dirty) { 'Yellow' } else { 'DarkGray' })
if ($dirty) {
    Write-Host '  the tree had uncommitted changes, so this binary contains something' -ForegroundColor Yellow
    Write-Host '  the hash above does not name. Deployable, and worth knowing.' -ForegroundColor Yellow
}
if ($corroborated) {
    Write-Host '  binary corroborates the hash (the commit string is in the exe)' -ForegroundColor DarkGray
} else {
    Write-Host '  WARNING: the exe does NOT contain that commit string.' -ForegroundColor Red
    Write-Host '  Either it was not rebuilt, or the write failed. Do not ship this.' -ForegroundColor Red
}
Write-Host ''
Write-Host 'To update a running server, copy the STAGED FOLDER and run, on the server:'
Write-Host ("  powershell -NoProfile -File deploy\update.ps1 -ServerOnly -Staged <folder>")
Write-Host ''
Write-Host 'On a NEW server: unpack the zip, then'
Write-Host '  powershell -NoProfile -ExecutionPolicy Bypass -File deploy\bootstrap.ps1'
Write-Host ''
Write-Host 'The live books (data\paper) are NOT in here. Copy them at cutover,'
Write-Host 'with the home desk stopped - they change every bar, and a copy taken'
Write-Host 'hours earlier would put the server a day behind the account it mirrors.'
