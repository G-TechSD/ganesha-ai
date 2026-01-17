#Requires -Version 5.1
<#
.SYNOPSIS
    Ganesha Installer - The Remover of Obstacles
    Cross-platform installer for Windows

.DESCRIPTION
    Downloads and installs Ganesha, the AI-powered system control tool.
    Tries pre-built binary first, falls back to building from source.

.EXAMPLE
    iwr -useb https://raw.githubusercontent.com/G-TechSD/ganesha-ai/main/install.ps1 | iex
#>

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # Faster downloads

$Version = "3.14.0-beta.2"
$Repo = "G-TechSD/ganesha-ai"
$InstallDir = if ($env:GANESHA_INSTALL_DIR) { $env:GANESHA_INSTALL_DIR } else { "$env:LOCALAPPDATA\Ganesha" }

function Write-Banner {
    Write-Host ""
    Write-Host "  ██████   █████  ███    ██ ███████ ███████ ██   ██  █████" -ForegroundColor Cyan
    Write-Host " ██       ██   ██ ████   ██ ██      ██      ██   ██ ██   ██" -ForegroundColor Cyan
    Write-Host " ██   ███ ███████ ██ ██  ██ █████   ███████ ███████ ███████" -ForegroundColor Cyan
    Write-Host " ██    ██ ██   ██ ██  ██ ██ ██           ██ ██   ██ ██   ██" -ForegroundColor Cyan
    Write-Host "  ██████  ██   ██ ██   ████ ███████ ███████ ██   ██ ██   ██" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "        ✦ The Remover of Obstacles ✦  v$Version" -ForegroundColor Yellow
    Write-Host ""
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default {
            Write-Host "❌ Unsupported architecture: $arch" -ForegroundColor Red
            exit 1
        }
    }
}

function Download-Binary {
    param([string]$Arch)

    $url = "https://github.com/$Repo/releases/download/v$Version/ganesha-windows-$Arch.zip"
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $zipFile = Join-Path $tempDir "ganesha.zip"

    Write-Host "⬇️  Downloading from: $url" -ForegroundColor Cyan

    try {
        Invoke-WebRequest -Uri $url -OutFile $zipFile -UseBasicParsing

        Write-Host "📂 Extracting..." -ForegroundColor Cyan
        Expand-Archive -Path $zipFile -DestinationPath $tempDir -Force

        # Find the exe
        $exe = Get-ChildItem -Path $tempDir -Filter "ganesha.exe" -Recurse | Select-Object -First 1
        if (-not $exe) {
            throw "Binary not found in archive"
        }

        # Install
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        }

        Copy-Item -Path $exe.FullName -Destination "$InstallDir\ganesha.exe" -Force

        Remove-Item -Path $tempDir -Recurse -Force
        Write-Host "✅ Installed to: $InstallDir\ganesha.exe" -ForegroundColor Green
        return $true
    }
    catch {
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
        return $false
    }
}

function Build-FromSource {
    Write-Host ""
    Write-Host "🔨 Pre-built binary not available. Building from source..." -ForegroundColor Yellow
    Write-Host ""

    # Check for Rust
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "📥 Installing Rust..." -ForegroundColor Cyan
        $rustupUrl = "https://win.rustup.rs/x86_64"
        $rustupExe = "$env:TEMP\rustup-init.exe"
        Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe -UseBasicParsing
        Start-Process -FilePath $rustupExe -ArgumentList "-y", "--default-toolchain", "stable" -Wait -NoNewWindow
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

        # Verify
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            Write-Host "❌ Failed to install Rust. Please install manually from https://rustup.rs" -ForegroundColor Red
            exit 1
        }
    }

    # Check for Git
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Write-Host "❌ Git is required to build from source." -ForegroundColor Red
        Write-Host "   Install from: https://git-scm.com/download/win" -ForegroundColor Yellow
        exit 1
    }

    # Clone and build
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }

    Write-Host "📥 Cloning repository..." -ForegroundColor Cyan
    git clone --depth 1 "https://github.com/$Repo.git" "$tempDir\ganesha" 2>$null
    if ($LASTEXITCODE -ne 0) {
        git clone --depth 1 "https://github.com/G-TechSD/ganesha-ai.git" "$tempDir\ganesha"
    }

    Set-Location "$tempDir\ganesha\ganesha-rs"

    Write-Host "🔨 Building (this may take several minutes)..." -ForegroundColor Cyan
    cargo build --release

    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }

    # Install
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    }

    Copy-Item -Path "target\release\ganesha.exe" -Destination "$InstallDir\ganesha.exe" -Force

    Set-Location $env:TEMP
    Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host "✅ Built and installed to: $InstallDir\ganesha.exe" -ForegroundColor Green
}

function Setup-Path {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")

    if ($userPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "📍 Adding $InstallDir to PATH..." -ForegroundColor Cyan

        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        $env:Path = "$env:Path;$InstallDir"

        Write-Host "   Added to User PATH" -ForegroundColor Green
    }
}

function Verify-Install {
    $exe = "$InstallDir\ganesha.exe"

    if (Test-Path $exe) {
        Write-Host ""
        Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Gray
        & $exe --version 2>$null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "ganesha v$Version" -ForegroundColor White
        }
        Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Gray
        Write-Host ""
        Write-Host "🎉 Installation complete!" -ForegroundColor Green
        Write-Host ""
        Write-Host '   Get started:  ganesha "hello world"' -ForegroundColor White
        Write-Host "   Interactive:  ganesha -i" -ForegroundColor White
        Write-Host "   Help:         ganesha --help" -ForegroundColor White
        Write-Host ""
        Write-Host "⚠️  Restart your terminal to use the 'ganesha' command." -ForegroundColor Yellow
    }
    else {
        Write-Host "❌ Installation failed" -ForegroundColor Red
        exit 1
    }
}

# Main
function Main {
    Write-Banner

    $arch = Get-Architecture
    Write-Host "📦 Detected: Windows $arch" -ForegroundColor Cyan

    # Try downloading pre-built binary first
    if (Download-Binary -Arch $arch) {
        Setup-Path
        Verify-Install
    }
    else {
        Write-Host "⚠️  Pre-built binary not available for Windows-$arch" -ForegroundColor Yellow
        Build-FromSource
        Setup-Path
        Verify-Install
    }
}

Main
