# build-release.ps1 - Tsunu Alive full build script (Windows)
#
# Packages Tauri App + VS Code Extension + Claude Code Skill
# Output: installer in src-tauri/target/release/bundle/

param(
    [switch]$SkipFrontend,  # Skip frontend build
    [switch]$Debug          # Debug mode build
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== Tsunu Alive Build Release ===" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"

# 1. Check prerequisites
Write-Host "`n[1/5] Checking prerequisites..." -ForegroundColor Yellow

$requiredTools = @("node", "npm", "cargo")
foreach ($tool in $requiredTools) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: $tool is not installed or not in PATH" -ForegroundColor Red
        exit 1
    }
}

# 2. Package VS Code Extension
Write-Host "`n[2/5] Packaging VS Code Extension..." -ForegroundColor Yellow
$vsceDir = Join-Path $ProjectRoot "vscode-extension"

Push-Location $vsceDir
try {
    npm install --silent 2>$null
    npx @vscode/vsce package --allow-missing-repository --out "tsunu-alive-connector.vsix" 2>&1
    if (-not (Test-Path "tsunu-alive-connector.vsix")) {
        Write-Host "ERROR: Failed to package VS Code extension" -ForegroundColor Red
        exit 1
    }
    Write-Host "  -> tsunu-alive-connector.vsix created" -ForegroundColor Green
} finally {
    Pop-Location
}

# 3. Prepare bundled resources
Write-Host "`n[3/5] Preparing bundled resources..." -ForegroundColor Yellow
$bundledDir = Join-Path (Join-Path (Join-Path $ProjectRoot "src-tauri") "resources") "bundled"

# Clean old bundled directory
if (Test-Path $bundledDir) {
    Remove-Item -Path $bundledDir -Recurse -Force
}
New-Item -ItemType Directory -Path $bundledDir -Force | Out-Null

# Copy .vsix
$vsixSource = Join-Path $vsceDir "tsunu-alive-connector.vsix"
Copy-Item $vsixSource -Destination (Join-Path $bundledDir "tsunu-alive-connector.vsix")
Write-Host "  -> Copied .vsix to bundled/" -ForegroundColor Green

# Copy Skill files
$skillSource = Join-Path (Join-Path (Join-Path $ProjectRoot ".claude") "skills") "uni"
$skillDest = Join-Path $bundledDir "skill"
New-Item -ItemType Directory -Path $skillDest -Force | Out-Null

# Copy SKILL.md and uni-full-setting.md
Copy-Item (Join-Path $skillSource "SKILL.md") -Destination $skillDest
Copy-Item (Join-Path $skillSource "uni-full-setting.md") -Destination $skillDest

# Copy scripts subdirectory
$scriptsSource = Join-Path $skillSource "scripts"
if (Test-Path $scriptsSource) {
    $scriptsDest = Join-Path $skillDest "scripts"
    New-Item -ItemType Directory -Path $scriptsDest -Force | Out-Null
    Copy-Item (Join-Path $scriptsSource "*") -Destination $scriptsDest -Recurse
}

# Copy hooks
$hooksSource = Join-Path (Join-Path $ProjectRoot ".claude") "hooks"
if (Test-Path $hooksSource) {
    $hooksDest = Join-Path $bundledDir "hooks"
    New-Item -ItemType Directory -Path $hooksDest -Force | Out-Null
    Copy-Item (Join-Path $hooksSource "*") -Destination $hooksDest -Recurse
}

Write-Host "  -> Copied skill files to bundled/skill/" -ForegroundColor Green

# 4. Install frontend dependencies
Write-Host "`n[4/5] Installing dependencies..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    npm install --silent 2>$null
} finally {
    Pop-Location
}

# 5. Build Tauri App
Write-Host "`n[5/5] Building Tauri application..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    if ($Debug) {
        npx tauri build --debug
    } else {
        npx tauri build
    }
} finally {
    Pop-Location
}

# Output results
Write-Host "`n=== Build Complete ===" -ForegroundColor Green
$bundleDir = Join-Path (Join-Path (Join-Path (Join-Path $ProjectRoot "src-tauri") "target") "release") "bundle"
if ($Debug) {
    $bundleDir = Join-Path (Join-Path (Join-Path (Join-Path $ProjectRoot "src-tauri") "target") "debug") "bundle"
}

if (Test-Path $bundleDir) {
    Write-Host "Output files:" -ForegroundColor Cyan
    Get-ChildItem -Path $bundleDir -Recurse -File | Where-Object {
        $_.Extension -in ".exe", ".msi", ".dmg", ".app", ".AppImage"
    } | ForEach-Object {
        Write-Host "  -> $($_.FullName)" -ForegroundColor Green
    }
}

# Clean up temp vsix
Remove-Item (Join-Path $vsceDir "tsunu-alive-connector.vsix") -ErrorAction SilentlyContinue

Write-Host "`nDone!" -ForegroundColor Cyan
