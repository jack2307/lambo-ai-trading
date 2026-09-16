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
Write-Host ''
Write-Host ("packed {0} ({1:N0} MB)" -f $zip, $mb)
Write-Host ''
Write-Host 'On the server: unpack it, then'
Write-Host '  powershell -NoProfile -ExecutionPolicy Bypass -File deploy\bootstrap.ps1'
Write-Host ''
Write-Host 'The live books (data\paper) are NOT in here. Copy them at cutover,'
Write-Host 'with the home desk stopped - they change every bar, and a copy taken'
Write-Host 'hours earlier would put the server a day behind the account it mirrors.'
