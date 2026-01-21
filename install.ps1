param(
  [string]$Prefix = "",
  [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

$REPO = "weykon/agent-hand"
$BIN_NAME = "agent-hand"

function Info($msg) { Write-Host "[INFO] $msg" }
function Warn($msg) { Write-Host "[WARN] $msg" }
function Fail($msg) { Write-Error "[ERROR] $msg" }

if ([string]::IsNullOrWhiteSpace($Prefix)) {
  $Prefix = Join-Path $env:USERPROFILE ".local\bin"
}
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null

$target = "x86_64-pc-windows-msvc"
$asset = "$BIN_NAME-$target.tar.gz"

$urlBase = "https://github.com/$REPO/releases"
if ($Version -eq "latest") {
  $url = "$urlBase/latest/download/$asset"
} else {
  $url = "$urlBase/download/$Version/$asset"
}

$tmpdir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $tmpdir | Out-Null

try {
  $tarPath = Join-Path $tmpdir $asset
  Info "Downloading $url"
  Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $tarPath

  $tarExe = Get-Command tar -ErrorAction SilentlyContinue
  if (-not $tarExe) {
    Fail "tar not found. Please install via WSL (recommended) or Git for Windows / MSYS2, or use agent-hand.exe from the GitHub Release assets."
  }

  & tar -xzf $tarPath -C $tmpdir

  $tmpBin = Join-Path $tmpdir "$BIN_NAME.exe"
  if (-not (Test-Path $tmpBin)) {
    Fail "Malformed archive: $asset (missing $BIN_NAME.exe)"
  }

  $dest = Join-Path $Prefix "$BIN_NAME.exe"
  Copy-Item -Force $tmpBin $dest

  Warn "Note: agent-hand requires tmux. On Windows, using WSL is recommended."
  Info "Installed $BIN_NAME.exe to $dest"
  Write-Host ""
  Write-Host "Next steps:"
  Write-Host "  1. Ensure '$Prefix' is in your PATH"
  Write-Host "  2. Run: $BIN_NAME"
} finally {
  Remove-Item -Recurse -Force $tmpdir -ErrorAction SilentlyContinue
}
