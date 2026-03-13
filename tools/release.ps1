# Reel Release Pipeline
# One command: bump version → build → package → deploy → serve.
#
# Usage:
#   .\tools\release.ps1 -Version 2.7.0
#   .\tools\release.ps1 -Version 2.7.0 -Notes "Fixed subtitle timing"
#   .\tools\release.ps1 -Version 2.7.0 -SkipBuild   # manifest + deploy only (installer already built)
#
# What it does:
#   1. Updates version in pubspec.yaml + installer.iss
#   2. Builds Flutter release (flutter build windows --release)
#   3. Packages installer (Inno Setup ISCC)
#   4. Generates SHA-256 manifest (latest.json)
#   5. Deploys installer + manifest to E:\reel-updates\
#   6. Starts update server on :8080 if not already running
#
# Ryan's Reel auto-updater polls: http://192.168.4.220:8080/latest.json

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,

    [string]$Notes = "",

    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$ReelRoot = "E:\reel"
$UpdatesDir = "E:\reel-updates"
$ServerPort = 8080
$BaseUrl = "http://192.168.4.220:$ServerPort"
$Flutter = "C:\flutter\bin\flutter.bat"
$ISCC = "C:\Users\OPTIPLEX-JELLY\AppData\Local\Programs\Inno Setup 6\ISCC.exe"

# --- Validate version format ---
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Host "ERROR: Version must be semver (e.g., 2.7.0), got: $Version" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== Reel Release Pipeline ===" -ForegroundColor Cyan
Write-Host "  Version:  $Version" -ForegroundColor White
Write-Host "  Server:   $BaseUrl" -ForegroundColor White
Write-Host ""

# --- Step 1: Bump version ---
Write-Host "[1/6] Bumping version to $Version..." -ForegroundColor Yellow

$pubspec = Join-Path $ReelRoot "pubspec.yaml"
$pubspecContent = Get-Content $pubspec -Raw
$pubspecOriginal = $pubspecContent
$pubspecContent = $pubspecContent -replace 'version:\s*\d+\.\d+\.\d+\+\d+', "version: $Version+1"
if ($pubspecContent -eq $pubspecOriginal) {
    Write-Host "ERROR: Failed to find version pattern in pubspec.yaml" -ForegroundColor Red
    exit 1
}
[System.IO.File]::WriteAllText($pubspec, $pubspecContent)

$iss = Join-Path $ReelRoot "installer.iss"
$issContent = Get-Content $iss -Raw
$issOriginal = $issContent
$issContent = $issContent -replace 'AppVersion=\d+\.\d+\.\d+', "AppVersion=$Version"
$issContent = $issContent -replace 'OutputBaseFilename=Reel_\d+\.\d+\.\d+_x64-setup', "OutputBaseFilename=Reel_${Version}_x64-setup"
if ($issContent -eq $issOriginal) {
    Write-Host "ERROR: Failed to update version in installer.iss" -ForegroundColor Red
    exit 1
}
[System.IO.File]::WriteAllText($iss, $issContent)

Write-Host "  pubspec.yaml: $Version+1" -ForegroundColor Green
Write-Host "  installer.iss: $Version" -ForegroundColor Green

if (-not $SkipBuild) {
    # --- Step 2: Flutter build ---
    Write-Host ""
    Write-Host "[2/6] Building Flutter release..." -ForegroundColor Yellow
    Push-Location $ReelRoot
    try {
        & $Flutter build windows --release
        if ($LASTEXITCODE -ne 0) { throw "Flutter build failed" }
        Write-Host "  Build complete." -ForegroundColor Green
    } finally {
        Pop-Location
    }

    # --- Step 3: Inno Setup ---
    Write-Host ""
    Write-Host "[3/6] Packaging installer..." -ForegroundColor Yellow
    Push-Location $ReelRoot
    try {
        & $ISCC installer.iss
        if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }
        Write-Host "  Installer packaged." -ForegroundColor Green
    } finally {
        Pop-Location
    }
} else {
    Write-Host ""
    Write-Host "[2/6] Skipping Flutter build (-SkipBuild)" -ForegroundColor DarkGray
    Write-Host "[3/6] Skipping installer packaging (-SkipBuild)" -ForegroundColor DarkGray
}

# --- Step 4: Generate manifest ---
Write-Host ""
Write-Host "[4/6] Generating release manifest..." -ForegroundColor Yellow

