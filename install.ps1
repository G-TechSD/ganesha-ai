# Ganesha Installer for Windows
# The Remover of Obstacles

Write-Host "🐘 Ganesha Installer - The Remover of Obstacles" -ForegroundColor Yellow
Write-Host "=============================================" -ForegroundColor Gray

# Check for Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust not found. Installing rustup..." -ForegroundColor Cyan
    # Download and run rustup-init
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupExe = "$env:TEMP\rustup-init.exe"
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe
    Start-Process -FilePath $rustupExe -ArgumentList "-y" -Wait
    
    # Reload path (this is tricky in current session, might need restart)
    $env:Path += ";$env:USERPROFILE\.cargo\bin"
    Write-Host "Rust installed. You may need to restart your terminal after this." -ForegroundColor Green
}

# Determine script location to find source
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceDir = Join-Path $ScriptDir "ganesha-rs"

if (-not (Test-Path $SourceDir)) {
    Write-Error "Could not find ganesha-rs directory at $SourceDir"
    exit 1
}

Set-Location $SourceDir

Write-Host "Building Ganesha with Voice support..." -ForegroundColor Cyan
cargo build --release --features "voice"

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed."
    exit 1
}

# Install to standard location (e.g., %LOCALAPPDATA%\Ganesha)
$InstallDir = "$env:LOCALAPPDATA\Ganesha"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

$TargetExe = "$InstallDir\ganesha.exe"
Copy-Item -Path "target\release\ganesha.exe" -Destination $TargetExe -Force

Write-Host "Installed to $TargetExe" -ForegroundColor Green

# Add to User PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Cyan
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "Added to PATH. Restart terminal to use 'ganesha' command." -ForegroundColor Yellow
}

Write-Host "`n✅ Ganesha installed successfully!" -ForegroundColor Green
Write-Host "Run 'ganesha --help' to get started."
