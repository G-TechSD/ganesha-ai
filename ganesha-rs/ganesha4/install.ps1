# Ganesha Windows Installer
# Usage: iex ((New-Object Net.WebClient).DownloadString('https://bill-dev-linux-1/gtechsd/ganesha-ai/-/raw/ganesha-4.0-design/ganesha-rs/ganesha4/install.ps1'))

$ErrorActionPreference = "Stop"
$installDir = "$env:USERPROFILE\.ganesha"
$binDir = "$env:USERPROFILE\.local\bin"

Write-Host "🐘 Installing Ganesha..." -ForegroundColor Cyan

# Check for Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Rust..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri "https://win.rustup.rs" -OutFile "$env:TEMP\rustup-init.exe"
    & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
}

# Create directories
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

# Clone or update repo
$repoDir = "$installDir\ganesha-ai"
if (Test-Path $repoDir) {
    Write-Host "Updating repository..." -ForegroundColor Yellow
    Push-Location $repoDir
    git fetch origin
    git checkout ganesha-4.0-design
    git pull origin ganesha-4.0-design
    Pop-Location
} else {
    Write-Host "Cloning repository..." -ForegroundColor Yellow
    git clone --branch ganesha-4.0-design https://bill-dev-linux-1/gtechsd/ganesha-ai.git $repoDir
}

# Build
Write-Host "Building Ganesha (this may take a few minutes)..." -ForegroundColor Yellow
Push-Location "$repoDir\ganesha-rs\ganesha4"
cargo build --release
Pop-Location

# Install binary
Copy-Item "$repoDir\ganesha-rs\ganesha4\target\release\ganesha.exe" "$binDir\ganesha.exe" -Force

# Add to PATH if not already there
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$binDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$userPath;$binDir", "User")
    $env:PATH = "$env:PATH;$binDir"
    Write-Host "Added $binDir to PATH" -ForegroundColor Green
}

Write-Host ""
Write-Host "✓ Ganesha installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Open a new terminal and run: ganesha" -ForegroundColor Cyan
Write-Host ""
Write-Host "Optional: Run 'ganesha voice setup' for voice input (requires Python)" -ForegroundColor DarkGray
