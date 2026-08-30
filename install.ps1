# install.ps1 — sets up human-detector on Windows.
#
# What it does:
#   1. Installs ffmpeg via winget if it's not already on PATH.
#   2. Installs Rust via rustup (winget) if `cargo` isn't already on PATH.
#   3. Copies .env.example to .env (if .env doesn't exist yet).
#   4. Builds the release binary.
#
# Usage (from an elevated or normal PowerShell prompt):
#   .\install.ps1

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

function Log($msg)  { Write-Host "==> $msg" -ForegroundColor Green }
function Warn($msg) { Write-Host "!! $msg" -ForegroundColor Yellow }
function Die($msg)  { Write-Host "Error: $msg" -ForegroundColor Red; exit 1 }

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Die "winget is required but wasn't found. Install ffmpeg and Rust manually: https://ffmpeg.org/download.html and https://rustup.rs, then re-run this script."
}

# ---------------------------------------------------------------------------
# 1. ffmpeg
# ---------------------------------------------------------------------------
if (Get-Command ffmpeg -ErrorAction SilentlyContinue) {
    Log "ffmpeg already installed"
} else {
    Log "ffmpeg not found — installing via winget..."
    winget install --id Gyan.FFmpeg -e --accept-source-agreements --accept-package-agreements
    Warn "You may need to restart this terminal for PATH changes to take effect."
}

# ---------------------------------------------------------------------------
# 2. Rust toolchain
# ---------------------------------------------------------------------------
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Log "Rust already installed"
} else {
    Log "Rust not found — installing via winget..."
    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
    Warn "You may need to restart this terminal for PATH changes to take effect, then re-run this script to build."
    Warn "If the build later fails with a linker error, install the 'Desktop development with C++' workload via the Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
    exit 0
}

# ---------------------------------------------------------------------------
# 3. .env
# ---------------------------------------------------------------------------
if (Test-Path .env) {
    Log ".env already exists — leaving it as is"
} else {
    Copy-Item .env.example .env
    Warn "Created .env from .env.example — edit it and fill in NIM_API_KEY and DISCORD_WEBHOOK_URL before running. Also set CAMERA_INPUT to your device name (see comments in .env)."
}

# ---------------------------------------------------------------------------
# 4. Build
# ---------------------------------------------------------------------------
Log "Building release binary (this can take a couple of minutes the first time)..."
cargo build --release

$BinPath = Join-Path $ScriptDir "target\release\human-detector.exe"
Log "Built: $BinPath"

Write-Host ""
Log "Setup complete."
Write-Host "  - Edit .env if you haven't already (NIM_API_KEY, DISCORD_WEBHOOK_URL, CAMERA_INPUT)."
Write-Host "  - Find your camera's dshow name:  ffmpeg -list_devices true -f dshow -i dummy"
Write-Host "  - Test against a single image:    $BinPath --image path\to\photo.jpg"
Write-Host "  - Run the live monitor:           $BinPath"
