# build-release.ps1 - Tsunu Alive 完整建置腳本（Windows）
#
# 打包 Tauri App + VS Code Extension + Claude Code Skill
# 輸出：src-tauri/target/release/bundle/ 中的安裝檔

param(
    [switch]$SkipFrontend,  # 跳過前端建置（已建置過時）
    [switch]$Debug          # Debug 模式建置
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== Tsunu Alive Build Release ===" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"

# 1. 確認必要工具
Write-Host "`n[1/5] Checking prerequisites..." -ForegroundColor Yellow

$requiredTools = @("node", "npm", "cargo")
foreach ($tool in $requiredTools) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: $tool is not installed or not in PATH" -ForegroundColor Red
        exit 1
    }
}

# 2. 打包 VS Code Extension
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

# 3. 準備 bundled resources
Write-Host "`n[3/5] Preparing bundled resources..." -ForegroundColor Yellow
$bundledDir = Join-Path $ProjectRoot "src-tauri" "resources" "bundled"

# 清除舊的 bundled 目錄
if (Test-Path $bundledDir) {
    Remove-Item -Path $bundledDir -Recurse -Force
}
New-Item -ItemType Directory -Path $bundledDir -Force | Out-Null

# 複製 .vsix
$vsixSource = Join-Path $vsceDir "tsunu-alive-connector.vsix"
Copy-Item $vsixSource -Destination (Join-Path $bundledDir "tsunu-alive-connector.vsix")
Write-Host "  -> Copied .vsix to bundled/" -ForegroundColor Green

# 複製 Skill 檔案
$skillSource = Join-Path $ProjectRoot ".claude" "skills" "uni"
$skillDest = Join-Path $bundledDir "skill"
New-Item -ItemType Directory -Path $skillDest -Force | Out-Null

# 複製 SKILL.md 和 uni-full-setting.md
Copy-Item (Join-Path $skillSource "SKILL.md") -Destination $skillDest
Copy-Item (Join-Path $skillSource "uni-full-setting.md") -Destination $skillDest

# 複製 scripts 子目錄
$scriptsSource = Join-Path $skillSource "scripts"
if (Test-Path $scriptsSource) {
    $scriptsDest = Join-Path $skillDest "scripts"
    New-Item -ItemType Directory -Path $scriptsDest -Force | Out-Null
    Copy-Item (Join-Path $scriptsSource "*") -Destination $scriptsDest -Recurse
}

# 複製 hooks
$hooksSource = Join-Path $ProjectRoot ".claude" "hooks"
if (Test-Path $hooksSource) {
    $hooksDest = Join-Path $bundledDir "hooks"
    New-Item -ItemType Directory -Path $hooksDest -Force | Out-Null
    Copy-Item (Join-Path $hooksSource "*") -Destination $hooksDest -Recurse
}

Write-Host "  -> Copied skill files to bundled/skill/" -ForegroundColor Green

# 4. 安裝前端依賴
Write-Host "`n[4/5] Installing dependencies..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    npm install --silent 2>$null
} finally {
    Pop-Location
}

# 5. 建置 Tauri App
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

# 輸出結果
Write-Host "`n=== Build Complete ===" -ForegroundColor Green
$bundleDir = Join-Path $ProjectRoot "src-tauri" "target" "release" "bundle"
if ($Debug) {
    $bundleDir = Join-Path $ProjectRoot "src-tauri" "target" "debug" "bundle"
}

if (Test-Path $bundleDir) {
    Write-Host "Output files:" -ForegroundColor Cyan
    Get-ChildItem -Path $bundleDir -Recurse -File | Where-Object {
        $_.Extension -in ".exe", ".msi", ".dmg", ".app", ".AppImage"
    } | ForEach-Object {
        Write-Host "  -> $($_.FullName)" -ForegroundColor Green
    }
}

# 清理 vsix 暫存檔
Remove-Item (Join-Path $vsceDir "tsunu-alive-connector.vsix") -ErrorAction SilentlyContinue

Write-Host "`nDone!" -ForegroundColor Cyan
