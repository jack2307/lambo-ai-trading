# Prepare a fresh Windows server to run the desk. RUN THIS ON THE SERVER.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\bootstrap.ps1
#
# Installs Python 3.9 and the pinned packages, lays out the directories, and
# checks the things that have actually gone wrong before. It does NOT install
# MetaTrader, log anything in, or start trading: those need decisions and
# credentials, and the script prints exactly what is left to do.
#
# Safe to run twice. Every step checks before it acts.
param(
    [string]$Root = '',
    [string]$PythonVersion = '3.9.13',
    [switch]$SkipPython,
    # Install Rust and Node so the server can build what it pulls.
    #
    # Needed when the desk arrives by `git clone` rather than as a packed zip,
    # because `target\` and `ui\dist\` are gitignored - a pull brings source
    # and nothing runnable. About 5.7 GB all in, measured: 3.2 GB of rustup and
    # cargo, 2.1 GB of release build output, 0.3 GB of node_modules. The 29 GB
    # `target\debug` that makes the home tree enormous is test and dev output
    # a server never produces.
    [switch]$WithToolchain
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


# Installing Python for all users into C:\Python39, and writing into it
# afterwards, both need an elevated session. Checked up front rather than
# discovered halfway through: a run that installs Python and then cannot
# install the packages leaves a machine that looks ready and is not.
$admin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
         ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $Root) {
    $here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $Root = Split-Path -Parent $here
}

function Step($n, $what) { Write-Host "`n[$n] $what" -ForegroundColor Cyan }
function Ok($what) { Write-Host "    OK   $what" -ForegroundColor Green }
function Todo($what) { Write-Host "    TODO $what" -ForegroundColor Yellow }
function Note($what) { Write-Host "    $what" -ForegroundColor DarkGray }

Write-Host "flowdesk bootstrap - $Root"
if (-not $admin) {
    Write-Host 'Not running as Administrator. Installing Python and writing into' -ForegroundColor Yellow
    Write-Host 'C:\Python39 will fail. Re-run this from an elevated PowerShell.' -ForegroundColor Yellow
}

# ---------------------------------------------------------------- 1. Python
Step 1 'Python 3.9'
$py = 'C:\Python39\python.exe'
if (Test-Path $py) {
    $v = (& $py --version 2>&1) -replace 'Python ', ''
    Ok "already installed: $v"
} elseif ($SkipPython) {
    Todo 'Python 3.9 missing and -SkipPython was passed'
} else {
    # C:\Python39 exactly: the desk's launchers hard-code it, and a Python
    # somewhere else would have every one of them fail at Start-Process with a
    # message about a file not found rather than about a missing interpreter.
    $url = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-amd64.exe"
    $exe = Join-Path $env:TEMP "python-$PythonVersion-amd64.exe"
    Note "downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $exe -UseBasicParsing
    Note 'installing to C:\Python39 (all users, on PATH)'
    Start-Process -FilePath $exe -Wait -ArgumentList @(
        '/quiet', 'InstallAllUsers=1', 'TargetDir=C:\Python39',
        'PrependPath=1', 'Include_pip=1', 'Include_test=0'
    )
    if (Test-Path $py) { Ok 'installed' } else { throw 'Python install did not produce C:\Python39\python.exe' }
}

# ------------------------------------------------------------- 2. packages
Step 2 'Python packages'
if (Test-Path $py) {
    $req = Join-Path $Root 'deploy\requirements.txt'
    # pip is NOT self-upgraded here. It cannot replace its own exe without
    # elevation, and the failure is an exit code rather than a PowerShell
    # error, so it slips past `$ErrorActionPreference` and the run goes on
    # looking fine. The pip that ships with 3.9.13 installs all of this.
    & $py -m pip install --disable-pip-version-check -r $req
    if ($LASTEXITCODE -ne 0) {
        throw "pip install failed (exit $LASTEXITCODE). Elevated? Packages are written into C:\Python39."
    }
    $check = & $py -c "import MetaTrader5, tomli, numpy, pyarrow, requests, zoneinfo; print(MetaTrader5.__version__)"
    Ok "MetaTrader5 $check, and the rest import"
}

# ----------------------------------------------------------- 2b. toolchain
if ($WithToolchain) {
    Step '2b' 'Rust and Node'
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Ok "cargo already installed"
    } else {
        $rustup = Join-Path $env:TEMP 'rustup-init.exe'
        Note 'downloading rustup'
        Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $rustup -UseBasicParsing
        & $rustup -y --default-toolchain stable --profile minimal
        if ($LASTEXITCODE -ne 0) { throw 'rustup failed' }
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        Ok 'rust installed'
    }
    if (Get-Command node -ErrorAction SilentlyContinue) {
        Ok ("node " + (node --version))
    } else {
        Todo 'Node.js missing - install the LTS MSI from nodejs.org, then re-run'
        Note 'Not scripted: the installer has no reliable silent switch across'
        Note 'versions, and a half-installed Node is worse than none.'
    }
}

