$ErrorActionPreference = "Stop"

$CccHome = "$env:USERPROFILE\.ccc"
$Repo = "wangnguen/claude-code-config"
$AssetName = "ccc-x86_64-pc-windows-msvc.zip"

# A plain release download rather than api.github.com: the API allows 60
# unauthenticated requests an hour per IP, which an office sharing one
# address runs out of.
$DownloadUrl = "https://github.com/$Repo/releases/latest/download/$AssetName"

# Download zip
Write-Host "Downloading $AssetName..." -ForegroundColor Cyan
$TmpZip = "$env:TEMP\ccc.zip"
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TmpZip -UseBasicParsing

# Extract to temp, then copy contents to CccHome
$TmpDir = "$env:TEMP\ccc-extract"
if (Test-Path $TmpDir) { Remove-Item $TmpDir -Recurse -Force }
Expand-Archive -Path $TmpZip -DestinationPath $TmpDir -Force

# Rename old exe if running (Windows lock workaround)
$OldExe = "$CccHome\ccc.old.exe"
$ExePath = "$CccHome\ccc.exe"
if (Test-Path $ExePath) {
    if (Test-Path $OldExe) { Remove-Item $OldExe -Force }
    Rename-Item $ExePath $OldExe -Force -ErrorAction SilentlyContinue
}

# Copy from extracted folder to CccHome
$ExtractedDir = Get-ChildItem $TmpDir | Select-Object -First 1
if (-not (Test-Path $CccHome)) {
    New-Item -ItemType Directory -Path $CccHome | Out-Null
}
Copy-Item -Path "$($ExtractedDir.FullName)\*" -Destination $CccHome -Recurse -Force

# Cleanup
Remove-Item $TmpZip
Remove-Item $TmpDir -Recurse -Force

# Add to PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -and $UserPath.Split(";") -contains $CccHome) {
    Write-Host "$CccHome is already in PATH." -ForegroundColor Yellow
} else {
    if ($UserPath) {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$CccHome", "User")
    } else {
        [Environment]::SetEnvironmentVariable("Path", $CccHome, "User")
    }
    Write-Host "Added $CccHome to PATH. Restart your terminal for changes to take effect." -ForegroundColor Green
}

Write-Host ""
Write-Host "Done! ccc installed to $CccHome" -ForegroundColor Green
Write-Host "  - $CccHome\ccc.exe"
Write-Host "  - $CccHome\.claude\"
Write-Host ""
Write-Host "Restart your terminal, then run: ccc key <your-api-key>" -ForegroundColor Cyan
