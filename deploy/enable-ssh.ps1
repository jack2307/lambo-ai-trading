# Put an SSH server on a Windows machine and authorise one key.
#
# PASTE THIS INTO AN ELEVATED POWERSHELL ON THE SERVER.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File enable-ssh.ps1 -PublicKey "ssh-ed25519 AAAA... comment"
#
# Two installation paths, because the usual one-liner does not exist on older
# Windows. `Add-WindowsCapability -Online -Name OpenSSH.Server` arrived with
# Windows Server 2019 and Windows 10 1809; on Server 2012 R2 and 2016 it fails
# with a message about an unrecognised capability, so those get the Win32-
# OpenSSH release from GitHub instead.
#
# Key authentication only. Password authentication over SSH on a machine
# holding broker credentials, reachable from the whole internet, is the kind of
# thing that is fine until the day it is not.
param(
    [Parameter(Mandatory = $true)][string]$PublicKey,
    [int]$Port = 22
)

$ErrorActionPreference = 'Stop'
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

$admin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
         ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Write-Error 'Run this from an elevated PowerShell.'; exit 1 }

function Step($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Note($m) { Write-Host "   $m" -ForegroundColor DarkGray }

Step 'installing OpenSSH server'
if (Get-Service sshd -ErrorAction SilentlyContinue) {
    Note 'sshd already present'
} else {
    $installed = $false

    # Features on Demand come from Windows Update, and VPS images routinely
    # ship with that service disabled. Without this, Add-WindowsCapability
    # fails with a message about a service and some devices that names neither
    # Windows Update nor OpenSSH.
    $wu = Get-Service wuauserv -ErrorAction SilentlyContinue
    if ($wu -and $wu.StartType -eq 'Disabled') {
        Note 'Windows Update service is disabled; enabling it for the install'
        Set-Service wuauserv -StartupType Manual
    }
    if ($wu -and $wu.Status -ne 'Running') {
        Start-Service wuauserv -ErrorAction SilentlyContinue
    }

    # Tried, not predicted. The capability is LISTED even on machines where it
    # cannot be installed, so asking whether it exists tells you nothing.
    if (Get-Command Add-WindowsCapability -ErrorAction SilentlyContinue) {
        try {
            Note 'trying the built-in capability'
            Add-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0' -ErrorAction Stop | Out-Null
            $installed = $true
            Note 'installed from Windows Update'
        } catch {
            Note ('built-in route failed: ' + $_.Exception.Message.Split([Environment]::NewLine)[0])
        }
    }

    if (-not $installed) {
        Note 'falling back to the Win32-OpenSSH release'
        $rel = Invoke-RestMethod 'https://api.github.com/repos/PowerShell/Win32-OpenSSH/releases/latest'
        $asset = $rel.assets | Where-Object { $_.name -eq 'OpenSSH-Win64.zip' } | Select-Object -First 1
        if (-not $asset) { throw 'no OpenSSH-Win64.zip in the latest Win32-OpenSSH release' }
        $zip = Join-Path $env:TEMP $asset.name
        Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip -UseBasicParsing
        $dest = 'C:\Program Files\OpenSSH'
        if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        if (Get-Command Expand-Archive -ErrorAction SilentlyContinue) {
            Expand-Archive -Path $zip -DestinationPath $dest -Force
        } else {
            # PowerShell 4 has no Expand-Archive; the Shell COM object is on
            # every Windows since XP.
            $shell = New-Object -ComObject Shell.Application
            $shell.NameSpace($dest).CopyHere($shell.NameSpace($zip).Items(), 0x14)
        }
        $inner = Join-Path $dest 'OpenSSH-Win64'
        if (Test-Path $inner) {
            Get-ChildItem $inner | Move-Item -Destination $dest -Force
            Remove-Item $inner -Recurse -Force
        }
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $dest 'install-sshd.ps1')
        $env:PATH = "$dest;$env:PATH"
        Note 'Win32-OpenSSH installed'
    }
}

Step 'starting the service'
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd
Note ((Get-Service sshd).Status)

Step 'firewall'
# Removed and re-made rather than left alone if it exists. A rule created with
# the wrong port is silent and looks exactly like a blocked one from outside.
Get-NetFirewallRule -Name 'flowdesk-sshd' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
New-NetFirewallRule -Name 'flowdesk-sshd' -DisplayName 'OpenSSH Server (flowdesk)' `
    -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort $Port | Out-Null
Note "opened TCP $Port"
$other = Get-NetFirewallRule -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like '*SSH*' -and $_.Name -ne 'flowdesk-sshd' }
if ($other) { $other | ForEach-Object { Note ('also present: ' + $_.DisplayName + ' [' + $_.Name + ']') } }

Step 'authorising the key'
# An ADMINISTRATOR logs in against this file and not against the one in their
# own profile. Windows OpenSSH is configured that way by default, and putting
# the key in ~\.ssh\authorized_keys instead is the single most common reason a
# correct key is refused on Windows.
$admins = 'C:\ProgramData\ssh\administrators_authorized_keys'
$existing = if (Test-Path $admins) { Get-Content $admins -Raw } else { '' }
if ($existing -notmatch [regex]::Escape($PublicKey.Split(' ')[1])) {
    Add-Content -Path $admins -Value $PublicKey -Encoding ascii
    Note 'key added'
} else {
    Note 'key already authorised'
}

# And the permissions matter: sshd refuses the file outright if anyone but
# SYSTEM and Administrators can read it, and says so only in its own log.
icacls $admins /inheritance:r /grant 'SYSTEM:F' /grant 'BUILTIN\Administrators:F' | Out-Null
Note 'permissions tightened to SYSTEM and Administrators'

Step 'disabling password authentication'
$cfg = 'C:\ProgramData\ssh\sshd_config'
if (Test-Path $cfg) {
    $text = Get-Content $cfg -Raw
    $text = $text -replace '(?m)^\s*#?\s*PasswordAuthentication\s+\w+', 'PasswordAuthentication no'
    $text = $text -replace '(?m)^\s*#?\s*PubkeyAuthentication\s+\w+', 'PubkeyAuthentication yes'
    if ($text -notmatch '(?m)^PasswordAuthentication') { $text += "`r`nPasswordAuthentication no`r`n" }
    Set-Content -Path $cfg -Value $text -Encoding ascii
    Restart-Service sshd
    Note 'passwords off, keys only, sshd restarted'
}

Write-Host "`nDone. From the other machine:" -ForegroundColor Green
Write-Host ("  ssh -i ~/.ssh/flowdesk_vps Administrator@<this machine's IP>") -ForegroundColor Green
Write-Host "`nIf it refuses the key, read C:\ProgramData\ssh\logs\sshd.log - it says" -ForegroundColor DarkGray
Write-Host "exactly why, and the reason is usually file permissions." -ForegroundColor DarkGray
