# Build script for Lamzu Stream Deck Plugin
# Usage: .\build-plugin.ps1 [-Release]

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

Write-Host "Building Lamzu Stream Deck Plugin..." -ForegroundColor Cyan

# Build the binary
$buildArgs = @("build", "--features", "streamdeck", "--bin", "lamzu-streamdeck")
if ($Release) {
    $buildArgs += "--release"
    $targetDir = "target/release"
} else {
    $targetDir = "target/debug"
}

Write-Host "Running: cargo $($buildArgs -join ' ')" -ForegroundColor Yellow
cargo @buildArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# Create plugin directory
$pluginDir = "io.github.denzonl.lamzuctl.sdPlugin"
Write-Host "Creating plugin directory: $pluginDir" -ForegroundColor Yellow

if (Test-Path $pluginDir) {
    Remove-Item -Recurse -Force $pluginDir
}

New-Item -ItemType Directory -Force -Path $pluginDir | Out-Null
New-Item -ItemType Directory -Force -Path "$pluginDir/bin" | Out-Null
New-Item -ItemType Directory -Force -Path "$pluginDir/icons" | Out-Null
New-Item -ItemType Directory -Force -Path "$pluginDir/property-inspector" | Out-Null

# Copy binary
Write-Host "Copying binary..." -ForegroundColor Yellow
Copy-Item "$targetDir/lamzu-streamdeck.exe" "$pluginDir/bin/"

# Copy manifest
Write-Host "Copying manifest and resources..." -ForegroundColor Yellow
Copy-Item "streamdeck/manifest.json" "$pluginDir/"

# Copy icons (if they exist)
if (Test-Path "streamdeck/icons/*") {
    Copy-Item "streamdeck/icons/*" "$pluginDir/icons/"
}

# Copy property inspector
Copy-Item "streamdeck/property-inspector/*" "$pluginDir/property-inspector/"

Write-Host ""
Write-Host "Plugin built successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "To install:" -ForegroundColor Cyan
Write-Host "  1. Close Stream Deck application"
Write-Host "  2. Copy '$pluginDir' to:"
Write-Host "     %APPDATA%\Elgato\StreamDeck\Plugins\"
Write-Host "  3. Start Stream Deck application"
Write-Host ""

# Optionally create .streamDeckPlugin package
$zipName = "io.github.denzonl.lamzuctl.zip"
$packageName = "io.github.denzonl.lamzuctl.streamDeckPlugin"

if (Test-Path $zipName) { Remove-Item $zipName }
if (Test-Path $packageName) { Remove-Item $packageName }

Write-Host "Creating .streamDeckPlugin package..." -ForegroundColor Yellow
Compress-Archive -Path $pluginDir -DestinationPath $zipName -Force
Rename-Item $zipName $packageName

Write-Host ""
Write-Host "Package created: $packageName" -ForegroundColor Green
Write-Host "Double-click to install via Stream Deck installer."
