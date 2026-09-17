# The one step that cannot live in the repository.
#
# PASTE THIS INTO AN ELEVATED POWERSHELL ON A FRESH SERVER.
#
#   iwr -useb https://raw.githubusercontent.com/<owner>/<repo>/main/deploy/first-run.ps1 | iex
#
# ...except that a PRIVATE repository will not serve that, which is the whole
# problem: `bootstrap.ps1` needs git to be cloned, and git is what this
# installs. So on the first machine this file is copied across by hand, or its
# four commands are typed. Every machine after that is `git clone`.
#
# It installs git, clones the desk, and hands over to bootstrap.ps1.
param(
    [string]$Repo = 'https://github.com/jack2307/lambo-ai-trading.git',
    [string]$Dest = 'C:\flowdesk',
    [switch]$SkipGit
)

$ErrorActionPreference = 'Stop'

# TLS 1.2, explicitly.
#
# Windows Server 2012 R2 and 2016 default .NET to TLS 1.0, which GitHub and
# python.org have both refused for years. Without this line every download
# below fails with "the underlying connection was closed" - a message that says
# nothing about protocols and sends people looking at firewalls.
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

# The scripts below assume PowerShell 5.0 or later in a couple of places
# (Compress-Archive, and -File on Get-ChildItem). Said once, here, rather than
# failing later with a message about a missing cmdlet.
if ($PSVersionTable.PSVersion.Major -lt 5) {
    Write-Host ''
    Write-Host "PowerShell $($PSVersionTable.PSVersion) - this is Windows Server 2012 R2 or older." -ForegroundColor Yellow
    Write-Host 'That build left extended support in October 2023. It will run the desk,' -ForegroundColor Yellow
    Write-Host 'but expect friction: old .NET, old TLS defaults, and no security updates' -ForegroundColor Yellow
    Write-Host 'on a machine that holds broker credentials. Worth asking the provider for' -ForegroundColor Yellow
    Write-Host 'a 2019 or 2022 image instead.' -ForegroundColor Yellow
    Write-Host ''
}


$admin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
         ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Write-Error 'Run this from an elevated PowerShell.'; exit 1 }

function Note($m) { Write-Host "   $m" -ForegroundColor DarkGray }

# ------------------------------------------------------------------- git
if (-not $SkipGit -and -not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host '== installing git' -ForegroundColor Cyan
    # Resolved from the releases API rather than pinned: a hard-coded installer
    # URL is a link that rots, and the failure would be a 404 on a machine
    # nobody is watching. Windows Server has no winget by default, so this does
    # not use it.
    # No -UseBasicParsing here: it is an Invoke-WebRequest parameter, and
    # Invoke-RestMethod rejects it outright on Windows PowerShell.
    $rel = Invoke-RestMethod 'https://api.github.com/repos/git-for-windows/git/releases/latest'
    $asset = $rel.assets | Where-Object { $_.name -match '^Git-.*-64-bit\.exe$' } | Select-Object -First 1
    if (-not $asset) { throw 'could not find a 64-bit Git for Windows installer in the latest release' }
    $exe = Join-Path $env:TEMP $asset.name
    Note "downloading $($asset.name)"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe -UseBasicParsing
    Note 'installing silently'
    Start-Process -FilePath $exe -Wait -ArgumentList '/VERYSILENT', '/NORESTART', '/NOCANCEL', '/SP-'
    $env:PATH = "C:\Program Files\Git\cmd;$env:PATH"
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'git still not on PATH after install' }
    Note (git --version)
} else {
    Write-Host '== git already present' -ForegroundColor Cyan
}

# ----------------------------------------------------------------- clone
Write-Host '== cloning' -ForegroundColor Cyan
if (Test-Path (Join-Path $Dest '.git')) {
    Note "$Dest is already a clone; leaving it alone"
} else {
    Note 'A private repository will ask for a GitHub login here. The credential'
    Note 'manager opens a browser, which is why this wants an RDP session and'
    Note 'not a headless one.'
    git clone $Repo $Dest
    if ($LASTEXITCODE -ne 0) { throw 'clone failed' }
}

# -------------------------------------------------------------- hand over
Write-Host '== handing over to bootstrap' -ForegroundColor Cyan
Set-Location $Dest
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Dest 'deploy\bootstrap.ps1') -WithToolchain
