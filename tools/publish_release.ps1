# Reel Release Publisher
# Generates a signed release manifest for the auto-updater.
#
# Usage:
#   .\tools\publish_release.ps1 -Version 2.7.0 [-InstallerPath path] [-BaseUrl url] [-OutputDir dir]
#
# Example:
#   .\tools\publish_release.ps1 -Version 2.7.0 -BaseUrl "http://192.168.4.220:8080"
#
# This script:
#   1. Finds or accepts the installer path
#   2. Computes SHA-256 hash
#   3. Generates latest.json manifest
#   4. Outputs both files to the output directory

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,

    [string]$InstallerPath = "",

    [string]$BaseUrl = "",

    [string]$OutputDir = "release_output",

    [string]$ReleaseNotes = ""
)

$ErrorActionPreference = "Stop"

# Find installer
if ($InstallerPath -eq "") {
    $InstallerPath = "installer_output\Reel_${Version}_x64-setup.exe"
}

if (-not (Test-Path $InstallerPath)) {
    Write-Host "ERROR: Installer not found at: $InstallerPath" -ForegroundColor Red
    Write-Host "Build it first: flutter build windows --release && ISCC installer.iss" -ForegroundColor Yellow
    exit 1
}

# Create output dir
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# Compute SHA-256
Write-Host "Computing SHA-256..." -ForegroundColor Cyan
$hash = (Get-FileHash -Path $InstallerPath -Algorithm SHA256).Hash.ToLower()
$fileSize = (Get-Item $InstallerPath).Length
Write-Host "  SHA-256: $hash" -ForegroundColor Green
Write-Host "  Size:    $fileSize bytes ($([math]::Round($fileSize / 1MB, 1)) MB)" -ForegroundColor Green

# Build download URL
if ($BaseUrl -eq "") {
    $downloadUrl = "Reel_${Version}_x64-setup.exe"
    Write-Host ""
    Write-Host "WARNING: No -BaseUrl provided. download_url is a relative filename." -ForegroundColor Yellow
    Write-Host "  Set -BaseUrl to your hosting URL (e.g., http://192.168.4.220:8080)" -ForegroundColor Yellow
} else {
    $BaseUrl = $BaseUrl.TrimEnd("/")
    $downloadUrl = "$BaseUrl/Reel_${Version}_x64-setup.exe"
}

# Generate manifest
$manifest = @{
    version = $Version
    download_url = $downloadUrl
    sha256 = $hash
    file_size = $fileSize
    release_notes = if ($ReleaseNotes -ne "") { $ReleaseNotes } else { "Reel v$Version" }
} | ConvertTo-Json -Depth 1

$manifestPath = Join-Path $OutputDir "latest.json"
$manifest | Out-File -FilePath $manifestPath -Encoding utf8

# Copy installer to output
$installerDest = Join-Path $OutputDir "Reel_${Version}_x64-setup.exe"
Copy-Item -Path $InstallerPath -Destination $installerDest -Force

Write-Host ""
Write-Host "Release ready:" -ForegroundColor Green
Write-Host "  Manifest:  $manifestPath" -ForegroundColor White
Write-Host "  Installer: $installerDest" -ForegroundColor White
Write-Host ""
Write-Host "To distribute:" -ForegroundColor Cyan
Write-Host "  1. Host both files on any HTTP server" -ForegroundColor White
Write-Host "  2. Set the manifest URL in Reel Settings > About > Update URL" -ForegroundColor White
Write-Host "     Point it to: <your-server>/latest.json" -ForegroundColor White
Write-Host ""
Write-Host "Quick LAN hosting (run from $OutputDir):" -ForegroundColor Cyan
Write-Host "  python -m http.server 8080" -ForegroundColor White
