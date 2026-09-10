# PPDRIVE Installer for Windows
# Usage: powershell -ExecutionPolicy Bypass -File install.ps1

# 1. Configuration
$Repo = "dududaa/ppdrive"
$InstallDir = Join-Path $env:LOCALAPPDATA "ppdrive"
$LocalExe = Join-Path $InstallDir "ppdrive.exe"

# 2. Detect architecture
$Arch = if ([System.Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x86_64" }
} else {
    Write-Error "32-bit Windows is not supported."
    exit 1
}
$Artifact = if ($Arch -eq "arm64") { "ppdrive-windows-arm64.tar.gz" } else { "ppdrive-windows.tar.gz" }

# 3. Fetch Latest Version from GitHub API
Write-Host "Checking GitHub for the latest release..."
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
try {
    $ReleaseInfo = Invoke-RestMethod -Uri $ApiUrl -UseBasicParsing
    $LatestTag = $ReleaseInfo.tag_name
    Write-Host "Latest remote version is: $LatestTag"
} catch {
    Write-Error "Failed to fetch version metadata from GitHub API: $_"
    exit 1
}

# Normalize tag string
$LatestVersion = $LatestTag -replace '^v', ''

# 4. Check Local Installation Version
if (Test-Path $LocalExe) {
    $LocalVersionRaw = & $LocalExe --version 2>$null
    if (-not $LocalVersionRaw) { $LocalVersionRaw = & $LocalExe -V 2>$null }

    if ($LocalVersionRaw -match '(\d+\.\d+\.\d+)') {
        $LocalVersion = $Matches[1]
    } else {
        $LocalVersion = "0.0.0"
    }

    Write-Host "Current local version is: $LocalVersion"

    if ($LocalVersion -eq $LatestVersion) {
        Write-Host "Success: ppdrive is already up to date ($LocalVersion)." -ForegroundColor Green
        exit 0
    }
    Write-Host "New version detected ($LatestVersion). Proceeding with upgrade..." -ForegroundColor Yellow
}

# 5. Download URL
$DownloadUrl = "https://github.com/$Repo/releases/download/$LatestTag/$Artifact"

# 6. Prepare install directory
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}
Write-Host "Installing to $InstallDir..."

# 7. Download and Extract
$TempDir = Join-Path $env:TEMP "ppdrive_install_$(Get-Random)"
$TempArchive = Join-Path $TempDir $Artifact

try {
    New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

    Write-Host "Downloading $DownloadUrl..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempArchive -UseBasicParsing -ErrorAction Stop

    Write-Host "Extracting artifacts..."
    tar -xzf $TempArchive -C $TempDir
    if ($LASTEXITCODE -ne 0) {
        throw "tar extraction failed (exit code $LASTEXITCODE). Ensure tar is available."
    }

    # Copy binaries to install dir
    $Binaries = @("ppdrive.exe", "server.exe")
    foreach ($Bin in $Binaries) {
        $Src = Join-Path $TempDir $Bin
        if (Test-Path $Src) {
            Copy-Item -Path $Src -Destination $InstallDir -Force
            Write-Host "  Installed $Bin"
        } else {
            # Check nested directory
            $Found = Get-ChildItem -Path $TempDir -Recurse -Filter $Bin | Select-Object -First 1
            if ($Found) {
                Copy-Item -Path $Found.FullName -Destination $InstallDir -Force
                Write-Host "  Installed $Bin"
            } else {
                Write-Warning "Binary '$Bin' not found in archive."
            }
        }
    }
} catch {
    Write-Error "Installation failed: $_"
    exit 1
} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Recurse -Force $TempDir
    }
}

# 8. Add to User PATH (no admin required)
$CurrentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($CurrentUserPath -notlike "*$InstallDir*") {
    $NewPath = if ($CurrentUserPath) { "$CurrentUserPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "✅ Added $InstallDir to your user PATH."
    Write-Host "   Restart your terminal for changes to take effect." -ForegroundColor Yellow
} else {
    Write-Host "✅ $InstallDir is already in your PATH."
}

Write-Host "Update/Installation complete!" -ForegroundColor Green