# ----------------------------------------------------------- 3. directories
Step 3 'Directories'
foreach ($d in 'data\paper\logs', 'data\live', 'data\news') {
    $p = Join-Path $Root $d
    if (-not (Test-Path $p)) { New-Item -ItemType Directory -Force -Path $p | Out-Null }
}
Ok 'data\paper, data\live, data\news'

# ------------------------------------------------------------- 4. the exe
Step 4 'fd-api'
$api = Join-Path $Root 'target\release\fd-api.exe'
if (Test-Path $api) {
    Ok ("{0:N0} MB" -f ((Get-Item $api).Length / 1MB))
} else {
    Todo 'target\release\fd-api.exe missing - it comes from the zip, not from here'
}
$dist = Join-Path $Root 'ui\dist\index.html'
if (Test-Path $dist) { Ok 'ui\dist present' } else { Todo 'ui\dist missing - the desk will serve the API with no client' }

# ------------------------------------------------------------ 5. MetaTrader
Step 5 'MetaTrader terminals'
$live = 'C:\Program Files\MetaTrader 5\terminal64.exe'
$demo = 'C:\MT5-demo\terminal64.exe'
if (Test-Path $live) { Ok "live terminal at $live" } else { Todo "install MT5 from VANTAGE's own download page to $live" }
if (Test-Path $demo) {
    Ok "second terminal at $demo"
    # The trap that cost an afternoon on 2026-09-16, checked rather than
    # remembered. A terminal copied from Program Files starts with an almost
    # empty servers.dat, cannot resolve the broker's server name, and then the
    # login does NOTHING - no error, no log line, no dialog. Size is the tell:
    # a real broker list is hundreds of kilobytes, an empty one is about 44 KB.
    $srv = 'C:\MT5-demo\config\servers.dat'
    if (Test-Path $srv) {
        $kb = (Get-Item $srv).Length / 1KB
        if ($kb -lt 200) {
            Todo ("servers.dat is only {0:N0} KB - almost certainly the empty default." -f $kb)
            Note 'Copy the live terminal''s own list over it, or the demo login will'
            Note 'silently do nothing and write no log line at all:'
            Note '  %APPDATA%\MetaQuotes\Terminal\<hash>\config\servers.dat'
            Note '  -> C:\MT5-demo\config\servers.dat'
        } else {
            Ok ("servers.dat looks like a real broker list ({0:N0} KB)" -f $kb)
        }
    } else {
        Todo 'C:\MT5-demo\config\servers.dat missing'
    }
} else {
    Todo 'second terminal for the demo account not present yet'
    Note 'Copy the whole "C:\Program Files\MetaTrader 5" folder to C:\MT5-demo,'
    Note 'then copy servers.dat across as above. One terminal holds one account,'
    Note 'so mirroring a demo while reading live prices needs two installations.'
}

# --------------------------------------------------------------- 6. config
Step 6 'Config'
$acct = Join-Path $Root 'config\accounts.toml'
$fromHome = Join-Path $Root 'config\accounts.toml.from-home'
if (Test-Path $acct) {
    Ok 'config\accounts.toml present'
} elseif (Test-Path $fromHome) {
    Todo 'config\accounts.toml.from-home needs renaming and its terminal paths checked'
    Note 'It names the HOME machine''s terminals. Read it - the comments explain'
    Note 'the rules - then save it as accounts.toml with this server''s paths.'
} else {
    Todo 'no account registry; nothing will be mirrored until there is one'
}
$local = Join-Path $Root 'config\local.toml'
if (Test-Path $local) { Ok 'config\local.toml present' } else { Todo 'config\local.toml missing - Telegram and DeepSeek keys live there' }

# --------------------------------------------------------------- what is left
Write-Host "`nWhat this script cannot do for you" -ForegroundColor Cyan
Write-Host @'
  1. Install MetaTrader 5 from Vantage's own download page (not MetaQuotes' -
     the broker build already knows the server names).
  2. Log the live terminal in, and the demo terminal in, by hand.
  3. Turn ON Algo Trading in each terminal, or every order comes back
     10027 "AutoTrading disabled by client" with nothing else wrong. Put
     [Experts] AllowLiveTrading=1 in the start config so it survives a restart.
  4. Log in the Claude and Codex CLIs - they authenticate against your PLAN
     through a browser, so they need a session on this machine.
  5. Copy config\local.toml across by hand.

  If the desk arrived by git clone, run this once with -WithToolchain, then
  use deploy\update.ps1 from then on - it pulls, rebuilds and restarts in the
  right order.

  Then, with the HOME desk stopped so two machines never mirror one account:
     copy data\paper across, start the pollers, start fd-api, and run the
     executors in -DryRun first. Only pass -Live once you have watched a bar
     or two agree.
'@
