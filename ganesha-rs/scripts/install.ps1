# Ganesha Installer for Windows
# https://bill-dev-linux-1/gtechsd/ganesha-ai
#
# Usage (PowerShell):
#   iwr -useb https://bill-dev-linux-1/gtechsd/ganesha-ai/-/releases/permalink/latest/downloads/install.ps1 | iex
#
# Or download and run:
#   .\install.ps1

$ErrorActionPreference = "Stop"

# Configuration
$GitLabUrl = "https://bill-dev-linux-1/gtechsd/ganesha-ai"
$Version = if ($env:GANESHA_VERSION) { $env:GANESHA_VERSION } else { "latest" }
$InstallDir = "$env:LOCALAPPDATA\Ganesha"
$BinaryName = "ganesha.exe"

# Banner
Write-Host ""
Write-Host "===========================================================" -ForegroundColor Cyan
Write-Host "              Ganesha Installer for Windows" -ForegroundColor Cyan
Write-Host "             The Remover of Obstacles" -ForegroundColor Cyan
Write-Host "===========================================================" -ForegroundColor Cyan
Write-Host ""

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }

if ($Arch -ne "x86_64") {
    Write-Host "Error: Only 64-bit Windows is supported" -ForegroundColor Red
    exit 1
}

Write-Host "Platform: windows-$Arch" -ForegroundColor DarkGray

# Determine download URL
if ($Version -eq "latest") {
    $DownloadUrl = "$GitLabUrl/-/releases/permalink/latest/downloads/ganesha-windows-x86_64.zip"
} else {
    $DownloadUrl = "$GitLabUrl/-/releases/$Version/downloads/ganesha-windows-x86_64.zip"
}

Write-Host "Download URL: $DownloadUrl" -ForegroundColor DarkGray
Write-Host ""

# Create temp directory
$TmpDir = New-Item -ItemType Directory -Path "$env:TEMP\ganesha-install-$(Get-Random)" -Force

try {
    # Download
    Write-Host "Downloading Ganesha..." -ForegroundColor Cyan
    $ZipPath = "$TmpDir\ganesha.zip"

    try {
        # Try with progress disabled for faster download
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
    }
    catch {
        Write-Host "Download failed: $_" -ForegroundColor Red
        Write-Host "URL: $DownloadUrl" -ForegroundColor DarkGray
        exit 1
    }

    # Extract
    Write-Host "Extracting..." -ForegroundColor Cyan
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

    # Find the binary
    $BinaryPath = Get-ChildItem -Path $TmpDir -Filter "ganesha.exe" -Recurse | Select-Object -First 1

    if (-not $BinaryPath) {
        Write-Host "Error: Binary not found in archive" -ForegroundColor Red
        exit 1
    }

    # Create install directory
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Install
    Write-Host "Installing to $InstallDir..." -ForegroundColor Cyan
    Copy-Item -Path $BinaryPath.FullName -Destination "$InstallDir\$BinaryName" -Force

    Write-Host ""
    Write-Host "[OK] Ganesha installed successfully!" -ForegroundColor Green
    Write-Host ""

    # Check if in PATH
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host "[!] $InstallDir is not in your PATH" -ForegroundColor Yellow
        Write-Host ""

        $AddToPath = Read-Host "Add to PATH? [Y/n]"
        if ($AddToPath -eq "" -or $AddToPath -eq "Y" -or $AddToPath -eq "y") {
            $NewPath = "$UserPath;$InstallDir"
            [Environment]::SetEnvironmentVariable("PATH", $NewPath, "User")
            $env:PATH = "$env:PATH;$InstallDir"
            Write-Host ""
            Write-Host "[OK] Added to PATH. Restart your terminal to use 'ganesha' command." -ForegroundColor Green
        } else {
            Write-Host ""
            Write-Host "To add manually, run:" -ForegroundColor DarkGray
            Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'User')" -ForegroundColor White
        }
    } else {
        Write-Host "[OK] You can now run 'ganesha' from anywhere!" -ForegroundColor Green
    }

    # Version check
    Write-Host ""
    Write-Host "Installed version:" -ForegroundColor DarkGray
    try {
        & "$InstallDir\$BinaryName" --version
    } catch {
        Write-Host "  (run 'ganesha --version' to verify)" -ForegroundColor DarkGray
    }

    # Optional: Browser automation
    Write-Host ""
    $NodeVersion = & node --version 2>$null
    if ($NodeVersion) {
        Write-Host "Node.js detected ($NodeVersion). For browser automation:" -ForegroundColor DarkGray
        Write-Host "  npx playwright install chromium" -ForegroundColor White
    } else {
        Write-Host "Optional: Install Node.js for browser automation features" -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "===========================================================" -ForegroundColor Cyan
    Write-Host "Documentation: $GitLabUrl" -ForegroundColor DarkGray
    Write-Host "===========================================================" -ForegroundColor Cyan
    Write-Host ""
}
finally {
    # Cleanup
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