$installerName = "Reel_${Version}_x64-setup.exe"
$installerPath = Join-Path $ReelRoot "installer_output\$installerName"

if (-not (Test-Path $installerPath)) {
    Write-Host "ERROR: Installer not found: $installerPath" -ForegroundColor Red
    Write-Host "  Run without -SkipBuild to build it first." -ForegroundColor Yellow
    exit 1
}

$hash = (Get-FileHash -Path $installerPath -Algorithm SHA256).Hash.ToLower()
$fileSize = (Get-Item $installerPath).Length
$downloadUrl = "$BaseUrl/$installerName"

if ($Notes -eq "") { $Notes = "Reel v$Version" }

$manifest = @{
    version = $Version
    download_url = $downloadUrl
    sha256 = $hash
    file_size = $fileSize
    release_notes = $Notes
} | ConvertTo-Json -Depth 1

Write-Host "  SHA-256:  $hash" -ForegroundColor Green
Write-Host "  Size:     $([math]::Round($fileSize / 1MB, 1)) MB" -ForegroundColor Green

# --- Step 5: Deploy to updates directory ---
Write-Host ""
Write-Host "[5/6] Deploying to $UpdatesDir..." -ForegroundColor Yellow

New-Item -ItemType Directory -Force -Path $UpdatesDir | Out-Null

# Deploy installer BEFORE manifest — if a client polls between these two operations,
# the old manifest still points to the old version (safe). Reverse order would 404.
Copy-Item -Path $installerPath -Destination (Join-Path $UpdatesDir $installerName) -Force
Write-Host "  $installerName deployed" -ForegroundColor Green

$manifestDest = Join-Path $UpdatesDir "latest.json"
# WriteAllText avoids UTF-8 BOM (PowerShell 5.1's -Encoding utf8 adds BOM, breaks JSON parsers)
[System.IO.File]::WriteAllText($manifestDest, $manifest)
Write-Host "  latest.json:  deployed" -ForegroundColor Green

# --- Step 6: Ensure update server is running ---
Write-Host ""
Write-Host "[6/6] Checking update server..." -ForegroundColor Yellow

$portInUse = Get-NetTCPConnection -LocalPort $ServerPort -State Listen -ErrorAction SilentlyContinue
if ($portInUse) {
    $ownerPid = $portInUse[0].OwningProcess
    $ownerName = (Get-Process -Id $ownerPid -ErrorAction SilentlyContinue).ProcessName
    if ($ownerName -eq 'python') {
        Write-Host "  Update server already running on :$ServerPort (PID $ownerPid)" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: Port $ServerPort in use by '$ownerName' (PID $ownerPid), not Python" -ForegroundColor Yellow
        Write-Host "  Kill it and re-run, or start the server manually." -ForegroundColor Yellow
    }
} else {
    $pythonPath = Get-Command python -ErrorAction SilentlyContinue
    if (-not $pythonPath) {
        Write-Host "  WARNING: Python not found on PATH. Start the server manually:" -ForegroundColor Yellow
        Write-Host "    cd $UpdatesDir && python -m http.server $ServerPort" -ForegroundColor White
    } else {
        Write-Host "  Starting update server on :$ServerPort..." -ForegroundColor White
        Start-Process -FilePath $pythonPath.Source -ArgumentList "-m http.server $ServerPort --bind 192.168.4.220" -WorkingDirectory $UpdatesDir -WindowStyle Hidden
        Start-Sleep -Seconds 1
        $check = Get-NetTCPConnection -LocalPort $ServerPort -State Listen -ErrorAction SilentlyContinue
        if ($check) {
            Write-Host "  Update server started on :$ServerPort (LAN only)" -ForegroundColor Green
        } else {
            Write-Host "  WARNING: Server may not have started. Run manually:" -ForegroundColor Yellow
            Write-Host "    cd $UpdatesDir && python -m http.server $ServerPort --bind 192.168.4.220" -ForegroundColor White
        }
    }
}

# --- Done ---
Write-Host ""
Write-Host "=== Release $Version published ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Manifest URL:  $BaseUrl/latest.json" -ForegroundColor White
Write-Host "  Installer URL: $BaseUrl/$installerName" -ForegroundColor White
Write-Host ""
Write-Host "  Ryan's Reel will auto-update within 30 minutes," -ForegroundColor Green
Write-Host "  or he can check manually in Settings > Updates." -ForegroundColor Green
Write-Host ""
