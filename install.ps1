param(
  [string]$Version = "latest",
  [switch]$SkipWslInstall
)

$ErrorActionPreference = "Stop"

$REPO = "weykon/agent-hand"
$BIN_NAME = "agent-hand"
$INSTALL_URL = "https://raw.githubusercontent.com/$REPO/master/install.sh"

function Info($msg)  { Write-Host "[INFO] $msg" -ForegroundColor Green }
function Warn($msg)  { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Fail($msg)  { Write-Error "[ERROR] $msg"; exit 1 }

function Test-Admin {
  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
  $principal = New-Object Security.Principal.WindowsPrincipal($identity)
  return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# ── Step 1: Check if WSL is available ──

$wslExists = Get-Command wsl.exe -ErrorAction SilentlyContinue

if ($wslExists) {
  # Check if a distro is actually installed
  $distros = wsl.exe --list --quiet 2>$null
  if ($LASTEXITCODE -eq 0 -and $distros) {
    Info "WSL is installed with a Linux distro."
  } else {
    Info "WSL is installed but no Linux distro found. Installing Ubuntu..."
    if (-not (Test-Admin)) {
      Warn "Installing a WSL distro requires Administrator privileges."
      Warn "Please re-run this script as Administrator, or manually run:"
      Write-Host "  wsl --install -d Ubuntu" -ForegroundColor Cyan
      exit 1
    }
    wsl --install -d Ubuntu
    Write-Host ""
    Warn "Ubuntu is being installed. You may need to restart your computer."
    Warn "After restart, open Ubuntu from the Start menu to finish setup,"
    Warn "then re-run this installer."
    exit 0
  }
} else {
  if ($SkipWslInstall) {
    Fail "WSL is not installed and -SkipWslInstall was specified. agent-hand requires WSL (tmux)."
  }

  Info "WSL is not installed. agent-hand requires Linux (tmux) to manage sessions."
  Info "Installing WSL with Ubuntu..."
  Write-Host ""

  if (-not (Test-Admin)) {
    Warn "WSL installation requires Administrator privileges."
    Warn "Please re-run this script as Administrator:"
    Write-Host "  Start-Process powershell -Verb RunAs -ArgumentList '-File', '$($MyInvocation.MyCommand.Path)'" -ForegroundColor Cyan
    exit 1
  }

  wsl --install -d Ubuntu
  Write-Host ""
  Warn "WSL + Ubuntu is being installed. You will need to restart your computer."
  Warn "After restart:"
  Warn "  1. Open 'Ubuntu' from the Start menu and create a Linux user"
  Warn "  2. Re-run this installer to finish agent-hand setup"
  exit 0
}

# ── Step 2: Install agent-hand inside WSL ──

Info "Installing $BIN_NAME inside WSL..."
Write-Host ""

if ($Version -eq "latest") {
  $installCmd = "curl -fsSL $INSTALL_URL | bash"
} else {
  $installCmd = "curl -fsSL $INSTALL_URL | bash -s -- --version $Version"
}

wsl bash -c $installCmd

if ($LASTEXITCODE -ne 0) {
  Fail "Installation inside WSL failed."
}

# ── Step 3: Create a Windows shim so 'agent-hand' works from PowerShell ──

$shimDir = Join-Path $env:USERPROFILE ".local\bin"
New-Item -ItemType Directory -Force -Path $shimDir | Out-Null

$shimPath = Join-Path $shimDir "$BIN_NAME.cmd"
$shimContent = "@echo off`r`nwsl $BIN_NAME %*"
Set-Content -Path $shimPath -Value $shimContent -Encoding ASCII

# Add shim dir to user PATH if needed
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$shimDir*") {
  [Environment]::SetEnvironmentVariable("Path", "$shimDir;$userPath", "User")
  Info "Added '$shimDir' to your user PATH."
}

Info "Installation complete!"
Write-Host ""
Write-Host "You can now run agent-hand in two ways:" -ForegroundColor Cyan
Write-Host "  1. From PowerShell (new terminal): agent-hand"
Write-Host "  2. From WSL/Ubuntu terminal:       agent-hand"
Write-Host ""
Write-Host "The PowerShell command forwards to WSL automatically."
Write-Host ""
