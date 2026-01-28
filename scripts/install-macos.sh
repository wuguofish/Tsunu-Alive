#!/bin/bash
# ============================================================================
# Tsunu Alive - macOS 安裝腳本
# ============================================================================
# 這個腳本會：
# 1. 檢查並安裝必要的依賴（Rust, Node.js）
# 2. 編譯 Tsunu Alive
# 3. 安裝 VS Code Extension
# ============================================================================

set -e  # 遇到錯誤就停止

# 顏色定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 輸出函數
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# 取得腳本所在目錄（專案根目錄）
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo ""
echo "============================================"
echo "   🎮 Tsunu Alive 安裝程式 (macOS)"
echo "   阿宇陪你寫程式"
echo "============================================"
echo ""

# ============================================================================
# 1. 檢查系統需求
# ============================================================================
info "檢查系統需求..."

# 檢查 macOS
if [[ "$(uname)" != "Darwin" ]]; then
    error "這個腳本只支援 macOS"
fi

# 檢查 Xcode Command Line Tools
if ! xcode-select -p &>/dev/null; then
    warn "需要安裝 Xcode Command Line Tools"
    info "正在安裝..."
    xcode-select --install
    echo "請在安裝完成後重新執行此腳本"
    exit 0
fi
success "Xcode Command Line Tools 已安裝"

# ============================================================================
# 2. 檢查並安裝 Homebrew
# ============================================================================
if ! command -v brew &>/dev/null; then
    warn "需要安裝 Homebrew"
    info "正在安裝 Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # 設定 PATH（Apple Silicon vs Intel）
    if [[ -f "/opt/homebrew/bin/brew" ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    else
        eval "$(/usr/local/bin/brew shellenv)"
    fi
fi
success "Homebrew 已安裝"

# ============================================================================
# 3. 檢查並安裝 Rust
# ============================================================================
if ! command -v rustc &>/dev/null; then
    warn "需要安裝 Rust"
    info "正在安裝 Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
success "Rust 已安裝 ($(rustc --version))"

# ============================================================================
# 4. 檢查並安裝 Node.js
# ============================================================================
if ! command -v node &>/dev/null; then
    warn "需要安裝 Node.js"
    info "正在透過 Homebrew 安裝 Node.js..."
    brew install node
fi
success "Node.js 已安裝 ($(node --version))"

# ============================================================================
# 5. 安裝 Tauri CLI
# ============================================================================
info "安裝 Tauri CLI..."
cargo install tauri-cli 2>/dev/null || true
success "Tauri CLI 已準備"

# ============================================================================
# 6. 安裝專案依賴
# ============================================================================
info "安裝前端依賴..."
cd "$PROJECT_DIR"
npm install

info "安裝 VS Code Extension 依賴..."
cd "$PROJECT_DIR/vscode-extension"
npm install
cd "$PROJECT_DIR"

success "依賴安裝完成"

# ============================================================================
# 7. 編譯 Tsunu Alive
# ============================================================================
info "開始編譯 Tsunu Alive（這可能需要幾分鐘）..."
cd "$PROJECT_DIR"
npm run tauri build

success "編譯完成！"

# ============================================================================
# 8. 安裝應用程式
# ============================================================================
info "安裝應用程式..."

# 找到編譯好的 .app
APP_PATH="$PROJECT_DIR/src-tauri/target/release/bundle/macos/Tsunu Alive.app"
DMG_PATH="$PROJECT_DIR/src-tauri/target/release/bundle/dmg"

if [[ -d "$APP_PATH" ]]; then
    # 複製到 Applications
    cp -R "$APP_PATH" "/Applications/"
    success "已安裝到 /Applications/Tsunu Alive.app"
elif [[ -d "$DMG_PATH" ]]; then
    info "請手動開啟 DMG 檔案安裝："
    open "$DMG_PATH"
else
    warn "找不到編譯好的應用程式，請檢查編譯輸出"
fi

# ============================================================================
# 9. 安裝 VS Code Extension
# ============================================================================
info "編譯並安裝 VS Code Extension..."
cd "$PROJECT_DIR/vscode-extension"
npm run compile
npx @vscode/vsce package --allow-missing-repository

VSIX_FILE=$(ls -t *.vsix 2>/dev/null | head -1)
if [[ -n "$VSIX_FILE" ]] && command -v code &>/dev/null; then
    code --install-extension "$VSIX_FILE" --force
    success "VS Code Extension 已安裝"
else
    warn "請手動安裝 VS Code Extension: $PROJECT_DIR/vscode-extension/$VSIX_FILE"
fi

# ============================================================================
# 10. 安裝 Claude Code CLI（選用）
# ============================================================================
echo ""
read -p "是否要安裝 Claude Code CLI？(y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    info "安裝 Claude Code CLI..."
    npm install -g @anthropic-ai/claude-code
    success "Claude Code CLI 已安裝"
    echo ""
    warn "請執行 'claude' 並完成登入設定"
fi

# ============================================================================
# 完成
# ============================================================================
echo ""
echo "============================================"
echo -e "   ${GREEN}✅ 安裝完成！${NC}"
echo "============================================"
echo ""
echo "使用方式："
echo "  1. 開啟 VS Code"
echo "  2. 點擊編輯器右上角的 Tsunu Alive 圖示"
echo "  3. 或從 Spotlight 搜尋 'Tsunu Alive'"
echo ""
echo "祝你使用愉快！ 🎉"
echo ""
