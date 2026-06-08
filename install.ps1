# 1. Configuration (Requires Administrator privileges to write to Program Files)
$Repo = "dududaa/ppdrive"
$Tag = "v1.0.0-alpha"

# 2. Target Resolution
$Artifact = "windows-x86_64"
$FileName = "release-${Artifact}.zip"
$Url = "https://github.com{Repo}/releases/download/${Tag}/${FileName}"

# 3. Establish System Install Path
$InstallDir = "C:\Program Files\ppdrive"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}
Write-Host "Installing to $InstallDir..."

# 4. Download and Extract
$TempZip = Join-Path $env:TEMP $FileName
$TempExtracted = Join-Path $env:TEMP "ppdrive_extracted"

if (Test-Path $TempExtracted) { Remove-Item -Recurse -Force $TempExtracted }

Write-Host "Downloading $Url..."
Invoke-WebRequest -Uri $Url -OutFile $TempZip -UseBasicParsing

Write-Host "Extracting artifacts..."
Expand-Archive -Path $TempZip -DestinationPath $TempExtracted -Force

# Move files cleanly out of nested folder
Copy-Item -Path "$TempExtracted\release-${Artifact}\*" -Destination $InstallDir -Recurse -Force

# 5. Add to Windows Permanent Environment Variables
Write-Host "Updating system Environment PATH variable..."
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")

if ($CurrentPath -notlike "*$InstallDir*") {
    $NewPath = "$CurrentPath;$InstallDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "Machine")
    Write-Host "PATH updated successfully."
} else {
    Write-Host "PATH already contains the installation directory."
}

# Clean up
Remove-Item -Force $TempZip
Remove-Item -Recurse -Force $TempExtracted
Write-Host "Success! Restart your terminal or applications to apply changes."
