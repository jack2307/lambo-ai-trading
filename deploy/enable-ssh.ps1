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
} elseif ((Get-Command Add-WindowsCapability -ErrorAction SilentlyContinue) -and
          (Get-WindowsCapability -Online -Name 'OpenSSH.Server*' -ErrorAction SilentlyContinue)) {
    Note 'using the built-in capability (Server 2019+)'
    Add-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0' | Out-Null
} else {
    Note 'no built-in capability; fetching Win32-OpenSSH (Server 2012 R2 / 2016)'
    $rel = Invoke-RestMethod 'https://api.github.com/repos/PowerShell/Win32-OpenSSH/releases/latest'
    $asset = $rel.assets | Where-Object { $_.name -eq 'OpenSSH-Win64.zip' } | Select-Object -First 1
    if (-not $asset) { throw 'no OpenSSH-Win64.zip in the latest Win32-OpenSSH release' }
    $zip = Join-Path $env:TEMP $asset.name
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip -UseBasicParsing
    # Expand-Archive needs PowerShell 5; 2012 R2 ships 4, so use the shell COM
    # object, which every Windows since XP has.
    $dest = 'C:\Program Files\OpenSSH'
    if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    $shell = New-Object -ComObject Shell.Application
    $shell.NameSpace($dest).CopyHere($shell.NameSpace($zip).Items(), 0x14)
    # The zip contains a top-level OpenSSH-Win64 folder; flatten it.
    $inner = Join-Path $dest 'OpenSSH-Win64'
    if (Test-Path $inner) {
        Get-ChildItem $inner | Move-Item -Destination $dest -Force
        Remove-Item $inner -Recurse -Force
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $dest 'install-sshd.ps1')
    Note 'Win32-OpenSSH installed'
}

Step 'starting the service'
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd
Note ((Get-Service sshd).Status)

Step 'firewall'
if (-not (Get-NetFirewallRule -Name 'flowdesk-sshd' -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -Name 'flowdesk-sshd' -DisplayName 'OpenSSH Server (flowdesk)' `
        -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort $Port | Out-Null
    Note "opened TCP $Port"
} else {
    Note 'rule already there'
}

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
