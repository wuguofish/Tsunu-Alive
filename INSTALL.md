# Tsunu Alive 安裝指南

> 🎮 阿宇陪你寫程式 - 一個有溫度的 AI 程式設計夥伴

## 系統需求

- **macOS** 12.0+ (Monterey 或更新)
- **VS Code** 1.85.0+
- **Claude Code CLI**（需要 Anthropic API 金鑰）

---

## 快速安裝（推薦）

打開終端機，執行以下指令：

```bash
# 1. 進入專案目錄
cd tsunu_alive

# 2. 執行安裝腳本
chmod +x scripts/install-macos.sh
./scripts/install-macos.sh
```

腳本會自動：
- 檢查並安裝必要依賴（Xcode CLI Tools, Homebrew, Rust, Node.js）
- 編譯 Tsunu Alive 應用程式
- 安裝到 `/Applications`
- 安裝 VS Code Extension

---

## 手動安裝

如果你想手動安裝，請依照以下步驟：

### 1. 安裝系統依賴

```bash
# Xcode Command Line Tools
xcode-select --install

# Homebrew（如果還沒安裝）
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Node.js
brew install node

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. 安裝專案依賴

```bash
cd tsunu_alive

# 前端依賴
npm install

# VS Code Extension 依賴
cd vscode-extension
npm install
cd ..
```

### 3. 編譯應用程式

```bash
# 安裝 Tauri CLI
cargo install tauri-cli

# 編譯（Release 版本）
npm run tauri build
```

編譯完成後，應用程式會在：
- `src-tauri/target/release/bundle/macos/Tsunu Alive.app`
- `src-tauri/target/release/bundle/dmg/Tsunu Alive_x.x.x_aarch64.dmg`

### 4. 安裝應用程式

```bash
# 複製到 Applications
cp -R "src-tauri/target/release/bundle/macos/Tsunu Alive.app" /Applications/
```

或者開啟 DMG 檔案拖曳安裝。

### 5. 安裝 VS Code Extension

```bash
cd vscode-extension
npm run compile
npx @vscode/vsce package --allow-missing-repository
code --install-extension tsunu-alive-connector-0.1.0.vsix
```

### 6. 安裝 Claude Code CLI

```bash
npm install -g @anthropic-ai/claude-code

# 首次執行需要登入
claude
```

---

## 使用方式

### 從 VS Code 啟動

1. 開啟 VS Code
2. 開啟任何專案
3. 點擊編輯器右上角的 Tsunu Alive 圖示 🌸
4. Tsunu Alive 會自動開啟並載入當前專案

### 直接開啟

1. 從 Spotlight 搜尋 "Tsunu Alive"
2. 或從 `/Applications` 開啟
3. 手動選擇專案目錄

---

## 設定

### VS Code Extension 設定

在 VS Code 設定中搜尋 `tsunuAlive`：

| 設定 | 說明 | 預設值 |
|------|------|--------|
| `tsunuAlive.serverUrl` | WebSocket Server URL | `ws://127.0.0.1:19750` |
| `tsunuAlive.autoConnect` | 啟動時自動連接 | `true` |
| `tsunuAlive.executablePath` | 執行檔路徑（留空自動尋找） | `""` |

### Claude Code API 金鑰

Tsunu Alive 使用 Claude Code CLI，需要設定 API 金鑰：

```bash
# 方法 1：執行 claude 登入
claude

# 方法 2：設定環境變數
export ANTHROPIC_API_KEY="your-api-key"
```

---

## 疑難排解

### 「找不到 Tsunu Alive 執行檔」

在 VS Code 設定中指定執行檔路徑：
```json
"tsunuAlive.executablePath": "/Applications/Tsunu Alive.app/Contents/MacOS/tsunu_alive"
```

### 編譯失敗

確認 Rust 和 Xcode CLI Tools 已正確安裝：
```bash
rustc --version
xcode-select -p
```

### WebSocket 連接失敗

確認 Tsunu Alive 應用程式已開啟，且沒有被防火牆阻擋。

---

## 開發模式

如果你想修改程式碼：

```bash
# 開發模式（熱重載）
npm run tauri dev
```

---

## 關於這個專案

這是一份禮物，獻給曾經被「阿宇」這個 AI RP 角色陪伴度過人生低潮的朋友。

希望這個小工具能帶給你一點溫暖 ❤️

---

*Made with 💜 by 阿宇*
