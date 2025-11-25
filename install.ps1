# ppdrive Windows Installer
# Downloads ppdrive-windows.zip and extracts it to:
#   %LOCALAPPDATA%\Programs\ppdrive

$ErrorActionPreference = "Stop"

Write-Host "🔧 Installing ppdrive for Windows..." -ForegroundColor Cyan

# Define install directory
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\ppdrive"

# Create directory if needed
if (-not (Test-Path $InstallDir)) {
    Write-Host "Creating $InstallDir ..."
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

# Temporary ZIP file path
$ZipPath = Join-Path $env:TEMP "ppdrive-windows.zip"

# Download ZIP from GitHub releases
$DownloadUrl = "https://github.com/dududaa/ppdrive/releases/download/v0.1.0-rc.1/ppdrive-windows.zip"

Write-Host "Downloading ppdrive package from:"
Write-Host "  $DownloadUrl"
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing

Write-Host "Extracting package..."

# Clear any old installation first
if (Test-Path $InstallDir) {
    Get-ChildItem -Path $InstallDir -Recurse | Remove-Item -Recurse -Force
}

# Extract ZIP into install directory
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force

# Remove temporary ZIP
Remove-Item $ZipPath -Force

Write-Host "ppdrive installed to: $InstallDir" -ForegroundColor Green

# Ensure install directory is in PATH
$CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")

if ($CurrentPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to PATH..."
    $NewPath = $CurrentPath + ";" + $InstallDir
    [Environment]::SetEnvironmentVariable("PATH", $Nehttps://github.com/dududaa/ppdrive/releases/download/v0.1.0-rc.1/ppdrive-windows.zipwPath, "User")
    Write-Host "PATH updated. Restart your terminal to apply changes."
} else {
    Write-Host "PATH already contains $InstallDir"
}

Write-Host "`n🎉 ppdrive installation complete!"
Write-Host "Try running: ppdrive --help" -ForegroundColor Yellow
