# 1. Configuration
$Repo = "dududaa/ppdrive"
$InstallDir = "C:\Program Files\ppdrive"
$LocalExe = Join-Path $InstallDir "ppdrive.exe"

# 2. Fetch Latest Version from GitHub API
Write-Host "Checking GitHub for the latest release..."
$UrlJson = "https://github.com{Repo}/releases/latest"
try {
    $ReleaseInfo = Invoke-RestMethod -Uri $UrlJson -UseBasicParsing
    $LatestTag = $ReleaseInfo.tag_name
    Write-Host "Latest remote version is: $LatestTag"
} catch {
    Write-Error "Failed to fetch version metadata from GitHub API."
    exit 1
}

# Normalize tag string
$LatestVersion = $LatestTag -replace '^v', ''

# 3. Check Local Installation Version
if (Test-Path $LocalExe) {
    # Runs your binary to capture its version string
    $LocalVersionRaw = & $LocalExe --version 2>$null
    if (-not $LocalVersionRaw) { $LocalVersionRaw = & $LocalExe -V 2>$null }

    # Extract structural version digits
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

# 4. Target Resolution
$Artifact = "windows-x86_64"
$FileName = "release-${Artifact}.zip"
$DownloadUrl = "https://github.com{Repo}/releases/download/${LatestTag}/${FileName}"

# 5. Establish System Install Path
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}
Write-Host "Installing to $InstallDir..."

# 6. Download and Extract
$TempZip = Join-Path $env:TEMP $FileName
$TempExtracted = Join-Path $env:TEMP "ppdrive_extracted"

if (Test-Path $TempExtracted) { Remove-Item -Recurse -Force $TempExtracted }

Write-Host "Downloading $DownloadUrl..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing

Write-Host "Extracting artifacts..."
Expand-Archive -Path $TempZip -DestinationPath $TempExtracted -Force

# Move files cleanly out of nested folder
Copy-Item -Path "$TempExtracted\release-${Artifact}\*" -Destination $InstallDir -Recurse -Force

# 7. Add to Windows Permanent Environment Variables
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")

if ($CurrentPath -notlike "*$InstallDir*") {
    $NewPath = "$CurrentPath;$InstallDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "Machine")
    Write-Host "PATH updated successfully."
}

# Clean up
Remove-Item -Force $TempZip
Remove-Item -Recurse -Force $TempExtracted
Write-Host "Update/Installation complete!" -ForegroundColor Green
