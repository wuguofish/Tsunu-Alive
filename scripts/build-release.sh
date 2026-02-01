#!/bin/bash
# build-release.sh - Tsunu Alive 完整建置腳本（macOS / Linux）
#
# 打包 Tauri App + VS Code Extension + Claude Code Skill
# 輸出：src-tauri/target/release/bundle/ 中的安裝檔

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DEBUG_MODE=false

# 解析參數
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --debug) DEBUG_MODE=true ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

echo "=== Tsunu Alive Build Release ==="
echo "Project root: $PROJECT_ROOT"

# 1. 確認必要工具
echo ""
echo "[1/5] Checking prerequisites..."
for tool in node npm cargo; do
    if ! command -v "$tool" &> /dev/null; then
        echo "ERROR: $tool is not installed or not in PATH"
        exit 1
    fi
done
echo "  -> All prerequisites found"

# 2. 打包 VS Code Extension
echo ""
echo "[2/5] Packaging VS Code Extension..."
cd "$PROJECT_ROOT/vscode-extension"
npm install --silent 2>/dev/null
npx @vscode/vsce package --allow-missing-repository --out "tsunu-alive-connector.vsix"
if [ ! -f "tsunu-alive-connector.vsix" ]; then
    echo "ERROR: Failed to package VS Code extension"
    exit 1
fi
echo "  -> tsunu-alive-connector.vsix created"

# 3. 準備 bundled resources
echo ""
echo "[3/5] Preparing bundled resources..."
BUNDLED_DIR="$PROJECT_ROOT/src-tauri/resources/bundled"

rm -rf "$BUNDLED_DIR"
mkdir -p "$BUNDLED_DIR"

# 複製 .vsix
cp "$PROJECT_ROOT/vscode-extension/tsunu-alive-connector.vsix" "$BUNDLED_DIR/"
echo "  -> Copied .vsix to bundled/"

# 複製 Skill 檔案
SKILL_DEST="$BUNDLED_DIR/skill"
mkdir -p "$SKILL_DEST"
cp "$PROJECT_ROOT/.claude/skills/uni/SKILL.md" "$SKILL_DEST/"
cp "$PROJECT_ROOT/.claude/skills/uni/uni-full-setting.md" "$SKILL_DEST/"

if [ -d "$PROJECT_ROOT/.claude/skills/uni/scripts" ]; then
    mkdir -p "$SKILL_DEST/scripts"
    cp "$PROJECT_ROOT/.claude/skills/uni/scripts/"* "$SKILL_DEST/scripts/" 2>/dev/null || true
fi

# 複製 hooks
if [ -d "$PROJECT_ROOT/.claude/hooks" ]; then
    mkdir -p "$BUNDLED_DIR/hooks"
    cp "$PROJECT_ROOT/.claude/hooks/"* "$BUNDLED_DIR/hooks/" 2>/dev/null || true
fi

echo "  -> Copied skill files to bundled/skill/"

# 4. 安裝前端依賴
echo ""
echo "[4/5] Installing dependencies..."
cd "$PROJECT_ROOT"
npm install --silent 2>/dev/null

# 5. 建置 Tauri App
echo ""
echo "[5/5] Building Tauri application..."
cd "$PROJECT_ROOT"
if [ "$DEBUG_MODE" = true ]; then
    npx tauri build --debug
else
    npx tauri build
fi

# 輸出結果
echo ""
echo "=== Build Complete ==="
BUNDLE_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle"
if [ "$DEBUG_MODE" = true ]; then
    BUNDLE_DIR="$PROJECT_ROOT/src-tauri/target/debug/bundle"
fi

if [ -d "$BUNDLE_DIR" ]; then
    echo "Output files:"
    find "$BUNDLE_DIR" -type f \( -name "*.dmg" -o -name "*.app" -o -name "*.AppImage" -o -name "*.deb" \) | while read -r f; do
        echo "  -> $f"
    done
fi

# 清理 vsix 暫存檔
rm -f "$PROJECT_ROOT/vscode-extension/tsunu-alive-connector.vsix"

echo ""
echo "Done!"
